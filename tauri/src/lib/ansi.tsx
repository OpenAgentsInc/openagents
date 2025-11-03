import React from 'react'

type SgrState = {
  bold?: boolean
  dim?: boolean
  italic?: boolean
  underline?: boolean
  fg?: string
  bg?: string
}

const ANSI_SIMPLE_COLORS: Record<number, string> = {
  0: '#000000', // black
  1: '#cc0000', // red
  2: '#00a000', // green
  3: '#b58900', // yellow (amber)
  4: '#268bd2', // blue
  5: '#d33682', // magenta
  6: '#2aa198', // cyan
  7: '#eaeaea', // white (light gray)
}

const ANSI_BRIGHT_COLORS: Record<number, string> = {
  0: '#555555',
  1: '#ff3b30',
  2: '#34c759',
  3: '#ffd60a',
  4: '#0a84ff',
  5: '#bf5af2',
  6: '#64d2ff',
  7: '#ffffff',
}

function color256(n: number): string {
  if (n < 0) return ''
  if (n < 16) {
    const idx = n & 7
    const bright = (n & 8) !== 0
    return (bright ? ANSI_BRIGHT_COLORS : ANSI_SIMPLE_COLORS)[idx]
  }
  if (n >= 16 && n <= 231) {
    const c = n - 16
    const r = Math.floor(c / 36)
    const g = Math.floor((c % 36) / 6)
    const b = c % 6
    const step = [0, 95, 135, 175, 215, 255]
    return `rgb(${step[r]}, ${step[g]}, ${step[b]})`
  }
  if (n >= 232 && n <= 255) {
    const v = 8 + (n - 232) * 10
    return `rgb(${v}, ${v}, ${v})`
  }
  return ''
}

function applySgrCodes(codes: number[], st: SgrState): SgrState {
  let i = 0
  const next: SgrState = { ...st }
  while (i < codes.length) {
    const c = codes[i++]
    if (c === 0 || Number.isNaN(c)) {
      next.bold = undefined; next.italic = undefined; next.underline = undefined; next.fg = undefined; next.bg = undefined
      continue
    }
    switch (c) {
      case 1: next.bold = true; break
      case 2: next.dim = true; break
      case 3: next.italic = true; break
      case 4: next.underline = true; break
      case 22: next.bold = undefined; next.dim = undefined; break
      case 23: next.italic = undefined; break
      case 24: next.underline = undefined; break
      case 39: next.fg = undefined; break
      case 49: next.bg = undefined; break
      default: {
        // 30–37 fg, 90–97 bright fg, 40–47 bg, 100–107 bright bg
        if (c >= 30 && c <= 37) next.fg = ANSI_SIMPLE_COLORS[c - 30]
        else if (c >= 90 && c <= 97) next.fg = ANSI_BRIGHT_COLORS[c - 90]
        else if (c >= 40 && c <= 47) next.bg = ANSI_SIMPLE_COLORS[c - 40]
        else if (c >= 100 && c <= 107) next.bg = ANSI_BRIGHT_COLORS[c - 100]
        else if (c === 38 || c === 48) {
          const isFg = c === 38
          const mode = codes[i++]
          if (mode === 5) {
            const n = codes[i++]
            const col = color256(n)
            if (col) (isFg ? (next.fg = col) : (next.bg = col))
          } else if (mode === 2) {
            const r = codes[i++] ?? 0
            const g = codes[i++] ?? 0
            const b = codes[i++] ?? 0
            const col = `rgb(${r}, ${g}, ${b})`
            if (col) (isFg ? (next.fg = col) : (next.bg = col))
          }
        }
        break
      }
    }
  }
  return next
}

export function renderAnsi(input: string): React.ReactNode {
  // ESC literal 0x1B followed by '['
  const ESC = '\u001b['
  const nodes: React.ReactNode[] = []
  let idx = 0
  let st: SgrState = {}
  let key = 0
  while (idx < input.length) {
    const j = input.indexOf(ESC, idx)
    if (j === -1) {
      const text = input.slice(idx)
      if (text) nodes.push(renderSpan(text, st, key++))
      break
    }
    // push preceding
    if (j > idx) {
      const text = input.slice(idx, j)
      if (text) nodes.push(renderSpan(text, st, key++))
    }
    // parse SGR until 'm'
    const end = input.indexOf('m', j + ESC.length)
    if (end === -1) {
      // bail out
      const rest = input.slice(j)
      nodes.push(renderSpan(rest, st, key++))
      break
    }
    const seq = input.slice(j + ESC.length, end)
    const parts = seq.split(';').map((s) => (s.trim() === '' ? 0 : Number(s)))
    st = applySgrCodes(parts, st)
    idx = end + 1
  }
  return <>{nodes}</>
}

function renderSpan(text: string, st: SgrState, key: number) {
  const style: React.CSSProperties = {}
  if (st.fg) style.color = st.fg
  if (st.bg) style.backgroundColor = st.bg
  if (st.bold) style.fontWeight = 700 as React.CSSProperties['fontWeight']
  if (st.dim) style.opacity = 0.7
  if (st.italic) style.fontStyle = 'italic'
  if (st.underline) style.textDecoration = 'underline'
  return <span key={key} style={style}>{text}</span>
}
