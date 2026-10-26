import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// Tauri expects a fixed port and must see Rust errors, so the screen is not cleared.
export default defineConfig({
  root: 'ui',
  // The Svelte config sits at the repository root (where svelte-check and
  // ESLint look for it), outside the Vite root.
  plugins: [svelte({ configFile: '../svelte.config.js' })],
  clearScreen: false,
  // 1430, not the 1420 of Tauri's template: other Tauri projects in
  // development on the same computer use that one, and the window loads
  // whatever answers there (tauri.conf.json, devUrl).
  server: { port: 1430, strictPort: true, watch: { ignored: ['**/src-tauri/**', '**/target/**'] } },
  build: { outDir: '../dist', emptyOutDir: true, target: 'es2022' },
  test: { include: ['src/**/*.test.ts'] },
});
