import { createFileRoute } from '@tanstack/react-router'

import { SplashPage, splashPageDescription } from './-splash-page'

export const Route = createFileRoute('/')({
  component: SplashPage,
  head: () => ({
    meta: [
      { title: 'Omega — OpenAgents' },
      { name: 'description', content: splashPageDescription },
      { name: 'theme-color', content: '#05070d' },
      { property: 'og:title', content: 'Omega — OpenAgents' },
      { property: 'og:description', content: splashPageDescription },
      { property: 'og:url', content: 'https://openagents.com/' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/' }],
  }),
})
