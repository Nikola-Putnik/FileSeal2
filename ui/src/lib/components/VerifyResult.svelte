<script lang="ts">
  import type { Candidate } from '../generated/Candidate';
  import type { VerifyReport } from '../generated/VerifyReport';
  import { fileName, formatBytes, formatDate } from '../format';
  import { t } from '../i18n/index.svelte';
  import { ALGORITHM_NAMES, ICONS, noteText, sourceOutcome, verifyStatus } from '../labels';
  import Confirm from './Confirm.svelte';
  import Digests from './Digests.svelte';
  import FindingPanel from './FindingPanel.svelte';
  import ProblemText from './ProblemText.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let {
    report,
    databaseName,
    busy = false,
    onaccept,
    onattach,
    onupdatelocation,
  }: {
    report: VerifyReport;
    databaseName: string | null;
    busy?: boolean;
    /** Decisions are offered only where the caller can take them. */
    onaccept?: () => void;
    onattach?: (candidate: Candidate, allowDifferent: boolean) => void;
    onupdatelocation?: () => void;
  } = $props();
  let acceptable = $derived(
    report.digests.length > 0 && (report.status === 'mismatch' || report.status === 'conflict'),
  );
  let status = $derived(verifyStatus(report.status));
</script>

<p class="headline">
  <StatusBadge tone={status.tone} icon={status.icon} label={status.label} />
  — {fileName(report.path)}
  {#if report.size !== null}<span class="muted">({formatBytes(report.size)})</span>{/if}
</p>
<!-- A problem says the same thing as the status, but more precisely. -->
{#if report.problem}
  <ProblemText problem={report.problem} />
{:else if report.status === 'noReference' && report.finding?.sameContent.length}
  <p>{t().finding.knownElsewhere}</p>
{:else}
  <p>{status.explanation}</p>
{/if}
{#if report.recordProblem}
  <ProblemText problem={report.recordProblem} prefix={t().result.notRecorded} />
{/if}
{#if acceptable && onaccept}
  <Confirm
    label={t().accept.button}
    question={t().accept.confirm(fileName(report.path))}
    disabled={busy}
    onconfirm={onaccept}
  />
{/if}
{#if report.locationChange && onupdatelocation}
  <button type="button" disabled={busy} onclick={onupdatelocation}>
    {t().finding.updateLocation(report.locationChange)}
  </button>
{/if}
{#if report.finding}
  <FindingPanel finding={report.finding} {busy} {onattach} />
{/if}

{#if report.sources.length > 0}
  <h3>{t().result.referencesUsed}</h3>
  <ul class="details">
    {#each report.sources as source, i (i)}
      {@const outcome = sourceOutcome(source.outcome)}
      <li>
        <span>
          {#if source.source === 'sidecar'}
            {t().result.hashFile(
              source.expected.map((d) => ALGORITHM_NAMES[d.algorithm]).join(', '),
            )}
          {:else}
            {t().result.database(databaseName ?? '?')}
          {/if}
        </span>
        <StatusBadge tone={outcome.tone} icon={ICONS[outcome.tone]} label={outcome.label} />
        <div class="path">{source.path}</div>
        {#if source.referenceCreatedAt}
          <div class="muted">{t().result.referenceOf(formatDate(source.referenceCreatedAt))}</div>
        {/if}
        {#if source.outcome !== 'match'}
          {#each source.expected as digest (digest.algorithm)}
            <div>
              {t().result.expected(ALGORITHM_NAMES[digest.algorithm])} <code>{digest.hex}</code>
            </div>
          {/each}
        {/if}
        {#if source.expectedSize !== null && report.size !== null && source.expectedSize !== report.size}
          <div>
            {t().result.sizeChanged(formatBytes(source.expectedSize), formatBytes(report.size))}
          </div>
        {/if}
        {#if source.outcome === 'invalid' && source.detail}
          <div class="muted">{t().result.invalidDetail(source.detail)}</div>
        {/if}
        {#each source.notes as note, n (n)}<div class="muted">{noteText(note)}</div>{/each}
      </li>
    {/each}
  </ul>
{/if}

{#if report.digests.length > 0}
  <h3>{t().result.observedDigests}</h3>
  <Digests digests={report.digests} />
{/if}
