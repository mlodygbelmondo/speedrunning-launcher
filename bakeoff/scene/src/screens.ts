// Sample data for the prototype screens: assets/screens.json, plus the Polish overlay screens.pl.json.
import raw from '../../assets/screens.json';
import plOverlay from '../../assets/screens.pl.json';
import { lang, type Lang } from './copy.svelte';

export type Os = 'windows' | 'linux' | 'macos';
const ua = navigator.userAgent;
export const os: Os = /Windows/i.test(ua) ? 'windows' : /Mac OS|Macintosh/i.test(ua) ? 'macos' : 'linux';

export interface Account { name: string; avatar: string; active: boolean; expired: boolean; ago: string }
export interface Mode {
  id: string; name: string; full: string; icon: string; mc: string; status: string;
  default: boolean; recommended: boolean; desc: string; leaderboard: string; note?: string; layers: string[];
}
export interface Layer { name: string; detail?: string; mods: string[][] }
export interface Check { id: string; status: string; args: Record<string, string> }
export interface MapItem {
  name: string; version: string; authors: string; includes?: string; category?: string; icon?: string;
  downloads?: string; status: string; desc: string;
}
export interface Tutorial {
  title: string; creator: string; kind: string; length: string; level: string; date: string; current: boolean; url: string;
}
export interface TutorialGroup { title: string; icon: string; items: Tutorial[] }
export interface Tool {
  id: string; name: string; version: string; authors: string; icon: string; platform: string; desc: string;
  installed: boolean; pinned: boolean; start_with_game: boolean; auto_update: boolean; newer?: string; external?: boolean;
}
export interface Run { date: string; mode: string; finished: boolean; pb: boolean; result: string; splits: [string, string, boolean][] }

export interface V3Split { name: string; art: string; secs: number; time: string; best: string }
export interface V3Run { pb: string; pb_date: string; sob: string; runs_month: number; splits: V3Split[] }
export interface V3 {
  season: number;
  online: string;
  default_mode: string;
  modes: Record<string, { short: string; art: string; mods_total: number; disk: string; update_mod?: string; download?: string }>;
  ranked: {
    elo: number; elo_delta: string; tier: string; season_w: number; season_l: number; best: string;
    matches: { opponent: string; avatar: string; won: boolean; time: string; elo: string }[];
    weekly: { n: number; place: number; total: string; time: string };
  };
  runs: Record<string, V3Run>;
  install: Record<string, { new: string[]; have: string[] }>;
  practice_art: Record<string, string>;
  practice_default: string;
}

export type Screens = {
  v3: V3;
  accounts: {
    store: Record<Os, string>; device_code: string; device_url: string; device_qr: string[]; list: Account[];
  };
  modes: Mode[];
  layers: Record<string, Layer>;
  disk_saved: string;
  checks: Check[];
  practice_categories: { id: string; name: string; icon: string }[];
  maps: MapItem[];
  practice_tools: MapItem[];
  tutorial_groups: TutorialGroup[];
  tools: Tool[];
  tools_checked_ago: string;
  stats: {
    total_runs: string;
    tiles: Record<string, string>;
    splits: string[];
    runs: Run[];
  };
  submit: {
    time: string; category: string; date: string; igt: string; rta: string; files: [string, string][];
    zip: string; zip_size: string; sample_link: string;
  };
  settings: {
    languages: string[]; memory: string[]; memory_default: number; keybinds: string[]; keybinds_default: number;
    sensitivity: string; channels: string[]; data_folder: Record<Os, string>;
    close_on_launch: boolean; start_with_system: boolean; standard_settings: boolean; auto_update: boolean;
  };
  game: { mode: string; started: string; resets: string; best: string; nether_entries: string };
  onboarding: { obs_version: string; signed_in_as: string };
};

/** Overlay merge: objects by key, arrays by index, null keeps the base value, anything else replaces it. */
function merge(base: unknown, over: unknown): unknown {
  if (over === null || over === undefined) return base;
  if (Array.isArray(base) && Array.isArray(over)) {
    const n = Math.max(base.length, over.length);
    return Array.from({ length: n }, (_, i) => merge(base[i], over[i]));
  }
  const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
  if (isObj(base) && isObj(over)) {
    const out: Record<string, unknown> = { ...base };
    for (const k of Object.keys(over)) out[k] = merge(base[k], over[k]);
    return out;
  }
  return over;
}

const DATA: Record<Lang, Screens> = {
  en: raw as unknown as Screens,
  pl: merge(raw, plOverlay) as Screens,
};

/** Data set of the current language. Reading through it is reactive (it reads `lang.cur`). */
export const S = new Proxy({} as Screens, {
  get: (_t, k) => DATA[lang.cur][k as keyof Screens],
});
