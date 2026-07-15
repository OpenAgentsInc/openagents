import { createFileRoute, notFound, redirect } from '@tanstack/react-router'

import { docsHead } from '../../docs/docs-head'
import { docsCompatibilityRedirects } from '../../docs/docs-navigation'
import { loadDocsPage } from '../../docs/generated/docs-manifest.generated'
import { DocsPage } from '../../docs/DocsPage'

export const Route = createFileRoute('/docs/$')({
  beforeLoad: ({ params }) => {
    const slug = params._splat
    if (slug === undefined) {
      return
    }
    const target = docsCompatibilityRedirects[slug]
    if (target !== undefined) {
      throw redirect({ href: target, replace: true, statusCode: 301 })
    }
  },
  component: DocsSplatRoute,
  head: ({ loaderData }) => docsHead(loaderData),
  loader: async ({ params }) => {
    const slug = params._splat
    if (slug === undefined) {
      throw notFound()
    }
    const page = await loadDocsPage(slug)
    if (page === undefined) {
      throw notFound()
    }
    return page
  },
})

function DocsSplatRoute() {
  return <DocsPage page={Route.useLoaderData()} />
}
