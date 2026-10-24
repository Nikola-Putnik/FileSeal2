<script lang="ts">
  // Which extensions of the files found in folders are kept: a list typed
  // ("only these"), or a chip per extension found, with its count and size.
  import { untrack } from 'svelte';
  import { formatBytes } from '../format';
  import { t } from '../i18n/index.svelte';
  import {
    ALL_EXTENSIONS,
    keeps,
    parseExtensions,
    toggleExtension,
    type ExtensionFilter,
    type ExtensionStat,
  } from '../list';

  let {
    extensions,
    filter,
    onfilter,
    disabled,
  }: {
    extensions: ExtensionStat[];
    filter: ExtensionFilter;
    onfilter: (next: ExtensionFilter) => void;
    disabled: boolean;
  } = $props();

  /** What the user typed for "only these extensions"; starts as applied. */
  let onlyText = $state(untrack(() => (filter.only ? [...filter.only].join(' ') : '')));

  function applyOnly(event: SubmitEvent) {
    event.preventDefault();
    const typed = parseExtensions(onlyText);
    onfilter(typed.length > 0 ? { leftOut: new Set(), only: new Set(typed) } : ALL_EXTENSIONS);
  }

  function allExtensions() {
    onlyText = '';
    onfilter(ALL_EXTENSIONS);
  }

  /** `.mkv, .iso` */
  function listed(extensions: Iterable<string>): string {
    return [...extensions].map((e) => (e ? `.${e}` : t().selection.noExtension)).join(', ');
  }
</script>

<form class="only" onsubmit={applyOnly}>
  <label>
    {t().selection.only}
    <input
      type="text"
      bind:value={onlyText}
      placeholder="mkv iso mp4"
      aria-describedby="only-hint"
      spellcheck="false"
      {disabled}
    />
  </label>
  <button type="submit" {disabled}>{t().selection.apply}</button>
  <span id="only-hint" class="muted hint">{t().selection.onlyHint}</span>
</form>
{#if filter.only}
  <p class="active-filter" role="status">{t().selection.onlyActive(listed(filter.only))}</p>
{/if}
{#if extensions.length > 0}
  <div class="chips">
    {#each extensions as stat (stat.extension)}
      {@const kept = keeps(filter, stat.extension)}
      <button
        type="button"
        class="chip"
        class:kept
        aria-pressed={kept}
        {disabled}
        onclick={() => onfilter(toggleExtension(filter, stat.extension))}
      >
        <span aria-hidden="true">{kept ? '☑' : '☐'}</span>
        {t().selection.extension(
          stat.extension ? `.${stat.extension}` : t().selection.noExtension,
          stat.files,
          formatBytes(stat.bytes),
        )}
      </button>
    {/each}
  </div>
{/if}
{#if filter.only || filter.leftOut.size > 0}
  <button type="button" class="all" {disabled} onclick={allExtensions}>
    {t().selection.allExtensions}
  </button>
{/if}

<style>
  .only {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.6rem;
  }
  .only input {
    margin-left: 0.4rem;
    width: 12rem;
    font: inherit;
  }
  .hint {
    flex-basis: 100%;
    font-size: 0.9em;
  }
  .active-filter {
    margin: 0.5rem 0 0;
    font-weight: 600;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.6rem;
  }
  .chip {
    border-radius: 999px;
    padding: 0.2rem 0.7rem;
  }
  /* Kept or not: said by the box and aria-pressed, not by colour alone. */
  .chip.kept {
    border-color: var(--accent);
  }
  .chip:not(.kept) {
    color: var(--muted);
    text-decoration: line-through;
  }
  .all {
    margin-top: 0.6rem;
  }
</style>
