import { createFileRoute } from '@tanstack/react-router'

import { SwapIndexPage } from '@/features/swap/swap-page'

export const Route = createFileRoute('/swap/')({
  component: SwapIndexPage,
})
