// Host bridge. One scene build serves both shells:
//  - Electron: the preload exposes `window.bakeoff` via contextBridge (sandbox + contextIsolation).
//  - Tauri: detected via `window.__TAURI_INTERNALS__`, bridge built on `invoke`.
//  - Plain browser (vite dev / Playwright): a stub. `?scenario=1&phaseMs=2000` runs the scenario,
//    reports go to console.log as JSON lines; `?screen=<id>&lang=pl` (or the same in the hash) deep-links a screen.
import { invoke } from '@tauri-apps/api/core';

export interface Config {
  scenario: boolean;
  phaseMs: number;
  stack: string;
  /** Deep-link screen id (SCREENS.md "Screen ids"), from BAKEOFF_SCREEN. Ignored in scenario mode. */
  screen?: string | null;
  /** Start language ('en' | 'pl'), from BAKEOFF_LANG. Ignored in scenario mode. */
  lang?: string | null;
}

export interface Bridge {
  report(obj: unknown): Promise<void> | void;
  minimize(): Promise<void> | void;
  destroy(): Promise<void> | void;
  config(): Promise<Config> | Config;
}

declare global {
  interface Window {
    bakeoff?: Bridge;
    __TAURI_INTERNALS__?: unknown;
  }
}

function tauriBridge(): Bridge {
  // Serialise every call so JSONL lines land in the order the scene emitted them.
  let chain: Promise<unknown> = Promise.resolve();
  const q = <T>(f: () => Promise<T>): Promise<T> => {
    const p = chain.then(f);
    chain = p.catch(() => undefined);
    return p;
  };
  return {
    report: (obj) => q(() => invoke<void>('report', { line: JSON.stringify(obj) })),
    minimize: () => q(() => invoke<void>('minimize')),
    destroy: () => q(() => invoke<void>('destroy')),
    config: () => q(() => invoke<Config>('config')),
  };
}

function browserBridge(): Bridge {
  const qs = new URLSearchParams(location.search);
  const cfg: Config = {
    scenario: qs.get('scenario') === '1',
    phaseMs: Number(qs.get('phaseMs') ?? 20000),
    stack: 'web',
    screen: qs.get('screen'),
    lang: qs.get('lang'),
  };
  return {
    report: (obj) => console.log('BAKEOFF ' + JSON.stringify(obj)),
    minimize: () => console.log('BAKEOFF minimize()'),
    destroy: () => console.log('BAKEOFF destroy()'),
    config: () => cfg,
  };
}

export function getBridge(): Bridge {
  if (window.bakeoff) return window.bakeoff; // Electron preload
  const b = window.__TAURI_INTERNALS__ ? tauriBridge() : browserBridge();
  window.bakeoff = b;
  return b;
}
