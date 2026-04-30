mod utils;

use bluenoise::BlueNoise;
use js_sys::{Array, Object, Reflect};
use rand::seq::SliceRandom;
use rand::RngCore;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;
use svelte_store::Readable;
use voronator::{delaunator::Point, VoronoiDiagram};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &str = r#"
import type { Readable } from "svelte/store";

export interface GameOptions {
  numCells: number;
  rngSeed: number;
  maxSamples?: number;
  slack?: number;
  /** 0.0 = smooth broad hills, 1.0 = tight spiky features. Default: 0.4 */
  spikiness?: number;
  /** Minimum vertex height in world units. Default: -0.4 */
  elevationMin?: number;
  /** Maximum vertex height in world units. Default: 0.4 */
  elevationMax?: number;
}

export interface MapCell {
    isExplored: boolean,
    isVoid: boolean,
    vertices: Array<[number, number, number]>,
}
"#;

#[wasm_bindgen]
pub struct GameState {
    cells: Readable<Array>,
    num_cells: u64,
    rng_seed: u64,
    max_samples: u32,
    slack: f32,
    spikiness: f64,
    elevation_min: f64,
    elevation_max: f64,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "GameOptions")]
    pub type GameOptions;

    #[wasm_bindgen(typescript_type = "MapCell")]
    pub type MapCell;

    #[wasm_bindgen(typescript_type = "Readable<Array<MapCell>>")]
    pub type MapCells;
}

impl MapCell {
    fn new(vertices: &Array, is_explored: bool, is_void: bool) -> Result<MapCell, JsValue> {
        let map_cell = Object::new();
        Reflect::set(
            map_cell.as_ref(),
            &JsValue::from_str("isExplored"),
            &JsValue::from_bool(is_explored),
        )?;
        Reflect::set(
            map_cell.as_ref(),
            &JsValue::from_str("isVoid"),
            &JsValue::from_bool(is_void),
        )?;
        Reflect::set(
            map_cell.as_ref(),
            &JsValue::from_str("vertices"),
            &vertices.clone().into(),
        )?;

        Ok(map_cell.unchecked_into::<MapCell>())
    }
}

#[wasm_bindgen]
impl GameState {
    #[wasm_bindgen(constructor)]
    pub fn new(options: GameOptions) -> Result<GameState, JsValue> {
        let options: Object = options
            .dyn_into::<Object>()
            .map_err(|_| JsValue::from_str("GameState constructor expects a plain object"))?;

        let num_cells = Reflect::get(options.as_ref(), &JsValue::from_str("numCells"))?
            .as_f64()
            .unwrap() as u64;
        let rng_seed = Reflect::get(options.as_ref(), &JsValue::from_str("rngSeed"))?
            .as_f64()
            .unwrap() as u64;
        let max_samples = Reflect::get(options.as_ref(), &JsValue::from_str("maxSamples"))?
            .as_f64()
            .unwrap_or(20.0)
            .clamp(1.0, 128.0) as u32;
        let slack = Reflect::get(options.as_ref(), &JsValue::from_str("slack"))?
            .as_f64()
            .unwrap_or(0.25)
            .clamp(0.0, 0.95) as f32;
        let spikiness = Reflect::get(options.as_ref(), &JsValue::from_str("spikiness"))?
            .as_f64()
            .unwrap_or(0.4)
            .clamp(0.0, 1.0);
        let elevation_min = Reflect::get(options.as_ref(), &JsValue::from_str("elevationMin"))?
            .as_f64()
            .unwrap_or(-0.4);
        let elevation_max = Reflect::get(options.as_ref(), &JsValue::from_str("elevationMax"))?
            .as_f64()
            .unwrap_or(0.4);

        Ok(GameState {
            cells: Readable::new(Array::new()),
            num_cells: num_cells,
            rng_seed: rng_seed,
            max_samples,
            slack,
            spikiness,
            elevation_min,
            elevation_max,
        })
    }

    #[wasm_bindgen(getter, js_name = cells)]
    pub fn cells_store(&self) -> MapCells {
        self.cells.get_store().into()
    }

    #[wasm_bindgen(getter, js_name = "elevationMin")]
    pub fn elevation_min(&self) -> f64 {
        self.elevation_min
    }

    #[wasm_bindgen(getter, js_name = "elevationMax")]
    pub fn elevation_max(&self) -> f64 {
        self.elevation_max
    }

    pub fn generate_map(&mut self) -> Result<JsValue, JsValue> {
        if self.num_cells > usize::MAX as u64 {
            return Err(JsValue::from_str(
                "numCells is too large for this target architecture",
            ));
        }

        let requested_cell_count = self.num_cells as usize;
        let output_cells = Array::new();
        let points = sample_points(
            requested_cell_count,
            self.rng_seed,
            self.max_samples,
            self.slack,
        )?;

        if let Some(diagram) =
            VoronoiDiagram::<Point>::from_tuple(&(0.0, 0.0), &(100.0, 100.0), &points)
        {
            for polygon in diagram.cells() {
                let polygon_points = Array::new();
                for point in polygon.points() {
                    let height = vertex_height(
                        point.x,
                        point.y,
                        self.rng_seed,
                        self.spikiness,
                        (self.elevation_min, self.elevation_max),
                    );
                    let point_pair = Array::new();
                    point_pair.push(&JsValue::from_f64(point.x));
                    point_pair.push(&JsValue::from_f64(point.y));
                    point_pair.push(&JsValue::from_f64(height));
                    polygon_points.push(&point_pair.into());
                }
                let map_cell = MapCell::new(&polygon_points, false, false)?;
                output_cells.push(map_cell.as_ref());
            }
        } else {
            return Err(JsValue::from_str(&format!(
                "Voronoi generation failed: size/seed combo isn't viable (numCells={}, rngSeed={})",
                requested_cell_count, self.rng_seed
            )));
        }

        self.cells.set(output_cells.clone());
        Ok(output_cells.into())
    }

    #[wasm_bindgen(js_name = "exploreCell")]
    pub fn explore_cell(&mut self, index: usize) -> Result<(), JsValue> {
        self.cells.set_with(|cells_array| {
            let cell = cells_array.get(index as u32);
            Reflect::set(
                &cell,
                &JsValue::from_str("isExplored"),
                &JsValue::from_bool(true),
            )?;
            Ok(())
        })
    }
}

fn vertex_height(x: f64, y: f64, seed: u64, spikiness: f64, elevation_range: (f64, f64)) -> f64 {
    let (elev_min, elev_max) = elevation_range;
    let mid = (elev_min + elev_max) / 2.0;
    let amplitude = (elev_max - elev_min) / 2.0;
    // spikiness 0.0 = smooth broad hills (large scale), 1.0 = tight spiky features (small scale)
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
) -> Result<Vec<(f64, f64)>, JsValue> {
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

    Err(JsValue::from_str(&format!(
        "Poisson sampling failed: size/seed combo isn't viable (numCells={}, rngSeed={}, tries={}, slack={})",
        requested_cell_count, rng_seed, max_samples, slack
    )))
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
        let h1 = vertex_height(12.345, 67.89, 4242);
        let h2 = vertex_height(12.345, 67.89, 4242);
        let h3 = vertex_height(12.345, 67.89, 4243);

        assert_eq!(h1, h2);
        assert!((h1 - h3).abs() > f64::EPSILON);
    }
}
