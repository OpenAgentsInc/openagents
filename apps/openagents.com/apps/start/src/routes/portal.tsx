import { createFileRoute } from '@tanstack/react-router'

import { PortalPage } from './-portal-page'

export const Route = createFileRoute('/portal')({
  component: PortalPage,
  head: () => ({
    meta: [
      { title: 'Client portal — OpenAgents' },
      {
        name: 'description',
        content:
          'Your OpenAgents engagement at a glance: funnel status, content calendar, and approval queue. Private to your account.',
      },
    ],
  }),
})
