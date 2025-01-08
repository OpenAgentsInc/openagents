import { NDKEvent, NDKSubscription } from '@nostr-dev-kit/ndk'

export interface NostrChatConfig {
  defaultRelays?: string[]
  messageTemplate?: string
  autoScroll?: boolean
  moderationEnabled?: boolean
  pollInterval?: number
  messageLimit?: number
}

export interface ChannelMetadata {
  name: string
  about?: string
  picture?: string
  relays?: string[]
}

export interface ChatMessage {
  id: string
  pubkey: string
  content: string
  created_at: number
  tags: string[][]
  isReply?: boolean
  replyTo?: string
}

export interface ModerationAction {
  type: 'hide' | 'mute'
  target: string // message id or pubkey
  reason?: string
  timestamp: number
}

export interface ChatState {
  channelId?: string
  subscription?: NDKSubscription
  messages: Map<string, NDKEvent>
  hiddenMessages: Set<string>
  mutedUsers: Set<string>
  moderationActions: ModerationAction[]
  lastFetch?: number
}