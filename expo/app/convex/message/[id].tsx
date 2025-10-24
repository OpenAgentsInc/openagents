import React from 'react'
import { useLocalSearchParams } from 'expo-router'
import { useQuery } from 'convex/react'
import { ScrollView, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'

export default function ConvexMessageDetail() {
  const { id } = useLocalSearchParams<{ id: string }>()
  const msg = (useQuery as any)('messages:byId', { id }) as any
  useHeaderTitle('Message')
  const title = React.useMemo(() => {
    if (!msg) return 'Message'
    const ts = typeof msg?.ts === 'number' ? new Date(msg.ts).toLocaleString() : ''
    return `${msg?.role || 'message'} ${ts ? `· ${ts}` : ''}`
  }, [msg])
  useHeaderTitle(title)
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      {!msg ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Loading…</Text>
      ) : (
        <View style={{ gap: 8 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{new Date(msg.ts).toLocaleString()} · {msg.role}</Text>
          {msg.role === 'assistant' ? (
            <MarkdownBlock markdown={String(msg.text || '')} />
          ) : (
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{String(msg.text || '')}</Text>
          )}
        </View>
      )}
    </ScrollView>
  )
}

