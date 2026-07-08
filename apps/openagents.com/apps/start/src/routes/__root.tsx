import {
  HeadContent,
  Scripts,
  createRootRouteWithContext,
} from '@tanstack/react-router'
import type { QueryClient } from '@tanstack/react-query'
import type * as React from 'react'

import '../styles.css'

const description =
  'OpenAgents builds public, verifiable AI agents for coding, research, payments, and operational work.'

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient
}>()({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'OpenAgents' },
      { name: 'description', content: description },
      { property: 'og:site_name', content: 'OpenAgents' },
      { property: 'og:title', content: 'OpenAgents' },
      { property: 'og:description', content: description },
      { property: 'og:url', content: 'https://openagents.com/' },
      { property: 'og:type', content: 'website' },
      { name: 'twitter:card', content: 'summary' },
      { name: 'twitter:title', content: 'OpenAgents' },
      { name: 'twitter:description', content: description },
      { name: 'twitter:url', content: 'https://openagents.com/' },
      { name: 'theme-color', content: '#000000' },
    ],
    links: [
      { rel: 'canonical', href: 'https://openagents.com/' },
      { rel: 'icon', href: '/favicon.ico', sizes: '32x32' },
      { rel: 'icon', type: 'image/svg+xml', sizes: 'any', href: '/icon.svg' },
      { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
      {
        rel: 'preconnect',
        href: 'https://fonts.gstatic.com',
        crossOrigin: 'anonymous',
      },
      {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap',
      },
    ],
  }),
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="dark scheme-only-dark antialiased">
      <head>
        <HeadContent />
      </head>
      <body>
        <main className="isolate">{children}</main>
        <Scripts />
      </body>
    </html>
  )
}
