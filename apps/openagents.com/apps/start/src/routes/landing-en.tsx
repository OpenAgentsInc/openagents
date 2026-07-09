import { createFileRoute } from '@tanstack/react-router'

import { LandingEnPage } from './-landing-en-page'

export const Route = createFileRoute('/landing-en')({
  component: LandingEnPage,
  head: () => ({
    meta: [
      { title: 'OpenAgents — Effect Native landing (WEB-1-EN)' },
      {
        name: 'description',
        content:
          'The OpenAgents landing re-authored as one typed Effect Native view tree from the standard marketing catalog. Copy pending owner sign-off (#8565).',
      },
    ],
  }),
})
