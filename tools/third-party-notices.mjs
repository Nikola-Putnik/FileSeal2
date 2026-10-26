// Writes THIRD-PARTY-NOTICES.txt: the licenses of the third-party code in
// the published program, as those licenses ask. Run by scripts\build.cmd
// before each build, so the notices shipped are those of the code shipped.
//
// What is shipped is measured, not listed by hand:
// - Rust: the crates fileseal.exe is built from, for Windows: normal
//   dependencies of the app, without build scripts' dependencies nor
//   procedural macros (they run at compile time, nothing of them is left
//   in the program);
// - JavaScript: the npm packages whose code the interface bundle holds,
//   read from the source maps of a build made for the purpose.
//
// A crate offering a choice of licenses is named with the one it is used
// under (MIT first). License texts are the crates' own files; one is
// written from the standard text only when a crate ships none. No local
// path is written.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUTPUT = join(ROOT, 'THIRD-PARTY-NOTICES.txt');
const RULE = '-'.repeat(80);

/** Licenses preferred when a package offers a choice, best first. */
const PREFERENCE = [
  'MIT',
  'MIT-0',
  'Apache-2.0',
  'BSD-3-Clause',
  'BSD-2-Clause',
  'ISC',
  'Zlib',
  'Unicode-3.0',
  'BSL-1.0',
  'Unlicense',
  'CC0-1.0',
  'MPL-2.0',
];

/** How a license file is recognised among a package's files. */
const FILE_PATTERNS = {
  MIT: /mit/i,
  'MIT-0': /mit/i,
  'Apache-2.0': /apache/i,
  'BSD-3-Clause': /bsd/i,
  'BSD-2-Clause': /bsd/i,
  Zlib: /zlib/i,
  'Unicode-3.0': /unicode/i,
  'MPL-2.0': /mpl/i,
  Unlicense: /unlicense/i,
};

/** Standard texts, for a package that ships no file of its license. */
const TEMPLATES = {
  MIT: (holder) => `MIT License

Copyright (c) ${holder}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`,
  'BSD-3-Clause': (holder) => `BSD 3-Clause License

Copyright (c) ${holder}

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.`,
};

/** The licenses a package is used under, from its SPDX expression. */
function chosen(expression) {
  const normalised = expression.replace(/\s*\/\s*/g, ' OR ').trim();
  // Top-level AND: every part applies. Within a part, one alternative.
  const parts = [];
  let depth = 0;
  let current = '';
  for (const word of normalised.split(/\s+/)) {
    depth += (word.match(/\(/g) ?? []).length - (word.match(/\)/g) ?? []).length;
    if (word === 'AND' && depth === 0) {
      parts.push(current.trim());
      current = '';
    } else {
      current += ' ' + word;
    }
  }
  parts.push(current.trim());
  return parts.map((part) => {
    const options = part
      .replace(/[()]/g, '')
      .split(/\s+OR\s+/)
      .map((o) => o.trim());
    const rank = (o) => (PREFERENCE.includes(o) ? PREFERENCE.indexOf(o) : PREFERENCE.length);
    return [...options].sort((a, b) => rank(a) - rank(b))[0];
  });
}

/** License and notice files of a package directory. */
function licenseFiles(dir) {
  return readdirSync(dir)
    .filter((f) => /^(licen[cs]e|copying|copyright|notice|unlicense)/i.test(f))
    .sort();
}

/** The texts to reproduce for one package: its files for the licenses it is used under. */
function texts(dir, licenses, holder, declaredFile) {
  const files = licenseFiles(dir);
  const picked = new Set();
  const missing = [];
  for (const license of licenses) {
    const pattern = FILE_PATTERNS[license];
    const specific = pattern ? files.filter((f) => pattern.test(f) && !/notice/i.test(f)) : [];
    const generic = files.filter((f) => /^(licen[cs]e|copying)(\.(md|txt))?$/i.test(f));
    if (specific.length > 0) specific.forEach((f) => picked.add(f));
    else if (declaredFile) picked.add(declaredFile);
    else if (generic.length > 0) generic.forEach((f) => picked.add(f));
    else missing.push(license);
  }
  // Apache-2.0 asks for NOTICE files; COPYRIGHT files hold the holders.
  files.filter((f) => /^(notice|copyright)/i.test(f)).forEach((f) => picked.add(f));
  const out = [...picked].map((f) =>
    readFileSync(join(dir, f), 'utf8').replace(/\r\n?/g, '\n').trim(),
  );
  for (const license of missing) {
    // A license without a holder's name (MPL-2.0…) is the same text for
    // every package: the one another package ships stands for it.
    const template = TEMPLATES[license];
    out.push(template ? template(holder) : { sameAs: license });
  }
  return out;
}

/** The crates compiled into fileseal.exe for Windows. */
function rustCrates() {
  const meta = JSON.parse(
    execFileSync(
      'cargo',
      ['metadata', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'],
      { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28 },
    ),
  );
  const packages = new Map(meta.packages.map((p) => [p.id, p]));
  const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
  const members = new Set(meta.workspace_members);
  const procMacro = (p) => p.targets.some((t) => t.kind.includes('proc-macro'));
  const app = meta.packages.find((p) => p.name === 'fileseal' && members.has(p.id));
  const seen = new Set();
  const stack = [app.id];
  while (stack.length > 0) {
    const id = stack.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const dep of nodes.get(id).deps) {
      const normal = dep.dep_kinds.some((k) => k.kind === null);
      if (normal && !procMacro(packages.get(dep.pkg))) stack.push(dep.pkg);
    }
  }
  const crates = [...seen]
    .filter((id) => !members.has(id))
    .map((id) => packages.get(id))
    .map((p) => {
      const dir = dirname(p.manifest_path);
      const licenses = chosen(p.license ?? 'NOASSERTION');
      const declared = p.license_file ? p.license_file.replace(/^.*[\\/]/, '') : null;
      const holder =
        p.authors.length > 0
          ? p.authors.map((a) => a.replace(/\s*<.*>/, '')).join(', ')
          : `the ${p.name} developers`;
      return {
        name: p.name,
        version: p.version,
        expression: p.license,
        licenses,
        source: `https://crates.io/crates/${p.name}/${p.version}`,
        texts: texts(
          dir,
          licenses,
          holder,
          declared && existsSync(join(dir, declared)) ? declared : null,
        ),
      };
    });
  return { version: app.version, crates };
}

/** The npm packages whose code is in the interface bundle. */
function npmPackages() {
  const out = mkdtempSync(join(tmpdir(), 'fileseal-notices-'));
  try {
    execFileSync(
      process.execPath,
      [
        join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'),
        'build',
        '--sourcemap',
        '--outDir',
        out,
        '--emptyOutDir',
        '--logLevel',
        'error',
      ],
      { cwd: ROOT, stdio: ['ignore', 'ignore', 'inherit'] },
    );
    const names = new Set();
    const assets = join(out, 'assets');
    for (const file of readdirSync(assets).filter((f) => f.endsWith('.map'))) {
      for (const source of JSON.parse(readFileSync(join(assets, file), 'utf8')).sources) {
        const found = /node_modules\/((?:@[^/]+\/)?[^/]+)/.exec(source);
        if (found) names.add(found[1]);
      }
    }
    return [...names].map((name) => {
      const dir = join(ROOT, 'node_modules', ...name.split('/'));
      const manifest = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
      const expression = manifest.license;
      const licenses = chosen(expression);
      const holder =
        typeof manifest.author === 'string'
          ? manifest.author.replace(/\s*<.*>/, '')
          : (manifest.author?.name ?? `the ${name} developers`);
      return {
        name,
        version: manifest.version,
        expression,
        licenses,
        source: `https://www.npmjs.com/package/${name}/v/${manifest.version}`,
        texts: texts(dir, licenses, holder, null),
      };
    });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}

function section(title, packages, printed, shipsText) {
  const lines = [RULE, title, RULE, ''];
  for (const p of [...packages].sort(
    (a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version),
  )) {
    const usedUnder = p.licenses.join(' AND ');
    const under =
      usedUnder === p.expression ? p.expression : `${p.expression}, used under ${usedUnder}`;
    lines.push(`${p.name} ${p.version} (${under})`, p.source, '');
    for (const text of p.texts) {
      if (typeof text !== 'string') {
        const other = shipsText.get(text.sameAs);
        if (!other) throw new Error(`no ${text.sameAs} text for ${p.name}: add a template`);
        lines.push(`[${text.sameAs}: same text as for ${other}.]`, '');
        continue;
      }
      const key = text.replace(/\s+/g, ' ');
      if (printed.has(key)) {
        lines.push(`[Same text as for ${printed.get(key)}.]`, '');
      } else {
        printed.set(key, `${p.name} ${p.version}`);
        lines.push(text, '');
      }
    }
    lines.push('');
  }
  return lines;
}

const { version, crates } = rustCrates();
const npm = npmPackages();
const counts = new Map();
for (const p of [...crates, ...npm]) {
  const key = [...p.licenses].sort().join(' AND ');
  counts.set(key, (counts.get(key) ?? 0) + 1);
}
const printed = new Map();
// For each license, a package used under it alone that ships its text.
const shipsText = new Map();
for (const p of [...crates, ...npm].sort((a, b) => a.name.localeCompare(b.name))) {
  if (p.licenses.length === 1 && typeof p.texts[0] === 'string' && !shipsText.has(p.licenses[0])) {
    shipsText.set(p.licenses[0], `${p.name} ${p.version}`);
  }
}
const lines = [
  `Third-party notices for FileSeal ${version}`,
  '',
  'FileSeal is released under the MIT License (see LICENSE). It contains code',
  'from the third-party projects listed below, each under its own license. Their',
  'license texts and notices are reproduced here, as those licenses ask. Where a',
  'project offers a choice of licenses, the one FileSeal uses it under is named.',
  'The source code of each project is available at the address given.',
  '',
  `${crates.length} Rust crates and ${npm.length} npm packages, by license:`,
  ...[...counts]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([l, n]) => `  ${String(n).padStart(4)}  ${l}`),
  '',
  'Also in the program:',
  '- the Rust standard library, under MIT OR Apache-2.0, used under MIT:',
  '  Copyright (c) The Rust Project Developers (https://github.com/rust-lang/rust);',
  '- SQLite, compiled in through the libsqlite3-sys crate, which is in the public',
  '  domain (https://sqlite.org/copyright.html).',
  'The installer is made with NSIS (https://nsis.sourceforge.io), under the',
  'zlib/libpng license. WebView2, which displays the interface, is part of',
  'Windows and is not distributed with FileSeal.',
  '',
  ...section('Rust crates', crates, printed, shipsText),
  ...section('npm packages (in the interface)', npm, printed, shipsText),
];
writeFileSync(OUTPUT, lines.join('\n').replace(/\n{3,}/g, '\n\n') + '\n');
console.log(`THIRD-PARTY-NOTICES.txt: ${crates.length} crates, ${npm.length} npm packages`);
