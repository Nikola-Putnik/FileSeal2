<script lang="ts">
  import type { Candidate } from '../generated/Candidate';
  import type { CreateReport } from '../generated/CreateReport';
  import type { WriteOutcome } from '../generated/WriteOutcome';
  import { fileName, formatBytes } from '../format';
  import { t } from '../i18n/index.svelte';
  import { ALGORITHM_NAMES, ICONS, createStatus, noteText, writeOutcome } from '../labels';
  import Digests from './Digests.svelte';
  import FindingPanel from './FindingPanel.svelte';
  import ProblemText from './ProblemText.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let {
    report,
    databaseName,
    busy = false,
    onattach,
    oncreateanyway,
    onupdatelocation,
  }: {
    report: CreateReport;
    databaseName: string | null;
    busy?: boolean;
    onattach?: (candidate: Candidate, allowDifferent: boolean) => void;
    oncreateanyway?: () => void;
    onupdatelocation?: () => void;
  } = $props();
  let status = $derived(createStatus(report.status));

  // "Not written" needs its reason: a conflict elsewhere, or a file that
  // could not be read (cancelled, locked…).
  function outcomeLabel(outcome: WriteOutcome): string {
    if (outcome !== 'skipped') return writeOutcome(outcome).label;
    if (report.status === 'conflict') return t().result.skippedConflict;
    if (report.status === 'skipped' || report.status === 'imported') {
      return t().result.skippedReferenced;
    }
    return t().result.skippedUnread;
  }
</script>

<p class="headline">
  <StatusBadge tone={status.tone} icon={status.icon} label={status.label} />
  — {fileName(report.path)}
  {#if report.size !== null}<span class="muted">({formatBytes(report.size)})</span>{/if}
</p>
<p>{status.explanation}</p>
{#if report.problem}<ProblemText problem={report.problem} />{/if}
{#if report.locationChange && onupdatelocation}
  <button type="button" disabled={busy} onclick={onupdatelocation}>
    {t().finding.updateLocation(report.locationChange)}
  </button>
{/if}
{#if report.finding}
  <FindingPanel finding={report.finding} {busy} {onattach} />
{/if}
{#if report.status === 'needsDecision' && oncreateanyway}
  <button type="button" disabled={busy} onclick={oncreateanyway}>{t().finding.createAnyway}</button>
{/if}

{#if report.sidecars.length > 0 || report.database}
  <h3>{t().result.destinations}</h3>
  <ul class="details">
    {#each report.sidecars as sidecar (sidecar.algorithm)}
      {@const outcome = writeOutcome(sidecar.outcome)}
      <li>
        <span>
          {t().result.hashFile(ALGORITHM_NAMES[sidecar.algorithm])}
          {#if !sidecar.requested}<span class="muted">{t().result.existingChecked}</span>{/if}
        </span>
        <StatusBadge
          tone={outcome.tone}
          icon={ICONS[outcome.tone]}
          label={outcomeLabel(sidecar.outcome)}
        />
        <div class="path">{sidecar.path}</div>
        {#if sidecar.existing && sidecar.outcome === 'conflict'}
          <div>{t().result.existingValue} <code>{sidecar.existing}</code></div>
        {/if}
        {#each sidecar.notes as note, i (i)}<div class="muted">{noteText(note)}</div>{/each}
        {#if sidecar.problem && sidecar.outcome === 'invalid'}
          <div class="muted">{t().result.invalidDetail(sidecar.problem.detail)}</div>
        {:else if sidecar.problem}
          <ProblemText problem={sidecar.problem} />
        {/if}
      </li>
    {/each}
    {#if report.database}
      {@const db = report.database}
      {@const outcome = writeOutcome(db.outcome)}
      <li>
        <span>{t().result.database(databaseName ?? '?')}</span>
        <StatusBadge
          tone={outcome.tone}
          icon={ICONS[outcome.tone]}
          label={outcomeLabel(db.outcome)}
        />
        {#if db.outcome === 'completed'}
          <div class="muted">
            {t().result.algorithmAdded(db.added.map((a) => ALGORITHM_NAMES[a]).join(', '))}
          </div>
        {/if}
        {#if db.outcome === 'conflict'}
          <div>
            {t().result.recordedReference(
              db.existingSize === null ? '' : ` (${formatBytes(db.existingSize)})`,
            )}
            {#each db.existing as digest (digest.algorithm)}
              <div>{ALGORITHM_NAMES[digest.algorithm]} <code>{digest.hex}</code></div>
            {/each}
          </div>
        {/if}
        {#if db.problem}<ProblemText problem={db.problem} />{/if}
      </li>
    {/if}
  </ul>
{/if}

<h3>{t().result.computedDigests}</h3>
{#if report.digests.length > 0}
  <Digests digests={report.digests} />
{:else}
  <p class="muted">{t().result.noDigest}</p>
{/if}
