import { Provider } from './base'
import { BalanceRecord, BalanceSnapshot, UsageLogItem, AppSettings } from '../../types'

const MODELS = [
  'example-model-pro',
  'example-model-mini',
  'demo-llm-large',
  'demo-llm-fast',
]
const TOKEN_NAMES = ['demo-key', 'test-key', 'dev-key']

let _balance = 10.0
let _totalCost = 0.0
let _requestCount = 0
const _logBuffer: UsageLogItem[] = []

function pad(n: number) {
  return String(n).padStart(2, '0')
}

function fmtTime(d: Date) {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

export const MockProvider: Provider = {
  name: 'mock',

  async fetch(_settings: AppSettings): Promise<BalanceRecord> {
    const inputTokens = Math.floor(Math.random() * 50000 + 10000)
    const outputTokens = Math.floor(Math.random() * 200 + 50)
    const cost = parseFloat((Math.random() * 0.05 + 0.15).toFixed(6))

    _balance = parseFloat((_balance - cost).toFixed(4))
    _totalCost = parseFloat((_totalCost + cost).toFixed(4))
    _requestCount++

    const now = Date.now()
    const newLog: UsageLogItem = {
      time: fmtTime(new Date()),
      timestamp: now,
      tokenName: TOKEN_NAMES[Math.floor(Math.random() * TOKEN_NAMES.length)],
      model: MODELS[Math.floor(Math.random() * MODELS.length)],
      inputTokens,
      outputTokens,
      cost,
    }

    _logBuffer.unshift(newLog)
    if (_logBuffer.length > 20) _logBuffer.pop()

    const snapshot: BalanceSnapshot = {
      wallet: {
        balance: _balance,
        totalCost: _totalCost,
        requestCount: _requestCount,
      },
      recentLogs: [..._logBuffer],
      source: 'mock',
      status: 'ok',
      timestamp: Date.now(),
    }

    return {
      balance: _balance,
      lastCost: cost,
      todayCost: null,
      totalCost: _totalCost,
      requestCount: _requestCount,
      tokenUsed: inputTokens + outputTokens,
      timestamp: Date.now(),
      source: 'mock',
      snapshot,
    }
  },
}
