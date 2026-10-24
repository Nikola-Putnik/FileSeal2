<script lang="ts" generics="T">
  // A list of any length shown as an accessible grid: only the visible rows
  // (and a few around them) are in the page, so 50,000 rows scroll as
  // smoothly as 50. Keyboard: arrows, Page Up/Down, Home/End, Ctrl+Home/End,
  // Space to select, Shift+arrows to extend, Ctrl+A for all, Enter to open.
  // Focus stays on the grid; the active cell is announced through
  // aria-activedescendant, so it survives rows being added or recycled.
  import type { Snippet } from 'svelte';
  import type { Column } from '../grid';
  import { toggled } from '../list';

  let {
    label,
    rows,
    rowKey,
    columns,
    cell,
    selected = new Set<number>(),
    onselect,
    active = $bindable(null),
    onopen,
    onkeys,
    sort = null,
    onsort,
    empty,
    height = '22rem',
    fill = false,
    description,
  }: {
    label: string;
    rows: T[];
    rowKey: (row: T) => number;
    columns: Column[];
    cell: Snippet<[T, string]>;
    selected?: ReadonlySet<number>;
    /** Without it, rows cannot be selected. */
    onselect?: (keys: Set<number>) => void;
    /** Key of the active row. */
    active?: number | null;
    onopen?: (row: T) => void;
    /** Extra keys (Delete…): return true when handled. */
    onkeys?: (event: KeyboardEvent) => boolean;
    sort?: { column: string; descending: boolean } | null;
    onsort?: (column: string) => void;
    empty: string;
    height?: string;
    /** Takes the height its parent gives (a flex column), not `height`. */
    fill?: boolean;
    /** How to use it, for screen readers (shown elsewhere, or not at all). */
    description?: string;
  } = $props();

  const ROW = 32;
  const OVERSCAN = 8;
  const id = `grid-${Math.random().toString(36).slice(2, 8)}`;

  let viewport = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewHeight = $state(320);
  let column = $state(0);
  let anchor: number | null = null;

  let template = $derived(columns.map((c) => c.width).join(' '));
  let first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  let last = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewHeight) / ROW) + OVERSCAN));
  let visible = $derived(rows.slice(first, last));
  let activeIndex = $derived(active === null ? -1 : rows.findIndex((r) => rowKey(r) === active));
  let activeCell = $derived(
    activeIndex < 0 ? undefined : `${id}-${rowKey(rows[activeIndex])}-${column}`,
  );
  let perPage = $derived(Math.max(1, Math.floor(viewHeight / ROW) - 1));

  function reveal(index: number) {
    if (!viewport) return;
    const top = index * ROW;
    // The header takes one row at the top of the viewport.
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + 2 * ROW > viewport.scrollTop + viewHeight)
      viewport.scrollTop = top + 2 * ROW - viewHeight;
  }

  /**
   * As in Explorer, the selection follows the active row (`select`), grows
   * from the anchor with Shift (`extend`), or stays as it is with Ctrl
   * (`keep`), to move without losing it.
   */
  function moveTo(index: number, mode: 'select' | 'extend' | 'keep') {
    if (rows.length === 0) return;
    const target = Math.max(0, Math.min(rows.length - 1, index));
    active = rowKey(rows[target]);
    reveal(target);
    if (!onselect || mode === 'keep') return;
    if (mode === 'extend') {
      const from = anchor ?? Math.max(activeIndex, 0);
      anchor = from;
      const [a, b] = from < target ? [from, target] : [target, from];
      onselect(new Set(rows.slice(a, b + 1).map(rowKey)));
    } else {
      anchor = target;
      onselect(new Set([active]));
    }
  }

  function toggle(index: number) {
    if (!onselect || index < 0) return;
    anchor = index;
    onselect(toggled(selected, rowKey(rows[index])));
  }

  function keydown(event: KeyboardEvent) {
    if (onkeys?.(event)) {
      event.preventDefault();
      return;
    }
    const index = activeIndex < 0 ? 0 : activeIndex;
    const mode = event.shiftKey ? 'extend' : event.ctrlKey ? 'keep' : 'select';
    let handled = true;
    switch (event.key) {
      case 'ArrowDown':
        moveTo(activeIndex < 0 ? 0 : index + 1, mode);
        break;
      case 'ArrowUp':
        moveTo(index - 1, mode);
        break;
      case 'PageDown':
        moveTo(index + perPage, mode);
        break;
      case 'PageUp':
        moveTo(index - perPage, mode);
        break;
      case 'Home':
        if (event.ctrlKey) moveTo(0, event.shiftKey ? 'extend' : 'select');
        else column = 0;
        break;
      case 'End':
        if (event.ctrlKey) moveTo(rows.length - 1, event.shiftKey ? 'extend' : 'select');
        else column = columns.length - 1;
        break;
      case 'ArrowLeft':
        column = Math.max(0, column - 1);
        break;
      case 'ArrowRight':
        column = Math.min(columns.length - 1, column + 1);
        break;
      case ' ':
        toggle(activeIndex);
        break;
      case 'Enter':
        if (activeIndex >= 0) onopen?.(rows[activeIndex]);
        break;
      case 'a':
      case 'A':
        if (event.ctrlKey && onselect) onselect(new Set(rows.map(rowKey)));
        else handled = false;
        break;
      default:
        handled = false;
    }
    if (handled) event.preventDefault();
  }

  function click(event: MouseEvent, index: number) {
    const key = rowKey(rows[index]);
    if (event.shiftKey && onselect) {
      moveTo(index, 'extend');
      return;
    }
    active = key;
    if (!onselect) return;
    if (event.ctrlKey) {
      toggle(index);
    } else {
      anchor = index;
      onselect(new Set([key]));
    }
  }

  // Clicks are taken by the grid, which also has the keyboard and the focus.
  function rowAt(event: MouseEvent): number | null {
    const row = (event.target as HTMLElement).closest<HTMLElement>('[data-index]');
    return row ? Number(row.dataset.index) : null;
  }

  // Coming back to the grid: the active row must be one that is drawn.
  function focus() {
    if (activeIndex >= first && activeIndex < last) return;
    const top = Math.min(rows.length - 1, Math.ceil(scrollTop / ROW));
    if (top >= 0) active = rowKey(rows[top]);
  }
</script>

<div
  class="viewport"
  role="grid"
  tabindex="0"
  aria-label={label}
  aria-rowcount={rows.length + 1}
  aria-colcount={columns.length}
  aria-multiselectable={onselect ? 'true' : undefined}
  aria-activedescendant={activeCell}
  aria-describedby={description ? `${id}-description` : undefined}
  class:fill
  style:height={fill ? undefined : height}
  style:--columns={template}
  bind:this={viewport}
  bind:clientHeight={viewHeight}
  onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
  onkeydown={keydown}
  onclick={(event) => {
    const index = rowAt(event);
    if (index !== null) click(event, index);
  }}
  ondblclick={(event) => {
    const index = rowAt(event);
    if (index !== null) onopen?.(rows[index]);
  }}
  onfocus={focus}
>
  <div class="row header" role="row" aria-rowindex={1}>
    {#each columns as col, c (col.id)}
      <div
        class="cell {col.align ?? 'start'}"
        role="columnheader"
        aria-colindex={c + 1}
        aria-sort={sort?.column === col.id
          ? sort.descending
            ? 'descending'
            : 'ascending'
          : undefined}
      >
        {#if col.sortable && onsort}
          <button type="button" tabindex="-1" onclick={() => onsort(col.id)}>
            {col.label}
            {#if sort?.column === col.id}<span aria-hidden="true"
                >{sort.descending ? '▼' : '▲'}</span
              >{/if}
          </button>
        {:else}
          {col.label}
        {/if}
      </div>
    {/each}
  </div>
  {#if rows.length === 0}
    <p class="empty">{empty}</p>
  {:else}
    <div class="body" style:height="{rows.length * ROW}px">
      <div class="window" style:transform="translateY({first * ROW}px)">
        {#each visible as row, i (rowKey(row))}
          {@const index = first + i}
          {@const key = rowKey(row)}
          <div
            class="row"
            class:selected={selected.has(key)}
            class:odd={index % 2 === 1}
            class:active={key === active}
            role="row"
            aria-rowindex={index + 2}
            aria-selected={onselect ? selected.has(key) : undefined}
            data-index={index}
          >
            {#each columns as col, c (col.id)}
              <div
                class="cell {col.align ?? 'start'}"
                class:current={key === active && c === column}
                role="gridcell"
                id="{id}-{key}-{c}"
                aria-colindex={c + 1}
              >
                {@render cell(row, col.id)}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
{#if description}<p id="{id}-description" class="sr-only">{description}</p>{/if}

<style>
  .viewport.fill {
    flex: 1 1 auto;
    min-height: 8rem;
  }
  .viewport {
    position: relative;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    contain: strict;
  }
  .viewport:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .row {
    display: grid;
    grid-template-columns: var(--columns);
    height: 32px;
    align-items: center;
    border-bottom: 1px solid var(--border);
    cursor: default;
  }
  .header {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--surface);
    font-weight: 600;
  }
  .header button {
    border: none;
    padding: 0;
    background: none;
    font-weight: 600;
  }
  .body {
    position: relative;
  }
  .window {
    position: absolute;
    inset: 0 0 auto 0;
  }
  .cell {
    padding: 0 0.6rem;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .cell.end {
    text-align: end;
    font-variant-numeric: tabular-nums;
  }
  /* Every other row tinted, to follow a line across a wide list. */
  .row.odd {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .row.selected {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  /* The active row is marked by a bar as well as the colour. */
  .row.active {
    box-shadow: inset 4px 0 0 var(--accent);
  }
  .viewport:focus-visible .cell.current {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  .empty {
    padding: 1rem;
    color: var(--muted);
  }
  /* Contrast themes: the selection in the system colours. The system would
     otherwise put a dark plate behind each text, which hides HighlightText. */
  @media (forced-colors: active) {
    .row.selected {
      forced-color-adjust: none;
      background: Highlight;
      color: HighlightText;
    }
    .row.selected :global(*) {
      color: inherit;
    }
  }
</style>
