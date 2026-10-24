import { describe, expect, it } from 'vitest';
import {
  ALL_EXTENSIONS,
  PathIndex,
  commonFolder,
  extensionStats,
  includedRows,
  matchingRows,
  moveRows,
  parseExtensions,
  removeRows,
  rowKey,
  sortRows,
  toBatchFiles,
  toggleExtension,
  toggled,
  totalBytes,
  type Row,
} from './list';

let next = 1;
function row(path: string, size = 10, direct = false): Row {
  const name = path.split('\\').pop() ?? '';
  const dot = name.lastIndexOf('.');
  return {
    id: next++,
    path,
    size,
    extension: dot > 0 ? name.slice(dot + 1).toLowerCase() : '',
    direct,
    listed: null,
  };
}

const names = (rows: Row[]) => rows.map((r) => r.path.split('\\').pop());

describe('the preparatory list', () => {
  it('sorts as Explorer does, and stably', () => {
    const rows = [
      row('D:\\B\\Episode 10.mkv', 5),
      row('D:\\A\\episode 2.mkv', 30),
      row('D:\\A\\Episode 1.MKV', 5),
      row('D:\\C\\notes.txt', 5),
    ];
    expect(names(sortRows(rows, 'name'))).toEqual([
      'Episode 1.MKV',
      'episode 2.mkv',
      'Episode 10.mkv',
      'notes.txt',
    ]);
    expect(names(sortRows(rows, 'name', true))[0]).toBe('notes.txt');
    // Equal sizes keep their order.
    expect(names(sortRows(rows, 'size'))).toEqual([
      'Episode 10.mkv',
      'Episode 1.MKV',
      'notes.txt',
      'episode 2.mkv',
    ]);
    expect(names(sortRows(rows, 'folder'))).toEqual([
      'Episode 1.MKV',
      'episode 2.mkv',
      'Episode 10.mkv',
      'notes.txt',
    ]);
    expect(names(rows)[0]).toBe('Episode 10.mkv');
  });

  it('moves a selection one place, as a block, never past the edges', () => {
    const rows = ['a', 'b', 'c', 'd', 'e'].map((n) => row(`D:\\${n}`));
    const ids = (...letters: string[]) =>
      new Set(rows.filter((r) => letters.some((l) => r.path.endsWith(l))).map((r) => r.id));
    expect(names(moveRows(rows, ids('c', 'd'), -1))).toEqual(['a', 'c', 'd', 'b', 'e']);
    expect(names(moveRows(rows, ids('c', 'd'), 1))).toEqual(['a', 'b', 'e', 'c', 'd']);
    expect(names(moveRows(rows, ids('a', 'c'), -1))).toEqual(['a', 'c', 'b', 'd', 'e']);
    expect(names(moveRows(rows, ids('e'), 1))).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(names(removeRows(rows, ids('b', 'e')))).toEqual(['a', 'c', 'd']);
  });

  it('counts extensions and filters them, files chosen directly always kept', () => {
    const rows = [
      row('D:\\a.mkv', 100),
      row('D:\\b.MKV', 50),
      row('D:\\c.srt', 1),
      row('D:\\README', 2),
      row('D:\\chosen.srt', 3, true),
    ];
    expect(extensionStats(rows)).toEqual([
      { extension: 'mkv', files: 2, bytes: 150 },
      { extension: 'srt', files: 2, bytes: 4 },
      { extension: '', files: 1, bytes: 2 },
    ]);
    const kept = includedRows(rows, { leftOut: new Set(['srt', '']), only: null });
    expect(names(kept)).toEqual(['a.mkv', 'b.MKV', 'chosen.srt']);
    expect(totalBytes(kept)).toBe(153);
    expect(includedRows(rows, ALL_EXTENSIONS)).toBe(rows);
  });

  it('keeps only the extensions typed, whatever their case', () => {
    expect(parseExtensions(' .MKV, iso;*.Mp4  mkv ')).toEqual(['mkv', 'iso', 'mp4']);
    expect(parseExtensions(' , ')).toEqual([]);
    const rows = [
      row('D:\\a.Mkv'),
      row('D:\\b.ISO'),
      row('D:\\c.srt'),
      row('D:\\d.nfo'),
      row('D:\\chosen.srt', 3, true),
    ];
    const only = { leftOut: new Set<string>(), only: new Set(parseExtensions('MKV .iso mp4')) };
    expect(names(includedRows(rows, only))).toEqual(['a.Mkv', 'b.ISO', 'chosen.srt']);
    // A chip ticked or unticked changes the list typed.
    const more = toggleExtension(only, 'nfo');
    expect(names(includedRows(rows, more))).toEqual(['a.Mkv', 'b.ISO', 'd.nfo', 'chosen.srt']);
    expect(names(includedRows(rows, toggleExtension(more, 'mkv')))).toEqual([
      'b.ISO',
      'd.nfo',
      'chosen.srt',
    ]);
  });

  it('filters by part of a path, case ignored', () => {
    const rows = [row('D:\\Films\\Alien.mkv'), row('D:\\Séries\\alien nation.mkv'), row('E:\\x')];
    expect(names(matchingRows(rows, ' ALIEN '))).toEqual(['Alien.mkv', 'alien nation.mkv']);
    expect(names(matchingRows(rows, 'séries'))).toEqual(['alien nation.mkv']);
    expect(matchingRows(rows, '  ')).toBe(rows);
  });

  it('keeps each file once, a listed file apart from the same file alone', () => {
    const index = new PathIndex();
    expect(index.add(rowKey('D:\\A.mkv', null))).toBe(true);
    expect(index.add(rowKey('d:\\a.MKV', null))).toBe(false);
    const listed = {
      list: 'D:\\SHA256SUMS',
      line: 1,
      digest: { algorithm: 'sha256' as const, hex: '0'.repeat(64) },
      notes: [],
    };
    expect(index.add(rowKey('D:\\A.mkv', listed))).toBe(true);
    index.delete(rowKey('D:\\A.mkv', null));
    expect(index.add(rowKey('D:\\A.mkv', null))).toBe(true);
  });

  it('hands the batch the paths, sizes and listed lines', () => {
    const r = row('D:\\a.mkv', 42);
    expect(toBatchFiles([r])).toEqual([{ path: 'D:\\a.mkv', size: 42, listed: null }]);
    expect([...toggled(new Set([1, 2]), 2)]).toEqual([1]);
    expect([...toggled(new Set([1]), 3)]).toEqual([1, 3]);
  });
});

describe('the common folder', () => {
  it('is the deepest folder holding every row, case ignored', () => {
    const rows = [row('D:\\Films\\SF\\a.mkv'), row('d:\\films\\SF\\Old\\b.mkv')];
    expect(commonFolder(rows)).toBe('D:\\Films\\SF');
    expect(commonFolder([...rows, row('D:\\Films\\c.mkv')])).toBe('D:\\Films');
    // Not cut inside a name: "D:\Film" is not a folder of "D:\Films".
    expect(commonFolder([row('D:\\Film\\a'), row('D:\\Films\\b')])).toBe('D:');
    expect(commonFolder([row('D:\\a'), row('E:\\b')])).toBe('');
    expect(commonFolder([])).toBe('');
  });
});
