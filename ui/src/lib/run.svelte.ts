// A batch run as the page sees it: the list it was started on, results as
// they arrive (by row), progress, summary, and the decisions taken on the
// results afterwards.

import { SvelteMap } from 'svelte/reactivity';
import { api } from './api';
import type { BatchAction } from './generated/BatchAction';
import type { BatchEntry } from './generated/BatchEntry';
import type { BatchProgress } from './generated/BatchProgress';
import type { BatchSummary } from './generated/BatchSummary';
import type { CurrentFile } from './generated/CurrentFile';
import { toBatchFiles, type Row } from './list';

export type ActionKind = BatchAction['kind'];

/** A decision taken on one result (attach, accept…), shown in its row. */
export interface Decided {
  ok: boolean;
  kind: 'attached' | 'updated' | 'accepted' | 'created' | 'failed';
  /** Why it failed, as a sentence (follows the language). */
  detail: (() => string) | null;
}

export class BatchRun {
  action = $state<ActionKind>('verify');
  /** The list the batch was started on, in its order. */
  rows = $state.raw<Row[]>([]);
  /** Row id → result. */
  results = new SvelteMap<number, BatchEntry>();
  decisions = new SvelteMap<number, Decided>();
  progress = $state.raw<BatchProgress | null>(null);
  summary = $state.raw<BatchSummary | null>(null);
  running = $state(false);
  cancelling = $state(false);
  error = $state.raw<unknown>(null);

  get paused(): boolean {
    return this.progress?.paused ?? false;
  }

  /** Whether results are shown (or arriving). */
  get started(): boolean {
    return this.running || this.summary !== null || this.error !== null;
  }

  async start(rows: Row[], action: BatchAction, parallelVolumes: number, useDatabase: boolean) {
    this.clear();
    this.action = action.kind;
    this.rows = rows;
    this.running = true;
    try {
      await api.runBatch(toBatchFiles(rows), { action, parallelVolumes }, useDatabase, (update) => {
        for (const entry of update.entries) {
          const row = this.rows[entry.index];
          if (row) this.results.set(row.id, entry);
        }
        this.progress = update.progress;
        if (update.summary) this.summary = update.summary;
      });
    } catch (error) {
      this.error = error;
    } finally {
      this.running = false;
      this.cancelling = false;
    }
  }

  async pause() {
    await api.pauseJob();
  }

  async resume() {
    await api.resumeJob();
  }

  async cancel() {
    this.cancelling = true;
    // A paused batch must wake up to see that it is cancelled.
    await api.cancelJob();
  }

  /** The file of this row being read now, if it is. */
  reading(rowId: number): CurrentFile | undefined {
    return this.progress?.current.find((c) => this.rows[c.index]?.id === rowId);
  }

  clear() {
    this.rows = [];
    this.results.clear();
    this.decisions.clear();
    this.progress = null;
    this.summary = null;
    this.error = null;
  }
}
