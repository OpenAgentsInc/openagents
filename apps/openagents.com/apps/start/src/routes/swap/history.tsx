import { createFileRoute } from '@tanstack/react-router'

import { SwapHistoryPage } from '@/features/swap/history-page'

export const Route = createFileRoute('/swap/history')({
  component: () => <SwapHistoryPage />,
  head: () => ({
    meta: [{ title: 'History - OpenAgents Swap' }],
  }),
})
