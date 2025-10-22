import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export type ExecBeginPayload = { command: string[] | ReadonlyArray<string> | string; cwd?: string; parsed?: unknown }

function joinCmd(cmd: string[] | ReadonlyArray<string> | string): string {
  if (Array.isArray(cmd)) return (cmd as ReadonlyArray<string>).join(' ')
  return String(cmd)
}

export function ExecBeginRow({ payload }: { payload: ExecBeginPayload }) {
  const cmd = joinCmd(payload.command)
  const cwd = payload.cwd
  // Try to pretty-print parsed command, e.g., [{ "ListFiles": { path: "docs" } }]
  let pretty: { action?: string; path?: string } | null = null
  const addSlash = (p: string) => (p.endsWith('/') ? p : p + '/')
  try {
    const parsed = (payload as any)?.parsed
    if (Array.isArray(parsed) && parsed.length > 0 && parsed[0] && typeof parsed[0] === 'object') {
      const first = parsed[0] as Record<string, any>
      const keys = Object.keys(first)
      if (keys.length === 1) {
        const k = keys[0]
        const v = first[k] ?? {}
        const p = typeof v?.path === 'string' ? v.path : undefined
        pretty = { action: k, path: p }
      }
    }
  } catch {}

  if (pretty?.action) {
    return (
      <View style={{ paddingVertical: 2 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>
          <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold }}>{pretty.action}</Text>
          {pretty.path ? <Text> {addSlash(pretty.path)}</Text> : null}
        </Text>
      </View>
    )
  }

  // Fallback to raw command display
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>
        [exec] <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold }}>{cmd}</Text>
      </Text>
      {cwd ? (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 }}>
          in <Text style={{ color: Colors.textPrimary }}>{cwd}</Text>
        </Text>
      ) : null}
    </View>
  )
}
