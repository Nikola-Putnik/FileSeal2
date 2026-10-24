<script lang="ts">
  // The settings of the next batch, shown when asked (the button above
  // sums them up): algorithms and destinations (create), references
  // (verify), and how folders are looked through.
  import type { DatabaseInfo } from '../generated/DatabaseInfo';
  import { t } from '../i18n/index.svelte';
  import type { ActionKind } from '../run.svelte';
  import type { RunSettings } from '../runSettings';

  let {
    id,
    action,
    settings = $bindable(),
    database,
    disabled,
    onfolderoptions,
    haslisted,
  }: {
    id: string;
    action: ActionKind;
    settings: RunSettings;
    database: DatabaseInfo | null;
    disabled: boolean;
    /** Subfolders or hidden files changed: the folders are looked through again. */
    onfolderoptions: () => void;
    /** Some files come from hash files (verified against their lines). */
    haslisted: boolean;
  } = $props();

  let databaseWritable = $derived(database !== null && !database.readOnly);
</script>

<section {id} class="settings" aria-labelledby="{id}-title">
  <h2 id="{id}-title" class="sr-only">{t().settings.title}</h2>
  <div class="columns">
    {#if action === 'create'}
      <fieldset>
        <legend>{t().settings.algorithms}</legend>
        <label>
          <input type="checkbox" bind:checked={settings.algorithms.sha256} {disabled} /> SHA-256
          <span class="muted">{t().settings.recommended}</span>
        </label>
        <label>
          <input type="checkbox" bind:checked={settings.algorithms.md5} {disabled} /> MD5
          <span class="muted">{t().settings.md5Note}</span>
        </label>
      </fieldset>
      <fieldset>
        <legend>{t().settings.destinations}</legend>
        <label>
          <input type="checkbox" bind:checked={settings.writeSidecar} {disabled} />
          {t().settings.sidecarNextToFile}
        </label>
        <label>
          <input
            type="checkbox"
            bind:checked={settings.useDatabase}
            disabled={disabled || !databaseWritable}
          />
          {t().settings.activeDatabase}
          {#if !database}
            <span class="muted">{t().settings.noDatabase}</span>
          {:else if database.readOnly}
            <span class="muted">{t().settings.readOnlyDatabase}</span>
          {/if}
        </label>
      </fieldset>
      <fieldset>
        <legend>{t().settings.existing.title}</legend>
        {#each ['skipComplete', 'skipAny', 'read'] as const as choice (choice)}
          <label>
            <input type="radio" bind:group={settings.existing} value={choice} {disabled} />
            {t().settings.existing[choice]}
          </label>
        {/each}
        <label>
          <input
            type="checkbox"
            bind:checked={settings.importHashFiles}
            disabled={disabled ||
              settings.existing === 'read' ||
              !settings.useDatabase ||
              !databaseWritable}
          />
          {t().settings.existing.importHashFiles}
        </label>
        <p class="muted hint">{t().settings.existing.hint}</p>
      </fieldset>
    {:else if action === 'verify'}
      <fieldset>
        <legend>{t().settings.referencesToUse}</legend>
        <label>
          <input type="checkbox" bind:checked={settings.useSidecar} {disabled} />
          {t().settings.sidecarsNextToFile}
        </label>
        <label>
          <input
            type="checkbox"
            bind:checked={settings.useDatabase}
            disabled={disabled || !database}
          />
          {t().settings.activeDatabase}
          {#if !database}<span class="muted">{t().settings.noDatabase}</span>{/if}
        </label>
        {#if haslisted}<p class="muted">{t().settings.listedReferences}</p>{/if}
      </fieldset>
    {:else}
      <fieldset>
        <legend>{t().settings.referencesToUse}</legend>
        <p>{database ? t().launch.whereDatabase(database.name) : t().launch.needDatabase}</p>
      </fieldset>
    {/if}

    <fieldset>
      <legend>{t().selection.title}</legend>
      <label>
        <input
          type="checkbox"
          bind:checked={settings.recursive}
          onchange={onfolderoptions}
          {disabled}
        />
        {t().selection.subfolders}
      </label>
      <label>
        <input
          type="checkbox"
          bind:checked={settings.includeHidden}
          onchange={onfolderoptions}
          {disabled}
        />
        {t().selection.hidden}
      </label>
      <p class="muted hint">{t().selection.optionsHint}</p>
      <label class="inline">
        {t().selection.parallel}
        <select bind:value={settings.parallelVolumes} {disabled}>
          {#each [1, 2, 3, 4] as count (count)}<option value={count}>{count}</option>{/each}
        </select>
      </label>
      <p class="muted hint">{t().selection.parallelHint}</p>
    </fieldset>
  </div>
</section>

<style>
  /* Set apart from the list: a tinted panel, opened above it. */
  .settings {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.5rem 0.75rem 0.1rem;
    margin-bottom: 0.6rem;
    /* Opened, it must leave room for the list. */
    max-height: 40vh;
    overflow: auto;
    flex: none;
  }
  .columns {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 24rem));
    gap: 0 1.25rem;
  }
  fieldset {
    border: none;
    margin: 0 0 0.5rem;
    padding: 0;
  }
  legend {
    font-weight: 600;
    padding: 0;
    margin-bottom: 0.15rem;
  }
  fieldset label {
    display: block;
    margin: 0.2rem 0;
  }
  .hint {
    margin: 0 0 0.3rem;
    font-size: 0.9em;
  }
  .inline select {
    margin-left: 0.4rem;
    font: inherit;
  }
</style>
