// How each status, outcome or problem is shown: an icon and a tone (fixed
// here) plus a text in the current language (from the dictionaries). A
// status always has an icon and words: colour is never the only signal.

import type { CreateStatus } from './generated/CreateStatus';
import type { ExclusionReason } from './generated/ExclusionReason';
import type { Presence } from './generated/Presence';
import type { ProblemKind } from './generated/ProblemKind';
import type { SearchStatus } from './generated/SearchStatus';
import type { SidecarNote } from './generated/SidecarNote';
import type { SkipReason } from './generated/SkipReason';
import type { SourceOutcome } from './generated/SourceOutcome';
import type { VerifyStatus } from './generated/VerifyStatus';
import type { WriteOutcome } from './generated/WriteOutcome';
import { t } from './i18n/index.svelte';

export type Tone = 'ok' | 'warn' | 'error' | 'neutral';

export interface StatusLabel {
  label: string;
  tone: Tone;
  icon: string;
  /** One sentence saying what it means, for the result panel. */
  explanation: string;
}

export const ICONS: Record<Tone, string> = { ok: '✔', warn: '⚠', error: '✖', neutral: 'ℹ' };

const CREATE_TONES: Record<CreateStatus, Tone> = {
  created: 'ok',
  completed: 'ok',
  alreadyReferenced: 'ok',
  skipped: 'ok',
  imported: 'ok',
  conflict: 'warn',
  partialSuccess: 'warn',
  needsDecision: 'warn',
  failed: 'error',
  cancelled: 'neutral',
};

const VERIFY_TONES: Record<VerifyStatus, Tone> = {
  match: 'ok',
  mismatch: 'error',
  noReference: 'neutral',
  noCompatibleReference: 'neutral',
  conflict: 'warn',
  invalidHashFile: 'warn',
  notFound: 'error',
  readError: 'error',
  locked: 'warn',
  modifiedDuringRead: 'warn',
  cancelled: 'neutral',
  notProcessed: 'neutral',
  failed: 'error',
};

/** Order in which statuses are listed in summaries: good news first. */
export const VERIFY_ORDER = Object.keys(VERIFY_TONES) as VerifyStatus[];

const SEARCH_TONES: Record<SearchStatus, Tone> = {
  unknown: 'neutral',
  knownHere: 'ok',
  changedHere: 'error',
  locationRecognized: 'ok',
  probablyMoved: 'warn',
  copyOfKnown: 'ok',
  ambiguous: 'warn',
  notFound: 'error',
  readError: 'error',
  locked: 'warn',
  modifiedDuringRead: 'warn',
  cancelled: 'neutral',
  failed: 'error',
};

const SPECIAL_ICONS: Partial<Record<CreateStatus | VerifyStatus | SearchStatus, string>> = {
  cancelled: '⏹',
  notProcessed: '–',
};

/** A file of a batch that was not read. */
const SKIP_TONES: Record<SkipReason, Tone> = {
  cancelled: 'neutral',
  volumeUnavailable: 'warn',
  databaseUnavailable: 'warn',
  duplicate: 'neutral',
};

const SKIP_ICONS: Record<SkipReason, string> = {
  cancelled: '–',
  volumeUnavailable: '–',
  databaseUnavailable: '–',
  duplicate: '=',
};

const WRITE_TONES: Record<WriteOutcome, Tone> = {
  written: 'ok',
  completed: 'ok',
  alreadyPresent: 'ok',
  conflict: 'warn',
  invalid: 'warn',
  skipped: 'neutral',
  needsDecision: 'warn',
  notRead: 'neutral',
  imported: 'ok',
  failed: 'error',
};

const SOURCE_TONES: Record<SourceOutcome, Tone> = {
  match: 'ok',
  mismatch: 'error',
  invalid: 'warn',
  notCompared: 'neutral',
};

export function createStatus(status: CreateStatus): StatusLabel {
  const tone = CREATE_TONES[status];
  return { tone, icon: SPECIAL_ICONS[status] ?? ICONS[tone], ...t().createStatus[status] };
}

export function verifyStatus(status: VerifyStatus): StatusLabel {
  const tone = VERIFY_TONES[status];
  return { tone, icon: SPECIAL_ICONS[status] ?? ICONS[tone], ...t().verifyStatus[status] };
}

export function searchStatus(status: SearchStatus): StatusLabel {
  const tone = SEARCH_TONES[status];
  return { tone, icon: SPECIAL_ICONS[status] ?? ICONS[tone], ...t().searchStatus[status] };
}

export function skipStatus(reason: SkipReason): StatusLabel {
  return { tone: SKIP_TONES[reason], icon: SKIP_ICONS[reason], ...t().skip[reason] };
}

export function exclusionText(reason: ExclusionReason): string {
  return t().exclusion[reason];
}

export function presenceText(presence: Presence): string {
  return t().finding[presence];
}

export function writeOutcome(outcome: WriteOutcome): { label: string; tone: Tone } {
  return { label: t().writeOutcome[outcome], tone: WRITE_TONES[outcome] };
}

export function sourceOutcome(outcome: SourceOutcome): { label: string; tone: Tone } {
  return { label: t().sourceOutcome[outcome], tone: SOURCE_TONES[outcome] };
}

export function problemText(kind: ProblemKind): string {
  return t().problem[kind];
}

export function noteText(note: SidecarNote): string {
  const notes = t().note;
  switch (note.kind) {
    case 'legacyEncoding':
      return notes.legacyEncoding;
    case 'nonStandardFormat':
      return notes.nonStandardFormat;
    case 'nameDiffers':
      return notes.nameDiffers(note.written);
    case 'nameCaseDiffers':
      return notes.nameCaseDiffers(note.written);
  }
}

export const ALGORITHM_NAMES = { sha256: 'SHA-256', md5: 'MD5' } as const;
