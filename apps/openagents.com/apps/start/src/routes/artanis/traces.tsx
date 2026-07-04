import { createFileRoute } from '@tanstack/react-router'

import { ArtanisTracesPage } from '../-artanis-traces-page'

export const Route = createFileRoute('/artanis/traces')({
  component: ArtanisTracesPage,
  head: () => ({
    meta: [{ title: 'Artanis RLM traces - OpenAgents' }],
  }),
})
