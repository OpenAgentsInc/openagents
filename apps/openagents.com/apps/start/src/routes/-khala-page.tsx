import { InternalLink } from '@/components/internal-link'
import { useEffect, useState } from 'react'

import {
  LIVE_VALUE_PENDING,
  fetchKhalaTokensServed,
  formatCount,
} from './-sales-landing-data'

// Public `/khala` inference page. Converted from an Effect Native view tree
// back to plain React (#9325): the typed view program and its `SubscriptionRef`
// are ordinary React state, and the live tokens-served projection is fetched in
// an effect. Copy is preserved verbatim. The fetch stays fail-soft — a failed
// or missing snapshot renders the honest pending placeholder, never a
// fabricated number.

export type KhalaLandingState = Readonly<{
  tokensServed: string
}>

const initialKhalaLandingState: KhalaLandingState = {
  tokensServed: LIVE_VALUE_PENDING,
}

export const khalaStateFromPublicSnapshot = (
  tokens: Awaited<ReturnType<typeof fetchKhalaTokensServed>>,
): KhalaLandingState => ({
  tokensServed: formatCount(tokens?.tokensServed ?? null),
})

const shellClass = 'min-h-dvh overflow-y-auto bg-khala-void text-khala-text'

const sectionClass = 'mx-auto grid w-full max-w-[960px] gap-6 p-6'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-energy-soft'

const headingClass =
  'm-0 text-balance text-5xl font-semibold leading-none text-white sm:text-7xl'

const bodyClass = 'm-0 max-w-[72ch] text-pretty text-base/7 text-khala-text-muted'

const cardClass =
  'grid content-start gap-3 border border-khala-border bg-khala-surface p-4'

const cardLabelClass = 'm-0 font-mono text-sm text-khala-text-faint'

const cardValueClass = 'm-0 break-all font-mono text-base text-white'

const navLinkClass =
  'khala-focus inline-flex min-h-12 w-fit items-center justify-center border border-khala-energy px-4 font-mono text-sm font-semibold text-khala-energy-soft'

const infoCards = [
  { key: 'model', label: 'Model', value: 'openagents/khala' },
  { key: 'base-url', label: 'Base URL', value: 'https://openagents.com/api/v1' },
  { key: 'free-key', label: 'Free key', value: 'POST /api/keys/free' },
] as const

export function KhalaPage() {
  const [tokensServed, setTokensServed] = useState(
    initialKhalaLandingState.tokensServed,
  )

  useEffect(() => {
    let cancelled = false

    void fetchKhalaTokensServed()
      .then(snapshot => {
        if (!cancelled) {
          setTokensServed(khalaStateFromPublicSnapshot(snapshot).tokensServed)
        }
      })
      .catch(() => undefined)

    return () => {
      cancelled = true
    }
  }, [])

  return (
    <main
      aria-label="Khala - OpenAgents inference"
      className={shellClass}
      data-route="khala"
    >
      <div className={sectionClass}>
        <InternalLink className={navLinkClass} href="/">
          ← OpenAgents
        </InternalLink>
      </div>

      <div className={sectionClass}>
        <p className={eyebrowClass}>OpenAgents inference</p>
        <h1 className={headingClass}>Khala</h1>
        <p className={bodyClass}>
          Khala is the OpenAgents inference and work rail: an OpenAI-compatible
          API for public model access, work receipts, and agent-readable
          evidence. This public page keeps the usable API basics visible without
          claiming paid capacity is generally live.
        </p>
      </div>

      <div className={`${sectionClass} md:grid-cols-3`}>
        {infoCards.map(card => (
          <article className={cardClass} key={card.key}>
            <p className={cardLabelClass}>{card.label}</p>
            <p className={cardValueClass}>{card.value}</p>
          </article>
        ))}
      </div>

      <div className={sectionClass}>
        <div className={cardClass}>
          <div className="flex flex-wrap items-baseline gap-3">
            <span className="font-mono text-sm uppercase tracking-wide text-khala-text-faint">
              Tokens Served
            </span>
            <span className="font-mono text-2xl font-semibold text-white">
              {tokensServed}
            </span>
          </div>
          <p className={bodyClass}>
            The live counter is hydrated by the production API on the live app.
            This route preserves the same live projection for the route-by-route
            migration.
          </p>
        </div>
      </div>

      <div className={sectionClass}>
        <div className="flex flex-wrap gap-3">
          <InternalLink className={navLinkClass} href="/docs/openagents">
            Read the overview
          </InternalLink>
          <InternalLink className={navLinkClass} href="/khala/chat-sync">
            Open web chat sync
          </InternalLink>
        </div>
      </div>
    </main>
  )
}
