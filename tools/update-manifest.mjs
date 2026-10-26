// Writes latest.json, what Tauri's updater reads in the latest release of
// the project: the version, its notes (the version's section of
// CHANGELOG.md), and the installer's address and signature. Run by
// scripts\build.cmd after a signed build; published with the release.
//
//   node tools/update-manifest.mjs <version>
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const version = process.argv[2];
if (!version) {
  console.error('usage: node tools/update-manifest.mjs <version>');
  process.exit(2);
}
const nsis = join(ROOT, 'target', 'release', 'bundle', 'nsis');
const installer = `FileSeal_${version}_x64-setup.exe`;
const signatureFile = join(nsis, `${installer}.sig`);
if (!existsSync(signatureFile)) {
  console.error(`${installer}.sig is missing: the installer was not signed for updates.`);
  process.exit(1);
}
const signature = readFileSync(signatureFile, 'utf8').trim();

// owner/name, from the URL of origin.
const origin = execFileSync('git', ['remote', 'get-url', 'origin'], {
  cwd: ROOT,
  encoding: 'utf8',
}).trim();
const repo = /github\.com[/:](.+?)(?:\.git)?$/.exec(origin)?.[1];
if (!repo) {
  console.error(`origin is not a GitHub repository: ${origin}`);
  process.exit(2);
}

// "## 2.6.0 — 2026-10-26", up to the next version.
const changelog = readFileSync(join(ROOT, 'CHANGELOG.md'), 'utf8').replace(/\r\n/g, '\n');
const heading = new RegExp(`^## ${version.replace(/\./g, '\\.')}\\b.*$`, 'm');
const found = heading.exec(changelog);
let notes = '';
if (found) {
  const rest = changelog.slice(found.index + found[0].length);
  const next = rest.search(/^## /m);
  notes = (next === -1 ? rest : rest.slice(0, next)).trim();
} else {
  console.warn(`CHANGELOG.md has no section for ${version}: no notes in latest.json.`);
}

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `https://github.com/${repo}/releases/download/v${version}/${installer}`,
    },
  },
};
writeFileSync(join(nsis, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`latest.json: ${version}, ${notes ? 'with' : 'without'} notes`);
