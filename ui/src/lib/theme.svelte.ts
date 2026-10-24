// The colour theme: the system's (light or dark, as Windows), or one chosen
// in the preferences. Kept in settings.json; also kept in the page's
// storage, only so that the next start shows the right colours at once,
// before the settings are read.

export const THEMES = ['system', 'light', 'dark', 'oled'] as const;
export type Theme = (typeof THEMES)[number];

const STORAGE_KEY = 'fileseal.theme';

let current = $state<Theme>('system');

export function isTheme(value: unknown): value is Theme {
  return typeof value === 'string' && (THEMES as readonly string[]).includes(value);
}

/** The theme in use. */
export function theme(): Theme {
  return current;
}

/** Colours the page: `data-theme` on the root, none for the system's. */
export function applyTheme(next: Theme) {
  current = next;
  const root = document.documentElement;
  if (next === 'system') delete root.dataset.theme;
  else root.dataset.theme = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // Storage refused: the settings still have it, read a moment later.
  }
}

/** At start-up, the theme of the last session, before the settings are read. */
export function restoreTheme() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (isTheme(saved)) applyTheme(saved);
  } catch {
    // Storage refused: the system's colours until the settings are read.
  }
}
