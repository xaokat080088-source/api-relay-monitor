import { BalanceRecord, AppSettings } from '../../types'

export interface Provider {
  fetch(settings: AppSettings): Promise<BalanceRecord>
  name: string
}
