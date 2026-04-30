<script lang="ts">
  import fragmentShader from "./fragment.glsl?raw";
  import vertexShader from "./vertex.glsl?raw";
  import type { GameState, MapCell } from "$lib/wasm-pkg/avoidant";
  import { T } from "@threlte/core";
  import { interactivity } from "@threlte/extras";
  import { DoubleSide, Vector3 } from "three";
  import { quadOut } from "svelte/easing";
  import { Tween } from "svelte/motion";

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

  interactivity();
  const pulsePosition = new Vector3();
  let pulsedCellIndex = $state<number | undefined>(undefined);
  const pulseTimer = new Tween(0, {
    easing: quadOut,
  });
</script>

<T.Group>
  {#each $cells as cell, cellIndex (cellIndex)}
    {@const trianglePositions = triangulateCell(cell.vertices)}
    {@const edgeRibbonBase = cellEdgeRibbon(cell.vertices, 0.24, 0.13)}
    {@const edgeRibbonHighlight = cellEdgeRibbon(cell.vertices, 0.13, 0.16)}
    {#if trianglePositions.length > 0}
      <T.Mesh
        onclick={(event: { point: Vector3 }) => {
          console.log("Cell clicked:", cellIndex, $state.snapshot(cell.isExplored));
          pulsedCellIndex = cellIndex;
          pulsePosition.copy(event.point);
          pulseTimer.set(0, { duration: 0 }).then(() => {
            pulseTimer.set(1, { duration: 2000 }).then(() => {
              gameState.exploreCell(cellIndex);
              pulsedCellIndex = undefined;
            });
          });
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
            isPulsedCell: { value: 0 },
            elevationMin: { value: gameState.elevationMin },
            elevationMax: { value: gameState.elevationMax },
            pulseTimer: { value: 0 },
            pulsePosition: { value: pulsePosition },
          }}
          uniforms.isExplored.value={cell.isExplored ? 1.0 : 0.0}
          uniforms.isPulsedCell.value={cellIndex === pulsedCellIndex ? 1.0 : 0.0}
          uniforms.pulseTimer.value={pulseTimer.current}
          uniforms.pulsePosition.value={pulsePosition}
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
