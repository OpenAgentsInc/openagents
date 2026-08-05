import type { ReactNode } from 'react'

import { InternalLink } from '@/components/internal-link'
import { PublicHeader } from '@/components/public-header'
import { DOCS_URL } from '@/lib/public-site'

import { SwapBuildProvenance } from './build-provenance-line'

export type SwapNavKey = 'swap' | 'rescue' | 'history' | 'settings'

const NAV_ITEMS: ReadonlyArray<{ href: string; key: SwapNavKey; label: string }> = [
  { href: '/swap', key: 'swap', label: 'Swap' },
  { href: '/swap/rescue', key: 'rescue', label: 'Rescue' },
  { href: '/swap/history', key: 'history', label: 'History' },
  { href: '/swap/settings', key: 'settings', label: 'Settings' },
]

/**
 * The swap surface shell (SWAP-7, #9322): the existing public header, one
 * secondary nav strip across Swap / Rescue / History / Settings, and the
 * build-provenance line on every page. Products is deliberately absent (our
 * equivalents are provider-side and the web never grows an operator view);
 * Help and Docs are links to existing surfaces, not new pages.
 */
export function SwapSurfaceShell({
  active,
  children,
}: Readonly<{ active: SwapNavKey; children: ReactNode }>) {
  return (
    <div
      className="min-h-dvh bg-black font-mono text-white"
      data-swap-surface=""
    >
      <PublicHeader />
      <nav
        aria-label="Swap surface navigation"
        className="border-b border-khala-border/60 bg-khala-surface/60"
        data-swap-nav=""
      >
        <div className="mx-auto flex w-full max-w-5xl flex-wrap items-center gap-1 px-4 py-2 sm:px-6">
          {NAV_ITEMS.map(item => (
            <InternalLink
              {...(item.key === active
                ? { 'aria-current': 'page' as const }
                : {})}
              className={[
                'px-3 py-1.5 text-sm',
                item.key === active
                  ? 'border border-khala-border bg-black text-white'
                  : 'border border-transparent text-khala-text-muted hover:text-white',
              ].join(' ')}
              href={item.href}
              key={item.key}
            >
              {item.label}
            </InternalLink>
          ))}
          <span className="flex-1" />
          <InternalLink
            className="px-3 py-1.5 text-sm text-khala-text-muted hover:text-white"
            href="/forum"
          >
            Help
          </InternalLink>
          <InternalLink
            className="px-3 py-1.5 text-sm text-khala-text-muted hover:text-white"
            href={DOCS_URL}
          >
            Docs
          </InternalLink>
        </div>
      </nav>
      <main className="mx-auto grid w-full max-w-5xl gap-6 px-4 py-8 sm:px-6">
        {children}
      </main>
      <footer className="mx-auto w-full max-w-5xl px-4 pb-8 sm:px-6">
        <SwapBuildProvenance />
      </footer>
    </div>
  )
}

/**
 * The fail-closed gate view. It renders no swap capability, links to
 * nothing that pretends to be one, and still states its build provenance.
 */
export function SwapSurfaceGateClosed() {
  return (
    <div
      className="min-h-dvh bg-black font-mono text-white"
      data-swap-surface-gate-closed=""
    >
      <PublicHeader />
      <main className="mx-auto grid w-full max-w-5xl gap-4 px-4 py-16 sm:px-6">
        <h1 className="m-0 text-2xl font-semibold tracking-tight">
          Swap surface not enabled
        </h1>
        <p className="m-0 max-w-[70ch] text-pretty text-sm/6 text-khala-text-muted">
          This deployment does not serve the swap surface. The gate fails
          closed: it opens only by an explicit operator decision, after the
          swap network the surface depends on is proven.
        </p>
        <SwapBuildProvenance />
      </main>
    </div>
  )
}
