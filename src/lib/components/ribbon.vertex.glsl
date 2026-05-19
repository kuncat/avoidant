attribute float aCellIndex;

uniform sampler2D uCellMeta;
uniform float uCellMetaSize;

varying float vCellIndex;
varying float vFallProgress;

void main() {
  vCellIndex = aCellIndex;

  // Match the terrain's fall offset so ribbons drop along with their cell.
  vec2 metaUv = vec2((aCellIndex + 0.5) / uCellMetaSize, 0.5);
  vec4 meta = texture2D(uCellMeta, metaUv);
  float fallProgress = meta[CELL_META_FALL_PROGRESS];
  vFallProgress = fallProgress;

  vec3 displaced = position;
  displaced.y -= fallProgress * VOID_FALL_DISTANCE;

  gl_Position = projectionMatrix * modelViewMatrix * vec4(displaced, 1.0);
}
