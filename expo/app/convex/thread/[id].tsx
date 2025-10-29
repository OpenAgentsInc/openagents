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
import { UserMessageRow } from '@/components/jsonl/UserMessageRow'
import { SessionUpdatePlan, SessionUpdateAvailableCommandsUpdate, SessionUpdateCurrentModeUpdate, SessionUpdateToolCall } from '@/components/acp'
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
  // ACP WS feed removed: Convex is the single source of truth for rendering.
  React.useEffect(() => {
    // Use 'will' events on iOS so the toggle happens in sync with
    // the keyboard animation, avoiding a choppy jump in padding.
    const showEvent = Platform.OS === 'ios' ? 'keyboardWillShow' : 'keyboardDidShow'
    const hideEvent = Platform.OS === 'ios' ? 'keyboardWillHide' : 'keyboardDidHide'
    const show = Keyboard.addListener(showEvent as any, () => setKbVisible(true))
    const hide = Keyboard.addListener(hideEvent as any, () => setKbVisible(false))
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
  
  // No WS ACP subscription; all content comes from Convex queries.


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
      ) : (!Array.isArray(messages) || (messages.length === 0)) ? (
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
            return (
              <>
                {arr.map((m: any) => {
                  const key = m._id || `${m.threadId}-${m.ts}`
                  const kind = String(m.kind || (m.role ? 'message' : '')).toLowerCase()
                  const role = String(m.role || '').toLowerCase()
                  if (kind === 'message') {
                    if (role === 'user') {
                      return (
                        <Pressable key={key}
                          onPress={() => { try { router.push(`/convex/message/${encodeURIComponent(String(m._id || ''))}`) } catch {} }}
                          accessibilityRole="button"
                          style={{ paddingVertical: 2 }}>
                          <UserMessageRow text={String(m.text || '')} />
                        </Pressable>
                      )
                    }
                    return (
                      <View key={key} style={{ paddingVertical: 2 }}>
                        <MarkdownBlock markdown={String(m.text || '')} />
                      </View>
                    )
                  }
                  if (kind === 'reason') {
                    return (
                      <View key={key} style={{ paddingVertical: 2 }}>
                        <ReasoningHeadline text={String(m.text || '')} />
                      </View>
                    )
                  }
                  if (kind === 'plan') {
                    const entries = (m?.data?.entries as any[]) || (() => { try { const o = JSON.parse(String(m.text||'')); return (o && o.entries) || [] } catch { return [] } })()
                    if (Array.isArray(entries) && entries.length > 0) {
                    return (
                      <View key={key} style={{ paddingVertical: 2 }}>
                        <SessionUpdatePlan entries={entries} />
                      </View>
                    )
                    }
                  }
                  if (kind === 'available_commands_update') {
                    const cmds = (Array.isArray(m?.data?.available_commands) ? m.data.available_commands : (() => { try { const o = JSON.parse(String(m.text||'')); return o?.available_commands || [] } catch { return [] } })()) as any[]
                    if (cmds.length > 0) {
                    return (
                      <View key={key} style={{ paddingVertical: 2 }}>
                        <SessionUpdateAvailableCommandsUpdate available_commands={cmds} />
                      </View>
                    )
                    }
                  }
                  if (kind === 'current_mode_update') {
                    const cm = (m?.data?.currentModeId || m?.data?.current_mode_id) || (() => { try { const o = JSON.parse(String(m.text||'')); return o?.currentModeId || o?.current_mode_id } catch { return '' } })()
                    if (cm) {
                    return (
                      <View key={key} style={{ paddingVertical: 2 }}>
                        <SessionUpdateCurrentModeUpdate currentModeId={String(cm || '')} />
                      </View>
                    )
                    }
                  }
                  if ((kind === 'tool' || kind === 'cmd' || kind === 'file' || kind === 'search' || kind === 'mcp' || kind === 'todo')) {
                    const tc = m.data || (() => { try { return JSON.parse(String(m.text||'')) } catch { return null } })()
                    if (tc && (tc.title || tc.kind || tc.status)) {
                      return (
                        <View key={key} style={{ paddingVertical: 2 }}>
                          <SessionUpdateToolCall {...(tc as any)} />
                        </View>
                      )
                    }
                    const body = typeof m.text === 'string' && m.text.trim().startsWith('{') ? `\n\n\`\`\`json\n${m.text}\n\`\`\`` : String(m.text || '')
                    return (
                      <View key={key} style={{ paddingVertical: 2 }}>
                        <MarkdownBlock markdown={`_${kind.toUpperCase()}_ ${body}`} />
                      </View>
                    )
                  }
                  return (
                    <View key={key} style={{ paddingVertical: 2 }}>
                      <MarkdownBlock markdown={`_${kind.toUpperCase() || 'ITEM'}_\n\n${String(m.text || '')}`}/>
                    </View>
                  )
                })}
              </>
            )
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
