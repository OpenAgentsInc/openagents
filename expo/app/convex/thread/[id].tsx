import React from 'react'
import { ScrollView, View, Text, Pressable, ActivityIndicator, KeyboardAvoidingView, Platform } from 'react-native'
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

export default function ConvexThreadDetail() {
  const { id } = useLocalSearchParams<{ id: string }>()

  // Load thread (for title) — tolerate null while loading
  const thread = (useQuery as any)('threads:byId', { id }) as any
  useHeaderTitle(thread?.title ? String(thread.title) : 'Thread')

  // Live messages subscription for this thread: use the thread.threadId, not the Convex doc _id
  const messages = (useQuery as any)('messages:forThread', { threadId: thread?.threadId || '' }) as any[] | undefined | null

  const createDemo = (useMutation as any)('messages:createDemo') as (args: { threadId: string }) => Promise<any>
  const enqueueRun = (useMutation as any)('runs:enqueue') as (args: { threadDocId: string; text: string; role?: string; projectId?: string }) => Promise<any>
  const headerHeight = useHeaderStore((s) => s.height)
  const ws = useBridge()
  const [busy, setBusy] = React.useState(false)
  const onCreateDemo = async () => {
    if (busy) return
    setBusy(true)
    try { await createDemo({ threadId: id! }) } catch {} finally { setBusy(false) }
  }

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 88 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18 }}>Messages</Text>
        <Pressable onPress={onCreateDemo} disabled={busy} style={{ opacity: busy ? 0.6 : 1, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 10, paddingVertical: 6 }}>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>Add demo message</Text>
        </Pressable>
      </View>

      {messages === undefined ? (
        <ActivityIndicator color={Colors.secondary} />
      ) : messages === null ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No Convex deployment or query missing (messages:forThread).</Text>
      ) : messages.length === 0 ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No messages yet.</Text>
      ) : (
        <View style={{ gap: 10 }}>
          {messages.map((m: any) => {
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
        <View style={{ paddingBottom: 8, paddingHorizontal: 8 }}>
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
