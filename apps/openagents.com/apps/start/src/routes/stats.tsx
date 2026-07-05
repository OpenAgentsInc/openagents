import { createFileRoute } from '@tanstack/react-router'

import { StatsPage } from './-stats-page'

export const Route = createFileRoute('/stats')({
  component: StatsPage,
  head: () => ({
    meta: [
      { title: 'Network Stats - OpenAgents' },
      {
        content:
          'Live public-safe evidence: receipt-backed counters, launch gates, and claim boundaries. No dummy values; missing evidence is marked unavailable.',
        name: 'description',
      },
    ],
  }),
})
