import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'

export default function ReasoningHeadlineScreen() {
  useHeaderTitle('ReasoningHeadline')
  const reasoning = `**Plan**\n\n- Parse lines\n- Render JSONL rows\n- Highlight code`
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <ReasoningHeadline text={reasoning} />
    </ScrollView>
  )
}

