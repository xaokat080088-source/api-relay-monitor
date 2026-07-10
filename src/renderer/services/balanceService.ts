import { AppSettings, BalanceRecord, StationProfile, getActiveProfile } from '../types'
import { getProvider } from './provider'
import { historyStore } from './historyStore'

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

  // 返回当前激活 profile（供外部读取 id）
  getActiveProfile(): StationProfile | null {
    return this.settings ? getActiveProfile(this.settings) : null
  }

  async refresh(): Promise<BalanceRecord | null> {
    if (!this.settings) return null
    const profile = getActiveProfile(this.settings)
    if (!profile) return null
    try {
      const provider = getProvider(profile.providerType)
      const record = await provider.fetch({ profile, debugMode: this.settings.debugMode })
      record.profileId = profile.id
      const history = await historyStore.append(profile.id, record)
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
