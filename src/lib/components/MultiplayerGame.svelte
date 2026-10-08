<script lang="ts">
  import { m } from "$lib/paraglide/messages";
  import { getLocale } from "$lib/paraglide/runtime";
  import { onMount } from "svelte";
  import { SvelteSet, SvelteURL } from "svelte/reactivity";
  import { writable } from "svelte/store";
  import { Canvas, T } from "@threlte/core";
  import { WebGLRenderer } from "three";
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
  }: {
    config: CompetitiveConfig;
    onexit: () => void;
    setup?: MultiplayerSetup;
    invitation?: string;
  } = $props();
  let account = $state<{ id: string; capabilities: string[] } | null>(null);
  let message = $state("");
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
  let myIndex = $derived(snapshot?.players.findIndex((p) => p.id === account?.id) ?? -1);
  let canPlay = $derived(
    !!snapshot?.started &&
      !snapshot.finished &&
      (snapshot.turn === null || snapshot.turn === myIndex),
  );

  onMount(() => {
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
      (m) => (message = m),
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

<div data-theme="night" class="not-prose flex h-dvh flex-col">
  <div
    class="z-20 m-3 max-h-[60dvh] shrink-0 overflow-y-auto rounded-lg bg-slate-950/90 p-4 text-slate-100"
  >
    <div class="flex flex-wrap items-center gap-3">
      <h2 class="m-0 text-xl text-slate-100!">{m.multiplayer_title()}</h2>
      <button class="btn btn-secondary" onclick={leave}>{m.action_back()}</button>
    </div>
    {#if message}<p role="status" class="mt-2 text-amber-300">{message}</p>
      <button class="btn btn-secondary" onclick={() => config.account.login()}
        >{m.action_login_again()}</button
      >{#if snapshot}<button class="btn btn-secondary" onclick={() => client?.connect()}
          >{m.action_reconnect()}</button
        >{/if}{/if}
    {#if !account}
      <p class="my-3">{m.text_account_multiplayer()}</p>
      <button class="btn btn-primary" onclick={() => config.account.login()}
        >{m.action_login()}</button
      >
      <button class="btn btn-secondary" onclick={() => config.account.login(true)}
        >{m.action_register()}</button
      >
    {:else if !snapshot}
      <p>{m.status_connecting()}</p>
    {:else}
      {#if snapshot.options.mode === "collaborative"}
        <p class="mt-2 font-bold">
          {m.label_team_score({
            score: snapshot.players
              .reduce((sum, p) => sum + p.score, 0)
              .toLocaleString(getLocale()),
          })} · {m.label_revealed_count({
            count: snapshot.cells
              .filter((c) => c.revealed && !c.isVoid)
              .length.toLocaleString(getLocale()),
          })}
        </p>
      {/if}
      <div class="mt-2 flex flex-wrap gap-4">
        {#each snapshot.players as p, i (p.id)}<span style:color={PLAYER_COLORS[i]}
            >{m.label_player_number({ number: (i + 1).toLocaleString(getLocale()) })}{p.id ===
            account.id
              ? m.label_you()
              : ""}: {m.label_points({ count: p.score.toLocaleString(getLocale()) })}{snapshot
              .options.mode === "territory"
              ? ` · ${m.label_claims({ count: p.claims.toLocaleString(getLocale()) })}`
              : ""}</span
          >{/each}
      </div>
      <p class="mt-2 text-sm select-all">{m.label_match_id({ id: matchId })}</p>
      <div class="flex flex-wrap items-end gap-2">
        <label class="grow"
          >{m.field_invitation_link()}<input
            id="competitive-invitation"
            class="input mb-3 w-full input-sm"
            readonly
            value={invitationUrl}
            onclick={(event) => event.currentTarget.select()}
          /></label
        >
        <button class="btn btn-secondary" onclick={copyInvitation}
          >{copied ? m.action_copied() : m.action_copy_invitation()}</button
        >
      </div>
      {#if !snapshot.started}
        <p>
          {m.text_waiting_lobby()}
        </p>
        {#if myIndex === 0}<button
            class="btn btn-primary"
            disabled={snapshot.players.length < 2}
            onclick={() => client?.start()}>{m.action_start_match()}</button
          >{/if}
      {:else if snapshot.finished && snapshot.options.mode === "collaborative"}
        <p class="font-bold">
          {m.status_session_complete()} · {m.label_team_score({
            score: snapshot.players
              .reduce((sum, p) => sum + p.score, 0)
              .toLocaleString(getLocale()),
          })}
        </p>
      {:else if snapshot.finished}
        {@const best = Math.max(...snapshot.players.map((p) => p.score))}
        <p class="font-bold">
          {m.status_match_complete()} — {snapshot.players.filter((p) => p.score === best).length > 1
            ? m.status_draw()
            : m.status_winner({
                number: (snapshot.players.findIndex((p) => p.score === best) + 1).toLocaleString(
                  getLocale(),
                ),
              })}.
        </p>
      {:else}
        <p>
          {snapshot.turn === null
            ? snapshot.options.mode === "collaborative"
              ? m.mode_collaborative_hint()
              : m.status_realtime_claims()
            : m.status_player_turn({ number: (snapshot.turn + 1).toLocaleString(getLocale()) })} · {m.label_match_ends(
            { time: new Date(snapshot.deadline).toLocaleTimeString(getLocale()) },
          )}
        </p>
        {#if snapshot.turn !== null}<button
            class="btn btn-secondary"
            disabled={!canPlay}
            onclick={() => player && client?.move(player.sequence + 1, { kind: "pass" })}
            >{m.action_pass()}</button
          >{/if}
      {/if}
    {/if}
  </div>
  {#if board && geometry}
    <div class="min-h-0 w-full flex-1">
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
