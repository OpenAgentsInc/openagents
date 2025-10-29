import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdateAgentMessageChunk } from '@/components/acp'

export default function AcpMessageDemo() {
  useHeaderTitle('ACP: Agent Message')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <View>
        <SessionUpdateAgentMessageChunk content={{ type: 'text', text: '**Hello** from ACP!\n\n- bullet\n- list' }} />
      </View>
    </ScrollView>
  )
}

