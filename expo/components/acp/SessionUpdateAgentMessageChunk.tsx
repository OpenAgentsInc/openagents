import React from 'react'
import { ContentText } from './ContentText'
import type { ContentBlock } from '@/types/acp'

export function SessionUpdateAgentMessageChunk({ content }: { content: ContentBlock }) {
  if (content.type === 'text') {
    return <ContentText text={content.text} />
  }
  // Fallback: render nothing for non-text agent chunks for now
  return null
}

