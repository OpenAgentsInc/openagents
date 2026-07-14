import { createFileRoute } from '@tanstack/react-router'

import { SiteCheckoutDemoPage } from '../-site-checkout-demo-page'

export const Route = createFileRoute('/sites/demo-checkout')({
  component: SiteCheckoutDemoPage,
  head: () => ({
    meta: [
      { title: 'Retired capability - OpenAgents' },
      {
        name: 'description',
        content:
          'Sites and money capabilities are retired from the Codex Workroom MVP.',
      },
    ],
  }),
})
