import { createFileRoute } from '@tanstack/react-router'

import { SwapSettingsPage } from '@/features/swap/settings-page'

export const Route = createFileRoute('/swap/settings')({
  component: SwapSettingsPage,
  head: () => ({
    meta: [{ title: 'Settings - OpenAgents Swap' }],
  }),
})
