import React from 'react'
import { useLocalSearchParams } from 'expo-router'
import { ScrollView, Text, View } from 'react-native'
import { useTinyvex } from '@/providers/tinyvex'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function ThreadScreen() {
  const { id } = useLocalSearchParams<{ id: string }>()
  const threadId = String(id || '')
  const { subscribeMessages, queryMessages, messagesByThread } = useTinyvex()
  React.useEffect(() => {
    if (!threadId) return
    queryMessages(threadId, 400)
    subscribeMessages(threadId)
  }, [threadId])
  const rows = messagesByThread[threadId] || []
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 40 }}>
      {rows.length === 0 ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No messages yet.</Text>
      ) : rows.map((m) => (
        <View key={`${m.id}`} style={{ paddingVertical: 4 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{m.role || m.kind}</Text>
          {!!m.text && <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 14 }}>{m.text}</Text>}
        </View>
      ))}
    </ScrollView>
  )
}
