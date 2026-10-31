// Shared prototype state (local only: nothing here does real work).
import { S } from './screens';

export type Page =
  | 'home' | 'library' | 'onboarding' | 'accounts' | 'modes' | 'practice' | 'learn'
  | 'tools' | 'stats' | 'submit' | 'settings' | 'game';

export const ONB_STEPS = ['welcome', 'signin', 'mode', 'java', 'eye', 'obs', 'done'] as const;

export const ui = $state({
  // Text (`ago`) stays in the per-language data; `fresh` = signed in again just now.
  accounts: S.accounts.list.map(({ name, avatar, active, expired }) => ({ name, avatar, active, expired, fresh: false })),
  defaultMode: (S.modes.find((m) => m.default) ?? S.modes[0]).id,
  checkMode: '', // mode shown in the pre-launch check ('' = default mode)
  modeStatus: Object.fromEntries(S.modes.map((m) => [m.id, m.status])) as Record<string, string>,
  installs: {} as Record<string, 'installing' | 'done'>,
  toast: null as { msg: string; id: number } | null,
  checkOpen: false,
  mode: S.v3.default_mode, // scene v3: mode selected on Home and Modes (shared)
  practiceCat: S.v3.practice_default, // scene v3 Practice: category id, or 'tools'
  onbStep: 0,
  onbCode: false, // open the sign-in code panel (deep link onboarding-signin-code)
  onbDeep: false, // opened through a deep link: Next is never blocked on sign-in
  settings: {
    memory: S.settings.memory_default,
    keybinds: S.settings.keybinds_default,
    channel: 0,
    close_on_launch: S.settings.close_on_launch,
    start_with_system: S.settings.start_with_system,
    standard_settings: S.settings.standard_settings,
    auto_update: S.settings.auto_update,
  },
});

let navImpl: (p: Page) => void = () => {};
export function setNavigator(f: (p: Page) => void): void {
  navImpl = f;
}
/** Navigate with the page transition. */
export function go(p: Page): void {
  navImpl(p);
}

export function openCheck(modeId = ''): void {
  ui.checkMode = modeId;
  ui.checkOpen = true;
}

let toastId = 0;
let toastTimer: ReturnType<typeof setTimeout> | undefined;
/** Toast: 150 ms in, 1800 ms shown, 150 ms out (the CSS animation in Toast.svelte matches). */
export function toast(msg: string): void {
  ui.toast = { msg, id: ++toastId };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (ui.toast = null), 2100);
}

/** Fake install: 1200 ms, then Installed. The progress bar is a CSS animation of the same length. */
export function install(key: string, done?: () => void): void {
  if (ui.installs[key]) return;
  ui.installs[key] = 'installing';
  setTimeout(() => {
    ui.installs[key] = 'done';
    done?.();
  }, 1200);
}

/** Effective install state of an item whose data status is `status`. */
export function installState(key: string, status: string): string {
  const s = ui.installs[key];
  return s === 'done' ? 'installed' : s === 'installing' ? 'installing' : status;
}

export function activeAccount() {
  return ui.accounts.find((a) => a.active) ?? ui.accounts[0];
}

export function modeById(id: string) {
  return S.modes.find((m) => m.id === id) ?? S.modes[0];
}
