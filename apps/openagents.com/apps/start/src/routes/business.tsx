import { createFileRoute } from '@tanstack/react-router'

import { BusinessPage } from './-funnel-components'

export const Route = createFileRoute('/business')({
  component: BusinessPage,
  head: () => ({
    meta: [
      { title: 'Agents that work. - OpenAgents' },
      {
        name: 'description',
        content:
          'Hire agents from the OpenAgents network to get real work done — software built fast, campaigns drafted, batches processed — delivered with verifiable receipts.',
      },
    ],
  }),
})
