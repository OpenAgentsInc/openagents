import React from 'react'
import { FlatList, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useIsDevEnv } from '@/lib/env'
import { useHeaderTitle } from '@/lib/header-store'
import { useTinyvex, type ThreadRow } from '@/providers/tinyvex'
import { useArchiveStore } from '@/lib/archive-store'
import { useThreadTimeline } from '@/hooks/use-thread-timeline'
import { useThreadProviders, type AgentProvider } from '@/lib/thread-provider-store'

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
  if (!isDev) {
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14 }}>Developer tools are only available in dev mode.</Text>
      </View>
    )
  }
  const { threads, subscribeMessages, queryMessages, queryToolCalls } = useTinyvex()
  const isArchived = useArchiveStore((s) => s.isArchived)
  const providerMap = useThreadProviders((s) => s.byThread)
  const target = React.useMemo(() => pickMostRecentCodex(threads, isArchived, providerMap), [threads, isArchived, providerMap])
  const threadId = String(target?.id || '')
  const timeline = useThreadTimeline(threadId)

  // Subscribe and warm the selected thread
  React.useEffect(() => {
    if (!threadId) return
    try { subscribeMessages(threadId) } catch {}
    try { queryMessages(threadId, 50) } catch {}
    try {
      // Query tool calls using canonical resume_id if present (stored on thread rows as resume_id)
      const canon = String(target?.resume_id || threadId)
      queryToolCalls(canon, 50)
    } catch {}
  }, [threadId, subscribeMessages, queryMessages, queryToolCalls, target])

  if (!threadId) {
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center', paddingHorizontal: 24 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, textAlign: 'center' }}>No Codex chats found yet. Start a chat and come back here.</Text>
      </View>
    )
  }

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <FlatList
        data={timeline}
        keyExtractor={(it) => it.key}
        renderItem={({ item }) => (<View style={{ paddingVertical: 4 }}>{item.node}</View>)}
        contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 24 }}
        keyboardShouldPersistTaps='handled'
        maintainVisibleContentPosition={{ minIndexForVisible: 0 }}
        style={{ flex: 1 }}
      />
      <View style={{ padding: 10, borderTopWidth: 1, borderTopColor: Colors.border, backgroundColor: Colors.background }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Thread: {threadId}</Text>
      </View>
    </View>
  )
}

