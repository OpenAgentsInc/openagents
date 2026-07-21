import { createFileRoute, notFound } from '@tanstack/react-router'

import { aisdkDocsHead } from '../../../aisdk/aisdk-head'
import { loadAisdkDocsPage } from '../../../aisdk/generated/aisdk-manifest.generated'
import { AisdkDocsNotFound, AisdkDocsPageView } from '../../-aisdk-docs-page'

/**
 * AI SDK docs slug pages — compiled from docs/ai-sdk (owner-directed,
 * 2026-07-21). An unknown slug is a 404, never a soft fallback.
 */
export const Route = createFileRoute('/aisdk/docs/$slug')({
  component: AisdkDocsSlugRoute,
  head: ({ loaderData }) => aisdkDocsHead(loaderData),
  loader: ({ params }) => {
    const page = params.slug === '' ? undefined : loadAisdkDocsPage(params.slug)
    if (page === undefined) {
      throw notFound()
    }
    return page
  },
  notFoundComponent: AisdkDocsNotFound,
})

function AisdkDocsSlugRoute() {
  return <AisdkDocsPageView page={Route.useLoaderData()} />
}
