import { createFileRoute } from '@tanstack/react-router'

import { DesktopLandingPage } from './-public-site'

const description =
  'A local-first desktop workroom for durable, reviewable Codex work.'

// The Electron OpenAgents Desktop app is retired; Omega is the OpenAgents
// desktop application, so this landing page names Omega rather than the
// removed product.
export const Route = createFileRoute('/astro')({
  component: DesktopLandingPage,
  head: () => ({
    meta: [
      { title: 'Omega — the OpenAgents desktop workroom' },
      { name: 'description', content: description },
      { name: 'theme-color', content: '#05070d' },
      { property: 'og:title', content: 'Omega — the OpenAgents desktop workroom' },
      { property: 'og:description', content: description },
      { property: 'og:url', content: 'https://openagents.com/astro' },
    ],
    links: [{ rel: 'canonical', href: 'https://openagents.com/astro' }],
  }),
})
