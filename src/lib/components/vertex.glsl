attribute float aCellIndex;
attribute vec3 aCellNormal;
attribute float aHeight;
attribute float aEdgeDistance;
attribute vec3 aNormal;

uniform sampler2D uCellMeta;
uniform float uCellMetaSize;

varying vec3 vWorldPosition;
varying float vHeight;
varying float vEdgeDistance;
varying float vRelief;
varying float vCellIndex;
varying float vFallProgress;

void main() {
  vec2 metaUv = vec2((aCellIndex + 0.5) / uCellMetaSize, 0.5);
  vec4 meta = texture2D(uCellMeta, metaUv);
  float fallProgress = meta[CELL_META_FALL_PROGRESS];
  vFallProgress = fallProgress;

  // Displace falling void cells along the cell's outward surface normal so they sink "into" the polyhedron/spheroid rather than along world -Y.
  vec3 displaced = position - aCellNormal * (fallProgress * VOID_FALL_DISTANCE);

  vec4 worldPosition = modelMatrix * vec4(displaced, 1.0);
  vWorldPosition = worldPosition.xyz;
  // Raw scalar elevation displacement (along the cell's outward normal) for the fragment shader's color ramp. Independent of fall animation so falling cells don't shift colors as they drop.
  vHeight = aHeight;
  vEdgeDistance = aEdgeDistance;
  // Local slope gives hills depth without favoring a world-space direction.
  vRelief = clamp(dot(normalize(aNormal), normalize(aCellNormal)), 0.0, 1.0);
  vCellIndex = aCellIndex;

  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
