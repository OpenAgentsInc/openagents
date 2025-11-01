import React from 'react'
import { View } from 'react-native'
import type { ContentBlock } from '@/types/acp'
import { UserMessageRow } from '@/components/jsonl/UserMessageRow'

export function SessionUpdateUserMessageChunk({ content }: { content: ContentBlock }) {
  if (content.type === 'text') {
    return (
      <View testID="user-message" accessibilityLabel="User message">
        <UserMessageRow text={content.text} />
      </View>
    )
  }
  return null
}
