// Number, size, duration and date formatting in the current language, with
// sizes counted the way Windows Explorer does (1 KB = 1024 bytes).

import { intlLocale, t } from './i18n/index.svelte';

function number(value: number, fractionDigits: number): string {
  return new Intl.NumberFormat(intlLocale(), { maximumFractionDigits: fractionDigits }).format(
    value,
  );
}

export function formatBytes(bytes: number): string {
  const units = t().units;
  if (bytes < 1024) return units.bytes(bytes, number(bytes, 0));
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.multiples.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${number(value, 1)} ${units.multiples[unit]}`;
}

export function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}${t().units.perSecond}`;
}

export function formatDuration(ms: number): string {
  const total = Math.round(ms / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  if (hours > 0) return `${hours} h ${pad(minutes)} min`;
  if (minutes > 0) return `${minutes} min ${pad(seconds)} s`;
  return `${seconds} s`;
}

/** UTC timestamp from the core, shown in local time. */
export function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString(intlLocale(), { dateStyle: 'short', timeStyle: 'short' });
}

/** Last component of a Windows or Unix path. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Characters always shown at the end of a long text cut in the middle. */
const KEPT_END = 14;

/**
 * A text as the part that may be cut and the end that stays shown
 * (`MiddleText`): up to 14 characters, never more than half the text. A
 * very short one is not split.
 */
export function splitEnd(text: string): [string, string] {
  const chars = [...text];
  const kept = Math.min(KEPT_END, Math.floor(chars.length / 2));
  if (kept < 6) return [text, ''];
  return [chars.slice(0, -kept).join(''), chars.slice(-kept).join('')];
}

/** Folder part of a path, without the trailing separator. */
export function folderOf(path: string): string {
  const cut = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
  return cut < 0 ? '' : path.slice(0, cut);
}

/**
 * `path` relative to `folder` when it is inside it (`Season 2\Episode 3.mkv`),
 * else the full path. Display only: case is ignored as on Windows.
 */
export function relativeTo(path: string, folder: string): string {
  if (!folder) return path;
  const prefix = folder.toLowerCase();
  const lower = path.toLowerCase();
  const separator = path.charAt(folder.length);
  if (lower.startsWith(prefix) && (separator === '\\' || separator === '/')) {
    return path.slice(folder.length + 1);
  }
  return path;
}

/** A count with the current language's digit grouping. */
export function formatCount(count: number): string {
  return number(count, 0);
}
