uniform vec3 uColor;
uniform sampler2D uCellMeta;
uniform float uCellMetaSize;

varying float vCellIndex;

void main() {
  // Ribbons currently render uniformly regardless of per-cell metadata. The
  // `uCellMeta` uniforms are still bound by the host material so the shader
  // can re-introduce per-cell logic without churning the JS side.
  gl_FragColor = vec4(uColor, 1.0);
}
