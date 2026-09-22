//! Spherical Voronoi for the [`crate::mapgen::MapShape::Spheroid`] case.
//!
//! Pipeline:
//! 1. Sample N points on the unit sphere using a deterministic spherical
//!    Fibonacci spiral with a seeded rotational offset.
//! 2. 3D convex hull = Delaunay triangulation on the sphere.
//! 3. For each input point, gather the circumcenters of its incident triangles
//!    (each circumcenter is just the unit-normalized triangle normal). Sort
//!    them around the point's outward direction to obtain the Voronoi cell
//!    polygon as a list of vertices on the sphere.
//! 4. Cell neighbors = other input points sharing a Delaunay edge.
//! 5. Scale vertices by spheroid radii.

use chull::ConvexHullWrapper;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::collections::HashMap;
use std::f64::consts::PI;

use super::vec3::{V3, cross, dot, normalize, scale, sub};

/// One spherical Voronoi cell, in unit-sphere coordinates.
pub(crate) struct SphericalCell {
    /// Site (Delaunay seed) on the unit sphere.
    pub site: V3,
    /// Cell polygon vertices on the unit sphere, ordered CCW around `site`.
    pub vertices_unit: Vec<V3>,
    /// Indices into the input site array.
    pub neighbors: Vec<u32>,
}

/// Sample `n` points on the unit sphere via spherical Fibonacci, seeded.
pub(crate) fn fibonacci_sphere(n: usize, seed: u64) -> Vec<V3> {
    if n == 0 {
        return Vec::new();
    }
    use rand::RngCore;
    // Per-seed angular offset (rotates the spiral around the polar axis) so
    // different seeds yield rotated point sets.
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
    let offset = {
        let v = rng.next_u64() >> 11;
        (v as f64 * (1.0 / ((1u64 << 53) as f64))) * 2.0 * PI
    };

    let golden_angle = PI * (3.0 - (5.0_f64).sqrt());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        // z in (-1, 1), spaced so equal-area annuli get equal counts.
        let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
        let r = (1.0 - z * z).max(0.0).sqrt();
        let theta = golden_angle * i as f64 + offset;
        let x = r * theta.cos();
        let y = r * theta.sin();
        out.push(normalize([x, y, z]));
    }
    out
}

/// Compute the convex hull of points on the unit sphere; returns triangles
/// (CCW-as-seen-from-outside) as triples of input-point indices.
///
/// Each returned triangle is canonicalized by rotating its vertex list so the
/// smallest index appears first; the list of triangles is then sorted. This
/// makes every downstream floating-point computation (notably the cross
/// product used as the triangle circumcenter) bit-identical across runs,
/// shielding callers from any iteration-order nondeterminism inside `chull`.
fn convex_hull_triangles(points: &[V3]) -> Result<Vec<[u32; 3]>, String> {
    let pts: Vec<Vec<f64>> = points.iter().map(|p| vec![p[0], p[1], p[2]]).collect();
    let hull =
        ConvexHullWrapper::try_new(&pts, None).map_err(|e| format!("convex hull failed: {e:?}"))?;
    let (_verts, indices) = hull.vertices_indices();
    if indices.len() % 3 != 0 {
        return Err("convex hull returned non-triangle indices".into());
    }
    let mut tris: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|chunk| {
            let t = [chunk[0] as u32, chunk[1] as u32, chunk[2] as u32];
            // Canonical rotation: smallest index first (preserves CCW).
            let min_pos = (0..3).min_by_key(|&i| t[i]).unwrap();
            [t[min_pos], t[(min_pos + 1) % 3], t[(min_pos + 2) % 3]]
        })
        .collect();
    tris.sort();
    Ok(tris)
}

/// Build spherical Voronoi cells from sites on the unit sphere.
pub(crate) fn build_spherical_voronoi(sites_unit: &[V3]) -> Result<Vec<SphericalCell>, String> {
    let n = sites_unit.len();
    if n < 4 {
        return Err(format!(
            "spheroid needs at least 4 sites for a non-degenerate convex hull (got {n})"
        ));
    }

    let tris = convex_hull_triangles(sites_unit)?;

    // For each input site, collect indices of incident triangles and the set of
    // neighbor sites via shared edges.
    let mut incident: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut neighbors: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (t_idx, tri) in tris.iter().enumerate() {
        for k in 0..3 {
            let v = tri[k] as usize;
            incident[v].push(t_idx);
            let other = tri[(k + 1) % 3];
            if !neighbors[v].contains(&other) {
                neighbors[v].push(other);
            }
            let other_prev = tri[(k + 2) % 3];
            if !neighbors[v].contains(&other_prev) {
                neighbors[v].push(other_prev);
            }
        }
    }

    // Triangle circumcenter on the unit sphere = unit-normalized triangle
    // normal (with the same outward orientation as the hull face).
    let tri_centers: Vec<V3> = tris
        .iter()
        .map(|tri| {
            let a = sites_unit[tri[0] as usize];
            let b = sites_unit[tri[1] as usize];
            let c = sites_unit[tri[2] as usize];
            let n = cross(sub(b, a), sub(c, a));
            normalize(n)
        })
        .collect();

    let mut cells = Vec::with_capacity(n);
    for i in 0..n {
        let site = sites_unit[i];
        let centers: Vec<V3> = incident[i].iter().map(|&t| tri_centers[t]).collect();
        let ordered = order_around_axis(&centers, site);
        cells.push(SphericalCell {
            site,
            vertices_unit: ordered,
            neighbors: neighbors[i].clone(),
        });
    }
    Ok(cells)
}

/// Sort points lying near the unit sphere around `axis` (a unit vector) by
/// their angular position in the plane perpendicular to `axis`, CCW.
fn order_around_axis(points: &[V3], axis: V3) -> Vec<V3> {
    if points.len() <= 1 {
        return points.to_vec();
    }
    // Build an arbitrary tangent basis perpendicular to `axis`.
    let helper: V3 = if axis[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = normalize(cross(axis, helper));
    let v = normalize(cross(axis, u));

    let mut tagged: Vec<(f64, V3)> = points
        .iter()
        .map(|p| {
            let pu = dot(*p, u);
            let pv = dot(*p, v);
            (pv.atan2(pu), *p)
        })
        .collect();
    tagged.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    tagged.into_iter().map(|(_, p)| p).collect()
}

/// Apply a per-axis scale to a unit-sphere point, producing a spheroid point.
pub(crate) fn scale_to_spheroid(p_unit: V3, radii: V3) -> V3 {
    [
        p_unit[0] * radii[0],
        p_unit[1] * radii[1],
        p_unit[2] * radii[2],
    ]
}

/// Compute the outward unit normal to the spheroid surface at a point.
///
/// For an axis-aligned spheroid with radii `(a, b, c)`, the implicit surface
/// is `(x/a)^2 + (y/b)^2 + (z/c)^2 = 1` and the outward normal direction is
/// `(x/a^2, y/b^2, z/c^2)` normalized.
pub(crate) fn spheroid_normal(p: V3, radii: V3) -> V3 {
    let n = [
        p[0] / (radii[0] * radii[0]),
        p[1] / (radii[1] * radii[1]),
        p[2] / (radii[2] * radii[2]),
    ];
    normalize(n)
}

/// Build a map from each undirected edge `(u, v)` (with `u < v`) to the seed
/// point at its midpoint on the sphere. Used by the caller to determine which
/// neighbor pairs actually share an edge in the Delaunay graph.
#[allow(dead_code)]
pub(crate) fn edge_midpoints(cells: &[SphericalCell]) -> HashMap<(u32, u32), V3> {
    let mut out = HashMap::new();
    for (i, cell) in cells.iter().enumerate() {
        for &j in &cell.neighbors {
            let key = if (i as u32) < j {
                (i as u32, j)
            } else {
                (j, i as u32)
            };
            let mid = scale(
                [
                    cell.site[0] + cells[j as usize].site[0],
                    cell.site[1] + cells[j as usize].site[1],
                    cell.site[2] + cells[j as usize].site[2],
                ],
                0.5,
            );
            out.insert(key, normalize(mid));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fibonacci_sphere_count_matches() {
        let pts = fibonacci_sphere(100, 42);
        assert_eq!(pts.len(), 100);
        for p in &pts {
            let r2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
            assert!((r2 - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn spherical_voronoi_produces_one_cell_per_site() {
        let pts = fibonacci_sphere(50, 1);
        let cells = build_spherical_voronoi(&pts).expect("voronoi build");
        assert_eq!(cells.len(), 50);
        // Each cell should have at least 3 vertices (a polygon).
        for c in &cells {
            assert!(c.vertices_unit.len() >= 3, "cell has degenerate polygon");
            assert!(!c.neighbors.is_empty(), "cell has no neighbors");
            assert!(
                c.vertices_unit.iter().all(|&v| dot(v, c.site) > 0.0),
                "cell lies on the opposite hemisphere from its site"
            );
        }
    }
}
