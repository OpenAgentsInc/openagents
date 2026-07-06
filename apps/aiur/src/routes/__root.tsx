import {
  HeadContent,
  Scripts,
  createRootRouteWithContext,
} from '@tanstack/react-router'
import type { QueryClient } from '@tanstack/react-query'
import type * as React from 'react'

import '../styles.css'

const description =
  'Aiur — the owner-only OpenAgents admin panel for Khala Code mobile: credits, users, and cloud-execution ops.'

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient
}>()({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'Aiur' },
      { name: 'description', content: description },
      { name: 'robots', content: 'noindex, nofollow' },
      { name: 'theme-color', content: '#000000' },
    ],
    links: [{ rel: 'icon', type: 'image/svg+xml', sizes: 'any', href: '/icon.svg' }],
  }),
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="scheme-only-dark antialiased">
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
