import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ToolCallContentTerminal({ terminalId }: { terminalId: string }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 8 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Terminal embed: {terminalId}</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Live output rendering TBD</Text>
    </View>
  )
}

