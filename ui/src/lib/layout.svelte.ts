// How wide the window is, for what the layout shows by itself.

import { MediaQuery } from 'svelte/reactivity';

/**
 * Wide enough for a pane beside a list (the detail of a file, the history
 * of a reference). Narrower, the pane goes below the list, where it takes
 * room from it: it then opens only when asked for. Same width as the
 * stylesheets' breakpoint (1400 CSS px).
 */
export const wideWindow = new MediaQuery('min-width: 1401px');
