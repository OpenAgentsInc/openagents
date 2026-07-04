import { createFileRoute } from '@tanstack/react-router'

import { DownloadPage } from './-download-page'

export const Route = createFileRoute('/download')({
  component: DownloadPage,
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
