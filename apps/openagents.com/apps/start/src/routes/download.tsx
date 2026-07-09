import { createFileRoute } from '@tanstack/react-router'

import { DownloadEffectNativePage } from './-download-effect-native-page'

export const Route = createFileRoute('/download')({
  component: DownloadEffectNativePage,
  head: () => ({
    meta: [
      { title: 'Download Autopilot - OpenAgents' },
      {
        name: 'description',
        content:
          'Autopilot Desktop download page rendered through the TanStack Start staging app.',
      },
    ],
  }),
})
