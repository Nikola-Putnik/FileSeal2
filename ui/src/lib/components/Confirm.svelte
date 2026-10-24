<script lang="ts">
  // A button whose action needs an explicit confirmation, asked in the page
  // (not a system dialog): readable, keyboard-friendly, and testable.
  import { t } from '../i18n/index.svelte';

  let {
    label,
    question,
    disabled = false,
    onconfirm,
  }: { label: string; question: string; disabled?: boolean; onconfirm: () => void } = $props();

  let asking = $state(false);
</script>

{#if asking}
  <div class="confirm" role="group" aria-label={label}>
    <p>{question}</p>
    <button
      type="button"
      class="primary"
      {disabled}
      onclick={() => {
        asking = false;
        onconfirm();
      }}>{t().confirm.yes}</button
    >
    <button type="button" onclick={() => (asking = false)}>{t().confirm.no}</button>
  </div>
{:else}
  <button type="button" {disabled} onclick={() => (asking = true)}>{label}</button>
{/if}

<style>
  .confirm {
    border: 1px solid var(--warn);
    border-radius: 8px;
    padding: 0.5rem 0.75rem;
    margin: 0.4rem 0;
  }
  .confirm p {
    margin: 0 0 0.5rem;
  }
  .confirm button {
    margin-right: 0.5rem;
  }
</style>
