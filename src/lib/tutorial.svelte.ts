import type { CellMetadataEntry, MapCell } from "$lib/wasm/avoidant_wasm";
import { m } from "$lib/paraglide/messages";

export type TutorialPhase =
  | { kind: "intro" }
  | { kind: "awaitingFirstClick" }
  | { kind: "afterVoid"; cellIndex: number }
  | { kind: "afterSafeGuess"; cellIndex: number }
  | { kind: "afterSafeDeducible"; clickedCellIndex: number; deducibleCellIndex: number }
  | { kind: "won"; efficiency: number }
  | { kind: "done" };

export class TutorialState {
  phase = $state<TutorialPhase>({ kind: "intro" });

  get text(): string {
    switch (this.phase.kind) {
      case "intro":
        return m.tutorial_intro();
      case "awaitingFirstClick":
        return m.tutorial_awaiting_first_click();
      case "afterVoid":
        return m.tutorial_after_void();
      case "afterSafeGuess":
        return m.tutorial_after_safe_guess();
      case "afterSafeDeducible":
        return m.tutorial_after_safe_deducible();
      case "won":
        return m.tutorial_won({ percent: Math.round(this.phase.efficiency * 100) });
      case "done":
        return "";
    }
  }

  /** Whether the UI should show a Next button to advance manually. */
  get canAdvance(): boolean {
    return this.phase.kind === "intro";
  }

  /** Whether cell click handlers should forward clicks to the game. */
  get isExplorationAllowed(): boolean {
    return this.phase.kind !== "intro" && this.phase.kind !== "won" && this.phase.kind !== "done";
  }

  /** Cell to visually highlight as provably safe, if any. */
  get highlightedCellIndex(): number | undefined {
    return this.phase.kind === "afterSafeDeducible" ? this.phase.deducibleCellIndex : undefined;
  }

  /** Advance from the intro to the interactive phase. */
  next(): void {
    if (this.phase.kind === "intro") {
      this.phase = { kind: "awaitingFirstClick" };
    }
  }

  observeExplore(
    cellIndex: number,
    cells: ReadonlyArray<MapCell>,
    metadata: ReadonlyArray<CellMetadataEntry>,
  ): void {
    if (!this.isExplorationAllowed) return;
    const entry = metadata[cellIndex];
    if (!entry) return;
    if (entry.isVoid) {
      this.phase = { kind: "afterVoid", cellIndex };
      return;
    }
    const deducible = findDeducibleSafeCell(cells, metadata, cellIndex);
    this.phase =
      deducible === undefined
        ? { kind: "afterSafeGuess", cellIndex }
        : {
            kind: "afterSafeDeducible",
            clickedCellIndex: cellIndex,
            deducibleCellIndex: deducible,
          };
  }

  observeWin(efficiency: number): void {
    if (this.phase.kind === "won" || this.phase.kind === "done") return;
    this.phase = { kind: "won", efficiency };
  }

  dismiss(): void {
    this.phase = { kind: "done" };
  }
}

/**
 * Find an unexplored cell that is provably safe given currently revealed metadata. This is not a comprehensive solver and often does not return any cell even when some safe cells are provable.
 *
 * A cell `n` is provably safe when some explored, non-void neighbour `c` has `voidNeighborCount` equal to the number of `c`'s neighbours already known to be voids — meaning every remaining unexplored neighbour of `c` (including `n`) must be safe. Returns the first such `n` found, preferring a neighbour of the freshly-clicked cell so the highlight is contextually adjacent.
 */
export function findDeducibleSafeCell(
  cells: ReadonlyArray<MapCell>,
  metadata: ReadonlyArray<CellMetadataEntry>,
  preferredSourceCell?: number,
): number | undefined {
  const tryFrom = (sourceCell: number): number | undefined => {
    const entry = metadata[sourceCell];
    if (!entry || !entry.isExplored || entry.isVoid) return undefined;
    const neighbours = cells[sourceCell]?.neighbors;
    if (!neighbours) return undefined;
    let knownVoids = 0;
    let firstUnexplored: number | undefined;
    for (const n of neighbours) {
      if (!metadata[n]) continue;
      if (metadata[n].isExplored) {
        if (metadata[n].isVoid) knownVoids++;
      } else if (firstUnexplored === undefined) {
        firstUnexplored = n;
      }
    }
    if (firstUnexplored !== undefined && entry.voidNeighborCount === knownVoids) {
      return firstUnexplored;
    }
    return undefined;
  };

  if (preferredSourceCell !== undefined) {
    const direct = tryFrom(preferredSourceCell);
    if (direct !== undefined) return direct;
  }
  for (let c = 0; c < cells.length; c++) {
    if (c === preferredSourceCell) continue;
    const found = tryFrom(c);
    if (found !== undefined) return found;
  }
  return undefined;
}
