mod utils;

use js_sys::{Array, Object, Reflect};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
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

        Ok(GameState {
            cells: Readable::new(Array::new()),
            num_cells: num_cells,
            rng_seed: rng_seed,
        })
    }

    #[wasm_bindgen(getter, js_name = cells)]
    pub fn cells_store(&self) -> MapCells {
        self.cells.get_store().into()
    }

    pub fn generate_map(&mut self) -> Result<JsValue, JsValue> {
        let requested_cell_count = self.num_cells;
        let output_cells = Array::new();
        let mut rng = ChaCha8Rng::seed_from_u64(self.rng_seed);
        let points: Vec<(f64, f64)> = (0..requested_cell_count)
            .map(|_| {
                (
                    next_unit_f64(&mut rng) * 100.0,
                    next_unit_f64(&mut rng) * 100.0,
                )
            })
            .collect();

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

fn next_unit_f64(rng: &mut impl RngCore) -> f64 {
    let value = rng.next_u64() >> 11;
    (value as f64) * (1.0 / ((1u64 << 53) as f64))
}
