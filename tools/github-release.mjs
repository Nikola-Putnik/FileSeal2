// Publishes the GitHub release of a tag already pushed, and uploads its
// files, through the REST API: `gh` is not needed. The credential is the one
// Git already uses to push (asked to Git's credential manager), kept in
// memory and never printed. Refuses if the tag already has a release.
//
//   node tools/github-release.mjs v2.5.0 notes.md target/release/bundle/nsis/FileSeal_2.5.0_x64-setup.exe ...
//
// Procedure: docs/tests-build-livraison.md, "Publier une version".
import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';
import { basename } from 'node:path';

const [tag, notesFile, ...files] = process.argv.slice(2);
if (!tag || !notesFile || files.length === 0) {
  console.error('usage: node tools/github-release.mjs <tag> <notes.md> <file>...');
  process.exit(2);
}
// owner/name, from the URL of origin.
const origin = execFileSync('git', ['remote', 'get-url', 'origin'], { encoding: 'utf8' }).trim();
const repo = /github\.com[/:](.+?)(?:\.git)?$/.exec(origin)?.[1];
if (!repo) {
  console.error(`origin is not a GitHub repository: ${origin}`);
  process.exit(2);
}

const fill = spawnSync('git', ['credential', 'fill'], {
  input: 'protocol=https\nhost=github.com\n\n',
  encoding: 'utf8',
  env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
});
const token = /^password=(.+)$/m.exec(fill.stdout ?? '')?.[1];
if (!token) {
  console.error('No GitHub credential stored for Git: nothing done.');
  process.exit(1);
}
const headers = {
  Authorization: `Bearer ${token}`,
  Accept: 'application/vnd.github+json',
  'X-GitHub-Api-Version': '2022-11-28',
  'User-Agent': 'fileseal-release',
};

async function call(url, init) {
  const response = await fetch(url, { ...init, headers: { ...headers, ...init.headers } });
  const text = await response.text();
  if (!response.ok)
    throw new Error(`${init.method} ${url}: ${response.status} ${text.slice(0, 300)}`);
  return JSON.parse(text);
}

const existing = await fetch(`https://api.github.com/repos/${repo}/releases/tags/${tag}`, {
  headers,
});
if (existing.ok) {
  console.error(`A release already exists for ${tag}: nothing done.`);
  process.exit(1);
}
const release = await call(`https://api.github.com/repos/${repo}/releases`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    tag_name: tag,
    name: `FileSeal ${tag.replace(/^v/, '')}`,
    body: readFileSync(notesFile, 'utf8'),
    draft: false,
    prerelease: false,
    make_latest: 'true',
  }),
});
console.log(`release ${release.html_url}`);
for (const file of files) {
  const name = basename(file);
  const type = name.endsWith('.zip')
    ? 'application/zip'
    : name.endsWith('.sha256')
      ? 'text/plain'
      : 'application/octet-stream';
  const asset = await call(
    `https://uploads.github.com/repos/${repo}/releases/${release.id}/assets?name=${encodeURIComponent(name)}`,
    {
      method: 'POST',
      headers: { 'Content-Type': type, 'Content-Length': String(statSync(file).size) },
      body: readFileSync(file),
    },
  );
  console.log(`asset ${asset.name} ${asset.size} bytes ${asset.state}`);
}
