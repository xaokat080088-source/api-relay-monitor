import React, { useCallback, useEffect, useRef } from 'react'
import { UsageLogItem } from '../types'

interface Props {
  logs: UsageLogItem[]
  width?: number
  height?: number
}

// 接收已按时间从旧到新排好序的 logs，不在内部做排序
export default function TrendChart({ logs, width = 210, height = 32 }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const tooltipRef = useRef<HTMLDivElement>(null)

  const points = logs.slice(-20)

  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    const dpr = window.devicePixelRatio || 1
    canvas.width = width * dpr
    canvas.height = height * dpr
    ctx.scale(dpr, dpr)
    ctx.clearRect(0, 0, width, height)

    if (points.length < 2) {
      ctx.fillStyle = 'rgba(124,111,247,0.25)'
      ctx.fillRect(0, height / 2 - 1, width, 2)
      return
    }

    const costs = points.map((p) => p.cost)
    const min = Math.min(...costs)
    const max = Math.max(...costs)
    const range = max - min || 0.0001
    const pad = { t: 4, b: 4, l: 2, r: 2 }
    const uw = (width - pad.l - pad.r) / (points.length - 1)
    const uh = height - pad.t - pad.b

    // x 按 index 均匀分布，y 按 cost 在 min/max 间映射
    const px = (i: number) => pad.l + i * uw
    const py = (v: number) => pad.t + uh - ((v - min) / range) * uh

    // 渐变填充
    const grad = ctx.createLinearGradient(0, pad.t, 0, height - pad.b)
    grad.addColorStop(0, 'rgba(124,111,247,0.32)')
    grad.addColorStop(1, 'rgba(124,111,247,0.02)')
    ctx.beginPath()
    ctx.moveTo(px(0), py(costs[0]))
    for (let i = 1; i < points.length; i++) {
      const cx = (px(i - 1) + px(i)) / 2
      ctx.bezierCurveTo(cx, py(costs[i - 1]), cx, py(costs[i]), px(i), py(costs[i]))
    }
    ctx.lineTo(px(points.length - 1), height - pad.b)
    ctx.lineTo(px(0), height - pad.b)
    ctx.closePath()
    ctx.fillStyle = grad
    ctx.fill()

    // 曲线
    ctx.beginPath()
    ctx.moveTo(px(0), py(costs[0]))
    for (let i = 1; i < points.length; i++) {
      const cx = (px(i - 1) + px(i)) / 2
      ctx.bezierCurveTo(cx, py(costs[i - 1]), cx, py(costs[i]), px(i), py(costs[i]))
    }
    ctx.strokeStyle = '#7c6ff7'
    ctx.lineWidth = 1.5
    ctx.stroke()

    // 最新（最右）点高亮
    const lx = px(points.length - 1)
    const ly = py(costs[points.length - 1])
    ctx.beginPath()
    ctx.arc(lx, ly, 2.5, 0, Math.PI * 2)
    ctx.fillStyle = '#a89ff9'
    ctx.fill()
  }, [points, width, height])

  useEffect(() => { draw() }, [draw])

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current
    const tip = tooltipRef.current
    if (!canvas || !tip || points.length < 2) return
    const rect = canvas.getBoundingClientRect()
    const mx = e.clientX - rect.left
    const pad = { l: 2, r: 2 }
    const uw = (width - pad.l - pad.r) / (points.length - 1)
    const idx = Math.min(Math.max(Math.round((mx - pad.l) / uw), 0), points.length - 1)
    const p = points[idx]
    const shortModel = p.model.replace('claude-', '').replace(/-\d{8}$/, '')
    const timeStr = p.time.length > 10 ? p.time.slice(11) : p.time
    tip.style.opacity = '1'
    tip.style.left = `${Math.min(mx + 6, width - 120)}px`
    tip.style.top = `-54px`
    tip.innerHTML = [
      `<b>第 ${idx + 1} 次请求  $${p.cost.toFixed(6)}</b>`,
      `<span style="color:#8a8a9a">${shortModel}</span>`,
      `<span style="color:#5a5a6a">in ${p.inputTokens.toLocaleString()} / out ${p.outputTokens}</span>`,
      `<span style="color:#5a5a6a">${timeStr}</span>`,
    ].join('<br>')
  }, [points, width])

  const handleMouseLeave = useCallback(() => {
    if (tooltipRef.current) tooltipRef.current.style.opacity = '0'
  }, [])

  return (
    <div style={{ position: 'relative', width, height }}>
      <canvas
        ref={canvasRef}
        style={{ width, height, display: 'block', cursor: 'crosshair' }}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
      />
      <div
        ref={tooltipRef}
        style={{
          position: 'absolute',
          background: 'rgba(0,0,0,0.85)',
          color: '#f0f0f0',
          fontSize: 10,
          padding: '4px 7px',
          borderRadius: 4,
          pointerEvents: 'none',
          opacity: 0,
          transition: 'opacity 0.1s',
          whiteSpace: 'nowrap',
          lineHeight: 1.6,
          zIndex: 10,
        }}
      />
    </div>
  )
}
