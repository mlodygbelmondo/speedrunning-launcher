// Scenario driver (SPEC.md "Scenario mode"). Runs only when the host says scenario mode is on.
import type { Bridge, Config } from './bridge';
import type { Page } from './state.svelte';

export interface SceneControl {
  navigate(to: Page): Promise<void>;
  setGlow(on: boolean): void;
  scroller(): HTMLElement | undefined;
  current(): Page;
}

const epoch = () => performance.timeOrigin + performance.now();
const r2 = (x: number) => Math.round(x * 100) / 100;

// Wall-clock sleep. Timers run in a dedicated worker so the hidden/minimised page's timer
// throttling (Chromium aligns hidden-page timers to 1 s) does not stretch the phases.
const sleep: (ms: number) => Promise<void> = (() => {
  try {
    const src = 'onmessage=e=>setTimeout(()=>postMessage(e.data[0]),e.data[1])';
    const w = new Worker(URL.createObjectURL(new Blob([src], { type: 'text/javascript' })));
    const pending = new Map<number, () => void>();
    let next = 0;
    w.onmessage = (e: MessageEvent<number>) => {
      pending.get(e.data)?.();
      pending.delete(e.data);
    };
    return (ms: number) =>
      new Promise<void>((resolve) => {
        const id = next++;
        pending.set(id, resolve);
        w.postMessage([id, ms]);
      });
  } catch {
    return (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
  }
})();

/** rAF loop for a whole phase; records performance.now() (relative to phase start) per callback. */
function frameLoop(phaseStart: number, onFrame?: (elapsed: number) => void) {
  const ts: number[] = [];
  let running = true;
  const cb = () => {
    if (!running) return;
    const el = performance.now() - phaseStart;
    ts.push(r2(el));
    onFrame?.(el);
    requestAnimationFrame(cb);
  };
  requestAnimationFrame(cb);
  return {
    stop(): number[] {
      running = false;
      return ts;
    },
  };
}

export function doubleRaf(): Promise<void> {
  return new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));
}

export async function runScenario(cfg: Config, bridge: Bridge, scene: SceneControl): Promise<void> {
  const ms = cfg.phaseMs;
  const phase = (name: string) => {
    bridge.report({ ev: 'phase', name, t: epoch() });
    return performance.now();
  };

  // 1. idle: Home static, no glow animation, no frames requested.
  scene.setGlow(false);
  phase('idle');
  await sleep(ms);

  // 2. transitions: glow on, toggle Home <-> Library every 1000 ms, frame loop for the whole phase.
  scene.setGlow(true);
  let t0 = phase('transitions');
  let loop = frameLoop(t0);
  const toggle = setInterval(() => {
    scene.navigate(scene.current() === 'home' ? 'library' : 'home');
  }, 1000);
  await sleep(ms);
  clearInterval(toggle);
  bridge.report({ ev: 'frames', phase: 'transitions', ts: loop.stop() });

  // 3. scroll: Library, 1200 px/s time-based, bouncing between top and bottom.
  t0 = phase('scroll');
  scene.navigate('library');
  const SPEED = 1.2; // px per ms
  loop = frameLoop(t0, (elapsed) => {
    const el = scene.scroller();
    if (!el) return;
    const max = el.scrollHeight - el.clientHeight;
    if (max <= 0) return;
    const d = (elapsed * SPEED) % (2 * max);
    el.scrollTop = d <= max ? d : 2 * max - d;
  });
  await sleep(ms);
  bridge.report({ ev: 'frames', phase: 'scroll', ts: loop.stop() });

  // 4. minimized: glow keeps "running"; count rAF callbacks to see whether the host throttles.
  phase('minimized');
  await bridge.minimize();
  let rafCount = 0;
  let counting = true;
  const count = () => {
    if (!counting) return;
    rafCount++;
    requestAnimationFrame(count);
  };
  requestAnimationFrame(count);
  await sleep(ms);
  counting = false;
  bridge.report({ ev: 'info', phase: 'minimized', raf_count: rafCount, visibility: document.visibilityState });

  // 5. destroyed: the host destroys the window, owns the timer, emits `done` and exits.
  scene.setGlow(false);
  phase('destroyed');
  await bridge.destroy();
}
