import React from 'react'
import { useLocalSearchParams, useRouter, Stack } from 'expo-router'
import { ActivityIndicator, FlatList, Pressable, Text, View } from 'react-native'
import { useThreads } from '@/lib/threads-store'
import { useBridge } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'

export default function ThreadHistoryView() {
  const { id, path } = useLocalSearchParams<{ id: string; path?: string }>()
  const { requestThread, setResumeNextId } = useBridge()
  const router = useRouter()
  const loadThread = useThreads((s) => s.loadThread)
  const thread = useThreads((s) => (id ? s.thread[id] : undefined))
  const loadingMap = useThreads((s) => s.loadingThread)
  const loading = Boolean(id && loadingMap[id])

  React.useEffect(() => { if (id) loadThread((id2, p) => requestThread(id2, p), id, typeof path === 'string' ? path : undefined).catch(()=>{}) }, [id, path, requestThread, loadThread])
  useHeaderTitle(thread?.title || 'Thread')

  const items = React.useMemo(() => {
    const base = (thread?.items || []).slice()
    if (base.length === 0) return base
    // Drop leading instruction dump or preface blocks
    const first = base[0]
    const text = String(first?.text ?? '')
    const isUserish = first?.role === 'user' || /^\s*>/.test(text) || /^You:\s*/i.test(text)
    const looksLikeInstructions = /<user_instructions>/i.test(text) || /#\s*Repository\s+Guidelines/i.test(text)
    const looksLikeEnvPreface = /<environment_context>/i.test(text) || /\bYou are a coding agent running\b/i.test(text) || /\bEnvironment:\b/.test(text)
    let arr = base
    if (isUserish && (looksLikeInstructions || looksLikeEnvPreface)) arr = base.slice(1)
    // Deduplicate consecutive identical entries after basic sanitization
    const out: typeof arr = []
    for (const it of arr) {
      const prev = out[out.length - 1]
      if (
        prev &&
        prev.kind === it.kind &&
        ((prev.role ?? '') === (it.role ?? '')) &&
        String(prev.text).trim() === String(it.text).trim()
      ) {
        continue
      }
      out.push(it)
    }
    return out
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

function sanitizeUserText(s: string): string {
  let text = String(s || '')
  // Remove <environment_context>...</environment_context> blocks if present
  text = text.replace(/<environment_context>[\s\S]*?<\/environment_context>/gi, '').trim()
  // If there is a clear preface separated by blank line(s), prefer the trailing segment
  const parts = text.split(/\n\n+/)
  if (parts.length > 1) {
    text = parts[parts.length - 1].trim()
  }
  return text
}

function Row({ it }: { it: { ts: number; kind: 'message'|'reason'|'cmd'; role?: 'assistant'|'user'; text: string } }) {
  if (it.kind === 'message' && it.role === 'assistant') {
    return <MarkdownBlock markdown={it.text} />
  }
  if (it.kind === 'message' && it.role === 'user') {
    const text = sanitizeUserText(it.text)
    return (
      <Text style={{ fontSize: 12, lineHeight: 16, color: Colors.foreground, fontFamily: Typography.primary, marginTop: 6, marginBottom: 8 }}>
        {`> ${text}`}
      </Text>
    )
  }
  if (it.kind === 'reason') {
    return <ReasoningHeadline text={it.text} />
  }
  if (it.kind === 'cmd') {
    return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
  }
  return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
}
