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

export type ProviderType = 'xiaoma' | 'jizhi' | 'jizhi_new' | 'xllm' | 'mock'

export interface BalanceSnapshot {
  wallet: WalletSummary
  recentLogs: UsageLogItem[]
  source: ProviderType
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
  source: ProviderType
  profileId?: string
  // 新字段（由 snapshot 填入）
  snapshot?: BalanceSnapshot
}

// 单个中转站配置
export interface StationProfile {
  id: string
  name: string
  providerType: ProviderType
  baseUrl: string
  cookie: string
  apiToken: string
  newApiUser: string
  // 运行时注入，不持久化
  _sessionCookie?: string
}

export interface AppSettings {
  profiles: StationProfile[]
  activeProfileId: string
  refreshInterval: number
  lowBalanceThreshold: number
  enableNotification: boolean
  alwaysOnTop: boolean
  autoLaunch: boolean
  windowX: number
  windowY: number
  debugMode: boolean
}

export function makeDefaultProfile(): StationProfile {
  return {
    id: (typeof crypto !== 'undefined' && crypto.randomUUID)
      ? crypto.randomUUID()
      : `p_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
    name: 'Mock（示例数据）',
    providerType: 'mock',
    baseUrl: '',
    cookie: '',
    apiToken: '',
    newApiUser: '',
  }
}

const DEFAULT_PROFILE = makeDefaultProfile()

export const DEFAULT_SETTINGS: AppSettings = {
  profiles: [DEFAULT_PROFILE],
  activeProfileId: DEFAULT_PROFILE.id,
  refreshInterval: 60,
  lowBalanceThreshold: 5,
  enableNotification: true,
  alwaysOnTop: true,
  autoLaunch: false,
  windowX: -1,
  windowY: -1,
  debugMode: false,
}

// 从 settings 取当前激活的 profile；找不到则返回第一个或 null
export function getActiveProfile(settings: AppSettings): StationProfile | null {
  if (!settings.profiles || settings.profiles.length === 0) return null
  return settings.profiles.find((p) => p.id === settings.activeProfileId)
    || settings.profiles[0]
}
