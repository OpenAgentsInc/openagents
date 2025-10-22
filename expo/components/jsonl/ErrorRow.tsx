import React from 'react'
import { Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ErrorRow({ message }: { message: string }) {
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: '#FCA5A5', fontFamily: Typography.bold }}>Error</Text>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>{message}</Text>
    </View>
  )
}

