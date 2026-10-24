<script lang="ts">
  // Progress of a running batch, in the bar at the bottom of the window:
  // overall (weighted by bytes), then the figures (files, volume, rate,
  // times) and the file read on each disk; pause, resume, cancel.
  import { fileName, formatBytes, formatDuration, formatRate } from '../format';
  import type { BatchProgress } from '../generated/BatchProgress';
  import { t } from '../i18n/index.svelte';

  let {
    progress,
    cancelling,
    onpause,
    onresume,
    oncancel,
  }: {
    progress: BatchProgress | null;
    cancelling: boolean;
    onpause: () => void;
    onresume: () => void;
    oncancel: () => void;
  } = $props();

  const percent = (done: number, total: number) =>
    total > 0 ? Math.min(100, Math.floor((done / total) * 100)) : 0;
  let overall = $derived(progress ? percent(progress.bytesDone, progress.bytesTotal) : 0);
  let reading = $derived(
    (progress?.current ?? [])
      .map(
        (file) =>
          `${fileName(file.path)} (${t().progress.percent(percent(file.done, file.total))})`,
      )
      .join(', '),
  );
</script>

<section class="progress" aria-labelledby="progress-title">
  <h2 id="progress-title" class="sr-only">{t().progress.title}</h2>
  <div class="line">
    <progress max="100" value={overall} aria-label={t().progress.overallBar}></progress>
    <strong class="percent">{t().progress.percent(overall)}</strong>
    {#if progress?.paused}
      <button type="button" class="primary" onclick={onresume} disabled={cancelling}>
        {t().launch.resume}
      </button>
    {:else}
      <button type="button" onclick={onpause} disabled={cancelling}>{t().launch.pause}</button>
    {/if}
    <button type="button" onclick={oncancel} disabled={cancelling}>
      {cancelling ? t().launch.cancelling : t().launch.cancel}
    </button>
  </div>
  {#if progress?.paused}
    <p class="paused" role="status"><span aria-hidden="true">⏸</span> {t().progress.paused}</p>
  {/if}
  {#if progress}
    <dl class="figures">
      <div>
        <dt>{t().progress.files}</dt>
        <dd>{t().progress.filesValue(progress.filesDone, progress.filesTotal)}</dd>
      </div>
      <div>
        <dt>{t().progress.volume}</dt>
        <dd>
          {t().progress.volumeValue(
            formatBytes(progress.bytesDone),
            formatBytes(progress.bytesTotal),
          )}
        </dd>
      </div>
      <div>
        <dt>{t().progress.rate}</dt>
        <dd>
          {progress.bytesPerSecond === null
            ? t().progress.measuring
            : formatRate(progress.bytesPerSecond)}
        </dd>
      </div>
      <div>
        <dt>{t().progress.elapsed}</dt>
        <dd>{formatDuration(progress.elapsedMs)}</dd>
      </div>
      <div>
        <dt>{t().progress.remaining}</dt>
        <dd>
          {progress.remainingMs === null
            ? t().progress.estimating
            : formatDuration(progress.remainingMs)}
        </dd>
      </div>
    </dl>
    {#if reading}<p class="reading muted" title={reading}>
        {t().progress.readingList(reading)}
      </p>{/if}
  {/if}
</section>

<style>
  .line {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  progress {
    flex: 1 1 auto;
    height: 0.9rem;
    accent-color: var(--accent);
  }
  .percent {
    min-width: 3.5rem;
    font-variant-numeric: tabular-nums;
  }
  .paused {
    margin: 0.3rem 0 0;
    font-weight: 600;
  }
  /* The figures on one line: "Files 12 done out of 50 · Volume read …". */
  .figures {
    display: flex;
    flex-wrap: wrap;
    gap: 0.1rem 1.25rem;
    margin: 0.35rem 0 0;
  }
  .figures dt {
    display: inline;
    color: var(--muted);
    margin-right: 0.35rem;
  }
  .figures dd {
    display: inline;
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
  .reading {
    margin: 0.25rem 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
