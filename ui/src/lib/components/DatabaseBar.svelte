<script module lang="ts">
  // A message about the database (opening failed, updated with a copy).
  export type BarMessage = { tone: 'problem' | 'notice'; text: () => string };
</script>

<script lang="ts">
  // The active database, as one button in the window's header: its state
  // (read-only, network) stays in view; the recent list, creating, opening,
  // locating and closing are in the panel it opens. Reopens the last
  // database when the window opens. Its messages are shown by the window.
  import { documentDir, join } from '@tauri-apps/api/path';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { api, problemMessage } from '../api';
  import { fileName } from '../format';
  import type { DatabaseInfo } from '../generated/DatabaseInfo';
  import type { RecentDatabaseStatus } from '../generated/RecentDatabaseStatus';
  import { t } from '../i18n/index.svelte';
  import Popover from './Popover.svelte';

  let {
    database = $bindable(),
    running,
    onmessage,
  }: {
    database: DatabaseInfo | null;
    running: boolean;
    /** A message to show, or none any more. */
    onmessage: (message: BarMessage | null) => void;
  } = $props();

  let recent = $state<RecentDatabaseStatus[]>([]);
  let menuOpen = $state(false);

  // The recent list read again each time the panel opens.
  $effect(() => {
    if (menuOpen) void refreshRecent();
  });

  async function refreshRecent() {
    try {
      recent = await api.databaseRecent();
    } catch {
      recent = [];
    }
  }

  function opened(info: DatabaseInfo) {
    database = info;
    const copy = info.migrationBackup;
    onmessage(copy ? { tone: 'notice', text: () => t().database.migrated(copy) } : null);
  }

  function failed(path: string, error: unknown) {
    onmessage({
      tone: 'problem',
      text: () => t().database.openFailed(fileName(path), problemMessage(error)),
    });
  }

  async function openPath(path: string): Promise<boolean> {
    menuOpen = false;
    try {
      opened(await api.databaseOpen(path));
      return true;
    } catch (e) {
      database = await api.databaseCurrent().catch(() => null);
      failed(path, e);
      return false;
    } finally {
      await refreshRecent();
    }
  }

  async function chooseAndOpen(): Promise<string | null> {
    menuOpen = false;
    const path = await open({
      title: t().database.openTitle,
      multiple: false,
      directory: false,
      filters: [{ name: t().database.fileFilter, extensions: ['fileseal'] }],
    });
    return typeof path === 'string' ? path : null;
  }

  async function openDatabase() {
    const path = await chooseAndOpen();
    if (path) await openPath(path);
  }

  /** A recent database that moved: find it, then replace the old entry. */
  async function locate(entry: RecentDatabaseStatus) {
    const path = await chooseAndOpen();
    if (path && (await openPath(path)) && path.toLowerCase() !== entry.path.toLowerCase()) {
      recent = await api.databaseForget(entry.path);
    }
  }

  async function createDatabase() {
    menuOpen = false;
    let defaultPath = t().database.defaultFileName;
    try {
      // Portable: beside the program, on the USB key; else Documents.
      const folder = (await api.appInfo()).databaseFolder ?? (await documentDir());
      defaultPath = await join(folder, defaultPath);
    } catch {
      // No Documents folder: the dialog starts wherever the system wants.
    }
    const path = await save({
      title: t().database.createTitle,
      defaultPath,
      filters: [{ name: t().database.fileFilter, extensions: ['fileseal'] }],
    });
    if (!path) return;
    try {
      opened(await api.databaseCreate(path, fileName(path).replace(/\.fileseal$/i, '')));
    } catch (e) {
      failed(path, e);
    }
    await refreshRecent();
  }

  async function closeDatabase() {
    menuOpen = false;
    try {
      await api.databaseClose();
      database = null;
      onmessage(null);
    } catch (e) {
      onmessage({ tone: 'problem', text: () => problemMessage(e) });
    }
  }

  async function forget(path: string) {
    recent = await api.databaseForget(path);
  }

  onMount(() => {
    void (async () => {
      try {
        const info = await api.databaseReopenLast();
        if (info) opened(info);
      } catch (e) {
        const settings = await api.settingsGet().catch(() => null);
        const last = settings?.lastDatabase ?? '';
        // Its name as the user knows it, from the recent list.
        const name = settings?.recentDatabases.find((r) => r.path === last)?.name ?? fileName(last);
        onmessage({
          tone: 'problem',
          text: () => t().database.reopenFailed(name, problemMessage(e)),
        });
      }
      await refreshRecent();
    })();
  });
</script>

<div class="database" role="group" aria-label={t().database.region}>
  <Popover bind:open={menuOpen} wide align="end">
    {#snippet label()}
      {#if database}
        {t().database.current} <strong>{database.name}</strong>
        <span class="muted">({t().database.references(database.references)})</span>
      {:else}
        <span class="muted">{t().database.none}</span>
      {/if}
      <span aria-hidden="true">▾</span>
    {/snippet}
    {#if database}<p class="path current">{database.path}</p>{/if}
    <div class="buttons">
      <button type="button" onclick={createDatabase} disabled={running}>
        {t().database.create}
      </button>
      <button type="button" onclick={openDatabase} disabled={running}>
        {t().database.open}
      </button>
      {#if database}
        <button type="button" onclick={closeDatabase} disabled={running}>
          {t().database.close}
        </button>
      {/if}
    </div>
    <h3>{t().database.recent}</h3>
    <ul>
      {#each recent as entry (entry.path)}
        <li>
          <button
            type="button"
            class="entry"
            disabled={running || !entry.exists}
            onclick={() => openPath(entry.path)}
          >
            <strong>{entry.name}</strong>
            {#if !entry.exists}<span class="muted">{t().database.notFound}</span>{/if}
            <span class="path">{entry.path}</span>
          </button>
          {#if !entry.exists}
            <button type="button" disabled={running} onclick={() => locate(entry)}>
              {t().database.locate}
            </button>
          {/if}
          <button
            type="button"
            class="forget"
            aria-label={t().database.forget(entry.name)}
            title={t().database.forget(entry.name)}
            onclick={() => forget(entry.path)}>✕</button
          >
        </li>
      {:else}
        <li class="muted">{t().database.noRecent}</li>
      {/each}
    </ul>
  </Popover>
  {#if database?.readOnly}
    <span class="badge" title={t().database.readOnlyHint}>🔒 {t().database.readOnly}</span>
  {/if}
  {#if database?.onNetwork}
    <span class="badge warn" title={t().database.networkHint}>⚠ {t().database.network}</span>
  {/if}
</div>

<style>
  .database {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }
  .current {
    margin: 0 0 0.5rem;
  }
  .badge {
    font-size: 0.85em;
    padding: 0.05rem 0.45rem;
    border-radius: 999px;
    border: 1px solid var(--border);
  }
  .badge.warn {
    border-color: var(--warn);
    color: var(--warn);
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  h3 {
    margin: 0.75rem 0 0.3rem;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.15rem 0;
  }
  .entry {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    gap: 0 0.5rem;
    text-align: left;
    border-color: transparent;
  }
  .entry .path {
    flex-basis: 100%;
    overflow-wrap: anywhere;
  }
  .forget {
    padding: 0.2rem 0.5rem;
  }
</style>
