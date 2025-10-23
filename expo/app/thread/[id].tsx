import React from 'react'
import { useLocalSearchParams, useRouter, Stack } from 'expo-router'
import { ActivityIndicator, FlatList, Pressable, Text, View } from 'react-native'
import { useThreads } from '@/lib/threads-store'
import { useWs } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'

export default function ThreadHistoryView() {
  const { id, path } = useLocalSearchParams<{ id: string; path?: string }>()
  const { wsUrl, setResumeNextId } = useWs()
  const router = useRouter()
  const loadThread = useThreads((s) => s.loadThread)
  const thread = useThreads((s) => (id ? s.thread[id] : undefined))
  const loadingMap = useThreads((s) => s.loadingThread)
  const loading = Boolean(id && loadingMap[id])

  React.useEffect(() => { if (id) loadThread(wsUrl, id, typeof path === 'string' ? path : undefined).catch(()=>{}) }, [id, path, wsUrl, loadThread])
  useHeaderTitle(thread?.title || 'Thread')

  const items = React.useMemo(() => {
    const base = thread?.items || []
    if (base.length === 0) return base
    const first = base[0]
    const text = String(first?.text ?? '')
    const isUserish = first?.role === 'user' || /^\s*>/.test(text) || /^You:\s*/i.test(text)
    const looksLikeInstructions = /<user_instructions>/i.test(text) || /#\s*Repository\s+Guidelines/i.test(text)
    if (isUserish && looksLikeInstructions) return base.slice(1)
    return base
  }, [thread?.items])

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ title: '', headerBackTitle: '' }} />
      {loading && items.length === 0 ? (
        <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
          <ActivityIndicator color={Colors.secondary} />
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginTop: 8 }}>Loading…</Text>
        </View>
      ) : (
        <FlatList
          data={items}
          keyExtractor={(_, i) => String(i)}
          contentContainerStyle={{ padding: 12, gap: 10, paddingBottom: 80 }}
          renderItem={({ item }) => <Row it={item} />}
          ListFooterComponent={id ? (
            <View style={{ marginTop: 12 }}>
              <Pressable
                onPress={() => { setResumeNextId(id); router.push('/thread') }}
                style={{ alignSelf: 'flex-start', backgroundColor: Colors.card, borderColor: Colors.border, borderWidth: 1, paddingHorizontal: 10, paddingVertical: 8 }}
                accessibilityRole="button"
              >
                <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Continue chat</Text>
              </Pressable>
            </View>
          ) : null}
        />
      )}
    </View>
  )
}

function Row({ it }: { it: { ts: number; kind: 'message'|'reason'|'cmd'; role?: 'assistant'|'user'; text: string } }) {
  if (it.kind === 'message' && it.role === 'assistant') {
    return (
      <View style={{ borderColor: Colors.border, borderWidth: 1, backgroundColor: Colors.card, padding: 8 }}>
        <MarkdownBlock markdown={it.text} />
      </View>
    )
  }
  if (it.kind === 'message' && it.role === 'user') {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>You: {it.text}</Text>
  }
  if (it.kind === 'reason') {
    return (
      <View style={{ paddingVertical: 2 }}>
        <ReasoningHeadline text={it.text} />
      </View>
    )
  }
  if (it.kind === 'cmd') {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
  }
  return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
}
