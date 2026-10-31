// Every UI string comes from assets/copy.json (en) and copy.pl.json (pl); SCREENS.md "Language".
import en from '../../assets/copy.json';
import pl from '../../assets/copy.pl.json';

export type Lang = 'en' | 'pl';
export const LANGS: Lang[] = ['en', 'pl']; // same order as settings.languages
const COPY: Record<Lang, Record<string, string>> = { en, pl };

/** Current language (reactive). `stack` feeds the window title. */
export const lang = $state({ cur: 'en' as Lang, stack: '' });

export function setLang(l: string | null | undefined): void {
  if (l === 'en' || l === 'pl') lang.cur = l;
}

/** Look up `key` (current language, then English, then the key) and fill `{name}` placeholders from `args`. */
export function t(key: string, args?: Record<string, string | number>): string {
  const s = COPY[lang.cur][key] ?? en[key as keyof typeof en] ?? key;
  if (!args) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in args ? String(args[k]) : m));
}
