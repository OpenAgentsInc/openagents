import React from 'react'
import { useLocalSearchParams, router } from 'expo-router'
import { typedRouter } from '@/lib/typed-router'
import { FlatList, Text, View, KeyboardAvoidingView, Platform, Pressable } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
// import { useTinyvex } from '@/providers/tinyvex'
import { useTinyvexThread } from 'tinyvex/react'
import type { MessageRowTs } from '@/types/bridge/MessageRowTs'
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
  // Title for thread screen: mirror drawer snippet/title logic
  const headerTitle = React.useMemo(() => {
    // If a live assistant message is present, prefer that as it's the most recent context
    const liveText = String(live?.assistant || '').trim()
    if (liveText) return sanitizeTitle(liveText)
    // Otherwise, use the latest message text from history
    const arr: MessageRowTs[] = Array.isArray(history) ? (history.slice() as MessageRowTs[]) : []
    if (arr.length > 0) {
      try {
        arr.sort((a, b) => (Number(a.ts || 0) - Number(b.ts || 0)))
        const last = arr[arr.length - 1] as MessageRowTs
        const raw = String((last && last.text) || '')
        const cleaned = sanitizeTitle(raw)
        if (cleaned) return cleaned
      } catch {}
    }
    // Fall back based on thread state
    return (initialId === 'new' || !threadId) ? 'New Thread' : 'Thread'
  }, [history, live?.assistant, threadId, initialId])
  useHeaderTitle(headerTitle)
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
    const base: MessageRowTs[] = Array.isArray(history) ? (history as MessageRowTs[]) : []
    const hist: Item[] = base.map((r, i) => ({ key: `h-${String(r.id)}-${i}`, ts: Number(r.ts as unknown as number || 0), role: (r.role === 'assistant' ? 'assistant' : 'user'), text: String(r.text || '') }))
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
    const len = items?.length ?? 0
    if (len === 0) return
    const toBottom = () => {
      try { listRef.current?.scrollToIndex?.({ index: len - 1, animated: false }) } catch {}
      try { listRef.current?.scrollToEnd?.({ animated: false }) } catch {}
      didAutoScrollFor.current = threadId
      pendingAutoScroll.current = false
    }
    // Defer twice to make sure layout is committed on web
    try {
      requestAnimationFrame(() => { setTimeout(toBottom, 0) })
    } catch { setTimeout(toBottom, 0) }
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
        initialScrollIndex={Math.max(0, items.length - 1)}
        onScrollToIndexFailed={(info) => {
          // Wait for layout to settle and try again
          setTimeout(() => {
            try { listRef.current?.scrollToIndex?.({ index: info.index, animated: false }) } catch {}
            try { listRef.current?.scrollToEnd?.({ animated: false }) } catch {}
          }, 50)
        }}
        removeClippedSubviews={false}
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

function sanitizeTitle(input: string): string {
  try {
    return input
      .replace(/```[\s\S]*?```/g, '')
      .replace(/`([^`]*)`/g, '$1')
      .replace(/^#+\s*/gm, '')
      .replace(/\*\*([^*]+)\*\*/g, '$1')
      .replace(/\*([^*]+)\*/g, '$1')
      .replace(/\[(.*?)\]\([^)]*\)/g, '$1')
      .replace(/\s+/g, ' ')
      .trim()
  } catch {
    return input
  }
}
