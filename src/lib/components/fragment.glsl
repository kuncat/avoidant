varying vec3 vWorldPosition;
varying float vHeight;
varying float vEdgeDistance;
varying float vRelief;
varying float vCellIndex;
varying float vFallProgress;

uniform bool uTransparentPass;
uniform float elevationMin;
uniform float elevationMax;
uniform sampler2D uCellMeta;
uniform float uCellMetaSize;
uniform int pulseCount;
uniform float pulseTimers[MAX_PULSES];
uniform vec3 pulsePositions[MAX_PULSES];
uniform float pulseOriginCells[MAX_PULSES];
uniform float pulseIsRemote[MAX_PULSES];
uniform float pulseMaxRadii[MAX_PULSES];
uniform float uHighlightedCell;
uniform float uTime;

float remapClamped(float value, float inMin, float inMax, float outMin, float outMax) {
  float t = clamp((value - inMin) / max(inMax - inMin, 0.0001), 0.0, 1.0);
  return mix(outMin, outMax, t);
}

void main() {
  vec2 metaUv = vec2((vCellIndex + 0.5) / uCellMetaSize, 0.5);
  vec4 meta = texture2D(uCellMeta, metaUv);
  float isVoid = meta[CELL_META_VOID];
  float isExplored = meta[CELL_META_EXPLORED];
  float isRevealing = meta[CELL_META_REVEALING];
  // Once an explored void cell has fully fallen, drop every fragment.
  if (isVoid > 0.5 && isExplored > 0.5 && vFallProgress >= 0.999) discard;

  float elevation = remapClamped(vHeight, elevationMin, elevationMax, 0.0, 1.0);

  float totalRing = 0.0;
  float remoteRing = 0.0;
  float totalSweep = 0.0;

  for (int i = 0; i < MAX_PULSES; i++) {
    if (i >= pulseCount) break;

    float pulseProgress = clamp(pulseTimers[i], 0.0, 1.0);
    float pulseActive = step(0.0001, pulseProgress) * (1.0 - step(0.9999, pulseProgress));
    float maxRadius = pulseMaxRadii[i];
    float ringRadius = pulseProgress * maxRadius;
    float ringWidth = 1.2;
    // Use full 3D distance — pulses radiate outward over the curved surface
    // of the polyhedron/spheroid, not within an XZ plane.
    float distToOrigin = distance(vWorldPosition, pulsePositions[i]);
    float ringDistance = abs(distToOrigin - ringRadius);
    float ring = (1.0 - smoothstep(ringWidth, ringWidth + 0.8, ringDistance)) * pulseActive;
    float pulseFade = 1.0 - pulseProgress;

    float ringContribution = ring * pulseFade;
    totalRing = max(totalRing, ringContribution);
    float isRemote = step(0.5, pulseIsRemote[i]);
    remoteRing = max(remoteRing, ringContribution * isRemote);

    float normalizedDist = distToOrigin / maxRadius;
    float pulseSweep =
      1.0 - smoothstep(pulseProgress - SWEEP_BAND, pulseProgress + SWEEP_BAND, normalizedDist);
    totalSweep = max(totalSweep, pulseSweep);
  }

  // Revealing cells (the clicked cell and every chord-revealed neighbor) show the radial sweep gradient instead of the unexplored color until the pulse finishes and `isExplored` flips to true.
  float colorFactor = mix(isExplored, totalSweep, isRevealing);

  vec3 unexploredLow = vec3(0.2588, 0.2588, 0.2784);
  vec3 unexploredHigh = vec3(0.4431, 0.451, 0.4706);
  vec3 exploredLow = vec3(0.43, 0.5, 0.59);
  vec3 exploredHigh = vec3(0.75, 0.79, 0.84);

  vec3 lowColor = mix(unexploredLow, exploredLow, colorFactor);
  vec3 highColor = mix(unexploredHigh, exploredHigh, colorFactor);
  vec3 terrainColor = mix(lowColor, highColor, elevation);

  // Uniform illumination keeps every face readable regardless of its
  // orientation, with a subdued level that avoids near-white revealed cells.
  terrainColor *= 0.9 * mix(0.82, 1.0, vRelief);

  vec3 localPulseTint = vec3(0.55, 0.95, 1.0);
  vec3 remotePulseTint = vec3(1.0, 0.42, 0.38);
  float remoteFactor =
    step(0.0001, totalRing) * clamp(remoteRing / (totalRing + 0.00001), 0.0, 1.0);
  vec3 pulseTint = mix(localPulseTint, remotePulseTint, remoteFactor);
  vec3 finalColor = mix(terrainColor, pulseTint, totalRing * 0.85);

  // Pulsing glow for a tutorial-highlighted cell. `uHighlightedCell` is -1 when no cell is highlighted.
  if (uHighlightedCell >= 0.0 && abs(vCellIndex - uHighlightedCell) < 0.5) {
    float glowPulse = 0.5 + 0.5 * sin(uTime * 0.005);
    vec3 glowColor = vec3(0.4745, 0.8667, 0.6706);
    finalColor = mix(finalColor, glowColor, 0.35 + 0.45 * glowPulse);
  }

  // Draw an antialiased perimeter on the surface itself. A pixel-based
  // minimum keeps borders visible when zooming out or viewing at an angle.
  float edgePixel = max(fwidth(vEdgeDistance), 0.0001);
  float borderWidth = max(0.04, 1.1 * edgePixel);
  float border = 1.0 - smoothstep(borderWidth, borderWidth + edgePixel, vEdgeDistance);
  finalColor = mix(finalColor, vec3(0.1, 0.13, 0.18), border * 0.9);

  // Unexplored cells retain slight transparency; revealed solid cells are opaque.
  float alpha = isExplored > 0.5 ? 1.0 : 0.95;
  // Fade alpha while an explored void cell is falling.
  if (isVoid > 0.5 && isExplored > 0.5) {
    alpha = clamp(1.0 - vFallProgress, 0.0, 1.0);
  }

  // Keep opaque depth separate from blended layers. The two passes must be
  // mutually exclusive so no cell is drawn twice.
  if (uTransparentPass ? alpha >= 1.0 : alpha < 1.0) discard;

  gl_FragColor = vec4(clamp(finalColor, 0.0, 1.0), alpha);
}
