// A demo collection for the README screenshots, in the folder given:
// movies as sparse files (real sizes, no disk space taken, content of
// zeros), a photo album with random content, a soundtrack (sparse too).
// Mounted as D: (subst) so that no local path shows on screen.
// Procedure: docs/tests-build-livraison.md, "Captures d'écran du README".
//
//   node tools/demo-collection.cjs <folder>          creates it
//   node tools/demo-collection.cjs <folder> --alter  one photo changes, one arrives
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { execFileSync } = require('child_process');

const [root, option] = process.argv.slice(2);
if (!root) {
  console.error('usage: node tools/demo-collection.cjs <folder> [--alter]');
  process.exit(2);
}
const album = path.join(root, 'Photos', 'Crète 2026');

if (option === '--alter') {
  // After the references are created: one photo changes by one byte, its
  // date kept (as bit rot would), and a new one arrives.
  const damaged = path.join(album, 'IMG_20260713_110902.jpg');
  const stat = fs.statSync(damaged);
  const fd = fs.openSync(damaged, 'r+');
  const byte = Buffer.alloc(1);
  fs.readSync(fd, byte, 0, 1, 1_000_000);
  byte[0] ^= 0x10;
  fs.writeSync(fd, byte, 0, 1, 1_000_000);
  fs.closeSync(fd);
  fs.utimesSync(damaged, stat.atime, stat.mtime);
  fs.writeFileSync(path.join(album, 'IMG_20260719_201744.jpg'), crypto.randomBytes(4_217_334));
  console.log('altered');
  process.exit(0);
}

if (fs.existsSync(root) && fs.readdirSync(root).length > 0) {
  console.error(`${root} is not empty: nothing done.`);
  process.exit(1);
}

function sparse(file, size) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  execFileSync('fsutil', ['file', 'createnew', file, '0'], { stdio: 'ignore' });
  execFileSync('fsutil', ['sparse', 'setflag', file], { stdio: 'ignore' });
  execFileSync('fsutil', ['file', 'seteof', file, String(size)], { stdio: 'ignore' });
}

const movies = [
  ['2001 A Space Odyssey (1968) 2160p Remux.mkv', 62_871_234_560],
  ["Alien (1979) Director's Cut 1080p Remux.mkv", 31_402_115_072],
  ['Blade Runner (1982) The Final Cut 2160p Remux.mkv', 58_712_431_616],
  ['Dune Part Two (2024) 2160p.mkv', 48_603_221_504],
  ['Star Wars Episode III - Revenge of the Sith (2005).iso', 41_236_889_600],
];
for (const [name, size] of movies) sparse(path.join(root, 'Movies', name), size);

const tracks = [
  ['01 Main Titles.flac', 21_436_112],
  ['02 Blush Response.flac', 38_912_540],
  ['03 Wait for Me.flac', 32_104_998],
  ["04 Rachel's Song.flac", 31_702_467],
  ['05 Love Theme.flac', 34_018_331],
  ['06 One More Kiss, Dear.flac', 27_309_874],
];
for (const [name, size] of tracks) {
  sparse(path.join(root, 'Music', 'Vangelis - Blade Runner (1994)', name), size);
}

fs.mkdirSync(album, { recursive: true });
const shots = [
  '20260712_094215',
  '20260712_101833',
  '20260713_110902',
  '20260713_152240',
  '20260714_073518',
  '20260714_190457',
  '20260715_121006',
  '20260716_084731',
  '20260716_201159',
  '20260717_133402',
  '20260718_175020',
  '20260718_193344',
];
for (const [i, shot] of shots.entries()) {
  const size = 2_800_000 + ((i * 1_234_567) % 3_400_000);
  fs.writeFileSync(path.join(album, `IMG_${shot}.jpg`), crypto.randomBytes(size));
}
console.log(`${movies.length} movies, ${tracks.length} tracks, ${shots.length} photos`);
