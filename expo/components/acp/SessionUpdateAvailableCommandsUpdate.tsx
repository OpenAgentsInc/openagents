import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import type { AvailableCommand } from '@/types/acp'

export function SessionUpdateAvailableCommandsUpdate({ available_commands }: { available_commands: ReadonlyArray<AvailableCommand> }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12, gap: 8 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Available Commands</Text>
      <View style={{ gap: 6 }}>
        {available_commands.map((c, i) => (
          <View key={i}>
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{c.name}</Text>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{c.description}</Text>
          </View>
        ))}
      </View>
    </View>
  )
}
