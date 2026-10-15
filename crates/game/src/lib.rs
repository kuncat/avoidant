mod game_api;
mod game_network;
mod mapgen;
mod mutation;
mod score;
mod sync;
mod sync_bridge;
mod ui_state;
mod utils;

use std::{cell::RefCell, rc::Rc};

use js_sys::Array;
pub use mapgen::MapShape;
pub use score::ScoreState;
use serde::{Deserialize, Serialize};
use svelte_store::Readable;
use tsify::Tsify;
pub use ui_state::UiState;
use wasm_bindgen::prelude::*;

mod shared_constants;
pub use shared_constants::{PULSE_MIN_DURATION_MS, PULSE_SWEEP_BAND, PULSE_SWEEP_VELOCITY};

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &str = r#"
import type { Readable } from "svelte/store";
"#;

#[derive(Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct MapCell {
    /// True 3D world-space polygon vertices on (or just above) the map
    /// surface, CCW around `normal`.
    vertices: Vec<[f64; 3]>,
    /// Global indices of adjacent cells, including cross-face neighbors that
    /// share an actual boundary edge on the surface.
    neighbors: Vec<u32>,
    /// 3D centroid of the cell on the surface (used for label anchoring, fan
    /// triangulation apex, and pulse origins).
    centroid: [f64; 3],
    /// Outward unit normal at the cell's centroid. For polyhedra this is the
    /// face normal; for the spheroid it's the surface normal at the centroid.
    /// Used by the renderer to displace falling void cells outward.
    normal: [f64; 3],
}

#[derive(Clone, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct CellMetadataEntry {
    is_explored: bool,
    is_void: bool,
    void_neighbor_count: u8,
    /// Toggled during a chord auto-reveal.
    is_revealing: bool,
}

#[derive(Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GameOptions {
    num_cells: u64,
    rng_seed: u64,
    #[tsify(optional)]
    /** Map surface shape. Defaults to Icosahedron { radius: 50.0 }. */
    shape: Option<MapShape>,
    #[tsify(optional)]
    /** 0.0 = smooth broad hills, 1.0 = tight spiky features. Default: 0.4 */
    spikiness: Option<f64>,
    #[tsify(optional)]
    /** Minimum vertex elevation displacement in world units. Default: -0.4 */
    elevation_min: Option<f64>,
    #[tsify(optional)]
    /** Maximum vertex elevation displacement in world units. Default: 0.4 */
    elevation_max: Option<f64>,
    #[tsify(optional)]
    /** Per-cell terrain mesh subdivision level. Each fan triangle inside a cell is split into `S²` sub-triangles, sampling noise at every sub-vertex. */
    terrain_subdivisions: Option<u32>,
    #[tsify(optional)]
    // Fraction of cells to mark as void in `[0.0, 1.0]`.
    void_fraction: Option<f64>,
    #[tsify(optional)]
    #[serde(default)]
    first_safe_cell: Option<usize>,
    #[tsify(optional)]
    #[serde(default)]
    authority: Option<String>,
    #[tsify(optional)]
    #[serde(default)]
    sync_version: Option<u8>,
}

/// Flat per-vertex terrain mesh, decoupled from cell corners.
///
/// `positions` is a `[x, y, z]`-packed `f32` array (length is a multiple of 9 (3 verts per triangle, 3 floats per vert). `normals` is `[nx, ny, nz]`) packed and parallel to `positions`; each vertex carries a smooth normal derived analytically from the noise field so the shader can per-pixel-interpolate it and avoid faceted flat shading. `cell_indices` carries the owning cell index for each emitted vertex (length = `positions.len() / 3`). `heights` is the scalar elevation displacement per vertex (the raw value sampled from the noise field along the cell's outward normal); the shader uses it for the elevation color ramp. Built once per map by [`crate::mapgen::generate_terrain_triangles`].
#[derive(Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct TerrainTriangles {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub cell_indices: Vec<u32>,
    pub heights: Vec<f32>,
}

/// Voronoi cell polygons paired with the subdivided terrain triangle mesh.
#[derive(Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct MapData {
    pub cells: Vec<MapCell>,
    pub terrain: TerrainTriangles,
    /// Estimated surface area of the generated shape in world units²; used
    /// by the renderer to size per-cell visuals (labels, gap insets) from
    /// the per-cell average area.
    pub surface_area: f64,
    /// Maximum radial extent of the un-displaced shape from the origin, in
    /// world units. Used for camera framing.
    pub bounds_radius: f64,
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct Pulse {
    id: u32,
    origin_cell: usize,
    position: [f64; 3],
    created_at_ms: f64,
    duration_ms: u32,
    is_remote: bool,
    /// World-space radius the pulse ring should grow to over its lifetime.
    max_radius: f64,
}

impl Pulse {
    pub(crate) fn new(
        id: u32,
        origin_cell: usize,
        position: [f64; 3],
        created_at_ms: f64,
        duration_ms: u32,
        is_remote: bool,
        max_radius: f64,
    ) -> Self {
        Self {
            id,
            origin_cell,
            position,
            created_at_ms,
            duration_ms,
            is_remote,
            max_radius,
        }
    }

    fn null_internal() -> Self {
        Self {
            id: u32::MAX,
            origin_cell: usize::MAX,
            position: [0.0, 0.0, 0.0],
            created_at_ms: 0.0,
            duration_ms: 1,
            is_remote: false,
            max_radius: 0.0,
        }
    }
}

#[wasm_bindgen]
impl Pulse {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u32 {
        self.id
    }

    #[wasm_bindgen(getter, js_name = "originCell")]
    pub fn origin_cell(&self) -> usize {
        self.origin_cell
    }

    #[wasm_bindgen(getter)]
    pub fn position(&self) -> Vec<f64> {
        self.position.to_vec()
    }

    #[wasm_bindgen(getter, js_name = "createdAtMs")]
    pub fn created_at_ms(&self) -> f64 {
        self.created_at_ms
    }

    #[wasm_bindgen(getter, js_name = "durationMs")]
    pub fn duration_ms(&self) -> u32 {
        self.duration_ms
    }

    #[wasm_bindgen(getter, js_name = "isRemote")]
    pub fn is_remote(&self) -> bool {
        self.is_remote
    }

    #[wasm_bindgen(getter, js_name = "maxRadius")]
    pub fn max_radius(&self) -> f64 {
        self.max_radius
    }

    #[wasm_bindgen(js_name = "nullPulse")]
    pub fn null_pulse() -> Pulse {
        Self::null_internal()
    }
}

#[wasm_bindgen]
pub struct GameState {
    session: Rc<RefCell<sync::Session>>,
    cells: Rc<RefCell<Readable<Array>>>,
    cell_metadata: Rc<RefCell<Readable<Array>>>,
    score: Rc<RefCell<Readable<ScoreState>>>,
    ui_state: UiState,
    rng_seed: u64,
    elevation_min: f64,
    elevation_max: f64,
    void_fraction: f64,
    first_safe_cell: std::cell::Cell<Option<usize>>,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Readable<Array<MapCell>>")]
    pub type MapCells;

    #[wasm_bindgen(typescript_type = "Readable<Array<CellMetadataEntry>>")]
    pub type CellMetadata;

    #[wasm_bindgen(typescript_type = "Readable<ScoreState>")]
    pub type Score;

    #[wasm_bindgen(typescript_type = "Readable<Array<Pulse>>")]
    pub type Pulses;
}

impl MapCell {
    pub(crate) fn new(
        vertices: Vec<[f64; 3]>,
        neighbors: Vec<u32>,
        centroid: [f64; 3],
        normal: [f64; 3],
    ) -> MapCell {
        MapCell {
            vertices,
            neighbors,
            centroid,
            normal,
        }
    }

    pub(crate) fn neighbors(&self) -> &[u32] {
        &self.neighbors
    }

    /// 3D world-space positions of this cell's vertices on the (displaced)
    /// surface. Returns one `[x, y, z]` triple per vertex.
    pub(crate) fn vertex_xyz(&self) -> impl Iterator<Item = [f64; 3]> + '_ {
        self.vertices.iter().copied()
    }
}

impl CellMetadataEntry {
    pub(crate) fn new(is_explored: bool, is_void: bool, void_neighbor_count: u8) -> Self {
        Self {
            is_explored,
            is_void,
            void_neighbor_count,
            is_revealing: false,
        }
    }

    pub(crate) fn set_revealing(&mut self, value: bool) {
        self.is_revealing = value;
    }
}

impl Drop for GameState {
    fn drop(&mut self) {
        self.session.borrow_mut().stopped = true;
    }
}

/// Build public competitive geometry for the server-approved map catalog.
/// No hidden void layout is included; that is generated by the match authority.
pub fn competitive_catalog_adjacency(options: GameOptions) -> Result<Vec<Vec<u32>>, String> {
    let map = mapgen::generate_map(
        options.num_cells as usize,
        options.rng_seed,
        &options.shape.unwrap_or_default(),
    )?;
    Ok(map
        .cells
        .iter()
        .map(|cell| cell.neighbors().to_vec())
        .collect())
}
