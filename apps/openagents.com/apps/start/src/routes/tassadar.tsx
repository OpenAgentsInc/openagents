import { createFileRoute } from '@tanstack/react-router'

import { TassadarEffectNativePage } from './-tassadar-effect-native-page'

export const Route = createFileRoute('/tassadar')({
  component: TassadarEffectNativePage,
  head: () => ({
    meta: [
      { title: 'Tassadar - OpenAgents' },
      {
        name: 'description',
        content:
          'Tassadar is OpenAgents open distributed AI model training run with replay verification and public receipts.',
      },
    ],
  }),
})
