// The preparatory list and how it is filled: files and folders added by the
// user, looked through by the core (in the background, in batches), plus
// the files listed by hash files added for verification.

import { api, isProblem } from './api';
import type { DiscoveryOptions } from './generated/DiscoveryOptions';
import type { DiscoveryProgress } from './generated/DiscoveryProgress';
import type { Exclusion } from './generated/Exclusion';
import type { Problem } from './generated/Problem';
import { fileName } from './format';
import { isHashFile } from './hashFiles';
import { PathIndex, moveRows, removeRows, rowKey, sortRows, type Row, type SortKey } from './list';

export interface SelectionProblem {
  path: string;
  /** Why a file or folder could not be looked through. */
  problem: Problem | null;
  /** A line of a hash file that names no usable file or hash. */
  line: { number: number; list: string; reason: string } | null;
}

export type Notice =
  | { kind: 'expanded'; count: number; name: string }
  | { kind: 'hashFileAsFile'; name: string }
  | { kind: 'stopped' }
  | { kind: 'rescanned' };

/** What was added, to look through again when the options change. */
interface Root {
  paths: string[];
  expandLists: boolean;
}

export class Selection {
  rows = $state.raw<Row[]>([]);
  exclusions = $state.raw<Exclusion[]>([]);
  problems = $state.raw<SelectionProblem[]>([]);
  notices = $state.raw<Notice[]>([]);
  scanning = $state(false);
  progress = $state.raw<DiscoveryProgress | null>(null);
  #roots: Root[] = [];
  #index = new PathIndex();
  #nextId = 1;

  /**
   * Adds files and folders. With `expandLists` (when verifying), a hash file
   * stands for the files it lists, each to be checked against its line.
   */
  async add(paths: string[], options: DiscoveryOptions, expandLists: boolean) {
    this.#roots.push({ paths, expandLists });
    this.notices = [];
    await this.#scan(paths, options, expandLists);
  }

  /** Looks through everything again, with new options. */
  async rescan(options: DiscoveryOptions) {
    const roots = this.#roots;
    if (roots.length === 0) return;
    this.clear();
    this.#roots = roots;
    for (const root of roots) await this.#scan(root.paths, options, root.expandLists);
    this.notices = [...this.notices, { kind: 'rescanned' }];
  }

  async #scan(paths: string[], options: DiscoveryOptions, expandLists: boolean) {
    const lists = expandLists ? paths.filter(isHashFile) : [];
    const others = paths.filter((path) => !lists.includes(path));
    const notices: Notice[] = others
      .filter(isHashFile)
      .map((path) => ({ kind: 'hashFileAsFile', name: fileName(path) }));
    this.scanning = true;
    try {
      if (others.length > 0) {
        const summary = await api.discoverSelection(others, options, (batch) => {
          const duplicates: Exclusion[] = [];
          const fresh: Row[] = [];
          for (const file of batch.files) {
            if (!this.#index.add(rowKey(file.path, null))) {
              duplicates.push({ path: file.path, reason: 'duplicate', folder: false });
              continue;
            }
            fresh.push({
              id: this.#nextId++,
              path: file.path,
              size: file.size,
              extension: file.extension,
              direct: file.direct,
              listed: null,
            });
          }
          if (fresh.length > 0) this.rows = this.rows.concat(fresh);
          if (batch.exclusions.length + duplicates.length > 0) {
            this.exclusions = this.exclusions.concat(batch.exclusions, duplicates);
          }
          if (batch.errors.length > 0) {
            this.problems = this.problems.concat(
              batch.errors.map((e) => ({ path: e.path, problem: e.problem, line: null })),
            );
          }
          this.progress = batch.progress;
        });
        if (summary.cancelled) notices.push({ kind: 'stopped' });
      }
      for (const list of lists) await this.#expand(list, notices);
    } catch (error) {
      this.problems = this.problems.concat({
        path: paths.join(', '),
        problem: isProblem(error) ? error : null,
        line: null,
      });
    } finally {
      this.scanning = false;
      this.progress = null;
      this.notices = [...this.notices, ...notices];
    }
  }

  async #expand(list: string, notices: Notice[]) {
    try {
      const listed = await api.expandHashFile(list);
      const fresh: Row[] = [];
      for (const file of listed.files) {
        if (!this.#index.add(rowKey(file.path, file.listed))) continue;
        fresh.push({
          id: this.#nextId++,
          path: file.path,
          size: file.size ?? 0,
          extension: extensionOf(file.path),
          // Named by the hash file: kept whatever the extension filter.
          direct: true,
          listed: file.listed,
        });
      }
      this.rows = this.rows.concat(fresh);
      this.problems = this.problems.concat(
        listed.problems.map((p) => ({
          path: p.path ?? listed.path,
          problem: null,
          line: { number: p.line, list: listed.path, reason: p.reason },
        })),
      );
      notices.push({ kind: 'expanded', count: fresh.length, name: fileName(listed.path) });
    } catch (error) {
      this.problems = this.problems.concat({
        path: list,
        problem: isProblem(error) ? error : null,
        line: null,
      });
    }
  }

  remove(ids: ReadonlySet<number>) {
    for (const row of this.rows) {
      if (ids.has(row.id)) this.#index.delete(rowKey(row.path, row.listed));
    }
    this.rows = removeRows(this.rows, ids);
  }

  move(ids: ReadonlySet<number>, direction: -1 | 1) {
    this.rows = moveRows(this.rows, ids, direction);
  }

  sort(key: SortKey, descending: boolean) {
    this.rows = sortRows(this.rows, key, descending);
  }

  clear() {
    this.rows = [];
    this.exclusions = [];
    this.problems = [];
    this.notices = [];
    this.#roots = [];
    this.#index.clear();
  }
}

/** Same rule as the core: after the last dot, unless it starts the name. */
export function extensionOf(path: string): string {
  const name = fileName(path);
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}
