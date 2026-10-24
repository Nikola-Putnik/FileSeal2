// The settings of the next batch, as chosen in the page, and as kept in
// settings.json for the next session.

import type { Algorithm } from './generated/Algorithm';
import type { ExistingReferences } from './generated/ExistingReferences';
import type { RunPreferences } from './generated/RunPreferences';

export interface RunSettings {
  algorithms: Record<Algorithm, boolean>;
  writeSidecar: boolean;
  /** Create: files that already have references, read again or skipped. */
  existing: ExistingReferences;
  /** Create: the hash files of skipped files recorded in the database. */
  importHashFiles: boolean;
  useSidecar: boolean;
  useDatabase: boolean;
  /** Look into subfolders. */
  recursive: boolean;
  /** Keep hidden and system files found in folders. */
  includeHidden: boolean;
  /** Disks read at the same time, 1 to 4. */
  parallelVolumes: number;
}

export function defaultSettings(): RunSettings {
  return {
    algorithms: { sha256: true, md5: false },
    writeSidecar: true,
    existing: 'skipComplete',
    importHashFiles: true,
    useSidecar: true,
    useDatabase: true,
    recursive: true,
    includeHidden: false,
    parallelVolumes: 2,
  };
}

/** What the page keeps besides the settings themselves. */
export interface Kept {
  settings: RunSettings;
  /** "Only these extensions", if any. */
  only: string[] | null;
  /** The settings panel was left open. */
  settingsOpen: boolean;
}

const EXISTING: ExistingReferences[] = ['read', 'skipComplete', 'skipAny'];

/**
 * The saved choices over the defaults. A value missing or out of range (a
 * file edited by hand, an older or newer FileSeal) keeps its default.
 */
export function fromPreferences(saved: RunPreferences | null | undefined): Kept {
  const settings = defaultSettings();
  const kept: Kept = { settings, only: null, settingsOpen: false };
  if (!saved) return kept;
  const flag = (value: boolean | null | undefined, fallback: boolean) =>
    typeof value === 'boolean' ? value : fallback;
  if (Array.isArray(saved.algorithms)) {
    settings.algorithms = {
      sha256: saved.algorithms.includes('sha256'),
      md5: saved.algorithms.includes('md5'),
    };
  }
  settings.writeSidecar = flag(saved.writeSidecar, settings.writeSidecar);
  settings.useSidecar = flag(saved.useSidecar, settings.useSidecar);
  settings.useDatabase = flag(saved.useDatabase, settings.useDatabase);
  settings.recursive = flag(saved.recursive, settings.recursive);
  settings.includeHidden = flag(saved.includeHidden, settings.includeHidden);
  settings.importHashFiles = flag(saved.importHashFiles, settings.importHashFiles);
  if (saved.existing && EXISTING.includes(saved.existing)) settings.existing = saved.existing;
  const parallel = saved.parallelVolumes;
  if (typeof parallel === 'number' && parallel >= 1 && parallel <= 4) {
    settings.parallelVolumes = parallel;
  }
  if (Array.isArray(saved.onlyExtensions) && saved.onlyExtensions.length > 0) {
    kept.only = saved.onlyExtensions.filter((e) => typeof e === 'string');
  }
  kept.settingsOpen = flag(saved.settingsOpen, false);
  return kept;
}

export function toPreferences(kept: Kept): RunPreferences {
  const { settings } = kept;
  return {
    algorithms: (Object.keys(settings.algorithms) as Algorithm[]).filter(
      (a) => settings.algorithms[a],
    ),
    writeSidecar: settings.writeSidecar,
    useSidecar: settings.useSidecar,
    useDatabase: settings.useDatabase,
    recursive: settings.recursive,
    includeHidden: settings.includeHidden,
    parallelVolumes: settings.parallelVolumes,
    existing: settings.existing,
    importHashFiles: settings.importHashFiles,
    onlyExtensions: kept.only,
    settingsOpen: kept.settingsOpen,
  };
}
