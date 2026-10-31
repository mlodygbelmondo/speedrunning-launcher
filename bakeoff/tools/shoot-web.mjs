// Usage: [SHOOT_LANG=pl] node bakeoff/tools/shoot-web.mjs <baseUrl> <outDir> [ids...]
// SHOOT_LANG appends `&lang=<lang>` to each deep link; output names stay web-<id>.png.
// Headless Chrome via CDP: 1280x800 CSS px, DPR 1, deep link each screen id, settle, screenshot.
import { spawn } from 'node:child_process';
import { writeFileSync, mkdtempSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [base, outDir, ...only] = process.argv.slice(2);
const IDS = ['home','home-rsg','home-fsg','home-ssg','home-draftout','library','onboarding-welcome','onboarding-signin','onboarding-signin-code','onboarding-mode',
  'onboarding-java','onboarding-eye','onboarding-obs','onboarding-done','accounts','modes','modes-ssg','check','practice','practice-end',
  'learn','tools','stats','submit','settings','game'];
const ids = only.length ? only : IDS;
mkdirSync(outDir, { recursive: true });
const langArg = process.env.SHOOT_LANG ? `&lang=${encodeURIComponent(process.env.SHOOT_LANG)}` : '';
const port = 9333 + Math.floor(Math.random() * 500);
const prof = mkdtempSync(join(tmpdir(), 'srl-chrome-'));
const chrome = spawn('/usr/bin/google-chrome', ['--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${prof}`,
  '--hide-scrollbars=false', '--no-first-run', '--window-size=1280,800', 'about:blank'], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let ws, nextId = 1; const pending = new Map();
try {
  let list;
  for (let i = 0; i < 50; i++) { try { list = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); break; } catch { await sleep(200); } }
  const page = list.find((t) => t.type === 'page');
  ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener('open', r));
  ws.addEventListener('message', (e) => { const m = JSON.parse(e.data); if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); } });
  const send = (method, params = {}) => new Promise((r) => { const id = nextId++; pending.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
  const evaluate = async (expr) => (await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true })).result?.result?.value;
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false });
  const probe = `(() => {
    const out = [];
    const vw = innerWidth, vh = innerHeight;
    if (document.documentElement.scrollHeight > vh || document.documentElement.scrollWidth > vw) out.push('WINDOW SCROLL ' + document.documentElement.scrollWidth + 'x' + document.documentElement.scrollHeight);
    for (const el of document.querySelectorAll('body *')) {
      const cs = getComputedStyle(el);
      if (cs.display === 'none' || cs.visibility === 'hidden') continue;
      const r = el.getBoundingClientRect();
      if (!r.width) continue;
      const scrolls = cs.overflowY === 'auto' || cs.overflowY === 'scroll';
      if (!scrolls && el.children.length === 0 && el.textContent.trim() && (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 2) && cs.overflow !== 'visible')
        out.push('CLIPPED ' + el.className + ' :: ' + el.textContent.trim().slice(0, 50));
      if (el.closest('.scroll, .list, .layers')) continue;
      if ((r.right > vw + 0.5 || r.bottom > vh + 0.5) && !el.closest('.glow') && !el.classList.contains('glow')) out.push('OUTSIDE ' + el.tagName + '.' + el.className + ' ' + Math.round(r.right) + ',' + Math.round(r.bottom));
    }
    return [innerWidth + 'x' + innerHeight + '@' + devicePixelRatio, ...out.slice(0, 15)];
  })()`;
  for (const id of ids) {
    const [sid, ...actions] = id.split('+');
    await send('Page.navigate', { url: 'about:blank' });
    await sleep(100);
    await send('Page.navigate', { url: `${base}#screen=${sid}${langArg}` });
    await sleep(1200);
    for (const a of actions) { await evaluate(a); await sleep(500); }
    await evaluate('document.fonts.ready.then(() => 1)');
    await sleep(300);
    const info = await evaluate(probe);
    const shot = await send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width: 1280, height: 800, scale: 1 } });
    const name = actions.length ? id.split('+')[0] + '-' + (process.env.SUFFIX || 'state') : sid;
    writeFileSync(join(outDir, `web-${name}.png`), Buffer.from(shot.result.data, 'base64'));
    console.log(name, JSON.stringify(info));
  }
} finally {
  ws?.close();
  chrome.kill('SIGTERM');
}
