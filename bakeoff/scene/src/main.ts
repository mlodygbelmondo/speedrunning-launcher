import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { getBridge } from './bridge';
import { lang, setLang } from './copy.svelte';
import { doubleRaf, runScenario } from './scenario';

const bridge = getBridge();
const app = mount(App, { target: document.getElementById('app')! });

(async () => {
  const cfgP = Promise.resolve(bridge.config());
  // Deep link and start language (interactive mode only; scenario mode is always English on Home):
  // host BAKEOFF_SCREEN / BAKEOFF_LANG, or #screen=<id>&lang=pl in the URL.
  cfgP.then((cfg) => {
    lang.stack = cfg.stack;
    if (cfg.scenario) return;
    const hash = () => new URLSearchParams(location.hash.slice(1));
    setLang(hash().get('lang') || cfg.lang);
    const id = hash().get('screen') || cfg.screen;
    if (id) app.openScreen(id);
    addEventListener('hashchange', () => {
      const h = hash();
      setLang(h.get('lang'));
      const s = h.get('screen');
      if (s) app.openScreen(s);
    });
  });
  await doubleRaf();
  await bridge.report({ ev: 'first_frame', t: performance.timeOrigin + performance.now() });
  const cfg = await cfgP;
  await bridge.report({
    ev: 'info',
    source: 'scene',
    ua: navigator.userAgent,
    dpr: window.devicePixelRatio,
    viewport: [window.innerWidth, window.innerHeight],
    screen: [screen.width, screen.height],
  });
  if (cfg.scenario) await runScenario(cfg, bridge, app);
})().catch((e) => {
  console.error(e);
  bridge.report({ ev: 'error', message: String(e?.stack ?? e) });
});
