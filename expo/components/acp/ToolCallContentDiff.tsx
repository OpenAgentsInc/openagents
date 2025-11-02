import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

type DiffLine = {
  kind: 'keep' | 'add' | 'remove'
  text: string
}

export function ToolCallContentDiff({ path, oldText, newText }: { path: string; oldText?: string | null; newText: string }) {
  const oldStr = String(oldText ?? '')
  const newStr = String(newText ?? '')
  const lines = computeUnifiedDiff(oldStr, newStr)

  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Diff: {path}</Text>
      <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.black, maxHeight: 240, overflow: 'hidden' as const }}>
        <View style={{ paddingVertical: 6 }}>
          {lines.map((line, i) => {
            const isAdd = line.kind === 'add'
            const isRemove = line.kind === 'remove'
            const bg = isAdd ? 'rgba(4,165,69,0.16)' : isRemove ? 'rgba(231,4,15,0.18)' : Colors.black
            const fg = isAdd ? Colors.success : isRemove ? Colors.destructive : Colors.foreground
            const prefix = isAdd ? '+' : isRemove ? '-' : ' '
            return (
              <View key={i} style={{ backgroundColor: bg, paddingVertical: 2, paddingHorizontal: 8 }}>
                <Text style={{ color: fg, fontFamily: Typography.primary, lineHeight: 18 }}>
                  {prefix} {line.text}
                </Text>
              </View>
            )
          })}
        </View>
      </View>
    </View>
  )
}

function computeUnifiedDiff(oldStr: string, newStr: string): DiffLine[] {
  const a = splitLines(oldStr)
  const b = splitLines(newStr)
  // LCS table
  const n = a.length
  const m = b.length
  const dp: number[][] = Array.from({ length: n + 1 }, () => Array<number>(m + 1).fill(0))
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      if (a[i] === b[j]) dp[i][j] = dp[i + 1][j + 1] + 1
      else dp[i][j] = dp[i + 1][j] >= dp[i][j + 1] ? dp[i + 1][j] : dp[i][j + 1]
    }
  }
  const out: DiffLine[] = []
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      out.push({ kind: 'keep', text: a[i] })
      i++
      j++
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ kind: 'remove', text: a[i] })
      i++
    } else {
      out.push({ kind: 'add', text: b[j] })
      j++
    }
  }
  while (i < n) {
    out.push({ kind: 'remove', text: a[i++] })
  }
  while (j < m) {
    out.push({ kind: 'add', text: b[j++] })
  }
  return out
}

function splitLines(s: string): string[] {
  return s.replace(/\r\n/g, '\n').replace(/\n+$/, '').split('\n')
}
