import { createFileRoute, notFound } from '@tanstack/react-router'

import { docsHead } from '../../docs/docs-head'
import { loadDocsPage } from '../../docs/generated/docs-manifest.generated'
import { DocsPage } from '../../docs/DocsPage'

export const Route = createFileRoute('/docs/')({
  component: DocsIndexRoute,
  head: ({ loaderData }) => docsHead(loaderData),
  loader: async () => {
    const page = await loadDocsPage('')
    if (page === undefined) {
      throw notFound()
    }
    return page
  },
})

function DocsIndexRoute() {
  return <DocsPage page={Route.useLoaderData()} />
}
