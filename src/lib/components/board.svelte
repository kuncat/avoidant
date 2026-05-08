<script module lang="ts">
  export const MAX_PULSES = 16;
  export const fragmentShader = `#define MAX_PULSES ${MAX_PULSES}\n` + _fragmentShader;
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import _fragmentShader from "./fragment.glsl?raw";
  import vertexShader from "./vertex.glsl?raw";
  import { Pulse } from "wasm-pkg";
  import type { GameState, MapCell } from "wasm-pkg";
  import { T } from "@threlte/core";
  import { interactivity } from "@threlte/extras";
  import { DoubleSide, Vector3 } from "three";

  interactivity();

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
    const now = () => globalThis.performance?.now?.() ?? Date.now();
    const tick = () => {
      nowMs = now();
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  });

  function triangulateCell(vertices: MapCell["vertices"]): Float32Array {
    if (vertices.length < 3) return new Float32Array(0);

    const [ax, ay, ah] = vertices[0];
    const trianglePositions: number[] = [];

    for (let i = 1; i < vertices.length - 1; i++) {
      const [bx, by, bh] = vertices[i];
      const [cx, cy, ch] = vertices[i + 1];
      trianglePositions.push(ax, ah, ay, bx, bh, by, cx, ch, cy);
    }

    return new Float32Array(trianglePositions);
  }

  function cellEdgeRibbon(
    vertices: MapCell["vertices"],
    halfWidth: number,
    lift: number,
  ): Float32Array {
    if (vertices.length < 2) return new Float32Array(0);

    const edgeTriangles: number[] = [];

    for (let i = 0; i < vertices.length; i++) {
      const [ax, ay, ah] = vertices[i];
      const [bx, by, bh] = vertices[(i + 1) % vertices.length];

      const dx = bx - ax;
      const dy = by - ay;
      const length = Math.hypot(dx, dy);
      if (length < 1e-6) continue;

      const nx = (-dy / length) * halfWidth;
      const ny = (dx / length) * halfWidth;

      const aLeft = [ax + nx, ah + lift, ay + ny];
      const aRight = [ax - nx, ah + lift, ay - ny];
      const bLeft = [bx + nx, bh + lift, by + ny];
      const bRight = [bx - nx, bh + lift, by - ny];

      edgeTriangles.push(...aLeft, ...aRight, ...bLeft, ...bLeft, ...aRight, ...bRight);
    }

    return new Float32Array(edgeTriangles);
  }

  let cellGeometries = $derived.by(() =>
    Array.from($cells ?? []).map((cell) => ({
      trianglePositions: triangulateCell(cell.vertices),
      edgeRibbonBase: cellEdgeRibbon(cell.vertices, 0.24, 0.13),
      edgeRibbonHighlight: cellEdgeRibbon(cell.vertices, 0.13, 0.16),
    })),
  );

  const pulsesArray = $derived(
    Array.from($pulses ?? [])
      .reverse()
      .slice(0, MAX_PULSES)
      .concat(Array(Math.max(0, MAX_PULSES - ($pulses ?? []).length)).fill(Pulse.nullPulse())),
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
</script>

<T.Group>
  {#each cellGeometries as geometry, cellIndex (cellIndex)}
    {@const metadata = $cellMetadata[cellIndex]}
    {#if geometry.trianglePositions.length > 0 && !metadata?.isVoid}
      <T.Mesh
        onclick={(event: { point: Vector3 }) =>
          gameState.queueExplorePulse(cellIndex, event.point.x, event.point.y, event.point.z)}
      >
        <T.BufferGeometry attach="geometry">
          <T.BufferAttribute attach="attributes.position" args={[geometry.trianglePositions, 3]} />
        </T.BufferGeometry>
        <T.ShaderMaterial
          side={DoubleSide}
          {fragmentShader}
          {vertexShader}
          uniforms={{
            isExplored: { value: 0 },
            elevationMin: { value: gameState.elevationMin },
            elevationMax: { value: gameState.elevationMax },
            cellIndex: { value: 0 },
            pulseCount: { value: 0 },
            pulseTimers: { value: new Array(MAX_PULSES).fill(0) },
            pulsePositions: { value: new Array(MAX_PULSES).fill(null).map(() => new Vector3()) },
            pulseOriginCells: { value: new Array(MAX_PULSES).fill(-1) },
            pulseIsRemote: { value: new Array(MAX_PULSES).fill(0) },
          }}
          uniforms.isExplored.value={metadata?.isExplored ? 1.0 : 0.0}
          uniforms.cellIndex.value={cellIndex}
          uniforms.pulseCount.value={Math.min(($pulses ?? []).length, MAX_PULSES)}
          uniforms.pulseTimers.value={pulseTimersUniform}
          uniforms.pulsePositions.value={pulsePositionsUniform}
          uniforms.pulseOriginCells.value={pulseOriginCellsUniform}
          uniforms.pulseIsRemote.value={pulseIsRemoteUniform}
        />
      </T.Mesh>

      {#if geometry.edgeRibbonBase.length > 0}
        <T.Mesh>
          <T.BufferGeometry attach="geometry">
            <T.BufferAttribute attach="attributes.position" args={[geometry.edgeRibbonBase, 3]} />
          </T.BufferGeometry>
          <T.MeshBasicMaterial attach="material" color="#8fbdd0" side={DoubleSide} />
        </T.Mesh>
      {/if}

      {#if geometry.edgeRibbonHighlight.length > 0}
        <T.Mesh>
          <T.BufferGeometry attach="geometry">
            <T.BufferAttribute
              attach="attributes.position"
              args={[geometry.edgeRibbonHighlight, 3]}
            />
          </T.BufferGeometry>
          <T.MeshBasicMaterial attach="material" color="#e7fbff" side={DoubleSide} />
        </T.Mesh>
      {/if}
    {/if}
  {/each}
</T.Group>
