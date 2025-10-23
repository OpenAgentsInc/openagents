import React from 'react'
import { Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ErrorRow({ message }: { message: string }) {
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: Colors.danger, fontFamily: Typography.bold }}>Error</Text>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{message}</Text>
    </View>
  )
}
