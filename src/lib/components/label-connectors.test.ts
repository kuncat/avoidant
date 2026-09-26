import { describe, expect, it } from "vitest";
import { BufferAttribute, BufferGeometry, Vector3 } from "three";
import { labelSurfacePoints } from "./label-connectors";

function terrain(vertices: number[], cells: number[]) {
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new BufferAttribute(new Float32Array(vertices), 3));
  geometry.setAttribute("aCellIndex", new BufferAttribute(new Float32Array(cells), 1));
  return geometry;
}

describe("label surface connectors", () => {
  it("reaches a surface just beyond the base centroid instead of omitting the line", () => {
    const geometry = terrain([-1, -0.01, -1, 1, -0.01, -1, 0, -0.01, 1], [0, 0, 0]);
    const [point] = labelSurfacePoints(geometry, [new Vector3(0, 2, 0)], [new Vector3()]);
    expect(point?.x).toBeCloseTo(0);
    expect(point?.y).toBeCloseTo(-0.01);
    expect(point?.z).toBeCloseTo(0);
  });

  it("attaches to its own inset edge when the center ray misses, not the adjacent cell", () => {
    const geometry = terrain(
      [0.1, 0, -1, 1, 0, -1, 0.1, 0, 1, -1, 1, -1, 1, 1, -1, 0, 1, 1],
      [0, 0, 0, 1, 1, 1],
    );
    const [point] = labelSurfacePoints(geometry, [new Vector3(0, 2, 0)], [new Vector3()]);
    expect(point?.x).toBeCloseTo(0.1);
    expect(point?.y).toBeCloseTo(0);
    expect(point?.z).toBeCloseTo(0);
  });

  it("stops at the first raised surface even when triangles are indexed in reverse order", () => {
    const geometry = terrain(
      [-1, 0, -1, 1, 0, -1, 0, 0, 1, -1, 1, -1, 1, 1, -1, 0, 1, 1],
      [0, 0, 0, 0, 0, 0],
    );
    geometry.setIndex([5, 4, 3, 2, 1, 0]);
    const [point] = labelSurfacePoints(geometry, [new Vector3(0, 2, 0)], [new Vector3()]);
    expect(point?.y).toBeCloseTo(1);
  });
});
