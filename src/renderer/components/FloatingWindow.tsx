import React, { useCallback, useEffect, useRef, useState } from 'react'
import {
  RefreshCw, EyeOff, Settings, ExternalLink, GripHorizontal,
  AlertTriangle,
} from 'lucide-react'
import TrendChart from './TrendChart'
import {
  BalanceRecord, BalanceSnapshot, UsageLogItem,
  AppSettings, DEFAULT_SETTINGS,
} from '../types'
import { balanceService } from '../services/balanceService'
import { historyStore } from '../services/historyStore'
import { checkLowBalance } from '../services/alertService'
import { tauriAPI } from '../services/tauriAPI'

// ── 状态角标 ──────────────────────────────────────────────────

type StatusTag = 'MOCK' | 'LIVE' | 'LOGIN' | 'AUTH' | 'ERROR' | 'NET' | 'COOKIE' | 'BALANCE'

function getStatusTag(
  providerType: string,
  error: string | null,
  status: BalanceSnapshot['status'] | null,
): StatusTag {
  if (providerType === 'mock') return 'MOCK'
  if (!error) return 'LIVE'
  if (status === 'cookie_missing') return 'COOKIE'
  if (status === 'new_api_user_missing') return 'AUTH'
  if (status === 'balance_insufficient') return 'BALANCE'
  if (status === 'auth_error' || error.includes('AUTH_ERROR')) return 'AUTH'
  if (status === 'network_error' || error.includes('NETWORK_ERROR')) return 'NET'
  return 'ERROR'
}

const TAG_STYLE: Record<StatusTag, { bg: string; text: string; border: string }> = {
  MOCK:    { bg: 'rgba(124,111,247,0.18)', text: '#a89ff9', border: 'rgba(124,111,247,0.4)' },
  LIVE:    { bg: 'rgba(34,197,94,0.15)',   text: '#4ade80', border: 'rgba(34,197,94,0.35)' },
  LOGIN:   { bg: 'rgba(245,158,66,0.18)',  text: '#f59e42', border: 'rgba(245,158,66,0.4)' },
  COOKIE:  { bg: 'rgba(245,158,66,0.18)',  text: '#f59e42', border: 'rgba(245,158,66,0.4)' },
  AUTH:    { bg: 'rgba(245,158,66,0.18)',  text: '#f59e42', border: 'rgba(245,158,66,0.4)' },
  BALANCE: { bg: 'rgba(245,158,66,0.18)',  text: '#f59e42', border: 'rgba(245,158,66,0.4)' },
  ERROR:   { bg: 'rgba(239,68,68,0.15)',   text: '#f87171', border: 'rgba(239,68,68,0.35)' },
  NET:     { bg: 'rgba(239,68,68,0.15)',   text: '#f87171', border: 'rgba(239,68,68,0.35)' },
}

// ── 工具函数 ──────────────────────────────────────────────────

function fmtUSD(v: number | null | undefined, dec = 2): string {
  if (v === null || v === undefined) return '--'
  return `$${v.toFixed(dec)}`
}

function fmtInt(v: number | null | undefined): string {
  if (v === null || v === undefined) return '--'
  return v.toLocaleString()
}

function shortModel(m: string): string {
  return m
    .replace('claude-', '')
    .replace('-20251001', '')
    .replace('-20240620', '')
    .replace('-20240229', '')
}

// ── 子组件 ────────────────────────────────────────────────────

function IconBtn({
  onClick, title, children,
}: {
  onClick: () => void
  title: string
  children: React.ReactNode
}) {
  return (
    <div className="tooltip-container">
      <button className="icon-btn" onClick={onClick} aria-label={title}>
        {children}
      </button>
      <span className="tooltip">{title}</span>
    </div>
  )
}

function StatCell({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ flex: 1, minWidth: 0, textAlign: 'center' }}>
      <div style={{
        fontSize: 11, fontWeight: 700, color: 'var(--text-primary)',
        whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
        lineHeight: 1.2,
      }}>
        {value}
      </div>
      <div style={{ fontSize: 8, color: 'var(--text-dim)', marginTop: 1 }}>{label}</div>
    </div>
  )
}

function LogRow({ log }: { log: UsageLogItem }) {
  const time = log.time.length > 10 ? log.time.slice(11) : log.time
  return (
    <div style={{
      padding: '3px 10px',
      borderTop: '1px solid rgba(255,255,255,0.04)',
      fontSize: 9,
      lineHeight: 1.5,
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', gap: 4 }}>
        <span style={{ color: 'var(--text-dim)', flexShrink: 0 }}>{time}</span>
        <span style={{
          color: 'var(--text-secondary)',
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          maxWidth: 50, flexShrink: 1,
        }}>{log.tokenName}</span>
        <span style={{
          color: 'var(--text-dim)',
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          flex: 1, minWidth: 0,
        }}>{shortModel(log.model)}</span>
        <span style={{ color: '#4ade80', fontWeight: 600, flexShrink: 0 }}>
          ${log.cost.toFixed(4)}
        </span>
      </div>
      <div style={{ color: 'var(--text-dim)', fontSize: 8 }}>
        in {log.inputTokens.toLocaleString()} / out {log.outputTokens}
      </div>
    </div>
  )
}

// ── 主组件 ────────────────────────────────────────────────────

export default function FloatingWindow() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS)
  const settingsRef = useRef<AppSettings>(DEFAULT_SETTINGS)
  const [snapshot, setSnapshot] = useState<BalanceSnapshot | null>(null)
  const [record, setRecord] = useState<BalanceRecord | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [errorStatus, setErrorStatus] = useState<BalanceSnapshot['status'] | null>(null)
  const dragRef = useRef<{ sx: number; sy: number; wx: number; wy: number } | null>(null)

  // 统一入口：重新从 Rust 读最新 settings，然后刷新数据
  // 所有刷新路径（启动、按钮、事件、托盘）都走这里，避免闭包持有旧 settings
  const refreshWithLatestSettings = useCallback(async () => {
    setLoading(true)
    setError(null)
    setErrorStatus(null)
    const latest = { ...DEFAULT_SETTINGS, ...(await tauriAPI.getSettings()) }
    settingsRef.current = latest
    setSettings(latest)
    balanceService.setSettings(latest)
    balanceService.stopAutoRefresh()
    balanceService.startAutoRefresh(latest.refreshInterval)
    await balanceService.refresh()
  }, [])

  useEffect(() => {
    let unsubRefresh: (() => void) | undefined
    let unsubSettings: (() => void) | undefined

    ;(async () => {
      // 启动时读一次历史，避免空白闪烁
      const h = await tauriAPI.getHistory()
      if (h.length > 0) {
        const last = h[h.length - 1]
        setRecord(last)
        if (last.snapshot) setSnapshot(last.snapshot)
      }

      await refreshWithLatestSettings()

      unsubRefresh = await tauriAPI.onRefresh(() => refreshWithLatestSettings())
      unsubSettings = await tauriAPI.onSettingsChanged(() => refreshWithLatestSettings())
    })()

    const unsubUpdate = balanceService.onUpdate((rec) => {
      setRecord(rec)
      if (rec.snapshot) setSnapshot(rec.snapshot)
      setError(null)
      setErrorStatus(null)
      setLoading(false)
    })
    const unsubErr = balanceService.onError((err) => {
      setError(err.message)
      const attached = (err as Error & { snapshot?: BalanceSnapshot }).snapshot
      if (attached) {
        setErrorStatus(attached.status)
      } else if (err.message.includes('COOKIE_MISSING')) {
        setErrorStatus('cookie_missing')
      } else if (err.message.includes('NEW_API_USER_MISSING')) {
        setErrorStatus('new_api_user_missing')
      } else if (err.message.includes('BALANCE_INSUFFICIENT')) {
        setErrorStatus('balance_insufficient')
      } else if (err.message.includes('AUTH_ERROR')) {
        setErrorStatus('auth_error')
      } else if (err.message.includes('NETWORK_ERROR')) {
        setErrorStatus('network_error')
      } else {
        setErrorStatus('parse_error')
      }
      setLoading(false)
    })

    return () => {
      unsubUpdate()
      unsubErr()
      unsubRefresh?.()
      unsubSettings?.()
      balanceService.stopAutoRefresh()
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    if (record) checkLowBalance(record, settingsRef.current)
  }, [record])

  const handleRefresh = useCallback(() => refreshWithLatestSettings(), [refreshWithLatestSettings])

  const handleDragStart = useCallback((e: React.MouseEvent) => {
    dragRef.current = {
      sx: e.screenX, sy: e.screenY,
      wx: window.screenX, wy: window.screenY,
    }
    const onMove = (ev: MouseEvent) => {
      if (!dragRef.current) return
      tauriAPI.moveWindow(
        dragRef.current.wx + ev.screenX - dragRef.current.sx,
        dragRef.current.wy + ev.screenY - dragRef.current.sy,
      )
    }
    const onUp = () => {
      dragRef.current = null
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }, [])

  // 优先用 snapshot，无 snapshot 时降级用 record 旧字段
  const wallet = snapshot?.wallet
  const balance = wallet?.balance ?? record?.balance ?? null
  const totalCost = wallet?.totalCost ?? record?.totalCost ?? null
  const requestCount = wallet?.requestCount ?? record?.requestCount ?? null
  const recentLogs = snapshot?.recentLogs ?? []

  // 折线图：按请求时间从旧到新排列（左旧右新），x 轴代表第几次请求
  const chartLogs = [...recentLogs]
    .filter((log) => typeof log.cost === 'number')
    .sort((a, b) => {
      const at = a.timestamp ?? 0
      const bt = b.timestamp ?? 0
      return at - bt
    })

  // 底部最新 3 条：最新在上
  const latestLogs = [...recentLogs]
    .sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0))
    .slice(0, 3)

  // 余额成功但日志失败时，logError 不影响顶部状态栏
  const logError = snapshot?.logError ?? null

  const isLow = balance !== null && settingsRef.current.lowBalanceThreshold > 0
    && balance < settingsRef.current.lowBalanceThreshold

  // 顶部状态角标：用 ref 避免 setState 异步导致仍显示旧 providerType
  const tag = getStatusTag(settingsRef.current.providerType, error, errorStatus)
  const tagStyle = TAG_STYLE[tag]

  // 顶部错误文案（只针对余额接口失败）
  function errorLabel(): string {
    if (!error) return ''
    if (errorStatus === 'cookie_missing') return '未配置 Cookie，请在设置中填写'
    if (errorStatus === 'new_api_user_missing') return '缺少 New-Api-User，请在设置中填写'
    if (errorStatus === 'balance_insufficient') return '余额/额度不足，请充值'
    if (errorStatus === 'auth_error') return '认证失败，请检查 Cookie 是否有效'
    if (errorStatus === 'network_error') return '网络连接失败'
    return '数据解析失败'
  }

  return (
    <div style={{
      width: 240,
      background: isLow ? 'rgba(40,18,18,0.95)' : 'rgba(18,18,24,0.95)',
      borderRadius: 10,
      border: `1px solid ${isLow ? 'rgba(239,68,68,0.35)' : 'rgba(255,255,255,0.07)'}`,
      backdropFilter: 'blur(12px)',
      WebkitBackdropFilter: 'blur(12px)',
      display: 'flex',
      flexDirection: 'column',
      overflow: 'hidden',
      userSelect: 'none',
      boxShadow: '0 8px 32px rgba(0,0,0,0.5)',
    }}>

      {/* ── Header ── */}
      <div
        onMouseDown={handleDragStart}
        style={{
          display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          padding: '5px 7px 4px', cursor: 'grab',
          borderBottom: '1px solid rgba(255,255,255,0.05)', flexShrink: 0,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 4, minWidth: 0 }}>
          <GripHorizontal size={11} color="var(--text-dim)" style={{ flexShrink: 0 }} />
          <span style={{ fontSize: 10, fontWeight: 600, color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>
            API Monitor
          </span>
          <span style={{
            fontSize: 9, borderRadius: 3, padding: '0 4px', lineHeight: '14px',
            background: tagStyle.bg, color: tagStyle.text,
            border: `1px solid ${tagStyle.border}`, flexShrink: 0,
          }}>
            {tag}
          </span>
        </div>
        <div style={{ display: 'flex', gap: 2, flexShrink: 0 }}>
          <IconBtn onClick={handleRefresh} title="刷新">
            <RefreshCw size={12} style={loading ? { animation: 'spin 1s linear infinite' } : undefined} />
          </IconBtn>
          <IconBtn onClick={() => tauriAPI.openSettingsWindow()} title="设置">
            <Settings size={12} />
          </IconBtn>
          <IconBtn
            onClick={() => {
              const url = (settingsRef.current.baseUrl || '').trim()
              if (url) tauriAPI.openUrl(url)
            }}
            title="打开中转站官网"
          >
            <ExternalLink size={12} />
          </IconBtn>
          <IconBtn onClick={() => tauriAPI.hideWindow()} title="隐藏">
            <EyeOff size={12} />
          </IconBtn>
        </div>
      </div>

      {/* ── 核心数字区 ── */}
      <div style={{
        display: 'flex', padding: '6px 8px 5px',
        borderBottom: '1px solid rgba(255,255,255,0.04)', flexShrink: 0,
      }}>
        <div style={{ flex: 1.2, minWidth: 0, textAlign: 'center' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 2 }}>
            {isLow && <AlertTriangle size={9} color="var(--warn)" style={{ flexShrink: 0 }} />}
            <span style={{
              fontSize: 14, fontWeight: 700, letterSpacing: '-0.5px', lineHeight: 1.2,
              color: isLow ? 'var(--warn)' : 'var(--text-primary)',
            }}>
              {balance !== null ? fmtUSD(balance) : '--'}
            </span>
          </div>
          <div style={{ fontSize: 8, color: 'var(--text-dim)', marginTop: 1 }}>余额</div>
        </div>
        <div style={{ width: 1, background: 'rgba(255,255,255,0.05)', flexShrink: 0, margin: '2px 0' }} />
        <StatCell label="历史消耗" value={totalCost !== null ? fmtUSD(totalCost) : '--'} />
        <div style={{ width: 1, background: 'rgba(255,255,255,0.05)', flexShrink: 0, margin: '2px 0' }} />
        <StatCell label="请求次数" value={fmtInt(requestCount)} />
      </div>

      {/* ── 错误提示 ── */}
      {error && (
        <div style={{
          margin: '3px 8px 0', padding: '2px 6px',
          background: 'rgba(239,68,68,0.1)', border: '1px solid rgba(239,68,68,0.2)',
          borderRadius: 4, fontSize: 9, color: '#f87171', lineHeight: 1.4, flexShrink: 0,
        }}>
          ⚠ {errorLabel()}
        </div>
      )}

      {/* ── 消费曲线图 ── */}
      <div style={{ padding: '4px 8px 2px', flexShrink: 0 }}>
        <div style={{ fontSize: 8, color: 'var(--text-dim)', marginBottom: 2 }}>消费曲线</div>
        {chartLogs.length >= 2
          ? <TrendChart logs={chartLogs} width={224} height={30} />
          : <div style={{
              height: 30, display: 'flex', alignItems: 'center',
              fontSize: 9, color: 'var(--text-dim)',
            }}>
              {error ? '' : '等待数据…'}
            </div>
        }
      </div>

      {/* ── 最新 3 条日志 ── */}
      <div style={{ flexShrink: 0 }}>
        {latestLogs.map((log, i) => (
          <LogRow key={i} log={log} />
        ))}
        {recentLogs.length === 0 && logError && (
          <div style={{
            padding: '4px 10px 5px',
            fontSize: 9, color: '#f87171',
            borderTop: '1px solid rgba(255,255,255,0.04)',
          }}>
            ⚠ 日志读取失败
          </div>
        )}
        {recentLogs.length === 0 && !logError && !error && (
          <div style={{
            padding: '4px 10px 5px',
            fontSize: 9, color: 'var(--text-dim)',
            borderTop: '1px solid rgba(255,255,255,0.04)',
          }}>
            暂无日志
          </div>
        )}
      </div>

      <style>{`
        @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
      `}</style>
    </div>
  )
}
