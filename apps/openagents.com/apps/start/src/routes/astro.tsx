import { createFileRoute } from '@tanstack/react-router'

import { DesktopLandingPage } from './-public-site'

const description =
  'A local-first desktop workroom for durable, reviewable Codex work.'

export const Route = createFileRoute('/astro')({
  component: DesktopLandingPage,
  head: () => ({
    meta: [
      { title: 'OpenAgents Desktop' },
      { name: 'description', content: description },
      { name: 'theme-color', content: '#05070d' },
      { property: 'og:title', content: 'OpenAgents Desktop' },
      { property: 'og:description', content: description },
      { property: 'og:url', content: 'https://openagents.com/astro' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/astro' }],
  }),
})
