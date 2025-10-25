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
import { CommandExecutionCard } from '@/components/jsonl/CommandExecutionCard'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { Ionicons } from '@expo/vector-icons'

export default function ConvexThreadDetail() {
  const params = useLocalSearchParams<{ id: string; new?: string }>()
  const id = params?.id as string
  const isNew = typeof params?.new === 'string'

  // Load thread (for title) — tolerate null while loading
  const thread = (useQuery as any)('threads:byId', { id }) as any
  useHeaderTitle(thread?.title ? String(thread.title) : 'Thread')

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
  React.useEffect(() => {
    const show = Keyboard.addListener('keyboardDidShow', () => setKbVisible(true))
    const hide = Keyboard.addListener('keyboardDidHide', () => setKbVisible(false))
    return () => { try { show.remove(); hide.remove(); } catch {} }
  }, [])
  

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 88 }}>

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
              style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, paddingVertical: 10, paddingHorizontal: 12, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}
            >
              <Text numberOfLines={1} style={{ color: Colors.secondary, fontFamily: Typography.primary }}>View conversation metadata</Text>
              <Ionicons name="chevron-forward" size={16} color={Colors.tertiary} />
            </Pressable>
          )}
          {(messages as any[]).slice(meta.count).map((m: any) => {
            const kind = m.kind || (m.role ? 'message' : 'item')
            if (kind === 'message') {
              return (
                <Pressable key={m._id || `${m.threadId}-${m.ts}`}
                  onPress={() => { try { router.push(`/convex/message/${encodeURIComponent(String(m._id || ''))}`) } catch {} }}
                  accessibilityRole="button"
                  style={{ borderWidth: 1, borderColor: Colors.border, padding: 10 }}>
                  <Text numberOfLines={1} style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{new Date(m.ts).toLocaleString()} · {m.role || 'message'}</Text>
                  <View style={{ height: 6 }} />
                  {m.role === 'assistant' ? (
                    <MarkdownBlock markdown={String(m.text || '')} />
                  ) : (
                    <Text numberOfLines={4} style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{String(m.text || '')}</Text>
                  )}
                </Pressable>
              )
            }
            if (kind === 'reason') {
              return (
                <View key={m._id || `${m.threadId}-${m.ts}`} style={{ borderWidth: 1, borderColor: Colors.border, padding: 10 }}>
                  <ReasoningHeadline text={String(m.text || '')} />
                </View>
              )
            }
            if (kind === 'cmd') {
              try {
                const d = m.data || (typeof m.text === 'string' ? JSON.parse(m.text) : {})
                const command = String(d.command || '')
                const status = String(d.status || '')
                const exitCode = typeof d.exit_code === 'number' ? d.exit_code : undefined
                const out = typeof d.aggregated_output === 'string' ? d.aggregated_output : ''
                return (
                  <View key={m._id || `${m.threadId}-${m.ts}`}>
                    <CommandExecutionCard command={command} status={status} exitCode={exitCode ?? null} sample={out} outputLen={out.length} showExitCode={false} showOutputLen={true} />
                  </View>
                )
              } catch { return null }
            }
            return null
          })}
        </View>
      )}
      </ScrollView>
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
              try { ws.send(JSON.stringify({ control: 'run.submit', threadDocId: String(thread._id), text: base, projectId: thread?.projectId || undefined, resumeId: thread?.resumeId || undefined })) } catch {}
            }}
            connected={true}
            isRunning={false}
            onQueue={() => {}}
            onInterrupt={() => {}}
            queuedMessages={[]}
            prefill={null}
            onDraftChange={() => {}}
            inputRef={undefined as any}
          />
        </View>
      </KeyboardAvoidingView>
    </View>
  )
}
