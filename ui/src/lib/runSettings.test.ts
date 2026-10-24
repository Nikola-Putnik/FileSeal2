import { afterEach, describe, expect, it } from 'vitest';
import type { RunPreferences } from './generated/RunPreferences';
import { setLocale } from './i18n/index.svelte';
import { defaultSettings, fromPreferences, toPreferences } from './runSettings';
import { settingsSummary } from './summary';

const none: RunPreferences = {
  algorithms: null,
  writeSidecar: null,
  useSidecar: null,
  useDatabase: null,
  recursive: null,
  includeHidden: null,
  parallelVolumes: null,
  existing: null,
  importHashFiles: null,
  onlyExtensions: null,
  settingsOpen: null,
};

describe('kept settings', () => {
  it('start from the defaults when nothing was kept', () => {
    expect(fromPreferences(null)).toEqual({
      settings: defaultSettings(),
      only: null,
      settingsOpen: false,
    });
    expect(fromPreferences(none).settings).toEqual(defaultSettings());
  });

  it('come back as they were saved', () => {
    const settings = {
      ...defaultSettings(),
      algorithms: { sha256: true, md5: true },
      writeSidecar: false,
      existing: 'skipAny' as const,
      recursive: false,
      parallelVolumes: 3,
    };
    const kept = { settings, only: ['mkv', 'iso'], settingsOpen: true };
    expect(fromPreferences(toPreferences(kept))).toEqual(kept);
  });

  it('keep the default of a value that makes no sense', () => {
    const odd = {
      ...none,
      parallelVolumes: 9,
      existing: 'sometimes',
      writeSidecar: 'yes',
    } as unknown as RunPreferences;
    const settings = fromPreferences(odd).settings;
    expect(settings.parallelVolumes).toBe(2);
    expect(settings.existing).toBe('skipComplete');
    expect(settings.writeSidecar).toBe(true);
  });
});

describe('settings summary', () => {
  afterEach(() => setLocale('en'));

  it('says what the next batch will do, in a few words', () => {
    const settings = defaultSettings();
    expect(settingsSummary('create', settings, 'Films')).toBe(
      'SHA-256 · hash files + database · referenced files skipped',
    );
    expect(settingsSummary('verify', settings, null)).toBe('references: hash files');
    expect(
      settingsSummary('search', { ...settings, recursive: false, includeHidden: true }, 'Films'),
    ).toBe('without subfolders · hidden files included');
    setLocale('fr');
    expect(settingsSummary('create', { ...settings, existing: 'read' }, null)).toBe(
      'SHA-256 · fichiers de hash · référencés relus',
    );
  });
});
