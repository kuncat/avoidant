<script module lang="ts">
  export const MAX_PULSES = 16;
  export const fragmentShader = `#define MAX_PULSES ${MAX_PULSES}\n` + _fragmentShader;
  const PULSE_DURATION = 250;
</script>

<script lang="ts">
  import _fragmentShader from "./fragment.glsl?raw";
  import vertexShader from "./vertex.glsl?raw";
  import type { GameState, MapCell } from "$lib/wasm-pkg/avoidant";
  import { T } from "@threlte/core";
  import { interactivity } from "@threlte/extras";
  import { DoubleSide, Vector3 } from "three";
  import { quadOut } from "svelte/easing";
  import { Tween } from "svelte/motion";
  import { SvelteSet } from "svelte/reactivity";

  interactivity();

  interface Props {
    gameState: GameState;
  }

  let { gameState = $bindable() }: Props = $props();
  let cells = $derived(gameState?.cells);

  function triangulateCell(vertices: MapCell["vertices"]): number[] {
    if (vertices.length < 3) return [];

    const [ax, ay, ah] = vertices[0];
    const trianglePositions: number[] = [];

    for (let i = 1; i < vertices.length - 1; i++) {
      const [bx, by, bh] = vertices[i];
      const [cx, cy, ch] = vertices[i + 1];
      trianglePositions.push(ax, ah, ay, bx, bh, by, cx, ch, cy);
    }

    return trianglePositions;
  }

  function cellEdgeRibbon(
    vertices: MapCell["vertices"],
    halfWidth: number,
    lift: number,
  ): number[] {
    if (vertices.length < 2) return [];

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

    return edgeTriangles;
  }

  class Pulse {
    originCell: number;
    position: Vector3;
    timer = new Tween(0, {
      easing: quadOut,
    });
    constructor(originCell: number, position: Vector3) {
      this.originCell = originCell;
      this.position = position;
    }

    start() {
      this.timer.set(0, { duration: 0 }).then(() => {
        this.timer.set(1, { duration: PULSE_DURATION }).then(() => {
          gameState.exploreCell(this.originCell);
          pulses.delete(this);
        });
      });
    }

    static null() {
      return new Pulse(-1, new Vector3());
    }
  }
  const pulses = new SvelteSet<Pulse>();
  const pulsesArray = $derived(
    Array.from(pulses)
      .reverse()
      .slice(0, MAX_PULSES)
      .concat(Array(Math.max(0, MAX_PULSES - pulses.size)).fill(Pulse.null())),
  );
  let pulseTimersUniform = $derived(pulsesArray.map((p) => p.timer.current));
  let pulsePositionsUniform = $derived(pulsesArray.map((p) => p.position));
  let pulseOriginCellsUniform = $derived(pulsesArray.map((p) => p.originCell));
</script>

<T.Group>
  {#each $cells as cell, cellIndex (cellIndex)}
    {@const trianglePositions = triangulateCell(cell.vertices)}
    {@const edgeRibbonBase = cellEdgeRibbon(cell.vertices, 0.24, 0.13)}
    {@const edgeRibbonHighlight = cellEdgeRibbon(cell.vertices, 0.13, 0.16)}
    {#if trianglePositions.length > 0}
      <T.Mesh
        onclick={(event: { point: Vector3 }) => {
          const pulse = new Pulse(cellIndex, event.point.clone());
          pulses.add(pulse);
          pulse.start();
        }}
      >
        <T.BufferGeometry attach="geometry">
          <T.BufferAttribute
            attach="attributes.position"
            args={[new Float32Array(trianglePositions), 3]}
          />
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
          }}
          uniforms.isExplored.value={cell.isExplored ? 1.0 : 0.0}
          uniforms.cellIndex.value={cellIndex}
          uniforms.pulseCount.value={pulses.size}
          uniforms.pulseTimers.value={pulseTimersUniform}
          uniforms.pulsePositions.value={pulsePositionsUniform}
          uniforms.pulseOriginCells.value={pulseOriginCellsUniform}
        />
      </T.Mesh>

      {#if edgeRibbonBase.length > 0}
        <T.Mesh>
          <T.BufferGeometry attach="geometry">
            <T.BufferAttribute
              attach="attributes.position"
              args={[new Float32Array(edgeRibbonBase), 3]}
            />
          </T.BufferGeometry>
          <T.MeshBasicMaterial attach="material" color="#8fbdd0" side={DoubleSide} />
        </T.Mesh>
      {/if}

      {#if edgeRibbonHighlight.length > 0}
        <T.Mesh>
          <T.BufferGeometry attach="geometry">
            <T.BufferAttribute
              attach="attributes.position"
              args={[new Float32Array(edgeRibbonHighlight), 3]}
            />
          </T.BufferGeometry>
          <T.MeshBasicMaterial attach="material" color="#e7fbff" side={DoubleSide} />
        </T.Mesh>
      {/if}
    {/if}
  {/each}
</T.Group>
