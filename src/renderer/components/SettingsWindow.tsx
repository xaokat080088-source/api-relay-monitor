import React, { useEffect, useState, useCallback } from 'react'
import {
  Save, Eye, EyeOff, RefreshCw, Trash2,
  CheckCircle, XCircle, Wifi, X, ExternalLink, AlertCircle,
} from 'lucide-react'
import { Plus } from 'lucide-react'
import { AppSettings, DEFAULT_SETTINGS, StationProfile, ProviderType, makeDefaultProfile } from '../types'
import { balanceService } from '../services/balanceService'
import { historyStore } from '../services/historyStore'
import { tauriAPI } from '../services/tauriAPI'

// ── 样式常量 ──────────────────────────────────────────────────

const inputStyle: React.CSSProperties = {
  width: '100%',
  background: 'rgba(255,255,255,0.06)',
  border: '1px solid rgba(255,255,255,0.1)',
  borderRadius: 5,
  padding: '5px 8px',
  color: '#f0f0f0',
  fontSize: 12,
  outline: 'none',
  boxSizing: 'border-box',
}

const btnBase: React.CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 5,
  borderRadius: 5,
  padding: '5px 12px',
  fontSize: 12,
  cursor: 'pointer',
  border: 'none',
  fontFamily: 'inherit',
}

// ── 子组件 ────────────────────────────────────────────────────

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: 'flex', alignItems: 'flex-start', marginBottom: 10 }}>
      <label style={{ width: 110, fontSize: 12, color: '#8a8a9a', flexShrink: 0, paddingTop: 5 }}>
        {label}
      </label>
      <div style={{ flex: 1 }}>{children}</div>
    </div>
  )
}

function SectionTitle({ title }: { title: string }) {
  return (
    <div style={{
      fontSize: 10, color: '#5a5a6a', marginBottom: 10,
      textTransform: 'uppercase', letterSpacing: '0.05em',
    }}>
      {title}
    </div>
  )
}

// ── 测试连接结果 ──────────────────────────────────────────────

interface TestResult {
  ok: boolean
  balance?: number
  totalCost?: number
  requestCount?: number
  logCount?: number
  error?: string
  // 调试摘要
  debugUrl?: string
  debugHttpStatus?: number
  debugRespKeys?: string
  debugMessage?: string
  errorType?: 'COOKIE' | 'AUTH' | 'NET' | 'ERROR' | 'NEW_API_USER' | 'BALANCE'
}

// ── 主组件 ────────────────────────────────────────────────────

export default function SettingsWindow() {
  // 全局设置（含 profiles 数组 + activeProfileId + 全局字段）
  const [g, setG] = useState<AppSettings>(DEFAULT_SETTINGS)
  // 当前正在编辑的 profile id
  const [editingId, setEditingId] = useState<string>('')
  const [loaded, setLoaded] = useState(false)
  const [loadError, setLoadError] = useState('')
  const [showCookie, setShowCookie] = useState(false)
  const [showToken, setShowToken] = useState(false)
  const [saved, setSaved] = useState(false)
  const [clearing, setClearing] = useState(false)
  const [autostartError, setAutostartError] = useState('')

  // 测试连接
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<TestResult | null>(null)

  // 当前编辑中的 profile
  const editing: StationProfile | undefined = g.profiles.find((p) => p.id === editingId)

  // ── 加载设置 ──────────────────────────────────────────────

  const reload = useCallback(async () => {
    try {
      const [raw, sysAutostart] = await Promise.all([
        tauriAPI.getSettings(),
        tauriAPI.getAutostartEnabled().catch(() => null),
      ])
      const profiles = (raw.profiles && raw.profiles.length > 0)
        ? raw.profiles
        : DEFAULT_SETTINGS.profiles
      const activeProfileId = profiles.some((p) => p.id === raw.activeProfileId)
        ? raw.activeProfileId
        : profiles[0].id
      const settings: AppSettings = {
        profiles,
        activeProfileId,
        debugMode: Boolean(raw.debugMode),
        refreshInterval: Number(raw.refreshInterval) || DEFAULT_SETTINGS.refreshInterval,
        lowBalanceThreshold: Number(raw.lowBalanceThreshold) ?? DEFAULT_SETTINGS.lowBalanceThreshold,
        enableNotification: Boolean(raw.enableNotification ?? DEFAULT_SETTINGS.enableNotification),
        alwaysOnTop: Boolean(raw.alwaysOnTop ?? DEFAULT_SETTINGS.alwaysOnTop),
        autoLaunch: sysAutostart !== null ? sysAutostart : Boolean(raw.autoLaunch ?? false),
        windowX: raw.windowX ?? -1,
        windowY: raw.windowY ?? -1,
      }
      setG(settings)
      // 默认编辑当前激活的 profile
      setEditingId(activeProfileId)
      setLoaded(true)
      setLoadError('')
    } catch (e) {
      console.error('[Settings] load error:', String(e))
      setLoadError(`设置加载失败：${String(e)}`)
      setLoaded(true)
    }
  }, [])

  useEffect(() => {
    reload()
  }, [reload])

  // 更新全局字段
  const updateGlobal = (patch: Partial<AppSettings>) => setG((prev) => ({ ...prev, ...patch }))

  // 更新当前编辑 profile 的字段
  const updateProfile = (patch: Partial<StationProfile>) => {
    setG((prev) => ({
      ...prev,
      profiles: prev.profiles.map((p) => (p.id === editingId ? { ...p, ...patch } : p)),
    }))
  }

  // ── Profile 管理 ──────────────────────────────────────────

  const handleAddProfile = () => {
    const np: StationProfile = {
      ...makeDefaultProfile(),
      name: `中转站 ${g.profiles.length + 1}`,
      providerType: 'xiaoma',
    }
    setG((prev) => ({ ...prev, profiles: [...prev.profiles, np] }))
    setEditingId(np.id)
    setTestResult(null)
  }

  const handleDeleteProfile = () => {
    if (g.profiles.length <= 1) return
    const remain = g.profiles.filter((p) => p.id !== editingId)
    const nextActive = g.activeProfileId === editingId ? remain[0].id : g.activeProfileId
    setG((prev) => ({ ...prev, profiles: remain, activeProfileId: nextActive }))
    setEditingId(remain[0].id)
    setTestResult(null)
    // 清掉被删 profile 的历史文件（可选，忽略失败）
    tauriAPI.clearHistory(editingId).catch(() => {})
  }

  const switchEditing = (id: string) => {
    setEditingId(id)
    setTestResult(null)
    setShowCookie(false)
    setShowToken(false)
  }

  const setActive = (id: string) => {
    updateGlobal({ activeProfileId: id })
  }

  // ── 登录状态文案 ──────────────────────────────────────────

  function authStatusInfo(): { text: string; color: string; icon: React.ReactNode } {
    if (!loaded || !editing) return { text: '读取中…', color: '#5a5a6a', icon: null }
    if (editing.providerType === 'mock') {
      return { text: 'Mock 示例数据，无需认证', color: '#8a8a9a', icon: <AlertCircle size={12} /> }
    }
    if (!editing.cookie && !editing.apiToken) {
      return { text: '未配置认证信息', color: '#f59e42', icon: <XCircle size={12} /> }
    }
    return {
      text: editing.apiToken ? '已填写 Token（未验证）' : '已填写 Cookie（未验证）',
      color: '#8a8a9a',
      icon: <AlertCircle size={12} />,
    }
  }

  const asi = authStatusInfo()

  // ── 打开官网 ──────────────────────────────────────────────

  const handleOpenWeb = () => {
    const url = (editing?.baseUrl || '').trim()
    if (url) tauriAPI.openUrl(url)
  }

  // ── 清除 Cookie ───────────────────────────────────────────

  const handleClearCookie = async () => {
    updateProfile({ cookie: '', apiToken: '' })
    setTestResult(null)
  }

  // ── 测试连接（走 Rust 后端，绕过渲染进程 CORS）─────────────

  const handleTest = async () => {
    if (!editing) return
    setTesting(true)
    setTestResult(null)

    const effectiveCookie = editing.cookie?.trim() || ''
    const effectiveToken = editing.apiToken?.trim() || ''
    const effectiveNewApiUser = editing.newApiUser?.trim() || ''
    const baseUrl = (editing.baseUrl || '').trim()

    try {
      if (editing.providerType === 'mock') {
        setTestResult({ ok: true, balance: 0, totalCost: 0, requestCount: 0, logCount: 0 })
        return
      }

      if (!baseUrl) {
        setTestResult({
          ok: false,
          error: '请先填写中转站地址（例如 https://example.com）',
          errorType: 'ERROR',
        })
        return
      }

      let snap
      if (editing.providerType === 'jizhi') {
        // 极智：Bearer JWT 认证，填在 API Token 字段
        if (!effectiveToken) {
          setTestResult({
            ok: false,
            error: '极智 API 需要 Bearer Token，请把 Authorization 里的 JWT 填到 API Token 字段',
            errorType: 'AUTH',
          })
          return
        }
        snap = await tauriAPI.jizhiFetch(baseUrl, effectiveToken, effectiveCookie || null, g.debugMode ?? false)
      } else {
        // xiaoma / New-API 系列：Cookie + New-Api-User
        if (!effectiveCookie && !effectiveToken) {
          setTestResult({ ok: false, error: 'Cookie 未填写，请先粘贴 Cookie 再测试', errorType: 'COOKIE' })
          return
        }
        if (effectiveCookie && !effectiveCookie.includes('=')) {
          setTestResult({
            ok: false,
            error: 'Cookie 格式不正确，请使用 session=xxx 的格式，不要只粘贴值',
            errorType: 'COOKIE',
          })
          return
        }
        if (!effectiveNewApiUser && !effectiveToken) {
          setTestResult({
            ok: false,
            error: '部分中转站接口需要 New-Api-User 请求头，请从浏览器 Network 请求头中复制（见下方引导）',
            errorType: 'AUTH',
          })
          return
        }
        snap = await tauriAPI.xiaomaFetch(
          baseUrl,
          effectiveCookie,
          effectiveToken || null,
          effectiveNewApiUser || null,
          g.debugMode ?? false,
        )
      }

      const dbg = {
        debugUrl: snap.debugUrl ?? undefined,
        debugHttpStatus: snap.debugHttpStatus ?? undefined,
        debugRespKeys: snap.debugRespKeys ?? undefined,
        debugMessage: snap.debugMessage ?? undefined,
      }

      if (snap.status === 'cookie_missing') {
        setTestResult({ ok: false, error: 'Cookie 未填写', errorType: 'COOKIE', ...dbg })
        return
      }
      if (snap.status === 'new_api_user_missing') {
        setTestResult({
          ok: false,
          error: '缺少 New-Api-User 请求头，请从浏览器 Network 请求头中复制（见下方引导）',
          errorType: 'NEW_API_USER',
          ...dbg,
        })
        return
      }
      if (snap.status === 'balance_insufficient') {
        setTestResult({
          ok: false,
          error: snap.logError ? `余额/额度不足：${snap.logError}` : '余额/额度不足，请充值后重试',
          errorType: 'BALANCE',
          ...dbg,
        })
        return
      }
      if (snap.status === 'auth_error') {
        const msg = snap.logError || ''
        const errText = msg.includes('未提供 New-Api-User') || msg.includes('New-Api-User')
          ? '缺少 New-Api-User 请求头，请从浏览器 Network 复制'
          : msg.includes('额度不足') || msg.includes('余额不足')
          ? `余额/额度不足：${msg}`
          : msg
          ? `认证失败：${msg}`
          : 'Cookie 无效或权限不足，请重新从浏览器复制'
        const errType = (msg.includes('未提供 New-Api-User') || msg.includes('New-Api-User'))
          ? 'NEW_API_USER'
          : (msg.includes('额度不足') || msg.includes('余额不足'))
          ? 'BALANCE'
          : 'AUTH'
        setTestResult({ ok: false, error: errText, errorType: errType, ...dbg })
        return
      }
      if (snap.status === 'network_error') {
        setTestResult({
          ok: false,
          error: snap.logError ? `网络失败：${snap.logError}` : '网络请求失败',
          errorType: 'NET',
          ...dbg,
        })
        return
      }
      if (snap.status === 'parse_error') {
        setTestResult({
          ok: false,
          error: snap.logError ? `字段解析失败：${snap.logError}` : '字段解析失败',
          errorType: 'ERROR',
          ...dbg,
        })
        return
      }

      setTestResult({
        ok: true,
        balance: snap.wallet.balance,
        totalCost: snap.wallet.totalCost,
        requestCount: snap.wallet.requestCount,
        logCount: snap.recentLogs.length,
        ...dbg,
      })
    } catch (e) {
      const msg = String(e)
      let errorType: TestResult['errorType'] = 'ERROR'
      let error = `错误：${msg}`
      if (msg.includes('new_api_user_missing') || msg.includes('New-Api-User')) {
        error = '缺少 New-Api-User 请求头'
        errorType = 'NEW_API_USER'
      } else if (msg.includes('balance_insufficient') || msg.includes('额度不足') || msg.includes('余额不足')) {
        error = `余额/额度不足：${msg}`
        errorType = 'BALANCE'
      } else if (msg.includes('auth_error')) {
        error = 'Cookie 无效或权限不足'
        errorType = 'AUTH'
      } else if (msg.includes('network_error')) {
        error = `网络失败：${msg}`
        errorType = 'NET'
      } else if (msg.includes('parse_error')) {
        error = `解析失败：${msg}`
        errorType = 'ERROR'
      }
      setTestResult({ ok: false, error, errorType })
    } finally {
      setTesting(false)
    }
  }

  // ── 保存 ──────────────────────────────────────────────────

  const handleSave = async () => {
    // 剥离运行时字段 _sessionCookie
    const cleanProfiles: StationProfile[] = g.profiles.map(({ _sessionCookie: _sc, ...p }) => p)
    const toSave: AppSettings = { ...g, profiles: cleanProfiles }
    const result = await tauriAPI.saveSettings(toSave)
    setG(result)
    if (!result.profiles.some((p) => p.id === editingId) && result.profiles[0]) {
      setEditingId(result.profiles[0].id)
    }
    tauriAPI.setAlwaysOnTop(result.alwaysOnTop)
    try {
      await tauriAPI.setAutostartEnabled(result.autoLaunch)
      setAutostartError('')
    } catch (e) {
      setAutostartError(`开机启动设置失败：${String(e)}`)
    }
    balanceService.setSettings(result)
    balanceService.stopAutoRefresh()
    balanceService.startAutoRefresh(result.refreshInterval)
    await tauriAPI.broadcastSettingsChanged()
    setSaved(true)
    setTimeout(() => setSaved(false), 1800)
  }

  const handleClearHistory = async () => {
    if (!editing) return
    setClearing(true)
    await historyStore.clear(editing.id)
    setClearing(false)
  }

  const sec = (style?: React.CSSProperties): React.CSSProperties => ({
    marginBottom: 18,
    paddingBottom: 14,
    borderBottom: '1px solid rgba(255,255,255,0.06)',
    ...style,
  })

  // ── 渲染 ──────────────────────────────────────────────────

  if (!loaded) {
    return (
      <div style={{
        background: '#141418', color: '#8a8a9a',
        minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center',
        fontFamily: "-apple-system,'Segoe UI',sans-serif", fontSize: 13,
      }}>
        加载中…
      </div>
    )
  }

  if (loadError) {
    return (
      <div style={{
        background: '#141418', color: '#f87171',
        minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center',
        fontFamily: "-apple-system,'Segoe UI',sans-serif", fontSize: 13, padding: 24,
        textAlign: 'center',
      }}>
        {loadError}
      </div>
    )
  }

  return (
    <div style={{
      background: '#141418',
      color: '#f0f0f0',
      fontFamily: "-apple-system,'Segoe UI',sans-serif",
      fontSize: 12,
      minHeight: '100%',
      padding: '20px 24px 88px',
      boxSizing: 'border-box',
    }}>
      <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 20, color: '#e0e0e0' }}>
        API Monitor 设置
      </div>

      {/* ── 中转站管理 ── */}
      <div style={sec()}>
        <SectionTitle title="中转站管理" />
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 12 }}>
          {g.profiles.map((p) => {
            const isEditing = p.id === editingId
            const isActive = p.id === g.activeProfileId
            return (
              <button
                key={p.id}
                onClick={() => switchEditing(p.id)}
                style={{
                  ...btnBase,
                  padding: '5px 10px',
                  background: isEditing ? 'rgba(124,111,247,0.22)' : 'rgba(255,255,255,0.05)',
                  border: `1px solid ${isActive ? 'rgba(124,111,247,0.6)' : 'rgba(255,255,255,0.1)'}`,
                  color: isEditing ? '#a89ff9' : '#c0c0cc',
                }}
                title={isActive ? '当前正在监控此中转站' : '点击编辑'}
              >
                {isActive && <CheckCircle size={11} style={{ color: '#4ade80' }} />}
                {p.name || '未命名'}
              </button>
            )
          })}
          <button
            onClick={handleAddProfile}
            style={{
              ...btnBase,
              padding: '5px 10px',
              background: 'rgba(34,197,94,0.1)',
              border: '1px dashed rgba(34,197,94,0.4)',
              color: '#4ade80',
            }}
          >
            <Plus size={12} /> 添加中转站
          </button>
        </div>

        {editing && editingId !== g.activeProfileId && (
          <button
            onClick={() => setActive(editingId)}
            style={{
              ...btnBase, marginBottom: 10,
              background: 'rgba(124,111,247,0.15)',
              border: '1px solid rgba(124,111,247,0.4)',
              color: '#a89ff9',
            }}
          >
            <CheckCircle size={12} /> 设为当前监控（保存后生效）
          </button>
        )}

        <Row label="配置名称">
          <input
            style={inputStyle}
            value={editing?.name ?? ''}
            onChange={(e) => updateProfile({ name: e.target.value })}
            placeholder="例如：小马API、极智API"
          />
        </Row>
        <Row label="适配器类型">
          <select
            value={editing?.providerType ?? 'mock'}
            onChange={(e) => updateProfile({ providerType: e.target.value as ProviderType })}
            style={{ ...inputStyle, cursor: 'pointer' }}
          >
            <option value="mock">Mock（本地假数据）</option>
            <option value="xiaoma">小马 / New API / One API（Cookie 认证）</option>
            <option value="jizhi">极智 API（jizhiapi.site，Token 认证）</option>
          </select>
        </Row>
        {editing?.providerType !== 'mock' && (
          <Row label="中转站地址">
            <input
              style={inputStyle}
              value={editing?.baseUrl ?? ''}
              onChange={(e) => updateProfile({ baseUrl: e.target.value })}
              placeholder={editing?.providerType === 'jizhi' ? 'https://jizhiapi.site' : 'https://example.com'}
            />
          </Row>
        )}
      </div>

      {/* ── 认证 ── */}
      {editing && editing.providerType !== 'mock' && (
      <div style={sec()}>
        <SectionTitle title={editing.providerType === 'jizhi' ? '认证 Token' : '认证 Cookie'} />

        {/* 状态提示 */}
        <div style={{
          display: 'flex', alignItems: 'center', gap: 6,
          marginBottom: 12, padding: '6px 10px',
          background: 'rgba(255,255,255,0.04)',
          borderRadius: 6, border: '1px solid rgba(255,255,255,0.06)',
        }}>
          <span style={{ color: asi.color, display: 'flex', alignItems: 'center', gap: 4 }}>
            {asi.icon}
            <span>{asi.text}</span>
          </span>
        </div>

        {/* Cookie 输入框（极智不需要 Cookie，可留空） */}
        <Row label="Cookie">
          <div style={{ position: 'relative' }}>
            <input
              style={{ ...inputStyle, paddingRight: 30, fontFamily: 'monospace' }}
              type={showCookie ? 'text' : 'password'}
              value={editing.cookie}
              onChange={(e) => updateProfile({ cookie: e.target.value })}
              placeholder={editing.providerType === 'jizhi' ? '极智无需 Cookie，可留空' : 'session=xxx; token=yyy'}
              autoComplete="off"
              spellCheck={false}
            />
            <button
              onClick={() => setShowCookie(!showCookie)}
              style={{
                position: 'absolute', right: 6, top: '50%', transform: 'translateY(-50%)',
                background: 'none', border: 'none', color: '#5a5a6a', cursor: 'pointer',
                padding: 0, display: 'flex',
              }}
            >
              {showCookie ? <EyeOff size={12} /> : <Eye size={12} />}
            </button>
          </div>
          <div style={{ fontSize: 10, color: '#5a5a6a', marginTop: 4 }}>
            {editing.providerType === 'jizhi'
              ? '极智 API 用下方 API Token 认证，此项可留空'
              : <>请填写完整 Cookie，例如 <code>session=你的值</code>，不要只粘贴 value</>}
          </div>
        </Row>

        {/* API Token */}
        <Row label="API Token">
          <div style={{ position: 'relative' }}>
            <input
              style={{ ...inputStyle, paddingRight: 30, fontFamily: 'monospace' }}
              type={showToken ? 'text' : 'password'}
              value={editing.apiToken}
              onChange={(e) => updateProfile({ apiToken: e.target.value })}
              placeholder={editing.providerType === 'jizhi' ? '粘贴 Authorization 里的 Bearer JWT（必填）' : 'Bearer Token（可选）'}
              autoComplete="off"
              spellCheck={false}
            />
            <button
              onClick={() => setShowToken(!showToken)}
              style={{
                position: 'absolute', right: 6, top: '50%', transform: 'translateY(-50%)',
                background: 'none', border: 'none', color: '#5a5a6a', cursor: 'pointer',
                padding: 0, display: 'flex',
              }}
            >
              {showToken ? <EyeOff size={12} /> : <Eye size={12} />}
            </button>
          </div>
          {editing.providerType === 'jizhi' && (
            <div style={{ fontSize: 10, color: '#5a5a6a', marginTop: 4 }}>
              F12 → 网络 → 打开 <code>/api/v1/auth/me</code> 请求 → 请求标头里 <code>Authorization: Bearer</code> 后面那一长串
            </div>
          )}
        </Row>

        {/* New-Api-User（极智不需要） */}
        {editing.providerType !== 'jizhi' && (
          <Row label="New-Api-User">
            <input
              style={{ ...inputStyle, fontFamily: 'monospace' }}
              type="text"
              value={editing.newApiUser}
              onChange={(e) => updateProfile({ newApiUser: e.target.value })}
              placeholder="从浏览器 Network 请求头中复制 New-Api-User"
              autoComplete="off"
              spellCheck={false}
            />
            <div style={{ fontSize: 10, color: '#5a5a6a', marginTop: 4 }}>
              部分中转站（New API / One API 系）接口需要这个请求头，否则返回"未提供 New-Api-User"
            </div>
          </Row>
        )}

        {/* 操作按钮 */}
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 10 }}>
          <button
            onClick={handleOpenWeb}
            style={{
              ...btnBase,
              background: 'rgba(124,111,247,0.15)',
              border: '1px solid rgba(124,111,247,0.35)',
              color: '#a89ff9',
            }}
          >
            <ExternalLink size={12} />
            打开中转站官网
          </button>

          <button
            onClick={handleTest}
            disabled={testing}
            style={{
              ...btnBase,
              background: 'rgba(34,197,94,0.12)',
              border: '1px solid rgba(34,197,94,0.3)',
              color: '#4ade80',
            }}
          >
            {testing
              ? <RefreshCw size={12} style={{ animation: 'spin 1s linear infinite' }} />
              : <Wifi size={12} />
            }
            {testing ? '正在测试连接…' : '测试连接'}
          </button>

          {(editing.cookie || editing.apiToken) && (
            <button
              onClick={handleClearCookie}
              style={{
                ...btnBase,
                background: 'rgba(239,68,68,0.1)',
                border: '1px solid rgba(239,68,68,0.25)',
                color: '#f87171',
              }}
            >
              <X size={12} />
              清除认证
            </button>
          )}
        </div>

        {/* 测试结果 */}
        {testResult && (
          <div style={{
            padding: '8px 10px', borderRadius: 6, marginBottom: 10, fontSize: 11,
            background: testResult.ok ? 'rgba(34,197,94,0.08)' : 'rgba(239,68,68,0.08)',
            border: `1px solid ${testResult.ok ? 'rgba(34,197,94,0.25)' : 'rgba(239,68,68,0.25)'}`,
            color: testResult.ok ? '#4ade80' : '#f87171',
          }}>
            {testResult.ok ? (
              <div>
                <div style={{ fontWeight: 600, marginBottom: 4, display: 'flex', alignItems: 'center', gap: 4 }}>
                  <CheckCircle size={12} /> 连接成功
                </div>
                <div style={{ color: '#8a8a9a', lineHeight: 1.9 }}>
                  余额 ${testResult.balance?.toFixed(4)}{' '}
                  &nbsp;·&nbsp; 历史消耗 ${testResult.totalCost?.toFixed(4)}{' '}
                  &nbsp;·&nbsp; 请求次数 {testResult.requestCount?.toLocaleString()}
                  {testResult.logCount !== undefined && (
                    <> &nbsp;·&nbsp; 最近日志 {testResult.logCount} 条</>
                  )}
                </div>
                {testResult.debugUrl && (
                  <div style={{ color: '#5a5a6a', fontSize: 10, marginTop: 4 }}>
                    URL: {testResult.debugUrl} · HTTP {testResult.debugHttpStatus}
                  </div>
                )}
              </div>
            ) : (
              <div>
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: 5, marginBottom: 4 }}>
                  <XCircle size={12} style={{ flexShrink: 0, marginTop: 1 }} />
                  <span>
                    {testResult.errorType && (
                      <span style={{
                        fontFamily: 'monospace', fontSize: 10,
                        background: 'rgba(239,68,68,0.15)', padding: '1px 5px',
                        borderRadius: 3, marginRight: 6,
                      }}>
                        {testResult.errorType}
                      </span>
                    )}
                    {testResult.error}
                  </span>
                </div>
                {/* 调试摘要 */}
                {(testResult.debugUrl || testResult.debugRespKeys || testResult.debugMessage) && (
                  <div style={{
                    marginTop: 6, padding: '6px 8px',
                    background: 'rgba(0,0,0,0.2)', borderRadius: 4,
                    fontSize: 10, color: '#8a8a9a', lineHeight: 1.8,
                    fontFamily: 'monospace',
                  }}>
                    {testResult.debugUrl && (
                      <div>URL: {testResult.debugUrl}</div>
                    )}
                    {testResult.debugHttpStatus !== undefined && (
                      <div>HTTP: {testResult.debugHttpStatus}</div>
                    )}
                    {testResult.debugRespKeys && (
                      <div>响应字段: {testResult.debugRespKeys}</div>
                    )}
                    {testResult.debugMessage && (
                      <div>message: {testResult.debugMessage}</div>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
      )}

      {/* ── Cookie 获取引导（仅 Cookie 认证类中转站显示）── */}
      {editing && editing.providerType === 'xiaoma' && (
      <>
      <div style={sec()}>
        <SectionTitle title="如何获取 Cookie" />
        <div style={{
          fontSize: 11, color: '#8a8a9a', lineHeight: 2,
          background: 'rgba(255,255,255,0.03)',
          border: '1px solid rgba(255,255,255,0.06)',
          borderRadius: 6, padding: '10px 12px',
        }}>
          <ol style={{ margin: 0, paddingLeft: 18 }}>
            <li>在浏览器登录上方设置的「中转站地址」</li>
            <li>按 F12 打开开发者工具</li>
            <li>进入 Application / 应用 → Cookies → 选中当前站点</li>
            <li>找到 <code style={{ color: '#f0c040' }}>session</code> 这一行，复制其值</li>
            <li>在上方 Cookie 框粘贴，格式：<code style={{ color: '#f0c040' }}>session=你的值</code></li>
            <li>多个 Cookie 用英文分号连接：<code style={{ color: '#f0c040' }}>session=xxx; token=yyy</code></li>
            <li>点击保存，再点击测试连接</li>
          </ol>
        </div>
        <div style={{
          marginTop: 8, fontSize: 10, color: '#5a5a6a',
          padding: '5px 8px',
          background: 'rgba(239,68,68,0.05)',
          border: '1px solid rgba(239,68,68,0.12)',
          borderRadius: 5,
        }}>
          Cookie 等同于登录凭证，只保存在本机，不要发给别人。
        </div>
      </div>

      {/* ── New-Api-User 获取引导 ── */}
      <div style={sec()}>
        <SectionTitle title="如何获取 New-Api-User（可选）" />
        <div style={{
          fontSize: 11, color: '#8a8a9a', lineHeight: 2,
          background: 'rgba(255,255,255,0.03)',
          border: '1px solid rgba(255,255,255,0.06)',
          borderRadius: 6, padding: '10px 12px',
        }}>
          <ol style={{ margin: 0, paddingLeft: 18 }}>
            <li>浏览器打开并登录上方设置的「中转站地址」</li>
            <li>按 F12 打开开发者工具</li>
            <li>切换到 <code style={{ color: '#f0c040' }}>Network</code> / 网络 标签</li>
            <li>刷新当前网页（按 F5）</li>
            <li>在请求列表顶部的搜索框里输入 <code style={{ color: '#f0c040' }}>self</code></li>
            <li>点击 <code style={{ color: '#f0c040' }}>/api/user/self</code> 这个请求</li>
            <li>打开 <code style={{ color: '#f0c040' }}>Headers</code> / 标头 面板</li>
            <li>向下滚动到 <code style={{ color: '#f0c040' }}>Request Headers</code> / 请求标头 区域</li>
            <li>找到 <code style={{ color: '#f0c040' }}>New-Api-User</code>，复制它右侧的值（通常是数字 ID）</li>
            <li>粘贴到上方的 New-Api-User 输入框，点击保存</li>
          </ol>
        </div>
        <div style={{
          marginTop: 8, fontSize: 10, color: '#5a5a6a',
          padding: '5px 8px',
          background: 'rgba(255,255,255,0.03)',
          border: '1px solid rgba(255,255,255,0.06)',
          borderRadius: 5,
        }}>
          仅 New API / One API 系站点需要。如果中转站不要求该请求头，可以留空。
        </div>
      </div>
      </>
      )}

      {/* ── 极智 API 获取引导（仅极智显示）── */}
      {editing && editing.providerType === 'jizhi' && (
      <div style={sec()}>
        <SectionTitle title="如何获取极智 API Token" />
        <div style={{
          fontSize: 11, color: '#8a8a9a', lineHeight: 2,
          background: 'rgba(255,255,255,0.03)',
          border: '1px solid rgba(255,255,255,0.06)',
          borderRadius: 6, padding: '10px 12px',
        }}>
          <ol style={{ margin: 0, paddingLeft: 18 }}>
            <li>浏览器登录 <code style={{ color: '#f0c040' }}>https://jizhiapi.site</code></li>
            <li>按 F12 打开开发者工具，切到 <code style={{ color: '#f0c040' }}>网络</code> / Network</li>
            <li>刷新页面（F5），在搜索框输入 <code style={{ color: '#f0c040' }}>me</code></li>
            <li>点击 <code style={{ color: '#f0c040' }}>/api/v1/auth/me</code> 请求 → 请求标头</li>
            <li>找到 <code style={{ color: '#f0c040' }}>Authorization</code>，复制 <code style={{ color: '#f0c040' }}>Bearer</code> 后面那一长串（JWT）</li>
            <li>粘贴到上方 API Token 框，点击保存，再测试连接</li>
          </ol>
        </div>
        <div style={{
          marginTop: 8, fontSize: 10, color: '#5a5a6a',
          padding: '5px 8px',
          background: 'rgba(239,68,68,0.05)',
          border: '1px solid rgba(239,68,68,0.12)',
          borderRadius: 5,
        }}>
          Token 等同于登录凭证，只保存在本机，不要发给别人。Token 过期后需重新复制。
        </div>
      </div>
      )}

      {/* ── 刷新 & 提醒 ── */}
      <div style={sec()}>
        <SectionTitle title="刷新 & 提醒" />
        <Row label="自动刷新间隔">
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <input
              style={{ ...inputStyle, width: 70 }}
              type="number"
              min={0}
              value={g.refreshInterval}
              onChange={(e) => updateGlobal({ refreshInterval: Number(e.target.value) })}
            />
            <span style={{ color: '#5a5a6a' }}>秒（0 = 不自动刷新）</span>
          </div>
        </Row>
        <Row label="低余额提醒">
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ color: '#5a5a6a' }}>$</span>
            <input
              style={{ ...inputStyle, width: 70 }}
              type="number"
              min={0}
              step={0.5}
              value={g.lowBalanceThreshold}
              onChange={(e) => updateGlobal({ lowBalanceThreshold: Number(e.target.value) })}
            />
          </div>
        </Row>
        <Row label="系统通知">
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={g.enableNotification}
              onChange={(e) => updateGlobal({ enableNotification: e.target.checked })}
            />
            <span>余额不足时弹系统通知</span>
          </label>
        </Row>
      </div>

      {/* ── 窗口 ── */}
      <div style={sec()}>
        <SectionTitle title="窗口" />
        <Row label="窗口置顶">
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={g.alwaysOnTop}
              onChange={(e) => updateGlobal({ alwaysOnTop: e.target.checked })}
            />
            <span>始终显示在最前</span>
          </label>
        </Row>
        <Row label="开机启动">
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={g.autoLaunch}
              onChange={(e) => updateGlobal({ autoLaunch: e.target.checked })}
            />
            <span>Windows 开机自动启动</span>
          </label>
          {autostartError && (
            <div style={{ fontSize: 10, color: '#f87171', marginTop: 4 }}>{autostartError}</div>
          )}
        </Row>
      </div>

      {/* ── 调试 ── */}
      <div style={sec()}>
        <SectionTitle title="调试" />
        <Row label="调试模式">
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={g.debugMode ?? false}
              onChange={(e) => updateGlobal({ debugMode: e.target.checked })}
            />
            <span>刷新时在控制台打印接口字段名和状态码</span>
          </label>
        </Row>
      </div>

      {/* ── 数据 ── */}
      <div style={sec()}>
        <SectionTitle title="数据" />
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <button
            onClick={handleClearHistory}
            disabled={clearing}
            style={{
              ...btnBase,
              background: 'rgba(239,68,68,0.12)',
              border: '1px solid rgba(239,68,68,0.25)',
              color: '#ef4444',
            }}
          >
            {clearing ? <RefreshCw size={12} style={{ animation: 'spin 1s linear infinite' }} /> : <Trash2 size={12} />}
            清除此中转站历史
          </button>
          {g.profiles.length > 1 && (
            <button
              onClick={handleDeleteProfile}
              style={{
                ...btnBase,
                background: 'rgba(239,68,68,0.12)',
                border: '1px solid rgba(239,68,68,0.25)',
                color: '#ef4444',
              }}
            >
              <Trash2 size={12} />
              删除此中转站配置
            </button>
          )}
        </div>
      </div>

      {/* ── 底部固定操作栏 ── */}
      <div style={{
        position: 'fixed', bottom: 0, left: 0, right: 0,
        padding: '12px 24px',
        background: '#141418',
        borderTop: '1px solid rgba(255,255,255,0.07)',
        display: 'flex', gap: 8, alignItems: 'center',
        zIndex: 100,
      }}>
        <button
          onClick={handleSave}
          style={{
            ...btnBase,
            padding: '7px 20px',
            fontSize: 13,
            fontWeight: 600,
            background: saved ? 'rgba(34,197,94,0.2)' : 'rgba(124,111,247,0.2)',
            border: `1px solid ${saved ? 'rgba(34,197,94,0.4)' : 'rgba(124,111,247,0.4)'}`,
            color: saved ? '#22c55e' : '#a89ff9',
            transition: 'all 0.2s',
          }}
        >
          <Save size={13} />
          {saved ? '已保存' : '保存设置'}
        </button>

        <button
          onClick={handleTest}
          disabled={testing}
          style={{
            ...btnBase,
            padding: '7px 16px',
            fontSize: 13,
            background: 'rgba(34,197,94,0.1)',
            border: '1px solid rgba(34,197,94,0.25)',
            color: '#4ade80',
          }}
        >
          {testing
            ? <RefreshCw size={13} style={{ animation: 'spin 1s linear infinite' }} />
            : <Wifi size={13} />
          }
          测试连接
        </button>

        <button
          onClick={() => tauriAPI.closeSettingsWindow()}
          style={{
            ...btnBase,
            padding: '7px 16px',
            fontSize: 13,
            background: 'rgba(255,255,255,0.05)',
            border: '1px solid rgba(255,255,255,0.1)',
            color: '#8a8a9a',
          }}
        >
          关闭
        </button>
      </div>

      <style>{`
        @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
        select option { background: #1e1e28; color: #f0f0f0; }
        input[type=number]::-webkit-inner-spin-button { opacity: 0.4; }
        input:focus { border-color: rgba(124,111,247,0.5) !important; }
        select:focus { border-color: rgba(124,111,247,0.5) !important; outline: none; }
        ::-webkit-scrollbar { width: 4px; }
        ::-webkit-scrollbar-track { background: transparent; }
        ::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }
        code { font-family: monospace; font-size: 10px; }
      `}</style>
    </div>
  )
}
