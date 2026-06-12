import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AppSettings, BalanceRecord } from '../types'

export interface SessionStatus {
  hasSession: boolean
  maskedCookie: string
  capturedAt: number
  source: string
}

export interface XiaomaLogItem {
  time: string
  timestamp: number
  tokenName: string
  model: string
  inputTokens: number
  outputTokens: number
  cost: number
}

export interface XiaomaWallet {
  balance: number
  totalCost: number
  requestCount: number
}

export interface XiaomaSnapshot {
  wallet: XiaomaWallet
  recentLogs: XiaomaLogItem[]
  status: string
  logError: string | null
  timestamp: number
  // 调试摘要（不含完整 Cookie/响应体）
  debugUrl: string | null
  debugHttpStatus: number | null
  debugRespKeys: string | null
  debugMessage: string | null
}

export const tauriAPI = {
  // ── 设置 ──────────────────────────────────────────────────
  getSettings: () => invoke<AppSettings>('get_settings'),
  saveSettings: (s: AppSettings) => invoke<AppSettings>('save_settings', { settings: s }),

  // ── 历史 ──────────────────────────────────────────────────
  getHistory: () => invoke<BalanceRecord[]>('get_history'),
  appendHistory: (r: BalanceRecord) => invoke<BalanceRecord[]>('append_history', { record: r }),
  clearHistory: () => invoke<BalanceRecord[]>('clear_history'),

  // ── 窗口 ──────────────────────────────────────────────────
  openUrl: (url: string) => invoke<void>('open_url', { url }),
  showWindow: () => invoke<void>('show_window'),
  hideWindow: () => invoke<void>('hide_window'),
  setAlwaysOnTop: (onTop: boolean) => invoke<void>('set_always_on_top', { onTop }),
  moveWindow: (x: number, y: number) => invoke<void>('move_window', { x, y }),
  notifyLowBalance: (balance: number) => invoke<void>('notify_low_balance', { balance }),
  openSettingsWindow: () => invoke<void>('open_settings_window'),
  closeSettingsWindow: () => invoke<void>('close_settings_window'),
  broadcastSettingsChanged: () => invoke<void>('broadcast_settings_changed'),

  // ── 开机自启动 ────────────────────────────────────────────
  getAutostartEnabled: () => invoke<boolean>('get_autostart_enabled'),
  setAutostartEnabled: (enabled: boolean) => invoke<void>('set_autostart_enabled', { enabled }),

  // ── Cookie 管理（手动粘贴方案）────────────────────────────
  saveSessionCookie: (cookie: string, source: string) =>
    invoke<void>('save_session_cookie', { cookie, source }),
  getSessionStatus: () => invoke<SessionStatus>('get_session_status'),
  getSessionCookie: () => invoke<string>('get_session_cookie'),
  clearSessionCookie: () => invoke<void>('clear_session_cookie'),

  // ── Xiaoma API 代理（Rust reqwest，绕过渲染进程 CORS）──────
  xiaomaFetch: (
    baseUrl: string,
    cookie: string,
    apiToken: string | null,
    newApiUser: string | null,
    debugMode: boolean,
  ) => invoke<XiaomaSnapshot>('xiaoma_fetch', {
    baseUrl,
    cookie,
    apiToken: apiToken || null,
    newApiUser: newApiUser || null,
    debugMode,
  }),

  // ── 事件 ──────────────────────────────────────────────────
  onRefresh: (cb: () => void) => listen('cmd:refresh', cb),
  onSettingsChanged: (cb: () => void) => listen('cmd:settings-changed', cb),
  onSessionCaptured: (cb: () => void) =>
    listen('session:captured', () => cb()),
}
