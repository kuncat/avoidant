use std::{cell::RefCell, collections::HashSet, collections::VecDeque, rc::Rc};

use js_sys::Array;
use n0_future::time::Duration;
use svelte_store::Readable;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use crate::score::{self, ScoreState};
use crate::{
    CellMetadataEntry, MapCell, PULSE_MIN_DURATION_MS, PULSE_SWEEP_BAND, PULSE_SWEEP_VELOCITY,
    UiState,
};

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
}

#[derive(Clone, Copy)]
pub(crate) enum MutationOrigin {
    Local,
    Peer,
}

/// Apply an inbound (local or peer) mutation.
///
/// The combined `is_explored` and `is_revealing` update prevents a one-frame race where the pulse is gone but `is_revealing` is still set.
pub(crate) fn apply_mutation_with_effects(
    cells: &Rc<RefCell<Readable<Array>>>,
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    score_state: &Rc<RefCell<Readable<ScoreState>>>,
    ui_state: &UiState,
    mutation: Mutation,
    origin: MutationOrigin,
) -> Result<(), JsValue> {
    let is_remote = matches!(origin, MutationOrigin::Peer);
    let (seed_index, pulse_position) = match mutation {
        Mutation::ExploreCell {
            index,
            pulse_position,
        } => (index, pulse_position),
    };
    let [px, py, pz] = pulse_position;

    let reveal_indices = compute_reveal_set(cells, cell_metadata, seed_index)?;
    if reveal_indices.is_empty() {
        return Ok(());
    }

    let (pulse_max_radius, pulse_duration_ms, finish_schedule): (f64, u32, Vec<(usize, u32)>) = {
        let cells_ref = cells.borrow();
        let cells_array: &Array = &**cells_ref;
        let len = cells_array.length() as usize;

        // (idx, farthest_vertex_distance_from_click)
        let mut per_cell: Vec<(usize, f64)> = Vec::with_capacity(reveal_indices.len());
        for &idx in &reveal_indices {
            if idx >= len {
                continue;
            }
            let cell: MapCell = match serde_wasm_bindgen::from_value(cells_array.get(idx as u32)) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let mut max_d2: f64 = 0.0;
            for (vx, vz) in cell.vertex_xz() {
                let dx = vx - px;
                let dz = vz - pz;
                let d2 = dx * dx + dz * dz;
                if d2 > max_d2 {
                    max_d2 = d2;
                }
            }
            per_cell.push((idx, max_d2.sqrt()));
        }

        let farthest = per_cell.iter().map(|&(_, d)| d).fold(0.0_f64, f64::max);
        let max_radius = farthest + 1.5;

        // Pulse duration scales with `max_radius` so the visible sweep band moves at a constant world-space velocity regardless of how far the chord reaches. A small floor keeps single-cell reveals visible for at least one frame.
        let duration_ms = ((max_radius / PULSE_SWEEP_VELOCITY) as u32).max(PULSE_MIN_DURATION_MS);

        // finish_ms = clamp((d/maxR) + band, 0, 1) * duration
        let duration = duration_ms as f64;
        let mut schedule: Vec<(usize, u32)> = per_cell
            .into_iter()
            .map(|(idx, d)| {
                let progress = (d / max_radius + PULSE_SWEEP_BAND).clamp(0.0, 1.0);
                (idx, (progress * duration) as u32)
            })
            .collect();
        schedule.sort_by_key(|&(_, t)| t);

        (max_radius, duration_ms, schedule)
    };

    // Flag each cell so the shader switches it from "unexplored" to the pulse-sweep gradient on the very next frame.
    cell_metadata
        .borrow_mut()
        .set_with(|metadata_array| -> Result<(), JsValue> {
            for &idx in &reveal_indices {
                if idx >= metadata_array.length() as usize {
                    continue;
                }
                let metadata_js = metadata_array.get(idx as u32);
                let mut entry: CellMetadataEntry = serde_wasm_bindgen::from_value(metadata_js)
                    .map_err(|err| {
                        JsValue::from_str(&format!(
                            "Failed to decode cell metadata for reveal flag: {err}"
                        ))
                    })?;
                entry.set_revealing(true);
                let updated = serde_wasm_bindgen::to_value(&entry).map_err(|err| {
                    JsValue::from_str(&format!(
                        "Failed to encode cell metadata for reveal flag: {err}"
                    ))
                })?;
                metadata_array.set(idx as u32, updated);
            }
            Ok(())
        })?;

    let pulse_id = ui_state.add_pulse_internal(
        seed_index,
        px,
        py,
        pz,
        pulse_duration_ms,
        is_remote,
        pulse_max_radius,
    )?;

    let cell_metadata = cell_metadata.clone();
    let score_state = score_state.clone();
    let ui_state = ui_state.clone();
    spawn_local(async move {
        // Walk the schedule in time order, batching cells whose finish moments fall within the same ~16 ms frame slice into a single `finalize_reveal` call.
        // The shader's per-fragment smoothstep still gives every cell its own visible sweep timing, so coalescing the metadata flip into frame buckets is imperceptible.
        const FRAME_MS: u32 = 16;
        let mut prev_ms: u32 = 0;
        let mut i = 0;
        while i < finish_schedule.len() {
            // Round this cell's finish_ms up to the next frame boundary
            // and pull in every later cell that also finishes by then.
            let bucket_deadline = finish_schedule[i]
                .1
                .div_ceil(FRAME_MS)
                .saturating_mul(FRAME_MS);
            let mut j = i + 1;
            while j < finish_schedule.len() && finish_schedule[j].1 <= bucket_deadline {
                j += 1;
            }
            let bucket: Vec<usize> = finish_schedule[i..j].iter().map(|&(idx, _)| idx).collect();

            let wait_ms = bucket_deadline.saturating_sub(prev_ms);
            if wait_ms > 0 {
                n0_future::time::sleep(Duration::from_millis(wait_ms as u64)).await;
            }
            if let Err(err) = finalize_reveal(&cell_metadata, &score_state, &bucket) {
                tracing::warn!("failed to finalize chord reveal batch: {:?}", err);
            }
            prev_ms = bucket_deadline;
            i = j;
        }

        let remaining = pulse_duration_ms.saturating_sub(prev_ms);
        if remaining > 0 {
            n0_future::time::sleep(Duration::from_millis(remaining as u64)).await;
        }
        if let Err(err) = ui_state.remove_pulse_by_id(pulse_id) {
            tracing::warn!("failed to remove pulse after reveal: {:?}", err);
        }
    });

    Ok(())
}

/// End-of-pulse cleanup: clear `is_revealing` for every chord cell, flip `is_explored` for any that weren't already explored, and score each newly-revealed cell.
fn finalize_reveal(
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    score_state: &Rc<RefCell<Readable<ScoreState>>>,
    reveal_indices: &[usize],
) -> Result<(), JsValue> {
    let newly_explored: Vec<bool> =
        cell_metadata
            .borrow_mut()
            .set_with(|metadata_array| -> Result<Vec<bool>, JsValue> {
                let mut newly: Vec<bool> = Vec::new();
                for &idx in reveal_indices {
                    if idx >= metadata_array.length() as usize {
                        continue;
                    }
                    let metadata_js = metadata_array.get(idx as u32);
                    let mut entry: CellMetadataEntry = serde_wasm_bindgen::from_value(metadata_js)
                        .map_err(|err| {
                            JsValue::from_str(&format!(
                                "Failed to decode cell metadata for finalize: {err}"
                            ))
                        })?;
                    entry.set_revealing(false);
                    if !entry.is_explored {
                        entry.mark_explored();
                        newly.push(entry.is_void);
                    }
                    let updated = serde_wasm_bindgen::to_value(&entry).map_err(|err| {
                        JsValue::from_str(&format!(
                            "Failed to encode cell metadata for finalize: {err}"
                        ))
                    })?;
                    metadata_array.set(idx as u32, updated);
                }
                Ok(newly)
            })?;

    for is_void in newly_explored {
        score::update_on_explore(score_state, is_void);
    }

    Ok(())
}

/// Build the BFS closure of cells that the chord auto-reveal will flip, starting from `seed`.]
fn compute_reveal_set(
    cells: &Rc<RefCell<Readable<Array>>>,
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    seed: usize,
) -> Result<Vec<usize>, JsValue> {
    let cells_ref = cells.borrow();
    let cells_array: &Array = &**cells_ref;
    let metadata_ref = cell_metadata.borrow();
    let metadata_array: &Array = &**metadata_ref;
    let len = metadata_array.length() as usize;

    if seed >= len {
        return Ok(Vec::new());
    }

    let seed_entry: CellMetadataEntry =
        serde_wasm_bindgen::from_value(metadata_array.get(seed as u32)).map_err(|err| {
            JsValue::from_str(&format!("Failed to decode seed cell metadata: {err}"))
        })?;
    if seed_entry.is_explored {
        return Ok(Vec::new());
    }

    let mut visited: HashSet<usize> = HashSet::new();
    let mut order: Vec<usize> = Vec::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    queue.push_back(seed);
    visited.insert(seed);

    while let Some(i) = queue.pop_front() {
        let entry: CellMetadataEntry = serde_wasm_bindgen::from_value(metadata_array.get(i as u32))
            .map_err(|err| JsValue::from_str(&format!("Failed to decode chord metadata: {err}")))?;
        if entry.is_explored {
            continue;
        }
        order.push(i);

        if entry.is_void || entry.void_neighbor_count != 0 {
            continue;
        }

        let cell: MapCell = serde_wasm_bindgen::from_value(cells_array.get(i as u32))
            .map_err(|err| JsValue::from_str(&format!("Failed to decode chord cell: {err}")))?;
        for &neighbor in cell.neighbors() {
            let n_idx = neighbor as usize;
            if n_idx >= len {
                continue;
            }
            if !visited.insert(n_idx) {
                continue;
            }
            queue.push_back(n_idx);
        }
    }

    Ok(order)
}
