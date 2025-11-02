import React from 'react'
import { useLocalSearchParams, useRouter } from 'expo-router'
import { ScrollView, Text, View, Pressable } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useTinyvex, type MessageRow } from '@/providers/tinyvex'
import { SessionUpdateAgentMessageChunk } from '@/components/acp/SessionUpdateAgentMessageChunk'
import { SessionUpdateAgentThoughtChunk } from '@/components/acp/SessionUpdateAgentThoughtChunk'

export default function MessageDetail() {
  const { id, mid } = useLocalSearchParams<{ id: string; mid: string }>()
  const threadId = String(id || '')
  const msgId = Number(String(mid || '0'))
  const { messagesByThread } = useTinyvex()
  const router = useRouter()
  const rows = Array.isArray(messagesByThread[threadId]) ? messagesByThread[threadId] : []
  const msg = rows.find((r) => Number(r.id) === msgId)
  const fullText = String(msg?.text || '')
  const reasons = rows.filter((r) => String(r.kind || '').toLowerCase() === 'reason')
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView style={{ flex: 1 }} contentContainerStyle={{ padding: 16, gap: 16 }}>
        <Pressable onPress={() => { try { router.back() } catch {} }} accessibilityRole='button' style={{ alignSelf: 'flex-start', borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 10, paddingVertical: 6 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Back</Text>
        </Pressable>
        <View style={{ gap: 10 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Message</Text>
          <SessionUpdateAgentMessageChunk content={{ type: 'text', text: fullText }} />
        </View>
        {reasons.length > 0 ? (
          <View style={{ gap: 10 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Reasoning</Text>
            {reasons.map((r, i) => (
              <SessionUpdateAgentThoughtChunk key={`reason-${i}-${r.id}`} content={{ type: 'text', text: String(r.text || '') }} />
            ))}
          </View>
        ) : null}
      </ScrollView>
    </View>
  )
}

