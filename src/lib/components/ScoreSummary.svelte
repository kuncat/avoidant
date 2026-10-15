<script lang="ts">
  import type { Snippet } from "svelte";
  import { m } from "$lib/paraglide/messages";
  let { summary, breakdown }: { summary: Snippet; breakdown: Snippet } = $props();
  const id = $props.id();
  let hovered = $state(false);
  let pinned = $state(false);
  let dismissed = $state(false);
  const open = $derived(!dismissed && (hovered || pinned));
</script>

<div
  class="relative min-w-0 text-sm"
  role="group"
  aria-label={m.aria_score_summary()}
  onmouseenter={() => {
    hovered = true;
    dismissed = false;
  }}
  onmouseleave={() => (hovered = false)}
>
  <button
    type="button"
    class="score-summary h-auto! text-inherit"
    aria-expanded={open}
    aria-controls={id}
    onclick={() => {
      pinned = !pinned;
      dismissed = !pinned;
    }}
    onkeydown={(event) => {
      if (event.key === "Escape") {
        pinned = false;
        dismissed = true;
      }
    }}>{@render summary()}</button
  >
  {#if open}
    <div {id} class="score-breakdown max-w-[calc(100vw-2rem)]" role="status">
      {@render breakdown()}
    </div>
  {/if}
</div>
