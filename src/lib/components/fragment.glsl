varying vec3 vWorldPosition;
varying float vHeight;

uniform float isExplored;
uniform float elevationMin;
uniform float elevationMax;
uniform float cellIndex;
uniform int pulseCount;
uniform float pulseTimers[MAX_PULSES];
uniform vec3 pulsePositions[MAX_PULSES];
uniform float pulseOriginCells[MAX_PULSES];

float remapClamped(float value, float inMin, float inMax, float outMin, float outMax) {
  float t = clamp((value - inMin) / (inMax - inMin), 0.0, 1.0);
  return mix(outMin, outMax, t);
}

void main() {
  float elevation = remapClamped(vHeight, elevationMin, elevationMax, 0.0, 1.0);

  float totalRing = 0.0;
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

    totalRing = max(totalRing, ring * pulseFade);

    // Sweep coloring only for pulses that originated from this cell
    float isPulseOriginCell = step(abs(pulseOriginCells[i] - cellIndex), 0.5);
    float pulseSweep = step(distToOrigin / 20.0, pulseProgress);
    totalSweep = max(totalSweep, pulseSweep * isPulseOriginCell);
    hasSweep = max(hasSweep, isPulseOriginCell);
  }

  float colorFactor = max(isExplored, mix(isExplored, totalSweep, hasSweep));

  vec3 unexploredLow = vec3(0.22, 0.23, 0.26);
  vec3 unexploredHigh = vec3(0.28, 0.29, 0.32);
  vec3 exploredLow = vec3(0.6588, 0.7098, 0.7922);
  vec3 exploredHigh = vec3(0.6784, 0.7137, 0.7451);

  vec3 lowColor = mix(unexploredLow, exploredLow, 1.0 - colorFactor);
  vec3 highColor = mix(unexploredHigh, exploredHigh, 1.0 - colorFactor);
  vec3 terrainColor = mix(lowColor, highColor, elevation);

  vec3 pulseTint = vec3(0.55, 0.95, 1.0);
  vec3 finalColor = mix(terrainColor, pulseTint, totalRing * 0.85);

  gl_FragColor = vec4(clamp(finalColor, 0.0, 1.0), 1.0);
}
