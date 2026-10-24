// Results of a batch: the status of each entry, and which entries a
// decision applies to (update locations, attach moved files, accept new
// versions). The rules themselves are in the core: this only reads what
// the reports say.

import type { BatchEntry } from './generated/BatchEntry';
import type { EntryOutcome } from './generated/EntryOutcome';
import type { Observation } from './generated/Observation';
import { t } from './i18n/index.svelte';
import {
  ICONS,
  createStatus,
  searchStatus,
  skipStatus,
  verifyStatus,
  type StatusLabel,
  type Tone,
} from './labels';

/** The status code, as in the CSV report and the core's summary. */
export function statusCode(outcome: EntryOutcome): string {
  if (outcome.kind === 'skipped') {
    return outcome.report.reason === 'duplicate' ? 'duplicate' : 'notProcessed';
  }
  return outcome.report.status;
}

export function statusLabel(outcome: EntryOutcome): StatusLabel {
  switch (outcome.kind) {
    case 'created':
      return createStatus(outcome.report.status);
    case 'verified':
      return verifyStatus(outcome.report.status);
    case 'searched':
      return searchStatus(outcome.report.status);
    case 'skipped':
      return skipStatus(outcome.report.reason);
  }
}

/** Label of a status code of a batch, for the summary and its filters. */
export function codeLabel(action: 'create' | 'verify' | 'search', code: string): StatusLabel {
  if (code === 'duplicate') return skipStatus('duplicate');
  if (code === 'notProcessed') return skipStatus('cancelled');
  switch (action) {
    case 'create':
      return createStatus(code as Parameters<typeof createStatus>[0]);
    case 'verify':
      return verifyStatus(code as Parameters<typeof verifyStatus>[0]);
    case 'search':
      return searchStatus(code as Parameters<typeof searchStatus>[0]);
  }
}

const TONE_ORDER: Record<Tone, number> = { error: 0, warn: 1, neutral: 2, ok: 3 };

/** Problems first: what needs attention comes at the top of a summary. */
export function byConcern(a: StatusLabel, b: StatusLabel): number {
  return TONE_ORDER[a.tone] - TONE_ORDER[b.tone];
}

/** What was read, as the report says: enough to decide without reading again. */
export function observationOf(outcome: EntryOutcome): Observation | null {
  if (outcome.kind === 'skipped') return null;
  const report = outcome.report;
  if (report.size === null || report.digests.length === 0) return null;
  const modified = outcome.kind === 'created' ? null : outcome.report.observedMtime;
  return { size: report.size, modified, digests: report.digests };
}

export interface Decision {
  /** Row the entry belongs to. */
  key: number;
  path: string;
  observed: Observation;
}

export interface Attachment extends Decision {
  itemId: number;
}

export interface Acceptance extends Decision {
  verificationId: number | null;
}

type Keyed = [number, BatchEntry];

/**
 * R2: files recognised at the same place of the same disk under another
 * drive letter, with the content recorded there: their location can be
 * updated (cadrage, rule 7, "for the whole batch").
 */
export function locationUpdates(entries: Iterable<Keyed>): Attachment[] {
  const found: Attachment[] = [];
  for (const [key, entry] of entries) {
    const outcome = entry.outcome;
    if (outcome.kind === 'skipped' || !outcome.report.locationChange) continue;
    const sameContent =
      (outcome.kind === 'verified' && outcome.report.status === 'match') ||
      (outcome.kind === 'searched' && outcome.report.status === 'locationRecognized') ||
      (outcome.kind === 'created' &&
        ['alreadyReferenced', 'completed'].includes(outcome.report.status));
    const observed = observationOf(outcome);
    const itemId = outcome.report.itemId;
    if (sameContent && observed && itemId !== null) {
      found.push({ key, path: outcome.report.path, observed, itemId });
    }
  }
  return found;
}

/**
 * R5: files whose content is a known item missing from its location, with
 * no other candidate: they can be attached to it (cadrage, rule 6). An
 * ambiguous case is never part of a batch decision.
 */
export function probableMoves(entries: Iterable<Keyed>): Attachment[] {
  const found: Attachment[] = [];
  for (const [key, entry] of entries) {
    const outcome = entry.outcome;
    if (outcome.kind === 'skipped') continue;
    const finding = outcome.report.finding;
    if (finding?.outcome !== 'probablyMoved') continue;
    const missing = finding.sameContent.filter((c) => c.presence === 'missing');
    const observed = observationOf(outcome);
    if (missing.length === 1 && observed) {
      found.push({ key, path: outcome.report.path, observed, itemId: missing[0].itemId });
    }
  }
  return found;
}

/** Verified files that differ from their reference: a new version to accept. */
export function acceptable(entries: Iterable<Keyed>): Acceptance[] {
  const found: Acceptance[] = [];
  for (const [key, entry] of entries) {
    const outcome = entry.outcome;
    if (outcome.kind !== 'verified') continue;
    const report = outcome.report;
    if (report.status !== 'mismatch' && report.status !== 'conflict') continue;
    const observed = observationOf(outcome);
    if (observed) {
      found.push({ key, path: report.path, observed, verificationId: report.verificationId });
    }
  }
  return found;
}

/** Entries by status code. */
/** A decision taken on a row after the batch, as far as its state shows. */
export interface ShownDecision {
  kind: 'attached' | 'updated' | 'accepted' | 'created' | 'failed';
}

/** The state a row shows: the decision taken on it, else its status. */
export function shownCode(entry: BatchEntry, decided: ShownDecision | undefined): string {
  return decided ? `decided:${decided.kind}` : statusCode(entry.outcome);
}

/**
 * Counts by shown state, of the rows still in the list: a row taken out no
 * longer counts, and a decision taken on a row counts under the decision.
 */
export function shownCounts(
  rows: Iterable<{ id: number }>,
  results: ReadonlyMap<number, BatchEntry>,
  decisions: ReadonlyMap<number, ShownDecision>,
): [string, number][] {
  const counts = new Map<string, number>();
  for (const row of rows) {
    const entry = results.get(row.id);
    if (!entry) continue;
    const code = shownCode(entry, decisions.get(row.id));
    counts.set(code, (counts.get(code) ?? 0) + 1);
  }
  return [...counts];
}

/** Label of a shown state: a status of the batch, or a decision taken. */
export function shownLabel(action: 'create' | 'verify' | 'search', code: string): StatusLabel {
  if (!code.startsWith('decided:')) return codeLabel(action, code);
  const kind = code.slice('decided:'.length) as ShownDecision['kind'];
  const ok = kind !== 'failed';
  return {
    tone: ok ? 'ok' : 'error',
    icon: ok ? ICONS.ok : ICONS.error,
    label: t().results.decided[kind],
    explanation: '',
  };
}

export function countByStatus(entries: Iterable<BatchEntry>): [string, number][] {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    const code = statusCode(entry.outcome);
    counts.set(code, (counts.get(code) ?? 0) + 1);
  }
  return [...counts];
}
