import { createFileRoute, notFound } from '@tanstack/react-router'

import { DocPageView } from '../-funnel-components'
import { findDocPage, type DocPage } from '../-funnel-data'

export const Route = createFileRoute('/docs/$slug')({
  component: DocsPageRoute,
  head: ({ params }) => {
    const page = findDocPage(params.slug)
    return {
      meta: [
        { title: `${page?.title ?? 'Docs'} - OpenAgents` },
        {
          name: 'description',
          content: page?.summary ?? 'OpenAgents documentation.',
        },
      ],
    }
  },
  loader: ({ params }) => {
    const page = findDocPage(params.slug)
    if (page === undefined) {
      throw notFound()
    }
    return { page }
  },
})

function DocsPageRoute() {
  const { page } = Route.useLoaderData() as { page: DocPage }
  return <DocPageView page={page} />
}
