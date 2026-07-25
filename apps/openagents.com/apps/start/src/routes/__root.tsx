import { PublicHeader } from '@/components/public-header'
import { WebAnalytics } from '@/components/web-analytics'
import {
  showsProductLogout,
  usesPersistentProductHeader,
} from '@/lib/persistent-product-header'
import type { QueryClient } from '@tanstack/react-query'
import {
  HeadContent,
  Outlet,
  Scripts,
  createRootRouteWithContext,
  useRouterState,
} from '@tanstack/react-router'
import type * as React from 'react'

import '../styles.css'

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient
}>()({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'OpenAgents' },
      { property: 'og:site_name', content: 'OpenAgents' },
      { property: 'og:type', content: 'website' },
      { name: 'twitter:card', content: 'summary' },
      { name: 'theme-color', content: '#05070d' },
    ],
    links: [
      { rel: 'icon', href: '/favicon.ico', sizes: '32x32' },
      { rel: 'icon', type: 'image/svg+xml', sizes: 'any', href: '/icon.svg' },
    ],
  }),
  component: RootLayout,
  shellComponent: RootDocument,
})

function RootLayout() {
  const pathname = useRouterState({
    select: state => state.location.pathname,
  })

  return (
    <>
      {usesPersistentProductHeader(pathname) ? (
        <PublicHeader showLogout={showsProductLogout(pathname)} />
      ) : null}
      <Outlet />
    </>
  )
}

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="dark scheme-only-dark antialiased">
      <head>
        <HeadContent />
      </head>
      <body>
        <main className="isolate">{children}</main>
        <WebAnalytics />
        <Scripts />
      </body>
    </html>
  )
}
