import { describe, expect, it } from "vitest";
import { revealSweep } from "./reveal";
import type { MapCell } from "$lib/wasm/avoidant_wasm";
const cell = (x: number): MapCell => ({
  centroid: [x, 0, 0],
  vertices: [
    [x - 1, 0, 0],
    [x + 1, 0, 0],
  ],
  neighbors: [],
  normal: [0, 1, 0],
});
describe("competitive reveal sweep", () => {
  it("sweeps flooded cells in distance order at the cooperative pulse speed", () => {
    const sweep = revealSweep([cell(0), cell(20)], [0, 1], 0);
    expect(sweep.maxRadius).toBe(22.5);
    expect(sweep.durationMs).toBe(225);
    expect(sweep.finishes[0].afterMs).toBeLessThan(sweep.finishes[1].afterMs);
    expect(sweep.finishes[1].afterMs).toBeLessThanOrEqual(sweep.durationMs);
  });
  it("uses the clicked world position and keeps single-cell reveals visible", () => {
    const sweep = revealSweep([cell(0)], [0], 0, [1, 0, 0]);
    expect(Array.from(sweep.position)).toEqual([1, 0, 0]);
    expect(sweep.durationMs).toBe(80);
    expect(sweep.finishes[0].afterMs).toBeGreaterThan(0);
  });
});
