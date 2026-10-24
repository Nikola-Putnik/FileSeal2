import { afterEach, describe, expect, it } from 'vitest';
import {
  fileName,
  folderOf,
  formatBytes,
  formatDuration,
  formatRate,
  relativeTo,
  splitEnd,
} from './format';
import { setLocale } from './i18n/index.svelte';

// Intl in French uses a narrow no-break space as thousands separator.
const normalize = (text: string) => text.replace(/\u202f|\u00a0/g, ' ');

describe('format', () => {
  afterEach(() => setLocale('en'));

  it('keeps the end of a long name, to cut it in the middle', () => {
    expect(splitEnd('Alien.mkv')).toEqual(['Alien.mkv', '']);
    const name = 'The.Expanse.S01E01.Dulcinea.1080p.BluRay.x264-ROVERS.mkv';
    const [head, tail] = splitEnd(name);
    expect(tail).toBe('264-ROVERS.mkv');
    expect(head + tail).toBe(name);
    // A shorter one keeps its last half: the season of a folder.
    expect(splitEnd('The Expanse\\Season 1')).toEqual(['The Expans', 'e\\Season 1']);
    // Characters outside the basic plane are not cut in two.
    expect(splitEnd('📼'.repeat(40))[1]).toBe('📼'.repeat(14));
  });

  it('formats sizes like Windows Explorer, in English by default', () => {
    expect(formatBytes(0)).toBe('0 bytes');
    expect(formatBytes(1)).toBe('1 byte');
    expect(formatBytes(512)).toBe('512 bytes');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 ** 3)).toBe('5 GB');
    expect(formatBytes(2048 * 1024 ** 4)).toBe('2,048 TB');
    expect(formatRate(150 * 1024 ** 2)).toBe('150 MB/s');
  });

  it('formats sizes in French', () => {
    setLocale('fr');
    expect(formatBytes(0)).toBe('0 octet');
    expect(formatBytes(512)).toBe('512 octets');
    expect(formatBytes(1536)).toBe('1,5 Ko');
    expect(formatBytes(5 * 1024 ** 3)).toBe('5 Go');
    expect(normalize(formatBytes(2048 * 1024 ** 4))).toBe('2 048 To');
    expect(formatRate(150 * 1024 ** 2)).toBe('150 Mo/s');
  });

  it('formats durations', () => {
    expect(formatDuration(4_400)).toBe('4 s');
    expect(formatDuration(185_000)).toBe('3 min 05 s');
    expect(formatDuration(3_725_000)).toBe('1 h 02 min');
  });

  it('extracts file names from any path', () => {
    expect(fileName('C:\\Films\\Blade Runner.mkv')).toBe('Blade Runner.mkv');
    expect(fileName('/home/films/a.mkv')).toBe('a.mkv');
  });

  it('shows paths relative to a folder when inside it', () => {
    const sums = 'D:\\Séries\\Andor\\SHA256SUMS';
    expect(folderOf(sums)).toBe('D:\\Séries\\Andor');
    expect(relativeTo('D:\\Séries\\Andor\\Saison 2\\é1.mkv', folderOf(sums))).toBe(
      'Saison 2\\é1.mkv',
    );
    expect(relativeTo('d:\\séries\\andor\\a.mkv', 'D:\\Séries\\Andor')).toBe('a.mkv');
    expect(relativeTo('D:\\Séries\\Andor 2\\a.mkv', 'D:\\Séries\\Andor')).toBe(
      'D:\\Séries\\Andor 2\\a.mkv',
    );
    expect(relativeTo('E:\\a.mkv', 'D:\\Séries')).toBe('E:\\a.mkv');
  });
});
