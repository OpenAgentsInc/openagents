import { createFileRoute } from '@tanstack/react-router'

import { DocsLayout, DocsNotFound } from '../docs/DocsLayout'
import { docsHead } from '../docs/docs-head'

export const Route = createFileRoute('/docs')({
  component: DocsLayout,
  head: () => docsHead(undefined),
  notFoundComponent: DocsNotFound,
})
