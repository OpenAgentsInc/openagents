import { createFileRoute, Outlet } from '@tanstack/react-router'

import { readSwapSurfaceStatus } from '@/features/swap/gate-server-fn'
import { SwapSurfaceGateClosed } from '@/features/swap/shell'

/**
 * The swap product surface layout (SWAP-7, #9322). The serving gate is read
 * server-side on every document request and fails closed; see
 * `features/swap/gate.ts` for the gate contract and the SWAP-0 (#9315)
 * route-naming caveat. Children render only when the deployment explicitly
 * enables the surface.
 */
export const Route = createFileRoute('/swap')({
  loader: () => readSwapSurfaceStatus(),
  component: SwapLayout,
  head: () => ({
    meta: [
      { title: 'Swap - OpenAgents' },
      {
        name: 'description',
        content:
          'Self-custodial atomic swaps negotiated on open relays. Keys stay in your browser.',
      },
    ],
  }),
})

function SwapLayout() {
  const status = Route.useLoaderData()
  if (!status.enabled) return <SwapSurfaceGateClosed />
  return <Outlet />
}
