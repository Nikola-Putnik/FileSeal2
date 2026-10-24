<script lang="ts">
  import type { Tone } from '../labels';

  let { tone, icon, label }: { tone: Tone; icon: string; label: string } = $props();
</script>

<span class="badge {tone}"><span class="icon" aria-hidden="true">{icon}</span>{label}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4em;
    font-weight: 600;
  }
  .icon {
    display: inline-grid;
    place-items: center;
    width: 1.4em;
    height: 1.4em;
    border-radius: 50%;
    font-size: 0.85em;
    color: var(--on-tone);
  }
  .ok .icon {
    background: var(--ok);
  }
  .warn .icon {
    background: var(--warn);
  }
  .error .icon {
    background: var(--error);
  }
  .neutral .icon {
    background: var(--neutral);
  }
</style>
