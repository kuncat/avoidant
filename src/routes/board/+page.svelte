<script lang="ts">
  import { onMount } from "svelte";
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
  let gameConfig: "host" | "join" | undefined = $state(undefined);
  let ticketInput = $state("");
  let inviteTicket = $state("");
  let nowMs = $state(Date.now());
  let nodeReadyAtMs = $state<number | undefined>(undefined);
  let networkSnapshot = $derived(gameState?.networkSnapshot);
  let hasSeenConnectedPeers = $state(false);

  let connectedPeers = $derived(
    ($networkSnapshot?.peers ?? [])
      .filter((peer) => peer.isConnected)
      .sort((left, right) => {
        const leftName = (left.nickname ?? left.endpointId).toLowerCase();
        const rightName = (right.nickname ?? right.endpointId).toLowerCase();
        return leftName.localeCompare(rightName);
      }),
  );

  let connectionDroppedWarning = $derived(
    $networkSnapshot?.hasNode &&
      $networkSnapshot?.listenerStarted &&
      hasSeenConnectedPeers &&
      connectedPeers.length === 0,
  );

  let connectionSummary = $derived.by(() => {
    if (!gameState) {
      return "No active game session.";
    }

    if (!$networkSnapshot?.hasNode) {
      return "Not connected yet. Host must create/invite, or joiner must join ticket.";
    }

    if (!$networkSnapshot?.listenerStarted) {
      return "Network listener is starting...";
    }

    const peerCount = connectedPeers.length;
    if (peerCount === 0) {
      return "Connected to session; waiting for peers.";
    }

    return `Connected to ${peerCount} peer${peerCount === 1 ? "" : "s"}.`;
  });

  function shortenEndpointId(endpointId: string): string {
    if (endpointId.length <= 18) {
      return endpointId;
    }

    return `${endpointId.slice(0, 9)}...${endpointId.slice(-9)}`;
  }

  function formatMutationAge(lastMutationMs: number | undefined, referenceMs: number): string {
    if (lastMutationMs === undefined) {
      return "never";
    }

    const elapsedMs = Math.max(0, referenceMs - lastMutationMs);
    if (elapsedMs < 1000) {
      return `${Math.round(elapsedMs)}ms ago`;
    }

    if (elapsedMs < 10_000) {
      return `${(elapsedMs / 1000).toFixed(1)}s ago`;
    }

    if (elapsedMs < 60_000) {
      return `${Math.round(elapsedMs / 1000)}s ago`;
    }

    return `${Math.round(elapsedMs / 60_000)}m ago`;
  }

  function formatDuration(elapsedMs: number): string {
    if (elapsedMs < 1000) {
      return `${Math.max(0, Math.round(elapsedMs))}ms`;
    }

    if (elapsedMs < 60_000) {
      return `${(elapsedMs / 1000).toFixed(1)}s`;
    }

    return `${(elapsedMs / 60_000).toFixed(1)}m`;
  }

  let timeSinceLastInboundMutation = $derived(
    formatMutationAge($networkSnapshot?.lastInboundMutationMs, nowMs),
  );
  let timeSinceLastOutboundMutation = $derived(
    formatMutationAge($networkSnapshot?.lastOutboundMutationMs, nowMs),
  );
  let timeSinceNodeReady = $derived(
    nodeReadyAtMs === undefined ? "n/a" : formatDuration(nowMs - nodeReadyAtMs),
  );
  let snapshotAge = $derived(
    $networkSnapshot === undefined ? "n/a" : formatDuration(nowMs - $networkSnapshot.sampledAtMs),
  );
  let prolongedZeroPeersWarning = $derived(
    $networkSnapshot?.hasNode &&
      $networkSnapshot?.listenerStarted &&
      connectedPeers.length === 0 &&
      nodeReadyAtMs !== undefined &&
      nowMs - nodeReadyAtMs > 15_000,
  );
  let networkSnapshotStaleWarning = $derived(
    $networkSnapshot !== undefined && nowMs - $networkSnapshot.sampledAtMs > 10_000,
  );

  $effect(() => {
    if (!gameState) {
      hasSeenConnectedPeers = false;
      nodeReadyAtMs = undefined;
      return;
    }

    if (connectedPeers.length > 0) {
      hasSeenConnectedPeers = true;
    }

    if ($networkSnapshot?.hasNode && nodeReadyAtMs === undefined) {
      nodeReadyAtMs = Date.now();
    }

    if (!$networkSnapshot?.hasNode) {
      nodeReadyAtMs = undefined;
    }
  });

  onMount(() => {
    const clockIntervalId = window.setInterval(() => {
      nowMs = Date.now();
    }, 250);

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

    return () => {
      clearInterval(clockIntervalId);
    };
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
      gameConfig = undefined;
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
      gameConfig = undefined;
      ticketInput = "";
      status = undefined;
    }
  }
</script>

<details
  class="fixed top-4 left-4 z-10 max-w-lg rounded border border-gray-400 bg-white p-4"
  class:opacity-85={gameState}
>
  <summary><h1>Avoidant</h1></summary>
  <div>
    {#if status}
      <p>{status}</p>
    {/if}
    {#if !gameState}
      {#if gameConfig === "host"}
        <form
          class="w-full max-w-lg"
          onsubmit={async (event) => {
            event.preventDefault();
            await startGame();
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
          onsubmit={async (event) => {
            event.preventDefault();
            await joinGame();
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

      <div class="mt-3 rounded border border-slate-300 bg-slate-50 p-3 text-xs text-slate-700">
        <p class="font-semibold uppercase">Network Status</p>
        <p class="mt-1">{connectionSummary}</p>
        {#if $networkSnapshot}
          <p class="mt-1">
            Last Update: {new Date($networkSnapshot?.sampledAtMs).toLocaleTimeString()}
          </p>
        {/if}
        <p class="mt-1">Listener: {$networkSnapshot?.listenerStarted ? "active" : "inactive"}</p>
        <p class="mt-1">Session Topic: {$networkSnapshot?.topicId ?? "n/a"}</p>
        <p class="mt-1">
          Local Endpoint: {$networkSnapshot?.endpointId
            ? shortenEndpointId($networkSnapshot.endpointId)
            : "n/a"}
        </p>
        <p class="mt-1">Connected Peers: {connectedPeers.length}</p>
        <p class="mt-1">Time Since Node Ready: {timeSinceNodeReady}</p>
        <p class="mt-1">Snapshot Age: {snapshotAge}</p>
        <p class="mt-1">
          Last Outbound Mutation: {timeSinceLastOutboundMutation}
        </p>
        <p class="mt-1">
          Last Inbound Mutation: {timeSinceLastInboundMutation}
        </p>

        {#if connectionDroppedWarning}
          <p
            class="mt-2 rounded border border-amber-300 bg-amber-100 px-2 py-1 text-[11px] font-semibold text-amber-800"
          >
            Peer connectivity dropped to zero after being connected. Relay/session might be
            unstable.
          </p>
        {/if}

        {#if prolongedZeroPeersWarning}
          <div
            class="mt-2 rounded border border-rose-300 bg-rose-100 px-2 py-2 text-[11px] text-rose-900"
          >
            <p class="font-semibold">Connectivity Stall Detected</p>
            <p class="mt-1">
              No peers connected for {timeSinceNodeReady} after node startup.
            </p>
            <p class="mt-1">
              Bootstrap retries are running, but relay discovery/dialing may still be failing.
            </p>
          </div>
        {/if}

        {#if networkSnapshotStaleWarning}
          <p
            class="mt-2 rounded border border-orange-300 bg-orange-100 px-2 py-1 text-[11px] font-semibold text-orange-900"
          >
            No new network events for {snapshotAge}. Connection state may be stale.
          </p>
        {/if}

        {#if connectedPeers.length > 0}
          <ul
            class="mt-2 max-h-28 overflow-auto rounded border border-slate-200 bg-white p-2 font-mono text-[11px]"
          >
            {#each connectedPeers as peer (peer.endpointId)}
              <li class="mb-1 last:mb-0" title={peer.endpointId}>
                <span class="font-semibold">{peer.nickname ?? "Unknown Player"}</span>
                <span class="text-slate-500"> ({shortenEndpointId(peer.endpointId)})</span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </div>
</details>

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
