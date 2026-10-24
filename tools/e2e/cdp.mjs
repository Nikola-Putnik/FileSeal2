// Drives the FileSeal webview through the WebView2 DevTools protocol, for
// end-to-end checks (docs/installation-developpement.md).
// usage: node cdp.mjs eval [@script.js | "<js>"]...   (REPL mode: use `await`
//                                                       to get a promise's value)
//        node cdp.mjs shot <file.png> [full] [WxH]    (full: the whole page;
//                                      WxH: laid out at that size, e.g. 1920x1040;
//                                      feature=value: emulated, e.g.
//                                      prefers-color-scheme=light)
//        node cdp.mjs keys <key>...    real key presses: Tab, Shift+Tab, Enter,
//                                      Space, ArrowDown, Control+a, Alt+ArrowUp…
//        node cdp.mjs ax [role…]       accessibility tree: role, name and
//                                      states of the nodes (of these roles)
//        node cdp.mjs media <feature=value|none>…   emulates a media feature,
//                                      e.g. forced-colors=active
import fs from 'node:fs';

const [command, ...rest] = process.argv.slice(2);
// Several pieces may be given (`@file` or inline); they are evaluated as one script.
const arg = rest
  .map((r) => (r.startsWith('@') ? fs.readFileSync(r.slice(1), 'utf8') : r))
  .join(';\n');
// The port given to WebView2 (--remote-debugging-port), 9222 by default.
const port = process.env.CDP_PORT ?? '9222';
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
// Only a FileSeal window: another application debugged on the same port
// must never receive these scripts, clicks or simulated drops.
const page = targets.find((t) => t.type === 'page' && t.title === 'FileSeal');
if (!page) {
  const others = targets.filter((t) => t.type === 'page').map((t) => `"${t.title}"`);
  throw new Error(`no FileSeal page on port ${port} (pages: ${others.join(', ') || 'none'})`);
}
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = reject;
});
let next = 0;
const pending = new Map();
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.id && pending.has(message.id)) {
    pending.get(message.id)(message);
    pending.delete(message.id);
  }
};
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = ++next;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });

// Windows virtual-key codes of the keys used by the tests.
const KEYS = {
  Tab: 9,
  Enter: 13,
  Escape: 27,
  Space: 32,
  PageUp: 33,
  PageDown: 34,
  End: 35,
  Home: 36,
  ArrowLeft: 37,
  ArrowUp: 38,
  ArrowRight: 39,
  ArrowDown: 40,
  Delete: 46,
};
const MODIFIERS = { Alt: 1, Control: 2, Meta: 4, Shift: 8 };

async function press(chord) {
  const parts = chord.split('+');
  const name = parts.pop();
  const modifiers = parts.reduce((sum, m) => sum | MODIFIERS[m], 0);
  const key = name === 'Space' ? ' ' : name;
  const code = KEYS[name] ?? key.toUpperCase().charCodeAt(0);
  const text = key.length === 1 && !(modifiers & (MODIFIERS.Control | MODIFIERS.Alt)) ? key : '';
  const base = { key, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code, modifiers };
  await send('Input.dispatchKeyEvent', { type: text ? 'keyDown' : 'rawKeyDown', text, ...base });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
}

if (command === 'eval') {
  const response = await send('Runtime.evaluate', {
    expression: arg,
    awaitPromise: true,
    replMode: true,
    returnByValue: true,
  });
  const result = response.result;
  if (result?.exceptionDetails) {
    console.log('EXCEPTION', JSON.stringify(result.exceptionDetails, null, 2));
  } else {
    console.log(JSON.stringify(result?.result?.value, null, 2));
  }
} else if (command === 'shot') {
  const [file, ...options] = rest;
  const full = options.includes('full') ? 'full' : undefined;
  // WIDTHxHEIGHT: the page laid out at that size (in CSS pixels), for this
  // capture only, to see how a bigger or smaller window would show it.
  const size = options.map((o) => /^(\d+)x(\d+)$/.exec(o)).find(Boolean);
  // feature=value: emulated for this capture (prefers-color-scheme=light,
  // forced-colors=active…).
  const media = options
    .filter((o) => /^[a-z-]+=[a-z-]+$/.test(o))
    .map((o) => {
      const [name, value] = o.split('=');
      return { name, value };
    });
  if (media.length > 0) await send('Emulation.setEmulatedMedia', { features: media });
  if (size) {
    await send('Emulation.setDeviceMetricsOverride', {
      width: Number(size[1]),
      height: Number(size[2]),
      deviceScaleFactor: 1,
      mobile: false,
    });
    await new Promise((resolve) => setTimeout(resolve, 300));
  }
  const params = { format: 'png' };
  if (full === 'full') {
    const metrics = await send('Page.getLayoutMetrics');
    const { width, height } = metrics.result.cssContentSize;
    params.captureBeyondViewport = true;
    params.clip = { x: 0, y: 0, width, height, scale: 1 };
  }
  const response = await send('Page.captureScreenshot', params);
  fs.writeFileSync(file, Buffer.from(response.result.data, 'base64'));
  console.log('saved', file);
} else if (command === 'keys') {
  for (const chord of rest) await press(chord);
  const focused = await send('Runtime.evaluate', {
    expression:
      '(() => { const e = document.activeElement; return e ? `${e.tagName} ${e.getAttribute("role") ?? ""} ${(e.getAttribute("aria-label") ?? e.innerText ?? "").slice(0, 60)}`.trim() : null })()',
    returnByValue: true,
  });
  console.log('focus:', focused.result?.result?.value);
} else if (command === 'ax') {
  await send('Accessibility.enable');
  const tree = await send('Accessibility.getFullAXTree');
  const roles = new Set(rest);
  for (const node of tree.result.nodes) {
    const role = node.role?.value;
    if (node.ignored || !role || (roles.size > 0 && !roles.has(role))) continue;
    const states = (node.properties ?? [])
      .filter((p) => p.value?.value !== false && p.value?.value !== undefined)
      .map((p) => `${p.name}=${p.value.value}`)
      .join(' ');
    console.log(`${role} "${node.name?.value ?? ''}" ${states}`.trim());
  }
} else if (command === 'media') {
  const features = rest
    .filter((r) => r !== 'none')
    .map((r) => {
      const [name, value] = r.split('=');
      return { name, value };
    });
  await send('Emulation.setEmulatedMedia', { features });
  console.log('media', features.length ? rest.join(' ') : 'reset');
}
ws.close();
