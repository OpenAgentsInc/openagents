import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdateAvailableCommandsUpdate } from '@/components/acp'

export default function AcpAvailableCommandsDemo() {
  useHeaderTitle('ACP: Available Commands')
  const cmds = [
    { name: 'create_plan', description: 'Draft a plan of action' },
    { name: 'search_repo', description: 'Search across repository files' },
  ]
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <View>
        <SessionUpdateAvailableCommandsUpdate available_commands={cmds} />
      </View>
    </ScrollView>
  )
}

