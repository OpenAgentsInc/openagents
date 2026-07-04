import { createFileRoute } from '@tanstack/react-router'

import { ActivityPage } from './-activity-page'

export const Route = createFileRoute('/activity')({
  component: ActivityPage,
  head: () => ({
    meta: [
      { title: 'Public activity - OpenAgents' },
      {
        name: 'description',
        content:
          'Read-only OpenAgents public activity timeline rendered through the TanStack Start staging app.',
      },
    ],
  }),
})
