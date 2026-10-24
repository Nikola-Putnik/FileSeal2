<script lang="ts">
  // The head of the Databases screen: the active database's name and
  // figures, and in a menu what concerns its file: rename, show, move, back
  // up, restore one of its backups.
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { api, problemMessage } from '../api';
  import { fileName, folderOf, formatBytes } from '../format';
  import type { DatabaseInfo } from '../generated/DatabaseInfo';
  import type { Inspection } from '../generated/Inspection';
  import { t } from '../i18n/index.svelte';
  import Popover from './Popover.svelte';

  let {
    database = $bindable(),
    busy,
  }: {
    database: DatabaseInfo;
    /** An operation runs elsewhere: nothing can be changed. */
    busy: boolean;
  } = $props();

  let renaming = $state(false);
  let name = $state('');
  let working = $state(false);
  let message = $state<{ ok: boolean; text: () => string } | null>(null);
  let restoring = $state<{ path: string; inspection: Inspection } | null>(null);
  let menuOpen = $state(false);
  let disabled = $derived(busy || working || database.readOnly);

  const filters = () => [{ name: t().database.fileFilter, extensions: ['fileseal'] }];

  async function act(work: () => Promise<() => string>) {
    menuOpen = false;
    working = true;
    message = null;
    try {
      const text = await work();
      message = { ok: true, text };
    } catch (error) {
      message = { ok: false, text: () => problemMessage(error) };
    } finally {
      working = false;
    }
  }

  function rename() {
    void act(async () => {
      database = await api.databaseRename(name);
      renaming = false;
      const shown = database.name;
      return () => t().dbScreen.renamed(shown);
    });
  }

  async function move() {
    menuOpen = false;
    const target = await save({
      title: t().dbScreen.moveTitle,
      defaultPath: database.path,
      filters: filters(),
    });
    if (!target) return;
    void act(async () => {
      database = await api.databaseMove(target);
      const path = database.path;
      return () => t().dbScreen.moved(path);
    });
  }

  async function backup() {
    menuOpen = false;
    const date = new Date().toISOString().slice(0, 10);
    const target = await save({
      title: t().dbScreen.backupTitle,
      defaultPath: `${folderOf(database.path)}\\${t().dbScreen.backupName(database.name, date)}`,
      filters: filters(),
    });
    if (!target) return;
    // The dialog has already asked before replacing an existing file.
    void act(async () => {
      const report = await api.databaseBackup(target, true);
      return () => t().dbScreen.backedUp(report.path, formatBytes(report.bytes));
    });
  }

  async function chooseBackup() {
    menuOpen = false;
    const picked = await open({
      title: t().dbScreen.restoreTitle,
      multiple: false,
      directory: false,
      filters: filters(),
    });
    if (typeof picked !== 'string') return;
    message = null;
    try {
      restoring = { path: picked, inspection: await api.databaseInspect(picked) };
    } catch (error) {
      message = { ok: false, text: () => problemMessage(error) };
    }
  }

  function restore() {
    const chosen = restoring;
    if (!chosen) return;
    restoring = null;
    void act(async () => {
      const result = await api.databaseRestore(chosen.path);
      database = result.info;
      return () => t().dbScreen.restored(result.safetyCopy.path);
    });
  }
</script>

<section class="card" aria-labelledby="active-title">
  <div class="head">
    <h2 id="active-title">{database.name}</h2>
    <span class="muted">{t().dbScreen.facts(database.references, database.schemaVersion)}</span>
    <span class="path" title={database.path}>{database.path}</span>
    <Popover bind:open={menuOpen} align="end">
      {#snippet label()}{t().dbScreen.fileMenu} <span aria-hidden="true">▾</span>{/snippet}
      <div class="menu">
        <button
          type="button"
          {disabled}
          onclick={() => {
            menuOpen = false;
            name = database.name;
            renaming = true;
          }}>{t().dbScreen.rename}</button
        >
        <button
          type="button"
          onclick={() => {
            menuOpen = false;
            void api.revealInFolder(database.path);
          }}
        >
          {t().dbScreen.showInFolder}
        </button>
        <button type="button" {disabled} onclick={() => void move()}>{t().dbScreen.move}</button>
        <button type="button" disabled={busy || working} onclick={() => void backup()}>
          {t().dbScreen.backup}
        </button>
        <button type="button" {disabled} onclick={() => void chooseBackup()}>
          {t().dbScreen.restore}
        </button>
      </div>
    </Popover>
  </div>
  {#if database.readOnly}<p class="notice">{t().dbScreen.readOnly}</p>{/if}
  {#if database.onNetwork}<p class="notice">{t().database.networkHint}</p>{/if}
  {#if renaming}
    <form
      class="rename"
      onsubmit={(event) => {
        event.preventDefault();
        rename();
      }}
    >
      <label>
        {t().dbScreen.renameLabel}
        <!-- svelte-ignore a11y_autofocus -->
        <input type="text" bind:value={name} autofocus required />
      </label>
      <button type="submit" class="primary" disabled={working || !name.trim()}>
        {t().dbScreen.save}
      </button>
      <button type="button" onclick={() => (renaming = false)}>{t().dbScreen.cancel}</button>
    </form>
  {/if}
  {#if restoring}
    <div class="confirm" role="group" aria-label={t().dbScreen.restore}>
      <p>
        {t().dbScreen.restoreConfirm(
          fileName(restoring.path),
          restoring.inspection.counts.references,
          database.name,
        )}
      </p>
      <button type="button" class="primary" onclick={restore}>{t().dbScreen.restoreYes}</button>
      <button type="button" onclick={() => (restoring = null)}>{t().dbScreen.cancel}</button>
    </div>
  {/if}
  {#if message}
    <p class={message.ok ? 'done' : 'problem'} role="status">{message.text()}</p>
  {/if}
</section>

<style>
  .card {
    margin-bottom: 0.5rem;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.3rem 0.9rem;
  }
  h2 {
    margin: 0;
    font-size: 1.15rem;
  }
  .head .path {
    flex: 1 1 12rem;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .menu {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .menu button {
    text-align: start;
  }
  .card p {
    margin: 0.4rem 0 0;
  }
  .rename {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
    margin-top: 0.5rem;
  }
  .rename input {
    font: inherit;
    margin-left: 0.4rem;
  }
  .confirm {
    border: 1px solid var(--warn);
    border-radius: 8px;
    padding: 0.5rem 0.75rem;
    margin-top: 0.6rem;
  }
  .confirm button {
    margin-right: 0.5rem;
  }
  .done {
    color: var(--ok);
    font-weight: 600;
  }
</style>
