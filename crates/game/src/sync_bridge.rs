use crate::mutation::{Mutation, MutationOrigin, animate_reveal};
use crate::sync::{Request, Session};
use crate::{CellMetadataEntry, ScoreState, UiState};
use js_sys::Array;
use std::{cell::RefCell, rc::Rc};
use svelte_store::Readable;
use wasm_bindgen::JsValue;

#[derive(Clone)]
pub(crate) struct SyncBridge {
    pub session: Rc<RefCell<Session>>,
    pub cells: Rc<RefCell<Readable<Array>>>,
    pub metadata: Rc<RefCell<Readable<Array>>>,
    pub score: Rc<RefCell<Readable<ScoreState>>>,
    pub ui: UiState,
}

impl SyncBridge {
    pub fn local(&self, index: usize, position: [f64; 3]) -> Result<(), JsValue> {
        let request = self.session.borrow_mut().request(index, position);
        if let Some(request) = request {
            self.session.borrow_mut().apply("offline", &request);
            self.present(Some(&request), MutationOrigin::Local)?;
        }
        Ok(())
    }

    /// Publish canonical score immediately; animation only controls presentation flags.
    pub fn present(
        &self,
        movement: Option<&Request>,
        origin: MutationOrigin,
    ) -> Result<(), JsValue> {
        let (entries, score) = {
            let session = self.session.borrow();
            (session.metadata.clone(), session.score.clone())
        };
        let previous: Array = {
            let store = self.metadata.borrow();
            let array: &Array = &**store;
            array.clone()
        };
        let next = Array::new();
        let mut animate = vec![];
        for (index, mut cell) in entries.into_iter().enumerate() {
            let old =
                serde_wasm_bindgen::from_value::<CellMetadataEntry>(previous.get(index as u32))
                    .ok();
            if cell.is_explored {
                if let Some(old) = old.filter(|old| old.is_explored || old.is_revealing) {
                    cell.is_explored = old.is_explored;
                    cell.is_revealing = old.is_revealing;
                } else if movement.is_some() {
                    cell.is_explored = false;
                    animate.push(index);
                }
            }
            next.push(&serde_wasm_bindgen::to_value(&cell)?);
        }
        self.metadata.borrow_mut().set(next);
        self.score.borrow_mut().set(score);
        if let Some(request) = movement {
            animate_reveal(
                &self.cells,
                &self.metadata,
                &self.ui,
                Mutation::ExploreCell {
                    index: request.index,
                    pulse_position: request.position,
                },
                origin,
                animate,
            )?;
        }
        Ok(())
    }
}
