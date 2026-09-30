import { Provider, ProviderContext } from './base'
import { BalanceRecord, BalanceSnapshot } from '../../types'
import { tauriAPI } from '../tauriAPI'

// 极智新版 API（jizhiapi.site 2026-08 更新后）
// 认证：Authorization: Bearer <JWT>
// 用户信息：GET /api/v1/auth/me → data.balance
// 使用记录：GET /api/v1/usage → data.items[]，actual_cost 为实际费用，created_at 为 ISO 时间戳

export const JizhiNewProvider: Provider = {
  name: 'jizhi_new',

  async fetch({ profile, debugMode }: ProviderContext): Promise<BalanceRecord> {
    const bearer = (profile.apiToken || '').trim()
    const baseUrl = (profile.baseUrl || 'https://jizhiapi.site').trim()
    // 配置了自动登录账号时，token 为空也可由 Rust 侧自动登录获取
    const hasAutoLogin = !!(profile.jizhiUsername || '').trim()

    if (!bearer && !hasAutoLogin) {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'jizhi_new',
        status: 'cookie_missing',
        timestamp: Date.now(),
        logError: '未填写 Token，也未配置自动登录账号',
      }
      throw Object.assign(new Error('COOKIE_MISSING'), { snapshot })
    }

    // 调用 Rust 后端的 jizhi_new_fetch 命令（Token 过期时后端自动账密续期）
    try {
      const snap = await tauriAPI.jizhiNewFetch(baseUrl, bearer, profile.id, debugMode ?? false)

      // 后端自动续期成功：把新 token 回写 settings（settings.json 由 Rust 侧已写，这里同步兜底）
      if (snap.renewedToken) {
        try {
          await tauriAPI.updateProfileToken(profile.id, snap.renewedToken)
        } catch {
          // 回写失败不影响本次数据展示，下次续期会再试
        }
      }

      if (snap.status === 'auth_error') {
        const snapshot: BalanceSnapshot = {
          wallet: { balance: 0, totalCost: 0, requestCount: 0 },
          recentLogs: [],
          source: 'jizhi_new',
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
          source: 'jizhi_new',
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
        source: 'jizhi_new',
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
        source: 'jizhi_new',
        snapshot,
      }
    } catch (e: any) {
      if (e.snapshot) throw e
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'jizhi_new',
        status: 'network_error',
        timestamp: Date.now(),
        logError: String(e),
      }
      throw Object.assign(new Error('NETWORK_ERROR'), { snapshot })
    }
  },
}
