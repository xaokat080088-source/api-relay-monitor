import { Provider, ProviderContext } from './base'
import { BalanceRecord, BalanceSnapshot } from '../../types'
import { tauriAPI, XiaomaSnapshot } from '../tauriAPI'

// 极智 API（jizhiapi.site）：New-API 前端但接口路径不同，认证走 Bearer JWT。
// 用户信息：GET /api/v1/auth/me
// 使用记录：GET /api/v1/usage
// JWT 填在 profile.apiToken 字段；Cookie 可选做兜底。
// 全部走 Rust 后端 jizhi_fetch，绕过 WebView CORS。

export const JizhiProvider: Provider = {
  name: 'jizhi',

  async fetch({ profile, debugMode }: ProviderContext): Promise<BalanceRecord> {
    const bearer = (profile.apiToken || '').trim()
    const cookie = profile._sessionCookie || profile.cookie || ''
    const baseUrl = (profile.baseUrl || 'https://jizhiapi.site').trim()
    const debug = debugMode ?? false

    let snap: XiaomaSnapshot
    try {
      snap = await tauriAPI.jizhiFetch(baseUrl, bearer, cookie || null, debug)
    } catch (e) {
      const msg = String(e)
      const status: BalanceSnapshot['status'] =
        msg.includes('auth_error') ? 'auth_error' :
        msg.includes('cookie_missing') ? 'cookie_missing' :
        msg.includes('parse_error') ? 'parse_error' :
        'network_error'
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'jizhi',
        status,
        timestamp: Date.now(),
        logError: msg,
      }
      throw Object.assign(new Error(msg.toUpperCase()), { snapshot })
    }

    if (snap.status === 'cookie_missing') {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'jizhi',
        status: 'cookie_missing',
        timestamp: Date.now(),
      }
      throw Object.assign(new Error('COOKIE_MISSING'), { snapshot })
    }

    if (snap.status === 'auth_error') {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'jizhi',
        status: 'auth_error',
        timestamp: Date.now(),
        logError: snap.logError ?? undefined,
      }
      throw Object.assign(new Error('AUTH_ERROR'), { snapshot })
    }

    if (snap.status === 'balance_insufficient') {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'jizhi',
        status: 'balance_insufficient',
        timestamp: Date.now(),
        logError: snap.logError ?? undefined,
      }
      throw Object.assign(new Error('BALANCE_INSUFFICIENT'), { snapshot })
    }

    const recentLogs = snap.recentLogs.map((item) => ({
      time: item.time,
      timestamp: item.timestamp,
      tokenName: item.tokenName,
      model: item.model,
      inputTokens: item.inputTokens,
      outputTokens: item.outputTokens,
      cost: item.cost,
    }))

    const snapshotStatus: BalanceSnapshot['status'] =
      snap.status === 'ok' ? 'ok' : 'parse_error'

    const snapshot: BalanceSnapshot = {
      wallet: {
        balance: snap.wallet.balance,
        totalCost: snap.wallet.totalCost,
        requestCount: snap.wallet.requestCount,
      },
      recentLogs,
      source: 'jizhi',
      status: snapshotStatus,
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
      source: 'jizhi',
      snapshot,
    }
  },
}
