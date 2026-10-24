<script lang="ts">
  // A long text cut in the middle when it does not fit, not at its end:
  // the end of a file name (episode, extension) stays in view, and two names
  // that differ only there no longer look the same. The whole text is in
  // the page (screen readers read it all) and in the tooltip.
  import { splitEnd } from '../format';

  let { text, muted = false }: { text: string; muted?: boolean } = $props();
  let [head, tail] = $derived(splitEnd(text));
</script>

<span class="middle" class:muted title={text}
  ><span class="head">{head}</span><span class="tail">{tail}</span></span
>

<style>
  .middle {
    display: flex;
    min-width: 0;
    max-width: 100%;
  }
  .head {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre;
  }
  .tail {
    flex: none;
    white-space: pre;
  }
</style>
