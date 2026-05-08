use bluenoise::BlueNoise;
use futures_channel::oneshot;
use rand::RngCore;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_xoshiro::Xoshiro256PlusPlus;
use rayon::prelude::*;
use voronator::{VoronoiDiagram, delaunator::Point};
use wasm_bindgen::JsValue;

use crate::MapCell;

pub(crate) async fn generate_map_cells_async(
    requested_cell_count: usize,
    rng_seed: u64,
    max_samples: u32,
    slack: f32,
    spikiness: f64,
    elevation_range: (f64, f64),
) -> Result<Vec<MapCell>, JsValue> {
    if rayon::current_num_threads() <= 1 {
        return Err(JsValue::from_str(
            "WASM thread pool is not initialized. Call initWasmThreadPool(...) after init().",
        ));
    }

    let (sender, receiver) = oneshot::channel();
    rayon::spawn(move || {
        let _ = sender.send(generate_map_cells_inner(
            requested_cell_count,
            rng_seed,
            max_samples,
            slack,
            spikiness,
            elevation_range,
        ));
    });

    let result = receiver
        .await
        .map_err(|_| JsValue::from_str("Map generation task was cancelled"))?;
    result.map_err(|err| JsValue::from_str(&err))
}

fn generate_map_cells_inner(
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

    let output_cells: Vec<MapCell> = diagram
        .cells()
        .par_iter()
        .map(|polygon| {
            let mut vertices = Vec::new();
            for point in polygon.points() {
                let height = vertex_height(point.x, point.y, rng_seed, spikiness, elevation_range);
                vertices.push([point.x, point.y, height]);
            }
            MapCell::from_vertices(vertices)
        })
        .collect();

    Ok(output_cells)
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
