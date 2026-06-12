import React, { useEffect } from 'react'
import FloatingWindow from './components/FloatingWindow'
import SettingsWindow from './components/SettingsWindow'

function getPage() {
  const params = new URLSearchParams(window.location.search)
  return params.get('page') || 'floating'
}

// ── ErrorBoundary ─────────────────────────────────────────────

interface EBState { error: Error | null }

class ErrorBoundary extends React.Component<{ children: React.ReactNode }, EBState> {
  constructor(props: { children: React.ReactNode }) {
    super(props)
    this.state = { error: null }
  }

  static getDerivedStateFromError(error: Error): EBState {
    return { error }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('[ErrorBoundary] 渲染错误:', error.message, info.componentStack)
  }

  render() {
    if (!this.state.error) return this.props.children
    const msg = this.state.error.message
    return (
      <div style={{
        background: '#101014', color: '#f87171',
        minHeight: '100vh', display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center',
        fontFamily: "-apple-system,'Segoe UI',sans-serif",
        fontSize: 13, padding: 24, textAlign: 'center', gap: 12,
      }}>
        <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 4 }}>设置页加载失败</div>
        <div style={{
          fontSize: 11, color: '#8a8a9a', maxWidth: 360,
          background: 'rgba(255,255,255,0.04)', borderRadius: 6,
          padding: '8px 12px', fontFamily: 'monospace', wordBreak: 'break-all',
        }}>
          {msg}
        </div>
        <div style={{ display: 'flex', gap: 8, marginTop: 4 }}>
          <button
            onClick={() => window.location.reload()}
            style={{
              background: 'rgba(124,111,247,0.2)', border: '1px solid rgba(124,111,247,0.4)',
              color: '#a89ff9', borderRadius: 5, padding: '5px 14px',
              fontSize: 12, cursor: 'pointer',
            }}
          >
            重新加载
          </button>
          <button
            onClick={() => window.close()}
            style={{
              background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)',
              color: '#8a8a9a', borderRadius: 5, padding: '5px 14px',
              fontSize: 12, cursor: 'pointer',
            }}
          >
            关闭
          </button>
        </div>
      </div>
    )
  }
}

// ── App ───────────────────────────────────────────────────────

export default function App() {
  const page = getPage()

  useEffect(() => {
    const root = document.getElementById('root')
    if (!root) return
    root.classList.remove('app-page-floating', 'app-page-settings')
    root.classList.add(page === 'settings' ? 'app-page-settings' : 'app-page-floating')
    if (page === 'settings') {
      console.log('[Settings] render page=settings')
    }
  }, [page])

  if (page === 'settings') {
    return (
      <ErrorBoundary>
        <SettingsWindow />
      </ErrorBoundary>
    )
  }
  return <FloatingWindow />
}
