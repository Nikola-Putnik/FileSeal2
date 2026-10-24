import { describe, expect, it } from 'vitest';
import { isHashFile } from './hashFiles';

describe('isHashFile', () => {
  it('recognises hash files by extension or usual name', () => {
    for (const path of [
      'C:\\Films\\Blade Runner.mkv.sha256',
      'C:\\Films\\Blade Runner.mkv.MD5',
      'D:\\Séries\\SHA256SUMS',
      '/home/films/md5sums.txt',
      'C:\\x\\MD5SUM',
    ]) {
      expect(isHashFile(path), path).toBe(true);
    }
  });

  it('does not mistake other files for hash files', () => {
    for (const path of [
      'C:\\Films\\Blade Runner.mkv',
      'C:\\Films\\sha256 notes.txt',
      'C:\\Films\\film.sha256.mkv',
      'C:\\Films\\mysha256sums.txt',
    ]) {
      expect(isHashFile(path), path).toBe(false);
    }
  });
});
