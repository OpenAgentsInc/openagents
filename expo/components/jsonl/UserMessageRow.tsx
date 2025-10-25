import React from 'react'
import { Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function UserMessageRow({ text, numberOfLines = 4 }: { text: string; numberOfLines?: number }) {
  return (
    <Text numberOfLines={numberOfLines} style={{ color: Colors.foreground, fontFamily: Typography.primary }}>
      {String(text || '')}
    </Text>
  )
}

