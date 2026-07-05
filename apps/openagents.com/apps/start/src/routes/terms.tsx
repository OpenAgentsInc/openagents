import { createFileRoute } from '@tanstack/react-router'

import { TermsPage } from './-terms-page'

export const Route = createFileRoute('/terms')({
  component: TermsPage,
  head: () => ({
    meta: [
      { title: 'Terms of Service - OpenAgents' },
      {
        name: 'description',
        content: 'OpenAgents Terms of Service.',
      },
    ],
  }),
})
