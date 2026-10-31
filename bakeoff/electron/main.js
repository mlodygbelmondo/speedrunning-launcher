// Electron shell for the Speedrunluncher bake-off. See bakeoff/SPEC.md.
'use strict';
const tStart = performance.timeOrigin + performance.now();
const fs = require('node:fs');
const path = require('node:path');
const { app, BrowserWindow, ipcMain, Menu } = require('electron');

const STACK = 'electron';
const OUT = process.env.BAKEOFF_OUT || '';
const PHASE_MS = Number.parseInt(process.env.BAKEOFF_PHASE_MS || '', 10) || 20000;
const scenario = OUT !== '';
// Deep-link screen id (SCREENS.md "Screen ids"); ignored in scenario mode.
const SCREEN = scenario ? null : process.env.BAKEOFF_SCREEN?.trim() || null;
const LANG = scenario ? null : process.env.BAKEOFF_LANG?.trim() || null; // start language: en | pl
const outFd = scenario ? fs.openSync(OUT, 'a') : null;

/** Append one JSON line; writeSync is unbuffered, so every line is flushed immediately. */
function line(s) {
  if (outFd !== null) fs.writeSync(outFd, s.trimEnd() + '\n');
}
const emit = (obj) => line(JSON.stringify(obj));
const now = () => performance.timeOrigin + performance.now();

emit({
  ev: 'start',
  t: tStart,
  stack: STACK,
  pid: process.pid,
  version: `electron ${process.versions.electron} / chromium ${process.versions.chrome}`,
});

// Packaged: resources/app/scene. Dev (`pnpm start`): ../scene/dist.
const packagedScene = path.join(__dirname, 'scene', 'index.html');
const scenePath = fs.existsSync(packagedScene)
  ? packagedScene
  : path.join(__dirname, '..', 'scene', 'dist', 'index.html');

let win = null;
let destroyedByScenario = false;

ipcMain.on('bakeoff:report', (_e, s) => line(String(s)));
ipcMain.handle('bakeoff:config', () => ({ scenario, phaseMs: PHASE_MS, stack: STACK, screen: SCREEN, lang: LANG }));
ipcMain.handle('bakeoff:minimize', () => {
  win?.minimize();
});
ipcMain.handle('bakeoff:destroy', () => {
  destroyedByScenario = true;
  setImmediate(() => {
    win?.destroy();
    win = null;
  });
  // The host owns the rest of the `destroyed` phase.
  setTimeout(() => {
    emit({ ev: 'done', t: now() });
    if (outFd !== null) fs.closeSync(outFd);
    app.exit(0);
  }, PHASE_MS);
});

// Keep the process alive after the scenario destroys the window ("launcher sits in the tray").
app.on('window-all-closed', () => {
  if (!destroyedByScenario) app.quit();
});

Menu.setApplicationMenu(null);

app.whenReady().then(() => {
  win = new BrowserWindow({
    width: 1280,
    height: 800,
    useContentSize: true,
    resizable: true,
    title: `Speedrunluncher bake-off (${STACK})`,
    backgroundColor: '#0A090E',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
  });
  win.loadFile(scenePath);

  if (scenario) {
    app
      .getGPUInfo('basic')
      .then((gpu) =>
        emit({
          ev: 'info',
          source: 'host',
          platform: `${process.platform}-${process.arch}`,
          gpuFeatures: app.getGPUFeatureStatus(),
          gpuDevices: gpu.gpuDevice,
        }),
      )
      .catch(() => {});
  }
});
