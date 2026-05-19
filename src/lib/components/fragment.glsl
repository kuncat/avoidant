varying vec3 vWorldPosition;
varying float vHeight;
varying float vCellIndex;
varying float vFallProgress;

uniform float elevationMin;
uniform float elevationMax;
uniform sampler2D uCellMeta;
uniform float uCellMetaSize;
uniform int pulseCount;
uniform float pulseTimers[MAX_PULSES];
uniform vec3 pulsePositions[MAX_PULSES];
uniform float pulseOriginCells[MAX_PULSES];
uniform float pulseIsRemote[MAX_PULSES];
uniform vec3 uLightDir;
uniform float uAmbient;
uniform float uDiffuse;

float remapClamped(float value, float inMin, float inMax, float outMin, float outMax) {
  float t = clamp((value - inMin) / (inMax - inMin), 0.0, 1.0);
  return mix(outMin, outMax, t);
}

void main() {
  vec2 metaUv = vec2((vCellIndex + 0.5) / uCellMetaSize, 0.5);
  vec4 meta = texture2D(uCellMeta, metaUv);
  float isVoid = meta[CELL_META_VOID];
  float isExplored = meta[CELL_META_EXPLORED];
  // Once an explored void cell has fully fallen, drop every fragment.
  if (isVoid > 0.5 && isExplored > 0.5 && vFallProgress >= 0.999) discard;

  float elevation = remapClamped(vHeight, elevationMin, elevationMax, 0.0, 1.0);

  float totalRing = 0.0;
  float remoteRing = 0.0;
  float totalSweep = 0.0;
  float hasSweep = 0.0;

  for (int i = 0; i < MAX_PULSES; i++) {
    if (i >= pulseCount) break;

    float pulseProgress = clamp(pulseTimers[i], 0.0, 1.0);
    float pulseActive = step(0.0001, pulseProgress) * (1.0 - step(0.9999, pulseProgress));
    float ringRadius = pulseProgress * 20.0;
    float ringWidth = 1.2;
    float distToOrigin = distance(vWorldPosition.xz, pulsePositions[i].xz);
    float ringDistance = abs(distToOrigin - ringRadius);
    float ring = (1.0 - smoothstep(ringWidth, ringWidth + 0.8, ringDistance)) * pulseActive;
    float pulseFade = 1.0 - pulseProgress;

    float ringContribution = ring * pulseFade;
    totalRing = max(totalRing, ringContribution);
    float isRemote = step(0.5, pulseIsRemote[i]);
    remoteRing = max(remoteRing, ringContribution * isRemote);

    // Sweep coloring only for pulses that originated from this cell
    float isPulseOriginCell = step(abs(pulseOriginCells[i] - vCellIndex), 0.5);
    float pulseSweep = step(distToOrigin / 20.0, pulseProgress);
    totalSweep = max(totalSweep, pulseSweep * isPulseOriginCell);
    hasSweep = max(hasSweep, isPulseOriginCell);
  }

  float colorFactor = max(isExplored, mix(isExplored, totalSweep, hasSweep));

  vec3 unexploredLow = vec3(0.2588, 0.2588, 0.2784);
  vec3 unexploredHigh = vec3(0.4431, 0.451, 0.4706);
  vec3 exploredLow = vec3(0.6588, 0.7098, 0.7922);
  vec3 exploredHigh = vec3(0.6784, 0.7137, 0.7451);

  vec3 lowColor = mix(unexploredLow, exploredLow, colorFactor);
  vec3 highColor = mix(unexploredHigh, exploredHigh, colorFactor);
  vec3 terrainColor = mix(lowColor, highColor, elevation);

  // Flat-shaded hillshading: derive per-face normal from screen-space derivatives
  // of world position so each triangle gets its own light contribution.
  vec3 faceNormal = normalize(cross(dFdx(vWorldPosition), dFdy(vWorldPosition)));
  if (faceNormal.y < 0.0) faceNormal = -faceNormal;
  float ndotl = max(dot(faceNormal, normalize(uLightDir)), 0.0);
  float shade = clamp(uAmbient + uDiffuse * ndotl, 0.0, 1.5);
  terrainColor *= shade;

  vec3 localPulseTint = vec3(0.55, 0.95, 1.0);
  vec3 remotePulseTint = vec3(1.0, 0.42, 0.38);
  float remoteFactor =
    step(0.0001, totalRing) * clamp(remoteRing / (totalRing + 0.00001), 0.0, 1.0);
  vec3 pulseTint = mix(localPulseTint, remotePulseTint, remoteFactor);
  vec3 finalColor = mix(terrainColor, pulseTint, totalRing * 0.85);

  float alpha = isExplored > 0.5 ? 1.0 : 0.25;
  // Fade alpha while an explored void cell is falling.
  if (isVoid > 0.5 && isExplored > 0.5) {
    alpha = clamp(1.0 - vFallProgress, 0.0, 1.0);
  }

  gl_FragColor = vec4(clamp(finalColor, 0.0, 1.0), alpha);
}
