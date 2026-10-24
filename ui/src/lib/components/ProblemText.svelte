<script lang="ts">
  // A problem as a sentence in the current language, with the technical
  // detail (often the system's own message) kept secondary.
  import type { Problem } from '../generated/Problem';
  import { t } from '../i18n/index.svelte';
  import { problemText } from '../labels';

  let { problem, prefix = '' }: { problem: Problem; prefix?: string } = $props();
</script>

<div class="problem">
  {prefix}{problemText(problem.kind)}
  {#if problem.kind !== 'cancelled'}
    <div class="muted technical">{t().result.technical(problem.detail)}</div>
  {/if}
</div>

<style>
  .technical {
    font-size: 0.85em;
  }
</style>
