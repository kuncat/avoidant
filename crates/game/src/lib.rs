#[cfg(not(target_arch = "wasm32"))]
compile_error!(
    "The avoidant game crate is wasm32-only. Build with --target wasm32-unknown-unknown."
);

mod game_api;
mod game_network;
mod listener;
mod mapgen;
mod mutation;
mod ui_state;
mod utils;

use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    rc::Rc,
};

use js_sys::Array;
use serde::{Deserialize, Serialize};
use svelte_store::Readable;
use tsify::Tsify;
use wasm_bindgen::prelude::*;
mod net;
use net::NetworkNode;
pub use ui_state::UiState;

const PULSE_DURATION_MS: u32 = 250; // TODO: Decrease after testing

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &str = r#"
import type { Readable } from "svelte/store";
"#;

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct MapCell {
    vertices: Vec<[f64; 3]>,
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(from_wasm_abi, into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct CellMetadataEntry {
    is_explored: bool,
    is_void: bool,
}

#[derive(Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPeerStatus {
    endpoint_id: String,
    #[tsify(optional)]
    nickname: Option<String>,
    #[tsify(optional)]
    last_seen_ms: Option<f64>,
    is_connected: bool,
}

#[derive(Clone, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSnapshot {
    pub(crate) has_node: bool,
    pub(crate) listener_started: bool,
    #[tsify(optional)]
    pub(crate) endpoint_id: Option<String>,
    #[tsify(optional)]
    pub(crate) topic_id: Option<String>,
    pub(crate) peers: Vec<NetworkPeerStatus>,
    #[tsify(optional)]
    pub(crate) last_inbound_mutation_ms: Option<f64>,
    #[tsify(optional)]
    pub(crate) last_outbound_mutation_ms: Option<f64>,
    pub(crate) sampled_at_ms: f64,
}

#[derive(Default, Clone)]
pub(crate) struct PeerPresenceEntry {
    pub(crate) nickname: Option<String>,
    pub(crate) last_seen_ms: Option<f64>,
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(from_wasm_abi, into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct GameOptions {
    num_cells: u64,
    rng_seed: u64,
    #[tsify(optional)]
    max_samples: Option<f64>,
    #[tsify(optional)]
    slack: Option<f64>,
    #[tsify(optional)]
    /** 0.0 = smooth broad hills, 1.0 = tight spiky features. Default: 0.4 */
    spikiness: Option<f64>,
    #[tsify(optional)]
    /** Minimum vertex height in world units. Default: -0.4 */
    elevation_min: Option<f64>,
    #[tsify(optional)]
    /** Maximum vertex height in world units. Default: 0.4 */
    elevation_max: Option<f64>,
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
}

impl Pulse {
    pub(crate) fn new(
        id: u32,
        origin_cell: usize,
        position: [f64; 3],
        created_at_ms: f64,
        duration_ms: u32,
        is_remote: bool,
    ) -> Self {
        Self {
            id,
            origin_cell,
            position,
            created_at_ms,
            duration_ms,
            is_remote,
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

    #[wasm_bindgen(js_name = "nullPulse")]
    pub fn null_pulse() -> Pulse {
        Self::null_internal()
    }
}

#[wasm_bindgen]
pub struct GameState {
    cells: Rc<RefCell<Readable<Array>>>,
    cell_metadata: Rc<RefCell<Readable<Array>>>,
    network_snapshot: Rc<RefCell<Readable<NetworkSnapshot>>>,
    connected_endpoints: Rc<RefCell<BTreeSet<String>>>,
    peer_presence: Rc<RefCell<HashMap<String, PeerPresenceEntry>>>,
    last_inbound_mutation_ms: Rc<RefCell<Option<f64>>>,
    last_outbound_mutation_ms: Rc<RefCell<Option<f64>>>,
    ui_state: UiState,
    num_cells: u64,
    rng_seed: u64,
    max_samples: u32,
    network_node: Option<NetworkNode>,
    network_channel: Option<net::Channel>,
    network_listener_started: bool,
    game_options_json: String,
    slack: f32,
    spikiness: f64,
    elevation_min: f64,
    elevation_max: f64,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Readable<Array<MapCell>>")]
    pub type MapCells;

    #[wasm_bindgen(typescript_type = "Readable<Array<CellMetadataEntry>>")]
    pub type CellMetadata;

    #[wasm_bindgen(typescript_type = "Readable<NetworkSnapshot>")]
    pub type NetworkSnapshotStore;

    #[wasm_bindgen(typescript_type = "Readable<Array<Pulse>>")]
    pub type Pulses;
}

impl MapCell {
    pub(crate) fn from_vertices(vertices: Vec<[f64; 3]>) -> MapCell {
        MapCell { vertices }
    }
}

impl CellMetadataEntry {
    pub(crate) fn new(is_explored: bool, is_void: bool) -> Self {
        Self {
            is_explored,
            is_void,
        }
    }

    pub(crate) fn mark_explored(&mut self) {
        self.is_explored = true;
    }
}

impl NetworkPeerStatus {
    pub(crate) fn new(
        endpoint_id: String,
        nickname: Option<String>,
        last_seen_ms: Option<f64>,
        is_connected: bool,
    ) -> Self {
        Self {
            endpoint_id,
            nickname,
            last_seen_ms,
            is_connected,
        }
    }
}

impl NetworkSnapshot {
    pub(crate) fn empty() -> Self {
        Self {
            has_node: false,
            listener_started: false,
            endpoint_id: None,
            topic_id: None,
            peers: Vec::new(),
            last_inbound_mutation_ms: None,
            last_outbound_mutation_ms: None,
            sampled_at_ms: js_sys::Date::now(),
        }
    }
}
