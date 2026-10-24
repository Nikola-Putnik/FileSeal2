// Current language, reactive: components read `t()` in their markup and
// update when the language changes. English is the default.

import { en, type Messages } from './en';
import { fr } from './fr';

export type Locale = 'en' | 'fr';

export const LOCALES: Record<Locale, Messages> = { en, fr };

/** BCP 47 tags for number and date formatting. */
const INTL: Record<Locale, string> = { en: 'en-GB', fr: 'fr-FR' };

const current = $state<{ locale: Locale }>({ locale: 'en' });

export function isLocale(value: unknown): value is Locale {
  return typeof value === 'string' && value in LOCALES;
}

export function locale(): Locale {
  return current.locale;
}

export function intlLocale(): string {
  return INTL[current.locale];
}

export function setLocale(next: Locale) {
  current.locale = next;
  if (typeof document !== 'undefined') document.documentElement.lang = next;
}

/** The messages of the current language. */
export function t(): Messages {
  return LOCALES[current.locale];
}
