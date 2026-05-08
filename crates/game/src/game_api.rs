use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use js_sys::{Array, JSON};
use svelte_store::Readable;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

use crate::mapgen;
use crate::mutation::{Mutation, MutationOrigin};
use crate::net::TicketOpts;
use crate::{
    CellMetadata, CellMetadataEntry, GameOptions, GameState, MapCell, MapCells, NetworkNode,
    NetworkPeerStatus, NetworkSnapshot, NetworkSnapshotStore, UiState, utils,
};
use networking::GameTicket;

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
        let initial_network_snapshot = NetworkSnapshot::empty();

        Ok(GameState {
            cells: Rc::new(RefCell::new(Readable::new(Array::new()))),
            cell_metadata: Rc::new(RefCell::new(Readable::new(Array::new()))),
            network_snapshot: Rc::new(RefCell::new(Readable::new_mapped(
                initial_network_snapshot,
                |snapshot| {
                    serde_wasm_bindgen::to_value(snapshot)
                        .expect("network snapshot serialization should not fail")
                },
            ))),
            connected_endpoints: Rc::new(RefCell::new(BTreeSet::new())),
            peer_presence: Rc::new(RefCell::new(Default::default())),
            last_inbound_mutation_ms: Rc::new(RefCell::new(None)),
            last_outbound_mutation_ms: Rc::new(RefCell::new(None)),
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

    #[wasm_bindgen(getter, js_name = "cellMetadata")]
    pub fn cell_metadata_store(&self) -> CellMetadata {
        self.cell_metadata.borrow().get_store().into()
    }

    #[wasm_bindgen(getter, js_name = "networkSnapshot")]
    pub fn network_snapshot_store(&self) -> NetworkSnapshotStore {
        self.network_snapshot.borrow().get_store().into()
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

    #[wasm_bindgen(getter, js_name = "networkListenerStarted")]
    pub fn network_listener_started(&self) -> bool {
        self.network_listener_started
    }

    #[wasm_bindgen(getter, js_name = "endpointId")]
    pub fn endpoint_id(&self) -> Option<String> {
        self.network_node.as_ref().map(|node| node.endpoint_id())
    }

    #[wasm_bindgen(getter, js_name = "topicId")]
    pub fn topic_id(&self) -> Option<String> {
        self.network_channel.as_ref().map(|channel| channel.id())
    }

    #[wasm_bindgen(getter, js_name = "networkPeers")]
    pub fn network_peers(&self) -> Vec<NetworkPeerStatus> {
        self.collect_network_peers()
    }

    pub(crate) fn collect_network_peers(&self) -> Vec<NetworkPeerStatus> {
        let connected_endpoints = self.connected_endpoints.borrow().clone();
        let mut all_endpoints = connected_endpoints.clone();
        all_endpoints.extend(self.peer_presence.borrow().keys().cloned());

        let peer_presence = self.peer_presence.borrow();
        all_endpoints
            .into_iter()
            .map(|endpoint_id| {
                let peer_entry = peer_presence.get(&endpoint_id);
                NetworkPeerStatus::new(
                    endpoint_id.clone(),
                    peer_entry.and_then(|entry| entry.nickname.clone()),
                    peer_entry.and_then(|entry| entry.last_seen_ms),
                    connected_endpoints.contains(&endpoint_id),
                )
            })
            .collect()
    }

    pub(crate) fn sync_network_snapshot(&self) {
        let snapshot = NetworkSnapshot {
            has_node: self.network_node.is_some(),
            listener_started: self.network_listener_started,
            endpoint_id: self.network_node.as_ref().map(|node| node.endpoint_id()),
            topic_id: self.network_channel.as_ref().map(|channel| channel.id()),
            peers: self.collect_network_peers(),
            last_inbound_mutation_ms: *self.last_inbound_mutation_ms.borrow(),
            last_outbound_mutation_ms: *self.last_outbound_mutation_ms.borrow(),
            sampled_at_ms: js_sys::Date::now(),
        };

        self.network_snapshot.borrow_mut().set(snapshot);
    }

    #[wasm_bindgen(getter, js_name = "lastInboundMutationMs")]
    pub fn last_inbound_mutation_ms(&self) -> Option<f64> {
        *self.last_inbound_mutation_ms.borrow()
    }

    #[wasm_bindgen(getter, js_name = "lastOutboundMutationMs")]
    pub fn last_outbound_mutation_ms(&self) -> Option<f64> {
        *self.last_outbound_mutation_ms.borrow()
    }

    fn apply_generated_cells(&mut self, cells: Vec<MapCell>) -> Result<JsValue, JsValue> {
        let output_cells = Array::new();
        let output_metadata = Array::new();
        for cell in cells {
            let map_cell = serde_wasm_bindgen::to_value(&cell)?;
            output_cells.push(&map_cell);

            let metadata = CellMetadataEntry::new(false, false);
            let metadata_value = serde_wasm_bindgen::to_value(&metadata)?;
            output_metadata.push(&metadata_value);
        }

        self.cells.borrow_mut().set(output_cells.clone());
        self.cell_metadata.borrow_mut().set(output_metadata);
        Ok(output_cells.into())
    }

    #[wasm_bindgen(js_name = "generateMapAsync")]
    pub async fn generate_map_async(&mut self) -> Result<JsValue, JsValue> {
        if self.num_cells > usize::MAX as u64 {
            return Err(JsValue::from_str(
                "numCells is too large for this target architecture",
            ));
        }

        let requested_cell_count = self.num_cells as usize;
        let cells = mapgen::generate_map_cells_async(
            requested_cell_count,
            self.rng_seed,
            self.max_samples,
            self.slack,
            self.spikiness,
            (self.elevation_min, self.elevation_max),
        )
        .await?;

        self.apply_generated_cells(cells)
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
            self.connected_endpoints
                .borrow_mut()
                .extend(channel.neighbors());
            self.network_channel = Some(channel);
            self.attach_network_listener();
            self.sync_network_snapshot();
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
        state.generate_map_async().await?;

        let node = NetworkNode::spawn().await.map_err(Into::<JsValue>::into)?;
        let channel = node
            .join(ticket, nickname)
            .await
            .map_err(Into::<JsValue>::into)?;
        state
            .connected_endpoints
            .borrow_mut()
            .extend(channel.neighbors());
        state.network_node = Some(node);
        state.network_channel = Some(channel);
        state.attach_network_listener();
        state.sync_network_snapshot();

        Ok(state)
    }
}
