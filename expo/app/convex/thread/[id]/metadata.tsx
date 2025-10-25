import React from 'react'
import { ScrollView, View, Text } from 'react-native'
import { useLocalSearchParams } from 'expo-router'
import { useQuery } from 'convex/react'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'
import { CodeBlock } from '@/components/code-block'

function classify(t: string): 'instructions' | 'environment' | 'config' | 'other' {
  if (/^\s*<user_instructions>/i.test(t) || /^\s*#\s*Repository\s+Guidelines/i.test(t)) return 'instructions'
  if (/<environment_context>/i.test(t)) return 'environment'
  if (/"sandbox"\s*:\s*"|"approval"\s*:\s*"/i.test(t)) return 'config'
  return 'other'
}

export default function ThreadMetadata() {
  const { id } = useLocalSearchParams<{ id: string }>()
  const thread = (useQuery as any)('threads:byId', { id }) as any
  const messages = (useQuery as any)('messages:forThread', { threadId: thread?.threadId || '', limit: 200 }) as any[] | undefined | null
  const metas = React.useMemo(() => {
    const arr: any[] = Array.isArray(messages) ? messages : []
    const out: { kind: string; text: string }[] = []
    for (let i = 0; i < Math.min(arr.length, 5); i++) {
      const m = arr[i];
      if (!m) break
      if ((m.kind || (m.role ? 'message' : '')) !== 'message') break
      if ((m.role || '').toLowerCase() !== 'user') break
      const t = String(m.text || '')
      const k = classify(t)
      if (k === 'other') break
      out.push({ kind: k, text: t })
    }
    return out
  }, [messages])

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18 }}>Conversation Metadata</Text>
      {metas.length === 0 ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No metadata detected for this thread.</Text>
      ) : metas.map((m, i) => (
        <View key={i} style={{ borderWidth: 1, borderColor: Colors.border, padding: 10, backgroundColor: Colors.card }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginBottom: 6 }}>
            {m.kind === 'instructions' ? 'User Instructions' : m.kind === 'environment' ? 'Environment Context' : 'Preface Config'}
          </Text>
          {m.kind === 'instructions' ? (
            <MarkdownBlock markdown={m.text} />
          ) : m.kind === 'environment' ? (
            <CodeBlock code={m.text} language={'xml'} />
          ) : (
            <CodeBlock code={m.text} language={'json'} />
          )}
        </View>
      ))}
    </ScrollView>
  )
}

