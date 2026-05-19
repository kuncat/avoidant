<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import init, { GameState, type GameOptions } from "wasm-pkg";
  import { Canvas, T } from "@threlte/core";
  import { OrbitControls } from "@threlte/extras";
  import { MOUSE } from "three";
  import Board from "$lib/components/board.svelte";
  import { generateMap } from "$lib/workers/mapgen-client";

  let status: string | undefined = $state(undefined);
  let gameState = $state<GameState | undefined>(undefined);
  let numCellsInput = $state(160);
  let rngSeedInput = $state(999);
  let playerNameInput = $state("Player");
  let setupMode: "host" | "join" | undefined = $state(undefined);
  let ticketInput = $state("");
  let inviteTicket = $state("");
  let isGeneratingInvite = $state(false);
  let networkSnapshot = $derived(gameState?.networkSnapshot);

  let connectedPeerCount = $derived(
    ($networkSnapshot?.peers ?? []).filter((peer) => peer.isConnected).length,
  );

  onMount(() => {
    const initializeWasm = async () => {
      try {
        status = "Loading...";
        await init();
        status = undefined;
      } catch (error) {
        status = "Failed to initialize wasm.";
        console.error("Failed to initialize wasm", error);
      }
    };

    void initializeWasm();
  });

  async function startGame() {
    try {
      const options: GameOptions = {
        elevationMax: 6.0,
        elevationMin: 0.0,
        numCells: $state.snapshot(numCellsInput),
        rngSeed: $state.snapshot(rngSeedInput),
        spikiness: 0.8,
      };
      status = "Generating map...";
      gameState = new GameState(options);
      gameState.applyMapCells(await generateMap(options));
      inviteTicket = "";
    } catch (error) {
      console.error("Failed to start game", error);
    } finally {
      setupMode = undefined;
      status = undefined;
    }
  }

  async function joinGame() {
    try {
      status = "Joining game...";
      const options = GameState.optionsFromTicket(ticketInput);
      gameState = new GameState(options);
      status = "Generating map...";
      gameState.applyMapCells(await generateMap(options));
      status = "Joining game...";
      await gameState.joinAsPeer(ticketInput, playerNameInput);
      inviteTicket = "";
    } catch (error) {
      console.error("Failed to join game", error);
    } finally {
      setupMode = undefined;
      ticketInput = "";
      status = undefined;
    }
  }

  function exitGame() {
    try {
      gameState?.free();
    } catch (error) {
      console.error("Failed to release game state", error);
    }
    gameState = undefined;
    setupMode = undefined;
    inviteTicket = "";
    ticketInput = "";
    status = undefined;
  }

  async function copyInviteTicket() {
    if (!inviteTicket) {
      return;
    }
    try {
      await navigator.clipboard.writeText(inviteTicket);
    } catch (error) {
      console.error("Failed to copy invitation ticket", error);
    }
  }

  async function generateInvite() {
    if (isGeneratingInvite) {
      return;
    }
    isGeneratingInvite = true;
    try {
      inviteTicket = (await gameState?.invite(playerNameInput)) ?? "";
      if (inviteTicket) {
        await copyInviteTicket();
      }
    } catch (error) {
      console.error("Failed to create invitation", error);
    } finally {
      isGeneratingInvite = false;
    }
  }
</script>

<div
  class="panel-wrapper fixed inset-x-0 z-20 flex justify-center select-none"
  class:[&_*]:text-white={gameState}
  class:panel-wrapper-card={!gameState}
  class:panel-wrapper-header={gameState}
>
  <div
    class="panel-shell w-full"
    class:max-w-full={gameState}
    class:max-w-md={!gameState && setupMode}
    class:max-w-xs={!gameState && !setupMode}
    class:panel-card={!gameState}
    class:panel-header={gameState}
    class:py-6={!gameState && setupMode}
    class:py-12={!gameState && !setupMode}
  >
    <div class="flex flex-wrap items-center gap-3" class:justify-center={!gameState}>
      <h1
        class="panel-title font-semibold tracking-wide"
        class:pb-4={!gameState}
        class:text-5xl={!gameState}
        class:text-lg={gameState}
      >
        Avoidant
      </h1>
      {#if gameState}
        <div class="ml-auto text-sm text-slate-600">
          {#if connectedPeerCount > 0}
            Players: <strong class="text-slate-800">{connectedPeerCount + 1}</strong>
          {/if}
        </div>
        <div class="flex gap-2">
          <button
            class="btn btn-primary"
            type="button"
            onclick={generateInvite}
            disabled={isGeneratingInvite}
          >
            {#if isGeneratingInvite}
              <span class="spinner" aria-hidden="true"></span>
              <span>Preparing…</span>
            {:else}
              Invite
            {/if}
          </button>
          <button class="btn btn-danger" type="button" onclick={exitGame}>Exit</button>
        </div>
      {/if}
    </div>

    {#if status}
      <p class="mt-2 text-sm text-slate-600">{status}</p>
    {/if}

    {#if !gameState}
      {#if setupMode === "host"}
        <form
          class="mt-4 w-full"
          transition:slide={{ duration: 150 }}
          onsubmit={async (event) => {
            event.preventDefault();
            await startGame();
          }}
        >
          <div class="-mx-3 mb-2 flex flex-wrap">
            <div class="mb-6 w-full px-3 md:mb-0">
              <label class="field-label" for="player-name">Player Name</label>
              <input class="field" id="player-name" type="text" bind:value={playerNameInput} />
            </div>
            <div class="mb-6 w-full px-3 md:mb-0 md:w-1/2">
              <label class="field-label" for="grid-first-name">Size</label>
              <input
                class="field"
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
              <label class="field-label" for="grid-last-name">RNG Seed</label>
              <input
                class="field"
                id="grid-last-name"
                type="number"
                inputmode="numeric"
                bind:value={rngSeedInput}
                min="0"
              />
            </div>
          </div>
          <div class="flex flex-wrap justify-center gap-2">
            <button class="btn btn-secondary" type="button" onclick={() => (setupMode = undefined)}>
              Back
            </button>
            <button class="btn btn-primary" type="submit">Start</button>
          </div>
        </form>
      {:else if setupMode === "join"}
        <form
          class="mt-4 w-full"
          transition:slide={{ duration: 220 }}
          onsubmit={async (event) => {
            event.preventDefault();
            await joinGame();
          }}
        >
          <div class="-mx-3 mb-2 flex flex-wrap">
            <div class="mb-6 w-full px-3 md:mb-0">
              <label class="field-label" for="join-player-name">Player Name</label>
              <input class="field" id="join-player-name" type="text" bind:value={playerNameInput} />
            </div>
            <div class="mb-6 w-full px-3 md:mb-0">
              <label class="field-label" for="grid-first-name">Ticket</label>
              <input class="field" id="grid-first-name" type="text" bind:value={ticketInput} />
            </div>
          </div>
          <div class="flex flex-wrap justify-center gap-2">
            <button class="btn btn-secondary" type="button" onclick={() => (setupMode = undefined)}>
              Back
            </button>
            <button class="btn btn-primary" type="submit">Join</button>
          </div>
        </form>
      {:else}
        <div class="mt-4 flex flex-wrap justify-center gap-2" transition:slide={{ duration: 220 }}>
          <button class="btn btn-primary" type="button" onclick={() => (setupMode = "host")}>
            New Game
          </button>
          <button class="btn btn-primary" type="button" onclick={() => (setupMode = "join")}>
            Join Game
          </button>
        </div>
      {/if}
    {:else if inviteTicket}
      <div class="mt-3" transition:slide={{ duration: 200 }}>
        <div class="mb-2 flex items-center justify-between gap-2">
          <label class="field-label mb-0" for="invite-ticket">Invitation Ticket</label>
          <button
            class="icon-btn"
            type="button"
            aria-label="Close invitation"
            onclick={() => (inviteTicket = "")}
          >
            ×
          </button>
        </div>
        <div class="flex items-stretch gap-2">
          <input
            id="invite-ticket"
            class="field mb-0 flex-1"
            type="text"
            readonly
            value={inviteTicket}
          />
          <button class="btn btn-secondary" type="button" onclick={copyInviteTicket}> Copy </button>
        </div>
      </div>
    {/if}
  </div>
</div>

{#if gameState}
  <div class="h-screen w-full">
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

<style>
  @reference "../layout.css";

  h1 {
    @apply m-0;
  }

  .panel-wrapper {
    @apply transition-all duration-500 ease-out;
  }

  .panel-wrapper-card {
    @apply top-1/2 -translate-y-1/2;
  }

  .panel-wrapper-header {
    @apply top-0 translate-y-0;
  }

  .panel-shell {
    @apply border border-slate-300/50 shadow-lg backdrop-blur transition-all duration-500 ease-out;
  }

  .panel-card {
    @apply rounded bg-slate-300 px-6 text-center shadow-xl;
  }

  .panel-header {
    @apply rounded-none border-x-0 border-t-0 bg-slate-900/75 p-2 text-left;

    button {
      @apply h-6;
    }

    .field {
      @apply mb-0 h-6 border-slate-600 bg-slate-700 px-2 py-0 text-sm text-slate-100 placeholder:text-slate-400 focus:bg-slate-600;
    }
  }

  .panel-title {
    @apply transition-all duration-500 ease-out;
  }

  .btn {
    @apply inline-flex items-center justify-center rounded border px-2 py-1 text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-1 disabled:cursor-not-allowed disabled:opacity-70;
  }

  .btn-primary {
    @apply border-sky-700/25 bg-sky-600 text-white/90 hover:border-sky-800 hover:text-white focus-visible:ring-sky-800;
  }

  .btn-secondary {
    @apply border-slate-300/25 bg-slate-200 text-slate-700/90 hover:border-slate-400 hover:text-slate-700 focus-visible:ring-slate-400;
  }

  .btn-danger {
    @apply border-rose-600/25 bg-rose-500 text-white/90 hover:border-rose-700 hover:text-white focus-visible:ring-rose-700;
  }

  .field {
    @apply mb-3 block w-full appearance-none rounded border border-gray-200 bg-gray-200 px-4 py-3 text-left leading-tight text-gray-700 focus:border-gray-500 focus:bg-white focus:outline-none;
  }

  .field-label {
    @apply mb-2 block text-left text-xs font-bold tracking-wide text-slate-500 uppercase;
  }

  .icon-btn {
    @apply inline-flex h-6 w-6 items-center justify-center rounded text-lg leading-none text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-slate-400;
  }

  .spinner {
    @apply mr-2 inline-block h-3 w-3 animate-spin rounded-full border-2 border-white/40 border-t-white;
  }
</style>
