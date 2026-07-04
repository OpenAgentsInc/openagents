import { createFileRoute } from '@tanstack/react-router'

import { GymPage } from './-gym-page'

export const Route = createFileRoute('/gym')({
  component: GymPage,
  head: () => ({
    meta: [
      { title: 'OpenAgents Gym - OpenAgents' },
      {
        name: 'description',
        content:
          'OpenAgents Gym is the no-spend public lab for Khala policy shapes and live Terminal-Bench run visualization.',
      },
    ],
  }),
})
