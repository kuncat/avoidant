<script lang="ts">
  import { onMount } from "svelte";
  import init, { GameState } from "$lib/wasm-pkg/avoidant.js";

  let isLoading = $state(true);
  let gameState = $state<GameState | undefined>(undefined);
  let cells = $derived(gameState?.cells);
  let numCellsInput = $state(160);
  let rngSeedInput = $state(999);

  onMount(async () => {
    await init();
    isLoading = false;
  });

  $effect(() => {
    console.log($state.snapshot($cells));
  });

  function startGame() {
    try {
      gameState = new GameState({
        numCells: $state.snapshot(numCellsInput),
        rngSeed: $state.snapshot(rngSeedInput),
      });
      gameState.generate_map();
    } catch (error) {
      console.error("Failed to start game", error);
    }
  }
</script>

<div class="border border-gray-400 max-w-lg rounded p-4 fixed top-4 left-4 bg-white">
  <h1>Avoidant</h1>

  {#if isLoading}
    <p>Loading...</p>
  {:else if (!gameState)}
    <form class="w-full max-w-lg">
      <div class="flex flex-wrap -mx-3 mb-2">
        <div class="w-full md:w-1/2 px-3 mb-6 md:mb-0">
          <label class="block uppercase tracking-wide text-gray-700 text-xs font-bold mb-2" for="grid-first-name">
            Size
          </label>
          <input class="appearance-none block w-full bg-gray-200 text-gray-700 border rounded py-3 px-4 mb-3 leading-tight focus:outline-none focus:bg-white" id="grid-first-name" type="number" inputmode="numeric" bind:value={numCellsInput} min="100" max="4096" step="1">
        </div>
        <div class="w-full md:w-1/2 px-3">
          <label class="block uppercase tracking-wide text-gray-700 text-xs font-bold mb-2" for="grid-last-name">
            RNG Seed
          </label>
          <input class="appearance-none block w-full bg-gray-200 text-gray-700 border border-gray-200 rounded py-3 px-4 leading-tight focus:outline-none focus:bg-white focus:border-gray-500" id="grid-last-name" type="number" inputmode="numeric" bind:value={rngSeedInput} min="0">
        </div>
      </div>
      <div class="flex flex-wrap gap-2">
        <button class="bg-teal-500 hover:bg-teal-700 border-teal-500 hover:border-teal-700 text-sm border-4 text-white py-1 px-2 rounded" type="button" onclick={startGame}>
          Start Game
        </button>
      </div>
    </form>
  {/if}
</div>

<div class="w-full h-screen p-2">
  <svg viewBox="0 0 100 100" preserveAspectRatio="none" class="stroke-teal-500 w-full h-full border border-gray-300 bg-white">
    {#each $cells as cell, idx (idx)}
      <polygon points={cell.vertices.map(([x, y]) => `${x},${y}`).join(" ")} fill="none" stroke-width="0.35" />
    {/each}
  </svg>
</div>
