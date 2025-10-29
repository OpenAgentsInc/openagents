import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ContentResourceLink({ name, uri, mimeType }: { name: string; uri: string; mimeType?: string | null }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 8 }}>
      <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{name}</Text>
      <Text selectable style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{uri}{mimeType ? ` (${mimeType})` : ''}</Text>
    </View>
  )
}

