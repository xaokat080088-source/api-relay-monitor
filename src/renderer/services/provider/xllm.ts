import { Provider, ProviderContext } from './base'
import { BalanceRecord, BalanceSnapshot } from '../../types'
import { tauriAPI } from '../tauriAPI'

// X 网站 (x-llm.net)
// 认证：Authorization: Bearer <JWT>
// 用户信息：返回 data.quota（剩余额度）、data.used_quota（已用额度）
// 使用记录：GET /api/log/self → data.items[]，quota 为费用（单位 1/500000 美元），created_at 为 Unix 秒时间戳

export const XllmProvider: Provider = {
  name: 'xllm',

  async fetch({ profile, debugMode }: ProviderContext): Promise<BalanceRecord> {
    const bearer = (profile.apiToken || '').trim()
    const baseUrl = (profile.baseUrl || 'https://x-llm.net').trim()

    if (!bearer) {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'xllm',
        status: 'cookie_missing',
        timestamp: Date.now(),
        logError: '未填写 Bearer Token',
      }
      throw Object.assign(new Error('COOKIE_MISSING'), { snapshot })
    }

    // 调用 Rust 后端的 xllm_fetch 命令
    try {
      const snap = await tauriAPI.xllmFetch(baseUrl, bearer, debugMode ?? false)

      if (snap.status === 'auth_error') {
        const snapshot: BalanceSnapshot = {
          wallet: { balance: 0, totalCost: 0, requestCount: 0 },
          recentLogs: [],
          source: 'xllm',
          status: 'auth_error',
          timestamp: Date.now(),
          logError: snap.logError ?? 'Token 无效',
        }
        throw Object.assign(new Error('AUTH_ERROR'), { snapshot })
      }

      if (snap.status !== 'ok') {
        const snapshot: BalanceSnapshot = {
          wallet: { balance: 0, totalCost: 0, requestCount: 0 },
          recentLogs: [],
          source: 'xllm',
          status: (snap.status as BalanceSnapshot['status']) || 'network_error',
          timestamp: Date.now(),
          logError: snap.logError ?? undefined,
        }
        throw Object.assign(new Error(snap.status.toUpperCase()), { snapshot })
      }

      const recentLogs = snap.recentLogs.map((item: any) => ({
        time: item.time,
        timestamp: item.timestamp,
        tokenName: item.tokenName,
        model: item.model,
        inputTokens: item.inputTokens,
        outputTokens: item.outputTokens,
        cost: item.cost,
      }))

      const snapshot: BalanceSnapshot = {
        wallet: {
          balance: snap.wallet.balance,
          totalCost: snap.wallet.totalCost,
          requestCount: snap.wallet.requestCount,
        },
        recentLogs,
        source: 'xllm',
        status: 'ok',
        timestamp: snap.timestamp * 1000,
        logError: snap.logError ?? undefined,
      }

      const latest = recentLogs[0]
      return {
        balance: snap.wallet.balance,
        lastCost: latest?.cost ?? null,
        todayCost: null,
        totalCost: snap.wallet.totalCost,
        requestCount: snap.wallet.requestCount,
        tokenUsed: latest ? latest.inputTokens + latest.outputTokens : null,
        timestamp: snap.timestamp * 1000,
        source: 'xllm',
        snapshot,
      }
    } catch (e: any) {
      if (e.snapshot) throw e
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'xllm',
        status: 'network_error',
        timestamp: Date.now(),
        logError: String(e),
      }
      throw Object.assign(new Error('NETWORK_ERROR'), { snapshot })
    }
  },
}
