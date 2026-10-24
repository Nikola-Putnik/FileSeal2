<script lang="ts">
  // The Databases screen: the active database in a line (its file's actions
  // in a menu), then its references or its disks, one at a time.
  import { api } from '../api';
  import type { DatabaseInfo } from '../generated/DatabaseInfo';
  import { t } from '../i18n/index.svelte';
  import DatabaseCard from './DatabaseCard.svelte';
  import DisksPanel from './DisksPanel.svelte';
  import ReferencesPanel from './ReferencesPanel.svelte';

  let { database = $bindable(), busy }: { database: DatabaseInfo | null; busy: boolean } = $props();

  type Section = 'references' | 'disks';
  let section = $state<Section>('references');

  /** Bumped when locations or items change, so that the references are read again. */
  let refresh = $state(0);

  async function changed() {
    refresh += 1;
    try {
      database = await api.databaseCurrent();
    } catch {
      // The bar keeps what it showed; the next action will say what is wrong.
    }
  }

  function sectionKeys(event: KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    section = section === 'references' ? 'disks' : 'references';
    document.getElementById(`section-${section}`)?.focus();
  }
</script>

{#if !database}
  <p class="muted">{t().dbScreen.none}</p>
{:else}
  <div class="screen">
    <DatabaseCard bind:database {busy} />
    <div class="tabs" role="tablist" aria-label={t().dbScreen.sections}>
      {#each [['references', t().dbScreen.references], ['disks', t().dbScreen.disks]] as [id, title] (id)}
        <button
          type="button"
          role="tab"
          id="section-{id}"
          aria-selected={section === id}
          aria-controls="panel-{id}"
          tabindex={section === id ? 0 : -1}
          onclick={() => (section = id as Section)}
          onkeydown={sectionKeys}>{title}</button
        >
      {/each}
    </div>
    {#if section === 'references'}
      <div id="panel-references" class="panel" role="tabpanel" aria-labelledby="section-references">
        <ReferencesPanel {database} {refresh} {busy} onchanged={() => void changed()} />
      </div>
    {:else}
      <div id="panel-disks" class="panel scroll" role="tabpanel" aria-labelledby="section-disks">
        <DisksPanel {database} {busy} onchanged={() => void changed()} />
      </div>
    {/if}
  </div>
{/if}

<style>
  .screen,
  .panel {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .panel.scroll {
    display: block;
    overflow: auto;
    padding-bottom: 1rem;
  }
  .tabs {
    display: flex;
    gap: 0.1rem;
    border-bottom: 1px solid var(--border);
    margin: 0.4rem 0 0.6rem;
  }
  .tabs button {
    border: none;
    border-bottom: 3px solid transparent;
    border-radius: 0;
    background: none;
    padding: 0.35rem 0.8rem;
  }
  /* The current part: a bar and bold text, not only a colour. */
  .tabs button[aria-selected='true'] {
    border-bottom-color: var(--accent);
    font-weight: 600;
  }
</style>
