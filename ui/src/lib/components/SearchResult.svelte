<script lang="ts">
  import type { Candidate } from '../generated/Candidate';
  import type { SearchReport } from '../generated/SearchReport';
  import { fileName, formatBytes } from '../format';
  import { t } from '../i18n/index.svelte';
  import { searchStatus } from '../labels';
  import Digests from './Digests.svelte';
  import FindingPanel from './FindingPanel.svelte';
  import ProblemText from './ProblemText.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let {
    report,
    busy = false,
    onattach,
    onupdatelocation,
  }: {
    report: SearchReport;
    busy?: boolean;
    onattach?: (candidate: Candidate, allowDifferent: boolean) => void;
    onupdatelocation?: () => void;
  } = $props();
  let status = $derived(searchStatus(report.status));
</script>

<p class="headline">
  <StatusBadge tone={status.tone} icon={status.icon} label={status.label} />
  — {fileName(report.path)}
  {#if report.size !== null}<span class="muted">({formatBytes(report.size)})</span>{/if}
</p>
{#if report.problem}
  <ProblemText problem={report.problem} />
{:else}
  <p>{status.explanation}</p>
{/if}
{#if report.locationChange && onupdatelocation}
  <button type="button" disabled={busy} onclick={onupdatelocation}>
    {t().finding.updateLocation(report.locationChange)}
  </button>
{/if}
{#if report.finding}
  <FindingPanel finding={report.finding} {busy} {onattach} />
{/if}
{#if report.digests.length > 0}
  <h3>{t().result.observedDigests}</h3>
  <Digests digests={report.digests} />
{/if}
