<script lang="ts">
  import { m } from "$lib/paraglide/messages";
  import { getLocale } from "$lib/paraglide/runtime";
  import { formatDuration } from "$lib/multiplayer/duration";
  import type { MatchSnapshot } from "$lib/multiplayer/types";
  let { players }: { players: MatchSnapshot["players"] } = $props();
</script>

<div class="overflow-x-auto">
  <table class="table table-sm">
    <thead class="text-inherit"
      ><tr
        ><th>{m.field_player_name()}</th><th class="text-right">{m.label_score()}</th><th
          class="text-right">{m.label_cumulative_time()}</th
        ></tr
      ></thead
    >
    <tbody
      >{#each players as player (player.id)}<tr
          ><td>{player.name || m.default_player_name()}</td><td class="text-right"
            >{player.score.toLocaleString(getLocale())}</td
          ><td class="text-right tabular-nums"
            >{formatDuration(player.cumulativeMs ?? 0, getLocale())}</td
          ></tr
        >{/each}</tbody
    >
  </table>
</div>
