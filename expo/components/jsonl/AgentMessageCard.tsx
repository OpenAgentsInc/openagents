import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import type { AgentMessageItem } from '@/types/exec-jsonl'

export function AgentMessageCard({ item }: { item: AgentMessageItem }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 12, padding: 12 }}>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold, marginBottom: 6 }}>agent_message</Text>
      <Text selectable style={{ color: Colors.textPrimary, fontFamily: Typography.primary, lineHeight: 18 }}>{item.text}</Text>
    </View>
  )
}

