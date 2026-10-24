// Typed calls to the Rust commands (src-tauri/src/commands.rs). The types
// come from Rust through ts-rs (`cargo test` regenerates ./generated).

import { Channel, invoke } from '@tauri-apps/api/core';
import type { AcceptReport } from './generated/AcceptReport';
import type { AppInfo } from './generated/AppInfo';
import type { AttachReport } from './generated/AttachReport';
import type { BatchFile } from './generated/BatchFile';
import type { BatchOptions } from './generated/BatchOptions';
import type { BatchSummary } from './generated/BatchSummary';
import type { BatchUpdate } from './generated/BatchUpdate';
import type { CreateOptions } from './generated/CreateOptions';
import type { CopyReport } from './generated/CopyReport';
import type { CreateReport } from './generated/CreateReport';
import type { DatabaseInfo } from './generated/DatabaseInfo';
import type { DiscoveryBatch } from './generated/DiscoveryBatch';
import type { DiscoveryOptions } from './generated/DiscoveryOptions';
import type { DiscoverySummary } from './generated/DiscoverySummary';
import type { Inspection } from './generated/Inspection';
import type { ItemHistory } from './generated/ItemHistory';
import type { ItemsChanged } from './generated/ItemsChanged';
import type { Listed } from './generated/Listed';
import type { ListedFiles } from './generated/ListedFiles';
import type { Observation } from './generated/Observation';
import type { Problem } from './generated/Problem';
import type { ProgressSnapshot } from './generated/ProgressSnapshot';
import type { Reassign } from './generated/Reassign';
import type { ReassignPreview } from './generated/ReassignPreview';
import type { ReassignReport } from './generated/ReassignReport';
import type { RecentDatabaseStatus } from './generated/RecentDatabaseStatus';
import type { RunPreferences } from './generated/RunPreferences';
import type { ReferencePage } from './generated/ReferencePage';
import type { Settings } from './generated/Settings';
import type { VolumeStatus } from './generated/VolumeStatus';

/** A restore: the database now, and its content before, kept as a copy. */
export interface Restored {
  info: DatabaseInfo;
  safetyCopy: CopyReport;
}
import { problemText } from './labels';

export function isProblem(value: unknown): value is Problem {
  return typeof value === 'object' && value !== null && 'kind' in value && 'detail' in value;
}

/** A rejected command as a sentence for the user, with the technical detail. */
export function problemMessage(error: unknown): string {
  if (isProblem(error)) return `${problemText(error.kind)} (${error.detail})`;
  return String(error);
}

function progressChannel(onProgress: (snapshot: ProgressSnapshot) => void) {
  const channel = new Channel<ProgressSnapshot>();
  channel.onmessage = onProgress;
  return channel;
}

export const api = {
  appInfo: () => invoke<AppInfo>('app_info'),
  settingsGet: () => invoke<Settings>('settings_get'),
  settingsSetLanguage: (language: string | null) =>
    invoke<Settings>('settings_set_language', { language }),
  settingsSetRun: (run: RunPreferences) => invoke<Settings>('settings_set_run', { run }),
  databaseCreate: (path: string, name: string) =>
    invoke<DatabaseInfo>('database_create', { path, name }),
  databaseOpen: (path: string) => invoke<DatabaseInfo>('database_open', { path }),
  databaseClose: () => invoke<void>('database_close'),
  databaseCurrent: () => invoke<DatabaseInfo | null>('database_current'),
  databaseReopenLast: () => invoke<DatabaseInfo | null>('database_reopen_last'),
  databaseRecent: () => invoke<RecentDatabaseStatus[]>('database_recent'),
  databaseForget: (path: string) => invoke<RecentDatabaseStatus[]>('database_forget', { path }),
  databaseRename: (name: string) => invoke<DatabaseInfo>('database_rename', { name }),
  databaseMove: (target: string) => invoke<DatabaseInfo>('database_move', { target }),
  databaseBackup: (target: string, replace: boolean) =>
    invoke<CopyReport>('database_backup', { target, replace }),
  databaseInspect: (path: string) => invoke<Inspection>('database_inspect', { path }),
  databaseRestore: (backup: string) => invoke<Restored>('database_restore', { backup }),
  databaseVolumes: () => invoke<VolumeStatus[]>('database_volumes'),
  databaseReassignPreview: (request: Reassign) =>
    invoke<ReassignPreview>('database_reassign_preview', { request }),
  databaseReassign: (request: Reassign) => invoke<ReassignReport>('database_reassign', { request }),
  databaseBrowse: (query: string, listed: Listed) =>
    invoke<ReferencePage>('database_browse', { query, listed }),
  databaseMatching: (query: string, listed: Listed) =>
    invoke<number[]>('database_matching', { query, listed }),
  databaseRemove: (itemIds: number[]) => invoke<ItemsChanged>('database_remove', { itemIds }),
  databasePutBack: (itemIds: number[]) => invoke<ItemsChanged>('database_put_back', { itemIds }),
  databaseHistory: (itemId: number) => invoke<ItemHistory>('database_history', { itemId }),
  createReference: (
    path: string,
    options: CreateOptions,
    useDatabase: boolean,
    onProgress: (snapshot: ProgressSnapshot) => void,
  ) =>
    invoke<CreateReport>('create_reference', {
      path,
      options,
      useDatabase,
      onProgress: progressChannel(onProgress),
    }),
  /** The files a hash file lists, each with its line. */
  expandHashFile: (path: string) => invoke<ListedFiles>('expand_hash_file', { path }),
  /** Opens Explorer on the file's folder, the file selected. */
  revealInFolder: (path: string) => invoke<void>('reveal_in_folder', { path }),
  /** Resolves once every batch has been handed to `onBatch`. */
  discoverSelection: (
    paths: string[],
    options: DiscoveryOptions,
    onBatch: (batch: DiscoveryBatch) => void,
  ) =>
    new Promise<DiscoverySummary>((resolve, reject) => {
      // The command's answer can overtake the last batches (measured: 1,000
      // files out of 20,000 still on their way). The last batch carries the
      // summary, so the list is complete when it arrives.
      const channel = new Channel<DiscoveryBatch>();
      channel.onmessage = (batch) => {
        onBatch(batch);
        if (batch.summary) resolve(batch.summary);
      };
      invoke<DiscoverySummary>('discover_selection', { paths, options, onBatch: channel }).catch(
        reject,
      );
    }),
  /** Resolves once every update has been handed to `onUpdate`. */
  runBatch: (
    files: BatchFile[],
    options: BatchOptions,
    useDatabase: boolean,
    onUpdate: (update: BatchUpdate) => void,
  ) =>
    new Promise<BatchSummary>((resolve, reject) => {
      // As for the selection: the answer can overtake the last updates, and
      // the last update carries the summary.
      const channel = new Channel<BatchUpdate>();
      channel.onmessage = (update) => {
        onUpdate(update);
        if (update.summary) resolve(update.summary);
      };
      invoke<BatchSummary>('run_batch', { files, options, useDatabase, onUpdate: channel }).catch(
        reject,
      );
    }),
  pauseJob: () => invoke<void>('pause_job'),
  resumeJob: () => invoke<void>('resume_job'),
  /** Stops the running operation and quits. */
  closeConfirmed: () => invoke<void>('close_confirmed'),
  attachFile: (path: string, itemId: number, observation: Observation, allowDifferent: boolean) =>
    invoke<AttachReport>('attach_file', { path, itemId, observation, allowDifferent }),
  acceptVersion: (
    path: string,
    observation: Observation,
    verificationId: number | null,
    updateSidecars: boolean,
  ) =>
    invoke<AcceptReport>('accept_version', { path, observation, verificationId, updateSidecars }),
  exportReport: (path: string, separator: string) =>
    invoke<number>('export_report', { path, separator }),
  cancelJob: () => invoke<void>('cancel_job'),
};
