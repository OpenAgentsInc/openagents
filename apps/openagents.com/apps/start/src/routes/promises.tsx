import { createFileRoute } from '@tanstack/react-router'

import { PromisesPage } from './-promises-page'

export const Route = createFileRoute('/promises')({
  component: PromisesPage,
  head: () => ({
    meta: [
      { title: 'Product promises - OpenAgents' },
      {
        name: 'description',
        content:
          'A visual map of what OpenAgents says it does, what is live, what is gated, and what should be reported when reality does not match the claim.',
      },
    ],
  }),
})
