<script lang="ts">
  import { onMount } from "svelte";
  import { m } from "$lib/paraglide/messages";
  let { label, help }: { label: string; help: string } = $props();
  const id = $props.id();
  let trigger: HTMLSpanElement;
  let popup: HTMLSpanElement;
  let pinned = false;
  let focused = false;
  let hovered = false;

  function position() {
    if (!popup?.matches(":popover-open")) return;
    const anchor = trigger.getBoundingClientRect();
    const gap = 8;
    const room = window.innerWidth - anchor.right - gap * 2;
    const width = Math.min(288, window.innerWidth - gap * 2);
    popup.style.width = `${room >= 160 ? Math.min(width, room) : width}px`;
    const height = popup.getBoundingClientRect().height;
    // Prefer the right; narrow screens fall below rather than clipping the text.
    const right = room >= 160;
    popup.style.left = `${right ? anchor.right + gap : Math.max(gap, window.innerWidth - width - gap)}px`;
    popup.style.top = `${Math.max(gap, Math.min(window.innerHeight - height - gap, right ? anchor.top + (anchor.height - height) / 2 : anchor.bottom + gap))}px`;
  }
  function show() {
    if (!popup.matches(":popover-open")) popup.showPopover();
    position();
  }
  function hide() {
    if (popup.matches(":popover-open")) popup.hidePopover();
  }
  function toggle(event: Event) {
    event.preventDefault();
    event.stopPropagation();
    pinned = !pinned;
    if (pinned) show();
    else hide();
  }
  onMount(() => {
    document.addEventListener("scroll", position, true);
    window.addEventListener("resize", position);
    return () => {
      document.removeEventListener("scroll", position, true);
      window.removeEventListener("resize", position);
    };
  });
</script>

<span
  class="inline-flex items-center gap-1"
  role="group"
  onmouseenter={() => {
    hovered = true;
    show();
  }}
  onmouseleave={() => {
    hovered = false;
    if (!pinned && !focused) hide();
  }}
>
  <span>{label}</span>
  <span
    bind:this={trigger}
    role="button"
    tabindex="0"
    aria-label={m.option_help({ label })}
    aria-describedby={id}
    class="inline-flex size-4 shrink-0 cursor-help items-center justify-center rounded-full border border-current text-[10px] font-normal opacity-70 focus-visible:outline-2 focus-visible:outline-offset-2"
    onclick={toggle}
    onfocus={() => {
      focused = true;
      show();
    }}
    onblur={() => {
      focused = false;
      pinned = false;
      if (!hovered) hide();
    }}
    onkeydown={(event) => {
      if (event.key === "Escape") {
        pinned = false;
        hide();
        event.stopPropagation();
      }
      if (event.key === "Enter" || event.key === " ") toggle(event);
    }}>?</span
  >
  <span
    bind:this={popup}
    {id}
    role="tooltip"
    popover="manual"
    class="pointer-events-none fixed inset-auto m-0 rounded-lg border-0 bg-neutral p-3 text-left text-sm leading-snug font-normal whitespace-normal text-neutral-content normal-case shadow-xl"
    >{help}</span
  >
</span>
