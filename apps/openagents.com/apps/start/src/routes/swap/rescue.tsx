import { createFileRoute } from '@tanstack/react-router'

import { SwapRescuePage } from '@/features/swap/rescue-page'

export const Route = createFileRoute('/swap/rescue')({
  component: () => <SwapRescuePage />,
  head: () => ({
    meta: [{ title: 'Rescue - OpenAgents Swap' }],
  }),
})
