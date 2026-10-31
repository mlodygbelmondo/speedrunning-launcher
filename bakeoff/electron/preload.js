// Sandboxed preload (contextIsolation + sandbox): exposes the host bridge as window.bakeoff.
'use strict';
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('bakeoff', {
  report: (obj) => ipcRenderer.send('bakeoff:report', JSON.stringify(obj)),
  config: () => ipcRenderer.invoke('bakeoff:config'), // { scenario, phaseMs, stack, screen, lang }
  minimize: () => ipcRenderer.invoke('bakeoff:minimize'),
  destroy: () => ipcRenderer.invoke('bakeoff:destroy'),
});
