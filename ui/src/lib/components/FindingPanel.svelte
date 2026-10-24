<script lang="ts">
  // What the database knows about a content: the same content elsewhere
  // (with where each copy stands), and the same name with another content
  // (hints only). Offers to attach when the user can decide.
  import type { Candidate } from '../generated/Candidate';
  import type { ContentFinding } from '../generated/ContentFinding';
  import { t } from '../i18n/index.svelte';
  import { presenceText } from '../labels';
  import Confirm from './Confirm.svelte';

  let {
    finding,
    busy = false,
    onattach,
  }: {
    finding: ContentFinding;
    busy?: boolean;
    /** Absent when no decision can be taken here (read-only database…). */
    onattach?: (candidate: Candidate, allowDifferent: boolean) => void;
  } = $props();
</script>

{#if finding.sameContent.length > 0}
  <h3>{t().finding.sameContent}</h3>
  <ul class="details">
    {#each finding.sameContent as candidate (candidate.itemId)}
      <li>
        <span class="path">{candidate.path}</span>
        <span class="muted">— {presenceText(candidate.presence)}</span>
        {#if onattach && candidate.presence === 'missing'}
          <div>
            <button type="button" disabled={busy} onclick={() => onattach(candidate, false)}>
              {finding.outcome === 'ambiguous' ? t().finding.attachThis : t().finding.attachMoved}
            </button>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

{#if finding.hints.length > 0}
  <h3>{t().finding.sameName}</h3>
  <ul class="details">
    {#each finding.hints as candidate (candidate.itemId)}
      <li>
        <span class="path">{candidate.path}</span>
        <span class="muted">— {presenceText(candidate.presence)}</span>
        {#if onattach}
          <div>
            <Confirm
              label={t().finding.attachDifferent}
              question={t().finding.confirmAttachDifferent(candidate.path)}
              disabled={busy}
              onconfirm={() => onattach(candidate, true)}
            />
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}
