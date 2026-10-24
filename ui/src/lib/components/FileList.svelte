<script lang="ts">
  // The preparatory list, which becomes the results table once a batch has
  // run: adding (pickers, drop), removing, ordering, filtering, and the
  // files and folders left out, with their reason. It takes the height the
  // window gives it; what is used now and then is behind a button.
  import { api, problemMessage } from '../api';
  import { fileName, folderOf, formatBytes, formatCount, relativeTo } from '../format';
  import type { Exclusion } from '../generated/Exclusion';
  import type { Column } from '../grid';
  import { t } from '../i18n/index.svelte';
  import { exclusionText, ICONS } from '../labels';
  import {
    commonFolder,
    matchingRows,
    totalBytes,
    type ExtensionFilter as Filter,
    type ExtensionStat,
    type Row,
    type SortKey,
  } from '../list';
  import { byConcern, shownCode, shownCounts, shownLabel, statusLabel } from '../results';
  import type { BatchRun } from '../run.svelte';
  import type { Selection, SelectionProblem } from '../selection.svelte';
  import ExtensionFilter from './ExtensionFilter.svelte';
  import MiddleText from './MiddleText.svelte';
  import Popover from './Popover.svelte';
  import StatusBadge from './StatusBadge.svelte';
  import VirtualGrid from './VirtualGrid.svelte';

  let {
    selection,
    run,
    included,
    leftOutCount,
    extensions,
    filter,
    onfilter,
    disabled,
    active = $bindable(null),
    onaddfiles,
    onaddfolder,
    onopen,
    detailShown = false,
    ondetail,
  }: {
    selection: Selection;
    run: BatchRun;
    /** Rows kept by the extension filter. */
    included: Row[];
    leftOutCount: number;
    /** Extensions found in the folders, and which are kept. */
    extensions: ExtensionStat[];
    filter: Filter;
    onfilter: (next: Filter) => void;
    /** A batch is running. */
    disabled: boolean;
    active?: number | null;
    onaddfiles: () => void;
    onaddfolder: () => void;
    /** Enter or double-click on a row: its details. */
    onopen: (row: Row) => void;
    detailShown?: boolean;
    /** A narrow window: the detail is opened and closed by a button. */
    ondetail?: () => void;
  } = $props();

  type Tab = 'files' | 'excluded' | 'problems';
  const SORT_KEYS: SortKey[] = ['name', 'folder', 'size', 'extension'];
  let tab = $state<Tab>('files');
  let text = $state('');
  let statusFilter = $state<string | null>(null);
  let selected = $state.raw<Set<number>>(new Set());
  let sort = $state<{ column: string; descending: boolean } | null>(null);

  let busy = $derived(disabled || selection.scanning);
  // Shown once: the folder column then shows only what differs.
  let base = $derived(commonFolder(included));
  let matching = $derived(matchingRows(included, text));
  let shown = $derived.by(() => {
    if (statusFilter === null) return matching;
    return matching.filter((row) => {
      const entry = run.results.get(row.id);
      return entry !== undefined && shownCode(entry, run.decisions.get(row.id)) === statusFilter;
    });
  });
  // Counts by what the rows in the list show, live: while the batch runs,
  // when rows are taken out, when decisions are taken. Problems first.
  let statuses = $derived.by(() => {
    return shownCounts(included, run.results, run.decisions)
      .map(([code, count]) => ({ code, count, label: shownLabel(run.action, code) }))
      .sort((a, b) => byConcern(a.label, b.label) || b.count - a.count);
  });
  let withResults = $derived(statuses.reduce((sum, status) => sum + status.count, 0));
  // A state no row shows any more (rows taken out, a new batch): all again.
  $effect(() => {
    if (statusFilter !== null && !statuses.some((s) => s.code === statusFilter)) {
      statusFilter = null;
    }
  });
  let selectedCount = $derived(included.filter((row) => selected.has(row.id)).length);
  let moreOpen = $state(false);
  let countsText = $derived(
    [
      t().list.count(included.length, formatBytes(totalBytes(included))),
      base ? t().list.inFolder(base) : null,
      leftOutCount > 0 ? t().list.leftOutByExtension(leftOutCount) : null,
      selectedCount > 0 ? t().list.selected(selectedCount) : null,
    ]
      .filter((part) => part !== null)
      .join(' · '),
  );
  // The extension filter in a few words, on its button.
  let extensionSummary = $derived(
    filter.only
      ? t().list.extensionsOnly([...filter.only].map((e) => `.${e}`).join(', '))
      : filter.leftOut.size > 0
        ? t().list.extensionsLeftOut(filter.leftOut.size)
        : t().list.extensionsAll,
  );

  const columns = $derived<Column[]>([
    { id: 'name', label: t().list.columns.name, width: 'minmax(14rem, 4fr)', sortable: true },
    { id: 'folder', label: t().list.columns.folder, width: 'minmax(7rem, 2fr)', sortable: true },
    { id: 'size', label: t().list.columns.size, width: '6.5rem', align: 'end', sortable: true },
    { id: 'state', label: t().list.columns.state, width: 'minmax(15rem, 2fr)' },
  ]);
  const exclusionColumns = $derived<Column[]>([
    { id: 'path', label: t().list.columns.path, width: '3fr' },
    { id: 'reason', label: t().list.columns.reason, width: '2fr' },
  ]);
  const problemColumns = $derived<Column[]>([
    { id: 'path', label: t().list.columns.path, width: '2fr' },
    { id: 'problem', label: t().list.columns.problem, width: '3fr' },
  ]);

  function resort(column: string) {
    if (busy) return;
    const descending = sort?.column === column ? !sort.descending : false;
    sort = { column, descending };
    selection.sort(column as SortKey, descending);
  }

  function remove() {
    // The row after the removed ones (or before) becomes active, so the
    // keyboard keeps a place in the list.
    const at = shown.findIndex((row) => row.id === active);
    const kept = (row: Row) => !selected.has(row.id);
    const next = shown.slice(Math.max(at, 0)).find(kept) ?? shown.slice(0, at).reverse().find(kept);
    selection.remove(selected);
    selected = new Set();
    active = next?.id ?? null;
  }

  /** Nothing in the list: neither files nor what was left out. */
  let empty = $derived(
    included.length + selection.exclusions.length + selection.problems.length === 0,
  );

  /** Every file out of the list, with the results of the last batch. */
  function clear() {
    selection.clear();
    run.clear();
    selected = new Set();
    sort = null;
    active = null;
  }

  function move(direction: -1 | 1) {
    selection.move(selected, direction);
    sort = null;
  }

  function keys(event: KeyboardEvent): boolean {
    if (busy) return false;
    if (event.key === 'Delete' && selected.size > 0) {
      remove();
      return true;
    }
    if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
      move(event.key === 'ArrowUp' ? -1 : 1);
      return true;
    }
    return false;
  }

  function tabKeys(event: KeyboardEvent) {
    const order: Tab[] = ['files', 'excluded', 'problems'];
    const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    tab = order[(order.indexOf(tab) + step + order.length) % order.length];
    document.getElementById(`tab-${tab}`)?.focus();
  }

  function problemText(problem: SelectionProblem): string {
    if (problem.line) {
      return t().list.lineProblem(
        problem.line.number,
        fileName(problem.line.list),
        problem.line.reason,
      );
    }
    return problem.problem ? problemMessage(problem.problem) : '';
  }

  function stateOf(row: Row) {
    const entry = run.results.get(row.id);
    if (entry) return { entry, label: statusLabel(entry.outcome) };
    return null;
  }

  function percent(done: number, total: number) {
    return total > 0 ? Math.floor((done / total) * 100) : 0;
  }
</script>

<section class="list" aria-labelledby="list-title">
  <h2 id="list-title" class="sr-only">{t().list.title}</h2>
  <div class="toolbar">
    <button type="button" onclick={onaddfiles} disabled={busy} aria-keyshortcuts="Control+O">
      {t().list.addFiles}
    </button>
    <button type="button" onclick={onaddfolder} disabled={busy} aria-keyshortcuts="Control+Shift+O">
      {t().list.addFolder}
    </button>
    <Popover pressed={filter.only !== null || filter.leftOut.size > 0} wide>
      {#snippet label()}{t().list.extensionsButton(extensionSummary)}
        <span aria-hidden="true">▾</span>{/snippet}
      <ExtensionFilter {extensions} {filter} {onfilter} disabled={busy} />
    </Popover>

    {#if tab === 'files'}
      <input
        class="filter"
        type="search"
        bind:value={text}
        aria-label={t().list.filter}
        placeholder={t().list.filterHint}
      />
      <button type="button" onclick={remove} disabled={busy || selected.size === 0}>
        {t().list.remove}
      </button>
      <button type="button" onclick={clear} disabled={busy || empty}>{t().list.clear}</button>
      <Popover title={t().list.more} bind:open={moreOpen} align="end">
        {#snippet label()}<span aria-hidden="true">⋯</span>{/snippet}
        <div class="more">
          <button type="button" onclick={() => move(-1)} disabled={busy || selected.size === 0}>
            {t().list.moveUp} <span class="muted">Alt+↑</span>
          </button>
          <button type="button" onclick={() => move(1)} disabled={busy || selected.size === 0}>
            {t().list.moveDown} <span class="muted">Alt+↓</span>
          </button>
          <label>
            {t().list.sortBy}
            <select
              disabled={busy}
              value={sort?.column ?? ''}
              onchange={(e) => {
                const value = e.currentTarget.value;
                if (value) resort(value);
              }}
            >
              <option value="" disabled>—</option>
              {#each SORT_KEYS as key (key)}
                <option value={key}>{t().list.sortKeys[key]}</option>
              {/each}
            </select>
          </label>
        </div>
      </Popover>
    {/if}
  </div>

  <div class="tabsline">
    <div class="tabs" role="tablist" aria-label={t().list.title}>
      {#each [['files', t().list.tabs.files(included.length)], ['excluded', t().list.tabs.excluded(selection.exclusions.length)], ['problems', t().list.tabs.problems(selection.problems.length)]] as [id, title] (id)}
        <button
          type="button"
          role="tab"
          id="tab-{id}"
          aria-selected={tab === id}
          aria-controls="panel-{id}"
          tabindex={tab === id ? 0 : -1}
          onclick={() => (tab = id as Tab)}
          onkeydown={tabKeys}
        >
          {title}
        </button>
      {/each}
    </div>
    {#if tab === 'files'}
      <p class="counts" title={countsText}>
        {t().list.count(included.length, formatBytes(totalBytes(included)))}
        {#if base}<span class="muted">· {t().list.inFolder(base)}</span>{/if}
        {#if leftOutCount > 0}<span class="muted"
            >· {t().list.leftOutByExtension(leftOutCount)}</span
          >{/if}
        {#if selectedCount > 0}<span class="muted">· {t().list.selected(selectedCount)}</span>{/if}
      </p>
      {#if ondetail}
        <button
          type="button"
          class="detail-toggle"
          aria-expanded={detailShown}
          disabled={active === null}
          onclick={ondetail}>{t().results.details}</button
        >
      {/if}
    {/if}
  </div>

  {#if selection.scanning}
    <p class="scanning" role="status">
      {t().list.scanning(selection.progress?.files ?? 0, selection.progress?.foldersScanned ?? 0)}
      <button type="button" onclick={() => void api.cancelJob()}>{t().list.stop}</button>
    </p>
  {/if}
  {#each selection.notices as notice, i (i)}
    <p class="notice" role="status">
      {#if notice.kind === 'expanded'}{t().list.notices.expanded(notice.count, notice.name)}
      {:else if notice.kind === 'hashFileAsFile'}{t().list.notices.hashFileAsFile(notice.name)}
      {:else if notice.kind === 'stopped'}{t().list.notices.stopped}
      {:else}{t().list.notices.rescanned}{/if}
    </p>
  {/each}

  {#if tab === 'files'}
    <div id="panel-files" class="panel" role="tabpanel" aria-labelledby="tab-files">
      {#if statuses.length > 0}
        <div class="statuses" role="group" aria-label={t().list.show}>
          <span class="muted">{t().list.showing}</span>
          <button
            type="button"
            class="chip"
            aria-pressed={statusFilter === null}
            onclick={() => (statusFilter = null)}
          >
            {t().list.showAll(withResults)}
          </button>
          {#each statuses as status (status.code)}
            <button
              type="button"
              class="chip"
              aria-pressed={statusFilter === status.code}
              onclick={() => (statusFilter = statusFilter === status.code ? null : status.code)}
            >
              <StatusBadge
                tone={status.label.tone}
                icon={status.label.icon}
                label="{status.label.label} ({formatCount(status.count)})"
              />
            </button>
          {/each}
        </div>
      {/if}

      <VirtualGrid
        label={t().list.grid}
        fill
        description={t().list.keyboard}
        rows={shown}
        rowKey={(row) => row.id}
        {columns}
        {selected}
        onselect={(keys) => (selected = keys)}
        bind:active
        onopen={(row) => onopen(row)}
        onkeys={keys}
        {sort}
        onsort={resort}
        empty={included.length === 0 ? t().list.empty : t().list.emptyFiltered}
      >
        {#snippet cell(row: Row, column: string)}
          {#if column === 'name'}
            <MiddleText text={fileName(row.path)} />
          {:else if column === 'folder'}
            {@const folder = folderOf(row.path)}
            <MiddleText text={folder === base ? '' : relativeTo(folder, base)} muted />
          {:else if column === 'size'}
            {formatBytes(row.size)}
          {:else}
            {@const state = stateOf(row)}
            {@const decided = run.decisions.get(row.id)}
            {#if decided}
              <StatusBadge
                tone={decided.ok ? 'ok' : 'error'}
                icon={decided.ok ? ICONS.ok : ICONS.error}
                label={t().results.decided[decided.kind]}
              />
            {:else if state}
              <StatusBadge
                tone={state.label.tone}
                icon={state.label.icon}
                label={state.label.label}
              />
            {:else if run.running}
              {@const reading = run.reading(row.id)}
              <span class="muted">
                {reading ? t().list.reading(percent(reading.done, reading.total)) : t().list.next}
              </span>
            {:else}
              <span class="muted">
                {t().list.ready}{#if row.listed}<span class="listed"
                    >— {t().list.listedFrom(row.listed.line, fileName(row.listed.list))}</span
                  >{/if}
              </span>
            {/if}
          {/if}
        {/snippet}
      </VirtualGrid>
    </div>
  {:else if tab === 'excluded'}
    <div id="panel-excluded" class="panel" role="tabpanel" aria-labelledby="tab-excluded">
      <VirtualGrid
        label={t().list.excludedGrid}
        fill
        rows={selection.exclusions.map((exclusion, index) => ({ exclusion, index }))}
        rowKey={(row) => row.index}
        columns={exclusionColumns}
        empty={t().list.emptyExcluded}
      >
        {#snippet cell(row: { exclusion: Exclusion; index: number }, column: string)}
          {#if column === 'path'}
            <MiddleText text={row.exclusion.path} />
          {:else}
            {exclusionText(row.exclusion.reason)}
            {#if row.exclusion.folder}<span class="muted">{t().list.folder}</span>{/if}
          {/if}
        {/snippet}
      </VirtualGrid>
    </div>
  {:else}
    <div id="panel-problems" class="panel" role="tabpanel" aria-labelledby="tab-problems">
      <VirtualGrid
        label={t().list.problemsGrid}
        fill
        rows={selection.problems.map((problem, index) => ({ problem, index }))}
        rowKey={(row) => row.index}
        columns={problemColumns}
        empty={t().list.emptyProblems}
      >
        {#snippet cell(row: { problem: SelectionProblem; index: number }, column: string)}
          {#if column === 'path'}
            <MiddleText text={row.problem.path} />
          {:else}
            <span title={problemText(row.problem)}>{problemText(row.problem)}</span>
          {/if}
        {/snippet}
      </VirtualGrid>
    </div>
  {/if}
</section>

<style>
  /* A column that fills the room it is given: the grid takes what is left. */
  .list,
  .panel {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    min-width: 0;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.5rem;
    margin-bottom: 0.5rem;
  }
  .filter {
    font: inherit;
    margin-left: auto;
    width: 13rem;
    padding: 0.35rem 0.5rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }
  .tabsline {
    display: flex;
    align-items: center;
    gap: 1rem;
    border-bottom: 1px solid var(--border);
    margin-bottom: 0.5rem;
  }
  .tabs {
    display: flex;
    flex: none;
    gap: 0.1rem;
  }
  .tabs button {
    border: none;
    border-bottom: 3px solid transparent;
    border-radius: 0;
    background: none;
    padding: 0.35rem 0.6rem;
  }
  /* The current tab: a bar and bold text, not only a colour. */
  .tabs button[aria-selected='true'] {
    border-bottom-color: var(--accent);
    font-weight: 600;
  }
  .more {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.4rem;
  }
  .more button {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    text-align: start;
  }
  .more select {
    font: inherit;
    margin-left: 0.4rem;
  }
  .statuses {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.4rem;
  }
  .chip {
    border-radius: 999px;
    padding: 0.15rem 0.6rem;
  }
  .chip[aria-pressed='true'] {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
    text-decoration: underline;
  }
  .detail-toggle {
    flex: none;
    padding: 0.2rem 0.7rem;
  }
  .detail-toggle[aria-expanded='true'] {
    border-color: var(--accent);
  }
  .counts {
    flex: 1 1 auto;
    min-width: 0;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.95em;
  }
  .listed {
    margin-left: 0.3em;
  }
  .scanning,
  .notice {
    margin: 0 0 0.4rem;
  }
</style>
