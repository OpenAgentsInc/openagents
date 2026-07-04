import { createFileRoute } from '@tanstack/react-router'

import { LandingPreviewPage } from '../-landing-preview-page'

export const Route = createFileRoute('/preview/landing')({
  component: LandingPreviewPage,
  head: () => ({
    meta: [
      { title: 'Landing preview - OpenAgents' },
      {
        name: 'description',
        content:
          'Review-only OpenAgents landing page candidate rendered through the TanStack Start staging app.',
      },
    ],
  }),
})
