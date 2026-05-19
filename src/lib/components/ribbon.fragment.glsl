uniform vec3 uColor;
uniform sampler2D uCellMeta;
uniform float uCellMetaSize;

varying float vCellIndex;
varying float vFallProgress;

void main() {
  // Ribbons fall with their cell (see ribbon.vertex.glsl) and fade with it here.
  if (vFallProgress >= 0.999) discard;
  gl_FragColor = vec4(uColor, clamp(1.0 - vFallProgress, 0.0, 1.0));
}
