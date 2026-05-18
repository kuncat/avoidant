uniform vec3 uColor;
uniform sampler2D uCellMeta;
uniform float uCellMetaSize;

varying float vCellIndex;

void main() {
  vec2 uv = vec2((vCellIndex + 0.5) / uCellMetaSize, 0.5);
  vec4 meta = texture2D(uCellMeta, uv);
  if (meta[CELL_META_VOID] > 0.5) discard;
  gl_FragColor = vec4(uColor, 1.0);
}
