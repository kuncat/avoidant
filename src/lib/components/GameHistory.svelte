<script lang="ts">
  import { onMount } from "svelte";
  import { m } from "$lib/paraglide/messages";
  import { getLocale } from "$lib/paraglide/runtime";
  import type { CompetitiveConfig, MatchHistory } from "$lib/multiplayer/types";
  import MatchResults from "./MatchResults.svelte";
  let { config, onexit }: { config: CompetitiveConfig; onexit: () => void } = $props();
  let matches = $state<MatchHistory[]>([]);
  let cursor = $state<string | null>(null);
  let busy = $state(false);
  let failed = $state(false);
  async function load() {
    busy = true;
    failed = false;
    try {
      const url = new URL("history", config.serverUrl.replace(/\/?$/, "/"));
      if (cursor) url.searchParams.set("cursor", cursor);
      const response = await fetch(url, {
        headers: { Authorization: `Bearer ${await config.account.token()}` },
      });
      if (!response.ok) throw new Error("History unavailable");
      const page = (await response.json()) as {
        matches: MatchHistory[];
        nextCursor: string | null;
      };
      matches = [...matches, ...page.matches];
      cursor = page.nextCursor;
    } catch {
      failed = true;
    } finally {
      busy = false;
    }
  }
  onMount(() => {
    void load();
  });
</script>

<div
  data-theme="winter"
  class="not-prose panel-wrapper panel-wrapper-card fixed inset-x-0 z-30 flex justify-center bg-transparent"
>
  <section
    class="panel-shell panel-card w-[min(40rem,calc(100%-2rem))] py-4 text-left"
    aria-label={m.action_game_history()}
  >
    <div class="flex items-center justify-between gap-4">
      <h2 class="text-lg font-semibold">{m.action_game_history()}</h2>
      <button class="btn btn-sm" onclick={onexit}>{m.action_exit()}</button>
    </div>
    {#each matches as match (match.id)}
      <article class="my-4 rounded-box border border-base-300 p-3">
        <h3 class="font-semibold">
          {match.options.mode === "collaborative" ? m.mode_collaborative() : m.mode_territory()}
        </h3>
        <p class="text-sm opacity-70">
          {new Date(match.finishedAt).toLocaleString(getLocale())}
        </p>
        <MatchResults players={match.players} turnBased={match.options.turnBased} />
      </article>
    {/each}
    {#if failed}<p role="alert">{m.error_history()}</p>{:else if !busy && !matches.length}<p
        class="my-4"
      >
        {m.history_empty()}
      </p>{/if}
    {#if busy}<span class="loading loading-spinner"></span>{:else if failed || cursor}<button
        class="btn btn-secondary"
        onclick={load}>{failed ? m.action_reconnect() : m.action_load_more()}</button
      >{/if}
  </section>
</div>
