import React from 'react'
import { View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import type { AgentMessageItem } from '@/types/exec-jsonl'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'

export function AgentMessageCard({ item }: { item: AgentMessageItem }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12 }}>
      <MarkdownBlock markdown={item.text} />
    </View>
  )
}
