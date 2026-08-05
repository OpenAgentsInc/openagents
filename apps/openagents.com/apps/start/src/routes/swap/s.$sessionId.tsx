import { createFileRoute } from '@tanstack/react-router'

import { SwapHistoryPage } from '@/features/swap/history-page'

/**
 * Deep link that resumes one stored session (#9322). The id resolves
 * against the local session store once SWAP-5 (#9320) lands; until then the
 * page states that honestly.
 */
export const Route = createFileRoute('/swap/s/$sessionId')({
  component: SwapSessionRoute,
  head: () => ({
    meta: [{ title: 'History - OpenAgents Swap' }],
  }),
})

function SwapSessionRoute() {
  const { sessionId } = Route.useParams()
  return <SwapHistoryPage sessionId={sessionId} />
}
