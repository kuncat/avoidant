<script module lang="ts">
  import _vertexShader from "./vertex.glsl?raw";
  import _fragmentShader from "./fragment.glsl?raw";
  import { PULSE_SWEEP_BAND } from "$lib/generated/shared-constants";
  import openSans from "$lib/assets/OpenSans-VariableFont_wdth,wght.ttf?url";
  import { getLocale } from "$lib/paraglide/runtime";

  export const MAX_PULSES = 16;
  export const VOID_FALL_DURATION_MS = 900;
  export const VOID_FALL_DISTANCE = 8;

  /**
   * Byte offsets within each RGBA8 texel of the per-cell metadata texture.
   *
   * The GLSL side sees these as `#define CELL_META_*` constants (see {@link cellMetaDefines}).
   */
  export const CellMetaChannel = {
    Explored: 0,
    Void: 1,
    // 0..255 fall-out progress for explored void cells; 0 = on the map, 255 = fully gone.
    FallProgress: 2,
    // 255 while a chord auto-reveal pulse is sweeping the cell. The shader uses this to render the pulse-sweep gradient (same effect as the clicked cell).
    Revealing: 3,
  } as const;

  /**
   * GLSL `#define` block exposing {@link CellMetaChannel} offsets to shaders.
   */
  export const cellMetaDefines = Object.entries(CellMetaChannel)
    .map(([name, offset]) => {
      const screamingSnakeName = name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toUpperCase();
      return `#define CELL_META_${screamingSnakeName} ${offset}\n`;
    })
    .join("");

  const fallDefines = `#define VOID_FALL_DISTANCE ${VOID_FALL_DISTANCE.toFixed(1)}\n`;

  export const vertexShader = cellMetaDefines + fallDefines + _vertexShader;
  export const fragmentShader =
    `#define MAX_PULSES ${MAX_PULSES}\n` +
    `#define SWEEP_BAND ${PULSE_SWEEP_BAND.toFixed(6)}\n` +
    cellMetaDefines +
    _fragmentShader;
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { TerrainDepthSort } from "./terrain-depth-sort";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { Pulse } from "$lib/wasm/avoidant_wasm";
  import type { GameState } from "$lib/wasm/avoidant_wasm";
  import { T, useThrelte } from "@threlte/core";
  import { interactivity, Text, Billboard } from "@threlte/extras";
  import {
    BufferAttribute,
    BufferGeometry,
    DataTexture,
    DoubleSide,
    Mesh,
    Matrix4,
    NearestFilter,
    RGBAFormat,
    ShaderMaterial,
    UnsignedByteType,
    Vector3,
  } from "three";

  interactivity();

  const { invalidate } = useThrelte();

  interface Props {
    gameState: GameState;
    terrain?:
      | { positions: number[]; normals: number[]; cellIndices: number[]; heights: number[] }
      | undefined;
    flat?: boolean;
    surfaceArea?: number;
    boundsRadius?: number;
    interactive?: boolean;
    highlightedCellIndex?: number | undefined;
    onCellClicked?: (cellIndex: number) => void;
  }

  let {
    gameState = $bindable(),
    terrain = undefined,
    flat = false,
    surfaceArea = 0,
    boundsRadius = 1,
    interactive = true,
    highlightedCellIndex = undefined,
    onCellClicked = undefined,
  }: Props = $props();
  let cells = $derived(gameState?.cells);
  let cellMetadata = $derived(gameState?.cellMetadata);
  let pulses = $derived(gameState?.uiState?.pulses);
  let nowMs = $state(0);

  onMount(() => {
    let rafId = 0;
    const tick = () => {
      const now = performance.now();
      const hasActivePulses = $pulses.some((p) => now - p.createdAtMs < Math.max(1, p.durationMs));
      const hasFallingCells = fallStart.size > 0;
      const hasHighlight = highlightedCellIndex !== undefined;
      if (hasActivePulses || hasFallingCells || hasHighlight) {
        nowMs = now;

        if (hasFallingCells) {
          const meta = cellMeta;
          for (const [cellIndex, startMs] of fallStart) {
            const progress = Math.min(1, (now - startMs) / VOID_FALL_DURATION_MS);
            meta.setFallProgress(cellIndex, progress);
            if (progress >= 1) {
              fallStart.delete(cellIndex);
              fellCells.add(cellIndex);
            }
          }
          meta.flush();
        }

        // Invalidate when remote state changes to draw a new frame.
        invalidate();
      }
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  });

  /**
   * Build a merged triangle mesh for all cell interiors from a Rust-side subdivided terrain payload.
   *
   * Each emitted vertex carries an `aCellIndex` attribute so the fragment shader can look up per-cell metadata in {@link CellMetaTexture}. It also carries `aCellNormal` (the cell's outward unit normal) so the vertex shader can displace falling void cells along the local outward direction — which is the face normal for polyhedra and the surface normal for spheroids — instead of along world Y.
   *
   * When `insetDistance > 0`, every vertex is pulled toward its cell's 3D centroid in the *tangent plane* perpendicular to the cell normal, producing uniform-width gaps between adjacent cells regardless of the cell's orientation on the surface. The component of the offset along the normal is left untouched so terrain elevation isn't squashed.
   */
  function buildTerrainLayer(
    payload:
      | { positions: number[]; normals: number[]; cellIndices: number[]; heights: number[] }
      | undefined,
    cellNormalsByIndex: Float32Array | undefined,
    cellVertices: number[][][],
    flat: boolean,
    insetDistance = 0,
  ): BufferGeometry {
    const geometry = new BufferGeometry();
    if (!payload || payload.positions.length === 0) {
      geometry.setAttribute("position", new BufferAttribute(new Float32Array(0), 3));
      geometry.setAttribute("aNormal", new BufferAttribute(new Float32Array(0), 3));
      geometry.setAttribute("aCellNormal", new BufferAttribute(new Float32Array(0), 3));
      geometry.setAttribute("aCellIndex", new BufferAttribute(new Float32Array(0), 1));
      geometry.setAttribute("aHeight", new BufferAttribute(new Float32Array(0), 1));
      geometry.setAttribute("aEdgeDistance", new BufferAttribute(new Float32Array(0), 1));
      return geometry;
    }

    const positions = new Float32Array(payload.positions);
    const cellIndices = payload.cellIndices;
    const vertexCount = cellIndices.length;

    // Measure against the original cell perimeter, not the terrain triangle
    // edges. Undo radial elevation before measuring so outlines follow hills
    // without outlining the interior tessellation.
    const edgeDistances = new Float32Array(vertexCount);
    for (let i = 0; i < vertexCount; i++) {
      const x = positions[i * 3];
      const y = positions[i * 3 + 1];
      const z = positions[i * 3 + 2];
      const radius = Math.hypot(x, y, z);
      const factor = radius > 0 ? 1 - payload.heights[i] / radius : 1;
      const p = flat ? [x, y - payload.heights[i], z] : [x * factor, y * factor, z * factor];
      const polygon = cellVertices[cellIndices[i]];
      let nearest = Infinity;
      for (let j = 0; j < polygon.length; j++) {
        const a = polygon[j];
        const b = polygon[(j + 1) % polygon.length];
        const dx = b[0] - a[0];
        const dy = b[1] - a[1];
        const dz = b[2] - a[2];
        const lengthSquared = dx * dx + dy * dy + dz * dz;
        const t =
          lengthSquared > 0
            ? Math.max(
                0,
                Math.min(
                  1,
                  ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy + (p[2] - a[2]) * dz) / lengthSquared,
                ),
              )
            : 0;
        nearest = Math.min(
          nearest,
          Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy, p[2] - a[2] - t * dz),
        );
      }
      edgeDistances[i] = nearest;
    }
    geometry.setAttribute("aEdgeDistance", new BufferAttribute(edgeDistances, 1));

    // Compute per-cell 3D centroid from terrain vertices (after de-displacement
    // we'd be on the un-noised surface, but for inset purposes the displaced
    // centroid works fine — the lateral move is tiny relative to elevation).
    let maxCell = 0;
    for (let i = 0; i < vertexCount; i++) {
      if (cellIndices[i] > maxCell) maxCell = cellIndices[i];
    }
    const cellCount = maxCell + 1;
    const sumX = new Float64Array(cellCount);
    const sumY = new Float64Array(cellCount);
    const sumZ = new Float64Array(cellCount);
    const count = new Uint32Array(cellCount);
    for (let i = 0; i < vertexCount; i++) {
      const c = cellIndices[i];
      sumX[c] += positions[i * 3 + 0];
      sumY[c] += positions[i * 3 + 1];
      sumZ[c] += positions[i * 3 + 2];
      count[c]++;
    }
    const cx = new Float32Array(cellCount);
    const cy = new Float32Array(cellCount);
    const cz = new Float32Array(cellCount);
    for (let c = 0; c < cellCount; c++) {
      if (count[c] > 0) {
        cx[c] = sumX[c] / count[c];
        cy[c] = sumY[c] / count[c];
        cz[c] = sumZ[c] / count[c];
      }
    }

    // Per-vertex cell normal lookup.
    const cellNormalAttr = new Float32Array(vertexCount * 3);
    if (cellNormalsByIndex && cellNormalsByIndex.length >= cellCount * 3) {
      for (let i = 0; i < vertexCount; i++) {
        const c = cellIndices[i];
        cellNormalAttr[i * 3 + 0] = cellNormalsByIndex[c * 3 + 0];
        cellNormalAttr[i * 3 + 1] = cellNormalsByIndex[c * 3 + 1];
        cellNormalAttr[i * 3 + 2] = cellNormalsByIndex[c * 3 + 2];
      }
    } else {
      // Fallback: derive cell normals from the average of per-vertex normals.
      const nSum = new Float32Array(cellCount * 3);
      for (let i = 0; i < vertexCount; i++) {
        const c = cellIndices[i];
        nSum[c * 3 + 0] += payload.normals[i * 3 + 0];
        nSum[c * 3 + 1] += payload.normals[i * 3 + 1];
        nSum[c * 3 + 2] += payload.normals[i * 3 + 2];
      }
      for (let c = 0; c < cellCount; c++) {
        const nx = nSum[c * 3 + 0];
        const ny = nSum[c * 3 + 1];
        const nz = nSum[c * 3 + 2];
        const len = Math.hypot(nx, ny, nz) || 1;
        nSum[c * 3 + 0] = nx / len;
        nSum[c * 3 + 1] = ny / len;
        nSum[c * 3 + 2] = nz / len;
      }
      for (let i = 0; i < vertexCount; i++) {
        const c = cellIndices[i];
        cellNormalAttr[i * 3 + 0] = nSum[c * 3 + 0];
        cellNormalAttr[i * 3 + 1] = nSum[c * 3 + 1];
        cellNormalAttr[i * 3 + 2] = nSum[c * 3 + 2];
      }
    }

    if (insetDistance > 0) {
      for (let i = 0; i < vertexCount; i++) {
        const c = cellIndices[i];
        const dx = positions[i * 3 + 0] - cx[c];
        const dy = positions[i * 3 + 1] - cy[c];
        const dz = positions[i * 3 + 2] - cz[c];
        const nx = cellNormalAttr[i * 3 + 0];
        const ny = cellNormalAttr[i * 3 + 1];
        const nz = cellNormalAttr[i * 3 + 2];
        const along = dx * nx + dy * ny + dz * nz;
        const tx = dx - along * nx;
        const ty = dy - along * ny;
        const tz = dz - along * nz;
        const tlen = Math.hypot(tx, ty, tz);
        if (tlen < 1e-6) continue;
        const move = Math.min(insetDistance, tlen);
        const k = move / tlen;
        positions[i * 3 + 0] -= tx * k;
        positions[i * 3 + 1] -= ty * k;
        positions[i * 3 + 2] -= tz * k;
      }
    }

    geometry.setAttribute("position", new BufferAttribute(positions, 3));
    geometry.setAttribute("aNormal", new BufferAttribute(new Float32Array(payload.normals), 3));
    geometry.setAttribute("aCellNormal", new BufferAttribute(cellNormalAttr, 3));
    geometry.setAttribute(
      "aCellIndex",
      new BufferAttribute(new Float32Array(payload.cellIndices), 1),
    );
    geometry.setAttribute("aHeight", new BufferAttribute(new Float32Array(payload.heights), 1));
    return geometry;
  }

  /**
   * One RGBA8 `DataTexture` of size N×1 holding per-cell metadata.
   *
   * Texel `i` stores the metadata for cell `i`; shaders sample it using `aCellIndex` as the U coordinate, which lets a single byte cover all of a cell's vertices across the terrain and ribbon layers without per-vertex metadata attributes.
   */
  class CellMetaTexture {
    /** Underlying Three.js texture to be bound to shader uniforms. */
    readonly texture: DataTexture;
    /** Backing byte buffer aliased by {@link texture}. Layout: 4 bytes per cell. */
    private readonly data: Uint8Array;
    /** Number of texels/cells. */
    readonly width: number;
    revision = 0;

    fallProgress(cellIndex: number): number {
      return this.data[cellIndex * 4 + CellMetaChannel.FallProgress] / 255;
    }

    /**
     * Allocate a cell metadata texture.
     */
    constructor(cellCount: number) {
      // Enforce a minimum width of 1 so the texture is always in a valid GPU-bindable state.
      this.width = Math.max(1, cellCount);
      this.data = new Uint8Array(this.width * 4);
      this.texture = new DataTexture(this.data, this.width, 1, RGBAFormat, UnsignedByteType);
      this.texture.minFilter = NearestFilter;
      this.texture.magFilter = NearestFilter;
      this.texture.generateMipmaps = false;
      this.texture.needsUpdate = true;
    }

    /**
     * Mark cell `cellIndex` as explored or unexplored.
     *
     * Call {@link flush} once after a batch of updates to schedule the GPU upload.
     */
    setExplored(cellIndex: number, value: boolean): void {
      this.data[cellIndex * 4 + CellMetaChannel.Explored] = value ? 255 : 0;
    }

    /**
     * Mark cell `cellIndex` as void (renders as a hole) or solid.
     *
     * Call {@link flush} once after a batch of updates to schedule the GPU upload.
     */
    setVoid(cellIndex: number, value: boolean): void {
      this.data[cellIndex * 4 + CellMetaChannel.Void] = value ? 255 : 0;
    }

    /**
     * Write the falling progress for an explored void cell.
     *
     * @param progress - Normalized fall progress in `[0, 1]`.
     */
    setFallProgress(cellIndex: number, progress: number): void {
      const byte = Math.max(0, Math.min(255, Math.round(progress * 255)));
      this.data[cellIndex * 4 + CellMetaChannel.FallProgress] = byte;
    }

    /**
     * Mark cell `cellIndex` as currently being revealed by a chord pulse.
     */
    setRevealing(cellIndex: number, value: boolean): void {
      this.data[cellIndex * 4 + CellMetaChannel.Revealing] = value ? 255 : 0;
    }

    /** Mark the texture dirty so Three.js re-uploads it on the next frame. */
    flush(): void {
      this.revision++;
      this.texture.needsUpdate = true;
    }

    /** Release the underlying GPU resources. */
    dispose(): void {
      this.texture.dispose();
    }
  }

  // Each cell's vertices are pulled toward its 3D centroid (in the cell's tangent plane) by a constant distance (`CELL_GAP_HALF_WIDTH`). The total visible gap width is ~2× this value.
  const CELL_GAP_HALF_WIDTH = 0.08;
  const cellRadius = $derived(
    Math.sqrt(Math.max(0, surfaceArea) / (Math.PI * Math.max(1, $cells.length))),
  );
  const cellNormalsByIndex = $derived.by(() => {
    const arr = new Float32Array($cells.length * 3);
    for (let i = 0; i < $cells.length; i++) {
      const n = $cells[i].normal;
      arr[i * 3 + 0] = n[0];
      arr[i * 3 + 1] = n[1];
      arr[i * 3 + 2] = n[2];
    }
    return arr;
  });
  const terrainGeometry = $derived(
    buildTerrainLayer(
      terrain,
      cellNormalsByIndex,
      $cells.map((cell) => cell.vertices),
      flat,
      CELL_GAP_HALF_WIDTH,
    ),
  );
  const cellMeta = $derived(new CellMetaTexture($cells.length));

  let fallStart = new SvelteMap<number, number>();
  let fellCells = new SvelteSet<number>();
  $effect(() => {
    // Reset every time a fresh metadata texture is built.
    void cellMeta;
    fallStart = new SvelteMap<number, number>();
    fellCells = new SvelteSet<number>();
  });

  // Dispose old GPU buffers when geometry / metadata texture is rebuilt.
  $effect(() => {
    const layer = terrainGeometry;
    const meta = cellMeta;
    return () => {
      layer.dispose();
      meta.dispose();
    };
  });

  let terrainMaterial = $derived(
    new ShaderMaterial({
      vertexShader,
      fragmentShader,
      side: DoubleSide,
      transparent: true,
      depthWrite: false,
      // Triangle sorting handles both sides together, including concave hills.
      forceSinglePass: true,
      uniforms: {
        uTransparentPass: { value: true },
        elevationMin: { value: gameState.elevationMin },
        elevationMax: { value: gameState.elevationMax },
        uCellMeta: { value: cellMeta.texture },
        uCellMetaSize: { value: cellMeta.width },
        pulseCount: { value: 0 },
        pulseTimers: { value: new Array(MAX_PULSES).fill(0) },
        pulsePositions: {
          value: new Array(MAX_PULSES).fill(null).map(() => new Vector3()),
        },
        pulseOriginCells: { value: new Array(MAX_PULSES).fill(-1) },
        pulseIsRemote: { value: new Array(MAX_PULSES).fill(0) },
        pulseMaxRadii: { value: new Array(MAX_PULSES).fill(0) },
        uHighlightedCell: { value: -1 },
        uTime: { value: 0 },
      },
    }),
  );

  // Dispose materials when re-derived.
  $effect(() => {
    const material = terrainMaterial;
    return () => {
      material.dispose();
    };
  });

  // Opaque revealed cells populate depth first. The transparent pass blends
  // unexplored/falling cells against that depth without blocking later layers.
  const opaqueMaterial = $derived(
    new ShaderMaterial({
      vertexShader,
      fragmentShader,
      side: DoubleSide,
      transparent: false,
      depthWrite: true,
      uniforms: { ...terrainMaterial.uniforms, uTransparentPass: { value: false } },
    }),
  );
  $effect(() => {
    const material = opaqueMaterial;
    return () => material.dispose();
  });
  const opaqueMesh = $derived(new Mesh(terrainGeometry, opaqueMaterial));
  let terrainMesh = $derived.by(() => {
    const mesh = new Mesh(terrainGeometry, terrainMaterial);
    const sorter = new TerrainDepthSort(terrainGeometry);
    const meta = cellMeta;
    const view = new Matrix4();
    mesh.onBeforeRender = (_renderer, _scene, camera) => {
      view.multiplyMatrices(camera.matrixWorldInverse, mesh.matrixWorld);
      sorter.update(view, meta.revision, (cell) => meta.fallProgress(cell) * VOID_FALL_DISTANCE);
    };
    return mesh;
  });

  const LABEL_LIFT_FACTOR = 0.4;
  /// Maximum per-vertex elevation displacement (raw scalar from the noise field) seen inside each cell, used to lift labels above the noisiest part of the terrain.
  const terrainCellMaxHeights = $derived.by(() => {
    const result: number[] = new Array($cells.length).fill(0);
    if (!terrain) return result;
    const heights = terrain.heights;
    const cellIndices = terrain.cellIndices;
    for (let i = 0; i < cellIndices.length; i++) {
      const ci = cellIndices[i];
      const h = heights[i];
      if (h > result[ci]) result[ci] = h;
    }
    return result;
  });
  const cellLabelAnchors = $derived(
    $cells.map((cell, idx) => {
      const [cxv, cyv, czv] = cell.centroid;
      const [nx, ny, nz] = cell.normal;
      const lift = (terrainCellMaxHeights[idx] ?? 0) + cellRadius * LABEL_LIFT_FACTOR;
      return {
        x: cxv + nx * lift,
        y: cyv + ny * lift,
        z: czv + nz * lift,
      };
    }),
  );

  const LABEL_COLORS = [
    undefined,
    "#1d4ed8",
    "#15803d",
    "#b91c1c",
    "#1e3a8a",
    "#7c2d12",
    "#0e7490",
    "#111827",
    "#374151",
  ];

  // Push cell metadata to the DataTexture used by shaders which sample it using `aCellIndex` so a single byte covers all of a cell's vertices in all three layers.
  $effect(() => {
    if ($cells.length === 0) return;
    if (!$cellMetadata) return;

    const meta = cellMeta;
    const now = performance.now();
    for (let i = 0; i < $cellMetadata.length; i++) {
      const entry = $cellMetadata[i];
      meta.setExplored(i, entry.isExplored);
      meta.setVoid(i, entry.isVoid);
      meta.setRevealing(i, entry.isRevealing);
      // Initiate the fall animation the first time we see a void cell as explored. `fellCells` records cells that have already completed their fall so subsequent re-runs of this effect (e.g. unrelated metadata changes) don't restart the animation.
      if (entry.isExplored && entry.isVoid && !fallStart.has(i) && !fellCells.has(i)) {
        fallStart.set(i, now);
      }
    }
    meta.flush();
    invalidate();
  });

  const pulsesArray = $derived(
    Array.from($pulses)
      .reverse()
      .slice(0, MAX_PULSES)
      .concat(Array(Math.max(0, MAX_PULSES - $pulses.length)).fill(Pulse.nullPulse())),
  );
  let pulseTimersUniform = $derived(
    pulsesArray.map((p) => {
      if (p.id < 0) return 0;
      const elapsed = Math.max(0, nowMs - p.createdAtMs);
      const duration = Math.max(1, p.durationMs);
      return Math.min(1, elapsed / duration);
    }),
  );
  let pulsePositionsUniform = $derived(
    pulsesArray.map((p) => new Vector3(p.position[0], p.position[1], p.position[2])),
  );
  let pulseOriginCellsUniform = $derived(pulsesArray.map((p) => p.originCell));
  let pulseIsRemoteUniform = $derived(pulsesArray.map((p) => (p.isRemote ? 1 : 0)));
  let pulseMaxRadiiUniform = $derived(pulsesArray.map((p) => p.maxRadius));

  $effect(() => {
    terrainMaterial.uniforms.pulseCount.value = Math.min($pulses.length, MAX_PULSES);
    terrainMaterial.uniforms.pulseTimers.value = pulseTimersUniform;
    terrainMaterial.uniforms.pulsePositions.value = pulsePositionsUniform;
    terrainMaterial.uniforms.pulseOriginCells.value = pulseOriginCellsUniform;
    terrainMaterial.uniforms.pulseIsRemote.value = pulseIsRemoteUniform;
    terrainMaterial.uniforms.pulseMaxRadii.value = pulseMaxRadiiUniform;
    terrainMaterial.uniforms.uHighlightedCell.value =
      highlightedCellIndex === undefined ? -1 : highlightedCellIndex;
    terrainMaterial.uniforms.uTime.value = nowMs;
    invalidate();
  });

  function handleTerrainClick(event: { point: Vector3; face: { a: number } | null }) {
    if (!interactive) return;
    if (!event.face || $cells.length === 0) return;
    const aCellIndexArr = terrainGeometry.attributes.aCellIndex.array as Float32Array;
    const cellIndex = aCellIndexArr[event.face.a];
    if (cellIndex === undefined) return;
    gameState.queueExplorePulse(cellIndex, event.point.x, event.point.y, event.point.z);
    onCellClicked?.(cellIndex);
  }
</script>

<T is={opaqueMesh} />
<T is={terrainMesh} onclick={handleTerrainClick} />

{#each $cellMetadata as entry, i (i)}
  {#if entry.isExplored && !entry.isVoid && entry.voidNeighborCount > 0}
    {@const anchor = cellLabelAnchors[i]}
    {#if anchor}
      <Billboard position={[anchor.x, anchor.y, anchor.z]}>
        <Text
          text={entry.voidNeighborCount.toLocaleString(getLocale())}
          font={openSans}
          fontSize={cellRadius * 0.7}
          color={LABEL_COLORS[Math.min(entry.voidNeighborCount, LABEL_COLORS.length - 1)]}
          anchorX="center"
          anchorY="middle"
          outlineWidth={cellRadius * 0.04}
          outlineColor="#f8fafc"
        />
      </Billboard>
    {/if}
  {/if}
{/each}
