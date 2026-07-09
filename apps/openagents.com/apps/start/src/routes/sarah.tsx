import { createFileRoute } from '@tanstack/react-router'

import { SarahHostPage } from './-sarah-page'

export const Route = createFileRoute('/sarah')({
  component: SarahHostPage,
  head: () => ({
    meta: [
      { title: 'Sarah — OpenAgents sales assistant' },
      {
        name: 'description',
        content:
          'Talk to Sarah, the OpenAgents AI sales assistant. Always disclosed as AI. Mounted at openagents.com/sarah.',
      },
      { property: 'og:url', content: 'https://openagents.com/sarah' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/sarah' }],
  }),
})
