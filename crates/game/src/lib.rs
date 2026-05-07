mod utils;

use std::{cell::RefCell, rc::Rc};

use bluenoise::BlueNoise;
use futures_util::StreamExt;
use js_sys::{Array, JSON, Reflect};
use n0_future::time::Duration;
use rand::RngCore;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};
use svelte_store::Readable;
use tsify::Tsify;
use voronator::{VoronoiDiagram, delaunator::Point};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
mod net;
use net::{NetworkNode, TicketOpts};
use networking::GameTicket;
use wasm_bindgen::{JsCast, JsValue};

const MESSAGE_RECEIVED_EVENT: &str = "messageReceived";
const MUTATION_DELIMITER: char = '|';
const PULSE_DURATION_MS: u32 = 250; // TODO: Decrease after testing

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &str = r#"
import type { Readable } from "svelte/store";
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum MutationKind {
    Cell = 0,
}

impl MutationKind {
    fn to_wire(self) -> u8 {
        self as u8
    }

    fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Cell),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum CellMutationOp {
    ExploreCell = 0,
}

impl CellMutationOp {
    fn to_wire(self) -> u8 {
        self as u8
    }

    fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::ExploreCell),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mutation {
    ExploreCell {
        index: usize,
        pulse_position: [f64; 3],
    },
}

impl Mutation {
    fn encode(self) -> String {
        match self {
            Self::ExploreCell {
                index,
                pulse_position: [x, y, z],
            } => format!(
                "{}{}{}{}{}{}{}{}{}{}{}",
                MutationKind::Cell.to_wire(),
                MUTATION_DELIMITER,
                CellMutationOp::ExploreCell.to_wire(),
                MUTATION_DELIMITER,
                index,
                MUTATION_DELIMITER,
                x,
                MUTATION_DELIMITER,
                y,
                MUTATION_DELIMITER,
                z
            ),
        }
    }

    fn decode(input: &str) -> Option<Self> {
        let mut parts = input.split(MUTATION_DELIMITER);
        let kind: u8 = parts.next()?.parse().ok()?;
        let op: u8 = parts.next()?.parse().ok()?;

        let mutation = match MutationKind::from_wire(kind)? {
            MutationKind::Cell => match CellMutationOp::from_wire(op)? {
                CellMutationOp::ExploreCell => {
                    let index: usize = parts.next()?.parse().ok()?;
                    let x: f64 = parts.next()?.parse().ok()?;
                    let y: f64 = parts.next()?.parse().ok()?;
                    let z: f64 = parts.next()?.parse().ok()?;
                    Self::ExploreCell {
                        index,
                        pulse_position: [x, y, z],
                    }
                }
            },
        };

        if parts.next().is_some() {
            return None;
        }

        Some(mutation)
    }

    fn apply(self, cells: &Rc<RefCell<Readable<Array>>>) -> Result<(), JsValue> {
        match self {
            Self::ExploreCell { index, .. } => mark_cell_explored(cells, index),
        }
    }
}

enum MutationOrigin {
    Local,
    Peer,
}

struct NetworkTextMessage {
    from: Option<String>,
    text: String,
}

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

#[derive(Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct Pulse {
    id: u32,
    origin_cell: usize,
    position: [f64; 3],
    created_at_ms: f64,
    duration_ms: u32,
    is_remote: bool,
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
        let pulse = Pulse {
            id: pulse_id,
            origin_cell,
            position: [x, y, z],
            created_at_ms,
            duration_ms,
            is_remote,
        };
        let pulse = serde_wasm_bindgen::to_value(&pulse)?;
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
    fn new(vertices: &Array, is_explored: bool, is_void: bool) -> Result<MapCell, JsValue> {
        let mut vertices_vec = Vec::with_capacity(vertices.length() as usize);
        for i in 0..vertices.length() {
            let vertex = vertices.get(i).dyn_into::<Array>()?;
            let x = vertex
                .get(0)
                .as_f64()
                .ok_or_else(|| JsValue::from_str("MapCell vertex missing x coordinate"))?;
            let y = vertex
                .get(1)
                .as_f64()
                .ok_or_else(|| JsValue::from_str("MapCell vertex missing y coordinate"))?;
            let z = vertex
                .get(2)
                .as_f64()
                .ok_or_else(|| JsValue::from_str("MapCell vertex missing z coordinate"))?;
            vertices_vec.push([x, y, z]);
        }

        let map_cell = MapCell {
            is_explored,
            is_void,
            vertices: vertices_vec,
        };
        Ok(map_cell)
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
                let map_cell = serde_wasm_bindgen::to_value(&map_cell)?;
                output_cells.push(&map_cell);
            }
        } else {
            return Err(JsValue::from_str(&format!(
                "Voronoi generation failed: size/seed combo isn't viable (numCells={}, rngSeed={})",
                requested_cell_count, self.rng_seed
            )));
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

        spawn_local(async move {
            let mut stream = wasm_streams::ReadableStream::from_raw(receiver).into_stream();
            while let Some(event_result) = stream.next().await {
                match event_result {
                    Ok(event) => {
                        if let Some(message) = extract_message_text(&event) {
                            if message
                                .from
                                .as_deref()
                                .zip(local_endpoint_id.as_deref())
                                .is_some_and(|(from, local)| from == local)
                            {
                                continue;
                            }

                            if let Err(err) = apply_incoming_mutation(
                                &cells,
                                &ui_state,
                                MutationOrigin::Peer,
                                &message.text,
                            ) {
                                tracing::warn!(
                                    "failed to apply state mutation from peer: {:?}",
                                    err
                                );
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!("network event stream error: {:?}", err);
                        break;
                    }
                }
            }
        });
    }

    fn dispatch_explore_mutation(
        &self,
        mutation: Mutation,
        origin: MutationOrigin,
    ) -> Result<(), JsValue> {
        let is_remote = matches!(origin, MutationOrigin::Peer);
        let (index, [x, y, z]) = match mutation {
            Mutation::ExploreCell {
                index,
                pulse_position,
            } => (index, pulse_position),
        };
        self.ui_state
            .add_pulse_internal(index, x, y, z, PULSE_DURATION_MS, is_remote)
            .map(|_| ())?;

        if matches!(origin, MutationOrigin::Local) {
            self.broadcast_state_mutation(mutation);
        }

        let cells = self.cells.clone();
        spawn_local(async move {
            n0_future::time::sleep(Duration::from_millis(PULSE_DURATION_MS as u64)).await;
            if let Err(err) = mutation.apply(&cells) {
                tracing::warn!("failed to apply delayed mutation: {:?}", err);
            }
        });

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

fn extract_message_text(event: &JsValue) -> Option<NetworkTextMessage> {
    let event_type = Reflect::get(event, &JsValue::from_str("type"))
        .ok()?
        .as_string()?;
    if event_type != MESSAGE_RECEIVED_EVENT {
        return None;
    }

    let text = Reflect::get(event, &JsValue::from_str("text"))
        .ok()?
        .as_string()?;
    let from = Reflect::get(event, &JsValue::from_str("from"))
        .ok()
        .and_then(|value| value.as_string());

    Some(NetworkTextMessage { from, text })
}

fn apply_incoming_mutation(
    cells: &Rc<RefCell<Readable<Array>>>,
    ui_state: &UiState,
    origin: MutationOrigin,
    message_text: &str,
) -> Result<(), JsValue> {
    let Some(mutation) = Mutation::decode(message_text) else {
        return Ok(());
    };

    let is_remote = matches!(origin, MutationOrigin::Peer);
    let (index, [x, y, z]) = match mutation {
        Mutation::ExploreCell {
            index,
            pulse_position,
        } => (index, pulse_position),
    };

    ui_state
        .add_pulse_internal(index, x, y, z, PULSE_DURATION_MS, is_remote)
        .map(|_| ())?;

    let delayed_cells = cells.clone();
    spawn_local(async move {
        n0_future::time::sleep(Duration::from_millis(PULSE_DURATION_MS as u64)).await;
        if let Err(err) = mutation.apply(&delayed_cells) {
            tracing::warn!("failed to apply delayed incoming mutation: {:?}", err);
        }
    });

    Ok(())
}
fn mark_cell_explored(cells: &Rc<RefCell<Readable<Array>>>, index: usize) -> Result<(), JsValue> {
    cells.borrow_mut().set_with(|cells_array| {
        if index >= cells_array.length() as usize {
            return Ok::<(), JsValue>(());
        }

        let cell = cells_array.get(index as u32);
        Reflect::set(
            &cell,
            &JsValue::from_str("isExplored"),
            &JsValue::from_bool(true),
        )?;

        Ok(())
    })
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
        let h1 = vertex_height(12.345, 67.89, 4242, 0.4, (-0.4, 0.4));
        let h2 = vertex_height(12.345, 67.89, 4242, 0.4, (-0.4, 0.4));
        let h3 = vertex_height(12.345, 67.89, 4243, 0.4, (-0.4, 0.4));

        assert_eq!(h1, h2);
        assert!((h1 - h3).abs() > f64::EPSILON);
    }
}
