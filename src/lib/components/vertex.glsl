attribute float aCellIndex;

varying vec3 vWorldPosition;
varying float vHeight;
varying float vCellIndex;

void main() {
  vec4 worldPosition = modelMatrix * vec4(position, 1.0);
  vWorldPosition = worldPosition.xyz;
  vHeight = position.y;
  vCellIndex = aCellIndex;

  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
