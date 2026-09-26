import { BufferGeometry, Ray, Triangle, Vector3 } from "three";

/** Find actual terrain endpoints, keeping edge labels attached to their own cell. */
export function labelSurfacePoints(
  geometry: BufferGeometry,
  anchors: Vector3[],
  centers: Vector3[],
): (Vector3 | undefined)[] {
  const positions = geometry.getAttribute("position");
  const cellIndices = geometry.getAttribute("aCellIndex");
  const triangles = new Map<number, Triangle[]>();
  const index = geometry.index;
  for (let offset = 0; offset < (index?.count ?? positions.count); offset += 3) {
    const a = index ? index.getX(offset) : offset;
    const b = index ? index.getX(offset + 1) : offset + 1;
    const c = index ? index.getX(offset + 2) : offset + 2;
    const cell = cellIndices.getX(a);
    const triangle = new Triangle(
      new Vector3().fromBufferAttribute(positions, a),
      new Vector3().fromBufferAttribute(positions, b),
      new Vector3().fromBufferAttribute(positions, c),
    );
    const group = triangles.get(cell);
    if (group) group.push(triangle);
    else triangles.set(cell, [triangle]);
  }

  return anchors.map((anchor, cell) => {
    const surface = triangles.get(cell);
    const center = centers[cell];
    if (!surface || !center) return undefined;
    const ray = new Ray(anchor, center.clone().sub(anchor).normalize());
    const hit = new Vector3();
    function firstHit() {
      let nearest: Vector3 | undefined;
      let distance = Infinity;
      for (const triangle of surface!) {
        if (!ray.intersectTriangle(triangle.a, triangle.b, triangle.c, false, hit)) continue;
        const candidateDistance = anchor.distanceToSquared(hit);
        if (candidateDistance < distance) {
          distance = candidateDistance;
          nearest = hit.clone();
        }
      }
      return nearest;
    }
    // A polyhedron edge's base centroid can lie above the inset triangles.
    // Do not stop the search at that centroid.
    const direct = firstHit();
    if (direct) return direct;

    // If the ray falls in an inset gap, attach to the nearest point on this
    // cell's surface instead of dropping the connector or hitting a neighbor.
    let closest: Vector3 | undefined;
    let distance = Infinity;
    for (const triangle of surface) {
      triangle.closestPointToPoint(center, hit);
      const candidateDistance = center.distanceToSquared(hit);
      if (candidateDistance < distance) {
        distance = candidateDistance;
        closest = hit.clone();
      }
    }
    if (!closest) return undefined;
    ray.direction.copy(closest).sub(anchor).normalize();
    return firstHit() ?? closest;
  });
}
