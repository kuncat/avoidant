attribute float aCellIndex;

varying float vCellIndex;

void main() {
  vCellIndex = aCellIndex;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
