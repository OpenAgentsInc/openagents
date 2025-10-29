import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdateAgentThoughtChunk } from '@/components/acp'

export default function AcpThoughtDemo() {
  useHeaderTitle('ACP: Agent Thought')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <View>
        <SessionUpdateAgentThoughtChunk content={{ type: 'text', text: '_Thinking_ about how to structure this.' }} />
      </View>
    </ScrollView>
  )
}

