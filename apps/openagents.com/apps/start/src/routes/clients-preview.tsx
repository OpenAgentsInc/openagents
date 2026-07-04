import { createFileRoute } from '@tanstack/react-router'

import { ClientsPreviewPage } from './-clients-preview-page'

export const Route = createFileRoute('/clients-preview')({
  component: ClientsPreviewPage,
  head: () => ({
    meta: [
      { title: 'Clients preview - OpenAgents' },
      {
        name: 'description',
        content:
          'Autopilot client protocol fixtures rendered through the TanStack Start staging app.',
      },
    ],
  }),
})
