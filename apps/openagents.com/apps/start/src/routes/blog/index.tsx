import { createFileRoute } from '@tanstack/react-router'

import { BlogIndexPage } from '../-funnel-components'

export const Route = createFileRoute('/blog/')({
  component: BlogIndexPage,
  head: () => ({
    meta: [
      { title: 'Blog - OpenAgents' },
      {
        name: 'description',
        content: 'Build notes and launch notes from the OpenAgents network.',
      },
    ],
  }),
})
