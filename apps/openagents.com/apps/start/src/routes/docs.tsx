import { createFileRoute } from '@tanstack/react-router'

import { DocsIndexPage } from './-funnel-components'

export const Route = createFileRoute('/docs')({
  component: DocsIndexPage,
  head: () => ({
    meta: [
      { title: 'Docs - OpenAgents' },
      {
        name: 'description',
        content:
          'Public documentation for Khala Code, OpenAgents, product promises, forum participation, and developer API surfaces.',
      },
    ],
  }),
})
