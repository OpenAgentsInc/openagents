import { createFileRoute } from '@tanstack/react-router'

import { DownloadPage } from './-public-site'

export const Route = createFileRoute('/download')({
  component: DownloadPage,
  head: () => ({
    meta: [
      { title: 'Download OpenAgents Desktop' },
      {
        name: 'description',
        content:
          'Download OpenAgents Desktop for Apple silicon Macs. Windows, Linux, and Intel Mac builds are coming soon.',
      },
      { name: 'theme-color', content: '#05070d' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/download' }],
  }),
})
