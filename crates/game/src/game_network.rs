use wasm_bindgen::JsValue;

use crate::GameState;
use crate::mutation::{Mutation, MutationOrigin};
use crate::sync_bridge::SyncBridge;

impl GameState {
    pub(crate) fn dispatch_explore_mutation(
        &self,
        mutation: Mutation,
        _origin: MutationOrigin,
    ) -> Result<(), JsValue> {
        let Mutation::ExploreCell {
            index,
            pulse_position,
        } = mutation;
        self.initialize_voids(index)?;
        self.sync_bridge().local(index, pulse_position)?;
        Ok(())
    }

    pub(crate) fn sync_bridge(&self) -> SyncBridge {
        SyncBridge {
            session: self.session.clone(),
            cells: self.cells.clone(),
            metadata: self.cell_metadata.clone(),
            score: self.score.clone(),
            ui: self.ui_state.clone(),
        }
    }
}
