import { createFileRoute } from '@tanstack/react-router'

import { PrivacyPage } from './-privacy-page'

export const Route = createFileRoute('/privacy')({
  component: PrivacyPage,
  head: () => ({
    meta: [
      { title: 'Privacy Policy - OpenAgents' },
      {
        name: 'description',
        content: 'OpenAgents Privacy Policy.',
      },
    ],
  }),
})
