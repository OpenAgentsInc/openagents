import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function WebSearchRow({ query }: { query: string }) {
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
        WebSearch <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>
          {query}
        </Text>
      </Text>
    </View>
  )
}
