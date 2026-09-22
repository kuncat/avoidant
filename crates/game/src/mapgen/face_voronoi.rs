//! Planar Voronoi cells clipped to each convex polyhedron face.

use bluenoise::BlueNoise;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::collections::BTreeMap;
use voronator::{VoronoiDiagram, delaunator::Point};

use super::shapes::Polyhedron;
use super::vec3::{V3, add, cross, dot, normalize, scale, sub};

const POISSON_MAX_SAMPLES: u32 = 20;

/// Per-face 2D frame: origin at face centroid, basis `(u_axis, v_axis)` in the
/// face plane, `normal` outward unit vector.
pub(crate) struct FaceFrame {
    pub origin: V3,
    pub u_axis: V3,
    pub v_axis: V3,
    pub normal: V3,
    /// Face polygon in 2D (CCW), one entry per face vertex.
    pub polygon_2d: Vec<(f64, f64)>,
    pub aabb_min: (f64, f64),
    pub aabb_max: (f64, f64),
}

impl FaceFrame {
    pub(crate) fn from_face(verts: &[V3], face: &[u32]) -> Self {
        let mut centroid = [0.0_f64; 3];
        for &i in face {
            centroid = add(centroid, verts[i as usize]);
        }
        centroid = scale(centroid, 1.0 / face.len() as f64);

        let v0 = verts[face[0] as usize];
        let v1 = verts[face[1] as usize];
        let v2 = verts[face[2] as usize];
        let e1 = sub(v1, v0);
        let e2 = sub(v2, v0);
        let normal = normalize(cross(e1, e2));
        let u_axis = normalize(e1);
        let v_axis = normalize(cross(normal, u_axis));

        let mut polygon_2d: Vec<(f64, f64)> = Vec::with_capacity(face.len());
        let mut aabb_min = (f64::INFINITY, f64::INFINITY);
        let mut aabb_max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for &i in face {
            let p = verts[i as usize];
            let local = sub(p, centroid);
            let u = dot(local, u_axis);
            let v = dot(local, v_axis);
            polygon_2d.push((u, v));
            if u < aabb_min.0 {
                aabb_min.0 = u;
            }
            if v < aabb_min.1 {
                aabb_min.1 = v;
            }
            if u > aabb_max.0 {
                aabb_max.0 = u;
            }
            if v > aabb_max.1 {
                aabb_max.1 = v;
            }
        }

        Self {
            origin: centroid,
            u_axis,
            v_axis,
            normal,
            polygon_2d,
            aabb_min,
            aabb_max,
        }
    }

    pub(crate) fn lift(&self, p2d: (f64, f64)) -> V3 {
        let u = scale(self.u_axis, p2d.0);
        let v = scale(self.v_axis, p2d.1);
        add(self.origin, add(u, v))
    }
}

/// Build the polyhedron's edge → adjacent-faces map.
///
/// Each undirected edge (a, b) (with `a < b`) maps to the (face_idx, edge_idx_within_face)
/// pairs that share it. For a manifold polyhedron each edge has exactly 2 entries.
pub(crate) fn build_edge_to_faces(poly: &Polyhedron) -> BTreeMap<(u32, u32), Vec<(u32, u32)>> {
    let mut map: BTreeMap<(u32, u32), Vec<(u32, u32)>> = BTreeMap::new();
    for (f_idx, face) in poly.faces.iter().enumerate() {
        let n = face.len();
        for k in 0..n {
            let a = face[k];
            let b = face[(k + 1) % n];
            let key = if a < b { (a, b) } else { (b, a) };
            map.entry(key).or_default().push((f_idx as u32, k as u32));
        }
    }
    map
}

/// Sample real Poisson-disk sites for one face in its 2D frame, deterministically.
pub(crate) fn sample_face_sites(
    frame: &FaceFrame,
    target_sites: usize,
    seed: u64,
) -> Vec<(f64, f64)> {
    if target_sites == 0 {
        return Vec::new();
    }

    let area = polygon_area(&frame.polygon_2d).abs().max(1e-6);
    let mut min_radius = ((area / target_sites as f64).sqrt() * 0.7).max(1e-3) as f32;

    let width = (frame.aabb_max.0 - frame.aabb_min.0) as f32;
    let height = (frame.aabb_max.1 - frame.aabb_min.1) as f32;
    let offset_u = frame.aabb_min.0;
    let offset_v = frame.aabb_min.1;

    for radius_try in 0..8u64 {
        let mut noise = BlueNoise::<Xoshiro256PlusPlus>::from_seed(
            width,
            height,
            min_radius,
            seed.wrapping_add(radius_try.wrapping_mul(0x9e37_79b9_7f4a_7c15)),
        );
        noise.with_samples(POISSON_MAX_SAMPLES);

        let mut points: Vec<(f64, f64)> = noise
            .filter_map(|point| {
                let p = (point.x as f64 + offset_u, point.y as f64 + offset_v);
                if point_in_convex_polygon(p, &frame.polygon_2d) {
                    Some(p)
                } else {
                    None
                }
            })
            .collect();

        if points.len() >= target_sites {
            let mut chooser = Xoshiro256PlusPlus::seed_from_u64(
                seed ^ (0xc2b2_ae3d_27d4_eb4fu64.wrapping_mul(radius_try + 1)),
            );
            points.shuffle(&mut chooser);
            points.truncate(target_sites);
            return points;
        }

        // Relax the spacing and try again.
        min_radius *= 0.8;
    }

    // Last resort: accept whatever we have.
    let mut noise =
        BlueNoise::<Xoshiro256PlusPlus>::from_seed(width, height, min_radius, seed ^ 0xDEAD_BEEF);
    noise.with_samples(POISSON_MAX_SAMPLES);
    noise
        .filter_map(|point| {
            let p = (point.x as f64 + offset_u, point.y as f64 + offset_v);
            if point_in_convex_polygon(p, &frame.polygon_2d) {
                Some(p)
            } else {
                None
            }
        })
        .collect()
}

/// Convex polygon containment test (CCW polygon).
fn point_in_convex_polygon(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let edge = (b.0 - a.0, b.1 - a.1);
        let to_p = (p.0 - a.0, p.1 - a.1);
        let cross = edge.0 * to_p.1 - edge.1 * to_p.0;
        if cross < -1e-9 {
            return false;
        }
    }
    true
}

/// Shoelace formula; positive for CCW input.
fn polygon_area(poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..n {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % n];
        sum += x1 * y2 - x2 * y1;
    }
    sum * 0.5
}

/// Sutherland-Hodgman polygon clipping. `clip` must be convex CCW.
pub(crate) fn clip_to_convex(subject: &[(f64, f64)], clip: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut output = subject.to_vec();
    let n = clip.len();
    for i in 0..n {
        if output.is_empty() {
            return output;
        }
        let a = clip[i];
        let b = clip[(i + 1) % n];
        let input = std::mem::take(&mut output);
        let m = input.len();
        for j in 0..m {
            let curr = input[j];
            let prev = input[(j + m - 1) % m];
            let inside_curr = side(a, b, curr) >= -1e-9;
            let inside_prev = side(a, b, prev) >= -1e-9;
            if inside_curr {
                if !inside_prev {
                    if let Some(x) = line_intersect(prev, curr, a, b) {
                        output.push(x);
                    }
                }
                output.push(curr);
            } else if inside_prev {
                if let Some(x) = line_intersect(prev, curr, a, b) {
                    output.push(x);
                }
            }
        }
    }
    output
}

fn side(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

fn line_intersect(
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    p4: (f64, f64),
) -> Option<(f64, f64)> {
    let d = (p1.0 - p2.0) * (p3.1 - p4.1) - (p1.1 - p2.1) * (p3.0 - p4.0);
    if d.abs() < 1e-12 {
        return None;
    }
    let t = ((p1.0 - p3.0) * (p3.1 - p4.1) - (p1.1 - p3.1) * (p3.0 - p4.0)) / d;
    Some((p1.0 + t * (p2.0 - p1.0), p1.1 + t * (p2.1 - p1.1)))
}

/// Per-face 2D Voronoi result for a single real site.
pub(crate) struct LocalCell {
    /// Clipped polygon in the face's 2D frame (CCW).
    pub polygon_2d: Vec<(f64, f64)>,
}

/// Partition real sites, then clip every cell to the face boundary.
fn compute_face_voronoi(frame: &FaceFrame, real_sites: &[(f64, f64)]) -> Option<Vec<LocalCell>> {
    let mut min = frame.aabb_min;
    let mut max = frame.aabb_max;
    let pad = ((max.0 - min.0) + (max.1 - min.1)) * 0.05 + 1e-3;
    min.0 -= pad;
    min.1 -= pad;
    max.0 += pad;
    max.1 += pad;

    let diagram = VoronoiDiagram::<Point>::from_tuple(&min, &max, real_sites)?;
    let cells = diagram.cells();
    let n_real = real_sites.len();

    let mut results: Vec<LocalCell> = Vec::with_capacity(n_real);
    for i in 0..n_real {
        let poly_input = cells[i].points();
        let raw_poly: Vec<(f64, f64)> = poly_input.iter().map(|p| (p.x, p.y)).collect();
        let clipped = clip_to_convex(&raw_poly, &frame.polygon_2d);

        results.push(LocalCell {
            polygon_2d: clipped,
        });
    }
    Some(results)
}

/// Build per-face Voronoi cells over **real sites only**, with each cell's
/// polygon clipped to the face polygon. Used for cell *geometry*: real cells
/// together fully tile the face polygon (unlike a real+ghost partition where
/// ghost cells can steal interior territory from real cells).
///
/// Handles the small-site corner cases that `voronator` doesn't:
/// * 0 sites → empty list.
/// * 1 site → one cell whose polygon is the entire face polygon.
/// * 2 sites → split the face polygon by the perpendicular bisector.
/// * 3+ sites → Voronoi, with half-plane clipping for degenerate layouts.
pub(crate) fn real_only_face_cells(frame: &FaceFrame, real_sites: &[(f64, f64)]) -> Vec<LocalCell> {
    match real_sites.len() {
        0 => Vec::new(),
        1 => vec![LocalCell {
            polygon_2d: frame.polygon_2d.clone(),
        }],
        2 => {
            let a = real_sites[0];
            let b = real_sites[1];
            let cell_a = clip_by_bisector(&frame.polygon_2d, a, b);
            let cell_b = clip_by_bisector(&frame.polygon_2d, b, a);
            vec![
                LocalCell { polygon_2d: cell_a },
                LocalCell { polygon_2d: cell_b },
            ]
        }
        _ => compute_face_voronoi(frame, real_sites).unwrap_or_else(|| {
            // Half-plane clipping also handles collinear sites without
            // creating empty, unreachable game cells.
            real_sites
                .iter()
                .enumerate()
                .map(|(i, &site)| {
                    let mut polygon = frame.polygon_2d.clone();
                    for (j, &other) in real_sites.iter().enumerate() {
                        if i != j {
                            polygon = clip_by_bisector(&polygon, site, other);
                        }
                    }
                    LocalCell {
                        polygon_2d: polygon,
                    }
                })
                .collect()
        }),
    }
}

/// Clip `poly` (convex CCW) to the half-plane on `keep`'s side of the
/// perpendicular bisector between `keep` and `against`.
fn clip_by_bisector(poly: &[(f64, f64)], keep: (f64, f64), against: (f64, f64)) -> Vec<(f64, f64)> {
    // Perpendicular bisector: midpoint and direction perpendicular to (against - keep).
    let mid = ((keep.0 + against.0) * 0.5, (keep.1 + against.1) * 0.5);
    let dx = against.0 - keep.0;
    let dy = against.1 - keep.1;
    // Bisector line points: mid and mid + (-dy, dx).
    // For Sutherland-Hodgman with `clip` expected CCW, we feed an edge
    // (a, b) such that "inside" (cross > 0) corresponds to the `keep` side.
    // The keep side is where (p - mid) · (-dx, -dy) > 0, i.e., the side away
    // from `against`. Construct edge a→b so cross((b-a), (p-a)) > 0 there.
    let a = (mid.0 + dy, mid.1 - dx);
    let b = (mid.0 - dy, mid.1 + dx);
    let mut out: Vec<(f64, f64)> = Vec::new();
    let n = poly.len();
    if n < 3 {
        return out;
    }
    let inside = |p: (f64, f64)| -> bool {
        // cross((b-a), (p-a)) >= 0  →  on `keep` side.
        let ex = b.0 - a.0;
        let ey = b.1 - a.1;
        let px = p.0 - a.0;
        let py = p.1 - a.1;
        ex * py - ey * px >= -1e-12
    };
    // Sutherland-Hodgman single half-plane.
    for j in 0..n {
        let curr = poly[j];
        let prev = poly[(j + n - 1) % n];
        let inside_curr = inside(curr);
        let inside_prev = inside(prev);
        if inside_curr {
            if !inside_prev {
                // Compute intersection of segment prev→curr with line a→b.
                let d = (prev.0 - curr.0) * (a.1 - b.1) - (prev.1 - curr.1) * (a.0 - b.0);
                if d.abs() >= 1e-12 {
                    let t = ((prev.0 - a.0) * (a.1 - b.1) - (prev.1 - a.1) * (a.0 - b.0)) / d;
                    out.push((
                        prev.0 + t * (curr.0 - prev.0),
                        prev.1 + t * (curr.1 - prev.1),
                    ));
                }
            }
            out.push(curr);
        } else if inside_prev {
            let d = (prev.0 - curr.0) * (a.1 - b.1) - (prev.1 - curr.1) * (a.0 - b.0);
            if d.abs() >= 1e-12 {
                let t = ((prev.0 - a.0) * (a.1 - b.1) - (prev.1 - a.1) * (a.0 - b.0)) / d;
                out.push((
                    prev.0 + t * (curr.0 - prev.0),
                    prev.1 + t * (curr.1 - prev.1),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collinear_sites_keep_every_cell_playable() {
        let poly = super::super::shapes::cube(10.0);
        let frame = FaceFrame::from_face(&poly.vertices, &poly.faces[0]);
        let cells = real_only_face_cells(&frame, &[(-2.0, 0.0), (0.0, 0.0), (2.0, 0.0)]);
        assert_eq!(cells.len(), 3);
        assert!(
            cells
                .iter()
                .all(|c| polygon_area(&c.polygon_2d).abs() > 1.0)
        );
        let area: f64 = cells
            .iter()
            .map(|c| polygon_area(&c.polygon_2d).abs())
            .sum();
        assert!((area - polygon_area(&frame.polygon_2d).abs()).abs() < 1e-6);
    }

    #[test]
    fn sutherland_hodgman_clips_outside_to_empty() {
        let square = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let outside = vec![(10.0, 10.0), (11.0, 10.0), (11.0, 11.0), (10.0, 11.0)];
        assert!(clip_to_convex(&outside, &square).is_empty());
    }

    #[test]
    fn sutherland_hodgman_keeps_interior_unchanged() {
        let square = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let inner = vec![(0.2, 0.2), (0.8, 0.2), (0.8, 0.8), (0.2, 0.8)];
        let clipped = clip_to_convex(&inner, &square);
        assert_eq!(clipped.len(), 4);
    }
}
