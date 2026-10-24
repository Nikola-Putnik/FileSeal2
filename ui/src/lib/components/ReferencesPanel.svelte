<script lang="ts">
  // The references of the database, searched by name, folder or hash, and
  // the history of the one chosen: its versions, where it was, and every
  // verification, in a pane beside the list. References can be removed
  // from the database (several at once, or every result of a search) and
  // put back.
  import { api, problemMessage } from '../api';
  import { fileName, folderOf, formatBytes, formatDate } from '../format';
  import type { DatabaseInfo } from '../generated/DatabaseInfo';
  import type { ItemHistory } from '../generated/ItemHistory';
  import type { ItemsChanged } from '../generated/ItemsChanged';
  import type { Listed } from '../generated/Listed';
  import type { ReferencePage } from '../generated/ReferencePage';
  import type { ReferenceRow } from '../generated/ReferenceRow';
  import type { VerifyStatus } from '../generated/VerifyStatus';
  import type { Column } from '../grid';
  import { t } from '../i18n/index.svelte';
  import { ICONS, VERIFY_ORDER, verifyStatus, type StatusLabel } from '../labels';
  import Confirm from './Confirm.svelte';
  import Digests from './Digests.svelte';
  import MiddleText from './MiddleText.svelte';
  import StatusBadge from './StatusBadge.svelte';
  import VirtualGrid from './VirtualGrid.svelte';

  let {
    database,
    refresh,
    busy,
    onchanged,
  }: {
    database: DatabaseInfo;
    refresh: number;
    /** An operation runs: nothing can be changed. */
    busy: boolean;
    /** Items removed or put back: the database is read again. */
    onchanged: () => void;
  } = $props();

  let query = $state('');
  let page = $state.raw<ReferencePage | null>(null);
  let problem = $state<unknown>(null);
  let active = $state<number | null>(null);
  let history = $state.raw<ItemHistory | null>(null);
  let listed = $state<Listed>('active');
  let selected = $state.raw<Set<number>>(new Set());
  let working = $state(false);
  // A function, so the text follows a change of language.
  let notice = $state<(() => string) | null>(null);

  let selectedIds = $derived(
    page ? page.rows.map((row) => row.itemId).filter((id) => selected.has(id)) : [],
  );
  let locked = $derived(busy || working || database.readOnly);

  const columns = $derived<Column[]>([
    { id: 'name', label: t().dbScreen.columns.name, width: 'minmax(12rem, 3fr)' },
    { id: 'folder', label: t().dbScreen.columns.folder, width: 'minmax(7rem, 1.5fr)' },
    { id: 'size', label: t().dbScreen.columns.size, width: '6.5rem', align: 'end' },
    {
      id: 'recorded',
      label: listed === 'retired' ? t().dbScreen.columns.retired : t().dbScreen.columns.recorded,
      width: '9.5rem',
    },
    { id: 'check', label: t().dbScreen.columns.lastCheck, width: 'minmax(15rem, 2fr)' },
  ]);

  // Searched at once when shown or when the database changes; shortly
  // after typing stops otherwise.
  let searched: string | null = null;
  $effect(() => {
    const text = query;
    void database.path;
    void refresh;
    void listed;
    const typing = searched !== null && text !== searched;
    searched = text;
    const timer = setTimeout(() => void search(text), typing ? 250 : 0);
    return () => clearTimeout(timer);
  });

  async function search(text: string) {
    try {
      page = await api.databaseBrowse(text, listed);
      problem = null;
    } catch (error) {
      problem = error;
    }
  }

  // The history follows the active row (arrows, click).
  $effect(() => {
    const item = active;
    if (item === null) {
      history = null;
      return;
    }
    const timer = setTimeout(() => {
      api
        .databaseHistory(item)
        .then((found) => {
          if (active === item) history = found;
        })
        .catch((error: unknown) => (problem = error));
    }, 120);
    return () => clearTimeout(timer);
  });

  /** A result code of the history, in words (codes of verifications). */
  function outcome(code: string): StatusLabel {
    if ((VERIFY_ORDER as string[]).includes(code)) return verifyStatus(code as VerifyStatus);
    return { tone: 'neutral', icon: ICONS.neutral, label: code, explanation: '' };
  }

  /** Shown in, or left out of, the database: what was listed is gone. */
  function show(next: Listed) {
    listed = next;
    selected = new Set();
    active = null;
    notice = null;
  }

  async function change(kind: 'remove' | 'putBack', ids: number[] | null) {
    working = true;
    try {
      // null: every result of the search, not only those listed.
      const items = ids ?? (await api.databaseMatching(query, listed));
      const done: ItemsChanged =
        kind === 'remove' ? await api.databaseRemove(items) : await api.databasePutBack(items);
      notice =
        kind === 'remove'
          ? () => t().dbScreen.removed(done.changed)
          : () => t().dbScreen.putBack(done.changed, done.taken.length);
      problem = null;
      selected = new Set();
      active = null;
      onchanged();
    } catch (error) {
      problem = error;
    } finally {
      working = false;
    }
  }

  function origin(code: string): string {
    const origins = t().dbScreen.origin;
    return code in origins ? origins[code as keyof typeof origins] : code;
  }
</script>

<section class="references" aria-labelledby="references-title">
  <h2 id="references-title" class="sr-only">{t().dbScreen.references}</h2>
  <div class="toolbar">
    <input
      class="search"
      type="search"
      bind:value={query}
      aria-label={t().dbScreen.search}
      placeholder={t().dbScreen.searchHint}
    />
    <div class="listed" role="radiogroup" aria-label={t().dbScreen.listed.label}>
      {#each ['active', 'retired'] as const as choice (choice)}
        <label>
          <input
            type="radio"
            name="listed"
            checked={listed === choice}
            onchange={() => show(choice)}
          />
          {t().dbScreen.listed[choice]}
        </label>
      {/each}
    </div>
    {#if page}
      <span class="muted" role="status">{t().dbScreen.shown(page.rows.length, page.total)}</span>
      <div class="changes">
        {#if listed === 'active'}
          {#if selectedIds.length > 0}
            <Confirm
              label={t().dbScreen.removeSelected(selectedIds.length)}
              question={t().dbScreen.confirmRemove(selectedIds.length, database.name)}
              disabled={locked}
              onconfirm={() => void change('remove', selectedIds)}
            />
          {/if}
          {#if query.trim() && page.total > 0}
            <Confirm
              label={t().dbScreen.removeMatching(page.total)}
              question={t().dbScreen.confirmRemove(page.total, database.name)}
              disabled={locked}
              onconfirm={() => void change('remove', null)}
            />
          {/if}
        {:else}
          {#if selectedIds.length > 0}
            <Confirm
              label={t().dbScreen.putBackSelected(selectedIds.length)}
              question={t().dbScreen.confirmPutBack(selectedIds.length)}
              disabled={locked}
              onconfirm={() => void change('putBack', selectedIds)}
            />
          {/if}
          {#if page.total > 0}
            <Confirm
              label={t().dbScreen.putBackMatching(page.total)}
              question={t().dbScreen.confirmPutBack(page.total)}
              disabled={locked}
              onconfirm={() => void change('putBack', null)}
            />
          {/if}
        {/if}
      </div>
    {/if}
  </div>
  {#if problem}<p class="problem" role="alert">{problemMessage(problem)}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice()}</p>{/if}

  <div class="workspace">
    {#if page}
      <VirtualGrid
        label={t().dbScreen.referencesGrid}
        fill
        description={t().dbScreen.selectHint}
        rows={page.rows}
        rowKey={(row) => row.itemId}
        {columns}
        bind:active
        {selected}
        onselect={(keys) => (selected = keys)}
        onopen={() => setTimeout(() => document.getElementById('history-title')?.focus())}
        empty={listed === 'retired' ? t().dbScreen.noRetired : t().dbScreen.noReference}
      >
        {#snippet cell(row: ReferenceRow, column: string)}
          {#if column === 'name'}
            <MiddleText text={fileName(row.path)} />
          {:else if column === 'folder'}
            <MiddleText text={folderOf(row.path)} muted />
          {:else if column === 'size'}
            {formatBytes(row.size)}
          {:else if column === 'recorded'}
            {formatDate(listed === 'retired' ? (row.retiredAt ?? row.createdAt) : row.createdAt)}
          {:else if row.lastOutcome && row.lastCheckedAt}
            {@const label = outcome(row.lastOutcome)}
            <span class="muted">{formatDate(row.lastCheckedAt)}</span>
            <StatusBadge tone={label.tone} icon={label.icon} label={label.label} />
          {:else}
            <span class="muted">{t().dbScreen.never}</span>
          {/if}
        {/snippet}
      </VirtualGrid>
    {/if}
    {#if history}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <aside
        class="history"
        aria-labelledby="history-title"
        onkeydown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            document.querySelector<HTMLElement>('.references [role="grid"]')?.focus();
          }
        }}
      >
        <div class="title">
          <h3 id="history-title" tabindex="-1">{t().dbScreen.history}</h3>
          <button
            type="button"
            class="close"
            title={t().results.closeDetails}
            onclick={() => (active = null)}
          >
            <span aria-hidden="true">✕</span><span class="sr-only">{t().results.closeDetails}</span>
          </button>
        </div>
        <p class="path">{history.locations[0]?.path ?? ''}</p>
        <p class="muted">{t().dbScreen.itemSince(formatDate(history.createdAt))}</p>
        {#if history.retiredAt}
          <p><strong>{t().dbScreen.retiredOn(formatDate(history.retiredAt))}</strong></p>
        {/if}

        <h4>{t().dbScreen.versions}</h4>
        <ul class="details">
          {#each history.versions as version (version.referenceId)}
            <li>
              <span>{formatDate(version.createdAt)}</span>
              <span class="muted">
                — {origin(version.origin)}, {formatBytes(version.size)}
                {#if version.supersededAt}
                  — {t().dbScreen.replacedOn(formatDate(version.supersededAt))}
                {:else}
                  — <strong>{t().dbScreen.current}</strong>
                {/if}
              </span>
              <Digests digests={version.digests} />
            </li>
          {/each}
        </ul>

        <h4>{t().dbScreen.locations}</h4>
        <ul class="details">
          {#each history.locations as location, i (i)}
            <li>
              <span class="path">{location.path}</span>
              {#if location.current}<strong> — {t().dbScreen.current}</strong>{/if}
              <div class="muted">
                {t().dbScreen.seen(
                  formatDate(location.firstSeenAt),
                  formatDate(location.lastSeenAt),
                )}
                {#if location.missingSince}
                  — {t().dbScreen.missingSince(formatDate(location.missingSince))}
                {/if}
              </div>
            </li>
          {/each}
        </ul>

        <h4>{t().dbScreen.checks}</h4>
        {#if history.verifications.length === 0}
          <p class="muted">{t().dbScreen.noChecks}</p>
        {:else}
          <ul class="details">
            {#each history.verifications as check, i (i)}
              {@const label = outcome(check.outcome)}
              <li>
                <span>{formatDate(check.finishedAt)}</span>
                <StatusBadge tone={label.tone} icon={label.icon} label={label.label} />
                <div class="path">{check.observedPath}</div>
                {#if check.detail}<div class="muted">{check.detail}</div>{/if}
              </li>
            {/each}
          </ul>
          {#if history.moreVerifications}<p class="muted">{t().dbScreen.moreChecks}</p>{/if}
        {/if}
      </aside>
    {/if}
  </div>
</section>

<style>
  /* Fills the screen's height: the list takes what the toolbar leaves. */
  .references {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.9rem;
    margin-bottom: 0.5rem;
  }
  .search {
    font: inherit;
    min-width: 20rem;
    padding: 0.35rem 0.5rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }
  .listed {
    display: flex;
    gap: 0.8rem;
  }
  .changes {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 0.6rem;
  }
  .notice,
  .problem {
    margin: 0 0 0.4rem;
  }
  .notice {
    font-weight: 600;
  }
  .workspace {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    gap: 1rem;
  }
  .history {
    flex: 0 0 clamp(20rem, 30%, 34rem);
    overflow: hidden auto;
    overflow-wrap: anywhere;
    padding: 0.6rem 0.8rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  .title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .title h3 {
    margin: 0;
  }
  .close {
    padding: 0.15rem 0.5rem;
  }
  h4 {
    margin: 0.8rem 0 0.3rem;
  }
  @media (max-width: 1400px) {
    .workspace {
      flex-direction: column;
    }
    .history {
      flex: 0 0 auto;
      max-height: 45%;
    }
  }
</style>
