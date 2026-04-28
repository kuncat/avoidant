mod utils;

use bluenoise::BlueNoise;
use js_sys::{Array, Object, Reflect};
use rand::seq::SliceRandom;
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
}

export interface MapCell {
    isExplored: boolean,
    isVoid: boolean,
    vertices: Array<[number, number]>,
}
"#;

#[wasm_bindgen]
pub struct GameState {
    cells: Readable<Array>,
    num_cells: u64,
    rng_seed: u64,
    max_samples: u32,
    slack: f32,
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

        Ok(GameState {
            cells: Readable::new(Array::new()),
            num_cells: num_cells,
            rng_seed: rng_seed,
            max_samples,
            slack,
        })
    }

    #[wasm_bindgen(getter, js_name = cells)]
    pub fn cells_store(&self) -> MapCells {
        self.cells.get_store().into()
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
                    let point_pair = Array::new();
                    point_pair.push(&JsValue::from_f64(point.x));
                    point_pair.push(&JsValue::from_f64(point.y));
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
    use super::sample_points;

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
}
