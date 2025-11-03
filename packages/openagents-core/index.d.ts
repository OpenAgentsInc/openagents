import * as React from 'react'

export interface ThreadListItemProps {
  title: string
  meta?: React.ReactNode
  timestamp?: number | null
  count?: number | null
  onPress?: () => void
  onLongPress?: () => void
  testID?: string
}

export const ThreadListItem: React.FC<ThreadListItemProps>

export interface ChatMessageBubbleProps {
  role: 'assistant' | 'user'
  text: string
}

export const ChatMessageBubble: React.FC<ChatMessageBubbleProps>

