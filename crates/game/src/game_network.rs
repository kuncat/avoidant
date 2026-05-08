use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use crate::GameState;
use crate::listener;
use crate::mutation::{Mutation, MutationOrigin, apply_mutation_with_effects};

impl GameState {
    pub(crate) fn attach_network_listener(&mut self) {
        if self.network_listener_started {
            return;
        }

        let Some(channel) = self.network_channel.as_mut() else {
            return;
        };

        let cell_metadata = self.cell_metadata.clone();
        let ui_state = self.ui_state.clone();
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
            cell_metadata,
            ui_state,
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
        apply_mutation_with_effects(&self.cell_metadata, &self.ui_state, mutation, origin)?;

        if matches!(origin, MutationOrigin::Local) {
            *self.last_outbound_mutation_ms.borrow_mut() = Some(js_sys::Date::now());
            self.broadcast_state_mutation(mutation);
            self.sync_network_snapshot();
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
