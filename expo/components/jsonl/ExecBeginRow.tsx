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
  const labelFor = (k: string) => {
    const map: Record<string, string> = {
      ReadFile: 'Read',
      Read: 'Read',
      WriteFile: 'Write',
      Write: 'Write',
      OpenFile: 'Open',
      ListFiles: 'ListFiles',
      Search: 'Search',
    }
    return map[k] ?? k
  }
  const pickPath = (v: any): string | undefined => {
    if (!v) return undefined
    if (typeof v === 'string') return v
    if (typeof v.path === 'string') return v.path
    if (typeof v.name === 'string') return v.name
    if (typeof v.file === 'string') return v.file
    if (typeof v.filename === 'string') return v.filename
    if (Array.isArray(v.files) && typeof v.files[0] === 'string') return v.files[0]
    return undefined
  }
  const shorten = (p?: string): string | undefined => {
    if (!p) return p
    // Prefer repo-relative subpaths if present
    for (const marker of ['/openagents/', '/expo/', '/crates/', '/docs/']) {
      const idx = p.indexOf(marker)
      if (idx >= 0) return p.slice(idx + (marker === '/openagents/' ? '/openagents/'.length : 0))
    }
    return p
  }
  try {
    const parsed = (payload as any)?.parsed
    if (Array.isArray(parsed) && parsed.length > 0 && parsed[0] && typeof parsed[0] === 'object') {
      const first = parsed[0] as Record<string, any>
      const keys = Object.keys(first)
      if (keys.length === 1) {
        const k = keys[0]
        const v = first[k] ?? {}
        const p = pickPath(v)
        let action = labelFor(k)
        let path = shorten(p)
        // Avoid surfacing literal "Unknown" to users.
        if (action === 'Unknown' || action.toLowerCase() === 'unknown') {
          // Try to recover: show the raw shell command if available
          const rawCmd = Array.isArray((v as any)?.cmd)
            ? (v as any).cmd.join(' ')
            : (typeof (v as any)?.cmd === 'string' ? (v as any).cmd : undefined)
          action = 'Run'
          path = shorten(rawCmd) ?? undefined
        }
        pretty = { action, path }
      }
    }
  } catch {}

  if (pretty?.action) {
    // If ListFiles has no explicit path, show the current directory indicator
    const isList = pretty.action === 'ListFiles'
    const shownPath = isList ? (pretty.path ?? '.') : pretty.path
    return (
      <View style={{ paddingVertical: 2 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>
          <Text style={{ color: Colors.textPrimary, fontFamily: Typography.primary }}>{pretty.action}</Text>
          {shownPath ? (
            <Text>
              {' '}
              {isList ? addSlash(shownPath) : shownPath}
            </Text>
          ) : null}
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
