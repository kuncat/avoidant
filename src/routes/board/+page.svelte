<script lang="ts">
  import { onMount } from "svelte";
  import init, { CounterState } from "$lib/wasm-pkg/avoidant.js";
  import { SvelteSet } from "svelte/reactivity";

  class UiState {
    counter: number;
    counter2: { value: number };
    set: SvelteSet<number>;
    testText: string;

    constructor() {
      this.counter = $state(0);
      this.counter2 = $state({ value: 0 });
      this.set = new SvelteSet([1, 2, 3]);
      this.testText = 'Svelte object reference is passed into Rust and mutated there.';
    }
  }

  let uiState = new UiState();
  let counterState = $state<CounterState | undefined>(undefined);

  onMount(async () => {
    await init();
    counterState = new CounterState(uiState);
  });
</script>

<h1>Welcome to SvelteKit</h1>
<p>Visit <a href="https://svelte.dev/docs/kit">svelte.dev/docs/kit</a> to read the documentation</p>

<button class="primary-action" type="button" onclick={() => counterState?.add_to_counter(3)} disabled={!counterState}>
  Change number
</button>

<button class="primary-action" type="button" onclick={() => counterState?.add_to_counter2(2)} disabled={!counterState}>
  Change number 2
</button>

<button class="primary-action" type="button" onclick={() => counterState?.add_to_set(Date.now())} disabled={!counterState}>
  Add num Rust
</button>

<!-- <button class="primary-action" type="button" onclick={() => uiState.set.add(Date.now())} disabled={!counterState}>
  Add num JS
</button> -->

<p class="wasm-counter">Counter: {uiState.counter}</p>
<p class="wasm-counter">Counter 2: {uiState.counter2.value}</p>
<p class="wasm-counter">Set size: {uiState.set.size}</p>
<p class="wasm-note">{uiState.testText}</p>
<p>{#each uiState.set as num, index (index)}
  {num}, 
{/each}
</p>
