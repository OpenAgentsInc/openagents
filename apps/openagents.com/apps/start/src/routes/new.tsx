import { createFileRoute } from '@tanstack/react-router'

import { LandingPage } from './app'

export const Route = createFileRoute('/new')({
  component: LandingPage,
  head: () => ({
    meta: [
      { title: 'Launch UI Replica' },
      {
        name: 'description',
        content:
          'Phase-1 Launch UI replica running on the OpenAgents TanStack Start app.',
      },
    ],
  }),
})
