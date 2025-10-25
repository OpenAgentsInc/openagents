import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { ReasoningCard } from '@/components/jsonl/ReasoningCard'

export default function ReasoningCardScreen() {
  useHeaderTitle('ReasoningCard')
  const reasoning = `**Plan**\n\n- Parse lines\n- Render JSONL rows\n- Highlight code`
  const item = { type: 'reasoning' as const, text: reasoning }
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <ReasoningCard item={item} />
    </ScrollView>
  )
}

