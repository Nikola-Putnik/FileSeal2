import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// Tauri expects a fixed port and must see Rust errors, so the screen is not cleared.
export default defineConfig({
  root: 'ui',
  // The Svelte config sits at the repository root (where svelte-check and
  // ESLint look for it), outside the Vite root.
  plugins: [svelte({ configFile: '../svelte.config.js' })],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**', '**/target/**'] } },
  build: { outDir: '../dist', emptyOutDir: true, target: 'es2022' },
  test: { include: ['src/**/*.test.ts'] },
});
