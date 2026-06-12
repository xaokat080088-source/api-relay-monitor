'use strict';

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  getSettings:    () => ipcRenderer.invoke('settings:get'),
  setSettings:    (s) => ipcRenderer.invoke('settings:set', s),
  getHistory:     () => ipcRenderer.invoke('history:get'),
  appendHistory:  (r) => ipcRenderer.invoke('history:append', r),
  clearHistory:   () => ipcRenderer.invoke('history:clear'),

  hideWindow:     () => ipcRenderer.send('window:hide'),
  showWindow:     () => ipcRenderer.send('window:show'),
  openSettings:   () => ipcRenderer.send('window:openSettings'),
  openUrl:        (url) => ipcRenderer.send('window:openUrl', url),
  moveWindow:     (x, y) => ipcRenderer.send('window:move', x, y),

  alertLowBalance: (balance) => ipcRenderer.send('alert:lowBalance', balance),
  applyOnTop:     (v) => ipcRenderer.send('settings:applyOnTop', v),

  onRefresh: (cb) => {
    ipcRenderer.on('cmd:refresh', cb);
    return () => ipcRenderer.removeListener('cmd:refresh', cb);
  },
});
