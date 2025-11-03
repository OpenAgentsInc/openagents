import React from 'react'
import { Text, View, Pressable } from 'react-native'
import { router } from 'expo-router'
import { typedRouter } from '@/lib/typed-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useIsDevEnv } from '@/lib/env'
import { useHeaderTitle } from '@/lib/header-store'
import { useTinyvex, type ThreadRow } from '@/providers/tinyvex'
import { useArchiveStore, useThreadProviders, type AgentProvider } from '@openagentsinc/core'

function pickMostRecentCodex(threads: ThreadRow[], isArchived: (id: string) => boolean, providers: Record<string, AgentProvider | undefined>): ThreadRow | null {
  const arr = Array.isArray(threads) ? threads.slice() : []
  if (arr.length === 0) return null
  // Sort newest → oldest
  arr.sort((a, b) => {
    const at = Number(a.updated_at ?? a.created_at ?? 0)
    const bt = Number(b.updated_at ?? b.created_at ?? 0)
    return bt - at
  })
  // Prefer threads marked codex (or defaulted to codex) and not archived; fall back to first available
  for (const r of arr) {
    const tid = String(r.id || '')
    if (!tid || isArchived(tid)) continue
    const p = providers[tid]
    // Default provider is codex when unspecified
    if (!p || p === 'codex') return r
  }
  for (const r of arr) {
    const tid = String(r.id || '')
    if (!tid || isArchived(tid)) continue
    if (providers[tid] === 'codex') return r
  }
  return null
}

export default function RecentCodexThread() {
  useHeaderTitle('Recent Codex Thread')
  const isDev = useIsDevEnv()
  const { threads } = useTinyvex()
  const isArchived = useArchiveStore((s) => s.isArchived)
  const providerMap = useThreadProviders((s) => s.byThread)
  const target = React.useMemo(() => pickMostRecentCodex(threads, isArchived, providerMap), [threads, isArchived, providerMap])
  const threadId = String(target?.id || '')

  React.useEffect(() => {
    if (!isDev) return
    if (threadId) {
      try { router.replace(`/thread/${encodeURIComponent(threadId)}` as any) } catch {}
    }
  }, [isDev, threadId])

  if (!isDev) {
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14 }}>Developer tools are only available in dev mode.</Text>
      </View>
    )
  }

  if (!threadId) {
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center', paddingHorizontal: 24 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, textAlign: 'center', marginBottom: 10 }}>No Codex chats found yet.</Text>
        <Pressable onPress={() => { try { typedRouter.replace('/thread/new') } catch {} }} accessibilityRole="button" style={{ paddingVertical: 8, paddingHorizontal: 12, borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card }}>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 14 }}>Start New Thread</Text>
        </Pressable>
      </View>
    )
  }

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14 }}>Opening latest Codex chat…</Text>
    </View>
  )
}
