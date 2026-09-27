import { describe, expect, it } from "vitest";
import { BufferAttribute, BufferGeometry, Vector3 } from "three";
import { cellLabelPlacements } from "./label-connectors";

function terrain(vertices: number[], cells: number[]) {
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new BufferAttribute(new Float32Array(vertices), 3));
  geometry.setAttribute("aCellIndex", new BufferAttribute(new Float32Array(cells), 1));
  return geometry;
}

describe("label surface connectors", () => {
  it("uses the area-weighted surface centroid, independent of unequal triangle sizes", () => {
    const geometry = terrain(
      [0, 0, 0, 4, 0, 0, 0, 0, 2, 4, 0, 0, 5, 0, 0, 4, 0, 2],
      [0, 0, 0, 0, 0, 0],
    );
    const [placement] = cellLabelPlacements(geometry, [new Vector3(0, 1, 0)], 0.4);
    expect(placement!.surface.x).toBeCloseTo((4 * (4 / 3) + 13 / 3) / 5);
    expect(placement!.surface.z).toBeCloseTo(2 / 3);
    expect(placement!.anchor.clone().sub(placement!.surface).toArray()).toEqual([0, 0.4, 0]);
  });

  it("keeps the connector normal to the face rather than to sloping terrain", () => {
    const geometry = terrain([0, 0, 0, 3, 3, 0, 0, 0, 3], [0, 0, 0]);
    const [placement] = cellLabelPlacements(geometry, [new Vector3(0, 1, 0)], 0.5);
    expect(placement!.surface.distanceTo(new Vector3(1, 1, 1))).toBeLessThan(1e-10);
    expect(placement!.anchor.distanceTo(new Vector3(1, 3.5, 1))).toBeLessThan(1e-10);
  });

  it("moves the label with a gap fallback so the connector remains normal", () => {
    const geometry = terrain(
      [-3, 0, -1, -1, 0, -1, -2, 0, 1, 1, 0, -1, 3, 0, -1, 2, 0, 1],
      [0, 0, 0, 0, 0, 0],
    );
    geometry.setIndex([5, 4, 3, 2, 1, 0]);
    const [placement] = cellLabelPlacements(geometry, [new Vector3(0, 1, 0)], 1);
    expect(Math.abs(placement!.surface.x)).toBeGreaterThan(1);
    expect(placement!.anchor.clone().sub(placement!.surface).toArray()).toEqual([0, 1, 0]);
  });

  it("supports a rotated polyhedron face and does not attach to another cell", () => {
    const geometry = terrain(
      [2, 0, 0, 2, 3, 0, 2, 0, 3, 9, 0, 0, 9, 3, 0, 9, 0, 3],
      [0, 0, 0, 1, 1, 1],
    );
    const [placement] = cellLabelPlacements(geometry, [new Vector3(1, 0, 0)], 0.5);
    expect(placement!.surface.toArray()).toEqual([2, 1, 1]);
    expect(placement!.anchor.toArray()).toEqual([2.5, 1, 1]);
  });
});
