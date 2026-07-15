import { createFileRoute } from '@tanstack/react-router'

import { HoldingPage } from './-public-site'

export const Route = createFileRoute('/')({
  component: HoldingPage,
  head: () => ({
    meta: [
      { title: 'OpenAgents' },
      { name: 'robots', content: 'noindex' },
      { name: 'theme-color', content: '#05070d' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/' }],
  }),
})
