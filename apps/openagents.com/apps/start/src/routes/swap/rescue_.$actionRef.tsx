import { createFileRoute } from '@tanstack/react-router'

import { SwapRescuePage } from '@/features/swap/rescue-page'

/**
 * Deep link addressing one specific recovery action (#9322). The ref
 * resolves against locally persisted exit packages once SWAP-4 (#9319)
 * lands; until then the page states that honestly.
 */
export const Route = createFileRoute('/swap/rescue_/$actionRef')({
  component: SwapRescueActionRoute,
  head: () => ({
    meta: [{ title: 'Rescue - OpenAgents Swap' }],
  }),
})

function SwapRescueActionRoute() {
  const { actionRef } = Route.useParams()
  return <SwapRescuePage actionRef={actionRef} />
}
