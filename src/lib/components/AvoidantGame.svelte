<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { SvelteSet } from "svelte/reactivity";
  import init, {
    GameState,
    type GameOptions,
    type MapData,
    type MapShape,
  } from "$lib/wasm/avoidant_wasm";
  import { Canvas, T } from "@threlte/core";
  import { OrbitControls } from "@threlte/extras";
  import { MOUSE, TOUCH } from "three";
  import Board from "$lib/components/board.svelte";
  import { m } from "$lib/paraglide/messages";
  import { getLocale, locales, setLocale } from "$lib/paraglide/runtime";
  import { generateMap } from "$lib/workers/mapgen-client";
  import { TutorialState } from "$lib/tutorial.svelte";

  const PLAYER_NAME_STORAGE_KEY = "avoidant:playerName";
  const SIZE_PRESETS = { small: 80, medium: 160, large: 320 } as const;

  // Default mapshape parameters used when a preset is picked or no custom value has been entered yet.
  const DEFAULT_RADIUS = 50;

  type ShapeKind = MapShape["kind"];
  const SHAPE_KINDS: ShapeKind[] = [
    "flat",
    "icosahedron",
    "spheroid",
    "geodesicIcosphere",
    "tetrahedron",
    "cube",
    "octahedron",
    "dodecahedron",
  ];

  const CAMERA_AZIMUTH_RAD = Math.PI / 4;
  const CAMERA_ELEVATION_RAD = Math.atan(0.5);
  /// Camera orbit distance, derived from the generated map's bounds radius so the camera frames any 3D shape consistently.
  const CAMERA_ORBIT_MULTIPLIER = 2.5;

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
  let surfaceArea = $state(0);
  let isFlatMap = $state(false);
  let boundsRadius = $state(DEFAULT_RADIUS);
  let numCellsInput = $state(SIZE_PRESETS.medium);
  let voidFractionInput = $state(0.15625);
  let spikinessInput = $state(0.8);
  let rngSeedInput = $state(0);
  // Shape selection state. The shape kind drives which numeric inputs are shown; each per-shape numeric value persists independently so switching back and forth doesn't reset the user's edits.
  let shapeKindInput = $state<ShapeKind>("icosahedron");
  let playerNameInput = $state(
    typeof window !== "undefined"
      ? (localStorage.getItem(PLAYER_NAME_STORAGE_KEY) ?? m.default_player_name())
      : m.default_player_name(),
  );
  let isTutorialMode = $state(false);
  let tutorial = $state<TutorialState | undefined>(undefined);
  let exploredCellsSeen = new SvelteSet<number>();
  let pendingTutorialClick = $state<number | undefined>(undefined);
  let setupMode: "host" | "join" | undefined = $state(undefined);
  let sizePreset = $state<SizePreset>("medium");
  let isAdvancedSettingsOpen = $state(false);
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
  let cellMetadataStore = $derived(gameState?.cellMetadata);
  let cellsStore = $derived(gameState?.cells);
  let pulsesStore = $derived(gameState?.uiState?.pulses);
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

  let selectedCellCount = $derived(
    sizePreset === "custom" ? numCellsInput : SIZE_PRESETS[sizePreset],
  );

  let shapeSubdivisionsInput = $derived(
    Math.max(
      0,
      Math.min(2, Math.floor(Math.log(Math.max(1, selectedCellCount / 80)) / Math.log(4))),
    ),
  );

  let faceCount = $derived(
    {
      tetrahedron: 4,
      cube: 6,
      octahedron: 8,
      dodecahedron: 12,
      icosahedron: 20,
      geodesicIcosphere: 20 * 4 ** shapeSubdivisionsInput,
      flat: 0,
      spheroid: 0,
    }[shapeKindInput],
  );
  let usesFaceDensity = $derived(faceCount > 0);

  function resolveShape(): MapShape {
    const radius = DEFAULT_RADIUS * Math.sqrt(selectedCellCount / SIZE_PRESETS.medium);
    switch (shapeKindInput) {
      case "spheroid":
        return {
          kind: "spheroid",
          radiusX: radius * 1.2,
          radiusY: radius * 0.8,
          radiusZ: radius,
        };
      case "geodesicIcosphere":
        return {
          kind: "geodesicIcosphere",
          radius,
          subdivisions: shapeSubdivisionsInput,
        };
      default:
        return { kind: shapeKindInput, radius };
    }
  }

  let cameraOrbitRadius = $derived(Math.max(1, boundsRadius) * CAMERA_ORBIT_MULTIPLIER);
  let cameraPosition = $derived<[number, number, number]>(
    isFlatMap
      ? [0, cameraOrbitRadius, 0.0001]
      : [
          Math.cos(CAMERA_ELEVATION_RAD) * Math.sin(CAMERA_AZIMUTH_RAD) * cameraOrbitRadius,
          Math.sin(CAMERA_ELEVATION_RAD) * cameraOrbitRadius,
          Math.cos(CAMERA_ELEVATION_RAD) * Math.cos(CAMERA_AZIMUTH_RAD) * cameraOrbitRadius,
        ],
  );

  function toggleScoreBreakdown() {
    isScoreBreakdownPinned = !isScoreBreakdownPinned;
  }

  function handleScoreSummaryKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleScoreBreakdown();
    }
  }

  /**
   * Pick a perspective FOV so the bounding sphere of the generated shape (including the maximum elevation displacement) fits the viewport on both axes, with a small margin so cells near the silhouette aren't clipped.
   */
  function calculateCoverFov(
    viewportWidth: number,
    viewportHeight: number,
    contentRadius: number,
    cameraDistance: number,
  ): number {
    const aspect = Math.max(1e-6, viewportWidth / viewportHeight);
    // Vertical FOV that exactly fits the bounding sphere's diameter on screen.
    const verticalFov = 2 * Math.atan(contentRadius / Math.max(1e-6, cameraDistance));
    // Adjust for the viewport aspect ratio so the sphere fits horizontally too.
    const verticalFovForWidth =
      2 * Math.atan(contentRadius / (Math.max(1e-6, cameraDistance) * aspect));
    const coverFovRadians = Math.max(verticalFov, verticalFovForWidth);
    const coverFovDegrees = (coverFovRadians * 180) / Math.PI;
    // Add 5% margin so silhouette cells aren't clipped at the viewport edges.
    return Math.min(120, Math.max(10, coverFovDegrees * 1.05));
  }

  function setInitialCameraFov() {
    if (typeof window !== "undefined") {
      const viewportWidth = Math.max(1, window.innerWidth);
      const viewportHeight = Math.max(1, window.innerHeight);
      const elevationMax = Math.abs(gameState?.elevationMax ?? 0);
      const contentRadius = boundsRadius + elevationMax;
      cameraFov = calculateCoverFov(
        viewportWidth,
        viewportHeight,
        contentRadius,
        cameraOrbitRadius,
      );
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

  $effect(() => {
    if (sizePreset === "custom") isTutorialMode = false;
  });

  $effect(() => {
    if (!tutorial || !$cellMetadataStore || !$cellsStore) return;
    const metadata = $cellMetadataStore;
    // Track every newly-explored cell immediately so we don't backfill later, but defer the tutorial state transition until any chord/flood pulses have settled — otherwise we'd announce the result and highlight the next safe cell mid-sweep.
    for (let i = 0; i < metadata.length; i++) {
      if (metadata[i].isExplored && !exploredCellsSeen.has(i)) {
        exploredCellsSeen.add(i);
      }
    }
    const pulsesActive = ($pulsesStore?.length ?? 0) > 0;
    const anyRevealing = metadata.some((entry) => entry.isRevealing);
    if (pulsesActive || anyRevealing) return;
    const clicked = pendingTutorialClick;
    if (clicked === undefined) return;
    pendingTutorialClick = undefined;
    tutorial.observeExplore(clicked, $cellsStore, metadata);
  });

  $effect(() => {
    if (!tutorial || !$score) return;
    if ($score.completed) {
      tutorial.observeWin($score.efficiency);
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
      const relayUrls = parseRelayServersInput($state.snapshot(relayServersInput));
      const options: GameOptions = {
        elevationMax: 6.0,
        elevationMin: 0.0,
        numCells: selectedCellCount,
        relayUrls: relayUrls.length > 0 ? relayUrls : undefined,
        rngSeed: $state.snapshot(rngSeedInput),
        shape: resolveShape(),
        spikiness: $state.snapshot(spikinessInput),
        voidFraction: $state.snapshot(voidFractionInput),
      };
      status = m.status_generating_map();
      isFlatMap = options.shape?.kind === "flat";
      gameState = new GameState(options);
      const generated = await generateMap(options);
      gameState.applyMapCells(generated.cells);
      terrain = generated.terrain;
      surfaceArea = generated.surfaceArea;
      boundsRadius = generated.boundsRadius;
      inviteTicket = "";
      exploredCellsSeen = new SvelteSet<number>();
      pendingTutorialClick = undefined;
      tutorial = isTutorialMode ? new TutorialState() : undefined;
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
      isFlatMap = options.shape?.kind === "flat";
      gameState = nextGameState;
      terrain = generated.terrain;
      surfaceArea = generated.surfaceArea;
      boundsRadius = generated.boundsRadius;
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
    tutorial = undefined;
    exploredCellsSeen = new SvelteSet<number>();
    pendingTutorialClick = undefined;
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
              {m.label_score()}:
              <strong class="text-slate-200!"
                >{Math.round($score.score).toLocaleString(getLocale())}</strong
              >
              <span class="opacity-70"
                >({$score.efficiency.toLocaleString(getLocale(), {
                  maximumFractionDigits: 0,
                  style: "percent",
                })})</span
              >
              {#if $score.completed}
                <span class="font-semibold text-emerald-600!">{m.text_avoided()}</span>
              {:else}
                <span class="opacity-80">
                  {m.label_safe_cells_remaining({
                    count: numSafeUnexploredCells.toLocaleString(getLocale()),
                  })}:
                  <strong class="text-slate-200!"
                    >{numSafeUnexploredCells.toLocaleString(getLocale())}</strong
                  >
                </span>
              {/if}
            </div>

            {#if showScoreBreakdown}
              <div id="score-breakdown" class="score-breakdown" role="status">
                {#if $score.streak > 1}<p>
                    <span class="opacity-70"
                      >×{(1 + Math.min($score.streak, 10) * 0.1).toLocaleString(getLocale(), {
                        maximumFractionDigits: 1,
                      })}
                      {m.label_streak()}</span
                    >
                  </p>
                {/if}
                <p class="text-emerald-600!">
                  {m.label_safe_cells_explored({
                    count: $score.safeExplored.toLocaleString(getLocale()),
                  })}:
                  <strong>{$score.safeExplored.toLocaleString(getLocale())}</strong>
                </p>
                <p class="text-rose-500!">
                  {m.label_voids_discovered()}:
                  <strong>{$score.voidExplored.toLocaleString(getLocale())}</strong>
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
              disabled={isGeneratingInvite || ($score?.safeExplored ?? 0) === 0}
              title={($score?.safeExplored ?? 0) === 0 ? m.text_invite_after_opening() : undefined}
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
              <label class="field-label" for="shape-select">{m.field_map_shape()}</label>
              <select id="shape-select" class="field" bind:value={shapeKindInput}>
                {#each SHAPE_KINDS as kind (kind)}
                  <option value={kind}>{m.shape_label({ kind })}</option>
                {/each}
              </select>
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
                        ? ""
                        : m.cells_count({ count: SIZE_PRESETS[preset.value] })}
                    </span>
                  </button>
                {/each}
              </div>
            </div>
            {#if sizePreset === "custom"}
              <div class="mb-4 w-full px-3">
                <label class="field-label" for="size-input">{m.field_size()}</label>

                <input
                  class="field"
                  id="size-input"
                  type="number"
                  required
                  min="32"
                  max="5000"
                  step="1"
                  bind:value={numCellsInput}
                />
              </div>
            {/if}
            <p class="field-help mb-4 w-full px-3">
              {usesFaceDensity
                ? m.text_density_summary({ faces: faceCount, count: selectedCellCount })
                : m.cells_count({ count: selectedCellCount })}
            </p>
            <div class="mb-4 w-full px-3">
              <div class="advanced-settings" class:advanced-settings-open={isAdvancedSettingsOpen}>
                <button
                  type="button"
                  class="advanced-settings-summary w-full"
                  aria-expanded={isAdvancedSettingsOpen}
                  aria-controls="advanced-settings-content"
                  onclick={() => (isAdvancedSettingsOpen = !isAdvancedSettingsOpen)}
                >
                  {m.label_advanced_settings()}
                </button>
                {#if isAdvancedSettingsOpen}
                  <div
                    id="advanced-settings-content"
                    class="advanced-settings-content"
                    transition:slide={{ duration: 180 }}
                  >
                    <label class="field-label" for="void-fraction-input"
                      >{m.field_void_fraction()}</label
                    >
                    <input
                      class="field mb-3"
                      id="void-fraction-input"
                      type="number"
                      required
                      min="0"
                      max="0.999"
                      step="0.00001"
                      bind:value={voidFractionInput}
                    />
                    <label class="field-label" for="spikiness-input">{m.field_spikiness()}</label>
                    <input
                      class="field mb-3"
                      id="spikiness-input"
                      type="number"
                      required
                      min="0"
                      max="1"
                      step="0.05"
                      bind:value={spikinessInput}
                    />
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
                      <label class="field-label" for="rng-seed-input">{m.field_seed()}</label>
                      <input
                        class="field"
                        id="rng-seed-input"
                        type="number"
                        inputmode="numeric"
                        bind:value={rngSeedInput}
                        min="0"
                      />
                    </div>
                  </div>
                {/if}
              </div>
            </div>
            <div class="checkbox-field mb-4 flex w-full items-center gap-2 px-3">
              <label class="field-label mb-0!" for="tutorial-mode">{m.field_tutorial_mode()}</label>
              <input
                id="tutorial-mode"
                type="checkbox"
                bind:checked={isTutorialMode}
                disabled={sizePreset === "custom"}
              />
            </div>
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
              <label class="field-label" for="ticket-input"
                >{m.field_ticket_or_invitation_url()}</label
              >
              <input
                class="field"
                id="ticket-input"
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
      <Board
        flat={isFlatMap}
        bind:gameState
        {terrain}
        {surfaceArea}
        {boundsRadius}
        interactive={tutorial?.isExplorationAllowed ?? true}
        highlightedCellIndex={tutorial?.highlightedCellIndex}
        onCellClicked={(cellIndex) => {
          if (tutorial) pendingTutorialClick = cellIndex;
        }}
      />
      <OrbitControls
        enableDamping
        enablePan={true}
        enableZoom={true}
        enableRotate={true}
        mouseButtons={{ LEFT: MOUSE.PAN, MIDDLE: MOUSE.DOLLY, RIGHT: MOUSE.ROTATE }}
        target={[0, 0, 0]}
        touches={{ ONE: TOUCH.PAN, TWO: TOUCH.DOLLY_ROTATE }}
      />
    </Canvas>
  </div>
{/if}

{#if tutorial && tutorial.phase.kind !== "done"}
  <div
    class="tutorial-panel fixed inset-x-0 bottom-0 z-20 flex justify-center px-4 pb-4"
    transition:slide={{ duration: 180 }}
  >
    <div
      class="pointer-events-auto flex w-full max-w-2xl items-start gap-3 rounded-md bg-slate-900/85 px-4 py-3 text-sm text-slate-100 shadow-lg backdrop-blur"
    >
      {tutorial.text}
      {#if tutorial.canAdvance}
        <button class="btn btn-primary" type="button" onclick={() => tutorial?.next()}>
          {m.tutorial_action_next()}
        </button>
      {:else if tutorial.phase.kind === "won"}
        <button class="btn btn-primary" type="button" onclick={() => tutorial?.dismiss()}>
          {m.tutorial_action_dismiss()}
        </button>
      {/if}
    </div>
  </div>
{/if}
