<script lang="ts">
  // What a batch came to: counts, why it stopped (if it did), decisions
  // that can be taken for all the results at once, and the report export.
  import { problemMessage } from '../api';
  import { t } from '../i18n/index.svelte';
  import type { Acceptance, Attachment } from '../results';
  import type { BatchRun } from '../run.svelte';
  import Confirm from './Confirm.svelte';

  let {
    run,
    canDecide,
    working,
    message,
    updates,
    moves,
    acceptances,
    onupdatelocations,
    onattachmoved,
    onaccept,
    onexport,
  }: {
    run: BatchRun;
    /** A writable database is open and nothing is running. */
    canDecide: boolean;
    working: { done: number; total: number } | null;
    message: { ok: boolean; text: () => string } | null;
    updates: Attachment[];
    moves: Attachment[];
    acceptances: Acceptance[];
    onupdatelocations: () => void;
    onattachmoved: () => void;
    onaccept: () => void;
    onexport: () => void;
  } = $props();

  let summary = $derived(run.summary);
  let busy = $derived(!canDecide || working !== null);
</script>

<section class="results" aria-labelledby="results-title">
  <h2 id="results-title" class="sr-only">{t().results.title}</h2>
  {#if run.error}<p class="problem" role="alert">{problemMessage(run.error)}</p>{/if}

  {#if summary}
    <div class="line">
      <p class="summary" role="status">
        {t().results.counts(
          summary.total - summary.notProcessed - summary.cancelled,
          summary.total,
        )}
        — {t().results.categories(
          summary.processed + summary.duplicates,
          summary.failed,
          summary.cancelled,
          summary.notProcessed,
        )}
      </p>
      {#if updates.length > 0 || moves.length > 0 || acceptances.length > 0}
        <div class="actions" role="group" aria-label={t().results.forAll}>
          <span class="muted">{t().results.forAll}</span>
          {#if updates.length > 0}
            <Confirm
              label={t().results.updateLocations(updates.length)}
              question={t().results.confirmUpdate(updates.length)}
              disabled={busy}
              onconfirm={onupdatelocations}
            />
          {/if}
          {#if moves.length > 0}
            <Confirm
              label={t().results.attachMoved(moves.length)}
              question={t().results.confirmAttach(moves.length)}
              disabled={busy}
              onconfirm={onattachmoved}
            />
          {/if}
          {#if acceptances.length > 0}
            <Confirm
              label={t().results.acceptVersions(acceptances.length)}
              question={t().results.confirmAccept(acceptances.length)}
              disabled={busy}
              onconfirm={onaccept}
            />
          {/if}
        </div>
      {/if}
      <button type="button" class="export" onclick={onexport} disabled={run.running}>
        {t().report.export}
      </button>
    </div>
    {#if summary.databaseProblem}
      <p class="problem" role="alert">
        {t().results.databaseLost(problemMessage(summary.databaseProblem))}
      </p>
    {/if}
    {#if summary.lostVolumes.length > 0}
      <p class="warning">{t().results.volumesLost(summary.lostVolumes.join(', '))}</p>
    {/if}
    {#if summary.stoppedByUser}<p>{t().results.stopped}</p>{/if}
  {/if}
  {#if working}
    <p role="status">{t().results.working(working.done, working.total)}</p>
  {/if}
  {#if message}
    <p class={message.ok ? 'done' : 'problem'} role="status">{message.text()}</p>
  {/if}
</section>

<style>
  .results p {
    margin: 0.2rem 0;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 1rem;
  }
  .summary {
    font-weight: 600;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }
  .export {
    margin-left: auto;
  }
  .warning {
    color: var(--warn);
    font-weight: 600;
  }
  .done {
    color: var(--ok);
    font-weight: 600;
  }
</style>
