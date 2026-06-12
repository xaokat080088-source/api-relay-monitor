import { BalanceRecord } from '../types'
import { tauriAPI } from './tauriAPI'

export class HistoryStore {
  async getAll(): Promise<BalanceRecord[]> {
    try {
      return (await tauriAPI.getHistory()) ?? []
    } catch {
      return []
    }
  }

  async append(record: BalanceRecord): Promise<BalanceRecord[]> {
    return tauriAPI.appendHistory(record)
  }

  async clear(): Promise<BalanceRecord[]> {
    return tauriAPI.clearHistory()
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
