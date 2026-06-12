import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('electronAPI', {
  getSettings: () => ipcRenderer.invoke('settings:get'),
  setSettings: (s: unknown) => ipcRenderer.invoke('settings:set', s),
  getHistory: () => ipcRenderer.invoke('history:get'),
  appendHistory: (r: unknown) => ipcRenderer.invoke('history:append', r),
  clearHistory: () => ipcRenderer.invoke('history:clear'),

  hideWindow: () => ipcRenderer.send('window:hide'),
  showWindow: () => ipcRenderer.send('window:show'),
  openSettings: () => ipcRenderer.send('window:openSettings'),
  openUrl: (url: string) => ipcRenderer.send('window:openUrl', url),
  moveWindow: (x: number, y: number) => ipcRenderer.send('window:move', x, y),

  alertLowBalance: (balance: number) => ipcRenderer.send('alert:lowBalance', balance),
  applyOnTop: (v: boolean) => ipcRenderer.send('settings:applyOnTop', v),

  onRefresh: (cb: () => void) => {
    ipcRenderer.on('cmd:refresh', cb)
    return () => ipcRenderer.removeListener('cmd:refresh', cb)
  },
})
