// New versions of FileSeal: the latest release of the project on GitHub
// (Tauri's updater plugin reads its `latest.json`, and checks the installer
// against the project's key before running it). Looked for at start-up when
// the preferences allow it, and on demand. Installing closes FileSeal and
// runs the installer, which starts it again; the portable version only
// points to the download.

import { check, type Update } from '@tauri-apps/plugin-updater';

export interface Available {
  version: string;
  /** The release notes, as plain text. */
  notes: string;
}

interface UpdateState {
  available: Available | null;
  checking: boolean;
  /** The last check found nothing newer. */
  upToDate: boolean;
  /** A check asked for, or an installation, failed. */
  error: unknown;
  /** Downloading: bytes so far, and in all when known. */
  progress: { done: number; total: number | null } | null;
  installing: boolean;
  /** "Later": not shown again before the next check. */
  dismissed: boolean;
}

export const updates: UpdateState = $state({
  available: null,
  checking: false,
  upToDate: false,
  error: null,
  progress: null,
  installing: false,
  dismissed: false,
});

let pending: Update | null = null;

/** Looks for a new version. `quiet`, at start-up: a failure (no network…) is not shown. */
export async function checkForUpdate(quiet = false) {
  if (updates.checking || updates.installing) return;
  updates.checking = true;
  updates.error = null;
  try {
    const found = await check();
    pending = found;
    updates.available = found ? { version: found.version, notes: plain(found.body ?? '') } : null;
    updates.upToDate = found === null;
    updates.dismissed = false;
  } catch (error) {
    if (quiet) console.warn('update check failed', error);
    else updates.error = error;
  } finally {
    updates.checking = false;
  }
}

/** Downloads the new version, checks its signature, then installs it: FileSeal closes. */
export async function installUpdate() {
  if (!pending || updates.installing) return;
  updates.installing = true;
  updates.error = null;
  let done = 0;
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        updates.progress = { done: 0, total: event.data.contentLength ?? null };
      } else if (event.event === 'Progress') {
        done += event.data.chunkLength;
        updates.progress = { done, total: updates.progress?.total ?? null };
      }
    });
  } catch (error) {
    updates.error = error;
    updates.installing = false;
    updates.progress = null;
  }
}

/**
 * The update found, for the end-to-end checks: downloading it (signature
 * checked) without installing.
 */
export function pendingUpdate(): Update | null {
  return pending;
}

/** Release notes are written in Markdown; shown as text. */
function plain(markdown: string): string {
  return markdown
    .replace(/\*\*(.+?)\*\*/g, '$1')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .trim();
}
