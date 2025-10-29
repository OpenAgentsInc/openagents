import React from 'react'
import type { ContentBlock } from '@/types/acp'
import { UserMessageRow } from '@/components/jsonl/UserMessageRow'

export function SessionUpdateUserMessageChunk({ content }: { content: ContentBlock }) {
  if (content.type === 'text') {
    return <UserMessageRow text={content.text} />
  }
  return null
}

