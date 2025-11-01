import React from 'react'
import { View } from 'react-native'
import { Colors } from '@/constants/theme'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'
import type { ContentBlock } from '@/types/acp'

export function SessionUpdateAgentThoughtChunk({ content }: { content: ContentBlock }) {
  if (content.type !== 'text') return null
  return (
    <View testID="agent-thought" style={{ borderLeftWidth: 2, borderLeftColor: Colors.quaternary, paddingLeft: 8 }}>
      <MarkdownBlock markdown={content.text} />
    </View>
  )
}
