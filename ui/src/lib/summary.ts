// The settings of the next batch in a few words, shown on the button that
// opens them: what matters can be read without opening the panel.

import { t } from './i18n/index.svelte';
import { ALGORITHM_NAMES } from './labels';
import type { ActionKind } from './run.svelte';
import type { RunSettings } from './runSettings';
import type { Algorithm } from './generated/Algorithm';

/**
 * `database`: the name of the database the batch will use, or null (none
 * open, not ticked, or read-only when creating).
 */
export function settingsSummary(
  action: ActionKind,
  settings: RunSettings,
  database: string | null,
): string {
  const words = t().settings.summary;
  const parts: string[] = [];
  if (action === 'create') {
    const algorithms = (Object.keys(settings.algorithms) as Algorithm[])
      .filter((a) => settings.algorithms[a])
      .map((a) => ALGORITHM_NAMES[a]);
    parts.push(algorithms.join(' + ') || words.none);
    const destinations = [
      settings.writeSidecar ? words.hashFiles : null,
      settings.writeSums ? words.hashLists : null,
      database ? words.database : null,
    ].filter((d): d is string => d !== null);
    parts.push(destinations.join(' + ') || words.none);
    parts.push(words.existing[settings.existing]);
  } else if (action === 'verify') {
    const sources = [
      settings.useSidecar ? words.hashFiles : null,
      database ? words.database : null,
    ].filter((d): d is string => d !== null);
    parts.push(words.references(sources.join(' + ') || words.none));
  }
  // Following moves writes to the database: nothing to follow without one.
  if (action !== 'search' && database && settings.followMoves) parts.push(words.followMoves);
  if (!settings.recursive) parts.push(words.noSubfolders);
  if (settings.includeHidden) parts.push(words.hidden);
  return parts.join(' · ');
}
