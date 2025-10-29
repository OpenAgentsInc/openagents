import React from 'react'
import { useLocalSearchParams, router } from 'expo-router'
import { ScrollView, Text, View, KeyboardAvoidingView, Platform } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { useTinyvex } from '@/providers/tinyvex'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { Composer } from '@/components/composer'
import { useBridge } from '@/providers/ws'
import { useHeaderTitle } from '@/lib/header-store'
import { useAcp } from '@/providers/acp'
import { SessionUpdateAgentMessageChunk } from '@/components/acp/SessionUpdateAgentMessageChunk'
import { SessionUpdateAgentThoughtChunk } from '@/components/acp/SessionUpdateAgentThoughtChunk'
import { SessionUpdateUserMessageChunk } from '@/components/acp/SessionUpdateUserMessageChunk'
import { SessionUpdatePlan } from '@/components/acp/SessionUpdatePlan'
import { SessionUpdateToolCall } from '@/components/acp/SessionUpdateToolCall'

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
  const { sessions } = useAcp()
  const { send, connected, addSubscriber } = useBridge()
  // Title for thread screen
  useHeaderTitle('New Thread')
  const [sessionId, setSessionId] = React.useState<string>('')
  React.useEffect(() => {
    const unsub = addSubscriber?.((line: string) => {
      try {
        const obj = JSON.parse(String(line || ''))
        if (obj?.type === 'bridge.session_started' && obj.clientThreadDocId && obj.sessionId) {
          if (String(obj.clientThreadDocId) === threadId) setSessionId(String(obj.sessionId))
        }
      } catch {}
    })
    return () => { try { unsub && unsub() } catch {} }
  }, [threadId, addSubscriber])
  const acpUpdates = React.useMemo(() => sessions[sessionId] || [], [sessions, sessionId])
  const onSend = React.useCallback((text: string) => {
    if (!threadId) return
    const payload = { control: 'run.submit', threadDocId: threadId, text, resumeId: 'new' as const }
    try { send(JSON.stringify(payload)) } catch {}
  }, [threadId, send])
  const insets = useSafeAreaInsets()
  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : undefined} style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView style={{ flex: 1 }} contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 80 }}>
        {acpUpdates.length === 0 ? (
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No messages yet.</Text>
        ) : acpUpdates.map((n, idx) => (
          <View key={idx} style={{ paddingVertical: 4 }}>
            {(() => {
              const u: any = (n as any).update
              if (u?.sessionUpdate === 'user_message_chunk') {
                return <SessionUpdateUserMessageChunk content={u.content} />
              }
              if (u?.sessionUpdate === 'agent_message_chunk') {
                return <SessionUpdateAgentMessageChunk content={u.content} />
              }
              if (u?.sessionUpdate === 'agent_thought_chunk') {
                return <SessionUpdateAgentThoughtChunk content={u.content} />
              }
              if (u?.sessionUpdate === 'plan') {
                return <SessionUpdatePlan entries={u.entries || []} />
              }
              if (u?.sessionUpdate === 'tool_call') {
                const props = { title: u.title, status: u.status, kind: u.kind, content: u.content, locations: u.locations }
                return <SessionUpdateToolCall {...props as any} />
              }
              return null
            })()}
          </View>
        ))}
      </ScrollView>
      <View style={{ paddingTop: 10, paddingHorizontal: 10, paddingBottom: Math.max(10, insets.bottom), borderTopWidth: 1, borderTopColor: Colors.border, backgroundColor: Colors.background }}>
        <Composer onSend={onSend} connected={connected} placeholder='Ask Codex' />
      </View>
    </KeyboardAvoidingView>
  )
}
