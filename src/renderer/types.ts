export interface WalletSummary {
  balance: number
  totalCost: number
  requestCount: number
}

export interface UsageLogItem {
  time: string
  timestamp?: number
  tokenName: string
  model: string
  inputTokens: number
  outputTokens: number
  cost: number
}

export interface BalanceSnapshot {
  wallet: WalletSummary
  recentLogs: UsageLogItem[]
  source: 'xiaoma' | 'mock'
  status: 'ok' | 'cookie_missing' | 'auth_error' | 'parse_error' | 'network_error' | 'new_api_user_missing' | 'balance_insufficient'
  timestamp: number
  logError?: string
}

// 兼容旧 historyStore / alertService 调用，不删除旧类型
export interface BalanceRecord {
  balance: number
  lastCost: number | null
  todayCost: number | null
  totalCost: number | null
  requestCount: number | null
  tokenUsed: number | null
  timestamp: number
  source: 'xiaoma' | 'mock'
  // 新字段（由 snapshot 填入）
  snapshot?: BalanceSnapshot
}

export interface AppSettings {
  providerType: 'mock' | 'xiaoma'
  baseUrl: string
  cookie: string
  apiToken: string
  newApiUser: string
  refreshInterval: number
  lowBalanceThreshold: number
  enableNotification: boolean
  alwaysOnTop: boolean
  autoLaunch: boolean
  windowX: number
  windowY: number
  debugMode: boolean
  // 运行时注入，不持久化到 settings.json
  _sessionCookie?: string
}

export const DEFAULT_SETTINGS: AppSettings = {
  providerType: 'mock',
  baseUrl: '',
  cookie: '',
  apiToken: '',
  newApiUser: '',
  refreshInterval: 60,
  lowBalanceThreshold: 5,
  enableNotification: true,
  alwaysOnTop: true,
  autoLaunch: false,
  windowX: -1,
  windowY: -1,
  debugMode: false,
}
