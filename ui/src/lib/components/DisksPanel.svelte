<script lang="ts">
  // The disks of the database and where they are now. A disk back under
  // another drive letter is updated in one confirmed click; any folder
  // whose files moved together can be reassigned after a preview (its form
  // shown when asked for).
  import { open } from '@tauri-apps/plugin-dialog';
  import { tick } from 'svelte';
  import { api, problemMessage } from '../api';
  import { formatCount } from '../format';
  import type { DatabaseInfo } from '../generated/DatabaseInfo';
  import type { ReassignPreview } from '../generated/ReassignPreview';
  import type { VolumeState } from '../generated/VolumeState';
  import type { VolumeStatus } from '../generated/VolumeStatus';
  import { t } from '../i18n/index.svelte';
  import { ICONS, type Tone } from '../labels';
  import Confirm from './Confirm.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let {
    database,
    busy,
    onchanged,
  }: {
    database: DatabaseInfo;
    busy: boolean;
    /** Locations changed: the database's figures and lists are refreshed. */
    onchanged: () => void;
  } = $props();

  let volumes = $state<VolumeStatus[] | null>(null);
  let loading = $state(false);
  let working = $state(false);
  let message = $state<{ ok: boolean; text: () => string } | null>(null);
  let from = $state('');
  let to = $state('');
  let includeMissing = $state(false);
  let preview = $state<ReassignPreview | null>(null);
  let toInput = $state<HTMLInputElement | null>(null);
  let reassigning = $state(false);
  let disabled = $derived(busy || working || database.readOnly);

  async function load() {
    loading = true;
    try {
      volumes = await api.databaseVolumes();
    } catch (error) {
      message = { ok: false, text: () => problemMessage(error) };
    } finally {
      loading = false;
    }
  }

  // Loaded when shown, and again when the database changes.
  $effect(() => {
    void database.path;
    void load();
  });

  function stateLabel(state: VolumeState): { tone: Tone; icon: string; label: string } {
    switch (state.kind) {
      case 'connected':
        return { tone: 'ok', icon: ICONS.ok, label: t().dbScreen.volume.connected };
      case 'movedTo':
        return { tone: 'warn', icon: ICONS.warn, label: t().dbScreen.volume.movedTo(state.root) };
      case 'notConnected':
        return { tone: 'neutral', icon: '–', label: t().dbScreen.volume.notConnected };
      case 'unidentified':
        return {
          tone: 'neutral',
          icon: ICONS.neutral,
          label: state.present
            ? t().dbScreen.volume.unidentifiedPresent
            : t().dbScreen.volume.unidentified,
        };
    }
  }

  async function act(work: () => Promise<() => string>) {
    working = true;
    message = null;
    try {
      const text = await work();
      message = { ok: true, text };
      onchanged();
      await load();
    } catch (error) {
      message = { ok: false, text: () => problemMessage(error) };
    } finally {
      working = false;
    }
  }

  /** The disk is the same, under another letter: its places on it hold. */
  function updateTo(volume: VolumeStatus, root: string) {
    void act(async () => {
      const report = await api.databaseReassign({
        from: volume.recordedRoot,
        to: root,
        serial: volume.serial,
        includeMissing: false,
      });
      return () => t().dbScreen.updated(report.moved, root);
    });
  }

  async function reassignFrom(volume: VolumeStatus) {
    from = volume.recordedRoot;
    to = '';
    preview = null;
    reassigning = true;
    await tick();
    toInput?.focus();
  }

  async function chooseTo() {
    const picked = await open({
      title: t().dbScreen.chooseTitle,
      directory: true,
      multiple: false,
    });
    if (typeof picked === 'string') {
      to = picked;
      preview = null;
    }
  }

  async function showPreview() {
    message = null;
    try {
      preview = await api.databaseReassignPreview({ from, to, serial: null, includeMissing });
    } catch (error) {
      preview = null;
      message = { ok: false, text: () => problemMessage(error) };
    }
  }

  function apply() {
    void act(async () => {
      const report = await api.databaseReassign({ from, to, serial: null, includeMissing });
      preview = null;
      return () => t().dbScreen.reassigned(report.moved, report.left, report.taken.length);
    });
  }

  let toMove = $derived(preview ? (includeMissing ? preview.files : preview.found) : 0);
</script>

<section aria-labelledby="disks-title">
  <h2 id="disks-title" class="sr-only">{t().dbScreen.disks}</h2>
  <div class="title">
    <p class="muted">{t().dbScreen.disksHint}</p>
    <button type="button" onclick={() => void load()} disabled={loading}>
      {t().dbScreen.refresh}
    </button>
    <button
      type="button"
      aria-expanded={reassigning}
      aria-controls="reassign"
      {disabled}
      onclick={() => (reassigning = !reassigning)}>{t().dbScreen.reassignOpen}</button
    >
  </div>

  {#if volumes && volumes.length === 0}
    <p class="muted">{t().dbScreen.disksEmpty}</p>
  {:else if volumes}
    <table>
      <caption class="sr-only">{t().dbScreen.disksGrid}</caption>
      <thead>
        <tr>
          <th scope="col">{t().dbScreen.columns.root}</th>
          <th scope="col" class="number">{t().dbScreen.columns.files}</th>
          <th scope="col">{t().dbScreen.columns.state}</th>
          <th scope="col"><span class="sr-only">{t().dbScreen.reassign}</span></th>
        </tr>
      </thead>
      <tbody>
        {#each volumes as volume (volume.serial + volume.recordedRoot)}
          {@const state = stateLabel(volume.state)}
          <tr>
            <th scope="row" class="root">{volume.recordedRoot}</th>
            <td class="number">{formatCount(volume.files)}</td>
            <td><StatusBadge tone={state.tone} icon={state.icon} label={state.label} /></td>
            <td class="row-actions">
              {#if volume.state.kind === 'movedTo'}
                {@const root = volume.state.root}
                <Confirm
                  label={t().dbScreen.update(root)}
                  question={t().dbScreen.confirmUpdate(volume.files, volume.recordedRoot, root)}
                  {disabled}
                  onconfirm={() => updateTo(volume, root)}
                />
              {/if}
              <button type="button" {disabled} onclick={() => void reassignFrom(volume)}>
                {t().dbScreen.reassign}
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if reassigning}
    <div id="reassign" class="reassign-box">
      <h3>{t().dbScreen.reassignTitle}</h3>
      <p class="muted">{t().dbScreen.reassignHint}</p>
      <form
        class="reassign"
        onsubmit={(event) => {
          event.preventDefault();
          void showPreview();
        }}
      >
        <label>
          {t().dbScreen.from}
          <input type="text" bind:value={from} oninput={() => (preview = null)} required />
        </label>
        <label>
          {t().dbScreen.to}
          <input
            type="text"
            bind:value={to}
            bind:this={toInput}
            oninput={() => (preview = null)}
            required
          />
        </label>
        <button type="button" {disabled} onclick={() => void chooseTo()}
          >{t().dbScreen.choose}</button
        >
        <label class="check">
          <input type="checkbox" bind:checked={includeMissing} onchange={() => (preview = null)} />
          {t().dbScreen.includeMissing}
        </label>
        <button type="submit" disabled={disabled || !from.trim() || !to.trim()}>
          {t().dbScreen.preview}
        </button>
      </form>
      {#if preview}
        <p role="status">{t().dbScreen.previewed(preview.files, preview.found, preview.missing)}</p>
        {#if preview.examples.length > 0}
          <ul class="examples">
            {#each preview.examples as example (example.from)}
              <li>
                <span class="path">{example.from}</span> →
                <span class="path">{example.to}</span>
                {#if !example.found}<span class="muted">({t().dbScreen.notFoundThere})</span>{/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if toMove > 0}
          <Confirm
            label={t().dbScreen.apply}
            question={t().dbScreen.confirmApply(toMove, from, to)}
            {disabled}
            onconfirm={apply}
          />
        {/if}
      {/if}
    </div>
  {/if}
  {#if message}
    <p class={message.ok ? 'done' : 'problem'} role="status">{message.text()}</p>
  {/if}
</section>

<style>
  .title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.75rem;
    margin-bottom: 0.5rem;
  }
  .title p {
    flex: 1 1 20rem;
    margin: 0;
  }
  .reassign-box {
    margin-top: 1rem;
    padding: 0.5rem 0.75rem 0.75rem;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  .reassign-box h3 {
    margin-top: 0.2rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: start;
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }
  .root {
    font-weight: 600;
    font-family: var(--mono);
  }
  .number {
    text-align: end;
    font-variant-numeric: tabular-nums;
  }
  .row-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    justify-content: flex-end;
  }
  .reassign {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.8rem;
  }
  .reassign input[type='text'] {
    font: inherit;
    margin-left: 0.4rem;
    min-width: 16rem;
  }
  .examples {
    margin: 0.3rem 0 0.6rem;
    padding-left: 1.2rem;
  }
  .done {
    color: var(--ok);
    font-weight: 600;
  }
</style>
