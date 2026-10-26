<script lang="ts">
  // A new version, said under the header until installed or put off. The
  // installed version installs it (never during a run: installing closes
  // FileSeal); the portable version points to the download page.
  import { api, problemMessage } from '../api';
  import { formatBytes } from '../format';
  import { t } from '../i18n/index.svelte';
  import { installUpdate, updates } from '../updates.svelte';

  let {
    portable,
    busy,
    current,
  }: {
    portable: boolean;
    /** A run is going on. */
    busy: boolean;
    /** The version in use. */
    current: string | null;
  } = $props();

  let notesOpen = $state(false);
  let amount = $derived.by(() => {
    const progress = updates.progress;
    if (!progress) return '';
    return progress.total
      ? `${Math.round((progress.done * 100) / progress.total)} %`
      : formatBytes(progress.done);
  });
</script>

{#if updates.available && (!updates.dismissed || updates.installing)}
  {@const available = updates.available}
  <div class="update" role="status" aria-live="polite">
    <span class="text">
      {#if updates.installing && updates.progress && updates.progress.done < (updates.progress.total ?? Infinity)}
        {t().updates.downloading(amount)}
      {:else if updates.installing}
        {t().updates.installing}
      {:else}
        <strong>{t().updates.available(available.version)}</strong>
        {#if current}<span class="muted">{t().updates.current(current)}</span>{/if}
      {/if}
    </span>
    {#if !updates.installing}
      {#if available.notes}
        <button type="button" aria-expanded={notesOpen} onclick={() => (notesOpen = !notesOpen)}
          >{t().updates.notes}</button
        >
      {/if}
      {#if portable}
        <button type="button" class="primary" onclick={() => void api.openReleasePage()}
          >{t().updates.download}</button
        >
      {:else}
        <button
          type="button"
          class="primary"
          disabled={busy}
          title={busy ? t().updates.busy : undefined}
          onclick={() => void installUpdate()}>{t().updates.install}</button
        >
      {/if}
      <button type="button" onclick={() => (updates.dismissed = true)}>{t().updates.later}</button>
    {/if}
    {#if notesOpen}<p class="notes">{available.notes}</p>{/if}
    {#if updates.error}
      <p class="problem">{t().updates.failed(problemMessage(updates.error))}</p>
    {/if}
  </div>
{/if}

<style>
  .update {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.75rem;
    padding: 0.5rem 1rem;
    border-bottom: 2px solid var(--accent);
    background: var(--surface);
  }
  .text {
    flex: 1 1 auto;
  }
  .text .muted {
    margin-left: 0.4rem;
  }
  .notes,
  .problem {
    flex-basis: 100%;
    margin: 0;
    white-space: pre-wrap;
  }
</style>
