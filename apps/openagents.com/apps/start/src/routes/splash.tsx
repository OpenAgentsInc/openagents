import { createFileRoute } from '@tanstack/react-router'

import { SplashPage } from './-splash-page'

const description =
  'A live OpenAgents Desktop workroom reconstruction rendered with real interactive components.'

export const Route = createFileRoute('/splash')({
  component: SplashPage,
  head: () => ({
    meta: [
      { title: 'OpenAgents Desktop — Live Workroom' },
      { name: 'description', content: description },
      { name: 'theme-color', content: '#05070d' },
      { property: 'og:title', content: 'OpenAgents Desktop — Live Workroom' },
      { property: 'og:description', content: description },
      { property: 'og:url', content: 'https://openagents.com/splash' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/splash' }],
  }),
})
