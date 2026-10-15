<script lang="ts">
  import { m } from "$lib/paraglide/messages";
  import { getLocale } from "$lib/paraglide/runtime";
  import { onMount } from "svelte";
  import { SvelteSet, SvelteURL } from "svelte/reactivity";
  import { writable } from "svelte/store";
  import { Canvas, T } from "@threlte/core";
  import { WebGLRenderer } from "three";
  import MatchResults from "./MatchResults.svelte";
  import ScoreSummary from "./ScoreSummary.svelte";
  import Board from "./board.svelte";
  import MapControls from "./MapControls.svelte";
  import { generateMap } from "$lib/workers/mapgen-client";
  import mapOptions from "$lib/multiplayer/maps.json";
  import type { GameOptions } from "$lib/wasm/avoidant_wasm";
  import { revealSweep } from "$lib/multiplayer/reveal";
  import { MatchClient } from "$lib/multiplayer/client";
  import {
    PLAYER_COLORS,
    type BoardState,
    type BoardPulse,
    type MultiplayerSetup,
    type CompetitiveConfig,
    type MatchSnapshot,
  } from "$lib/multiplayer/types";
  import init, {
    type CellMetadataEntry,
    type MapCell,
    type MapData,
  } from "$lib/wasm/avoidant_wasm";
  let {
    config,
    onexit,
    setup,
    invitation,
    playerName = $bindable(),
  }: {
    config: CompetitiveConfig;
    onexit: () => void;
    setup?: MultiplayerSetup;
    invitation?: string;
    playerName: string;
  } = $props();
  let account = $state<{ id: string; capabilities: string[] } | null>(null);
  let message = $state("");
  let connectionIssue = $state(false);
  let flat = $state(false);
  let matchId = $state("");
  let snapshot = $state<MatchSnapshot>();
  let geometry = $state<MapData>();
  let board = $state<BoardState>();
  let client: MatchClient | undefined;
  let geometryPromise: Promise<void> | undefined;
  let disposed = false;
  let connectionGeneration = 0;
  const cells = writable<MapCell[]>([]);
  const metadata = writable<CellMetadataEntry[]>([]);
  const pulses = writable<BoardPulse[]>([]);
  let invitationUrl = $state("");
  let copied = $state(false);
  let savingName = $state(false);
  let nameSaved = $state(false);
  let now = $state(Date.now());
  let renderedSnapshot: MatchSnapshot | undefined;
  let pulseId = 0;
  let clicked: { cell: number; position: [number, number, number] } | undefined;
  const animationTimers = new SvelteSet<ReturnType<typeof setTimeout>>();
  const revealing = new SvelteSet<number>();
  function resetAnimations() {
    for (const timer of animationTimers) clearTimeout(timer);
    animationTimers.clear();
    revealing.clear();
    pulses.set([]);
    renderedSnapshot = undefined;
    clicked = undefined;
  }
  function later(callback: () => void, delay: number) {
    const timer = setTimeout(() => {
      animationTimers.delete(timer);
      callback();
    }, delay);
    animationTimers.add(timer);
  }
  function updateMetadata() {
    if (!renderedSnapshot) return;
    metadata.set(
      renderedSnapshot.cells.map((c, i) => ({
        isExplored: c.revealed && !revealing.has(i),
        isVoid: c.isVoid ?? false,
        voidNeighborCount: c.count ?? 0,
        isRevealing: revealing.has(i),
      })),
    );
  }
  function renderSnapshot(next: MatchSnapshot) {
    if (!geometry || (renderedSnapshot && next.revision <= renderedSnapshot.revision)) return;
    const previous = renderedSnapshot;
    renderedSnapshot = next;
    const opened = previous
      ? next.cells.flatMap((c, i) => (c.revealed && !previous.cells[i].revealed ? [i] : []))
      : [];
    if (opened.length) {
      const originCell = opened.find((i) => next.cells[i].owner !== null) ?? opened[0];
      const id = ++pulseId;
      const localMoved =
        next.players.find((p) => p.id === account?.id)?.sequence !==
        previous?.players.find((p) => p.id === account?.id)?.sequence;
      const sweep = revealSweep(
        geometry.cells,
        opened,
        originCell,
        localMoved && clicked && opened.includes(clicked.cell) ? clicked.position : undefined,
      );
      if (localMoved) clicked = undefined;
      pulses.update((items) => [
        ...items,
        {
          id,
          createdAtMs: performance.now(),
          durationMs: sweep.durationMs,
          position: sweep.position,
          originCell,
          isRemote: !localMoved,
          maxRadius: sweep.maxRadius,
        },
      ]);
      for (const { cell, afterMs } of sweep.finishes) {
        revealing.add(cell);
        later(() => {
          revealing.delete(cell);
          updateMetadata();
        }, afterMs);
      }
      later(() => pulses.update((items) => items.filter((p) => p.id !== id)), sweep.durationMs);
    }
    updateMetadata();
  }
  let player = $derived(snapshot?.players.find((p) => p.id === account?.id));
  function displayName(index: number) {
    return snapshot?.players[index]?.name || m.default_player_name();
  }
  let myIndex = $derived(snapshot?.players.findIndex((p) => p.id === account?.id) ?? -1);
  let canPlay = $derived(
    !!snapshot?.started &&
      !snapshot.finished &&
      (snapshot.turn === null || snapshot.turn === myIndex),
  );

  onMount(() => {
    const clock = setInterval(() => (now = Date.now()), 1000);
    void (async () => {
      try {
        await init();
        account = await config.account.account();
        if (account && invitation) {
          matchId = invitation;
          await join();
        } else if (account && setup) await create();
      } catch (e) {
        message = String(e);
      }
    })();
    return () => {
      clearInterval(clock);
      disposed = true;
      resetAnimations();
      client?.close();
    };
  });
  async function receive(next: MatchSnapshot, mapName: string, neighbors: number[][]) {
    const generation = connectionGeneration;
    if (snapshot && next.revision < snapshot.revision) return;
    snapshot = next;
    message = "";
    connectionIssue = false;
    const own = next.players.find((p) => p.id === account?.id);
    if (own && own.name === undefined) message = m.error_name_server_update();
    if (savingName && own) {
      savingName = false;
      nameSaved = own.name === playerName.trim();
      if (!nameSaved) message = m.error_name_not_saved();
    }
    geometryPromise ??= (async () => {
      const options = (mapOptions as Record<string, GameOptions>)[mapName];
      if (!options) throw new Error(m.error_unsupported_map());
      const generated = await generateMap(options);
      flat = options.shape?.kind === "flat";
      if (disposed || generation !== connectionGeneration) return;
      if (
        generated.cells.length !== next.cells.length ||
        JSON.stringify(generated.cells.map((c) => c.neighbors)) !== JSON.stringify(neighbors)
      )
        throw new Error(m.error_map_version());
      cells.set(generated.cells);
      geometry = generated;
      board = {
        cells,
        cellMetadata: metadata,
        uiState: { pulses },
        elevationMin: 0,
        elevationMax: 6,
        queueExplorePulse(index, x, y, z) {
          clicked = { cell: index, position: [x, y, z] };
          select(index);
        },
      };
    })();
    try {
      await geometryPromise;
      if (!disposed && generation === connectionGeneration && snapshot) renderSnapshot(snapshot);
    } catch (e) {
      message = String(e);
      client?.close();
    }
  }
  function select(cell: number) {
    const target = snapshot?.cells[cell];
    if (
      !target ||
      target.owner !== null ||
      (target.revealed && (target.isVoid || snapshot?.options.mode !== "territory"))
    )
      return;
    if (canPlay && player) client?.move(player.sequence + 1, { kind: "select", cell });
  }
  async function join() {
    if (!account) return;
    sessionStorage.setItem("avoidant:competitiveMatch", matchId);
    client?.close();
    connectionGeneration++;
    copied = false;
    resetAnimations();
    snapshot = undefined;
    geometry = undefined;
    board = undefined;
    geometryPromise = undefined;
    client = new MatchClient(
      config,
      matchId,
      account.id,
      (s, m, n) => void receive(s, m, n),
      (m, reconnect) => {
        message = m;
        connectionIssue = !!reconnect;
        savingName = false;
      },
      () => playerName,
    );
    const url = new SvelteURL(window.location.href);
    url.search = "";
    url.hash = "";
    url.searchParams.set("match", matchId);
    invitationUrl = url.toString();
    await client.connect();
  }
  async function create() {
    try {
      const response = await fetch(`${config.serverUrl}/matches`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${await config.account.token()}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(setup),
      });
      const data = await response.json();
      if (!response.ok) throw new Error(data.error);
      matchId = data.id;
      await join();
    } catch (e) {
      message = String(e);
    }
  }
  async function copyInvitation() {
    try {
      await navigator.clipboard.writeText(invitationUrl);
      copied = true;
    } catch {
      const input = document.getElementById("competitive-invitation") as HTMLInputElement | null;
      input?.focus();
      input?.select();
    }
  }
  function leave() {
    resetAnimations();
    sessionStorage.removeItem("avoidant:competitiveMatch");
    const url = new SvelteURL(window.location.href);
    url.searchParams.delete("match");
    history.replaceState(null, "", url);
    client?.close();
    onexit();
  }
</script>

<div data-theme="winter" class="not-prose h-dvh bg-transparent text-white">
  <div
    class="panel-wrapper panel-wrapper-header fixed inset-x-0 z-20 flex justify-center bg-transparent"
  >
    <div class="panel-shell panel-header w-full max-w-full">
      <div class="flex flex-wrap items-center gap-3">
        <h1 class="panel-title text-lg font-semibold tracking-wide">Avoidant</h1>
        {#if snapshot?.started}
          {@const match = snapshot}
          <ScoreSummary>
            {#snippet summary()}
              {#if match.options.mode === "collaborative"}
                <strong
                  >{m.label_team_score({
                    score: match.players
                      .reduce((sum, p) => sum + p.score, 0)
                      .toLocaleString(getLocale()),
                  })}</strong
                >
              {:else}
                {m.label_score()}:
                <strong>{(player?.score ?? 0).toLocaleString(getLocale())}</strong>
              {/if}
              <span class="opacity-80">
                {#if match.finished}
                  {#if match.options.mode === "collaborative"}{m.status_session_complete()}
                  {:else}
                    {@const best = Math.max(...match.players.map((p) => p.score))}
                    {m.status_match_complete()} — {match.players.filter((p) => p.score === best)
                      .length > 1
                      ? m.status_draw()
                      : m.status_named_winner({
                          name: displayName(match.players.findIndex((p) => p.score === best)),
                        })}
                  {/if}
                {:else if match.turn !== null}
                  {m.status_named_turn({ name: displayName(match.turn) })}
                  {#if match.turnDeadline > 0}<span class="tabular-nums">
                      · {m.label_turn_remaining({
                        seconds: Math.max(
                          0,
                          Math.ceil((match.turnDeadline - now) / 1000),
                        ).toLocaleString(getLocale()),
                      })}</span
                    >{/if}
                {:else if match.options.mode === "collaborative"}{m.mode_collaborative_hint()}
                {:else}{m.status_realtime_claims()}{/if}
              </span>
            {/snippet}
            {#snippet breakdown()}
              <div class="grid gap-2">
                {#each match.players as p, i (p.id)}
                  <div class="flex items-start justify-between gap-4">
                    <span style:color={PLAYER_COLORS[i]}
                      >{displayName(i)}{p.id === account?.id ? m.label_you() : ""}</span
                    >
                    <div class="text-right">
                      <strong
                        >{m.label_points({ count: p.score.toLocaleString(getLocale()) })}</strong
                      >
                      {#if match.options.mode === "territory"}<p class="opacity-80">
                          {m.label_claims({ count: p.claims.toLocaleString(getLocale()) })}
                        </p>{/if}
                      <p class="opacity-70">
                        {m.label_voids_discovered()}: {p.voids.toLocaleString(getLocale())}
                      </p>
                    </div>
                  </div>
                {/each}
                <p class="border-t border-slate-600 pt-2">
                  {m.label_revealed_count({
                    count: match.cells
                      .filter((c) => c.revealed && !c.isVoid)
                      .length.toLocaleString(getLocale()),
                  })}
                </p>
                {#if !match.finished}<p class="opacity-70">
                    {m.label_match_ends({
                      time: new Date(match.deadline).toLocaleTimeString(getLocale()),
                    })}
                  </p>{/if}
              </div>
            {/snippet}
          </ScoreSummary>
        {/if}
        <div class="ml-auto flex gap-2">
          {#if snapshot?.started && !snapshot.finished && snapshot.turn !== null}
            <button
              class="btn btn-secondary"
              disabled={!canPlay}
              onclick={() => player && client?.move(player.sequence + 1, { kind: "pass" })}
              >{m.action_pass()}</button
            >
          {/if}
          <button class="btn btn-error" onclick={leave}>{m.action_exit()}</button>
        </div>
      </div>
      {#if snapshot?.finished}<MatchResults players={snapshot.players} />{/if}
      {#if message}
        <div class="mt-2 flex flex-wrap items-center gap-2 text-sm">
          <p role="status" class="text-amber-300">{message}</p>
          {#if connectionIssue}
            <button class="btn btn-secondary" onclick={() => config.account.login()}
              >{m.action_login_again()}</button
            >
            {#if snapshot}<button class="btn btn-secondary" onclick={() => client?.connect()}
                >{m.action_reconnect()}</button
              >{/if}
          {/if}
        </div>
      {/if}
    </div>
  </div>
  {#if !snapshot?.started}
    <div class="panel-wrapper panel-wrapper-card fixed inset-x-0 z-10 flex justify-center px-3">
      <div class="panel-shell panel-card w-full max-w-md space-y-4 py-6 text-slate-700">
        {#if !account}
          <p>{m.text_account_multiplayer()}</p>
          <button class="btn btn-primary" onclick={() => config.account.login()}
            >{m.action_login()}</button
          >
          <button class="btn btn-secondary" onclick={() => config.account.login(true)}
            >{m.action_register()}</button
          >
        {:else if !snapshot}<p>{m.status_connecting()}</p>
        {:else}
          <p>{m.text_waiting_lobby()}</p>
          <form
            class="flex items-end gap-2"
            onsubmit={(event) => {
              event.preventDefault();
              savingName = true;
              nameSaved = false;
              void client?.connect();
            }}
          >
            <label class="fieldset min-w-0 flex-1 py-0 text-left">
              <span class="fieldset-legend">{m.field_player_name()}</span>
              <input
                class="input w-full input-sm"
                type="text"
                required
                maxlength="32"
                bind:value={playerName}
              />
            </label>
            <button
              class="btn btn-secondary btn-sm"
              type="submit"
              disabled={savingName || playerName.trim() === player?.name}
              >{m.action_save_name()}</button
            >
          </form>
          {#if nameSaved && playerName.trim() === player?.name}<p class="text-sm" role="status">
              {m.status_name_saved()}
            </p>{/if}
          <div class="flex flex-wrap justify-center gap-3">
            {#each snapshot.players as p, i (p.id)}<span
                >{displayName(i)}{p.id === account.id ? m.label_you() : ""}</span
              >{/each}
          </div>
          <label class="fieldset text-left">
            <span class="fieldset-legend">{m.field_invitation_link()}</span>
            <input
              id="competitive-invitation"
              class="input w-full input-sm"
              readonly
              value={invitationUrl}
              onclick={(event) => event.currentTarget.select()}
            />
          </label>
          <div class="flex flex-wrap justify-center gap-2">
            <button class="btn btn-secondary" onclick={copyInvitation}
              >{copied ? m.action_copied() : m.action_copy_invitation()}</button
            >
            {#if myIndex === 0}<button
                class="btn btn-primary"
                disabled={snapshot.players.length < 2}
                onclick={() => client?.start()}>{m.action_start_match()}</button
              >{/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}
  {#if board && geometry}
    <div class="game-canvas-container h-full w-full">
      <Canvas
        colorSpace="srgb-linear"
        renderMode="on-demand"
        shadows={false}
        createRenderer={(canvas) => new WebGLRenderer({ canvas, alpha: true, antialias: true })}
      >
        <T.PerspectiveCamera makeDefault fov={50} near={0.1} far={1000} position={[90, 70, 90]} />
        <Board
          gameState={board}
          terrain={geometry.terrain}
          surfaceArea={geometry.surfaceArea}
          interactive={canPlay}
          owners={snapshot?.cells.map((c) => c.owner) ?? []}
          showZeroCounts={snapshot?.options.floodLimit !== null}
        />
        <MapControls {flat} radius={geometry.boundsRadius} />
      </Canvas>
    </div>
  {/if}
</div>
