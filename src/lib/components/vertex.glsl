attribute float aCellIndex;
attribute vec3 aNormal;

uniform sampler2D uCellMeta;
uniform float uCellMetaSize;

varying vec3 vWorldPosition;
varying vec3 vNormal;
varying float vHeight;
varying float vCellIndex;
varying float vFallProgress;

void main() {
  vec2 metaUv = vec2((aCellIndex + 0.5) / uCellMetaSize, 0.5);
  vec4 meta = texture2D(uCellMeta, metaUv);
  float fallProgress = meta[CELL_META_FALL_PROGRESS];
  vFallProgress = fallProgress;

  vec3 displaced = position;
  displaced.y -= fallProgress * VOID_FALL_DISTANCE;

  vec4 worldPosition = modelMatrix * vec4(displaced, 1.0);
  vWorldPosition = worldPosition.xyz;
  // Smooth per-vertex normal. Interpolated across the triangle for Phong-style smooth shading instead of faceted flat shading.
  vNormal = normalize(mat3(modelMatrix) * aNormal);
  // Keep the elevation-based color ramp anchored to the cell's original height so falling cells don't shift colors as they drop.
  vHeight = position.y;
  vCellIndex = aCellIndex;

  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
