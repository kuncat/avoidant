use wasm_bindgen::JsValue;

use crate::GameState;
use crate::listener;
use crate::mutation::{Mutation, MutationOrigin};
use crate::sync_bridge::SyncBridge;

impl GameState {
    pub(crate) fn attach_network_listener(&mut self) {
        if self.network_listener_started {
            return;
        }

        let sync = self.sync_bridge();
        let Some(channel) = self.network_channel.as_mut() else {
            return;
        };

        let connected_endpoints = self.connected_endpoints.clone();
        let peer_presence = self.peer_presence.clone();
        let network_snapshot = self.network_snapshot.clone();
        let last_inbound_mutation_ms = self.last_inbound_mutation_ms.clone();
        let last_outbound_mutation_ms = self.last_outbound_mutation_ms.clone();
        let local_endpoint_id = self.network_node.as_ref().map(|node| node.endpoint_id());
        let topic_id = Some(channel.id());
        let receiver = channel.receiver();
        self.network_listener_started = true;

        listener::spawn_network_listener(
            receiver,
            sync,
            connected_endpoints,
            peer_presence,
            network_snapshot,
            last_inbound_mutation_ms,
            last_outbound_mutation_ms,
            local_endpoint_id,
            topic_id,
        );
        self.sync_network_snapshot();
    }

    pub(crate) fn dispatch_explore_mutation(
        &self,
        mutation: Mutation,
        origin: MutationOrigin,
    ) -> Result<(), JsValue> {
        let Mutation::ExploreCell {
            index,
            pulse_position,
        } = mutation;
        self.initialize_voids(index)?;
        self.sync_bridge().local(index, pulse_position)?;
        if matches!(origin, MutationOrigin::Local) {
            *self.last_outbound_mutation_ms.borrow_mut() = Some(js_sys::Date::now());
            self.sync_network_snapshot();
        }
        Ok(())
    }

    pub(crate) fn sync_bridge(&self) -> SyncBridge {
        SyncBridge {
            session: self.session.clone(),
            cells: self.cells.clone(),
            metadata: self.cell_metadata.clone(),
            score: self.score.clone(),
            ui: self.ui_state.clone(),
            sender: self.network_channel.as_ref().map(|c| c.sender()),
        }
    }
}
