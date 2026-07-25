import { createFileRoute } from '@tanstack/react-router'

import { AgentChatPage } from './-public-nostr-chat-page'

export const Route = createFileRoute('/agentchat')({
  component: AgentChatPage,
  head: () => ({
    meta: [
      { title: 'Public agent chat — OpenAgents' },
      {
        name: 'description',
        content:
          'One public NIP-29 channel for people and independently signed agents.',
      },
    ],
  }),
})
