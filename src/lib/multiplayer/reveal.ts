import type { MapCell } from "$lib/wasm/avoidant_wasm";

/** Match the cooperative renderer's world-space sweep speed and soft-edge timing. */
export function revealSweep(
  cells: MapCell[],
  opened: number[],
  originCell: number,
  position: [number, number, number] = cells[originCell].centroid,
) {
  const distance = (v: number[]) => Math.hypot(...v.map((n, i) => n - position[i]));
  const radii = opened.map((i) => Math.max(...cells[i].vertices.map(distance)));
  const maxRadius = Math.max(...radii) + 1.5;
  const durationMs = Math.max(80, maxRadius / 0.1);
  return {
    position: new Float64Array(position),
    maxRadius,
    durationMs,
    finishes: opened.map((cell, i) => ({
      cell,
      afterMs: Math.min(durationMs, (radii[i] / maxRadius + 0.12) * durationMs),
    })),
  };
}
