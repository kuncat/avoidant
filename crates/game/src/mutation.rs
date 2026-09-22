use std::collections::{HashSet, VecDeque};
use std::{cell::RefCell, rc::Rc};

use js_sys::Array;
use n0_future::time::Duration;
use svelte_store::Readable;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use crate::{
    CellMetadataEntry, MapCell, PULSE_MIN_DURATION_MS, PULSE_SWEEP_BAND, PULSE_SWEEP_VELOCITY,
    UiState,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Mutation {
    ExploreCell {
        index: usize,
        pulse_position: [f64; 3],
    },
}

#[derive(Clone, Copy)]
pub(crate) enum MutationOrigin {
    Local,
    Peer,
}

/// Animate cells already committed by the authoritative game state.
///
/// The combined `is_explored` and `is_revealing` update prevents a one-frame race where the pulse is gone but `is_revealing` is still set.
pub(crate) fn animate_reveal(
    cells: &Rc<RefCell<Readable<Array>>>,
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    ui_state: &UiState,
    mutation: Mutation,
    origin: MutationOrigin,
    reveal_indices: Vec<usize>,
) -> Result<(), JsValue> {
    let is_remote = matches!(origin, MutationOrigin::Peer);
    let (seed_index, pulse_position) = match mutation {
        Mutation::ExploreCell {
            index,
            pulse_position,
        } => (index, pulse_position),
    };
    let [px, py, pz] = pulse_position;

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
            for [vx, vy, vz] in cell.vertex_xyz() {
                let dx = vx - px;
                let dy = vy - py;
                let dz = vz - pz;
                let d2 = dx * dx + dy * dy + dz * dz;
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
            if let Err(err) = finalize_reveal(&cell_metadata, &bucket) {
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

/// End-of-pulse cleanup changes presentation only, never gameplay or score.
fn finalize_reveal(
    cell_metadata: &Rc<RefCell<Readable<Array>>>,
    reveal_indices: &[usize],
) -> Result<(), JsValue> {
    cell_metadata
        .borrow_mut()
        .set_with(|array| -> Result<(), JsValue> {
            for &index in reveal_indices {
                let mut cell: CellMetadataEntry =
                    serde_wasm_bindgen::from_value(array.get(index as u32))?;
                cell.is_revealing = false;
                cell.is_explored = true;
                array.set(index as u32, serde_wasm_bindgen::to_value(&cell)?);
            }
            Ok(())
        })
}

pub(crate) fn reveal_set(
    len: usize,
    seed: usize,
    mut metadata: impl FnMut(usize) -> Result<CellMetadataEntry, JsValue>,
    mut neighbors: impl FnMut(usize) -> Result<Vec<u32>, JsValue>,
) -> Result<Vec<usize>, JsValue> {
    if seed >= len {
        return Ok(Vec::new());
    }

    let seed_entry = metadata(seed)?;
    if seed_entry.is_explored || seed_entry.is_revealing {
        return Ok(Vec::new());
    }

    let mut visited: HashSet<usize> = HashSet::new();
    let mut order: Vec<usize> = Vec::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    queue.push_back(seed);
    visited.insert(seed);

    while let Some(i) = queue.pop_front() {
        let entry = metadata(i)?;
        if entry.is_explored || entry.is_revealing {
            continue;
        }
        order.push(i);

        if entry.is_void || entry.void_neighbor_count != 0 {
            continue;
        }

        for neighbor in neighbors(i)? {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveals_cross_faces_and_stop_at_numbered_cells() {
        let map =
            crate::mapgen::generate_map(60, 42, &crate::MapShape::Cube { radius: 50.0 }).unwrap();
        let run = |seed, void: Option<usize>, busy: bool| {
            reveal_set(
                map.cells.len(),
                seed,
                |i| {
                    let count = map.cells[i]
                        .neighbors()
                        .iter()
                        .filter(|&&n| Some(n as usize) == void)
                        .count() as u8;
                    let mut entry = CellMetadataEntry::new(false, Some(i) == void, count);
                    entry.is_revealing = busy;
                    Ok(entry)
                },
                |i| Ok(map.cells[i].neighbors().to_vec()),
            )
            .unwrap()
        };
        assert_eq!(run(0, None, false).len(), map.cells.len());
        assert!(run(0, None, true).is_empty());
        assert!(run(map.cells.len(), None, false).is_empty());
        let mine = 0;
        assert_eq!(run(mine, Some(mine), false), vec![mine]);
        let number = map.cells[mine].neighbors()[0] as usize;
        assert_eq!(run(number, Some(mine), false), vec![number]);
        let blank = (0..map.cells.len())
            .find(|&i| i != mine && !map.cells[i].neighbors().contains(&(mine as u32)))
            .unwrap();
        let revealed = run(blank, Some(mine), false);
        assert!(!revealed.contains(&mine));
        assert!(revealed.len() > 1);
        assert!(
            revealed
                .iter()
                .any(|&i| map.cells[i].normal != map.cells[blank].normal)
        );
    }
}
