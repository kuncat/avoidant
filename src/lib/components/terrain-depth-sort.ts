import { BufferAttribute, BufferGeometry, Matrix4 } from "three";

/** Sort individual triangles in a merged surface, including falling cells.
 * Three.js sorts transparent objects, but cannot sort triangles inside a mesh.
 * Only the index buffer changes, preserving all per-vertex cell IDs for picking.
 */
export class TerrainDepthSort {
  private readonly centers: Float64Array;
  private readonly normals: Float64Array;
  private readonly cells: Uint32Array;
  private readonly depths: Float64Array;
  private readonly order: Uint32Array;
  private readonly index: BufferAttribute;
  private readonly previousView = new Matrix4();
  private previousRevision = -1;

  constructor(geometry: BufferGeometry) {
    const positions = geometry.getAttribute("position");
    const normals = geometry.getAttribute("aCellNormal");
    const cells = geometry.getAttribute("aCellIndex");
    const count = positions.count / 3;
    this.centers = new Float64Array(count * 3);
    this.normals = new Float64Array(count * 3);
    this.cells = new Uint32Array(count);
    this.depths = new Float64Array(count);
    this.order = Uint32Array.from({ length: count }, (_, i) => i);
    this.index = new BufferAttribute(
      Uint32Array.from({ length: positions.count }, (_, i) => i),
      1,
    );
    geometry.setIndex(this.index);
    for (let t = 0; t < count; t++) {
      this.cells[t] = cells.getX(t * 3);
      for (let axis = 0; axis < 3; axis++) {
        this.centers[t * 3 + axis] =
          (positions.getComponent(t * 3, axis) +
            positions.getComponent(t * 3 + 1, axis) +
            positions.getComponent(t * 3 + 2, axis)) /
          3;
        this.normals[t * 3 + axis] = normals.getComponent(t * 3, axis);
      }
    }
  }

  update(view: Matrix4, revision: number, fallDistance: (cell: number) => number) {
    if (revision === this.previousRevision && view.equals(this.previousView)) return;
    this.previousView.copy(view);
    this.previousRevision = revision;
    const m = view.elements;
    for (let t = 0; t < this.order.length; t++) {
      const offset = t * 3;
      const fall = fallDistance(this.cells[t]);
      const x = this.centers[offset] - this.normals[offset] * fall;
      const y = this.centers[offset + 1] - this.normals[offset + 1] * fall;
      const z = this.centers[offset + 2] - this.normals[offset + 2] * fall;
      // The camera looks along -Z: smaller view-space Z is farther away.
      this.depths[t] = m[2] * x + m[6] * y + m[10] * z + m[14];
    }
    this.order.sort((a, b) => this.depths[a] - this.depths[b] || a - b);
    for (let i = 0; i < this.order.length; i++) {
      const vertex = this.order[i] * 3;
      this.index.setX(i * 3, vertex);
      this.index.setX(i * 3 + 1, vertex + 1);
      this.index.setX(i * 3 + 2, vertex + 2);
    }
    this.index.needsUpdate = true;
  }
}
