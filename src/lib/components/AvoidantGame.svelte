<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import init, { GameState, type GameOptions, type MapData } from "$lib/wasm/avoidant_wasm";
  import { Canvas, T } from "@threlte/core";
  import { OrbitControls } from "@threlte/extras";
  import { MOUSE } from "three";
  import Board from "$lib/components/board.svelte";
  import { MAP_HEIGHT, MAP_WIDTH } from "$lib/generated/shared-constants";
  import { m } from "$lib/paraglide/messages";
  import { getLocale, locales, setLocale } from "$lib/paraglide/runtime";
  import { generateMap } from "$lib/workers/mapgen-client";

  const PLAYER_NAME_STORAGE_KEY = "avoidant:playerName";
  const SIZE_PRESETS = { small: 80, medium: 160, large: 320 } as const;
  const MAP_MIN_X = 0;
  const MAP_MIN_Z = 0;
  const MAP_MAX_X = MAP_MIN_X + MAP_WIDTH;
  const MAP_MAX_Z = MAP_MIN_Z + MAP_HEIGHT;
  const MAP_CENTER_X = MAP_MIN_X + MAP_WIDTH / 2;
  const MAP_CENTER_Z = MAP_MIN_Z + MAP_HEIGHT / 2;

  const CAMERA_AZIMUTH_RAD = Math.PI / 4;
  const CAMERA_ELEVATION_RAD = Math.atan(0.5);
  const CAMERA_ORBIT_RADIUS = 240;

  const horizontalDistance = Math.cos(CAMERA_ELEVATION_RAD) * CAMERA_ORBIT_RADIUS;
  const cameraPosition: [number, number, number] = [
    MAP_CENTER_X + horizontalDistance * Math.sin(CAMERA_AZIMUTH_RAD),
    Math.sin(CAMERA_ELEVATION_RAD) * CAMERA_ORBIT_RADIUS,
    MAP_CENTER_Z + horizontalDistance * Math.cos(CAMERA_AZIMUTH_RAD),
  ];

  type SizePreset = keyof typeof SIZE_PRESETS | "custom";
  type Locale = (typeof locales)[number];
  type AvoidantGameProps = {
    relayServers?: string[];
  };

  let { relayServers = [] }: AvoidantGameProps = $props();

  const localeLabels: Record<Locale, string> = {
    en: "English",
    bn: "বাংলা",
  };

  function normalizeRelayServerList(input: string[]): string[] {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const seen = new Set<string>();
    const urls: string[] = [];
    for (const item of input) {
      const trimmed = item.trim();
      if (!trimmed || seen.has(trimmed)) {
        continue;
      }
      seen.add(trimmed);
      urls.push(trimmed);
    }
    return urls;
  }

  function parseRelayServersInput(input: string): string[] {
    return normalizeRelayServerList(input.split(/\r?\n/));
  }

  let status: string | undefined = $state(undefined);
  let gameState = $state<GameState | undefined>(undefined);
  let terrain = $state<MapData["terrain"] | undefined>(undefined);
  let numCellsInput = $state(SIZE_PRESETS.medium);
  let voidFractionInput = $state(0.15625);
  let rngSeedInput = $state(0);
  let playerNameInput = $state(
    typeof window !== "undefined"
      ? (localStorage.getItem(PLAYER_NAME_STORAGE_KEY) ?? m.default_player_name())
      : m.default_player_name(),
  );
  let isTutorialMode = $state(false);
  let tutorialText = $state("");
  let setupMode: "host" | "join" | undefined = $state(undefined);
  let sizePreset = $state<SizePreset>("medium");
  let relayServersInput = $state("");
  let relayServersInitialized = false;
  let hasRelayServersConfigured = $derived(parseRelayServersInput(relayServersInput).length > 0);
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
  let numSafeUnexploredCells = $derived(
    $score ? Math.max(0, $score.totalCells - $score.voidTotal - $score.safeExplored) : 0,
  );
  let isScoreBreakdownHovered = $state(false);
  let isScoreBreakdownPinned = $state(false);
  let showScoreBreakdown = $derived(isScoreBreakdownHovered || isScoreBreakdownPinned);
  let connectedPeerCount = $derived(
    ($networkSnapshot?.peers ?? []).filter((peer) => peer.isConnected).length,
  );
  let cameraFov = $state(50);

  function toggleScoreBreakdown() {
    isScoreBreakdownPinned = !isScoreBreakdownPinned;
  }

  function handleScoreSummaryKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleScoreBreakdown();
    }
  }

  function calculateCoverFov(
    viewportWidth: number,
    viewportHeight: number,
    elevationMin: number,
    elevationMax: number,
  ): number {
    const forwardX = -Math.cos(CAMERA_ELEVATION_RAD) * Math.sin(CAMERA_AZIMUTH_RAD);
    const forwardY = -Math.sin(CAMERA_ELEVATION_RAD);
    const forwardZ = -Math.cos(CAMERA_ELEVATION_RAD) * Math.cos(CAMERA_AZIMUTH_RAD);

    const rightX = -forwardZ;
    const rightZ = forwardX;
    const rightLength = Math.hypot(rightX, rightZ);
    const normalizedRightX = rightX / rightLength;
    const normalizedRightZ = rightZ / rightLength;

    const upX = normalizedRightZ * forwardY;
    const upY = -(normalizedRightX * forwardZ - normalizedRightZ * forwardX);
    const upZ = -normalizedRightX * forwardY;

    const yMin = Math.min(elevationMin, elevationMax);
    const yMax = Math.max(elevationMin, elevationMax);
    const corners = [
      [MAP_MIN_X, yMin, MAP_MIN_Z],
      [MAP_MIN_X, yMin, MAP_MAX_Z],
      [MAP_MAX_X, yMin, MAP_MIN_Z],
      [MAP_MAX_X, yMin, MAP_MAX_Z],
      [MAP_MIN_X, yMax, MAP_MIN_Z],
      [MAP_MIN_X, yMax, MAP_MAX_Z],
      [MAP_MAX_X, yMax, MAP_MIN_Z],
      [MAP_MAX_X, yMax, MAP_MAX_Z],
    ];

    let minRight = Infinity;
    let maxRight = -Infinity;
    let minUp = Infinity;
    let maxUp = -Infinity;

    for (const [x, y, z] of corners) {
      const projectedRight = x * normalizedRightX + z * normalizedRightZ;
      const projectedUp = x * upX + y * upY + z * upZ;
      minRight = Math.min(minRight, projectedRight);
      maxRight = Math.max(maxRight, projectedRight);
      minUp = Math.min(minUp, projectedUp);
      maxUp = Math.max(maxUp, projectedUp);
    }

    const projectedWidth = Math.max(1e-6, maxRight - minRight);
    const projectedHeight = Math.max(1e-6, maxUp - minUp);

    const aspect = Math.max(1e-6, viewportWidth / viewportHeight);

    // For perspective cover framing, choose the smaller of width/height fitting FOVs so the map fills the viewport similarly to the previous orthographic cover zoom.
    const verticalFovForHeight = 2 * Math.atan(projectedHeight / (2 * CAMERA_ORBIT_RADIUS));
    const verticalFovForWidth = 2 * Math.atan(projectedWidth / (2 * CAMERA_ORBIT_RADIUS * aspect));
    const coverFovRadians = Math.min(verticalFovForHeight, verticalFovForWidth) * 0.99;
    const coverFovDegrees = (coverFovRadians * 180) / Math.PI;
    return Math.min(120, Math.max(10, coverFovDegrees));
  }

  function setInitialCameraFov() {
    if (typeof window !== "undefined") {
      const viewportWidth = Math.max(1, window.innerWidth);
      const viewportHeight = Math.max(1, window.innerHeight);
      const elevationMin = gameState?.elevationMin ?? 0;
      const elevationMax = gameState?.elevationMax ?? 0;
      cameraFov = calculateCoverFov(viewportWidth, viewportHeight, elevationMin, elevationMax);
    }
  }

  onMount(() => {
    rngSeedInput = Math.floor(Date.now() / 1000);

    const initializeWasm = async () => {
      try {
        status = m.status_loading();
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
        status = m.status_initialization_failed();
        console.error("Failed to initialize wasm", error);
      }
    };

    void initializeWasm();
  });

  $effect(() => {
    if (relayServersInitialized) {
      return;
    }

    relayServersInput = normalizeRelayServerList(relayServers).join("\n");
    relayServersInitialized = true;
  });

  $effect(() => {
    if (!gameState) return;
    setInitialCameraFov();
  });

  $effect(() => {
    if (gameState) return;
    isScoreBreakdownHovered = false;
    isScoreBreakdownPinned = false;
  });

  $effect(() => {
    if (typeof window !== "undefined") {
      try {
        localStorage.setItem(PLAYER_NAME_STORAGE_KEY, playerNameInput);
      } catch (error) {
        console.warn("Failed to persist player name", error);
      }
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

  type PresetIcon = {
    value: SizePreset;
    cells: string[] | null;
    readonly label: string;
  };

  const presetIcons: PresetIcon[] = [
    {
      value: "small",
      cells: hexCells(11),
      get label() {
        return m.preset_small();
      },
    },
    {
      value: "medium",
      cells: hexCells(7),
      get label() {
        return m.preset_medium();
      },
    },
    {
      value: "large",
      cells: hexCells(4.5),
      get label() {
        return m.preset_large();
      },
    },
    {
      value: "custom",
      cells: null,
      get label() {
        return m.preset_custom();
      },
    },
  ];

  async function startGame() {
    try {
      const resolvedNumCells =
        sizePreset === "custom" ? $state.snapshot(numCellsInput) : SIZE_PRESETS[sizePreset];
      const voidFraction = sizePreset === "custom" ? $state.snapshot(voidFractionInput) : 0.15625;
      const relayUrls = parseRelayServersInput($state.snapshot(relayServersInput));
      const options: GameOptions = {
        elevationMax: 6.0,
        elevationMin: 0.0,
        numCells: resolvedNumCells,
        relayUrls: relayUrls.length > 0 ? relayUrls : undefined,
        rngSeed: $state.snapshot(rngSeedInput),
        spikiness: 0.8,
        voidFraction: voidFraction,
      };
      status = m.status_generating_map();
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
      status = m.status_joining_game();
      const ticket = extractTicket(ticketInput);
      if (!ticket) {
        throw new Error(m.error_join_ticket_required());
      }
      let options: GameOptions;
      try {
        options = GameState.optionsFromTicket(ticket);
      } catch (error) {
        console.error("Failed to parse ticket", error);
        throw new Error(m.error_join_ticket_invalid(), { cause: error });
      }
      const nextGameState = new GameState(options);
      status = m.status_generating_map();
      const generated = await generateMap(options);
      nextGameState.applyMapCells(generated.cells);
      status = m.status_joining_game();
      try {
        await nextGameState.joinAsPeer(ticket, playerNameInput);
      } catch (error) {
        console.error("Failed to join peer", error);
        try {
          nextGameState.free();
        } catch (freeError) {
          console.error("Failed to release game state", freeError);
        }
        throw new Error(m.error_join_connect_failed(), { cause: error });
      }
      gameState = nextGameState;
      terrain = generated.terrain;
      inviteTicket = "";
      succeeded = true;
    } catch (error) {
      console.error("Failed to join game", error);
      joinError = error instanceof Error ? error.message : m.error_join_failed();
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
    rngSeedInput = Math.floor(Date.now() / 1000);
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
          <div
            class="relative text-sm text-slate-600"
            role="group"
            aria-label={m.aria_score_summary()}
            onmouseenter={() => (isScoreBreakdownHovered = true)}
            onmouseleave={() => (isScoreBreakdownHovered = false)}
          >
            <div
              class="score-summary"
              role="button"
              tabindex="0"
              aria-expanded={showScoreBreakdown}
              aria-controls="score-breakdown"
              onclick={toggleScoreBreakdown}
              onfocus={() => (isScoreBreakdownHovered = true)}
              onblur={() => (isScoreBreakdownHovered = false)}
              onkeydown={handleScoreSummaryKeydown}
            >
              {m.label_score()}: <strong class="text-slate-200!">{Math.round($score.score)}</strong>
              <span class="opacity-70">({Math.round($score.efficiency * 100)}%)</span>
              {#if $score.completed}
                <span class="font-semibold text-emerald-600!">{m.text_avoided()}</span>
              {:else}
                <span class="opacity-80">
                  {m.label_safe_cells_remaining({ count: numSafeUnexploredCells })}:
                  <strong class="text-slate-200!">{numSafeUnexploredCells}</strong>
                </span>
              {/if}
            </div>

            {#if showScoreBreakdown}
              <div id="score-breakdown" class="score-breakdown" role="status">
                {#if $score.streak > 1}<p>
                    <span class="opacity-70"
                      >×{(1 + Math.min($score.streak, 10) * 0.1).toFixed(1)}
                      {m.label_streak()}</span
                    >
                  </p>
                {/if}
                <p class="text-emerald-600!">
                  {m.label_safe_cells_explored({ count: $score.safeExplored })}:
                  <strong>{$score.safeExplored}</strong>
                </p>
                <p class="text-rose-500!">
                  {m.label_voids_discovered()}: <strong>{$score.voidExplored}</strong>
                </p>
              </div>
            {/if}
          </div>
        {/if}
        <div class="ml-auto text-sm text-slate-200!">
          {#if connectedPeerCount > 0}
            {m.label_players({ count: connectedPeerCount + 1 })}:
            <strong>{connectedPeerCount + 1}</strong>
          {/if}
        </div>
        <div class="flex gap-2">
          {#if !isTutorialMode && hasRelayServersConfigured && ($score?.safeExplored ?? 0) + ($score?.voidExplored ?? 0) === 0}
            <button
              class="btn btn-primary"
              type="button"
              onclick={generateInvite}
              disabled={isGeneratingInvite}
            >
              {#if isGeneratingInvite}
                <span class="spinner" aria-hidden="true"></span>
                <span>{m.status_preparing()}</span>
              {:else}
                {m.action_invite()}
              {/if}
            </button>
          {/if}
          <button class="btn btn-danger" type="button" onclick={exitGame}>{m.action_exit()}</button>
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
              <label class="field-label" for="player-name">{m.field_player_name()}</label>
              <input class="field" id="player-name" type="text" bind:value={playerNameInput} />
            </div>
            <div class="mb-4 w-full px-3">
              <span class="field-label">{m.field_map_size()}</span>
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
                          ? m.cells_count({ count: numCellsInput })
                          : ""
                        : m.cells_count({ count: SIZE_PRESETS[preset.value] })}
                    </span>
                  </button>
                {/each}
              </div>
            </div>
            {#if sizePreset === "custom"}
              <div class="w-full px-3" transition:slide={{ duration: 180 }}>
                <div class="-mx-3 flex flex-wrap">
                  <div class="mb-6 w-full px-3 md:mb-0 md:w-1/2">
                    <label class="field-label" for="grid-first-name">{m.field_size()}</label>
                    <input
                      class="field"
                      id="grid-first-name"
                      type="number"
                      inputmode="numeric"
                      bind:value={numCellsInput}
                      min="32"
                      max="5000"
                      step="1"
                    />
                  </div>
                  <div class="w-full px-3 md:w-1/2">
                    <label class="field-label" for="grid-last-name">{m.field_void_fraction()}</label
                    >
                    <input
                      class="field"
                      id="grid-last-name"
                      type="number"
                      inputmode="numeric"
                      bind:value={voidFractionInput}
                      max="0.999"
                      min="0"
                      step="0.001"
                    />
                  </div>
                </div>
              </div>
            {/if}
            <div class="mb-4 w-full px-3">
              <details class="advanced-settings">
                <summary class="advanced-settings-summary">{m.label_advanced_settings()}</summary>
                <div class="advanced-settings-content">
                  <label class="field-label" for="relay-servers"
                    ><a
                      href="https://docs.iroh.computer/deployment/dedicated-infrastructure"
                      target="_blank"
                      rel="noopener noreferrer">{m.field_relay_servers()}</a
                    ></label
                  >
                  <textarea
                    class="field field-textarea mb-1"
                    id="relay-servers"
                    bind:value={relayServersInput}
                    rows="4"
                    spellcheck="false"
                  ></textarea>
                  <p class="field-help">{m.text_relay_servers_hint()}</p>
                  <div class="mt-6 w-full">
                    <label class="field-label" for="grid-last-name">{m.field_seed()}</label>
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
              </details>
            </div>
            <!-- <div class="checkbox-field mb-4 flex w-full items-center gap-2 px-3">
              <label class="field-label mb-0!" for="tutorial-mode">Tutorial Mode</label>
              <input id="tutorial-mode" type="checkbox" bind:checked={isTutorialMode} />
            </div> -->
          </div>
          <div class="flex flex-wrap justify-center gap-2">
            <button class="btn btn-secondary" type="button" onclick={() => (setupMode = undefined)}>
              {m.action_back()}
            </button>
            <button class="btn btn-primary" type="submit">{m.action_start()}</button>
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
              <label class="field-label" for="join-player-name">{m.field_player_name()}</label>
              <input class="field" id="join-player-name" type="text" bind:value={playerNameInput} />
            </div>
            <div class="mb-6 w-full px-3 md:mb-0">
              <label class="field-label" for="grid-first-name"
                >{m.field_ticket_or_invitation_url()}</label
              >
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
              {m.action_back()}
            </button>
            <button class="btn btn-primary" type="submit">{m.action_join()}</button>
          </div>
        </form>
      {:else}
        <div class="mt-4 flex flex-wrap justify-center gap-2" transition:slide={{ duration: 220 }}>
          <button class="btn btn-primary" type="button" onclick={() => (setupMode = "host")}>
            {m.action_new_game()}
          </button>
          <button class="btn btn-primary" type="button" onclick={() => (setupMode = "join")}>
            {m.action_join_game()}
          </button>
        </div>
        <div class="mt-4 flex justify-center" transition:slide={{ duration: 180 }}>
          <label class="sr-only" for="language-select">Language</label>
          <select
            id="language-select"
            class="locale-select min-w-1/2"
            value={getLocale()}
            aria-label="Language"
            onchange={(event) => {
              setLocale((event.currentTarget as HTMLSelectElement).value as Locale);
            }}
          >
            {#each locales as locale (locale)}
              <option value={locale}
                >{localeLabels[locale] ? `${localeLabels[locale]} (${locale})` : locale}</option
              >
            {/each}
          </select>
        </div>
      {/if}
    {:else if inviteTicket}
      <div class="mt-3" transition:slide={{ duration: 200 }}>
        <div class="mb-2 flex items-center justify-between gap-2">
          <label class="field-label mb-0" for="invite-ticket">{m.field_invitation_url()}</label>
          <button
            class="icon-btn"
            type="button"
            aria-label={m.aria_close_invitation()}
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
          <button class="btn btn-secondary" type="button" onclick={copyInviteTicket}>
            {m.action_copy()}
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>

{#if gameState}
  <div id="game-canvas-container" style="height: 100vh; width: 100%;">
    <Canvas colorSpace="srgb-linear">
      <T.PerspectiveCamera
        makeDefault
        fov={cameraFov}
        near={0.1}
        far={1000}
        position={cameraPosition}
      />
      <Board bind:gameState {terrain} />
      <OrbitControls
        enableDamping
        enablePan={true}
        enableZoom={true}
        enableRotate={true}
        minPolarAngle={0}
        maxPolarAngle={Math.PI / 2}
        target={[MAP_CENTER_X, 0, MAP_CENTER_Z]}
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
