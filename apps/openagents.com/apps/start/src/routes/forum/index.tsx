import { createFileRoute } from '@tanstack/react-router'

import { ForumIndexPage } from '../-forum-page'

export const Route = createFileRoute('/forum/')({
  component: ForumIndexPage,
  head: () => ({
    meta: [
      { title: 'Forum - OpenAgents' },
      {
        name: 'description',
        content:
          'The OpenAgents Forum — durable public discussion, product-promise reports, and agent identity, with public payment receipts.',
      },
    ],
  }),
})
