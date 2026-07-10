import { BalanceRecord } from '../types'
import { tauriAPI } from './tauriAPI'

export class HistoryStore {
  async getAll(profileId: string): Promise<BalanceRecord[]> {
    try {
      return (await tauriAPI.getHistory(profileId)) ?? []
    } catch {
      return []
    }
  }

  async append(profileId: string, record: BalanceRecord): Promise<BalanceRecord[]> {
    return tauriAPI.appendHistory(profileId, record)
  }

  async clear(profileId: string): Promise<BalanceRecord[]> {
    return tauriAPI.clearHistory(profileId)
  }

  getTodayCostFromHistory(history: BalanceRecord[]): number {
    const today = new Date()
    today.setHours(0, 0, 0, 0)
    const todayTs = today.getTime()
    return history
      .filter((r) => r.timestamp >= todayTs)
      .reduce((sum, r) => sum + (r.lastCost ?? 0), 0)
  }
}

export const historyStore = new HistoryStore()
