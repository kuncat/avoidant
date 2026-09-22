import { describe, expect, it } from "vitest";
import { BufferAttribute, BufferGeometry, Matrix4 } from "three";
import { TerrainDepthSort } from "./terrain-depth-sort";

function surface() {
  const geometry = new BufferGeometry();
  geometry.setAttribute(
    "position",
    new BufferAttribute(
      new Float32Array([0, 0, -2, 1, 0, -2, 0, 1, -2, 0, 0, -8, 1, 0, -8, 0, 1, -8]),
      3,
    ),
  );
  geometry.setAttribute(
    "aCellNormal",
    new BufferAttribute(
      new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1]),
      3,
    ),
  );
  geometry.setAttribute("aCellIndex", new BufferAttribute(new Float32Array([7, 7, 7, 9, 9, 9]), 1));
  return geometry;
}

describe("transparent terrain depth order", () => {
  it("draws far triangles first and reverses with the camera, preserving picking IDs", () => {
    const geometry = surface();
    const sorter = new TerrainDepthSort(geometry);
    expect(Array.from(geometry.index!.array)).toEqual([0, 1, 2, 3, 4, 5]);
    sorter.update(new Matrix4(), 0, () => 0);
    expect(Array.from(geometry.index!.array)).toEqual([3, 4, 5, 0, 1, 2]);
    expect(geometry.getAttribute("aCellIndex").getX(geometry.index!.getX(0))).toBe(9);
    sorter.update(new Matrix4().makeRotationY(Math.PI), 0, () => 0);
    expect(Array.from(geometry.index!.array)).toEqual([0, 1, 2, 3, 4, 5]);
    geometry.dispose();
  });

  it("reorders falling cells and skips uploads for an unchanged view", () => {
    const geometry = surface();
    const sorter = new TerrainDepthSort(geometry);
    const view = new Matrix4();
    sorter.update(view, 0, () => 0);
    const version = geometry.index!.version;
    sorter.update(view, 0, () => 0);
    expect(geometry.index!.version).toBe(version);
    sorter.update(view, 1, (cell) => (cell === 7 ? 10 : 0));
    expect(Array.from(geometry.index!.array)).toEqual([0, 1, 2, 3, 4, 5]);
    geometry.dispose();
  });
});
