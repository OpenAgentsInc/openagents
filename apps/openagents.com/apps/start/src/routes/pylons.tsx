import { createFileRoute } from '@tanstack/react-router'

import { PylonsPage } from './-pylons-page'

export const Route = createFileRoute('/pylons')({
  component: PylonsPage,
  head: () => ({
    meta: [
      { title: 'Pylon - OpenAgents' },
      {
        content:
          'Run a Pylon node and join the OpenAgents network — live pylons online, work-ready capacity, and sats settled.',
        name: 'description',
      },
    ],
  }),
})
