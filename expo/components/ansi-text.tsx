import React from 'react'
import { Text, type TextStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

type Segment = { text: string; style: TextStyle }

// Minimal ANSI SGR parser for common codes we see in bridge logs.
// Supports: reset (0), bold (1), dim (2), italic (3), underline (4),
// colors: 30-37, 90-97, and their resets.
export function parseAnsi(text: string): Segment[] {
  const segments: Segment[] = []
  let i = 0
  let cur: TextStyle = { fontFamily: Typography.primary, color: Colors.secondary }
  let buf = ''
  const push = () => {
    if (buf.length > 0) { segments.push({ text: buf, style: { ...cur } }); buf = '' }
  }
  const setColor = (code: number) => {
    const map: Record<number, string> = {
      30: '#000000', 31: '#e53935', 32: '#43a047', 33: '#fdd835', 34: '#1e88e5', 35: '#8e24aa', 36: '#00acc1', 37: Colors.foreground,
      90: '#757575', 91: '#ef5350', 92: '#66bb6a', 93: '#ffee58', 94: '#42a5f5', 95: '#ab47bc', 96: '#26c6da', 97: '#ffffff',
    }
    const c = map[code]
    if (c) cur = { ...cur, color: c }
  }
  while (i < text.length) {
    const ch = text.charCodeAt(i)
    if (ch === 0x1b /* ESC */ && text[i + 1] === '[') {
      push()
      i += 2
      // read until 'm'
      let codes = ''
      while (i < text.length && text[i] !== 'm') { codes += text[i++]; }
      if (text[i] === 'm') i++
      const parts = codes.split(';').map((s) => parseInt(s || '0', 10))
      for (const code of parts) {
        if (isNaN(code)) continue
        if (code === 0) {
          cur = { fontFamily: Typography.primary, color: Colors.secondary, fontWeight: '400', fontStyle: 'normal', textDecorationLine: 'none', opacity: 1 }
        } else if (code === 1) {
          cur = { ...cur, fontWeight: '700' }
        } else if (code === 2) {
          cur = { ...cur, opacity: 0.7 }
        } else if (code === 3) {
          cur = { ...cur, fontStyle: 'italic' }
        } else if (code === 4) {
          cur = { ...cur, textDecorationLine: 'underline' }
        } else if ((code >= 30 && code <= 37) || (code >= 90 && code <= 97)) {
          setColor(code)
        } else if (code === 39) {
          cur = { ...cur, color: Colors.secondary }
        }
      }
      continue
    }
    buf += text[i]
    i++
  }
  push()
  return segments
}

export function AnsiText({ line }: { line: string }) {
  const segs = React.useMemo(() => parseAnsi(line), [line])
  return (
    <Text
      style={{
        fontFamily: Typography.primary,
        fontSize: 12,
        color: Colors.secondary,
        // Ensure long tokens don't blow out layout on web
        // web-only styles for React Native Web
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        whiteSpace: 'pre-wrap',
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        wordBreak: 'break-word',
      }}
    >
      {segs.map((s, idx) => (
        <Text key={idx} style={s.style}>{s.text}</Text>
      ))}
    </Text>
  )
}
