import { createFileRoute } from '@tanstack/react-router'

import { MirrorCodePage } from './-mirrorcode-page'

export const Route = createFileRoute('/mirrorcode')({
  component: MirrorCodePage,
  head: () => ({
    meta: [
      { title: 'MirrorCode, powered by Khala - OpenAgents' },
      {
        name: 'description',
        content:
          'Khala reimplements real tools from scratch, then a held-out public test suite scores the result — the Epoch Research MirrorCode benchmark, public tasks only.',
      },
    ],
  }),
})
