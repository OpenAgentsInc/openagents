import type { ReactNode } from 'react'

import { PublicFooter } from './public-footer'
import { PublicHeader } from './public-header'

type PublicPageShellProps = Readonly<{
  children: ReactNode
  dataRoute: string
}>

export function PublicPageShell({ children, dataRoute }: PublicPageShellProps) {
  return (
    <div
      className="min-h-dvh bg-khala-void font-sans text-khala-text selection:bg-khala-energy selection:text-white"
      data-route={dataRoute}
    >
      <PublicHeader />
      {children}
      <PublicFooter />
    </div>
  )
}
