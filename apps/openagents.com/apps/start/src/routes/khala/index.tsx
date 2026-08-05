import { createFileRoute } from '@tanstack/react-router'

import { KhalaPage } from '../-khala-page'

export const Route = createFileRoute('/khala/')({
  component: KhalaPage,
  head: () => ({
    meta: [
      { title: 'Khala - OpenAgents' },
      {
        name: 'description',
        content:
          'Khala is the OpenAgents inference and work rail, with OpenAI-compatible API basics and public receipt discipline.',
      },
    ],
  }),
})
