import { createFileRoute } from '@tanstack/react-router'

import { DownloadPage } from './-public-site'

export const Route = createFileRoute('/download')({
  component: DownloadPage,
  head: () => ({
    meta: [
      { title: 'Download OpenAgents for Mac' },
      {
        name: 'description',
        content:
          'Download the latest OpenAgents Desktop release candidate for Apple-silicon Macs.',
      },
      { name: 'theme-color', content: '#05070d' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/download' }],
  }),
})
