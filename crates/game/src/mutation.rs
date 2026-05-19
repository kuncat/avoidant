use std::{cell::RefCell, rc::Rc};

use js_sys::Array;
use n0_future::time::Duration;
use svelte_store::Readable;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use crate::score::{self, ScoreState};
use crate::{CellMetadataEntry, PULSE_DURATION_MS, UiState};

const MUTATION_DELIMITER: char = '|';

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
pub(crate) enum Mutation {
    ExploreCell {
        index: usize,
        pulse_position: [f64; 3],
    },
}

impl Mutation {
    pub(crate) fn encode(self) -> String {
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

    pub(crate) fn decode(input: &str) -> Option<Self> {
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

    fn apply(
        self,
        cell_metadata: &Rc<RefCell<Readable<Array>>>,
        score_state: &Rc<RefCell<Readable<ScoreState>>>,
    ) -> Result<(), JsValue> {
        match self {
            Self::ExploreCell { index, .. } => {
                mark_cell_explored(cell_metadata, score_state, index)
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum MutationOrigin {
    Local,
    Peer,
}

pub(crate) fn apply_mutation_with_effects(
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    score_state: &Rc<RefCell<Readable<ScoreState>>>,
    ui_state: &UiState,
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

    ui_state
        .add_pulse_internal(index, x, y, z, PULSE_DURATION_MS, is_remote)
        .map(|_| ())?;

    let delayed_cell_metadata = cell_metadata.clone();
    let delayed_score_state = score_state.clone();
    spawn_local(async move {
        n0_future::time::sleep(Duration::from_millis(PULSE_DURATION_MS as u64)).await;
        if let Err(err) = mutation.apply(&delayed_cell_metadata, &delayed_score_state) {
            tracing::warn!("failed to apply delayed mutation: {:?}", err);
        }
    });

    Ok(())
}

fn mark_cell_explored(
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    score_state: &Rc<RefCell<Readable<ScoreState>>>,
    index: usize,
) -> Result<(), JsValue> {
    // Returns `Some(is_void)` only when the cell transitioned from
    // unexplored to explored; `None` means we should not touch the score.
    let transitioned_void: Option<bool> =
        cell_metadata.borrow_mut().set_with(|metadata_array| {
            if index >= metadata_array.length() as usize {
                return Ok::<Option<bool>, JsValue>(None);
            }

            let metadata = metadata_array.get(index as u32);
            let mut typed_metadata: CellMetadataEntry = serde_wasm_bindgen::from_value(metadata)
                .map_err(|err| {
                    JsValue::from_str(&format!("Failed to decode cell metadata from store: {err}"))
                })?;

            if typed_metadata.is_explored {
                // Already explored — score must not double-count.
                return Ok(None);
            }

            let is_void = typed_metadata.is_void;
            typed_metadata.mark_explored();

            let updated_metadata =
                serde_wasm_bindgen::to_value(&typed_metadata).map_err(|err| {
                    JsValue::from_str(&format!("Failed to encode updated cell metadata: {err}"))
                })?;
            metadata_array.set(index as u32, updated_metadata);

            Ok(Some(is_void))
        })?;

    if let Some(is_void) = transitioned_void {
        score::update_on_explore(score_state, is_void);
    }

    Ok(())
}
