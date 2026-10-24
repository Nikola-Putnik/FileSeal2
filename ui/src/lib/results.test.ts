import { describe, expect, it } from 'vitest';
import type { BatchEntry } from './generated/BatchEntry';
import type { ContentFinding } from './generated/ContentFinding';
import type { EntryOutcome } from './generated/EntryOutcome';
import type { SearchReport } from './generated/SearchReport';
import type { VerifyReport } from './generated/VerifyReport';
import {
  acceptable,
  codeLabel,
  countByStatus,
  locationUpdates,
  observationOf,
  probableMoves,
  shownCounts,
  statusCode,
} from './results';

const digests = [{ algorithm: 'sha256' as const, hex: 'a'.repeat(64) }];

function verified(path: string, changes: Partial<VerifyReport>): EntryOutcome {
  return {
    kind: 'verified',
    report: {
      path,
      status: 'match',
      size: 10,
      digests,
      observedMtime: '2026-10-22T10:00:00.000Z',
      sources: [],
      problem: null,
      recordProblem: null,
      itemId: 7,
      verificationId: 70,
      locationChange: null,
      finding: null,
      relocatedFrom: null,
      ...changes,
    },
  };
}

function searched(path: string, changes: Partial<SearchReport>): EntryOutcome {
  return {
    kind: 'searched',
    report: {
      path,
      status: 'unknown',
      size: 10,
      digests,
      observedMtime: null,
      itemId: null,
      locationChange: null,
      finding: null,
      problem: null,
      ...changes,
    },
  };
}

const moved = (...presences: ('missing' | 'present' | 'volumeUnavailable')[]): ContentFinding => ({
  outcome: 'probablyMoved',
  sameContent: presences.map((presence, i) => ({
    itemId: 100 + i,
    path: `E:\\old ${i}.mkv`,
    presence,
    size: 10,
  })),
  hints: [],
});

function entries(...outcomes: EntryOutcome[]): [number, BatchEntry][] {
  return outcomes.map((outcome, index) => [
    index + 1,
    { index, path: `D:\\${index}.mkv`, outcome },
  ]);
}

describe('batch results', () => {
  it('gives each entry its status code, as the report does', () => {
    expect(statusCode(verified('a', { status: 'mismatch' }))).toBe('mismatch');
    expect(
      statusCode({
        kind: 'skipped',
        report: { reason: 'volumeUnavailable', sameAs: null, problem: null },
      }),
    ).toBe('notProcessed');
    expect(
      statusCode({
        kind: 'skipped',
        report: { reason: 'duplicate', sameAs: { index: 0, path: 'x' }, problem: null },
      }),
    ).toBe('duplicate');
    expect(codeLabel('create', 'notProcessed').label).toBe('Not processed');
    expect(codeLabel('verify', 'match').tone).toBe('ok');
    const list = entries(
      verified('a', {}),
      verified('b', {}),
      verified('c', { status: 'mismatch' }),
    );
    expect(countByStatus(list.map(([, e]) => e))).toEqual([
      ['match', 2],
      ['mismatch', 1],
    ]);
  });

  it('counts what the rows still in the list show, decisions included', () => {
    const results = new Map(
      entries(
        verified('a', {}),
        verified('b', {}),
        verified('c', { status: 'mismatch' }),
        verified('d', { status: 'mismatch' }),
      ),
    );
    const decisions = new Map([[4, { kind: 'accepted' as const }]]);
    // Row 2 was taken out of the list; row 5 was added after the batch.
    const rows = [{ id: 1 }, { id: 3 }, { id: 4 }, { id: 5 }];
    expect(shownCounts(rows, results, decisions)).toEqual([
      ['match', 1],
      ['mismatch', 1],
      ['decided:accepted', 1],
    ]);
    expect(shownCounts([{ id: 5 }], results, decisions)).toEqual([]);
  });

  it('keeps what was read, to decide without reading again', () => {
    expect(observationOf(verified('a', {}))).toEqual({
      size: 10,
      modified: '2026-10-22T10:00:00.000Z',
      digests,
    });
    // Nothing read completely: nothing to decide on.
    expect(observationOf(verified('a', { status: 'readError', digests: [] }))).toBeNull();
  });

  it('offers to update locations only for content found the same (R2)', () => {
    const list = entries(
      verified('a', { locationChange: 'E:\\a' }),
      verified('b', { locationChange: 'E:\\b', status: 'mismatch' }),
      verified('c', {}),
      searched('d', { status: 'locationRecognized', locationChange: 'E:\\d', itemId: 9 }),
    );
    expect(locationUpdates(list).map((u) => [u.key, u.itemId])).toEqual([
      [1, 7],
      [4, 9],
    ]);
  });

  it('attaches a moved file only when exactly one known file is missing (R5, R6)', () => {
    const list = entries(
      searched('a', { status: 'probablyMoved', finding: moved('missing') }),
      searched('b', {
        status: 'ambiguous',
        finding: { ...moved('missing', 'missing'), outcome: 'ambiguous' },
      }),
      verified('c', { status: 'noReference', finding: moved('missing', 'present') }),
      searched('d', { status: 'probablyMoved', finding: moved('missing'), digests: [] }),
    );
    expect(probableMoves(list).map((m) => [m.key, m.itemId])).toEqual([
      [1, 100],
      [3, 100],
    ]);
  });

  it('offers new versions for differing files only', () => {
    const list = entries(
      verified('a', { status: 'mismatch' }),
      verified('b', {}),
      verified('c', { status: 'conflict', verificationId: null }),
      searched('d', { status: 'changedHere' }),
    );
    expect(acceptable(list).map((a) => [a.key, a.verificationId])).toEqual([
      [1, 70],
      [3, null],
    ]);
  });
});
