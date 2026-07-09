import { createFileRoute } from '@tanstack/react-router'

import { KhalaEffectNativePage } from '../-khala-effect-native-page'

export const Route = createFileRoute('/khala/')({
  component: KhalaEffectNativePage,
  head: () => ({
    meta: [
      { title: 'Khala - OpenAgents' },
      {
        name: 'description',
        content:
          'Khala is the OpenAgents inference and work rail, with OpenAI-compatible API basics and public receipt discipline.',
      },
    ],
  }),
})
