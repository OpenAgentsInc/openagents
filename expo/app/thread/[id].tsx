import React from 'react'
import { useLocalSearchParams, router } from 'expo-router'
import { typedRouter } from '@/lib/typed-router'
import { FlatList, Text, View, KeyboardAvoidingView, Platform, Pressable } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
// import { useTinyvex } from '@/providers/tinyvex'
import { useTinyvexThread } from 'tinyvex/react'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { Composer } from '@/components/Composer'
import { useBridge } from '@/providers/ws'
import { useHeaderTitle, useHeaderStore } from '@/lib/header-store'
import { useAcp, type SessionNotificationWithTs } from '@/providers/acp'
import { useSettings } from '@/lib/settings-store'
import { useThreadProviders, ChatMessageBubble } from '@openagentsinc/core'
import * as Haptics from 'expo-haptics'
// tinyvex/react provides history + live; no custom timeline hook needed

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
  const { status, history, live, send: sendViaTinyvex, refresh, threadId: resolvedId } = useTinyvexThread({ idOrAlias: threadId || '' })
  const { connected } = useBridge()
  const agentProvider = useSettings((s) => s.agentProvider)
  const setAgentProvider = useSettings((s) => s.setAgentProvider)
  const providerByThread = useThreadProviders((s) => s.byThread)
  const setThreadProvider = useThreadProviders((s) => s.setProvider)
  // Title for thread screen
  useHeaderTitle('New Thread')
  const acpUpdates = React.useMemo(() => eventsForThread(threadId), [eventsForThread, threadId])

  // Ensure we are subscribed to Tinyvex messages for this thread and have a recent snapshot
  React.useEffect(() => { /* tinyvex/react hook handles subscribe/query */ }, [threadId])
  // If the hook reports ready but no history yet, opportunistically refresh once
  React.useEffect(() => {
    if (status === 'ready' && (history?.length ?? 0) === 0 && (resolvedId || threadId)) {
      try { refresh() } catch {}
    }
  }, [status, history?.length, resolvedId, threadId, refresh])
  // When navigating into a thread, if we have a recorded provider for it, switch the active agent accordingly
  React.useEffect(() => {
    if (!threadId) return
    try {
      const p = providerByThread[threadId]
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
    try { sendViaTinyvex(text, { resumeId: 'last', provider: agentProvider === 'claude_code' ? 'claude_code' : undefined }) } catch {}
    try { setThreadProvider(threadId, agentProvider) } catch {}
  }, [threadId, sendViaTinyvex, agentProvider])
  const insets = useSafeAreaInsets()
  const headerHeight = useHeaderStore((s) => s.height)
  const keyboardOffset = Platform.OS === 'ios' ? headerHeight : 0

  // Auto-scroll to the bottom when opening a thread (once per thread navigation)
  type Item = { key: string; ts: number; role: 'assistant' | 'user'; text: string }
  const listRef = React.useRef<FlatList<Item>>(null)
  const didAutoScrollFor = React.useRef<string | null>(null)
  const pendingAutoScroll = React.useRef<boolean>(false)
  const items: Item[] = React.useMemo(() => {
    const hist: Item[] = (history || []).map((r, i) => ({ key: `h-${r.id}-${i}`, ts: r.ts, role: r.role === 'assistant' ? 'assistant' : 'user', text: String(r.text || '') }))
    const liveItems: Item[] = live.assistant ? [{ key: 'live-a', ts: Date.now(), role: 'assistant', text: live.assistant }] : []
    return [...hist, ...liveItems]
  }, [history, live.assistant])

  React.useEffect(() => {
    // Reset flag whenever thread changes
    didAutoScrollFor.current = null
    pendingAutoScroll.current = true
  }, [threadId])

  React.useEffect(() => {
    if (!threadId) return
    if (didAutoScrollFor.current === threadId) return
    if ((items?.length ?? 0) === 0) return
    // Defer to next frame to ensure layout complete
    try {
      requestAnimationFrame(() => {
        try { listRef.current?.scrollToEnd({ animated: false }) } catch {}
        didAutoScrollFor.current = threadId
        pendingAutoScroll.current = false
      })
    } catch {
      try { listRef.current?.scrollToEnd({ animated: false }) } catch {}
      didAutoScrollFor.current = threadId
      pendingAutoScroll.current = false
    }
  }, [threadId, items])
  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={keyboardOffset} style={{ flex: 1, backgroundColor: Colors.background }}>
      <View key={`thread-${threadId}`} style={{ flex: 1 }}>
      <FlatList
        ref={listRef}
        data={items}
        keyExtractor={(it) => it.key}
        renderItem={({ item }) => {
          const role: 'assistant' | 'user' = item.role === 'assistant' ? 'assistant' : 'user'
          return (
            <View style={{ paddingVertical: 4 }}>
              <ChatMessageBubble role={role} text={item.text} />
            </View>
          )
        }}
        contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 80 }}
        keyboardShouldPersistTaps='handled'
        onLayout={() => {
          if (pendingAutoScroll.current && threadId) {
            try { listRef.current?.scrollToEnd({ animated: false }) } catch {}
            didAutoScrollFor.current = threadId
            pendingAutoScroll.current = false
          }
        }}
        onContentSizeChange={() => {
          if (pendingAutoScroll.current && threadId) {
            try { listRef.current?.scrollToEnd({ animated: false }) } catch {}
            didAutoScrollFor.current = threadId
            pendingAutoScroll.current = false
          }
        }}
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
