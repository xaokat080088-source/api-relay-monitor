import {
  app,
  BrowserWindow,
  ipcMain,
  Tray,
  Menu,
  nativeImage,
  shell,
  Notification,
  screen,
} from 'electron'
import * as path from 'node:path'
import * as fs from 'node:fs'

function isDev() { return !app.isPackaged }

interface AppSettings {
  providerType: 'mock' | 'xiaoma'
  baseUrl: string
  cookie: string
  apiToken: string
  refreshInterval: number
  lowBalanceThreshold: number
  enableNotification: boolean
  alwaysOnTop: boolean
  autoLaunch: boolean
  windowX: number
  windowY: number
}

const DEFAULT_SETTINGS: AppSettings = {
  providerType: 'mock',
  baseUrl: '',
  cookie: '',
  apiToken: '',
  refreshInterval: 60,
  lowBalanceThreshold: 5,
  enableNotification: true,
  alwaysOnTop: true,
  autoLaunch: false,
  windowX: -1,
  windowY: -1,
}

function getDataDir() { return app.getPath('userData') }

function readJSON<T>(file: string, fallback: T): T {
  try { return JSON.parse(fs.readFileSync(file, 'utf-8')) as T }
  catch { return fallback }
}

function writeJSON(file: string, data: unknown) {
  fs.mkdirSync(path.dirname(file), { recursive: true })
  fs.writeFileSync(file, JSON.stringify(data, null, 2), 'utf-8')
}

function settingsFile() { return path.join(getDataDir(), 'settings.json') }
function historyFile()  { return path.join(getDataDir(), 'history.json') }

function getSettings(): AppSettings {
  return { ...DEFAULT_SETTINGS, ...readJSON<Partial<AppSettings>>(settingsFile(), {}) }
}
function saveSettings(s: AppSettings) { writeJSON(settingsFile(), s) }
function getHistory(): unknown[] { return readJSON<unknown[]>(historyFile(), []) }
function saveHistory(h: unknown[]) { writeJSON(historyFile(), h) }

let floatingWin: InstanceType<typeof BrowserWindow> | null = null
let settingsWin: InstanceType<typeof BrowserWindow> | null = null
let tray: InstanceType<typeof Tray> | null = null
let lastAlertTime = 0

function createFloatingWindow() {
  const settings = getSettings()
  const { width: sw } = screen.getPrimaryDisplay().workAreaSize
  const x = settings.windowX >= 0 ? settings.windowX : sw - 240
  const y = settings.windowY >= 0 ? settings.windowY : 20

  floatingWin = new BrowserWindow({
    width: 230,
    height: 150,
    x,
    y,
    frame: false,
    transparent: true,
    alwaysOnTop: settings.alwaysOnTop,
    skipTaskbar: true,
    resizable: false,
    hasShadow: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  if (isDev()) {
    floatingWin.loadURL('http://localhost:5174/?page=floating')
  } else {
    floatingWin.loadFile(path.join(__dirname, '../renderer/index.html'), {
      query: { page: 'floating' },
    })
  }

  floatingWin.on('moved', () => {
    if (!floatingWin) return
    const [wx, wy] = floatingWin.getPosition()
    saveSettings({ ...getSettings(), windowX: wx, windowY: wy })
  })

  floatingWin.on('closed', () => { floatingWin = null })
}

function createSettingsWindow() {
  if (settingsWin) { settingsWin.focus(); return }

  settingsWin = new BrowserWindow({
    width: 480,
    height: 580,
    frame: true,
    transparent: false,
    alwaysOnTop: false,
    skipTaskbar: false,
    resizable: false,
    title: 'API Monitor 设置',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  if (isDev()) {
    settingsWin.loadURL('http://localhost:5174/?page=settings')
  } else {
    settingsWin.loadFile(path.join(__dirname, '../renderer/index.html'), {
      query: { page: 'settings' },
    })
  }

  settingsWin.on('closed', () => { settingsWin = null })
}

function createTray() {
  const iconPath = path.join(__dirname, '../../assets/icon.png')
  let icon: ReturnType<typeof nativeImage.createFromPath>
  try {
    icon = nativeImage.createFromPath(iconPath)
    if (icon.isEmpty()) throw new Error('empty')
  } catch {
    icon = nativeImage.createEmpty()
  }

  tray = new Tray(icon)
  tray.setToolTip('API Monitor')
  updateTrayMenu()
  tray.on('double-click', () => {
    if (floatingWin) floatingWin.show()
    else createFloatingWindow()
  })
}

function updateTrayMenu() {
  if (!tray) return
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: '显示悬浮窗', click: () => { if (floatingWin) floatingWin.show(); else createFloatingWindow() } },
    { label: '刷新余额',   click: () => { floatingWin?.webContents.send('cmd:refresh') } },
    { type: 'separator' },
    { label: '设置',       click: () => createSettingsWindow() },
    { label: '打开中转站', click: () => { const u = (getSettings().baseUrl || '').trim(); if (u) shell.openExternal(u) } },
    { type: 'separator' },
    { label: '退出',       click: () => app.quit() },
  ]))
}

app.whenReady().then(() => {
  ipcMain.handle('settings:get', () => getSettings())
  ipcMain.handle('settings:set', (_e: unknown, patch: Partial<AppSettings>) => {
    const next = { ...getSettings(), ...patch }
    saveSettings(next)
    floatingWin?.setAlwaysOnTop(next.alwaysOnTop)
    return next
  })

  ipcMain.handle('history:get', () => getHistory())
  ipcMain.handle('history:append', (_e: unknown, record: unknown) => {
    const h = [...getHistory(), record].slice(-200)
    saveHistory(h)
    return h
  })
  ipcMain.handle('history:clear', () => { saveHistory([]); return [] })

  ipcMain.on('window:hide',         () => floatingWin?.hide())
  ipcMain.on('window:show',         () => floatingWin?.show())
  ipcMain.on('window:openSettings', () => createSettingsWindow())
  ipcMain.on('window:openUrl',      (_e: unknown, url: string) => shell.openExternal(url))
  ipcMain.on('window:move',         (_e: unknown, x: number, y: number) => floatingWin?.setPosition(Math.round(x), Math.round(y)))

  ipcMain.on('alert:lowBalance', (_e: unknown, balance: number) => {
    const s = getSettings()
    if (!s.enableNotification) return
    const now = Date.now()
    if (now - lastAlertTime < 10 * 60 * 1000) return
    lastAlertTime = now
    new Notification({
      title: 'API Monitor — 余额不足',
      body: `当前余额 ¥${balance.toFixed(2)}，已低于阈值 ¥${s.lowBalanceThreshold}`,
    }).show()
  })

  ipcMain.on('settings:applyOnTop', (_e: unknown, onTop: boolean) => floatingWin?.setAlwaysOnTop(onTop))

  createFloatingWindow()
  createTray()
})

app.on('window-all-closed', () => { /* stay in tray */ })
app.on('activate', () => { if (!floatingWin) createFloatingWindow() })
