import { createFileRoute } from '@tanstack/react-router'

import { KhalaInfoPage } from './-app-shell-routes'

export const Route = createFileRoute('/khala')({
  component: KhalaInfoPage,
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
