varying vec3 vWorldPosition;
varying float vHeight;

void main() {
  vec4 worldPosition = modelMatrix * vec4(position, 1.0);
  vWorldPosition = worldPosition.xyz;
  vHeight = position.y;

  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
