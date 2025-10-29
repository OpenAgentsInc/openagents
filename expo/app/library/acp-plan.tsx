import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdatePlan } from '@/components/acp'

export default function AcpPlanDemo() {
  useHeaderTitle('ACP: Plan')
  const entries = [
    { content: 'Parse repository', priority: 'high', status: 'completed' },
    { content: 'Search for vulnerabilities', priority: 'medium', status: 'in_progress' },
    { content: 'Write fixes', priority: 'medium', status: 'pending' },
  ] as const
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <View>
        <SessionUpdatePlan entries={entries as any} />
      </View>
    </ScrollView>
  )
}

