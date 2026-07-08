import { createFileRoute } from '@tanstack/react-router'

import { Stage1EffectNativePage } from './-stage1-effect-native-page'

export const Route = createFileRoute('/stage1')({
  component: Stage1EffectNativePage,
  head: () => ({
    meta: [
      { title: 'Stage 1 Effect Native - OpenAgents' },
      {
        name: 'description',
        content:
          'Safe, unlinked OpenAgents Effect Native landing surface for EN-1 validation. Not the live homepage.',
      },
      { name: 'robots', content: 'noindex,nofollow' },
    ],
  }),
})
