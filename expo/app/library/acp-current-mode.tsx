import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdateCurrentModeUpdate } from '@/components/acp'

export default function AcpCurrentModeDemo() {
  useHeaderTitle('ACP: Current Mode')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <View>
        <SessionUpdateCurrentModeUpdate currentModeId={'review'} />
      </View>
    </ScrollView>
  )
}

