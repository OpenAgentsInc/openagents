import { createFileRoute } from '@tanstack/react-router'

import { CodePage } from '../-code-page'

export const Route = createFileRoute('/code/')({
  component: CodePage,
  head: () => ({
    meta: [
      { title: 'Khala Code - OpenAgents' },
      {
        name: 'description',
        content:
          'Khala Code is the OpenAgents coding app around your own local Codex install, rendered through the TanStack Start staging app.',
      },
    ],
  }),
})
