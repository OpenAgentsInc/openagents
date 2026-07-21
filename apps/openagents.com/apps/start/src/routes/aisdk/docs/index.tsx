import { createFileRoute, notFound } from '@tanstack/react-router'

import { aisdkDocsHead } from '../../../aisdk/aisdk-head'
import { loadAisdkDocsPage } from '../../../aisdk/generated/aisdk-manifest.generated'
import { AisdkDocsNotFound, AisdkDocsPageView } from '../../-aisdk-docs-page'

/** AI SDK docs overview — docs/ai-sdk/README.md (owner-directed, 2026-07-21). */
export const Route = createFileRoute('/aisdk/docs/')({
  component: AisdkDocsIndexRoute,
  head: ({ loaderData }) => aisdkDocsHead(loaderData),
  loader: () => {
    const page = loadAisdkDocsPage('')
    if (page === undefined) {
      throw notFound()
    }
    return page
  },
  notFoundComponent: AisdkDocsNotFound,
})

function AisdkDocsIndexRoute() {
  return <AisdkDocsPageView page={Route.useLoaderData()} />
}
