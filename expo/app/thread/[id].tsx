import React from 'react'
import { useLocalSearchParams, router } from 'expo-router'
import { FlatList, Text, View, KeyboardAvoidingView, Platform, Pressable } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { useTinyvex } from '@/providers/tinyvex'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { Composer } from '@/components/composer'
import { useBridge } from '@/providers/ws'
import { useHeaderTitle, useHeaderStore } from '@/lib/header-store'
import { useAcp, type SessionNotificationWithTs } from '@/providers/acp'
import { useSettings } from '@/lib/settings-store'
import { useThreadProviders } from '@/lib/thread-provider-store'
import { SessionUpdateAgentMessageChunk } from '@/components/acp/SessionUpdateAgentMessageChunk'
import { SessionUpdateAgentThoughtChunk } from '@/components/acp/SessionUpdateAgentThoughtChunk'
import { SessionUpdateUserMessageChunk } from '@/components/acp/SessionUpdateUserMessageChunk'
import { SessionUpdatePlan } from '@/components/acp/SessionUpdatePlan'
import { SessionUpdateToolCall } from '@/components/acp/SessionUpdateToolCall'
import * as Haptics from 'expo-haptics'
import { useThreadTimeline } from '@/hooks/use-thread-timeline'

export default function ThreadScreen() {
  const { id } = useLocalSearchParams<{ id: string }>()
  const initialId = String(id || '')
  const [threadId, setThreadId] = React.useState<string>('')
  React.useEffect(() => {
    if (initialId === 'new' || initialId === '') {
      const gen = `t-${Date.now()}`
      setThreadId(gen)
      try { router.replace(`/thread/${encodeURIComponent(gen)}` as any) } catch {}
    } else {
      setThreadId(initialId)
    }
    // Only re-run if the URL param changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialId])
  const { eventsForThread } = useAcp()
  const { messagesByThread, subscribeMessages, queryMessages, queryToolCalls, toolCallsByThread, threads } = useTinyvex() as any
  const { send, connected } = useBridge()
  const agentProvider = useSettings((s) => s.agentProvider)
  const setAgentProvider = useSettings((s) => s.setAgentProvider)
  const threadProviders = useThreadProviders()
  // Title for thread screen
  useHeaderTitle('New Thread')
  const acpUpdates = React.useMemo(() => eventsForThread(threadId), [eventsForThread, threadId])
  const tvxMessages = React.useMemo(() => messagesByThread[threadId] || [], [messagesByThread, threadId])
  const timeline = useThreadTimeline(threadId)

  // Ensure we are subscribed to Tinyvex messages for this thread and have a recent snapshot
  React.useEffect(() => {
    if (!threadId) return
    try { subscribeMessages(threadId) } catch {}
    try { queryMessages(threadId, 50) } catch {}
    try {
      // Prefer canonical session id if the threads list records it as resume_id
      const rows: any[] = Array.isArray(threads) ? threads : []
      const row = rows.find((r: any) => String((r?.id || r?.thread_id || r?.threadId || '')) === threadId)
      const canon = String((row?.resume_id || row?.resumeId || threadId) || threadId)
      queryToolCalls?.(canon, 50)
    } catch {}
  }, [threadId, subscribeMessages, queryMessages])
  // When navigating into a thread, if we have a recorded provider for it, switch the active agent accordingly
  React.useEffect(() => {
    if (!threadId) return
    try {
      const p = threadProviders.getProvider(threadId)
      if (p) {
        if (p !== agentProvider) setAgentProvider(p)
      } else {
        // Default to Codex for legacy threads with no mapping
        if (agentProvider !== 'codex') setAgentProvider('codex')
      }
    } catch {}
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [threadId])
  const onSend = React.useCallback((text: string) => {
    if (!threadId) return
    const payload: any = { control: 'run.submit', threadDocId: threadId, text, resumeId: 'last' as const }
    if (agentProvider === 'claude_code') payload.provider = 'claude_code'
    try { send(JSON.stringify(payload)) } catch {}
    // Remember provider for this thread so future resumes use the correct agent
    try { threadProviders.setProvider(threadId, agentProvider) } catch {}
  }, [threadId, send, agentProvider])
  const insets = useSafeAreaInsets()
  const headerHeight = useHeaderStore((s) => s.height)
  const keyboardOffset = Platform.OS === 'ios' ? headerHeight : 0
  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={keyboardOffset} style={{ flex: 1, backgroundColor: Colors.background }}>
      <View key={`thread-${threadId}`} style={{ flex: 1 }}>
      <FlatList
        data={timeline}
        keyExtractor={(it) => it.key}
        renderItem={({ item }) => (<View style={{ paddingVertical: 4 }}>{item.node}</View>)}
        contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 80 }}
        keyboardShouldPersistTaps='handled'
        maintainVisibleContentPosition={{ minIndexForVisible: 0 }}
        style={{ flex: 1 }}
      />
        {/* Provider selector: show only before any messages exist */}
      
      <View style={{ paddingTop: 10, paddingHorizontal: 10, paddingBottom: Math.max(10, insets.bottom), borderTopWidth: 1, borderTopColor: Colors.border, backgroundColor: Colors.background }}>
        <Composer onSend={onSend} connected={connected} placeholder={agentProvider === 'claude_code' ? 'Ask Claude Code' : 'Ask Codex'} />
      </View>
      </View>
    </KeyboardAvoidingView>
  )
}
