use bluenoise::BlueNoise;
use rand::RngCore;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_xoshiro::Xoshiro256PlusPlus;
use voronator::{VoronoiDiagram, delaunator::Point};
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{GameOptions, MapCell, MapData, TerrainTriangles};

pub(crate) fn generate_map_cells(
    requested_cell_count: usize,
    rng_seed: u64,
    max_samples: u32,
    slack: f32,
    spikiness: f64,
    elevation_range: (f64, f64),
) -> Result<Vec<MapCell>, String> {
    let points = sample_points(requested_cell_count, rng_seed, max_samples, slack)?;

    let Some(diagram) = VoronoiDiagram::<Point>::from_tuple(&(0.0, 0.0), &(100.0, 100.0), &points)
    else {
        return Err(format!(
            "Voronoi generation failed: size/seed combo isn't viable (numCells={}, rngSeed={})",
            requested_cell_count, rng_seed
        ));
    };

    let cells = diagram.cells();

    // Compute Voronoi cell neighbors using the d3-delaunay rule: two cells are neighbors iff their clipped polygons share a directed edge (cell `i` has edge a -> b and cell `j` has the reverse edge b -> a). Two sites that are Delaunay-adjacent but whose shared Voronoi edge is fully clipped by the viewport (a common case for hull sites) are correctly excluded.
    //
    // [`edge_owner`] maps quantized directed edges to owning cells, and is used to look up each edge's reverse to discover the neighbor across it. Quantization tolerates the tiny floating point differences voronator can produce when emitting the same shared vertex from two different cell clip operations.
    let quant = |v: f64| -> i64 { (v * 1e9).round() as i64 };
    type EdgeKey = ((i64, i64), (i64, i64));
    let mut edge_owner: std::collections::HashMap<EdgeKey, u32> = std::collections::HashMap::new();
    let quantized_cells: Vec<Vec<(i64, i64)>> = cells
        .iter()
        .map(|polygon| {
            polygon
                .points()
                .iter()
                .map(|p| (quant(p.x), quant(p.y)))
                .collect()
        })
        .collect();
    for (i, poly) in quantized_cells.iter().enumerate() {
        let n = poly.len();
        if n < 2 {
            continue;
        }
        for k in 0..n {
            let a = poly[k];
            let b = poly[(k + 1) % n];
            edge_owner.insert((a, b), i as u32);
        }
    }
    let mut neighbors: Vec<Vec<u32>> = vec![Vec::new(); quantized_cells.len()];
    for (i, poly) in quantized_cells.iter().enumerate() {
        let n = poly.len();
        if n < 2 {
            continue;
        }
        for k in 0..n {
            let a = poly[k];
            let b = poly[(k + 1) % n];
            if let Some(&j) = edge_owner.get(&(b, a)) {
                let j_usize = j as usize;
                if j_usize != i && !neighbors[i].contains(&j) {
                    neighbors[i].push(j);
                }
            }
        }
    }

    let mut output_cells = Vec::with_capacity(cells.len());
    for (cell_index, polygon) in cells.iter().enumerate() {
        let mut vertices = Vec::with_capacity(polygon.points().len());
        for point in polygon.points() {
            let height = vertex_height(point.x, point.y, rng_seed, spikiness, elevation_range);
            vertices.push([point.x, point.y, height]);
        }
        let cell_neighbors = neighbors.get(cell_index).cloned().unwrap_or_default();
        output_cells.push(MapCell::new(vertices, cell_neighbors));
    }

    Ok(output_cells)
}

/// Build map cells and a subdivided terrain triangle mesh.
#[wasm_bindgen(js_name = "generateMapData")]
pub fn generate_map_data_js(options: GameOptions) -> Result<MapData, JsValue> {
    if options.num_cells > usize::MAX as u64 {
        return Err(JsValue::from_str(
            "numCells is too large for this target architecture",
        ));
    }

    let max_samples = options.max_samples.unwrap_or(20.0).clamp(1.0, 128.0) as u32;
    let slack = options.slack.unwrap_or(0.25).clamp(0.0, 0.95) as f32;
    let spikiness = options.spikiness.unwrap_or(0.4).clamp(0.0, 1.0);
    let elevation_min = options.elevation_min.unwrap_or(-0.4);
    let elevation_max = options.elevation_max.unwrap_or(0.4);
    let subdivisions = options.terrain_subdivisions.unwrap_or(4).clamp(1, 16);

    let cells = generate_map_cells(
        options.num_cells as usize,
        options.rng_seed,
        max_samples,
        slack,
        spikiness,
        (elevation_min, elevation_max),
    )
    .map_err(|err| JsValue::from_str(&err))?;

    let terrain = generate_terrain_triangles(
        &cells,
        options.rng_seed,
        spikiness,
        (elevation_min, elevation_max),
        subdivisions,
    );

    Ok(MapData { cells, terrain })
}

/// Builds a flat triangle mesh that covers every cell, with each fan triangle subdivided into `subdivisions²` sub-triangles using barycentric tessellation.
///
/// Each emitted vertex resamples the terrain noise at its position, so detail no longer pins to Voronoi corners.
///
/// Output layout: `positions` is `[x, height, y]`-packed, three floats per vertex, three vertices per triangle. `cell_indices` carries the owning cell index for each vertex (one entry per vertex).
fn generate_terrain_triangles(
    cells: &[MapCell],
    rng_seed: u64,
    spikiness: f64,
    elevation_range: (f64, f64),
    subdivisions: u32,
) -> TerrainTriangles {
    let s = subdivisions.max(1);
    let s_f = s as f64;

    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut cell_indices: Vec<u32> = Vec::new();

    // Finite-difference step. Small enough to capture the detail layer of `vertex_height` (whose smallest scale is roughly `(25 - 22*spikiness) / 3`, i.e. ~1.0 at max spikiness), but large enough to avoid floating-point cancellation noise.
    let eps = 0.05_f64;
    let sample = |x: f64, y: f64| -> ([f32; 3], [f32; 3]) {
        let h = vertex_height(x, y, rng_seed, spikiness, elevation_range);
        let hx = vertex_height(x + eps, y, rng_seed, spikiness, elevation_range);
        let hy = vertex_height(x, y + eps, rng_seed, spikiness, elevation_range);
        let dh_dx = (hx - h) / eps;
        let dh_dy = (hy - h) / eps;
        // Surface z = h(x, y); tangents are (1, dh/dx, 0) and (0, dh/dy, 1) in (x, height, z) world axes. Normal = T_x \u00d7 T_y normalized.
        let nx = -dh_dx;
        let ny = 1.0_f64;
        let nz = -dh_dy;
        let inv_len = 1.0 / (nx * nx + ny * ny + nz * nz).sqrt();
        (
            [x as f32, h as f32, y as f32],
            [
                (nx * inv_len) as f32,
                (ny * inv_len) as f32,
                (nz * inv_len) as f32,
            ],
        )
    };

    for (cell_idx, cell) in cells.iter().enumerate() {
        let verts: Vec<(f64, f64)> = cell.vertex_xz().collect();
        if verts.len() < 3 {
            continue;
        }
        let cell_idx_u32 = cell_idx as u32;
        let a = verts[0];

        // Fan-triangulate the polygon, then barycentrically subdivide each fan triangle. The fan apex `a` is shared, so every fan triangle inside a cell already agrees on sub-vertex positions along the radii from `a`.
        for j in 1..verts.len() - 1 {
            let b = verts[j];
            let c = verts[j + 1];

            // Walk the (i, k) lattice where i + k <= s. Weights: a -> (s - i - k)/s, b -> k/s, c -> i/s.
            for i in 0..s {
                for k in 0..(s - i) {
                    let p00 = bary_xy(a, b, c, i, k, s_f);
                    let p01 = bary_xy(a, b, c, i, k + 1, s_f);
                    let p10 = bary_xy(a, b, c, i + 1, k, s_f);

                    let (v00, n00) = sample(p00.0, p00.1);
                    let (v01, n01) = sample(p01.0, p01.1);
                    let (v10, n10) = sample(p10.0, p10.1);
                    positions.extend_from_slice(&v00);
                    positions.extend_from_slice(&v01);
                    positions.extend_from_slice(&v10);
                    normals.extend_from_slice(&n00);
                    normals.extend_from_slice(&n01);
                    normals.extend_from_slice(&n10);
                    cell_indices.extend_from_slice(&[cell_idx_u32; 3]);

                    if i + k + 1 < s {
                        let p11 = bary_xy(a, b, c, i + 1, k + 1, s_f);
                        let (v11, n11) = sample(p11.0, p11.1);
                        positions.extend_from_slice(&v01);
                        positions.extend_from_slice(&v11);
                        positions.extend_from_slice(&v10);
                        normals.extend_from_slice(&n01);
                        normals.extend_from_slice(&n11);
                        normals.extend_from_slice(&n10);
                        cell_indices.extend_from_slice(&[cell_idx_u32; 3]);
                    }
                }
            }
        }
    }

    TerrainTriangles {
        positions,
        normals,
        cell_indices,
    }
}

fn bary_xy(a: (f64, f64), b: (f64, f64), c: (f64, f64), i: u32, k: u32, s: f64) -> (f64, f64) {
    let wa = (s - i as f64 - k as f64) / s;
    let wb = (k as f64) / s;
    let wc = (i as f64) / s;
    (
        wa * a.0 + wb * b.0 + wc * c.0,
        wa * a.1 + wb * b.1 + wc * c.1,
    )
}

/// Computes terrain elevation for a world-space point using layered value noise.
///
/// # Arguments
/// * `x` - World-space x coordinate.
/// * `y` - World-space y coordinate.
/// * `seed` - Deterministic seed that controls the generated terrain pattern.
/// * `spikiness` - Shape control in `[0.0, 1.0]`; lower is smoother and broader,
///   higher is tighter and spikier.
/// * `elevation_range` - Output elevation bounds as `(min, max)`.
fn vertex_height(x: f64, y: f64, seed: u64, spikiness: f64, elevation_range: (f64, f64)) -> f64 {
    let (elev_min, elev_max) = elevation_range;
    let mid = (elev_min + elev_max) / 2.0;
    let amplitude = (elev_max - elev_min) / 2.0;
    let scale = 25.0 - 22.0 * spikiness;
    let detail_scale = scale / 3.0;
    let detail_amplitude = amplitude * 0.08;
    mid + value_noise_2d(x, y, seed, scale) * amplitude
        + value_noise_2d(x, y, seed ^ 0x9e37_79b9_7f4a_7c15, detail_scale) * detail_amplitude
}

fn value_noise_2d(x: f64, y: f64, seed: u64, scale: f64) -> f64 {
    let fx = x / scale;
    let fy = y / scale;

    let x0 = fx.floor() as i64;
    let y0 = fy.floor() as i64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let tx = smoothstep(fx - x0 as f64);
    let ty = smoothstep(fy - y0 as f64);

    let v00 = lattice_random(x0, y0, seed);
    let v10 = lattice_random(x1, y0, seed);
    let v01 = lattice_random(x0, y1, seed);
    let v11 = lattice_random(x1, y1, seed);

    let a = lerp(v00, v10, tx);
    let b = lerp(v01, v11, tx);
    lerp(a, b, ty)
}

fn lattice_random(ix: i64, iy: i64, seed: u64) -> f64 {
    let mixed = seed
        ^ (ix as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (iy as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(mixed);
    next_unit_f64(&mut rng) * 2.0 - 1.0
}

fn next_unit_f64(rng: &mut impl RngCore) -> f64 {
    let value = rng.next_u64() >> 11;
    (value as f64) * (1.0 / ((1u64 << 53) as f64))
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn sample_points(
    requested_cell_count: usize,
    rng_seed: u64,
    max_samples: u32,
    slack: f32,
) -> Result<Vec<(f64, f64)>, String> {
    if requested_cell_count == 0 {
        return Ok(Vec::new());
    }

    let width = 100.0_f32;
    let height = 100.0_f32;
    let area = width * height;
    let spacing = (area / requested_cell_count as f32).sqrt();
    let mut min_radius = (spacing * (0.6 - (0.4 * slack))).max(0.05);
    let relax_factor = (0.92 - (0.22 * slack)).clamp(0.55, 0.92);

    for radius_try in 0..8u64 {
        let mut noise =
            BlueNoise::<Xoshiro256PlusPlus>::from_seed(width, height, min_radius, rng_seed);
        noise.with_samples(max_samples);

        let mut points: Vec<(f64, f64)> = noise
            .map(|point| (point.x as f64, point.y as f64))
            .collect();

        if points.len() >= requested_cell_count {
            // BlueNoise emits points incrementally around active seeds; shuffle to avoid
            // early-iteration spatial bias (which can look center-clustered when truncated).
            let mut chooser = Xoshiro256PlusPlus::seed_from_u64(
                rng_seed ^ (0x9e37_79b9_7f4a_7c15u64.wrapping_mul(radius_try + 1)),
            );
            points.shuffle(&mut chooser);
            points.truncate(requested_cell_count);
            return Ok(points);
        }

        min_radius *= relax_factor;
    }

    Err(format!(
        "Poisson sampling failed: size/seed combo isn't viable (numCells={}, rngSeed={}, tries={}, slack={})",
        requested_cell_count, rng_seed, max_samples, slack
    ))
}

#[cfg(test)]
mod tests {
    use super::{sample_points, vertex_height};

    #[test]
    fn sample_points_are_reproducible_for_same_seed_and_options() {
        let num_cells = 200;
        let rng_seed = 1337;
        let max_samples = 30;
        let slack = 0.2;

        let first = sample_points(num_cells, rng_seed, max_samples, slack)
            .expect("first sampling should succeed");
        let second = sample_points(num_cells, rng_seed, max_samples, slack)
            .expect("second sampling should succeed");

        assert_eq!(first.len(), num_cells);
        assert_eq!(first, second);
    }

    #[test]
    fn vertex_height_is_reproducible() {
        let h1 = vertex_height(12.345, 67.89, 4242, 0.4, (-0.4, 0.4));
        let h2 = vertex_height(12.345, 67.89, 4242, 0.4, (-0.4, 0.4));
        let h3 = vertex_height(12.345, 67.89, 4243, 0.4, (-0.4, 0.4));

        assert_eq!(h1, h2);
        assert!((h1 - h3).abs() > f64::EPSILON);
    }
}
