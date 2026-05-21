//! Per-game scoring metrics.

use std::{cell::RefCell, rc::Rc};

use serde::{Deserialize, Serialize};
use svelte_store::Readable;
use tsify::Tsify;

/// Base points awarded per safe (non-void) explore, before streak multiplier.
pub const SAFE_REWARD: f64 = 1.0;
/// Streak multiplier increment per consecutive safe explore.
pub const STREAK_BONUS_PER_STEP: f64 = 0.1;
/// Maximum number of streak steps that contribute to the multiplier.
pub const STREAK_BONUS_CAP_STEPS: u32 = 10;
/// One-shot completion bonus expressed as a fraction of the safe-only baseline `SAFE_REWARD * safe_total` (i.e. without streak multipliers).
pub const COMPLETION_BONUS_FRACTION: f64 = 0.25;

#[derive(Clone, Default, Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct ScoreState {
    /// Risk-balanced point total. May be negative.
    ///
    /// - Each non-void cell explored awards [`SAFE_REWARD`] points, multiplied by a streak multiplier that grows by [`STREAK_BONUS_PER_STEP`] per consecutive non-void cell explored (capped at [`STREAK_BONUS_CAP_STEPS`] steps).
    /// - Each void explore deducts a *risk-balanced* penalty equal to `reward * (1 - p) / p`, where `p` is the realized void fraction. This makes random clicking have an expected value of zero so positive scores reflect *information* the player applied, not map size.
    /// - Clearing every non-void cell awards a completion bonus equal to [`COMPLETION_BONUS_FRACTION`] of the non-void-only baseline.
    pub score: f64,
    /// Non-void cells explored so far.
    pub safe_explored: u32,
    /// Void cells explored so far (each large-penalized and ends the streak).
    pub void_explored: u32,
    /// Current run of consecutive safe explores; resets to 0 on a void.
    pub streak: u32,
    /// Longest streak achieved during this game.
    pub best_streak: u32,
    /// Total cells on the map.
    pub total_cells: u32,
    /// Void cell count.
    pub void_total: u32,
    /// True once every non-void cell has been explored.
    pub completed: bool,
    /// `score / max_score`, in `[0, 1]`. Zero before the map has been generated or if the map contains no safe cells.
    pub efficiency: f64,
}

impl ScoreState {
    /// Reset all scoring counters and record the realized cell / void counts.
    pub(crate) fn reset_for_map(&mut self, total_cells: u32, void_total: u32) {
        *self = Self {
            total_cells,
            void_total: void_total.min(total_cells),
            ..Self::default()
        };
        self.recompute_derived();
    }

    fn recompute_derived(&mut self) {
        let safe_total = self.total_cells.saturating_sub(self.void_total);
        self.completed = safe_total > 0 && self.safe_explored >= safe_total;
        self.efficiency = max_score(safe_total)
            .map(|max| (self.score / max).clamp(0.0, 1.0))
            .unwrap_or(0.0);
    }
}

/// Risk-balanced void penalty so that random clicking has E\[score] = 0: `reward * (1 - p) / p`, with `p = void_total / total_cells`.
///
/// Returns `0.0` for maps with no voids or no cells.
fn void_penalty(total_cells: u32, void_total: u32) -> f64 {
    if void_total == 0 || total_cells == 0 {
        return 0.0;
    }
    let p = f64::from(void_total) / f64::from(total_cells);
    SAFE_REWARD * (1.0 - p) / p
}

/// Maximum achievable score: every safe cell explored at the capped streak multiplier, plus the completion bonus, and no explored voids.
fn max_score(safe_total: u32) -> Option<f64> {
    if safe_total == 0 {
        return None;
    }
    // Sum the streak ramp: first cell gets 1.0, second 1.1, ..., up to the capped multiplier.
    let mut safe_points = 0.0;
    for i in 0..safe_total {
        let streak = i as u32;
        let multiplier = streak_multiplier(streak);
        safe_points += SAFE_REWARD * multiplier;
    }
    let completion_bonus = COMPLETION_BONUS_FRACTION * SAFE_REWARD * f64::from(safe_total);
    Some(safe_points + completion_bonus)
}

fn streak_multiplier(streak: u32) -> f64 {
    let capped = f64::from(streak.min(STREAK_BONUS_CAP_STEPS));
    1.0 + capped * STREAK_BONUS_PER_STEP
}

/// Apply the score change for a single newly explored cell.
///
/// `is_void` reflects the cell's pre-explore void status. Callers MUST only invoke this once per cell-explore transition.
pub(crate) fn update_on_explore(store: &Rc<RefCell<Readable<ScoreState>>>, is_void: bool) {
    store.borrow_mut().set_with(|state| {
        let was_completed = state.completed;

        if is_void {
            state.void_explored = state.void_explored.saturating_add(1);
            state.streak = 0;
            state.score -= void_penalty(state.total_cells, state.void_total);
        } else {
            state.safe_explored = state.safe_explored.saturating_add(1);
            // Award the current streak's multiplier, then advance the streak so the next safe explore earns a higher reward.
            state.score += SAFE_REWARD * streak_multiplier(state.streak);
            state.streak = state.streak.saturating_add(1);
            if state.streak > state.best_streak {
                state.best_streak = state.streak;
            }
        }

        state.recompute_derived();

        // One-shot completion bonus on the transition into `completed`.
        if !was_completed && state.completed {
            let safe_total = state.total_cells.saturating_sub(state.void_total);
            state.score += COMPLETION_BONUS_FRACTION * SAFE_REWARD * f64::from(safe_total);
            state.recompute_derived();
        }
    });
}
