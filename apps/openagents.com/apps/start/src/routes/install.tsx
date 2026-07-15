import { createFileRoute } from '@tanstack/react-router'

import { InstallPage } from './-public-site'

export const Route = createFileRoute('/install')({
  component: InstallPage,
  head: () => ({
    meta: [
      { title: 'Install OpenAgents for Mac' },
      {
        name: 'description',
        content:
          'Download the latest OpenAgents Desktop release candidate for Apple-silicon Macs.',
      },
      { name: 'theme-color', content: '#05070d' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/install' }],
  }),
})
