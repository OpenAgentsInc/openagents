import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function SessionUpdateCurrentModeUpdate({ currentModeId }: { currentModeId: string }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Mode switched</Text>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{currentModeId}</Text>
    </View>
  )
}
