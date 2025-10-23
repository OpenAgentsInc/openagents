import React from 'react'
import { Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ThreadStartedRow({ threadId }: { threadId: string }) {
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>Thread</Text>{' '}
        started · <Text style={{ color: Colors.foreground }}>{threadId}</Text>
      </Text>
    </View>
  )
}
