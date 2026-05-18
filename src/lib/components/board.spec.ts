import { describe, it, expect } from "vitest";
import { CellMetaChannel, cellMetaDefines } from "./board.svelte";

describe("CellMetaChannel", () => {
  const entries = Object.entries(CellMetaChannel);

  it("declares at least one channel", () => {
    expect(entries.length).toBeGreaterThan(0);
  });

  it.each(entries)("channel %s offset %d fits in an RGBA8 texel", (_name, offset) => {
    expect(Number.isInteger(offset)).toBe(true);
    // RGBA8 DataTexture has 4 bytes per texel; valid offsets are 0..3.
    expect(offset).toBeGreaterThanOrEqual(0);
    expect(offset).toBeLessThanOrEqual(3);
  });

  it("assigns each channel a unique offset", () => {
    const offsets = entries.map(([, v]) => v);
    expect(new Set(offsets).size).toBe(offsets.length);
  });

  it("does not exceed the four channels available in an RGBA8 texel", () => {
    expect(entries.length).toBeLessThanOrEqual(4);
  });
});

describe("cellMetaDefines", () => {
  it("emits one #define per CellMetaChannel entry", () => {
    const lines = cellMetaDefines.trim().split("\n");
    expect(lines).toHaveLength(Object.keys(CellMetaChannel).length);
    for (const line of lines) {
      expect(line).toMatch(/^#define CELL_META_[A-Z0-9_]+ \d+$/);
    }
  });

  it("emits CELL_META_<NAME> matching each channel's offset", () => {
    for (const [name, offset] of Object.entries(CellMetaChannel)) {
      const macro = `#define CELL_META_${name.toUpperCase()} ${offset}`;
      expect(cellMetaDefines).toContain(macro);
    }
  });
});
