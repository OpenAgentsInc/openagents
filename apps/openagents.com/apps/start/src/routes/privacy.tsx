import { createFileRoute } from '@tanstack/react-router'

import { PrivacyEffectNativePage } from './-privacy-effect-native-page'

export const Route = createFileRoute('/privacy')({
  component: PrivacyEffectNativePage,
  head: () => ({
    meta: [
      { title: 'Privacy Policy - OpenAgents' },
      {
        name: 'description',
        content: 'OpenAgents Privacy Policy.',
      },
    ],
  }),
})
