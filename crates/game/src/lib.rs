mod listener;
mod mapgen;
mod mutation;
mod utils;

use std::{cell::RefCell, rc::Rc};

use js_sys::{Array, JSON, Reflect};
use n0_future::time::Duration;
use serde::{Deserialize, Serialize};
use svelte_store::Readable;
use tsify::Tsify;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
mod net;
use mutation::{Mutation, MutationOrigin, apply_mutation_with_effects};
use net::{NetworkNode, TicketOpts};
use networking::GameTicket;
use wasm_bindgen::{JsCast, JsValue};

const MESSAGE_RECEIVED_EVENT: &str = "messageReceived";
const PULSE_DURATION_MS: u32 = 250; // TODO: Decrease after testing

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &str = r#"
import type { Readable } from "svelte/store";
"#;

#[derive(Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct MapCell {
    is_explored: bool,
    is_void: bool,
    vertices: Vec<[f64; 3]>,
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
    fn new(
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

#[derive(Clone)]
#[wasm_bindgen]
pub struct UiState {
    pulses: Rc<RefCell<Readable<Array>>>,
    next_pulse_id: Rc<RefCell<u32>>,
}

#[wasm_bindgen]
pub struct GameState {
    cells: Rc<RefCell<Readable<Array>>>,
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

    #[wasm_bindgen(typescript_type = "Readable<Array<Pulse>>")]
    pub type Pulses;
}

#[wasm_bindgen]
impl UiState {
    #[wasm_bindgen(getter, js_name = pulses)]
    pub fn pulses_store(&self) -> Pulses {
        self.pulses.borrow().get_store().into()
    }

    #[wasm_bindgen(js_name = "addPulse")]
    pub fn add_pulse(
        &self,
        origin_cell: usize,
        x: f64,
        y: f64,
        z: f64,
        duration_ms: u32,
    ) -> Result<u32, JsValue> {
        self.add_pulse_internal(origin_cell, x, y, z, duration_ms, false)
    }
}

impl UiState {
    fn add_pulse_internal(
        &self,
        origin_cell: usize,
        x: f64,
        y: f64,
        z: f64,
        duration_ms: u32,
        is_remote: bool,
    ) -> Result<u32, JsValue> {
        let mut next_pulse_id = self.next_pulse_id.borrow_mut();
        let pulse_id = *next_pulse_id;
        *next_pulse_id = next_pulse_id.wrapping_add(1);

        let created_at_ms = monotonic_now_ms();
        let pulse = Pulse::new(
            pulse_id,
            origin_cell,
            [x, y, z],
            created_at_ms,
            duration_ms,
            is_remote,
        );
        let pulse: JsValue = pulse.into();
        self.pulses.borrow_mut().set_with(|pulses_array| {
            pulses_array.push(pulse.as_ref());
        });

        let ui_state = self.clone();
        spawn_local(async move {
            n0_future::time::sleep(Duration::from_millis(duration_ms as u64)).await;
            if let Err(err) = ui_state.remove_pulse_by_id(pulse_id) {
                tracing::warn!("failed to remove expired pulse: {:?}", err);
            }
        });

        Ok(pulse_id)
    }
}

fn monotonic_now_ms() -> f64 {
    let global = js_sys::global();
    let performance = Reflect::get(&global, &JsValue::from_str("performance"))
        .ok()
        .filter(|value| !value.is_null() && !value.is_undefined());

    if let Some(performance) = performance {
        let now_fn = Reflect::get(&performance, &JsValue::from_str("now"))
            .ok()
            .and_then(|value| value.dyn_into::<js_sys::Function>().ok());
        if let Some(now_fn) = now_fn {
            if let Ok(result) = now_fn.call0(&performance) {
                if let Some(ms) = result.as_f64() {
                    return ms;
                }
            }
        }
    }

    js_sys::Date::now()
}

impl UiState {
    fn new() -> Self {
        Self {
            pulses: Rc::new(RefCell::new(Readable::new(Array::new()))),
            next_pulse_id: Rc::new(RefCell::new(0)),
        }
    }

    fn remove_pulse_by_id(&self, pulse_id: u32) -> Result<(), JsValue> {
        self.pulses.borrow_mut().set_with(|pulses_array| {
            let filtered = Array::new();
            for idx in 0..pulses_array.length() {
                let pulse = pulses_array.get(idx);
                let id = Reflect::get(&pulse, &JsValue::from_str("id"))?
                    .as_f64()
                    .unwrap_or(-1.0) as u32;
                if id != pulse_id {
                    filtered.push(&pulse);
                }
            }
            *pulses_array = filtered;
            Ok::<(), JsValue>(())
        })
    }
}

impl MapCell {
    pub(crate) fn from_vertices(
        vertices: Vec<[f64; 3]>,
        is_explored: bool,
        is_void: bool,
    ) -> MapCell {
        MapCell {
            is_explored,
            is_void,
            vertices,
        }
    }
}

#[wasm_bindgen]
impl GameState {
    #[wasm_bindgen(constructor)]
    pub fn new(options: GameOptions) -> Result<GameState, JsValue> {
        utils::set_panic_hook();

        let options_value = serde_wasm_bindgen::to_value(&options)?;
        let options_json = JSON::stringify(&options_value)?
            .as_string()
            .ok_or_else(|| JsValue::from_str("Failed to serialize game options"))?;

        let max_samples = options.max_samples.unwrap_or(20.0).clamp(1.0, 128.0) as u32;
        let slack = options.slack.unwrap_or(0.25).clamp(0.0, 0.95) as f32;
        let spikiness = options.spikiness.unwrap_or(0.4).clamp(0.0, 1.0);
        let elevation_min = options.elevation_min.unwrap_or(-0.4);
        let elevation_max = options.elevation_max.unwrap_or(0.4);

        Ok(GameState {
            cells: Rc::new(RefCell::new(Readable::new(Array::new()))),
            ui_state: UiState::new(),
            num_cells: options.num_cells,
            rng_seed: options.rng_seed,
            max_samples,
            slack,
            spikiness,
            elevation_min,
            elevation_max,
            network_node: None,
            network_channel: None,
            network_listener_started: false,
            game_options_json: options_json,
        })
    }

    #[wasm_bindgen(getter, js_name = cells)]
    pub fn cells_store(&self) -> MapCells {
        self.cells.borrow().get_store().into()
    }

    #[wasm_bindgen(getter, js_name = "uiState")]
    pub fn ui_state(&self) -> UiState {
        self.ui_state.clone()
    }

    #[wasm_bindgen(getter, js_name = "elevationMin")]
    pub fn elevation_min(&self) -> f64 {
        self.elevation_min
    }

    #[wasm_bindgen(getter, js_name = "elevationMax")]
    pub fn elevation_max(&self) -> f64 {
        self.elevation_max
    }

    #[wasm_bindgen(getter, js_name = "hasNetworkNode")]
    pub fn has_network_node(&self) -> bool {
        self.network_node.is_some()
    }

    pub fn generate_map(&mut self) -> Result<JsValue, JsValue> {
        if self.num_cells > usize::MAX as u64 {
            return Err(JsValue::from_str(
                "numCells is too large for this target architecture",
            ));
        }

        let requested_cell_count = self.num_cells as usize;
        let output_cells = Array::new();
        let cells = mapgen::generate_map_cells(
            requested_cell_count,
            self.rng_seed,
            self.max_samples,
            self.slack,
            self.spikiness,
            (self.elevation_min, self.elevation_max),
        )?;

        for cell in cells {
            let map_cell = serde_wasm_bindgen::to_value(&cell)?;
            output_cells.push(&map_cell);
        }

        self.cells.borrow_mut().set(output_cells.clone());
        Ok(output_cells.into())
    }

    #[wasm_bindgen(js_name = "exploreCell")]
    pub fn explore_cell(&mut self, index: usize, x: f64, y: f64, z: f64) -> Result<(), JsValue> {
        self.queue_explore_pulse(index, x, y, z)
    }

    #[wasm_bindgen(js_name = "queueExplorePulse")]
    pub fn queue_explore_pulse(&self, index: usize, x: f64, y: f64, z: f64) -> Result<(), JsValue> {
        self.dispatch_explore_mutation(
            Mutation::ExploreCell {
                index,
                pulse_position: [x, y, z],
            },
            MutationOrigin::Local,
        )
    }

    #[wasm_bindgen(js_name = "invite")]
    pub async fn invite(&mut self, nickname: String) -> Result<String, JsValue> {
        if self.network_node.is_none() {
            let node = NetworkNode::spawn().await.map_err(Into::<JsValue>::into)?;
            self.network_node = Some(node);
        }

        if self.network_channel.is_none() {
            let node = self
                .network_node
                .as_ref()
                .ok_or_else(|| JsValue::from_str("Network node is unavailable"))?;
            let channel = node.create(nickname).await.map_err(Into::<JsValue>::into)?;
            self.network_channel = Some(channel);
            self.attach_network_listener();
        }

        let channel = self
            .network_channel
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network channel is unavailable"))?;
        let ticket_opts = TicketOpts {
            include_myself: true,
            include_bootstrap: true,
            include_neighbors: true,
        };

        channel
            .ticket_with_game_options(ticket_opts, Some(self.game_options_json.clone()))
            .map_err(Into::<JsValue>::into)
    }

    #[wasm_bindgen(js_name = "joinFromTicket")]
    pub async fn join_from_ticket(ticket: String, nickname: String) -> Result<GameState, JsValue> {
        let parsed_ticket = GameTicket::deserialize(&ticket)
            .map_err(|err| JsValue::from_str(&format!("Invalid game ticket: {err}")))?;
        let options_json = parsed_ticket
            .game_options_json
            .ok_or_else(|| JsValue::from_str("Ticket does not include game options"))?;
        let options_value = JSON::parse(&options_json)?;
        let options: GameOptions = serde_wasm_bindgen::from_value(options_value)?;
        let mut state = GameState::new(options)?;
        state.generate_map()?;

        let node = NetworkNode::spawn().await.map_err(Into::<JsValue>::into)?;
        let channel = node
            .join(ticket, nickname)
            .await
            .map_err(Into::<JsValue>::into)?;
        state.network_node = Some(node);
        state.network_channel = Some(channel);
        state.attach_network_listener();

        Ok(state)
    }
}

impl GameState {
    fn attach_network_listener(&mut self) {
        if self.network_listener_started {
            return;
        }

        let Some(channel) = self.network_channel.as_mut() else {
            return;
        };

        let cells = self.cells.clone();
        let ui_state = self.ui_state.clone();
        let local_endpoint_id = self.network_node.as_ref().map(|node| node.endpoint_id());
        let receiver = channel.receiver();
        self.network_listener_started = true;

        listener::spawn_network_listener(receiver, cells, ui_state, local_endpoint_id);
    }

    fn dispatch_explore_mutation(
        &self,
        mutation: Mutation,
        origin: MutationOrigin,
    ) -> Result<(), JsValue> {
        apply_mutation_with_effects(&self.cells, &self.ui_state, mutation, origin)?;

        if matches!(origin, MutationOrigin::Local) {
            self.broadcast_state_mutation(mutation);
        }

        Ok(())
    }

    fn broadcast_state_mutation(&self, mutation: Mutation) {
        let Some(channel) = self.network_channel.as_ref() else {
            return;
        };

        let sender = channel.sender();
        let payload = mutation.encode();

        spawn_local(async move {
            if let Err(err) = sender.broadcast(payload).await {
                tracing::warn!("failed to broadcast state mutation: {:?}", err);
            }
        });
    }
}
