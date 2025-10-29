import React from 'react'
import { useLocalSearchParams, router } from 'expo-router'
import { ScrollView, Text, View, KeyboardAvoidingView, Platform } from 'react-native'
import { useTinyvex } from '@/providers/tinyvex'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { Composer } from '@/components/composer'
import { useBridge } from '@/providers/ws'

export default function ThreadScreen() {
  const { id } = useLocalSearchParams<{ id: string }>()
  const initialId = String(id || '')
  const threadId = React.useMemo(() => {
    if (initialId === 'new' || initialId === '') {
      // Generate a client thread id and replace the route for a stable URL
      const gen = `t-${Date.now()}`
      try { router.replace(`/thread/${encodeURIComponent(gen)}` as any) } catch {}
      return gen
    }
    return initialId
  }, [initialId])
  const { subscribeMessages, queryMessages, messagesByThread } = useTinyvex()
  const { send, connected } = useBridge()
  React.useEffect(() => {
    if (!threadId) return
    queryMessages(threadId, 400)
    subscribeMessages(threadId)
  }, [threadId])
  const rows = messagesByThread[threadId] || []
  const onSend = React.useCallback((text: string) => {
    if (!threadId) return
    const payload = { control: 'run.submit', threadDocId: threadId, text, resumeId: 'new' as const }
    try { send(JSON.stringify(payload)) } catch {}
  }, [threadId, send])
  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : undefined} style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView style={{ flex: 1 }} contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 80 }}>
        {rows.length === 0 ? (
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No messages yet.</Text>
        ) : rows.map((m) => (
          <View key={`${m.id}`} style={{ paddingVertical: 4 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{m.role || m.kind}</Text>
            {!!m.text && <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 14 }}>{m.text}</Text>}
          </View>
        ))}
      </ScrollView>
      <View style={{ padding: 10, borderTopWidth: 1, borderTopColor: Colors.border, backgroundColor: Colors.background }}>
        <Composer onSend={onSend} connected={connected} placeholder='Ask Codex' />
      </View>
    </KeyboardAvoidingView>
  )
}
