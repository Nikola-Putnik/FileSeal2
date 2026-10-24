// The preparatory list, as plain data and pure functions: what the user
// added, in the order the batch will start the files. Kept in the page:
// 50,000 rows weigh a few megabytes; only the visible ones are drawn.

import type { BatchFile } from './generated/BatchFile';
import type { ListedReference } from './generated/ListedReference';
import { fileName, folderOf } from './format';

export interface Row {
  /** Stable for the life of the list: survives sorting and moving. */
  id: number;
  path: string;
  size: number;
  /** Lowercase, without the dot; empty when none. */
  extension: string;
  /** Chosen directly rather than found in a folder. */
  direct: boolean;
  /** From a hash file: verified against this line only. */
  listed: ListedReference | null;
}

export type SortKey = 'name' | 'folder' | 'size' | 'extension';

// Explorer's order: case ignored, numbers compared as numbers.
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

const SORTERS: Record<SortKey, (a: Row, b: Row) => number> = {
  name: (a, b) => collator.compare(fileName(a.path), fileName(b.path)),
  folder: (a, b) =>
    collator.compare(folderOf(a.path), folderOf(b.path)) ||
    collator.compare(fileName(a.path), fileName(b.path)),
  size: (a, b) => a.size - b.size,
  extension: (a, b) =>
    collator.compare(a.extension, b.extension) ||
    collator.compare(fileName(a.path), fileName(b.path)),
};

/** A new list in that order (stable: equal rows keep their order). */
export function sortRows(rows: Row[], key: SortKey, descending = false): Row[] {
  const compare = SORTERS[key];
  return [...rows].sort((a, b) => (descending ? compare(b, a) : compare(a, b)));
}

/**
 * Moves the selected rows one place up (-1) or down (+1). A selected row
 * against the edge, or against another selected row that cannot move,
 * stays where it is, so a block keeps its shape.
 */
export function moveRows(rows: Row[], selected: ReadonlySet<number>, direction: -1 | 1): Row[] {
  const moved = [...rows];
  const indices = moved.map((_, i) => i);
  if (direction === 1) indices.reverse();
  for (const i of indices) {
    const j = i + direction;
    if (!selected.has(moved[i].id) || j < 0 || j >= moved.length) continue;
    if (selected.has(moved[j].id)) continue;
    [moved[i], moved[j]] = [moved[j], moved[i]];
  }
  return moved;
}

export function removeRows(rows: Row[], selected: ReadonlySet<number>): Row[] {
  return rows.filter((row) => !selected.has(row.id));
}

export interface ExtensionStat {
  extension: string;
  files: number;
  bytes: number;
}

/** Files and bytes per extension, the most frequent first. */
export function extensionStats(rows: Row[]): ExtensionStat[] {
  const stats = new Map<string, ExtensionStat>();
  for (const row of rows) {
    const stat = stats.get(row.extension) ?? { extension: row.extension, files: 0, bytes: 0 };
    stat.files += 1;
    stat.bytes += row.size;
    stats.set(row.extension, stat);
  }
  return [...stats.values()].sort(
    (a, b) => b.files - a.files || collator.compare(a.extension, b.extension),
  );
}

/**
 * Which extensions of the files found in folders are kept: all but those
 * unticked, or only those typed. A list typed also leaves out the
 * extensions of folders added afterwards.
 */
export interface ExtensionFilter {
  /** Unticked, when `only` is null. */
  leftOut: ReadonlySet<string>;
  only: ReadonlySet<string> | null;
}

export const ALL_EXTENSIONS: ExtensionFilter = { leftOut: new Set(), only: null };

export function keeps(filter: ExtensionFilter, extension: string): boolean {
  return filter.only ? filter.only.has(extension) : !filter.leftOut.has(extension);
}

/** The chip of this extension ticked or unticked. */
export function toggleExtension(filter: ExtensionFilter, extension: string): ExtensionFilter {
  return filter.only
    ? { ...filter, only: toggled(filter.only, extension) }
    : { ...filter, leftOut: toggled(filter.leftOut, extension) };
}

/**
 * Extensions as the user types them: `mkv iso`, `.MKV, .Iso`, `*.mp4`.
 * Lowercase, as the core gives them, so the case never matters.
 */
export function parseExtensions(text: string): string[] {
  const found = text
    .split(/[\s,;|]+/)
    .map((part) => part.replace(/^\*?\.+/, '').toLowerCase())
    .filter((part) => part.length > 0);
  return [...new Set(found)];
}

/**
 * The rows the batch will process: those whose extension is kept. A file
 * chosen directly is always kept (cadrage, rule 10).
 */
export function includedRows(rows: Row[], filter: ExtensionFilter): Row[] {
  if (!filter.only && filter.leftOut.size === 0) return rows;
  return rows.filter((row) => row.direct || keeps(filter, row.extension));
}

/** Rows whose name or folder contains `text`, case ignored. */
export function matchingRows(rows: Row[], text: string): Row[] {
  const needle = text.trim().toLowerCase();
  if (!needle) return rows;
  return rows.filter((row) => row.path.toLowerCase().includes(needle));
}

export function totalBytes(rows: Row[]): number {
  return rows.reduce((sum, row) => sum + row.size, 0);
}

export function toBatchFiles(rows: Row[]): BatchFile[] {
  return rows.map((row) => ({ path: row.path, size: row.size, listed: row.listed }));
}

/** The key under which Windows sees two paths as the same file. */
export function pathKey(path: string): string {
  return path.toLowerCase();
}

/** Paths already in the list, to keep each file once. */
export class PathIndex {
  #keys = new Set<string>();

  /** False when the key was already there. */
  add(key: string): boolean {
    if (this.#keys.has(key)) return false;
    this.#keys.add(key);
    return true;
  }

  delete(key: string) {
    this.#keys.delete(key);
  }

  clear() {
    this.#keys.clear();
  }
}

/** A file listed by a hash file is another entry than the same file alone. */
export function rowKey(path: string, listed: ListedReference | null): string {
  return listed ? `${pathKey(path)}|${pathKey(listed.list)}` : pathKey(path);
}

/** A new set with `value` added, or removed if it was there. */
export function toggled<T>(set: ReadonlySet<T>, value: T): Set<T> {
  const next = new Set(set);
  if (next.has(value)) next.delete(value);
  else next.add(value);
  return next;
}

/**
 * The deepest folder holding every row, case ignored (Windows), or an
 * empty string when they share none: shown once, so that the folder column
 * shows what differs.
 */
export function commonFolder(rows: Row[]): string {
  if (rows.length === 0) return '';
  let common = folderOf(rows[0].path);
  let lower = common.toLowerCase();
  for (const row of rows) {
    const path = row.path.toLowerCase();
    while (common && !(path.startsWith(lower) && /[\\/]/.test(path.charAt(lower.length)))) {
      common = folderOf(common);
      lower = common.toLowerCase();
    }
    if (!common) return '';
  }
  return common;
}
