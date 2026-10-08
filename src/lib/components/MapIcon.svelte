<script lang="ts">
  import type { MapShape } from "$lib/wasm/avoidant_wasm";
  let { kind }: { kind: MapShape["kind"] } = $props();
</script>

<svg
  viewBox="0 0 40 40"
  fill="none"
  stroke="currentColor"
  stroke-width="1.3"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if kind === "flat"}
    <path d="M3 25 16 9 37 15 24 32ZM7 20l21 6M12 14l21 6M23 11 10 27M30 13 17 30" />
  {:else if kind === "tetrahedron"}
    <path d="M20 3 3 32 36 29ZM20 3l2 21M3 32l19-8 14 5" />
  {:else if kind === "cube"}
    <path d="m20 3 15 9v17l-15 9-15-9V12ZM5 12l15 9 15-9M20 21v17" />
  {:else if kind === "octahedron"}
    <path d="M20 2 3 21 20 38 37 21ZM20 2l-7 22 7 14 8-14ZM3 21l10 3h15l9-3" />
  {:else if kind === "dodecahedron"}
    <path
      d="m12 3 16 1 10 14-5 15-15 5L4 28 2 13ZM15 12l13 2 3 12-10 7-11-8ZM12 3l3 9M28 4v10M38 18l-7 8M33 33l-12 0M18 38l3-5M4 28l6-3M2 13l13-1"
    />
  {:else if kind === "icosahedron"}
    <path
      d="M20 2 35 10 38 26 25 38 8 33 2 17ZM20 2 13 14 2 17M20 2l8 14 7-6M13 14l15 2-6 15ZM2 17l6 16 14-2 3 7M8 33l5-19M28 16l10 10-16 5M25 38l3-22"
    />
  {:else if kind === "geodesicIcosphere"}
    <circle cx="20" cy="20" r="18" />
    <path
      d="m20 2-9 7 9 5 9-5ZM2 20l9-11 1 14-10-3 8 12 2-9 8 7 10 2-2-9 10-3-9-11-1 14-8-9-8 9M10 32l10 6 10-6M20 14v16l-10 2M20 30l8-7M20 30v8"
    />
  {:else}
    <ellipse cx="20" cy="20" rx="14" ry="18" />
    <ellipse cx="20" cy="20" rx="6" ry="18" />
    <ellipse cx="20" cy="20" rx="14" ry="6" />
  {/if}
</svg>
