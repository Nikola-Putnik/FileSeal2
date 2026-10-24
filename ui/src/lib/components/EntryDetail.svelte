<script lang="ts">
  // Everything about the file chosen in the list: where it is, and the full
  // result of the batch for it, with the decisions it allows. Shown beside
  // the list; Enter on a row brings the focus here, Escape takes it back.
  import { fileName, formatBytes } from '../format';
  import type { BatchEntry } from '../generated/BatchEntry';
  import type { Candidate } from '../generated/Candidate';
  import type { DatabaseInfo } from '../generated/DatabaseInfo';
  import { t } from '../i18n/index.svelte';
  import { ICONS, skipStatus } from '../labels';
  import type { Row } from '../list';
  import type { Decided } from '../run.svelte';
  import CreateResult from './CreateResult.svelte';
  import ProblemText from './ProblemText.svelte';
  import SearchResult from './SearchResult.svelte';
  import StatusBadge from './StatusBadge.svelte';
  import VerifyResult from './VerifyResult.svelte';

  let {
    row,
    entry,
    decided,
    database,
    busy,
    onattach,
    onaccept,
    onupdatelocation,
    oncreateanyway,
    onreveal,
    onclose,
    onback,
  }: {
    row: Row | null;
    entry: BatchEntry | undefined;
    decided: Decided | undefined;
    database: DatabaseInfo | null;
    busy: boolean;
    onattach: (candidate: Candidate, allowDifferent: boolean) => void;
    onaccept: () => void;
    onupdatelocation: () => void;
    oncreateanyway: () => void;
    onreveal: () => void;
    onclose: () => void;
    /** Escape: the focus goes back to the list. */
    onback: () => void;
  } = $props();

  // Decisions need a database that can be written, and are taken once.
  let open = $derived(!busy && !decided && database !== null && !database.readOnly);
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="detail"
  aria-labelledby="detail-title"
  onkeydown={(event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onback();
    }
  }}
>
  <div class="title">
    <h2 id="detail-title" tabindex="-1">{t().results.details}</h2>
    <button type="button" class="close" title={t().results.closeDetails} onclick={onclose}>
      <span aria-hidden="true">✕</span><span class="sr-only">{t().results.closeDetails}</span>
    </button>
  </div>
  {#if !row}
    <p class="muted">{t().results.noActive}</p>
  {:else}
    <p class="where">
      <strong>{fileName(row.path)}</strong>
      <span class="muted">— {formatBytes(row.size)}</span>
      <button type="button" onclick={onreveal}>{t().results.reveal}</button>
    </p>
    <p class="path">{row.path}</p>
    {#if row.listed}
      <p class="muted">{t().result.listedFrom(row.listed.line, row.listed.list)}</p>
    {/if}
    {#if decided}
      <p>
        <StatusBadge
          tone={decided.ok ? 'ok' : 'error'}
          icon={decided.ok ? ICONS.ok : ICONS.error}
          label={t().results.decided[decided.kind]}
        />
        {#if decided.detail}<span>{decided.detail()}</span>{/if}
      </p>
    {/if}

    {#if !entry}
      <p class="muted">{t().results.notRun}</p>
    {:else if entry.outcome.kind === 'created'}
      <CreateResult
        report={entry.outcome.report}
        databaseName={database?.name ?? null}
        {busy}
        onattach={open ? onattach : undefined}
        oncreateanyway={open ? oncreateanyway : undefined}
        onupdatelocation={open ? onupdatelocation : undefined}
      />
    {:else if entry.outcome.kind === 'verified'}
      <VerifyResult
        report={entry.outcome.report}
        databaseName={database?.name ?? null}
        {busy}
        onaccept={!busy && !decided ? onaccept : undefined}
        onattach={open ? onattach : undefined}
        onupdatelocation={open ? onupdatelocation : undefined}
      />
    {:else if entry.outcome.kind === 'searched'}
      <SearchResult
        report={entry.outcome.report}
        {busy}
        onattach={open ? onattach : undefined}
        onupdatelocation={open ? onupdatelocation : undefined}
      />
    {:else}
      {@const skip = entry.outcome.report}
      {@const status = skipStatus(skip.reason)}
      <p class="headline">
        <StatusBadge tone={status.tone} icon={status.icon} label={status.label} />
      </p>
      <p>{status.explanation}</p>
      {#if skip.sameAs}<p>{t().results.sameAs(skip.sameAs.path)}</p>{/if}
      {#if skip.problem}<ProblemText problem={skip.problem} />{/if}
    {/if}
  {/if}
</section>

<style>
  .title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .title h2 {
    margin: 0;
  }
  .close {
    padding: 0.15rem 0.5rem;
  }
  .where {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.5rem;
    margin-bottom: 0.2rem;
  }
  .where button {
    margin-left: auto;
  }
</style>
