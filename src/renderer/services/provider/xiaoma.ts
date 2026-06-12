import { Provider } from './base'
import { BalanceRecord, BalanceSnapshot, AppSettings } from '../../types'
import { tauriAPI, XiaomaSnapshot } from '../tauriAPI'

// XiaomaProvider 不再在渲染进程直接 fetch（会被 WebView CORS 拦截）。
// 全部走 Rust 后端 xiaoma_fetch command，由 reqwest 发起请求。

export const XiaomaProvider: Provider = {
  name: 'xiaoma',

  async fetch(settings: AppSettings): Promise<BalanceRecord> {
    const cookie = settings._sessionCookie || settings.cookie || ''
    const apiToken = settings.apiToken || ''
    const newApiUser = settings.newApiUser || ''
    const baseUrl = (settings.baseUrl || '').trim()
    const debug = settings.debugMode ?? false

    let snap: XiaomaSnapshot
    try {
      snap = await tauriAPI.xiaomaFetch(baseUrl, cookie, apiToken || null, newApiUser || null, debug)
    } catch (e) {
      // Rust 侧抛出字符串错误（network_error / parse_error 等）
      const msg = String(e)
      const status: BalanceSnapshot['status'] =
        msg.includes('auth_error') ? 'auth_error' :
        msg.includes('cookie_missing') ? 'cookie_missing' :
        msg.includes('parse_error') ? 'parse_error' :
        'network_error'

      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'xiaoma',
        status,
        timestamp: Date.now(),
        logError: msg,
      }
      throw Object.assign(new Error(msg.toUpperCase()), { snapshot })
    }

    // cookie_missing / auth_error 时 Rust 返回 Ok(snapshot) 而非 Err
    if (snap.status === 'cookie_missing') {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'xiaoma',
        status: 'cookie_missing',
        timestamp: Date.now(),
      }
      throw Object.assign(new Error('COOKIE_MISSING'), { snapshot })
    }

    if (snap.status === 'new_api_user_missing') {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'xiaoma',
        status: 'new_api_user_missing',
        timestamp: Date.now(),
        logError: snap.logError ?? undefined,
      }
      throw Object.assign(new Error('NEW_API_USER_MISSING'), { snapshot })
    }

    if (snap.status === 'balance_insufficient') {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'xiaoma',
        status: 'balance_insufficient',
        timestamp: Date.now(),
        logError: snap.logError ?? undefined,
      }
      throw Object.assign(new Error('BALANCE_INSUFFICIENT'), { snapshot })
    }

    if (snap.status === 'auth_error') {
      const snapshot: BalanceSnapshot = {
        wallet: { balance: 0, totalCost: 0, requestCount: 0 },
        recentLogs: [],
        source: 'xiaoma',
        status: 'auth_error',
        timestamp: Date.now(),
        logError: snap.logError ?? undefined,
      }
      throw Object.assign(new Error('AUTH_ERROR'), { snapshot })
    }

    // 转换 Rust camelCase 字段到前端类型
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
      source: 'xiaoma',
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
      source: 'xiaoma',
      snapshot,
    }
  },
}
