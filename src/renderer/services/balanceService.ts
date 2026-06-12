import { AppSettings, BalanceRecord } from '../types'
import { getProvider } from './provider'
import { historyStore } from './historyStore'
import { tauriAPI } from './tauriAPI'

type Listener = (record: BalanceRecord, history: BalanceRecord[]) => void
type ErrorListener = (err: Error) => void

export class BalanceService {
  private timer: ReturnType<typeof setInterval> | null = null
  private listeners: Listener[] = []
  private errorListeners: ErrorListener[] = []
  private settings: AppSettings | null = null

  setSettings(s: AppSettings) {
    this.settings = s
  }

  onUpdate(cb: Listener) {
    this.listeners.push(cb)
    return () => { this.listeners = this.listeners.filter((l) => l !== cb) }
  }

  onError(cb: ErrorListener) {
    this.errorListeners.push(cb)
    return () => { this.errorListeners = this.errorListeners.filter((l) => l !== cb) }
  }

  async refresh(): Promise<BalanceRecord | null> {
    if (!this.settings) return null
    try {
      // 读取 sessionCookie（来自登录窗口捕获），注入到本次调用的 settings 副本
      let sessionCookie = ''
      try {
        sessionCookie = await tauriAPI.getSessionCookie()
      } catch {
        // 读取失败不阻塞，留空让 provider 自己判断
      }

      const settingsWithSession: AppSettings = {
        ...this.settings,
        _sessionCookie: sessionCookie,
      }

      const provider = getProvider(settingsWithSession)
      const record = await provider.fetch(settingsWithSession)
      const history = await historyStore.append(record)
      this.listeners.forEach((l) => l(record, history))
      return record
    } catch (e) {
      const err = e instanceof Error ? e : new Error(String(e))
      this.errorListeners.forEach((l) => l(err))
      return null
    }
  }

  startAutoRefresh(intervalSec: number) {
    this.stopAutoRefresh()
    if (intervalSec <= 0) return
    this.timer = setInterval(() => this.refresh(), intervalSec * 1000)
  }

  stopAutoRefresh() {
    if (this.timer) {
      clearInterval(this.timer)
      this.timer = null
    }
  }
}

export const balanceService = new BalanceService()
