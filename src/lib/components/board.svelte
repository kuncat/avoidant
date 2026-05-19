<script module lang="ts">
  import _vertexShader from "./vertex.glsl?raw";
  import _fragmentShader from "./fragment.glsl?raw";
  import _ribbonVertexShader from "./ribbon.vertex.glsl?raw";
  import _ribbonFragmentShader from "./ribbon.fragment.glsl?raw";

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
  export const ribbonVertexShader = cellMetaDefines + fallDefines + _ribbonVertexShader;
  export const fragmentShader =
    `#define MAX_PULSES ${MAX_PULSES}\n` + cellMetaDefines + _fragmentShader;
  export const ribbonFragmentShader = cellMetaDefines + _ribbonFragmentShader;
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { Pulse } from "wasm-pkg";
  import type { GameState, MapCell } from "wasm-pkg";
  import { T, useThrelte } from "@threlte/core";
  import { interactivity } from "@threlte/extras";
  import {
    BufferAttribute,
    BufferGeometry,
    Color,
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
  }

  let { gameState = $bindable() }: Props = $props();
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
   * Build a merged triangle mesh for all cell interiors.
   *
   * Fan-triangulates each cell's polygon and packs every vertex with an `aCellIndex` attribute so the fragment shader can look up per-cell metadata in {@link CellMetaTexture}.
   *
   * @param cellList - Map cells whose `vertices` are `[x, y, height]` 3ples.
   * @returns A `BufferGeometry` with `position` and `aCellIndex` attributes.
   */
  function buildTerrainLayer(cellList: MapCell[]): BufferGeometry {
    const cellCount = cellList.length;
    const positions: number[] = [];
    const aCellIndex: number[] = [];

    for (let i = 0; i < cellCount; i++) {
      const vertices = cellList[i].vertices;

      if (vertices.length >= 3) {
        const [ax, ay, ah] = vertices[0];
        for (let j = 1; j < vertices.length - 1; j++) {
          const [bx, by, bh] = vertices[j];
          const [cx, cy, ch] = vertices[j + 1];
          positions.push(ax, ah, ay, bx, bh, by, cx, ch, cy);
          for (let k = 0; k < 3; k++) aCellIndex.push(i);
        }
      }
    }

    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new BufferAttribute(new Float32Array(positions), 3));
    geometry.setAttribute("aCellIndex", new BufferAttribute(new Float32Array(aCellIndex), 1));
    return geometry;
  }

  /**
   * Build a merged ribbon mesh that traces each cell's edges as a thin quad strip.
   *
   * Each edge is extruded along its 2D normal by `halfWidth` and lifted by `lift` to sit above the terrain. Every emitted vertex carries an `aCellIndex` attribute so the fragment shader can discard ribbon segments that belong to void cells.
   *
   * @param cellList - Map cells whose `vertices` are `[x, y, height]` 3ples.
   * @param halfWidth - Half the ribbon's width in world units.
   * @param lift - Vertical offset added to each ribbon vertex.
   * @returns A `BufferGeometry` with `position` and `aCellIndex` attributes.
   */
  function buildRibbonLayer(cellList: MapCell[], halfWidth: number, lift: number): BufferGeometry {
    const cellCount = cellList.length;
    const positions: number[] = [];
    const aCellIndex: number[] = [];

    for (let i = 0; i < cellCount; i++) {
      const vertices = cellList[i].vertices;

      if (vertices.length >= 2) {
        for (let v = 0; v < vertices.length; v++) {
          const [ax, ay, ah] = vertices[v];
          const [bx, by, bh] = vertices[(v + 1) % vertices.length];
          const dx = bx - ax;
          const dy = by - ay;
          const length = Math.hypot(dx, dy);
          if (length < 1e-6) continue;

          const nx = (-dy / length) * halfWidth;
          const ny = (dx / length) * halfWidth;
          const yA = ah + lift;
          const yB = bh + lift;
          const aLx = ax + nx,
            aLz = ay + ny;
          const aRx = ax - nx,
            aRz = ay - ny;
          const bLx = bx + nx,
            bLz = by + ny;
          const bRx = bx - nx,
            bRz = by - ny;

          // Two triangles forming the edge quad.
          positions.push(aLx, yA, aLz, aRx, yA, aRz, bLx, yB, bLz);
          positions.push(bLx, yB, bLz, aRx, yA, aRz, bRx, yB, bRz);
          for (let k = 0; k < 6; k++) aCellIndex.push(i);
        }
      }
    }

    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new BufferAttribute(new Float32Array(positions), 3));
    geometry.setAttribute("aCellIndex", new BufferAttribute(new Float32Array(aCellIndex), 1));
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

    /** Mark the texture dirty so Three.js re-uploads it on the next frame. */
    flush(): void {
      this.texture.needsUpdate = true;
    }

    /** Release the underlying GPU resources. */
    dispose(): void {
      this.texture.dispose();
    }
  }

  const terrain = $derived(buildTerrainLayer($cells));

  // `*_HALF_WIDTH_FACTOR`: ribbon thickness as a fraction of cell radius.
  // `*_LIFT_FACTOR`: vertical offset above the terrain, also as a fraction of cell radius (keeps z-fight margin proportional and prevents the sheen from towering over tiny cells at high counts).
  const MAP_AREA = 100 * 100;
  const RIBBON_BASE_HALF_WIDTH_FACTOR = 0.025;
  const RIBBON_BASE_LIFT_FACTOR = 0.001;
  const RIBBON_HIGHLIGHT_HALF_WIDTH_FACTOR = 0.005;
  const RIBBON_HIGHLIGHT_LIFT_FACTOR = 0.0011;
  const cellRadius = $derived(Math.sqrt(MAP_AREA / (Math.PI * Math.max(1, $cells.length))));
  const ribbonBase = $derived(
    buildRibbonLayer(
      $cells,
      cellRadius * RIBBON_BASE_HALF_WIDTH_FACTOR,
      cellRadius * RIBBON_BASE_LIFT_FACTOR,
    ),
  );
  const ribbonHighlight = $derived(
    buildRibbonLayer(
      $cells,
      cellRadius * RIBBON_HIGHLIGHT_HALF_WIDTH_FACTOR,
      cellRadius * RIBBON_HIGHLIGHT_LIFT_FACTOR,
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
    const layers = [terrain, ribbonBase, ribbonHighlight];
    const meta = cellMeta;
    return () => {
      layers.forEach((layer) => layer.dispose());
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
      },
    }),
  );

  let ribbonBaseMaterial = $derived(
    new ShaderMaterial({
      vertexShader: ribbonVertexShader,
      fragmentShader: ribbonFragmentShader,
      side: DoubleSide,
      transparent: true,
      uniforms: {
        uColor: { value: new Color("#08090c") },
        uCellMeta: { value: cellMeta.texture },
        uCellMetaSize: { value: cellMeta.width },
      },
    }),
  );

  let ribbonHighlightMaterial = $derived(
    new ShaderMaterial({
      vertexShader: ribbonVertexShader,
      fragmentShader: ribbonFragmentShader,
      side: DoubleSide,
      transparent: true,
      uniforms: {
        uColor: { value: new Color("#f4fbff") },
        uCellMeta: { value: cellMeta.texture },
        uCellMetaSize: { value: cellMeta.width },
      },
    }),
  );

  // Dispose materials when re-derived.
  $effect(() => {
    const materials = [terrainMaterial, ribbonBaseMaterial, ribbonHighlightMaterial];
    return () => {
      materials.forEach((material) => material.dispose());
    };
  });

  let terrainMesh = $derived(new Mesh(terrain, terrainMaterial));
  let ribbonBaseMesh = $derived(new Mesh(ribbonBase, ribbonBaseMaterial));
  let ribbonHighlightMesh = $derived(new Mesh(ribbonHighlight, ribbonHighlightMaterial));

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

  $effect(() => {
    terrainMaterial.uniforms.pulseCount.value = Math.min($pulses.length, MAX_PULSES);
    terrainMaterial.uniforms.pulseTimers.value = pulseTimersUniform;
    terrainMaterial.uniforms.pulsePositions.value = pulsePositionsUniform;
    terrainMaterial.uniforms.pulseOriginCells.value = pulseOriginCellsUniform;
    terrainMaterial.uniforms.pulseIsRemote.value = pulseIsRemoteUniform;
    invalidate();
  });

  function handleTerrainClick(event: { point: Vector3; face: { a: number } | null }) {
    if (!event.face || $cells.length === 0) return;
    const aCellIndexArr = terrain.attributes.aCellIndex.array as Float32Array;
    const cellIndex = aCellIndexArr[event.face.a];
    if (cellIndex === undefined) return;
    gameState.queueExplorePulse(cellIndex, event.point.x, event.point.y, event.point.z);
  }
</script>

<T is={terrainMesh} onclick={handleTerrainClick} />
<T is={ribbonBaseMesh} />
<T is={ribbonHighlightMesh} />
