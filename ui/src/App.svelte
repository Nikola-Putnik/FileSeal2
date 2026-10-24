<script lang="ts">
  // The single window: choose an action and its settings, build the list of
  // files (pickers, drop, folders looked through by the core), start the
  // batch, follow it, then read the results and take decisions on them.
  // It does not scroll: a header, the command line, the list (with the
  // detail beside it) taking the height left, and a bar at the bottom to
  // start, follow and conclude. The settings are kept for the next session.
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { api, problemMessage } from './lib/api';
  import BatchProgressPanel from './lib/components/BatchProgressPanel.svelte';
  import DatabaseBar, { type BarMessage } from './lib/components/DatabaseBar.svelte';
  import DatabasesScreen from './lib/components/DatabasesScreen.svelte';
  import EntryDetail from './lib/components/EntryDetail.svelte';
  import FileList from './lib/components/FileList.svelte';
  import Popover from './lib/components/Popover.svelte';
  import ResultsPanel from './lib/components/ResultsPanel.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import { formatBytes } from './lib/format';
  import type { Algorithm } from './lib/generated/Algorithm';
  import type { AppInfo } from './lib/generated/AppInfo';
  import type { BatchAction } from './lib/generated/BatchAction';
  import type { Candidate } from './lib/generated/Candidate';
  import type { DatabaseInfo } from './lib/generated/DatabaseInfo';
  import type { DiscoveryOptions } from './lib/generated/DiscoveryOptions';
  import { isLocale, locale, setLocale, t, type Locale } from './lib/i18n/index.svelte';
  import { ALGORITHM_NAMES } from './lib/labels';
  import { settingsSummary } from './lib/summary';
  import {
    ALL_EXTENSIONS,
    extensionStats,
    includedRows,
    totalBytes,
    type ExtensionFilter,
  } from './lib/list';
  import {
    acceptable,
    locationUpdates,
    observationOf,
    probableMoves,
    type Attachment,
  } from './lib/results';
  import { BatchRun, type ActionKind, type Decided } from './lib/run.svelte';
  import { defaultSettings, fromPreferences, toPreferences } from './lib/runSettings';
  import { Selection } from './lib/selection.svelte';

  let view = $state<'files' | 'databases'>('files');
  let action = $state<ActionKind>('create');
  let settings = $state(defaultSettings());
  let database = $state<DatabaseInfo | null>(null);
  const selection = new Selection();
  const run = new BatchRun();
  /** Extensions kept from the folders. */
  let extensionFilter = $state.raw<ExtensionFilter>(ALL_EXTENSIONS);
  /** Row whose details are shown. */
  let active = $state<number | null>(null);
  let dragging = $state(false);
  /** Decisions being taken for several results. */
  let working = $state<{ done: number; total: number } | null>(null);
  // A function, so the text follows a change of language.
  let message = $state<{ ok: boolean; text: () => string } | null>(null);
  // The window was asked to close while an operation runs.
  let closeRequested = $state(false);
  let appInfo = $state<AppInfo | null>(null);
  let settingsOpen = $state(false);
  let databaseMessage = $state<BarMessage | null>(null);
  // The kept settings are saved only once they have been read back.
  let restored = false;
  // Focus on the harmless answer: Enter pressed by habit must not quit.
  let stayButton = $state<HTMLButtonElement | null>(null);
  $effect(() => stayButton?.focus());

  let busy = $derived(run.running || selection.scanning || working !== null);
  let included = $derived(includedRows(selection.rows, extensionFilter));
  // Files chosen directly are always kept: only the others can be filtered.
  let extensions = $derived(extensionStats(selection.rows.filter((row) => !row.direct)));
  let leftOutCount = $derived(selection.rows.length - included.length);
  let haslisted = $derived(included.some((row) => row.listed !== null));
  let selectedAlgorithms = $derived(
    (Object.keys(settings.algorithms) as Algorithm[]).filter((a) => settings.algorithms[a]),
  );
  let databaseWritable = $derived(database !== null && !database.readOnly);
  let databaseActive = $derived(
    action === 'search'
      ? database !== null
      : settings.useDatabase && database !== null && (action === 'verify' || databaseWritable),
  );
  let destinations = $derived(
    [
      action === 'create' && settings.writeSidecar ? t().launch.whereSidecar : null,
      action === 'verify' && settings.useSidecar && included.some((row) => !row.listed)
        ? t().launch.whereSidecars
        : null,
      action === 'verify' && haslisted ? t().launch.whereLists : null,
      databaseActive && database ? t().launch.whereDatabase(database.name) : null,
    ].filter((d): d is string => d !== null),
  );
  let missing = $derived.by(() => {
    if (selection.scanning) return t().launch.busyScanning;
    if (included.length === 0) return t().launch.needFiles;
    if (action === 'search') return database ? null : t().launch.needDatabase;
    if (action === 'create' && selectedAlgorithms.length === 0) return t().launch.needAlgorithm;
    if (destinations.length === 0) {
      return action === 'create' ? t().launch.needDestination : t().launch.needSource;
    }
    return null;
  });
  let sentence = $derived.by(() => {
    const files = t().launch.files(included.length);
    const size = formatBytes(totalBytes(included));
    const where = destinations.join(t().launch.and);
    if (action === 'create') {
      const algorithms = selectedAlgorithms.map((a) => ALGORITHM_NAMES[a]).join(' + ');
      const summary = t().launch.createSummary(algorithms, files, size, where);
      if (settings.existing === 'read') return summary;
      const importing = settings.importHashFiles && databaseActive;
      return `${summary} ${importing ? t().launch.importing : t().launch.skipping}`;
    }
    if (action === 'search') return t().launch.searchSummary(files, size, database?.name ?? '');
    return t().launch.verifySummary(files, size, where);
  });

  let summary = $derived(
    settingsSummary(action, settings, databaseActive && database ? database.name : null),
  );

  // Saved shortly after each change, for the next session.
  $effect(() => {
    const kept = toPreferences({
      settings: $state.snapshot(settings),
      only: extensionFilter.only ? [...extensionFilter.only] : null,
      settingsOpen,
    });
    if (!restored) return;
    const timer = setTimeout(() => void api.settingsSetRun(kept).catch(() => {}), 400);
    return () => clearTimeout(timer);
  });

  let activeRow = $derived(
    active === null ? null : (selection.rows.find((row) => row.id === active) ?? null),
  );
  let activeEntry = $derived(active === null ? undefined : run.results.get(active));
  // Decisions for all results: computed once the batch is over.
  let updates = $derived(run.summary && !run.running ? locationUpdates(run.results) : []);
  let moves = $derived(run.summary && !run.running ? probableMoves(run.results) : []);
  let acceptances = $derived(run.summary && !run.running ? acceptable(run.results) : []);

  function discoveryOptions(): DiscoveryOptions {
    return {
      recursive: settings.recursive,
      includeHidden: settings.includeHidden,
      extensions: [],
    };
  }

  async function add(paths: string[]) {
    if (busy || paths.length === 0) return;
    // When verifying, a hash file stands for the files it lists.
    await selection.add(paths, discoveryOptions(), action === 'verify');
  }

  async function pickFiles() {
    if (busy) return;
    const picked = await open({ multiple: true, directory: false, title: t().list.addFilesTitle });
    if (picked) await add(Array.isArray(picked) ? picked : [picked]);
  }

  async function pickFolder() {
    if (busy) return;
    const picked = await open({ multiple: true, directory: true, title: t().list.addFolderTitle });
    if (picked) await add(Array.isArray(picked) ? picked : [picked]);
  }

  async function start() {
    if (busy || missing) return;
    message = null;
    const batch: BatchAction =
      action === 'create'
        ? {
            kind: 'create',
            algorithms: selectedAlgorithms,
            writeSidecar: settings.writeSidecar,
            newItem: false,
            existing: settings.existing,
            importHashFiles: settings.importHashFiles,
          }
        : action === 'verify'
          ? { kind: 'verify', useSidecar: settings.useSidecar }
          : { kind: 'search' };
    await run.start(included, batch, settings.parallelVolumes, databaseActive);
    await refreshDatabase();
  }

  async function refreshDatabase() {
    if (!database) return;
    try {
      database = await api.databaseCurrent();
    } catch {
      // The bar keeps what it showed; the next action will say what is wrong.
    }
  }

  function failure(error: unknown): Decided {
    return { ok: false, kind: 'failed', detail: () => problemMessage(error) };
  }

  /** One decision on the active row. */
  async function decide(work: () => Promise<Decided>) {
    const key = active;
    if (key === null || working) return;
    working = { done: 0, total: 1 };
    try {
      run.decisions.set(key, await work());
    } catch (error) {
      run.decisions.set(key, failure(error));
    } finally {
      working = null;
      await refreshDatabase();
    }
  }

  /** The same decision on several results, one after the other. */
  async function decideMany<T extends { key: number }>(
    items: T[],
    work: (item: T) => Promise<Decided>,
  ) {
    if (working) return;
    message = null;
    let failed = 0;
    working = { done: 0, total: items.length };
    for (const item of items) {
      let decided: Decided;
      try {
        decided = await work(item);
      } catch (error) {
        decided = failure(error);
      }
      if (!decided.ok) failed += 1;
      run.decisions.set(item.key, decided);
      working = { done: working.done + 1, total: items.length };
    }
    working = null;
    const done = items.length - failed;
    message = { ok: failed === 0, text: () => t().results.decisionsDone(done, failed) };
    await refreshDatabase();
  }

  const attachTo = (kind: 'attached' | 'updated') => async (item: Attachment) => {
    await api.attachFile(item.path, item.itemId, item.observed, false);
    return { ok: true, kind, detail: null } satisfies Decided;
  };

  async function accept(path: string, observed: Attachment['observed'], id: number | null) {
    const report = await api.acceptVersion(path, observed, id, true);
    const failed =
      report.database === 'failed' || report.sidecars.some((s) => s.outcome === 'failed');
    if (failed) throw report.databaseProblem ?? report.sidecars.find((s) => s.problem)?.problem;
    return { ok: true, kind: 'accepted', detail: null } satisfies Decided;
  }

  function attachActive(candidate: Candidate, allowDifferent: boolean) {
    const observed = activeEntry && observationOf(activeEntry.outcome);
    if (!activeEntry || !observed) return;
    const path = activeEntry.path;
    void decide(async () => {
      await api.attachFile(path, candidate.itemId, observed, allowDifferent);
      return { ok: true, kind: 'attached', detail: null };
    });
  }

  function updateActiveLocation() {
    const outcome = activeEntry?.outcome;
    const observed = outcome && observationOf(outcome);
    if (!activeEntry || !outcome || outcome.kind === 'skipped' || !observed) return;
    const itemId = outcome.report.itemId;
    if (itemId === null) return;
    void decide(() =>
      attachTo('updated')({ key: 0, path: activeEntry.path, observed, itemId: itemId }),
    );
  }

  function acceptActive() {
    const outcome = activeEntry?.outcome;
    const observed = outcome && observationOf(outcome);
    if (!activeEntry || outcome?.kind !== 'verified' || !observed) return;
    const id = outcome.report.verificationId;
    void decide(() => accept(activeEntry.path, observed, id));
  }

  /** "It is another copy": a new item for this file, against the hint. */
  function createActiveAnyway() {
    const entry = activeEntry;
    const key = active;
    if (!entry || key === null) return;
    void decide(async () => {
      const report = await api.createReference(
        entry.path,
        {
          algorithms: selectedAlgorithms,
          writeSidecar: settings.writeSidecar,
          newItem: true,
          existing: 'read',
          importHashFiles: false,
        },
        true,
        () => {},
      );
      run.results.set(key, { ...entry, outcome: { kind: 'created', report } });
      return { ok: report.status !== 'failed', kind: 'created', detail: null };
    });
  }

  async function reveal() {
    if (!activeRow) return;
    try {
      await api.revealInFolder(activeRow.path);
    } catch (error) {
      message = { ok: false, text: () => problemMessage(error) };
    }
  }

  async function exportReport() {
    const path = await save({
      title: t().report.title,
      defaultPath: t().report.fileName,
      filters: [{ name: t().report.filter, extensions: ['csv'] }],
    });
    if (!path) return;
    try {
      // Excel expects ';' in French and ',' in English.
      const count = await api.exportReport(path, locale() === 'fr' ? ';' : ',');
      message = { ok: true, text: () => t().report.exported(count, path) };
    } catch (e) {
      message = { ok: false, text: () => problemMessage(e) };
    }
  }

  async function changeLocale(next: Locale) {
    setLocale(next);
    try {
      await api.settingsSetLanguage(next);
    } catch {
      // Only the next start is concerned: the window already uses it.
    }
  }

  /** Escape in the detail: back to the row in the list. */
  function focusList() {
    document.querySelector<HTMLElement>('#panel-files [role="grid"]')?.focus();
  }

  function shortcuts(event: KeyboardEvent) {
    if (!event.ctrlKey || event.altKey) return;
    if (event.key.toLowerCase() === 'o') {
      event.preventDefault();
      void (event.shiftKey ? pickFolder() : pickFiles());
    } else if (event.key === 'Enter') {
      event.preventDefault();
      void start();
    }
  }

  onMount(() => {
    api
      .appInfo()
      .then((info) => (appInfo = info))
      .catch(() => {});
    api
      .settingsGet()
      .then((saved) => {
        if (isLocale(saved.language)) setLocale(saved.language);
        const kept = fromPreferences(saved.run);
        settings = kept.settings;
        settingsOpen = kept.settingsOpen;
        if (kept.only) extensionFilter = { leftOut: new Set(), only: new Set(kept.only) };
      })
      .catch(() => {})
      .finally(() => (restored = true));
    let stopDrop: (() => void) | undefined;
    let stopClose: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === 'enter' || payload.type === 'over') {
          dragging = !busy;
        } else if (payload.type === 'drop') {
          dragging = false;
          void add(payload.paths);
        } else {
          dragging = false;
        }
      })
      .then((stop) => (stopDrop = stop));
    void listen('close-requested', () => (closeRequested = true)).then(
      (stop) => (stopClose = stop),
    );
    return () => {
      stopDrop?.();
      stopClose?.();
    };
  });
</script>

<svelte:window onkeydown={shortcuts} />

<div class="app" class:dragging>
  <header>
    <div class="brand">
      <h1>FileSeal</h1>
      {#if appInfo}
        <span class="muted" title={t().about.version(appInfo.version)}>{appInfo.version}</span>
        {#if appInfo.portable}
          <span class="portable" title={t().about.portableHint}>{t().about.portable}</span>
        {/if}
      {/if}
    </div>
    <nav class="views" aria-label={t().nav.label}>
      <button
        type="button"
        aria-current={view === 'files' ? 'page' : undefined}
        onclick={() => (view = 'files')}>{t().nav.files}</button
      >
      <button
        type="button"
        aria-current={view === 'databases' ? 'page' : undefined}
        onclick={() => (view = 'databases')}>{t().nav.databases}</button
      >
    </nav>
    <div class="end">
      <DatabaseBar bind:database running={busy} onmessage={(next) => (databaseMessage = next)} />
      <Popover title={t().header.help} align="end" wide>
        {#snippet label()}<span aria-hidden="true">?</span>{/snippet}
        <h3>{t().shortcuts.title}</h3>
        <p>{t().shortcuts.list}</p>
        <p>{t().list.keyboard}</p>
      </Popover>
      <Popover title={t().header.preferences} align="end">
        {#snippet label()}<span aria-hidden="true">⚙</span>{/snippet}
        <label class="language">
          {t().language.label}
          <select
            value={locale()}
            onchange={(e) => {
              const value = e.currentTarget.value;
              if (isLocale(value)) void changeLocale(value);
            }}
          >
            <option value="en">{t().language.en}</option>
            <option value="fr">{t().language.fr}</option>
          </select>
        </label>
      </Popover>
    </div>
  </header>

  {#if closeRequested}
    <div class="closing" role="alertdialog" aria-labelledby="closing-question">
      <p id="closing-question">{t().close.question}</p>
      <button type="button" class="primary" onclick={() => void api.closeConfirmed()}
        >{t().close.quit}</button
      >
      <button type="button" bind:this={stayButton} onclick={() => (closeRequested = false)}
        >{t().close.stay}</button
      >
    </div>
  {/if}
  {#if databaseMessage}
    {@const shown = databaseMessage}
    <p class="strip {shown.tone}" role="status">
      <span>{shown.text()}</span>
      <button type="button" onclick={() => (databaseMessage = null)}>{t().close.dismiss}</button>
    </p>
  {/if}

  <main>
    {#if view === 'databases'}
      <DatabasesScreen bind:database {busy} />
    {/if}
    <!-- Kept while hidden: the list and its results survive a visit to the
         Databases screen. -->
    <div class="files" hidden={view !== 'files'}>
      <div class="commands">
        <div class="actions" role="radiogroup" aria-label={t().action.label}>
          {#each [['create', t().action.create], ['verify', t().action.verify], ['search', t().action.search]] as [value, label] (value)}
            <label class:selected={action === value}>
              <input type="radio" name="action" {value} bind:group={action} disabled={busy} />
              {label}
            </label>
          {/each}
        </div>
        <button
          type="button"
          class="settings-toggle"
          aria-expanded={settingsOpen}
          aria-controls="settings-panel"
          onclick={() => (settingsOpen = !settingsOpen)}
        >
          <strong>{t().settings.title}</strong>
          {#if summary}<span class="muted">{summary}</span>{/if}
          <span aria-hidden="true">{settingsOpen ? '▴' : '▾'}</span>
        </button>
      </div>

      {#if settingsOpen}
        <SettingsPanel
          id="settings-panel"
          {action}
          bind:settings
          {database}
          disabled={busy}
          onfolderoptions={() => void selection.rescan(discoveryOptions())}
          {haslisted}
        />
      {/if}

      <div class="workspace" class:with-detail={activeRow !== null}>
        <FileList
          {selection}
          {run}
          {included}
          {leftOutCount}
          {extensions}
          filter={extensionFilter}
          onfilter={(next) => (extensionFilter = next)}
          disabled={run.running || working !== null}
          bind:active
          onaddfiles={() => void pickFiles()}
          onaddfolder={() => void pickFolder()}
          onopen={(row) => {
            active = row.id;
            // The pane is drawn once the row is active: focused just after.
            setTimeout(() => document.getElementById('detail-title')?.focus());
          }}
        />
        {#if activeRow}
          <aside class="detail-pane">
            <EntryDetail
              row={activeRow}
              entry={activeEntry}
              decided={active === null ? undefined : run.decisions.get(active)}
              {database}
              {busy}
              onattach={attachActive}
              onaccept={acceptActive}
              onupdatelocation={updateActiveLocation}
              oncreateanyway={createActiveAnyway}
              onreveal={() => void reveal()}
              onclose={() => (active = null)}
              onback={focusList}
            />
          </aside>
        {/if}
      </div>

      <footer class="bar">
        {#if run.running}
          <BatchProgressPanel
            progress={run.progress}
            cancelling={run.cancelling}
            onpause={() => void run.pause()}
            onresume={() => void run.resume()}
            oncancel={() => void run.cancel()}
          />
        {:else}
          {#if run.started}
            <ResultsPanel
              {run}
              canDecide={databaseWritable && !busy}
              {working}
              {message}
              {updates}
              {moves}
              {acceptances}
              onupdatelocations={() => void decideMany(updates, attachTo('updated'))}
              onattachmoved={() => void decideMany(moves, attachTo('attached'))}
              onaccept={() =>
                void decideMany(acceptances, (item) =>
                  accept(item.path, item.observed, item.verificationId),
                )}
              onexport={() => void exportReport()}
            />
          {:else if message}
            <p class={message.ok ? 'done' : 'problem'} role="status">{message.text()}</p>
          {/if}
          <section class="launch" aria-label={t().launch.region}>
            <p class="sentence" aria-live="polite">{missing ?? sentence}</p>
            <button
              type="button"
              class="primary"
              onclick={() => void start()}
              disabled={busy || missing !== null}
              aria-keyshortcuts="Control+Enter"
            >
              {action === 'create'
                ? t().launch.create
                : action === 'search'
                  ? t().launch.search
                  : t().launch.verify}
            </button>
            <span class="muted">{t().launch.shortcut}</span>
          </section>
        {/if}
      </footer>
    </div>
  </main>
</div>

<style>
  /* The window does not scroll: header, then the screen, which shares the
     height between its parts. */
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .app.dragging {
    outline: 3px dashed var(--accent);
    outline-offset: -6px;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 1.25rem;
    padding: 0.45rem 1rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    font-size: 0.85rem;
  }
  h1 {
    margin: 0;
    font-size: 1.25rem;
  }
  .portable {
    padding: 0 0.4rem;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .views {
    display: flex;
    gap: 0.1rem;
    align-self: stretch;
  }
  .views button {
    border: none;
    border-bottom: 3px solid transparent;
    border-radius: 0;
    background: none;
    font-size: 1rem;
    padding: 0.3rem 0.8rem;
  }
  /* The current screen: a bar and bold text, not only a colour. */
  .views button[aria-current='page'] {
    border-bottom-color: var(--accent);
    font-weight: 600;
  }
  .end {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-left: auto;
  }
  .language select {
    font: inherit;
    margin-left: 0.5rem;
  }
  .closing,
  .strip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.75rem;
    margin: 0;
    padding: 0.5rem 1rem;
    border-bottom: 2px solid var(--warn);
    background: var(--surface);
  }
  .closing p {
    margin: 0;
    flex-basis: 100%;
  }
  .strip.problem {
    border-bottom-color: var(--error);
  }
  .strip span {
    flex: 1 1 auto;
  }
  main {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0.75rem 1rem 0;
  }
  .files {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .files[hidden] {
    display: none;
  }
  .commands {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.75rem;
    margin-bottom: 0.6rem;
  }
  .actions {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface);
  }
  .actions label {
    padding: 0.4rem 0.9rem;
    cursor: pointer;
  }
  /* The chosen action: filled and bold, not only coloured. */
  .actions label.selected {
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
  }
  .actions input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .actions label:has(input:focus-visible) {
    outline: 2px solid var(--focus);
    outline-offset: -3px;
  }
  .settings-toggle {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.2rem 0.6rem;
    text-align: start;
    max-width: 100%;
  }
  .settings-toggle[aria-expanded='true'] {
    border-color: var(--accent);
  }
  /* The list, and its detail beside it once a file is chosen. */
  .workspace {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    gap: 1rem;
  }
  .detail-pane {
    flex: 0 0 clamp(20rem, 28%, 32rem);
    overflow: hidden auto;
    overflow-wrap: anywhere;
    padding: 0.6rem 0.8rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  /* Not wide enough for both: the detail below the list, which keeps its
     width (long names need it more than the height). */
  @media (max-width: 1400px) {
    .workspace.with-detail {
      flex-direction: column;
    }
    .detail-pane {
      flex: 0 0 auto;
      max-height: 40%;
    }
  }
  .bar {
    flex: 0 0 auto;
    max-height: 45vh;
    overflow: auto;
    margin: 0.75rem -1rem 0;
    padding: 0.6rem 1rem 0.7rem;
    background: var(--surface);
    border-top: 1px solid var(--border);
  }
  .launch {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 1rem;
  }
  .sentence {
    flex: 1 1 24rem;
    font-weight: 600;
    margin: 0;
  }
  .bar > p {
    margin: 0 0 0.4rem;
  }
  .done {
    color: var(--ok);
    font-weight: 600;
  }
</style>
