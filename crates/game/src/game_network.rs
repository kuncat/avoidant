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

        let cells = self.cells.clone();
        let ui_state = self.ui_state.clone();
        let local_endpoint_id = self.network_node.as_ref().map(|node| node.endpoint_id());
        let receiver = channel.receiver();
        self.network_listener_started = true;

        listener::spawn_network_listener(receiver, cells, ui_state, local_endpoint_id);
    }

    pub(crate) fn dispatch_explore_mutation(
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
