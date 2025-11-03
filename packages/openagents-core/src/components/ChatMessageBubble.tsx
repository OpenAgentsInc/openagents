import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@openagentsinc/theme'
import { Typography } from '@openagentsinc/theme'

export type ChatMessageBubbleProps = {
  role: 'assistant' | 'user'
  text: string
}

export function ChatMessageBubble({ role, text }: ChatMessageBubbleProps) {
  const isAssistant = role === 'assistant'
  const label = isAssistant ? 'assistant' : 'you'
  return (
    <View style={{ display: 'flex', alignItems: isAssistant ? 'flex-start' : 'flex-end', paddingVertical: 6 }}>
      <View
        style={{
          maxWidth: 680,
          paddingVertical: 10,
          paddingHorizontal: 12,
          borderRadius: 8,
          backgroundColor: isAssistant ? '#121317' : '#1b1d22',
          borderWidth: 1,
          borderColor: Colors.border,
        }}
      >
        <Text style={{ fontSize: 12, color: Colors.tertiary, marginBottom: 4, fontFamily: Typography.primary }}>{label}</Text>
        <Text style={{ fontFamily: Typography.primary, color: Colors.foreground, lineHeight: 18 }}>
          {text}
        </Text>
      </View>
    </View>
  )
}

