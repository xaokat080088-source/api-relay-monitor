import { BalanceRecord, StationProfile } from '../../types'

// provider 拿到的是单个中转站配置 + 全局 debug 开关
export interface ProviderContext {
  profile: StationProfile
  debugMode: boolean
}

export interface Provider {
  fetch(ctx: ProviderContext): Promise<BalanceRecord>
  name: string
}
