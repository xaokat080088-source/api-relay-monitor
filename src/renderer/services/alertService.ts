import { AppSettings, BalanceRecord } from '../types'
import { tauriAPI } from './tauriAPI'

const COOLDOWN_MS = 10 * 60 * 1000 // 10 minutes between notifications
let lastAlertTs = 0

export function checkLowBalance(record: BalanceRecord, settings: AppSettings) {
  if (!settings.enableNotification) return
  if (record.balance <= 0) return
  if (record.balance < settings.lowBalanceThreshold) {
    const now = Date.now()
    if (now - lastAlertTs >= COOLDOWN_MS) {
      lastAlertTs = now
      tauriAPI.notifyLowBalance(record.balance)
    }
  }
}
