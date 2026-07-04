import { createFileRoute } from '@tanstack/react-router'

import { RunPage } from './-run-page'

export const Route = createFileRoute('/run')({
  component: RunPage,
  head: () => ({
    meta: [
      { title: 'Live Tassadar run - OpenAgents' },
      {
        name: 'description',
        content:
          'Retired web Tassadar run pointer rendered through the TanStack Start staging app.',
      },
    ],
  }),
})
