import React from 'react'
import { View } from 'react-native'
import { ContentText } from './ContentText'
import type { ContentBlock } from '@/types/acp'

export function SessionUpdateAgentMessageChunk({ content }: { content: ContentBlock }) {
  if (content.type === 'text') {
    return (
      <View testID="agent-message" accessibilityLabel="Agent message">
        <ContentText text={content.text} />
      </View>
    )
  }
  // Fallback: render nothing for non-text agent chunks for now
  return null
}
