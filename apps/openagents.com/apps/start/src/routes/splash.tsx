import { createFileRoute } from '@tanstack/react-router'

import { SplashPage } from './-splash-page'

const description =
  'Plan, delegate, review, and steer coding work in one local-first OpenAgents Desktop workroom.'

export const Route = createFileRoute('/splash')({
  component: SplashPage,
  head: () => ({
    meta: [
      { title: 'OpenAgents Desktop — Build with agents' },
      { name: 'description', content: description },
      { name: 'theme-color', content: '#16161e' },
      { property: 'og:title', content: 'OpenAgents Desktop — Build with agents' },
      { property: 'og:description', content: description },
      { property: 'og:url', content: 'https://openagents.com/splash' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/splash' }],
  }),
})
