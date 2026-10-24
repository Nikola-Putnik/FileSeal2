<script lang="ts">
  import type { Digest } from '../generated/Digest';
  import { ALGORITHM_NAMES } from '../labels';

  let { digests }: { digests: Digest[] } = $props();
</script>

{#if digests.length > 0}
  <dl class="digests">
    {#each digests as digest (digest.algorithm)}
      <dt>{ALGORITHM_NAMES[digest.algorithm]}</dt>
      <dd><code>{digest.hex}</code></dd>
    {/each}
  </dl>
{/if}

<style>
  .digests {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.2rem 1rem;
    margin: 0.5rem 0 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  code {
    font-family: var(--mono);
    user-select: all;
  }
</style>
