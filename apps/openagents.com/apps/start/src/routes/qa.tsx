import { createFileRoute } from '@tanstack/react-router'

import { QaBoardPage } from './-qa-board-page'

export const Route = createFileRoute('/qa')({
  component: QaBoardPage,
  head: () => ({
    meta: [
      { title: 'Live QA Board - OpenAgents' },
      {
        content:
          'Live observer checks, issue-linked QA findings, and six-lane swarm evidence.',
        name: 'description',
      },
    ],
  }),
})
