import React from 'react'
import { ScrollView, View, Text, Pressable, ActivityIndicator, KeyboardAvoidingView, Platform, Keyboard } from 'react-native'
import { useLocalSearchParams, router } from 'expo-router'
import { useQuery, useMutation } from 'convex/react'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useBridge } from '@/providers/ws'
import { Composer } from '@/components/composer'
import { useHeaderStore } from '@/lib/header-store'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'
import { ExecBeginRow } from '@/components/jsonl/ExecBeginRow'
import { UserMessageRow } from '@/components/jsonl/UserMessageRow'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { Ionicons } from '@expo/vector-icons'

export default function ConvexThreadDetail() {
  const params = useLocalSearchParams<{ id: string; new?: string; send?: string }>()
  const id = params?.id as string
  const isNew = typeof params?.new === 'string'

  // Load thread (for title) — tolerate null while loading
  const thread = (useQuery as any)('threads:byId', { id }) as any
  const [headerTitle, setHeaderTitle] = React.useState<string>(() => (isNew ? 'New Thread' : ''))

  // Keep the header stable: prefer the loaded title, fall back to "New Thread" for brand-new flows,
  // otherwise retain the last known title instead of flashing a generic placeholder.
  React.useEffect(() => {
    setHeaderTitle((prev) => {
      if (thread?.title) return String(thread.title)
      if (isNew) return 'New Thread'
      return prev
    })
  }, [thread?.title, isNew])

  const lastThreadIdRef = React.useRef<string | null>(null)
  React.useEffect(() => {
    if (lastThreadIdRef.current !== id) {
      lastThreadIdRef.current = id || null
      setHeaderTitle(thread?.title ? String(thread.title) : (isNew ? 'New Thread' : ''))
    }
  }, [id, isNew, thread?.title])

  useHeaderTitle(headerTitle)

  // Live messages subscription for this thread: use the thread.threadId, not the Convex doc _id
  const messages = (useQuery as any)('messages:forThread', { threadId: thread?.threadId || (isNew ? String(id || '') : ''), limit: 400 }) as any[] | undefined | null

  const meta = React.useMemo(() => {
    const arr: any[] = Array.isArray(messages) ? messages : []
    const isMeta = (m: any) => {
      if (!m) return false
      if ((m.kind || (m.role ? 'message' : '')) !== 'message') return false
      if ((m.role || '').toLowerCase() !== 'user') return false
      const t = String(m.text || '')
      // Heuristics: instructions, environment, or preface config JSON
      if (/^\s*<user_instructions>/i.test(t)) return true
      if (/^\s*#\s*Repository\s+Guidelines/i.test(t)) return true
      if (/<environment_context>/i.test(t)) return true
      if (/"sandbox"\s*:\s*"|"approval"\s*:\s*"/i.test(t)) return true
      return false
    }
    let count = 0
    for (let i = 0; i < arr.length; i++) {
      if (isMeta(arr[i])) count++; else break
      if (count >= 3) break
    }
    return { count, items: arr.slice(0, count) }
  }, [messages])

  const enqueueRun = (useMutation as any)('runs:enqueue') as (args: { threadDocId: string; text: string; role?: string; projectId?: string }) => Promise<any>
  const headerHeight = useHeaderStore((s) => s.height)
  const ws = useBridge()
  const insets = useSafeAreaInsets()
  const [kbVisible, setKbVisible] = React.useState(false)
  const scrollRef = React.useRef<ScrollView | null>(null)
  const [atBottom, setAtBottom] = React.useState(true)
  const prevLenRef = React.useRef(0)
  const composerRef = React.useRef<any>(null)
  React.useEffect(() => {
    const show = Keyboard.addListener('keyboardDidShow', () => setKbVisible(true))
    const hide = Keyboard.addListener('keyboardDidHide', () => setKbVisible(false))
    return () => { try { show.remove(); hide.remove(); } catch {} }
  }, [])
  // Auto-focus input for brand-new threads
  React.useEffect(() => {
    if (!isNew) return
    const t = setTimeout(() => { try { composerRef.current?.focus?.() } catch {} }, 120)
    return () => clearTimeout(t)
  }, [isNew])
  // Auto-send initial text if provided from fallback page
  const autoSentRef = React.useRef(false)
  React.useEffect(() => {
    if (!isNew || autoSentRef.current) return
    const initial = typeof params?.send === 'string' ? String(params.send) : ''
    if (!initial || !thread?._id) return
    autoSentRef.current = true
    ;(async () => {
      try { await enqueueRun({ threadDocId: String(thread._id), text: decodeURIComponent(initial), role: 'user', projectId: thread?.projectId || undefined }) } catch {}
      // Debug echo to confirm WS path before run.submit
      try { ws.send(JSON.stringify({ control: 'echo', tag: 'autoSend', threadDocId: String(thread._id), previewLen: decodeURIComponent(initial).length })) } catch {}
      try { ws.send(JSON.stringify({ control: 'run.submit', threadDocId: String(thread._id), text: decodeURIComponent(initial), projectId: thread?.projectId || undefined, resumeId: thread?.resumeId || undefined })) } catch {}
    })()
  }, [isNew, params?.send, thread?._id])
  // Auto-scroll: snap to bottom on first load; keep pinned when more messages arrive
  React.useEffect(() => {
    const len = Array.isArray(messages) ? messages.length : 0
    if (len <= 0) { prevLenRef.current = len; return }
    if (prevLenRef.current === 0) {
      // First hydrate
      const t = setTimeout(() => { try { scrollRef.current?.scrollToEnd({ animated: false }) } catch {} }, 0)
      prevLenRef.current = len
      return () => clearTimeout(t)
    }
    if (len > prevLenRef.current && atBottom) {
      const t = setTimeout(() => { try { scrollRef.current?.scrollToEnd({ animated: true }) } catch {} }, 0)
      prevLenRef.current = len
      return () => clearTimeout(t)
    }
    prevLenRef.current = len
  }, [messages, atBottom])
  

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView
        ref={scrollRef}
        style={{ flex: 1, backgroundColor: Colors.background }}
        contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 88 }}
        onScroll={(e) => {
          try {
            const { contentOffset, layoutMeasurement, contentSize } = e.nativeEvent
            const atBot = contentOffset.y + layoutMeasurement.height >= contentSize.height - 24
            setAtBottom(atBot)
          } catch {}
        }}
        scrollEventThrottle={32}
      >

      {(messages === undefined && !isNew) ? (
        <ActivityIndicator color={Colors.secondary} />
      ) : (messages === null) ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No Convex deployment or query missing (messages:forThread).</Text>
      ) : (!Array.isArray(messages) || messages.length === 0) ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No messages yet.</Text>
      ) : (
        <View style={{ gap: 10 }}>
          {/* Collapsed metadata bar */}
          {meta.count > 0 && (
            <Pressable
              onPress={() => { try { router.push(`/convex/thread/${encodeURIComponent(String(id))}/metadata`) } catch {} }}
              accessibilityRole="button"
              style={{ paddingVertical: 6, paddingHorizontal: 2, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}
            >
              <Text numberOfLines={1} style={{ color: Colors.secondary, fontFamily: Typography.primary }}>View conversation metadata</Text>
              <Ionicons name="chevron-forward" size={16} color={Colors.tertiary} />
            </Pressable>
          )}
          {(() => {
            const arr: any[] = (messages as any[]).slice(meta.count)
            const lastIdx = new Map<string, number>()
            arr.forEach((m, i) => {
              const kind = m?.kind || (m?.role ? 'message' : 'item')
              if (kind !== 'cmd') return
              try {
                const d = m.data || (typeof m.text === 'string' ? JSON.parse(m.text) : {})
                const command = String(d.command || '')
                if (command) lastIdx.set(command, i)
              } catch {}
            })
            return arr.map((m: any, idx: number) => {
              const kind = m.kind || (m.role ? 'message' : 'item')
              if (kind === 'message') {
                return (
                  <Pressable key={m._id || `${m.threadId}-${m.ts}`}
                    onPress={() => { try { router.push(`/convex/message/${encodeURIComponent(String(m._id || ''))}`) } catch {} }}
                    accessibilityRole="button"
                    style={{ paddingVertical: 2 }}>
                    {m.role === 'assistant' ? (
                      <MarkdownBlock markdown={String(m.text || '')} />
                    ) : (
                      <UserMessageRow text={String(m.text || '')} />
                    )}
                  </Pressable>
                )
              }
              if (kind === 'reason') {
                return (
                  <ReasoningHeadline key={m._id || `${m.threadId}-${m.ts}`} text={String(m.text || '')} />
                )
              }
              if (kind === 'cmd') {
                try {
                  const d = m.data || (typeof m.text === 'string' ? JSON.parse(m.text) : {})
                  const command = String(d.command || '')
                  const status = String(d.status || '')
                  const isLatest = command ? lastIdx.get(command) === idx : true
                  if (!isLatest) return null
                  return (
                    <Pressable
                      key={m._id || `${m.threadId}-${m.ts}`}
                      onPress={() => { try { router.push(`/convex/message/${encodeURIComponent(String(m._id || ''))}`) } catch {} }}
                      accessibilityRole="button"
                      style={{ paddingVertical: 2 }}
                    >
                      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{'>'}</Text>
                        <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 12 }}>
                          {command || 'shell'}
                        </Text>
                      </View>
                    </Pressable>
                  )
                } catch { return null }
              }
              return null
            })
          })()}
        </View>
      )}
      </ScrollView>
      {!atBottom && (
        <Pressable
          onPress={() => { try { scrollRef.current?.scrollToEnd({ animated: true }) } catch {} }}
          accessibilityRole="button"
          style={{ position: 'absolute', right: 16, bottom: 88, width: 36, height: 36, borderRadius: 18, backgroundColor: Colors.foreground, alignItems: 'center', justifyContent: 'center' }}
        >
          <Ionicons name="chevron-down" size={20} color={Colors.black} />
        </Pressable>
      )}
      <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 4}>
        <View style={{ paddingBottom: Math.max(kbVisible ? 8 : insets.bottom, 8), paddingHorizontal: 8 }}>
          <Composer
            onSend={async (txt) => {
              const base = String(txt || '').trim()
              if (!base || !thread?._id) return
              try {
                await enqueueRun({ threadDocId: String(thread._id), text: base, role: 'user', projectId: thread?.projectId || undefined })
              } catch {}
              // Trigger bridge workflow via WebSocket control command (no HTTP)
              try { ws.send(JSON.stringify({ control: 'echo', tag: 'composer.onSend', threadDocId: String(thread._id), previewLen: base.length })) } catch {}
              try { ws.send(JSON.stringify({ control: 'run.submit', threadDocId: String(thread._id), text: base, projectId: thread?.projectId || undefined, resumeId: thread?.resumeId || undefined })) } catch {}
            }}
            connected={true}
            isRunning={false}
            onQueue={() => {}}
            onInterrupt={() => {}}
            queuedMessages={[]}
            prefill={null}
            onDraftChange={() => {}}
            inputRef={composerRef}
          />
        </View>
      </KeyboardAvoidingView>
    </View>
  )
}
