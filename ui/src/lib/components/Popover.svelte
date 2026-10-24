<script lang="ts">
  // A button that shows a panel below it, and hides it again: the button,
  // a click outside or Escape. For what is used now and then (extensions,
  // more actions, menus), so that it does not take room all the time.
  // A disclosure, not an ARIA menu: the panel holds ordinary controls,
  // reached with Tab.
  import type { Snippet } from 'svelte';

  let {
    label,
    title,
    open = $bindable(false),
    disabled = false,
    align = 'start',
    wide = false,
    pressed = false,
    children,
  }: {
    /** What the button shows. */
    label: Snippet;
    /** Its accessible name when `label` is only a symbol (⋯, ⚙). */
    title?: string;
    open?: boolean;
    disabled?: boolean;
    /** Which edge of the button the panel lines up with. */
    align?: 'start' | 'end';
    wide?: boolean;
    /** Shows the button as active (a filter applies, for instance). */
    pressed?: boolean;
    children: Snippet;
  } = $props();

  const id = `popover-${Math.random().toString(36).slice(2, 8)}`;
  let root = $state<HTMLElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);

  // Closed by a click anywhere else.
  $effect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (root && !root.contains(event.target as Node)) open = false;
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  });

  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) {
      event.stopPropagation();
      open = false;
      button?.focus();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="popover" bind:this={root} onkeydown={keydown}>
  <button
    type="button"
    class="trigger"
    class:pressed
    bind:this={button}
    aria-expanded={open}
    aria-controls={id}
    aria-label={title}
    {title}
    {disabled}
    onclick={() => (open = !open)}
  >
    {@render label()}
  </button>
  {#if open}
    <div {id} class="panel {align}" class:wide role="group" aria-label={title}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .popover {
    position: relative;
    display: inline-block;
  }
  .trigger.pressed {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .panel {
    position: absolute;
    z-index: 20;
    top: calc(100% + 0.3rem);
    min-width: 16rem;
    max-width: min(34rem, 92vw);
    max-height: 70vh;
    overflow: auto;
    padding: 0.6rem 0.75rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: var(--shadow);
  }
  .panel.wide {
    width: min(40rem, 92vw);
    max-width: none;
  }
  .panel.start {
    left: 0;
  }
  .panel.end {
    right: 0;
  }
</style>
