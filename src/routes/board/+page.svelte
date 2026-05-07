<script lang="ts">
  import { onMount } from "svelte";
  import init, { GameState } from "wasm-pkg";
  import { Canvas, T } from "@threlte/core";
  import { OrbitControls } from "@threlte/extras";
  import { MOUSE } from "three";
  import Board from "$lib/components/board.svelte";

  let isLoading = $state(true);
  let gameState = $state<GameState | undefined>(undefined);
  let numCellsInput = $state(160);
  let rngSeedInput = $state(999);
  let playerNameInput = $state("Player");
  let gameConfig: "host" | "join" | undefined = $state(undefined);
  let ticketInput = $state("");
  let inviteTicket = $state("");

  onMount(async () => {
    await init();
    isLoading = false;
  });

  function startGame() {
    try {
      gameState = new GameState({
        elevationMax: 6.0,
        elevationMin: 0.0,
        numCells: $state.snapshot(numCellsInput),
        rngSeed: $state.snapshot(rngSeedInput),
        spikiness: 0.8,
      });
      gameState.generate_map();
      inviteTicket = "";
    } catch (error) {
      console.error("Failed to start game", error);
    } finally {
      gameConfig = undefined;
    }
  }

  async function joinGame() {
    try {
      gameState = await GameState.joinFromTicket(ticketInput, playerNameInput);
      inviteTicket = "";
    } catch (error) {
      console.error("Failed to join game", error);
    } finally {
      gameConfig = undefined;
      ticketInput = "";
    }
  }
</script>

<div
  class="fixed top-4 left-4 z-10 max-w-lg rounded border border-gray-400 bg-white p-4"
  class:opacity-50={gameState}
>
  <h1>Avoidant</h1>

  {#if isLoading}
    <p>Loading...</p>
  {:else if !gameState}
    {#if gameConfig === "host"}
      <form
        class="w-full max-w-lg"
        onsubmit={(event) => {
          event.preventDefault();
          startGame();
        }}
      >
        <div class="-mx-3 mb-2 flex flex-wrap">
          <div class="mb-6 w-full px-3 md:mb-0">
            <label
              class="mb-2 block text-xs font-bold tracking-wide text-gray-700 uppercase"
              for="player-name"
            >
              Player Name
            </label>
            <input
              class="mb-3 block w-full appearance-none rounded border bg-gray-200 px-4 py-3 leading-tight text-gray-700 focus:bg-white focus:outline-none"
              id="player-name"
              type="text"
              bind:value={playerNameInput}
            />
          </div>
          <div class="mb-6 w-full px-3 md:mb-0 md:w-1/2">
            <label
              class="mb-2 block text-xs font-bold tracking-wide text-gray-700 uppercase"
              for="grid-first-name"
            >
              Size
            </label>
            <input
              class="mb-3 block w-full appearance-none rounded border bg-gray-200 px-4 py-3 leading-tight text-gray-700 focus:bg-white focus:outline-none"
              id="grid-first-name"
              type="number"
              inputmode="numeric"
              bind:value={numCellsInput}
              min="100"
              max="4096"
              step="1"
            />
          </div>
          <div class="w-full px-3 md:w-1/2">
            <label
              class="mb-2 block text-xs font-bold tracking-wide text-gray-700 uppercase"
              for="grid-last-name"
            >
              RNG Seed
            </label>
            <input
              class="block w-full appearance-none rounded border border-gray-200 bg-gray-200 px-4 py-3 leading-tight text-gray-700 focus:border-gray-500 focus:bg-white focus:outline-none"
              id="grid-last-name"
              type="number"
              inputmode="numeric"
              bind:value={rngSeedInput}
              min="0"
            />
          </div>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            class="rounded border-4 border-teal-500 bg-teal-500 px-2 py-1 text-sm text-white hover:border-teal-700 hover:bg-teal-700"
            type="submit"
          >
            Start
          </button>
        </div>
      </form>
    {:else if gameConfig === "join"}
      <form
        class="w-full max-w-lg"
        onsubmit={(event) => {
          event.preventDefault();
          void joinGame();
        }}
      >
        <div class="-mx-3 mb-2 flex flex-wrap">
          <div class="mb-6 w-full px-3 md:mb-0">
            <label
              class="mb-2 block text-xs font-bold tracking-wide text-gray-700 uppercase"
              for="join-player-name"
            >
              Player Name
            </label>
            <input
              class="mb-3 block w-full appearance-none rounded border bg-gray-200 px-4 py-3 leading-tight text-gray-700 focus:bg-white focus:outline-none"
              id="join-player-name"
              type="text"
              bind:value={playerNameInput}
            />
          </div>
          <div class="mb-6 w-full px-3 md:mb-0">
            <label
              class="mb-2 block text-xs font-bold tracking-wide text-gray-700 uppercase"
              for="grid-first-name"
            >
              Ticket
            </label>
            <input
              class="mb-3 block w-full appearance-none rounded border bg-gray-200 px-4 py-3 leading-tight text-gray-700 focus:bg-white focus:outline-none"
              id="grid-first-name"
              type="text"
              bind:value={ticketInput}
            />
          </div>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            class="rounded border-4 border-teal-500 bg-teal-500 px-2 py-1 text-sm text-white hover:border-teal-700 hover:bg-teal-700"
            type="submit"
          >
            Join
          </button>
        </div>
      </form>
    {:else}
      <button
        class="rounded border-4 border-teal-500 bg-teal-500 px-2 py-1 text-sm text-white hover:border-teal-700 hover:bg-teal-700"
        type="button"
        onclick={() => (gameConfig = "host")}
      >
        New Game
      </button>
      <button
        class="rounded border-4 border-teal-500 bg-teal-500 px-2 py-1 text-sm text-white hover:border-teal-700 hover:bg-teal-700"
        type="button"
        onclick={() => (gameConfig = "join")}
      >
        Join Game
      </button>
    {/if}
  {:else}
    <button
      class="rounded border-4 border-teal-500 bg-teal-500 px-2 py-1 text-sm text-white hover:border-teal-700 hover:bg-teal-700"
      type="button"
      onclick={async () => {
        try {
          inviteTicket = (await gameState?.invite(playerNameInput)) ?? "";
        } catch (error) {
          console.error("Failed to create invitation", error);
        }
      }}
    >
      Invite Player
    </button>

    {#if inviteTicket}
      <div class="mt-3">
        <label
          class="mb-2 block text-xs font-bold tracking-wide text-gray-700 uppercase"
          for="invite-ticket"
        >
          Invitation Ticket
        </label>
        <input
          id="invite-ticket"
          class="mb-2 block w-full appearance-none rounded border bg-gray-200 px-4 py-3 leading-tight text-gray-700 focus:bg-white focus:outline-none"
          type="text"
          readonly
          value={inviteTicket}
        />
      </div>
    {/if}
  {/if}
</div>

{#if gameState}
  <div class="h-screen w-full p-2">
    <Canvas colorSpace="srgb-linear">
      <T.OrthographicCamera makeDefault zoom={7} near={0.1} far={1000} position={[50, 180, 180]} />
      <Board bind:gameState />
      <OrbitControls
        enableDamping
        enablePan={true}
        enableZoom={true}
        enableRotate={true}
        minPolarAngle={0}
        maxPolarAngle={Math.PI / 2}
        target={[50, 0, 50]}
        mouseButtons={{ LEFT: MOUSE.PAN, MIDDLE: MOUSE.DOLLY, RIGHT: MOUSE.ROTATE }}
      />
    </Canvas>
  </div>
{/if}
