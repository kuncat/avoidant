<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import init, { GameState, type GameOptions, type MapData } from "wasm-pkg";
  import { Canvas, T } from "@threlte/core";
  import { OrbitControls } from "@threlte/extras";
  import { MOUSE } from "three";
  import Board from "$lib/components/board.svelte";
  import { generateMap } from "$lib/workers/mapgen-client";

  const PLAYER_NAME_STORAGE_KEY = "avoidant:playerName";
  const SIZE_PRESETS = { small: 80, medium: 160, large: 320 } as const;
  type SizePreset = keyof typeof SIZE_PRESETS | "custom";

  let status: string | undefined = $state(undefined);
  let gameState = $state<GameState | undefined>(undefined);
  let terrain = $state<MapData["terrain"] | undefined>(undefined);
  let numCellsInput = $state(SIZE_PRESETS.medium);
  let rngSeedInput = $state(0);
  let playerNameInput = $state(localStorage.getItem(PLAYER_NAME_STORAGE_KEY) ?? "Player");
  let isTutorialMode = $state(false);
  let tutorialText = $state("");
  let setupMode: "host" | "join" | undefined = $state(undefined);
  let sizePreset = $state<SizePreset>("medium");
  let ticketInput = $state("");
  let joinError: string | undefined = $state(undefined);
  let inviteTicket = $state("");
  let inviteUrl = $derived(
    inviteTicket && typeof window !== "undefined"
      ? `${window.location.origin}${window.location.pathname}?ticket=${encodeURIComponent(inviteTicket)}`
      : "",
  );
  let isGeneratingInvite = $state(false);
  let networkSnapshot = $derived(gameState?.networkSnapshot);
  let score = $derived(gameState?.score);
  let connectedPeerCount = $derived(
    ($networkSnapshot?.peers ?? []).filter((peer) => peer.isConnected).length,
  );

  onMount(() => {
    rngSeedInput = Math.floor(Date.now() / 1000);

    const initializeWasm = async () => {
      try {
        status = "Loading...";
        await init();
        status = undefined;

        const sharedTicket = new URLSearchParams(window.location.search).get("ticket");
        if (sharedTicket) {
          // Clear the ticket from the visible URL so a refresh doesn't re-trigger join.
          window.history.replaceState({}, "", `${window.location.pathname}${window.location.hash}`);
          ticketInput = sharedTicket;
          setupMode = "join";
        }
      } catch (error) {
        status = "Failed to initialize wasm.";
        console.error("Failed to initialize wasm", error);
      }
    };

    void initializeWasm();
  });

  $effect(() => {
    try {
      localStorage.setItem(PLAYER_NAME_STORAGE_KEY, playerNameInput);
    } catch (error) {
      console.warn("Failed to persist player name", error);
    }
  });

  function hexCells(radius: number): string[] {
    const cells: string[] = [];
    const dx = radius * Math.sqrt(3);
    const dy = radius * 1.5;
    const size = 40;
    for (let row = 0; ; row++) {
      const cy = row * dy + radius;
      if (cy - radius > size) break;
      const xOffset = (row % 2) * (dx / 2);
      for (let col = 0; ; col++) {
        const cx = col * dx + xOffset + dx / 2;
        if (cx - dx / 2 > size) break;
        cells.push(
          [
            [cx, cy - radius],
            [cx + dx / 2, cy - radius / 2],
            [cx + dx / 2, cy + radius / 2],
            [cx, cy + radius],
            [cx - dx / 2, cy + radius / 2],
            [cx - dx / 2, cy - radius / 2],
          ]
            .map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`)
            .join(" "),
        );
      }
    }
    return cells;
  }

  const presetIcons: { value: SizePreset; label: string; cells: string[] | null }[] = [
    { value: "small", label: "Small", cells: hexCells(11) },
    { value: "medium", label: "Medium", cells: hexCells(7) },
    { value: "large", label: "Large", cells: hexCells(4.5) },
    { value: "custom", label: "Custom", cells: null },
  ];

  async function startGame() {
    try {
      const resolvedNumCells =
        sizePreset === "custom" ? $state.snapshot(numCellsInput) : SIZE_PRESETS[sizePreset];
      const options: GameOptions = {
        elevationMax: 6.0,
        elevationMin: 0.0,
        numCells: resolvedNumCells,
        rngSeed: $state.snapshot(rngSeedInput),
        spikiness: 0.8,
      };
      status = "Generating map...";
      gameState = new GameState(options);
      const generated = await generateMap(options);
      gameState.applyMapCells(generated.cells);
      terrain = generated.terrain;
      inviteTicket = "";
    } catch (error) {
      console.error("Failed to start game", error);
    } finally {
      setupMode = undefined;
      status = undefined;
    }
  }

  function extractTicket(input: string): string {
    const trimmed = input.trim();
    if (!trimmed) {
      return "";
    }
    if (/^https?:\/\//i.test(trimmed)) {
      try {
        return new URL(trimmed).searchParams.get("ticket")?.trim() ?? "";
      } catch {
        return "";
      }
    }
    return trimmed;
  }

  async function joinGame() {
    joinError = undefined;
    let succeeded = false;
    try {
      status = "Joining game...";
      const ticket = extractTicket(ticketInput);
      if (!ticket) {
        throw new Error("Enter a ticket or invitation URL to join a game.");
      }
      let options: GameOptions;
      try {
        options = GameState.optionsFromTicket(ticket);
      } catch (error) {
        console.error("Failed to parse ticket", error);
        throw new Error("That invitation link or ticket is not valid.", { cause: error });
      }
      const nextGameState = new GameState(options);
      status = "Generating map...";
      const generated = await generateMap(options);
      nextGameState.applyMapCells(generated.cells);
      status = "Joining game...";
      try {
        await nextGameState.joinAsPeer(ticket, playerNameInput);
      } catch (error) {
        console.error("Failed to join peer", error);
        try {
          nextGameState.free();
        } catch (freeError) {
          console.error("Failed to release game state", freeError);
        }
        throw new Error(
          "Could not connect to the game. The invitation may have expired or the host may be offline.",
          { cause: error },
        );
      }
      gameState = nextGameState;
      terrain = generated.terrain;
      inviteTicket = "";
      succeeded = true;
    } catch (error) {
      console.error("Failed to join game", error);
      joinError = error instanceof Error ? error.message : "Failed to join the game.";
    } finally {
      status = undefined;
      if (succeeded) {
        setupMode = undefined;
        ticketInput = "";
      } else {
        setupMode = "join";
      }
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
    joinError = undefined;
    status = undefined;
  }

  async function copyInviteTicket() {
    if (!inviteUrl) {
      return;
    }
    try {
      await navigator.clipboard.writeText(inviteUrl);
    } catch (error) {
      console.error("Failed to copy invitation URL", error);
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
  oncontextmenu={(event) => event.preventDefault()}
  role="application"
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
        {#if $score}
          <div class="text-sm text-slate-600">
            Score: <strong class="text-slate-200!">{Math.round($score.score)}</strong>
            <span class="opacity-70">({Math.round($score.efficiency * 100)}%)</span>
            {#if $score.streak > 1}
              <span class="ml-2 opacity-70"
                >×{(1 + Math.min($score.streak, 10) * 0.1).toFixed(1)} streak</span
              >
            {/if}
            {#if $score.completed}
              <span class="ml-2 font-semibold text-emerald-600!">Avoided!</span>
            {/if}
          </div>
        {/if}
        <div class="ml-auto text-sm text-slate-200!">
          {#if connectedPeerCount > 0}
            Players: <strong>{connectedPeerCount + 1}</strong>
          {/if}
        </div>
        <div class="flex gap-2">
          {#if !$score?.completed}
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
          {/if}
          <button class="btn btn-danger" type="button" onclick={exitGame}>Exit</button>
        </div>
      {/if}
    </div>

    {#if status}
      <p class="mt-2 text-sm text-slate-600!">{status}</p>
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
            <div class="mb-4 w-full px-3">
              <label class="field-label" for="player-name">Player Name</label>
              <input class="field" id="player-name" type="text" bind:value={playerNameInput} />
            </div>
            <div class="mb-4 w-full px-3">
              <span class="field-label">Map Size</span>
              <div class="grid grid-cols-4 gap-2">
                {#each presetIcons as preset (preset.value)}
                  <button
                    type="button"
                    class="preset-btn"
                    class:preset-btn-active={sizePreset === preset.value}
                    aria-pressed={sizePreset === preset.value}
                    onclick={() => (sizePreset = preset.value)}
                  >
                    <span class="preset-icon">
                      {#if preset.cells}
                        <svg
                          viewBox="0 0 40 40"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="1"
                          stroke-linejoin="round"
                          aria-hidden="true"
                        >
                          {#each preset.cells as points, i (i)}
                            <polygon {points} />
                          {/each}
                        </svg>
                      {:else}
                        <svg
                          viewBox="0 0 40 40"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="2.5"
                          stroke-linecap="round"
                          aria-hidden="true"
                        >
                          <line x1="8" y1="12" x2="32" y2="12" />
                          <circle cx="15" cy="12" r="3.5" fill="currentColor" />
                          <line x1="8" y1="20" x2="32" y2="20" />
                          <circle cx="25" cy="20" r="3.5" fill="currentColor" />
                          <line x1="8" y1="28" x2="32" y2="28" />
                          <circle cx="19" cy="28" r="3.5" fill="currentColor" />
                        </svg>
                      {/if}
                    </span>
                    <span class="preset-label">{preset.label}</span>
                    <span class="preset-count">
                      {preset.value === "custom"
                        ? sizePreset === "custom"
                          ? `${numCellsInput} cells`
                          : ""
                        : `${SIZE_PRESETS[preset.value]} cells`}
                    </span>
                  </button>
                {/each}
              </div>
            </div>
            {#if sizePreset === "custom"}
              <div class="w-full px-3" transition:slide={{ duration: 180 }}>
                <div class="-mx-3 flex flex-wrap">
                  <div class="mb-6 w-full px-3 md:mb-0 md:w-1/2">
                    <label class="field-label" for="grid-first-name">Size</label>
                    <input
                      class="field"
                      id="grid-first-name"
                      type="number"
                      inputmode="numeric"
                      bind:value={numCellsInput}
                      min="32"
                      max="262144"
                      step="1"
                    />
                  </div>
                  <div class="w-full px-3 md:w-1/2">
                    <label class="field-label" for="grid-last-name">Seed</label>
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
              </div>
            {/if}
            <!-- <div class="checkbox-field mb-4 flex w-full items-center gap-2 px-3">
              <label class="field-label mb-0!" for="tutorial-mode">Tutorial Mode</label>
              <input id="tutorial-mode" type="checkbox" bind:checked={isTutorialMode} />
            </div> -->
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
              <label class="field-label" for="grid-first-name">Ticket or Invitation URL</label>
              <input
                class="field"
                id="grid-first-name"
                type="text"
                bind:value={ticketInput}
                oninput={() => (joinError = undefined)}
              />
            </div>
            {#if joinError}
              <div class="w-full px-3" transition:slide={{ duration: 150 }}>
                <p class="join-error" role="alert">{joinError}</p>
              </div>
            {/if}
          </div>
          <div class="flex flex-wrap justify-center gap-2">
            <button
              class="btn btn-secondary"
              type="button"
              onclick={() => {
                setupMode = undefined;
                joinError = undefined;
              }}
            >
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
          <label class="field-label mb-0" for="invite-ticket">Invitation URL</label>
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
            value={inviteUrl}
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
      <Board bind:gameState {terrain} />
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

{#if isTutorialMode}
  <div class="absolute bottom-0 h-10 w-full px-4 text-slate-200">
    {tutorialText}
  </div>
{/if}

<style>
  @reference "../layout.css";

  h1 {
    @apply m-0;
  }

  .join-error {
    @apply rounded border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700;
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
      @apply h-6 px-2 py-0;
    }

    .field {
      @apply mb-0 h-6 border-slate-600 bg-slate-700 px-2 py-0 text-sm text-slate-100 placeholder:text-slate-400 focus:bg-slate-600;
    }
  }

  .panel-title {
    @apply transition-all duration-500 ease-out;
  }

  .btn {
    @apply inline-flex items-center justify-center rounded border px-3 py-2 text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-1 disabled:cursor-not-allowed disabled:opacity-70;
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

  .checkbox-field {
    @apply flex items-center gap-2;

    input[type="checkbox"] {
      @apply h-5 w-5 appearance-none rounded border border-gray-300 bg-gray-200 align-top transition-colors checked:border-sky-600 checked:bg-sky-500 focus:ring-0 focus:ring-offset-0 focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-400 focus-visible:ring-offset-1;
    }
    input[type="checkbox"]:checked {
      @apply border-sky-600 bg-sky-500;
    }
  }

  .icon-btn {
    @apply inline-flex h-6 w-6 items-center justify-center rounded text-lg leading-none text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-slate-400;
  }

  .preset-btn {
    @apply flex aspect-square w-full flex-col items-center justify-center gap-1 rounded border border-slate-300 bg-slate-100 p-2 text-slate-600 transition-colors hover:border-slate-400 hover:bg-slate-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-400;
  }

  .preset-btn-active {
    @apply border-sky-500 bg-sky-100 text-sky-800 hover:border-sky-600 hover:bg-sky-100;
  }

  .preset-icon {
    @apply block h-8 w-8 text-slate-500;
  }

  .preset-btn-active .preset-icon {
    @apply text-sky-600;
  }

  .preset-icon svg {
    @apply h-full w-full;
  }

  .preset-label {
    @apply text-xs font-semibold tracking-wide uppercase;
  }

  .preset-count {
    @apply text-[10px] leading-none text-slate-500;
  }

  .preset-btn-active .preset-count {
    @apply text-sky-700;
  }

  .spinner {
    @apply mr-2 inline-block h-3 w-3 animate-spin rounded-full border-2 border-white/40 border-t-white;
  }
</style>
