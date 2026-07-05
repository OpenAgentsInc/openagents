import { createFileRoute } from '@tanstack/react-router'

import { ArtanisAccountsPage } from '../-artanis-accounts-page'

export const Route = createFileRoute('/artanis/accounts')({
  component: ArtanisAccountsPage,
  head: () => ({
    meta: [{ title: 'Operator account observability - OpenAgents' }],
  }),
})
