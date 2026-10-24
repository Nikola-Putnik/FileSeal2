import { describe, expect, it } from 'vitest';
import { en } from './en';
import { fr } from './fr';

// Every leaf of a dictionary, as "path → value" (functions are called with
// sample arguments so their text can be compared too).
function leaves(value: unknown, path = ''): Map<string, string> {
  const found = new Map<string, string>();
  if (typeof value === 'function') {
    found.set(path, String(value(1, 'X', 'Y', 'Z')));
  } else if (Array.isArray(value)) {
    value.forEach((item, i) => found.set(`${path}[${i}]`, String(item)));
  } else if (typeof value === 'object' && value !== null) {
    for (const [key, child] of Object.entries(value)) {
      for (const [p, v] of leaves(child, path ? `${path}.${key}` : key)) found.set(p, v);
    }
  } else {
    found.set(path, String(value));
  }
  return found;
}

describe('dictionaries', () => {
  const english = leaves(en);
  const french = leaves(fr);

  it('have the same keys', () => {
    expect([...french.keys()].sort()).toEqual([...english.keys()].sort());
  });

  it('French is actually translated', () => {
    // Texts that are the same in both languages on purpose.
    const same = new Set([
      'language.label',
      'language.en',
      'language.fr',
      'action.label',
      'settings.destinations',
      'createStatus.partialSuccess.label',
      'result.destinations',
      'units.perSecond',
      'list.sortKeys.extension',
      'selection.extensions',
      'launch.pause',
      'dbScreen.versions',
      'about.version',
      'about.portable',
    ]);
    const untranslated = [...english]
      .filter(([path, text]) => !same.has(path) && french.get(path) === text)
      .map(([path]) => path);
    expect(untranslated).toEqual([]);
  });
});
