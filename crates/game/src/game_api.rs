use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use js_sys::{Array, JSON};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_xoshiro::Xoshiro256PlusPlus;
use svelte_store::Readable;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

use crate::mutation::{Mutation, MutationOrigin};
use crate::net::TicketOpts;
use crate::score::ScoreState;
use crate::{
    CellMetadata, CellMetadataEntry, GameOptions, GameState, MapCell, MapCells, NetworkNode,
    NetworkPeerStatus, NetworkSnapshot, NetworkSnapshotStore, Score, UiState, utils,
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
        let relay_urls = options.relay_urls.clone().unwrap_or_default();
        let elevation_min = options.elevation_min.unwrap_or(-0.4);
        let elevation_max = options.elevation_max.unwrap_or(0.4);
        let void_fraction = options.void_fraction.unwrap_or(0.15625).clamp(0.0, 1.0);
        let initial_network_snapshot = NetworkSnapshot::empty();

        Ok(GameState {
            session: Rc::new(RefCell::new(crate::sync::Session::new(
                options.authority.clone(),
            ))),
            cells: Rc::new(RefCell::new(Readable::new(Array::new()))),
            cell_metadata: Rc::new(RefCell::new(Readable::new(Array::new()))),
            score: Rc::new(RefCell::new(Readable::new_mapped(
                ScoreState::default(),
                |state| {
                    serde_wasm_bindgen::to_value(state)
                        .expect("score state serialization should not fail")
                },
            ))),
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
            rng_seed: options.rng_seed,
            elevation_min,
            elevation_max,
            void_fraction,
            first_safe_cell: std::cell::Cell::new(options.first_safe_cell),
            network_node: None,
            network_channel: None,
            network_listener_started: false,
            game_options_json: options_json,
            relay_urls,
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

    /// Canonical score, synchronized from the host independently of reveal animations.
    #[wasm_bindgen(getter, js_name = "score")]
    pub fn score_store(&self) -> Score {
        self.score.borrow().get_store().into()
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
        let cell_count = cells.len();
        if self
            .first_safe_cell
            .get()
            .is_some_and(|index| index >= cell_count)
        {
            return Err(JsValue::from_str("Invalid opening cell in game options"));
        }
        let void_mask = self
            .first_safe_cell
            .get()
            .map(|safe| compute_void_mask(cell_count, self.rng_seed, self.void_fraction, safe))
            .unwrap_or_else(|| vec![false; cell_count]);
        let void_total = ((cell_count as f64 * self.void_fraction).floor() as usize)
            .min(cell_count.saturating_sub(1));

        let neighbors = cells.iter().map(|cell| cell.neighbors().to_vec()).collect();
        let mut logical_metadata = Vec::new();
        let output_cells = Array::new();
        let output_metadata = Array::new();
        for (index, cell) in cells.into_iter().enumerate() {
            let void_neighbor_count = if void_mask[index] {
                0
            } else {
                cell.neighbors()
                    .iter()
                    .filter(|n| {
                        let idx = **n as usize;
                        idx < cell_count && void_mask[idx]
                    })
                    .count()
                    .min(u8::MAX as usize) as u8
            };

            let map_cell = serde_wasm_bindgen::to_value(&cell)?;
            output_cells.push(&map_cell);

            let metadata = CellMetadataEntry::new(false, void_mask[index], void_neighbor_count);
            logical_metadata.push(metadata.clone());
            let metadata_value = serde_wasm_bindgen::to_value(&metadata)?;
            output_metadata.push(&metadata_value);
        }

        self.session
            .borrow_mut()
            .set_board(neighbors, logical_metadata, void_total as u32);
        self.cells.borrow_mut().set(output_cells.clone());
        self.cell_metadata.borrow_mut().set(output_metadata);
        self.score.borrow_mut().set_with(|state| {
            state.reset_for_map(cell_count as u32, void_total as u32);
        });
        Ok(output_cells.into())
    }

    /// Apply cells produced off-thread (typically by the mapgen Web Worker).
    ///
    /// `cells` must be a JS array of objects matching the `MapCell` shape (i.e. `{ vertices: [[x, y, z], ...] }`).
    #[wasm_bindgen(js_name = "applyMapCells", unchecked_return_type = "MapCell[]")]
    pub fn apply_map_cells(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "MapCell[]")] cells: JsValue,
    ) -> Result<JsValue, JsValue> {
        let parsed: Vec<MapCell> = serde_wasm_bindgen::from_value(cells)
            .map_err(|err| JsValue::from_str(&format!("Invalid map cells payload: {err}")))?;
        self.apply_generated_cells(parsed)
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
        let safe = self
            .first_safe_cell
            .get()
            .ok_or_else(|| JsValue::from_str("Explore a cell before inviting players"))?;
        let mut options: GameOptions =
            serde_wasm_bindgen::from_value(JSON::parse(&self.game_options_json)?)?;
        options.first_safe_cell = Some(safe);

        if self.network_node.is_none() {
            let node = NetworkNode::spawn_with_relay_urls(self.relay_urls.clone())
                .await
                .map_err(Into::<JsValue>::into)?;
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
            let id = self.network_node.as_ref().unwrap().endpoint_id();
            self.session.borrow_mut().local_id = Some(id.clone());
            self.session.borrow_mut().authority = Some(id);
            self.attach_network_listener();
            self.sync_network_snapshot();
        }

        let channel = self
            .network_channel
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Network channel is unavailable"))?;
        options.authority = self.session.borrow().authority.clone();
        options.sync_version = Some(crate::sync::PROTOCOL);
        let invite_options: String =
            JSON::stringify(&serde_wasm_bindgen::to_value(&options)?)?.into();
        let ticket_opts = TicketOpts {
            include_myself: true,
            include_bootstrap: true,
            include_neighbors: true,
        };

        channel
            .ticket_with_game_options(ticket_opts, Some(invite_options))
            .map_err(Into::<JsValue>::into)
    }

    #[wasm_bindgen(js_name = "optionsFromTicket")]
    pub fn options_from_ticket(ticket: String) -> Result<GameOptions, JsValue> {
        let parsed_ticket = GameTicket::deserialize(&ticket)
            .map_err(|err| JsValue::from_str(&format!("Invalid game ticket: {err}")))?;
        let options_json = parsed_ticket
            .game_options_json
            .ok_or_else(|| JsValue::from_str("Ticket does not include game options"))?;
        let options_value = JSON::parse(&options_json)?;
        let options: GameOptions = serde_wasm_bindgen::from_value(options_value)?;
        if options.sync_version != Some(crate::sync::PROTOCOL)
            || options.authority.is_none()
            || options.first_safe_cell.is_none()
        {
            return Err(JsValue::from_str(
                "Incompatible invitation; ask the host to create a new invite",
            ));
        }
        Ok(options)
    }

    /// Spawn a network node and join the gossip topic described by `ticket`.
    ///
    /// The caller is responsible for having populated the map cells (typically
    /// via [`GameState::apply_map_cells`]) before invoking this; the function
    /// waits for an authoritative snapshot before allowing gameplay.
    #[wasm_bindgen(js_name = "joinAsPeer")]
    pub async fn join_as_peer(&mut self, ticket: String, nickname: String) -> Result<(), JsValue> {
        let node = NetworkNode::spawn_with_relay_urls(self.relay_urls.clone())
            .await
            .map_err(Into::<JsValue>::into)?;
        let channel = node
            .join(ticket, nickname)
            .await
            .map_err(Into::<JsValue>::into)?;
        self.connected_endpoints
            .borrow_mut()
            .extend(channel.neighbors());
        self.session.borrow_mut().local_id = Some(node.endpoint_id());
        self.network_node = Some(node);
        self.network_channel = Some(channel);
        self.attach_network_listener();
        self.sync_network_snapshot();
        for _ in 0..200 {
            if self.session.borrow().ready {
                return Ok(());
            }
            n0_future::time::sleep(n0_future::time::Duration::from_millis(100)).await;
        }
        Err(JsValue::from_str(
            "Timed out waiting for the host's game state",
        ))
    }
}

impl GameState {
    pub(crate) fn initialize_voids(&self, safe: usize) -> Result<(), JsValue> {
        if self.first_safe_cell.get().is_some() {
            return Ok(());
        }
        let cells: Vec<MapCell> = {
            let store = self.cells.borrow();
            let array: &Array = &**store;
            serde_wasm_bindgen::from_value(array.clone().into())?
        };
        if safe >= cells.len() {
            return Err(JsValue::from_str("Invalid exploration cell"));
        }
        let mask = compute_void_mask(cells.len(), self.rng_seed, self.void_fraction, safe);
        let metadata = Array::new();
        for (index, cell) in cells.iter().enumerate() {
            let count = cell
                .neighbors()
                .iter()
                .filter(|&&neighbor| mask.get(neighbor as usize).copied().unwrap_or(false))
                .count()
                .min(u8::MAX as usize) as u8;
            metadata.push(&serde_wasm_bindgen::to_value(&CellMetadataEntry::new(
                false,
                mask[index],
                if mask[index] { 0 } else { count },
            ))?);
        }
        let logical: Vec<CellMetadataEntry> =
            serde_wasm_bindgen::from_value(metadata.clone().into())?;
        self.session.borrow_mut().set_board(
            cells.iter().map(|cell| cell.neighbors().to_vec()).collect(),
            logical,
            mask.iter().filter(|&&v| v).count() as u32,
        );
        self.first_safe_cell.set(Some(safe));
        self.cell_metadata.borrow_mut().set(metadata);
        Ok(())
    }
}

/// Deterministically choose which cell indices are void.
fn compute_void_mask(cell_count: usize, rng_seed: u64, fraction: f64, safe: usize) -> Vec<bool> {
    let mut mask = vec![false; cell_count];
    if cell_count == 0 || fraction <= 0.0 {
        return mask;
    }

    let clamped = fraction.clamp(0.0, 1.0);
    let void_count = ((cell_count as f64) * clamped).floor() as usize;
    if void_count == 0 {
        return mask;
    }

    let mut indices: Vec<usize> = (0..cell_count).filter(|&index| index != safe).collect();
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(rng_seed ^ 0xA110_CA7E_BEEF_5EEDu64);
    indices.shuffle(&mut rng);

    for &index in indices.iter().take(void_count) {
        mask[index] = true;
    }
    mask
}

#[cfg(test)]
mod opening_tests {
    use super::compute_void_mask;
    #[test]
    fn opening_is_safe_with_exact_void_count_and_repeatable_layout() {
        for count in [1, 4, 80, 160, 320] {
            for safe in 0..count {
                for fraction in [0.0, 0.15625, 0.999, 1.0] {
                    let mask = compute_void_mask(count, 42, fraction, safe);
                    assert!(!mask[safe]);
                    assert_eq!(
                        mask.iter().filter(|&&v| v).count(),
                        ((count as f64 * fraction).floor() as usize).min(count - 1)
                    );
                    assert_eq!(mask, compute_void_mask(count, 42, fraction, safe));
                }
            }
        }
    }
}
