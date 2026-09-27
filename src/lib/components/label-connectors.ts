import { BufferGeometry, Ray, Triangle, Vector3 } from "three";

/** Center each label on its cell's exposed mesh, aligned with the base face normal. */
export function cellLabelPlacements(
  geometry: BufferGeometry,
  normals: Vector3[],
  clearance: number,
): ({ surface: Vector3; anchor: Vector3 } | undefined)[] {
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

  return normals.map((faceNormal, cell) => {
    const mesh = triangles.get(cell);
    if (!mesh || !faceNormal.lengthSq()) return undefined;
    const normal = faceNormal.clone().normalize();
    const centroid = new Vector3();
    const midpoint = new Vector3();
    let area = 0;
    let top = -Infinity;
    for (const triangle of mesh) {
      const weight = triangle.getArea();
      centroid.addScaledVector(triangle.getMidpoint(midpoint), weight);
      area += weight;
      top = Math.max(top, triangle.a.dot(normal), triangle.b.dot(normal), triangle.c.dot(normal));
    }
    if (!area) return undefined;
    centroid.divideScalar(area);

    // A curved mesh's area centroid may lie inside the terrain. Project outward
    // along the face normal, then intersect back onto the exposed surface.
    const gap = Math.max(clearance, 1e-6);
    const anchor = centroid.clone().addScaledVector(normal, top + gap - centroid.dot(normal));
    const ray = new Ray(anchor, normal.clone().negate());
    const hit = new Vector3();
    let surface: Vector3 | undefined;
    let nearest = Infinity;
    for (const triangle of mesh) {
      if (!ray.intersectTriangle(triangle.a, triangle.b, triangle.c, false, hit)) continue;
      const distance = anchor.distanceToSquared(hit);
      if (distance < nearest) {
        nearest = distance;
        surface = hit.clone();
      }
    }
    if (!surface) {
      // For a non-convex cell or inset gap, use its nearest surface point and
      // move the label with it so the connector never tilts away from the normal.
      nearest = Infinity;
      for (const triangle of mesh) {
        triangle.closestPointToPoint(centroid, hit);
        const distance = centroid.distanceToSquared(hit);
        if (distance < nearest) {
          nearest = distance;
          surface = hit.clone();
        }
      }
    }
    if (!surface) return undefined;
    anchor.copy(surface).addScaledVector(normal, top + gap - surface.dot(normal));
    return { surface, anchor };
  });
}
