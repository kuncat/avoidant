<script module lang="ts">
  import _vertexShader from "./vertex.glsl?raw";
  import _fragmentShader from "./fragment.glsl?raw";
  import { PULSE_SWEEP_BAND } from "$lib/generated/shared-constants";

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
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { Pulse } from "wasm-pkg";
  import type { GameState } from "wasm-pkg";
  import { T, useThrelte } from "@threlte/core";
  import { interactivity, Text, Billboard } from "@threlte/extras";
  import {
    BufferAttribute,
    BufferGeometry,
    DataTexture,
    DoubleSide,
    Mesh,
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
    terrain?: { positions: number[]; normals: number[]; cellIndices: number[] } | undefined;
  }

  let { gameState = $bindable(), terrain = undefined }: Props = $props();
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
      if (hasActivePulses || hasFallingCells) {
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
   * Each emitted vertex carries an `aCellIndex` attribute so the fragment shader can look up per-cell metadata in {@link CellMetaTexture}. The subdivision (and therefore the terrain detail) is decoupled from the Voronoi cell-corner density and is controlled by the `terrainSubdivisions` field on {@link GameOptions}.
   *
   * When `shrinkFactor < 1`, every vertex's XZ position is contracted toward its cell's XZ centroid by that factor, producing gaps between adjacent cells. A second copy of the geometry rendered un-shrunken (with `shrinkFactor = 1`) underneath shows through those gaps as the "gap lines". Y values and normals are preserved as-is. At the 3-4% shrink range the slight inconsistency between a perimeter vertex's stored Y/normal and the true noise surface at its new XZ is invisible.
   *
   * @param payload - Pre-built positions (`[x, height, y]`-packed) and
   *   per-vertex cell indices produced by the mapgen worker. `undefined`
   *   yields an empty geometry, which is the expected state before mapgen
   *   completes.
   * @param insetDistance - World-space distance (XZ plane) to pull each vertex toward its cell centroid, producing a uniform-width gap along every cell boundary. `0` (default) leaves the mesh untouched. Per vertex the move is clamped to 80% of its centroid distance to avoid collapsing small cells.
   * @returns A `BufferGeometry` with `position`, `aNormal`, and `aCellIndex` attributes.
   */
  function buildTerrainLayer(
    payload: { positions: number[]; normals: number[]; cellIndices: number[] } | undefined,
    insetDistance = 0,
  ): BufferGeometry {
    const geometry = new BufferGeometry();
    if (!payload || payload.positions.length === 0) {
      geometry.setAttribute("position", new BufferAttribute(new Float32Array(0), 3));
      geometry.setAttribute("aNormal", new BufferAttribute(new Float32Array(0), 3));
      geometry.setAttribute("aCellIndex", new BufferAttribute(new Float32Array(0), 1));
      return geometry;
    }

    const positions = new Float32Array(payload.positions);
    if (insetDistance > 0) {
      const cellIndices = payload.cellIndices;
      const vertexCount = cellIndices.length;
      let maxCell = 0;
      for (let i = 0; i < vertexCount; i++) {
        if (cellIndices[i] > maxCell) maxCell = cellIndices[i];
      }
      const cellCount = maxCell + 1;
      const sumX = new Float64Array(cellCount);
      const sumZ = new Float64Array(cellCount);
      const count = new Uint32Array(cellCount);
      for (let i = 0; i < vertexCount; i++) {
        const c = cellIndices[i];
        sumX[c] += positions[i * 3 + 0];
        sumZ[c] += positions[i * 3 + 2];
        count[c]++;
      }
      const cx = new Float32Array(cellCount);
      const cz = new Float32Array(cellCount);
      for (let c = 0; c < cellCount; c++) {
        if (count[c] > 0) {
          cx[c] = sumX[c] / count[c];
          cz[c] = sumZ[c] / count[c];
        }
      }
      for (let i = 0; i < vertexCount; i++) {
        const c = cellIndices[i];
        const dx = positions[i * 3 + 0] - cx[c];
        const dz = positions[i * 3 + 2] - cz[c];
        const d = Math.hypot(dx, dz);
        if (d < 1e-6) continue;
        const move = Math.min(insetDistance, d);
        const k = move / d;
        positions[i * 3 + 0] -= dx * k;
        positions[i * 3 + 2] -= dz * k;
      }
    }

    geometry.setAttribute("position", new BufferAttribute(positions, 3));
    geometry.setAttribute("aNormal", new BufferAttribute(new Float32Array(payload.normals), 3));
    geometry.setAttribute(
      "aCellIndex",
      new BufferAttribute(new Float32Array(payload.cellIndices), 1),
    );
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
      this.texture.needsUpdate = true;
    }

    /** Release the underlying GPU resources. */
    dispose(): void {
      this.texture.dispose();
    }
  }

  // Each cell's vertices are pulled toward its XZ centroid by a constant distance (`CELL_GAP_HALF_WIDTH`). The total visible gap width is ~2× this value.
  const MAP_AREA = 100 * 100;
  const CELL_GAP_HALF_WIDTH = 0.08;
  const cellRadius = $derived(Math.sqrt(MAP_AREA / (Math.PI * Math.max(1, $cells.length))));
  const terrainGeometry = $derived(buildTerrainLayer(terrain, CELL_GAP_HALF_WIDTH));
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
      uniforms: {
        elevationMin: { value: gameState.elevationMin },
        elevationMax: { value: gameState.elevationMax },
        uCellMeta: { value: cellMeta.texture },
        uCellMetaSize: { value: cellMeta.width },
        uLightDir: { value: new Vector3(0.45, 1.0, 0.3).normalize() },
        uAmbient: { value: 0.55 },
        uDiffuse: { value: 0.75 },
        pulseCount: { value: 0 },
        pulseTimers: { value: new Array(MAX_PULSES).fill(0) },
        pulsePositions: {
          value: new Array(MAX_PULSES).fill(null).map(() => new Vector3()),
        },
        pulseOriginCells: { value: new Array(MAX_PULSES).fill(-1) },
        pulseIsRemote: { value: new Array(MAX_PULSES).fill(0) },
        pulseMaxRadii: { value: new Array(MAX_PULSES).fill(0) },
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

  let terrainMesh = $derived(new Mesh(terrainGeometry, terrainMaterial));

  const LABEL_LIFT_FACTOR = 0.075;
  const terrainCellMaxHeights = $derived.by(() => {
    const result: number[] = new Array($cells.length).fill(-Infinity);
    if (!terrain) return result;
    const positions = terrain.positions;
    const cellIndices = terrain.cellIndices;
    for (let i = 0; i < cellIndices.length; i++) {
      const ci = cellIndices[i];
      const h = positions[i * 3 + 1];
      if (h > result[ci]) result[ci] = h;
    }
    return result;
  });
  const cellLabelAnchors = $derived(
    $cells.map((cell, idx) => {
      const vs = cell.vertices;
      if (vs.length === 0) return { x: 0, y: 0, z: 0 };
      let sx = 0;
      let sz = 0;
      let maxH = -Infinity;
      for (const [vx, vy, vh] of vs) {
        sx += vx;
        sz += vy;
        if (vh > maxH) maxH = vh;
      }
      const terrainMax = terrainCellMaxHeights[idx];
      if (terrainMax !== undefined && terrainMax > maxH) maxH = terrainMax;
      return {
        x: sx / vs.length,
        y: maxH + cellRadius * LABEL_LIFT_FACTOR,
        z: sz / vs.length,
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
    invalidate();
  });

  function handleTerrainClick(event: { point: Vector3; face: { a: number } | null }) {
    if (!event.face || $cells.length === 0) return;
    const aCellIndexArr = terrainGeometry.attributes.aCellIndex.array as Float32Array;
    const cellIndex = aCellIndexArr[event.face.a];
    if (cellIndex === undefined) return;
    gameState.queueExplorePulse(cellIndex, event.point.x, event.point.y, event.point.z);
  }
</script>

<T is={terrainMesh} onclick={handleTerrainClick} />

{#each $cellMetadata as entry, i (i)}
  {#if entry.isExplored && !entry.isVoid && entry.voidNeighborCount > 0}
    {@const anchor = cellLabelAnchors[i]}
    {#if anchor}
      <Billboard position={[anchor.x, anchor.y, anchor.z]}>
        <Text
          text={String(entry.voidNeighborCount)}
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
