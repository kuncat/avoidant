//! Shape-aware map generation. Polyhedral faces are tiled independently,
//! then stitched at common edge breakpoints. Adjacency uses visible shared
//! edges. Terrain displacement is radial and position-dependent so the same
//! surface point moves identically on both sides of every seam.

mod face_voronoi;
mod noise;
mod shapes;
mod sphere_voronoi;
mod terrain;
mod vec3;

use serde::{Deserialize, Serialize};
use tsify::Tsify;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{GameOptions, MapCell, MapData};

use face_voronoi::{FaceFrame, build_edge_to_faces, real_only_face_cells, sample_face_sites};
use shapes::Polyhedron;
use sphere_voronoi::{
    build_spherical_voronoi, fibonacci_sphere, scale_to_spheroid, spheroid_normal,
};
use terrain::{MeshCell, generate_terrain_triangles};
use vec3::V3;

/// Map surface shape. Serialized as a tagged JS union; on the TS side this
/// becomes a discriminated union by `kind`.
#[derive(Clone, Debug, Serialize, Deserialize, Tsify)]
#[tsify(from_wasm_abi, into_wasm_abi)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MapShape {
    Flat {
        radius: f64,
    },
    Tetrahedron {
        radius: f64,
    },
    Cube {
        radius: f64,
    },
    Octahedron {
        radius: f64,
    },
    Dodecahedron {
        radius: f64,
    },
    Icosahedron {
        radius: f64,
    },
    GeodesicIcosphere {
        radius: f64,
        subdivisions: u32,
    },
    Spheroid {
        radius_x: f64,
        radius_y: f64,
        radius_z: f64,
    },
}

impl Default for MapShape {
    fn default() -> Self {
        MapShape::Icosahedron { radius: 50.0 }
    }
}

impl MapShape {
    /// Maximum radial extent from origin (used by the renderer for camera framing).
    pub fn bounds_radius(&self) -> f64 {
        match *self {
            MapShape::Flat { radius } => radius * 2.0_f64.sqrt(),
            MapShape::Tetrahedron { radius }
            | MapShape::Cube { radius }
            | MapShape::Octahedron { radius }
            | MapShape::Dodecahedron { radius }
            | MapShape::Icosahedron { radius }
            | MapShape::GeodesicIcosphere { radius, .. } => radius,
            MapShape::Spheroid {
                radius_x,
                radius_y,
                radius_z,
            } => radius_x.max(radius_y).max(radius_z),
        }
    }
}

/// Generated map plus shape-derived metadata used by the renderer.
pub(crate) struct GeneratedMap {
    pub cells: Vec<MapCell>,
    pub surface_area: f64,
    pub bounds_radius: f64,
}

/// Build map cells and the subdivided terrain triangle mesh.
#[wasm_bindgen(js_name = "generateMapData")]
pub fn generate_map_data_js(options: GameOptions) -> Result<MapData, JsValue> {
    let shape = options.shape.clone().unwrap_or_default();
    let spikiness = options.spikiness.unwrap_or(0.4).clamp(0.0, 1.0);
    let elev_min = options.elevation_min.unwrap_or(-0.4);
    let elev_max = options.elevation_max.unwrap_or(0.4);
    let subdivisions = options.terrain_subdivisions.unwrap_or(4).clamp(1, 16);

    let generated = generate_map(options.num_cells as usize, options.rng_seed, &shape)
        .map_err(|err| JsValue::from_str(&err))?;

    // Map cells retain the undisplaced surface geometry.
    let mesh_cells: Vec<MeshCell> = generated
        .cells
        .iter()
        .map(|c| MeshCell {
            vertices: c.vertices.clone(),
            normal: c.normal,
            centroid: c.centroid,
        })
        .collect();

    let terrain = generate_terrain_triangles(
        &mesh_cells,
        options.rng_seed,
        spikiness,
        (elev_min, elev_max),
        subdivisions,
        matches!(shape, MapShape::Flat { .. }),
    );

    Ok(MapData {
        cells: generated.cells,
        terrain,
        surface_area: generated.surface_area,
        bounds_radius: generated.bounds_radius,
    })
}

pub(crate) fn generate_map(
    target_cells: usize,
    rng_seed: u64,
    shape: &MapShape,
) -> Result<GeneratedMap, String> {
    match shape {
        MapShape::Flat { radius } => {
            if !radius.is_finite() || *radius <= 0.0 || target_cells == 0 {
                return Err("flat maps need a positive radius and at least one cell".into());
            }
            let r = *radius;
            generate_polyhedron_map(
                target_cells,
                rng_seed,
                Polyhedron {
                    vertices: vec![[-r, 0.0, -r], [-r, 0.0, r], [r, 0.0, r], [r, 0.0, -r]],
                    faces: vec![vec![0, 1, 2, 3]],
                },
            )
        }
        MapShape::Spheroid {
            radius_x,
            radius_y,
            radius_z,
        } => generate_spheroid_map(target_cells, rng_seed, [*radius_x, *radius_y, *radius_z]),
        MapShape::Tetrahedron { radius } => {
            generate_polyhedron_map(target_cells, rng_seed, shapes::tetrahedron(*radius))
        }
        MapShape::Octahedron { radius } => {
            generate_polyhedron_map(target_cells, rng_seed, shapes::octahedron(*radius))
        }
        MapShape::Cube { radius } => {
            generate_polyhedron_map(target_cells, rng_seed, shapes::cube(*radius))
        }
        MapShape::Icosahedron { radius } => {
            generate_polyhedron_map(target_cells, rng_seed, shapes::icosahedron(*radius))
        }
        MapShape::Dodecahedron { radius } => {
            generate_polyhedron_map(target_cells, rng_seed, shapes::dodecahedron(*radius))
        }
        MapShape::GeodesicIcosphere {
            radius,
            subdivisions,
        } => generate_polyhedron_map(
            target_cells,
            rng_seed,
            shapes::geodesic_icosphere(*radius, *subdivisions),
        ),
    }
}

// --- Polyhedron pipeline ----------------------------------------------------

fn generate_polyhedron_map(
    target_cells: usize,
    rng_seed: u64,
    poly: Polyhedron,
) -> Result<GeneratedMap, String> {
    let face_frames: Vec<FaceFrame> = poly
        .faces
        .iter()
        .map(|f| FaceFrame::from_face(&poly.vertices, f))
        .collect();

    let face_areas: Vec<f64> = face_frames
        .iter()
        .map(|f| polygon_area_abs(&f.polygon_2d))
        .collect();
    let total_area: f64 = face_areas.iter().sum();
    if total_area <= 0.0 {
        return Err("polyhedron has degenerate faces".into());
    }

    // Reserve one cell per face, then distribute the remainder by area.
    // Largest remainders keep the total exact instead of independently rounding.
    let remaining = target_cells.saturating_sub(face_areas.len());
    let quotas: Vec<f64> = face_areas
        .iter()
        .map(|area| area / total_area * remaining as f64)
        .collect();
    let mut cells_per_face: Vec<usize> = quotas
        .iter()
        .map(|quota| 1 + quota.floor() as usize)
        .collect();
    let mut order: Vec<usize> = (0..face_areas.len()).collect();
    order.sort_by(|&a, &b| {
        quotas[b]
            .fract()
            .total_cmp(&quotas[a].fract())
            .then(a.cmp(&b))
    });
    let allocated: usize = cells_per_face.iter().sum();
    for &index in order.iter().take(target_cells.saturating_sub(allocated)) {
        cells_per_face[index] += 1;
    }

    // Sample real sites per face.
    let real_sites_per_face: Vec<Vec<(f64, f64)>> = face_frames
        .iter()
        .enumerate()
        .map(|(f_idx, frame)| {
            let face_seed =
                rng_seed.wrapping_add((f_idx as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
            sample_face_sites(frame, cells_per_face[f_idx], face_seed)
        })
        .collect();

    let mut cells = Vec::new();
    let mut face_ranges = Vec::new();
    for (frame, sites) in face_frames.iter().zip(&real_sites_per_face) {
        let start = cells.len();
        for cell in real_only_face_cells(frame, sites) {
            let (centroid, vertices) = build_cell_geometry(frame, &cell.polygon_2d);
            cells.push(MapCell::new(vertices, Vec::new(), centroid, frame.normal));
        }
        face_ranges.push(start..cells.len());
    }

    // Both faces must use every breakpoint on their shared edge. Otherwise
    // independent subdivisions form T-junctions that open up under noise.
    for ((va, vb), faces) in build_edge_to_faces(&poly) {
        let a = poly.vertices[va as usize];
        let delta = vec3::sub(poly.vertices[vb as usize], a);
        let length2 = vec3::dot(delta, delta);
        let parameter = |p: V3| {
            let t = vec3::dot(vec3::sub(p, a), delta) / length2;
            let projected = vec3::add(a, vec3::scale(delta, t));
            (vec3::length(vec3::sub(projected, p)) < 1e-7 && (-1e-8..=1.0 + 1e-8).contains(&t))
                .then_some(t.clamp(0.0, 1.0))
        };
        let mut breaks = vec![0.0, 1.0];
        for &(face, _) in &faces {
            for idx in face_ranges[face as usize].clone() {
                breaks.extend(cells[idx].vertices.iter().filter_map(|&p| parameter(p)));
            }
        }
        breaks.sort_by(f64::total_cmp);
        breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        for &(face, _) in &faces {
            for idx in face_ranges[face as usize].clone() {
                let old = &cells[idx].vertices;
                let mut vertices = Vec::new();
                for k in 0..old.len() {
                    vertices.push(old[k]);
                    if let (Some(t0), Some(t1)) =
                        (parameter(old[k]), parameter(old[(k + 1) % old.len()]))
                    {
                        let mut interior: Vec<_> = breaks
                            .iter()
                            .copied()
                            .filter(|t| *t > t0.min(t1) + 1e-9 && *t < t0.max(t1) - 1e-9)
                            .collect();
                        if t1 < t0 {
                            interior.reverse();
                        }
                        vertices.extend(
                            interior
                                .into_iter()
                                .map(|t| vec3::add(a, vec3::scale(delta, t))),
                        );
                    }
                }
                cells[idx].vertices = vertices;
            }
        }
    }
    // Canonicalize shared coordinates and derive reciprocal adjacency from
    // the polygons actually drawn, including clipped edges within each face.
    let key = |p: V3| p.map(|v| (v * 1e6).round() as i64);
    let mut vertices = std::collections::HashMap::new();
    let mut edges = std::collections::HashMap::new();
    let mut pairs = Vec::new();
    for (i, cell) in cells.iter_mut().enumerate() {
        for p in &mut cell.vertices {
            *p = *vertices.entry(key(*p)).or_insert(*p);
        }
        cell.vertices.dedup_by(|a, b| key(*a) == key(*b));
        if cell.vertices.len() > 1 && key(cell.vertices[0]) == key(*cell.vertices.last().unwrap()) {
            cell.vertices.pop();
        }
        for k in 0..cell.vertices.len() {
            let a = key(cell.vertices[k]);
            let b = key(cell.vertices[(k + 1) % cell.vertices.len()]);
            let edge = if a < b { (a, b) } else { (b, a) };
            if let Some(j) = edges.insert(edge, i) {
                pairs.push((i, j));
            }
        }
    }
    for (i, j) in pairs {
        cells[i].neighbors.push(j as u32);
        cells[j].neighbors.push(i as u32);
    }
    for cell in &mut cells {
        cell.neighbors.sort_unstable();
        cell.neighbors.dedup();
    }

    Ok(GeneratedMap {
        cells,
        surface_area: total_area,
        bounds_radius: shape_bounds_radius_from_verts(&poly.vertices),
    })
}

/// Lift a cell polygon's 2D vertices onto the face plane (un-displaced) and
/// return them along with the polygon's centroid on the same plane.
///
/// Noise displacement is intentionally applied later, only inside
/// [`generate_terrain_triangles`], so a cell's vertices and centroid stay on
/// the un-displaced surface. Applying the displacement here as well would
/// double-displace every interior sub-triangle vertex (interpolation between
/// already-displaced apex+corners + a second noise lookup), shattering the
/// surface into disconnected shards.
fn build_cell_geometry(frame: &FaceFrame, poly_2d: &[(f64, f64)]) -> (V3, Vec<V3>) {
    let mut sum2d = (0.0, 0.0);
    let n = poly_2d.len().max(1);
    for p in poly_2d {
        sum2d.0 += p.0;
        sum2d.1 += p.1;
    }
    let centroid_2d = (sum2d.0 / n as f64, sum2d.1 / n as f64);
    let centroid = frame.lift(centroid_2d);
    let vertices: Vec<V3> = poly_2d.iter().map(|p| frame.lift(*p)).collect();
    (centroid, vertices)
}

fn polygon_area_abs(poly: &[(f64, f64)]) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..poly.len() {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % poly.len()];
        sum += x1 * y2 - x2 * y1;
    }
    (sum * 0.5).abs()
}

fn shape_bounds_radius_from_verts(verts: &[V3]) -> f64 {
    let mut max = 0.0_f64;
    for v in verts {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if len > max {
            max = len;
        }
    }
    max
}

// --- Spheroid pipeline ------------------------------------------------------

fn generate_spheroid_map(
    target_cells: usize,
    rng_seed: u64,
    radii: V3,
) -> Result<GeneratedMap, String> {
    let n = target_cells.max(4);
    let sites = fibonacci_sphere(n, rng_seed);
    let s_cells = build_spherical_voronoi(&sites)?;

    // Approximate surface area of the spheroid using Knud Thomsen's formula.
    let p = 1.6075;
    let a = radii[0];
    let b = radii[1];
    let c = radii[2];
    let surface_area = 4.0
        * std::f64::consts::PI
        * (((a * b).powf(p) + (a * c).powf(p) + (b * c).powf(p)) / 3.0).powf(1.0 / p);
    let bounds_radius = a.max(b).max(c);

    let mut cells: Vec<MapCell> = Vec::with_capacity(s_cells.len());
    for cell in &s_cells {
        // Un-displaced spheroid-surface positions; noise displacement is
        // applied later by `generate_terrain_triangles` (see
        // `build_cell_geometry` for the rationale).
        let centroid = scale_to_spheroid(cell.site, radii);
        let normal = spheroid_normal(centroid, radii);
        let vertices: Vec<V3> = cell
            .vertices_unit
            .iter()
            .map(|p_unit| scale_to_spheroid(*p_unit, radii))
            .collect();

        cells.push(MapCell::new(
            vertices,
            cell.neighbors.clone(),
            centroid,
            normal,
        ));
    }

    Ok(GeneratedMap {
        cells,
        surface_area,
        bounds_radius,
    })
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn size_presets_have_exact_cell_counts_across_shapes() {
        for count in [80, 160, 320] {
            for shape in [
                MapShape::Flat { radius: 50.0 },
                MapShape::Tetrahedron { radius: 50.0 },
                MapShape::Cube { radius: 50.0 },
                MapShape::Octahedron { radius: 50.0 },
                MapShape::Dodecahedron { radius: 50.0 },
                MapShape::Icosahedron { radius: 50.0 },
                MapShape::GeodesicIcosphere {
                    radius: 50.0,
                    subdivisions: if count >= 320 { 1 } else { 0 },
                },
                MapShape::Spheroid {
                    radius_x: 60.0,
                    radius_y: 40.0,
                    radius_z: 50.0,
                },
            ] {
                let map = generate_map(count, 42, &shape).unwrap();
                assert_eq!(map.cells.len(), count, "{shape:?}");
            }
        }
    }

    #[test]
    fn flat_map_tiles_square_with_connected_neighbors_and_vertical_bumps() {
        let shape = MapShape::Flat { radius: 50.0 };
        for count in [1, 2, 80, 160] {
            let map = generate_map(count, 42, &shape).unwrap();
            let repeat = generate_map(count, 42, &shape).unwrap();
            assert_eq!(map.cells.len(), count);
            assert_eq!(map.surface_area, 10000.0);
            let mut area = 0.0;
            let mut seen = std::collections::BTreeSet::new();
            let mut pending = vec![0];
            while let Some(i) = pending.pop() {
                if seen.insert(i) {
                    pending.extend(map.cells[i].neighbors.iter().map(|&n| n as usize));
                }
            }
            assert_eq!(seen.len(), count);
            for (i, cell) in map.cells.iter().enumerate() {
                assert_eq!(cell.normal, [0.0, 1.0, 0.0]);
                assert_eq!(cell.vertices, repeat.cells[i].vertices);
                assert_eq!(cell.neighbors, repeat.cells[i].neighbors);
                for &n in &cell.neighbors {
                    assert!(map.cells[n as usize].neighbors.contains(&(i as u32)));
                }
                for p in &cell.vertices {
                    assert_eq!(p[1], 0.0);
                }
                for k in 1..cell.vertices.len() - 1 {
                    area += vec3::length(vec3::cross(
                        vec3::sub(cell.vertices[k], cell.vertices[0]),
                        vec3::sub(cell.vertices[k + 1], cell.vertices[0]),
                    )) * 0.5;
                }
            }
            assert!((area - map.surface_area).abs() < 0.01);
            let mesh: Vec<_> = map
                .cells
                .iter()
                .map(|c| MeshCell {
                    vertices: c.vertices.clone(),
                    centroid: c.centroid,
                    normal: c.normal,
                })
                .collect();
            let flat = generate_terrain_triangles(&mesh, 42, 0.8, (0.0, 6.0), 4, true);
            let base = generate_terrain_triangles(&mesh, 42, 0.8, (0.0, 0.0), 4, true);
            assert!(flat.heights.iter().any(|&h| h > 0.1));
            for ((p, b), h) in flat
                .positions
                .chunks_exact(3)
                .zip(base.positions.chunks_exact(3))
                .zip(&flat.heights)
            {
                assert_eq!(p[0], b[0]);
                assert_eq!(p[2], b[2]);
                assert_eq!(p[1], *h);
            }
        }
    }

    fn shapes_to_test() -> Vec<MapShape> {
        vec![
            MapShape::Tetrahedron { radius: 50.0 },
            MapShape::Cube { radius: 50.0 },
            MapShape::Octahedron { radius: 50.0 },
            MapShape::Dodecahedron { radius: 50.0 },
            MapShape::Icosahedron { radius: 50.0 },
            MapShape::GeodesicIcosphere {
                radius: 50.0,
                subdivisions: 1,
            },
            MapShape::Spheroid {
                radius_x: 50.0,
                radius_y: 40.0,
                radius_z: 30.0,
            },
        ]
    }

    #[test]
    fn all_shapes_have_closed_meshes_and_matching_neighbors() {
        use std::collections::{BTreeMap, BTreeSet};
        for shape in shapes_to_test() {
            for count in [12, 160] {
                let map = generate_map(count, 1337, &shape).unwrap();
                let mut edges = BTreeMap::<_, Vec<usize>>::new();
                let key = |p: V3| p.map(|v| (v * 1e6).round() as i64);
                for (i, cell) in map.cells.iter().enumerate() {
                    assert!(cell.vertices.len() >= 3);
                    for k in 0..cell.vertices.len() {
                        let a = key(cell.vertices[k]);
                        let b = key(cell.vertices[(k + 1) % cell.vertices.len()]);
                        assert_ne!(a, b);
                        edges
                            .entry(if a < b { (a, b) } else { (b, a) })
                            .or_default()
                            .push(i);
                    }
                }
                let mut expected = vec![BTreeSet::new(); map.cells.len()];
                for owners in edges.values() {
                    assert_eq!(owners.len(), 2, "open edge on {shape:?}: {owners:?}");
                    expected[owners[0]].insert(owners[1] as u32);
                    expected[owners[1]].insert(owners[0] as u32);
                }
                for (cell, neighbors) in map.cells.iter().zip(expected) {
                    assert_eq!(
                        cell.neighbors.iter().copied().collect::<BTreeSet<_>>(),
                        neighbors
                    );
                }
                let mut visited = BTreeSet::new();
                let mut pending = vec![0];
                while let Some(i) = pending.pop() {
                    if visited.insert(i) {
                        pending.extend(map.cells[i].neighbors.iter().map(|&n| n as usize));
                    }
                }
                assert_eq!(visited.len(), map.cells.len(), "disconnected game graph");

                let mesh: Vec<_> = map
                    .cells
                    .iter()
                    .map(|c| MeshCell {
                        vertices: c.vertices.clone(),
                        centroid: c.centroid,
                        normal: c.normal,
                    })
                    .collect();
                let terrain = generate_terrain_triangles(&mesh, 1337, 0.8, (0.0, 6.0), 4, false);
                let mut triangle_edges = BTreeMap::<_, usize>::new();
                for tri in terrain.positions.chunks_exact(9) {
                    let points: Vec<_> = tri
                        .chunks_exact(3)
                        .map(|p| {
                            p.iter()
                                .map(|v| (v * 1e4).round() as i64)
                                .collect::<Vec<_>>()
                        })
                        .collect();
                    if points[0] == points[1] || points[1] == points[2] || points[2] == points[0] {
                        continue;
                    }
                    for k in 0..3 {
                        let a = &points[k];
                        let b = &points[(k + 1) % 3];
                        *triangle_edges
                            .entry(if a < b {
                                (a.clone(), b.clone())
                            } else {
                                (b.clone(), a.clone())
                            })
                            .or_default() += 1;
                    }
                }
                assert!(
                    triangle_edges.values().all(|&n| n == 2),
                    "terrain cracks on {shape:?}: {:?}",
                    triangle_edges
                        .iter()
                        .filter(|(_, n)| **n != 2)
                        .take(8)
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn all_shapes_repeat_cell_ids_geometry_and_neighbors() {
        for shape in shapes_to_test() {
            let a = generate_map(160, 9, &shape).unwrap();
            let b = generate_map(160, 9, &shape).unwrap();
            assert_eq!(a.cells.len(), b.cells.len());
            for (a, b) in a.cells.iter().zip(&b.cells) {
                assert_eq!(a.vertices, b.vertices, "{shape:?}");
                assert_eq!(a.centroid, b.centroid);
                assert_eq!(a.neighbors, b.neighbors);
            }
        }
    }

    fn count_cross_face_neighbors(cells: &[MapCell]) -> usize {
        let mut sum = 0;
        for cell in cells {
            sum += cell.neighbors().len();
        }
        sum
    }

    #[test]
    fn icosahedron_pipeline_produces_cells_and_neighbors() {
        let map = generate_map(60, 1337, &MapShape::Icosahedron { radius: 10.0 })
            .expect("icosahedron pipeline");
        assert!(
            map.cells.len() >= 20,
            "expected >= 1 cell per face, got {}",
            map.cells.len()
        );
        // Every cell should have at least one neighbor and a non-degenerate polygon.
        for (i, c) in map.cells.iter().enumerate() {
            assert!(!c.neighbors().is_empty(), "cell {i} has no neighbors");
            assert!(
                c.vertex_xyz().count() >= 3,
                "cell {i} polygon is degenerate"
            );
        }
        // Cross-face stitching should produce *some* neighbor edges (a cell on a
        // face boundary picks up a neighbor on the adjacent face). Naively each
        // face's cells would have only same-face neighbors; cross-face linking
        // should make total degree exceed e.g. 3 * cell_count.
        let degree_sum = count_cross_face_neighbors(&map.cells);
        assert!(
            degree_sum * 2 >= 7 * map.cells.len(),
            "low total neighbor degree {degree_sum} for {} cells",
            map.cells.len()
        );
    }

    #[test]
    fn spheroid_pipeline_is_deterministic() {
        let m1 = generate_map(
            120,
            99,
            &MapShape::Spheroid {
                radius_x: 8.0,
                radius_y: 10.0,
                radius_z: 6.0,
            },
        )
        .expect("spheroid pipeline #1");
        let m2 = generate_map(
            120,
            99,
            &MapShape::Spheroid {
                radius_x: 8.0,
                radius_y: 10.0,
                radius_z: 6.0,
            },
        )
        .expect("spheroid pipeline #2");
        assert_eq!(m1.cells.len(), m2.cells.len());
        for (a, b) in m1.cells.iter().zip(m2.cells.iter()) {
            let va: Vec<[f64; 3]> = a.vertex_xyz().collect();
            let vb: Vec<[f64; 3]> = b.vertex_xyz().collect();
            assert_eq!(va, vb);
            assert_eq!(a.neighbors(), b.neighbors());
        }
    }

    /// Clipped cells must cover every face without holes or overlaps.
    #[test]
    fn polyhedron_cells_cover_their_face() {
        // 60 cells on a cube → 10 per face, easy to verify against the
        // analytic face area (cube side = 2*r/sqrt(3) for our circumscribed
        // construction).
        let radius = 50.0_f64;
        let map = generate_map(60, 1337, &MapShape::Cube { radius }).expect("cube pipeline");
        let face_area_each = (2.0 * radius / 3.0_f64.sqrt()).powi(2);
        let expected_total = face_area_each * 6.0;
        let mut total_cell_area = 0.0_f64;
        for c in map.cells.iter() {
            let verts: Vec<[f64; 3]> = c.vertex_xyz().collect();
            let n = verts.len();
            if n < 3 {
                continue;
            }
            for i in 1..(n - 1) {
                let e1 = [
                    verts[i][0] - verts[0][0],
                    verts[i][1] - verts[0][1],
                    verts[i][2] - verts[0][2],
                ];
                let e2 = [
                    verts[i + 1][0] - verts[0][0],
                    verts[i + 1][1] - verts[0][1],
                    verts[i + 1][2] - verts[0][2],
                ];
                let cr = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                total_cell_area += (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt() * 0.5;
            }
        }
        let coverage = total_cell_area / expected_total;
        // With real-only Voronoi for geometry, real cells must fully tile
        // every face; tiny rounding from polygon clipping gives ~0.999.
        assert!(
            coverage >= 0.999,
            "cell coverage too low: {:.3}% (cell_area={:.1}, face_total={:.1}) — real-only Voronoi must fully tile the face polygon",
            coverage * 100.0,
            total_cell_area,
            expected_total
        );
    }

    /// Regression: per-face coverage must also be uniform — every face's
    /// real cells together cover that face. Catches a partial-coverage
    /// regression where, e.g., one face's cells get clipped short.
    #[test]
    fn icosahedron_each_face_is_fully_tiled() {
        let radius = 50.0_f64;
        let map = generate_map(160, 1337, &MapShape::Icosahedron { radius })
            .expect("icosahedron pipeline");
        // Group cells by face via their normal (each face has a unique
        // outward normal).
        let mut face_normals: Vec<[f64; 3]> = Vec::new();
        let mut face_of: Vec<usize> = Vec::with_capacity(map.cells.len());
        for c in map.cells.iter() {
            let n = c.normal;
            let idx = face_normals
                .iter()
                .position(|fn_| {
                    (n[0] - fn_[0]).powi(2) + (n[1] - fn_[1]).powi(2) + (n[2] - fn_[2]).powi(2)
                        < 1e-6
                })
                .unwrap_or_else(|| {
                    face_normals.push(n);
                    face_normals.len() - 1
                });
            face_of.push(idx);
        }
        assert_eq!(
            face_normals.len(),
            20,
            "icosahedron should have 20 distinct face normals"
        );
        let mut per_face_area = vec![0.0_f64; face_normals.len()];
        for (i, c) in map.cells.iter().enumerate() {
            let verts: Vec<[f64; 3]> = c.vertex_xyz().collect();
            if verts.len() < 3 {
                continue;
            }
            for k in 1..(verts.len() - 1) {
                let e1 = [
                    verts[k][0] - verts[0][0],
                    verts[k][1] - verts[0][1],
                    verts[k][2] - verts[0][2],
                ];
                let e2 = [
                    verts[k + 1][0] - verts[0][0],
                    verts[k + 1][1] - verts[0][1],
                    verts[k + 1][2] - verts[0][2],
                ];
                let cr = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                per_face_area[face_of[i]] +=
                    (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt() * 0.5;
            }
        }
        let edge_len = 4.0 * radius / (10.0_f64 + 2.0 * 5.0_f64.sqrt()).sqrt();
        let expected = 3.0_f64.sqrt() / 4.0 * edge_len * edge_len;
        for (fi, &area) in per_face_area.iter().enumerate() {
            let cov = area / expected;
            assert!(
                cov >= 0.999,
                "face {fi} coverage too low: {:.3}% (area={area:.2}, expected={expected:.2})",
                cov * 100.0
            );
        }
    }
}
