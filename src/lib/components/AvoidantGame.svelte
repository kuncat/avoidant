<script lang="ts">
  import { onMount } from "svelte";
  import OptionHelp from "./OptionHelp.svelte";
  import ScoreSummary from "./ScoreSummary.svelte";
  import MapIcon from "./MapIcon.svelte";
  import MultiplayerGame from "./MultiplayerGame.svelte";
  import type { CompetitiveConfig, MultiplayerSetup } from "$lib/multiplayer/types";
  import { devicePixelRatio } from "svelte/reactivity/window";
  import { slide } from "svelte/transition";
  import { SvelteSet } from "svelte/reactivity";
  import init, {
    GameState,
    type GameOptions,
    type MapData,
    type MapShape,
  } from "$lib/wasm/avoidant_wasm";
  import { Canvas, T } from "@threlte/core";
  import MapControls from "./MapControls.svelte";
  import { WebGLRenderer } from "three";
  import Board from "$lib/components/board.svelte";
  import { m } from "$lib/paraglide/messages";
  import { getLocale, locales, setLocale } from "$lib/paraglide/runtime";
  import { generateMap } from "$lib/workers/mapgen-client";
  import { TutorialState } from "$lib/tutorial.svelte";

  const PLAYER_NAME_STORAGE_KEY = "avoidant:playerName";
  let playerNameInput = $state<string>(m.default_player_name());
  let playerNameLoaded = $state(false);
  $effect(() => {
    if (playerNameLoaded && playerNameInput !== m.default_player_name())
      localStorage.setItem(PLAYER_NAME_STORAGE_KEY, playerNameInput);
  });

  const SETTINGS_STORAGE_KEY = "avoidant:gameSettings:v1";
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
    competitive?: CompetitiveConfig;
  };

  let { competitive }: AvoidantGameProps = $props();
  let competitiveOpen = $state(false);
  let account = $state<{ id: string; capabilities: string[] } | null>(null);
  let authMessage = $state("");
  let playMode = $state<"solo" | MultiplayerSetup["mode"]>("solo");
  let multiplayerSetup = $state<MultiplayerSetup>();
  let invitation = $state<string>();
  let bordering = $state(false);
  let turnBased = $state(true);
  let infiniteFlood = $state(true);
  let matchDuration = $state(60);
  let matchUnit = $state<"minutes" | "hours">("minutes");
  let turnSeconds = $state(5);
  let turnTimer = $state(true);
  let floodLimit = $state(0);
  const playModes = [
    { id: "solo", label: m.mode_solo, description: m.mode_solo_hint },
    { id: "collaborative", label: m.mode_collaborative, description: m.mode_collaborative_hint },
    { id: "territory", label: m.mode_territory, description: m.mode_territory_hint },
  ] as const;
  $effect(() => {
    const config = competitive;
    if (!config) return;
    let active = true;
    void (async () => {
      try {
        const params = new URL(window.location.href).searchParams;
        const pending =
          params.get("match") ??
          (params.has("state") ? sessionStorage.getItem("avoidant:competitiveMatch") : null);
        if (pending) sessionStorage.setItem("avoidant:competitiveMatch", pending);
        await config.account.completeLogin();
        const current = await config.account.account();
        if (!active) return;
        account = current;
        if (
          current?.username &&
          (!localStorage.getItem(PLAYER_NAME_STORAGE_KEY) ||
            localStorage.getItem(PLAYER_NAME_STORAGE_KEY) === m.default_player_name())
        ) {
          playerNameInput = Array.from(current.username.trim()).slice(0, 32).join("");
        }
        if (pending) {
          invitation = pending;
          ticketInput = pending;
          if (current) competitiveOpen = true;
          else setupMode = "join";
        }
      } catch (error) {
        if (active) authMessage = String(error);
      }
    })();
    return () => {
      active = false;
    };
  });
  async function login(register = false) {
    try {
      await competitive?.account.login(register);
    } catch (error) {
      authMessage = String(error);
    }
  }
  async function logout() {
    try {
      await competitive?.account.logout();
      account = null;
      playMode = "solo";
    } catch (error) {
      authMessage = String(error);
    }
  }
  function selectMode(mode: typeof playMode) {
    playMode = mode;
    if (mode !== "solo") {
      if (sizePreset === "custom") sizePreset = "medium";
      isTutorialMode = false;
    }
    infiniteFlood = mode === "collaborative";
  }

  const localeLabels: Record<Locale, string> = {
    en: "English",
    bn: "বাংলা",
  };

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
  let randomSeed = $state(true);

  function resetRandomSeed() {
    rngSeedInput = crypto.getRandomValues(new Uint32Array(1))[0];
  }
  // Shape selection state. The shape kind drives which numeric inputs are shown; each per-shape numeric value persists independently so switching back and forth doesn't reset the user's edits.
  let shapeKindInput = $state<ShapeKind>("icosahedron");
  let settingsLoaded = $state(false);
  let isTutorialMode = $state(false);
  let tutorial = $state<TutorialState | undefined>(undefined);
  let exploredCellsSeen = new SvelteSet<number>();
  let pendingTutorialClick = $state<number | undefined>(undefined);
  let setupMode: "host" | "join" | undefined = $state(undefined);
  let sizePreset = $state<SizePreset>("medium");
  let isAdvancedSettingsOpen = $state(false);
  let ticketInput = $state("");
  let joinError: string | undefined = $state(undefined);
  let score = $derived(gameState?.score);
  let cellMetadataStore = $derived(gameState?.cellMetadata);
  let cellsStore = $derived(gameState?.cells);
  let pulsesStore = $derived(gameState?.uiState?.pulses);
  let numSafeUnexploredCells = $derived(
    $score ? Math.max(0, $score.totalCells - $score.voidTotal - $score.safeExplored) : 0,
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
    playerNameInput = localStorage.getItem(PLAYER_NAME_STORAGE_KEY) || m.default_player_name();
    playerNameLoaded = true;
    resetRandomSeed();
    try {
      const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "null");
      if (saved && typeof saved === "object" && !Array.isArray(saved)) {
        if (SHAPE_KINDS.includes(saved.shapeKind)) shapeKindInput = saved.shapeKind;
        if (["small", "medium", "large", "custom"].includes(saved.sizePreset))
          sizePreset = saved.sizePreset;
        if (Number.isInteger(saved.numCells) && saved.numCells >= 32 && saved.numCells <= 5000)
          numCellsInput = saved.numCells;
        if (
          typeof saved.voidFraction === "number" &&
          saved.voidFraction >= 0 &&
          saved.voidFraction <= 0.999
        )
          voidFractionInput = saved.voidFraction;
        if (typeof saved.spikiness === "number" && saved.spikiness >= 0 && saved.spikiness <= 1)
          spikinessInput = saved.spikiness;
        randomSeed = saved.randomSeed !== false;
        if (!randomSeed && Number.isSafeInteger(saved.rngSeed) && saved.rngSeed >= 0)
          rngSeedInput = saved.rngSeed;
        if (typeof saved.tutorialMode === "boolean") isTutorialMode = saved.tutorialMode;
      }
    } catch {
      // Storage may be unavailable or contain malformed data; keep usable defaults.
    }
    settingsLoaded = true;

    const initializeWasm = async () => {
      try {
        status = m.status_loading();
        await init();
        status = undefined;
      } catch (error) {
        status = m.status_initialization_failed();
        console.error("Failed to initialize wasm", error);
      }
    };

    void initializeWasm();
  });

  $effect(() => {
    if (!gameState) return;
    setInitialCameraFov();
  });

  $effect(() => {
    if (settingsLoaded) {
      const settings = {
        shapeKind: shapeKindInput,
        sizePreset,
        numCells: numCellsInput,
        voidFraction: voidFractionInput,
        spikiness: spikinessInput,
        randomSeed,
        rngSeed: randomSeed ? undefined : rngSeedInput,
        tutorialMode: isTutorialMode,
      };
      try {
        localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
      } catch (error) {
        console.warn("Failed to persist game settings", error);
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

  function mosaicCells(divisions: number): string[] {
    const point = (x: number, y: number) => [
      (x * 40) / divisions + (x > 0 && x < divisions ? Math.sin(x * 3 + y * 7) * 2 : 0),
      (y * 40) / divisions + (y > 0 && y < divisions ? Math.cos(x * 5 + y * 3) * 2 : 0),
    ];
    const polygons: string[] = [];
    for (let y = 0; y < divisions; y++)
      for (let x = 0; x < divisions; x++) {
        const [a, b, c, d] = [point(x, y), point(x + 1, y), point(x + 1, y + 1), point(x, y + 1)];
        for (const polygon of (x + y) % 3 === 0
          ? [
              [a, b, c],
              [a, c, d],
            ]
          : [[a, b, c, d]])
          polygons.push(polygon.map((p) => p.join(",")).join(" "));
      }
    return polygons;
  }

  type PresetIcon = {
    value: SizePreset;
    cells: string[] | null;
    readonly label: string;
  };

  const presetIcons: PresetIcon[] = [
    {
      value: "small",
      cells: mosaicCells(2),
      get label() {
        return m.preset_small();
      },
    },
    {
      value: "medium",
      cells: mosaicCells(3),
      get label() {
        return m.preset_medium();
      },
    },
    {
      value: "large",
      cells: mosaicCells(5),
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
    if (status) return;
    if (playMode !== "solo") {
      if (!competitive || !account || sizePreset === "custom") return;
      multiplayerSetup = {
        mode: playMode,
        map: shapeKindInput === "icosahedron" ? sizePreset : `${shapeKindInput}-${sizePreset}`,
        bordering: playMode === "territory" && bordering,
        turnBased: playMode === "territory" && turnBased,
        floodLimit: infiniteFlood ? null : floodLimit,
        matchMinutes: matchDuration * (matchUnit === "hours" ? 60 : 1),
        turnSeconds: turnTimer ? turnSeconds : null,
      };
      invitation = undefined;
      competitiveOpen = true;
      setupMode = undefined;
      return;
    }
    let nextGameState: GameState | undefined;
    try {
      if (randomSeed) resetRandomSeed();
      const options: GameOptions = {
        elevationMax: 6.0,
        elevationMin: 0.0,
        numCells: selectedCellCount,
        rngSeed: $state.snapshot(rngSeedInput),
        shape: resolveShape(),
        spikiness: $state.snapshot(spikinessInput),
        voidFraction: $state.snapshot(voidFractionInput),
      };
      status = m.status_generating_map();
      setupMode = undefined;
      const generated = await generateMap(options);
      nextGameState = new GameState(options);
      nextGameState.applyMapCells(generated.cells);
      // Publish only a complete map: mounting an empty game while old terrain
      // remains makes the board look up terrain indices in an empty cell list.
      isFlatMap = options.shape?.kind === "flat";
      terrain = generated.terrain;
      surfaceArea = generated.surfaceArea;
      boundsRadius = generated.boundsRadius;
      exploredCellsSeen = new SvelteSet<number>();
      pendingTutorialClick = undefined;
      tutorial = isTutorialMode ? new TutorialState() : undefined;
      gameState = nextGameState;
    } catch (error) {
      nextGameState?.free();
      console.error("Failed to start game", error);
    } finally {
      setupMode = undefined;
      status = undefined;
    }
  }

  function joinGame() {
    joinError = undefined;
    if (!account || !competitive) {
      joinError = m.error_login_to_join();
      return;
    }
    try {
      const input = ticketInput.trim();
      const id = /^https?:\/\//i.test(input) ? new URL(input).searchParams.get("match") : input;
      if (!id || !/^[a-f0-9-]{36}$/.test(id)) throw new Error(m.error_invitation_invalid());
      invitation = id;
      multiplayerSetup = undefined;
      competitiveOpen = true;
      setupMode = undefined;
    } catch (error) {
      joinError = String(error);
    }
  }

  function exitGame() {
    try {
      gameState?.free();
    } catch (error) {
      console.error("Failed to release game state", error);
    }
    gameState = undefined;
    terrain = undefined;
    surfaceArea = 0;
    boundsRadius = DEFAULT_RADIUS;
    setupMode = undefined;
    ticketInput = "";
    joinError = undefined;
    status = undefined;
    tutorial = undefined;
    exploredCellsSeen = new SvelteSet<number>();
    pendingTutorialClick = undefined;
  }
</script>

{#if competitiveOpen && competitive}
  <MultiplayerGame
    bind:playerName={playerNameInput}
    config={competitive}
    setup={multiplayerSetup}
    {invitation}
    onexit={() => {
      competitiveOpen = false;
      multiplayerSetup = undefined;
      invitation = undefined;
    }}
  />
{/if}

<div
  data-theme="winter"
  class="panel-wrapper fixed inset-x-0 z-20 flex justify-center bg-transparent select-none"
  style:display={competitiveOpen && competitive ? "none" : undefined}
  class:[&_*]:text-white={gameState}
  class:panel-wrapper-card={!gameState}
  class:panel-wrapper-header={gameState}
  oncontextmenu={(event) => event.preventDefault()}
  role="application"
>
  <div
    class="panel-shell w-full"
    class:max-w-full={gameState}
    class:max-w-xl={!gameState && setupMode}
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
          <ScoreSummary>
            {#snippet summary()}
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
            {/snippet}
            {#snippet breakdown()}
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
            {/snippet}
          </ScoreSummary>
        {/if}
        <div class="ml-auto flex gap-2">
          <button class="btn btn-error" type="button" onclick={exitGame}>{m.action_exit()}</button>
        </div>
      {/if}
    </div>

    {#if !gameState}
      <div class="my-3 flex flex-wrap justify-center gap-2">
        {#if account}<button class="btn btn-secondary btn-sm" onclick={logout}
            >{m.action_logout()}</button
          >
        {:else}<button
            class="btn btn-secondary btn-sm"
            disabled={!competitive}
            onclick={() => login()}>{m.action_login()}</button
          ><button
            class="btn btn-secondary btn-sm"
            disabled={!competitive}
            onclick={() => login(true)}>{m.action_register()}</button
          >{/if}
      </div>
      {#if authMessage}<p role="status" class="text-sm">{authMessage}</p>{/if}
    {/if}
    {#if status}
      <p class="mt-2 text-sm text-slate-600!">{status}</p>
    {/if}

    {#if !gameState && !status}
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
              <span class="field-label"
                ><OptionHelp label={m.field_play_mode()} help={m.help_play_mode()} /></span
              >
              <div class="grid grid-cols-3 gap-2">
                {#each playModes as mode (mode.id)}
                  <button
                    type="button"
                    class="btn h-auto min-h-28 flex-col gap-1 py-3 font-normal disabled:bg-base-200 disabled:text-base-content/60 disabled:opacity-100"
                    class:btn-primary={playMode === mode.id}
                    aria-pressed={playMode === mode.id}
                    disabled={mode.id !== "solo" && (!account || !competitive)}
                    onclick={() => selectMode(mode.id)}
                  >
                    <span class="preset-icon"
                      ><svg
                        viewBox="0 0 40 40"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.6"
                        aria-hidden="true"
                      >
                        {#if mode.id === "solo"}<circle cx="20" cy="10" r="5" /><path
                            d="M9 34v-8a11 11 0 0 1 22 0v8M15 25l5 5 5-5"
                          />
                        {:else if mode.id === "collaborative"}<circle
                            cx="12"
                            cy="10"
                            r="4"
                          /><circle cx="28" cy="10" r="4" /><path
                            d="M5 26v-5a7 7 0 0 1 14 0v5M21 26v-5a7 7 0 0 1 14 0v5M12 29h16M20 24v10"
                          />
                        {:else}<path
                            d="M4 5h15v13H4zM21 5h15v13H21zM4 20h15v15H4zM21 20h15v15H21zM7 14l9-6M24 31l9-7"
                          />{/if}
                      </svg></span
                    >
                    <span class="preset-label">{mode.label()}</span><span class="text-xs"
                      >{mode.description()}</span
                    >
                  </button>
                {/each}
              </div>
              {#if !account}<p class="field-help mt-2">
                  {m.text_multiplayer_login()}
                </p>{/if}
            </div>

            <fieldset
              class="mx-3 mb-6 fieldset min-w-0 flex-1 rounded-lg border border-secondary px-4 pb-4"
              transition:slide={{ duration: 180 }}
            >
              <legend class="fieldset-legend">{m.label_gameplay_options()}</legend>
              {#if playMode !== "solo"}
                <label
                  class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                >
                  <span
                    ><OptionHelp label={m.field_player_name()} help={m.help_player_name()} /></span
                  >
                  <input
                    class="input w-30 min-w-0 text-right input-sm"
                    type="text"
                    maxlength="32"
                    required
                    aria-label={m.field_player_name()}
                    bind:value={playerNameInput}
                  />
                </label>
              {/if}
              {#if playMode !== "solo"}
                <div class="flex w-full items-center justify-between gap-4">
                  <label
                    class="label text-left whitespace-normal text-base-content"
                    for="match-duration"
                    ><OptionHelp
                      label={m.field_match_duration()}
                      help={m.help_match_duration()}
                    /></label
                  >
                  <div class="join w-44 shrink-0">
                    <input
                      id="match-duration"
                      class="input join-item min-w-0 flex-1 text-right input-sm"
                      type="number"
                      min="1"
                      max={matchUnit === "hours" ? 24 : 1440}
                      step="1"
                      required
                      aria-label={m.field_match_duration()}
                      bind:value={matchDuration}
                    />
                    <select
                      class="select join-item w-24 select-sm"
                      aria-label={m.field_duration_unit()}
                      value={matchUnit}
                      onchange={(event) => {
                        const unit = event.currentTarget.value as "minutes" | "hours";
                        const minutes = matchDuration * (matchUnit === "hours" ? 60 : 1);
                        matchDuration = Math.min(
                          unit === "hours" ? 24 : 1440,
                          Math.max(1, Math.ceil(unit === "hours" ? minutes / 60 : minutes)),
                        );
                        matchUnit = unit;
                      }}
                      ><option value="minutes">{m.unit_minutes()}</option><option value="hours"
                        >{m.unit_hours()}</option
                      ></select
                    >
                  </div>
                </div>
              {/if}
              {#if playMode === "solo"}
                <div class="flex flex-col items-start gap-2" transition:slide={{ duration: 180 }}>
                  <label
                    class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                    for="tutorial-mode"
                  >
                    <span
                      ><OptionHelp
                        label={m.field_tutorial_mode()}
                        help={m.help_tutorial_mode()}
                      /></span
                    >
                    <input
                      id="tutorial-mode"
                      type="checkbox"
                      class="checkbox shrink-0 rounded-sm checkbox-sm checkbox-primary"
                      aria-label={m.field_tutorial_mode()}
                      bind:checked={isTutorialMode}
                      disabled={sizePreset === "custom"}
                    />
                  </label>
                </div>
              {/if}
              {#if playMode !== "solo"}
                {#if playMode === "territory"}
                  <div class="flex flex-col items-start gap-2" transition:slide={{ duration: 180 }}>
                    <div
                      class="form-control flex w-full flex-row items-center justify-between gap-4"
                    >
                      <label
                        class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                        ><span
                          ><OptionHelp
                            label={m.field_bordering()}
                            help={m.help_bordering()}
                          /></span
                        ><input
                          type="checkbox"
                          class="checkbox shrink-0 rounded-sm checkbox-sm checkbox-primary"
                          aria-label={m.field_bordering()}
                          bind:checked={bordering}
                        /></label
                      >
                    </div>
                    <div
                      class="form-control flex w-full flex-row items-center justify-between gap-4"
                    >
                      <label
                        class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                        ><span
                          ><OptionHelp
                            label={m.field_turn_based()}
                            help={m.help_turn_based()}
                          /></span
                        ><input
                          type="checkbox"
                          class="checkbox shrink-0 rounded-sm checkbox-sm checkbox-primary"
                          aria-label={m.field_turn_based()}
                          bind:checked={turnBased}
                        /></label
                      >
                    </div>
                  </div>
                {/if}
                {#if playMode === "territory" && turnBased}
                  <div class="grid gap-2" transition:slide={{ duration: 180 }}>
                    <label
                      class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                      ><span
                        ><OptionHelp
                          label={m.field_turn_timer()}
                          help={m.help_turn_timer()}
                        /></span
                      ><input
                        type="checkbox"
                        class="checkbox shrink-0 rounded-sm checkbox-sm checkbox-primary"
                        aria-label={m.field_turn_timer()}
                        bind:checked={turnTimer}
                      /></label
                    >
                    {#if turnTimer}<label
                        class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                        transition:slide={{ duration: 180 }}
                        ><span
                          ><OptionHelp
                            label={m.field_turn_seconds()}
                            help={m.help_turn_seconds()}
                          /></span
                        ><input
                          class="input w-30 text-right input-sm"
                          type="number"
                          min="1"
                          max="86400"
                          step="1"
                          required
                          aria-label={m.field_turn_seconds()}
                          bind:value={turnSeconds}
                        /></label
                      >{/if}
                  </div>
                {/if}
                <div class="form-control flex w-full flex-row items-center justify-between gap-4">
                  <label
                    class="label text-left whitespace-normal text-base-content"
                    for="flood-limit"
                    ><OptionHelp label={m.field_flood_limit()} help={m.help_flood_limit()} /></label
                  >
                  <div class="join w-30 shrink-0" role="group" aria-label={m.field_flood_limit()}>
                    <input
                      id="flood-limit"
                      class="input join-item min-w-0 flex-1 text-right input-sm focus:outline-none"
                      aria-label={m.field_flood_cells()}
                      type="number"
                      min={infiniteFlood ? undefined : 0}
                      max={infiniteFlood ? undefined : 4096}
                      step="1"
                      required={!infiniteFlood}
                      class:input-ghost={infiniteFlood}
                      class:border-base-300={infiniteFlood}
                      class:border-2={!infiniteFlood}
                      class:input-primary={!infiniteFlood}
                      bind:value={floodLimit}
                      onclick={() => (infiniteFlood = false)}
                    />
                    <button
                      type="button"
                      class="btn join-item btn-sm"
                      class:btn-primary={infiniteFlood}
                      class:btn-outline={!infiniteFlood}
                      class:border-base-300={!infiniteFlood}
                      aria-label={m.field_infinite_flood()}
                      aria-pressed={infiniteFlood}
                      onclick={() => (infiniteFlood = true)}
                      ><span aria-hidden="true" class="text-xl">∞</span></button
                    >
                  </div>
                </div>
              {/if}
            </fieldset>
            <div class="mb-4 w-full px-3" role="group" aria-label={m.field_map()}>
              <span id="map-label" class="field-label"
                ><OptionHelp label={m.field_map()} help={m.help_map()} /></span
              >
              <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
                {#each SHAPE_KINDS as kind (kind)}
                  <button
                    type="button"
                    class="btn h-auto min-h-28 min-w-0 flex-col gap-2 px-2 py-3 font-normal"
                    class:btn-primary={shapeKindInput === kind}
                    aria-pressed={shapeKindInput === kind}
                    onclick={() => (shapeKindInput = kind)}
                  >
                    <span class="preset-icon"><MapIcon {kind} /></span>
                    <span class="text-xs whitespace-normal">{m.shape_label({ kind })}</span>
                  </button>
                {/each}
              </div>
            </div>
            <fieldset
              class="mx-3 mb-6 fieldset w-full min-w-0 rounded-lg border border-secondary px-3"
              transition:slide={{ duration: 180 }}
            >
              <legend class="fieldset-legend">{m.label_map_options()}</legend>
              <div class="w-full">
                <span class="field-label"
                  ><OptionHelp label={m.field_map_size()} help={m.help_map_size()} /></span
                >
                <div class="grid grid-cols-4 gap-2">
                  {#each presetIcons as preset (preset.value)}
                    <button
                      type="button"
                      class="btn h-auto min-h-24 flex-col gap-1 py-3 font-normal"
                      class:btn-primary={sizePreset === preset.value}
                      disabled={playMode !== "solo" && preset.value === "custom"}
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
                            stroke-width="2"
                            stroke-linecap="round"
                            aria-hidden="true"
                          >
                            <line x1="8" y1="10" x2="32" y2="10" />
                            <circle cx="15" cy="10" r="1.5" fill="currentColor" />
                            <line x1="8" y1="20" x2="32" y2="20" />
                            <circle cx="25" cy="20" r="1.5" fill="currentColor" />
                            <line x1="8" y1="30" x2="32" y2="30" />
                            <circle cx="19" cy="30" r="1.5" fill="currentColor" />
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
                <fieldset
                  class="fieldset w-full min-w-0 rounded-lg border border-secondary px-3 pb-4"
                  transition:slide={{ duration: 180 }}
                >
                  <legend class="fieldset-legend">{m.label_custom_map_options()}</legend>
                  <div class="form-control flex w-full flex-row items-center justify-between gap-4">
                    <label
                      class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                      for="size-input"
                      ><OptionHelp label={m.field_size()} help={m.help_size()} />
                      <input
                        class="input w-32 input-sm"
                        id="size-input"
                        type="number"
                        required
                        min="32"
                        max="5000"
                        step="1"
                        aria-label={m.field_size()}
                        bind:value={numCellsInput}
                      />
                    </label>
                  </div>
                </fieldset>
              {/if}
              <div class="field-help my-4 w-full px-3">
                {usesFaceDensity
                  ? m.text_density_summary({ faces: faceCount, count: selectedCellCount })
                  : m.cells_count({ count: selectedCellCount })}
              </div>
            </fieldset>
            {#if playMode === "solo"}
              <fieldset class="mb-4 fieldset w-full px-3" transition:slide={{ duration: 180 }}>
                <div
                  class="advanced-settings"
                  class:advanced-settings-open={isAdvancedSettingsOpen}
                >
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
                      <div
                        class="form-control flex w-full flex-row items-center justify-between gap-4"
                      >
                        <label
                          class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                          ><span
                            ><OptionHelp
                              label={m.field_void_fraction()}
                              help={m.help_void_fraction()}
                            /></span
                          ><input
                            class="input w-24 text-right input-sm"
                            max="0.999"
                            min="0"
                            required
                            step="0.00001"
                            type="number"
                            aria-label={m.field_void_fraction()}
                            bind:value={voidFractionInput}
                          /></label
                        >
                      </div>
                      <div
                        class="form-control flex w-full flex-row items-center justify-between gap-4"
                      >
                        <label
                          class="label w-full justify-between gap-4 text-left whitespace-normal text-base-content"
                          ><span
                            ><OptionHelp
                              label={m.field_spikiness()}
                              help={m.help_spikiness()}
                            /></span
                          ><input
                            class="input w-16 text-right input-sm"
                            max="1"
                            min="0"
                            required
                            step="0.05"
                            type="number"
                            aria-label={m.field_spikiness()}
                            bind:value={spikinessInput}
                          /></label
                        >
                      </div>
                      <div
                        class="form-control flex w-full flex-row items-center justify-between gap-4"
                      >
                        <label
                          class="label text-left whitespace-normal text-base-content"
                          for="rng-seed-input"
                          ><OptionHelp label={m.field_seed()} help={m.help_seed()} /></label
                        >
                        <div class="join w-48 shrink-0" role="group" aria-label={m.field_seed()}>
                          <input
                            id="rng-seed-input"
                            class="input join-item min-w-0 flex-1 text-right input-sm focus:outline-none"
                            type="number"
                            min="0"
                            max={Number.MAX_SAFE_INTEGER}
                            step="1"
                            required
                            class:input-ghost={randomSeed}
                            class:border-base-300={randomSeed}
                            class:border-2={!randomSeed}
                            class:input-primary={!randomSeed}
                            aria-label={m.field_seed()}
                            bind:value={rngSeedInput}
                            onclick={() => (randomSeed = false)}
                          />
                          <button
                            aria-label={m.action_random()}
                            aria-pressed={randomSeed}
                            class="btn join-item btn-sm"
                            class:border-base-300={!randomSeed}
                            class:btn-outline={!randomSeed}
                            class:btn-primary={randomSeed}
                            onclick={() => (randomSeed = true)}
                            title={m.action_random()}
                            type="button"><span aria-hidden="true" class="text-xl">⚄</span></button
                          >
                        </div>
                      </div>
                    </div>
                  {/if}
                </div>
              </fieldset>
            {/if}
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
              <label class="field-label" for="ticket-input"
                >{m.field_ticket_or_invitation_url()}</label
              >
              <input
                class="input mb-3 w-full input-sm"
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
            <button class="btn btn-primary" type="submit" disabled={!account || !competitive}
              >{m.action_join()}</button
            >
          </div>
        </form>
      {:else}
        <div class="mt-4 flex flex-wrap justify-center gap-2" transition:slide={{ duration: 220 }}>
          <button class="btn btn-primary" type="button" onclick={() => (setupMode = "host")}>
            {m.action_new_game()}
          </button>
          <button
            class="btn btn-primary"
            type="button"
            disabled={!account || !competitive}
            onclick={() => (setupMode = "join")}
          >
            {m.action_join_game()}
          </button>
        </div>
        <div class="mt-4 flex justify-center" transition:slide={{ duration: 180 }}>
          <label class="sr-only" for="language-select">{m.field_language()}</label>
          <select
            id="language-select"
            class="select w-40 select-sm"
            value={getLocale()}
            aria-label={m.field_language()}
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
    {/if}
  </div>
</div>

{#if gameState}
  <div id="game-canvas-container" style="height: 100vh; width: 100%;">
    <Canvas
      colorSpace="srgb-linear"
      renderMode="on-demand"
      dpr={Math.min(devicePixelRatio.current ?? 1, 1.5)}
      shadows={false}
      createRenderer={(canvas) =>
        new WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "default" })}
    >
      <T.PerspectiveCamera
        makeDefault
        fov={cameraFov}
        near={0.1}
        far={1000}
        position={cameraPosition}
      />
      <Board
        flat={isFlatMap}
        {gameState}
        {terrain}
        {surfaceArea}
        interactive={tutorial?.isExplorationAllowed ?? true}
        highlightedCellIndex={tutorial?.highlightedCellIndex}
        onCellClicked={(cellIndex) => {
          if (tutorial) pendingTutorialClick = cellIndex;
        }}
      />
      <MapControls flat={isFlatMap} radius={boundsRadius} />
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
