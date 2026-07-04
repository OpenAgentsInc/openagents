import { createFileRoute } from '@tanstack/react-router'

import { SiteCheckoutDemoPage } from '../-site-checkout-demo-page'

export const Route = createFileRoute('/sites/demo-checkout')({
  component: SiteCheckoutDemoPage,
  head: () => ({
    meta: [
      { title: 'Demo checkout - OpenAgents' },
      {
        name: 'description',
        content:
          'Start a demo checkout for an Omega Site product and inspect the clean return status.',
      },
    ],
  }),
})
