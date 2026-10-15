<script lang="ts">
  import { onMount } from "svelte";
  import AvoidantGame from "$lib/components/AvoidantGame.svelte";
  import type { CompetitiveConfig } from "$lib/multiplayer/types";
  let competitive = $state<CompetitiveConfig>();
  onMount(() => {
    // Only the explicit local launcher supplies this dev-only configuration.
    if (!import.meta.env.DEV || !import.meta.env.VITE_LOCAL_COMPETITIVE_PORT) return;
    const actor =
      new URL(window.location.href).searchParams.get("actor") ??
      sessionStorage.getItem("avoidant:testActor") ??
      Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) =>
        b.toString(16).padStart(2, "0"),
      ).join("");
    sessionStorage.setItem("avoidant:testActor", actor);
    competitive = {
      serverUrl: `http://${window.location.hostname}:${import.meta.env.VITE_LOCAL_COMPETITIVE_PORT}`,
      account: {
        completeLogin: async () => {},
        account: async () => ({ id: actor, username: actor, capabilities: [] }),
        token: async () => actor,
        login: async () => {},
        logout: async () => {},
      },
    };
  });
</script>

<AvoidantGame {competitive} />
