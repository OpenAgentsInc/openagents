import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  LandingEnPage,
  initialLandingEnState,
  landingEnStateFromPublicSnapshots,
  landingEnView,
  mountLandingEnSurface,
} from './-landing-en-page'
import { Effect, Exit, Scope } from '@effect-native/core/effect'

describe('WEB-1-EN landing Effect Native route (#8595)', () => {
  test('server render is only a thin mount shim, not landing-content React', () => {
    const html = renderToStaticMarkup(<LandingEnPage />)

    expect(html).toContain('data-route="landing-en"')
    expect(html).toContain('data-landing-en-root=""')
    // Landing content lives in the EN tree, never in the React shim.
    expect(html).not.toContain('Software, built by agents.')
    expect(html).not.toContain('Live network activity')
  })

  test('authored landing content is one typed Effect Native tree from the catalog', () => {
    const tree = landingEnView(initialLandingEnState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({ tag: 'Stack', key: 'landing-en-root' })
    // Pinned catalog version (guarded by the vendor-freshness test).
    expect(serialized).toContain('"catalogVersion":"effect-native/v25"')
    // Every marketing catalog component the landing must be authored from.
    // (PricingTable/PricingColumn render in the hydrated `ready` state — see
    // the live-projection test below.)
    for (const tag of [
      'NavBar',
      'AnnouncementBadge',
      'Hero',
      'MockupFrame',
      'Glow',
      'StatsBand',
      'Accordion',
      'CtaSection',
      'Footer',
      'Section',
    ]) {
      expect(serialized).toContain(`"_tag":"${tag}"`)
    }
    // Existing OpenAgents copy preserved verbatim; no React className leakage.
    expect(serialized).toContain('Software, built by agents.')
    expect(serialized).toContain('LandingEnNavigated')
    expect(serialized).not.toContain('className')
    // Copy freeze: launch-ui template author copy must NOT ship here.
    expect(serialized).not.toContain('Launch UI')
    expect(serialized).not.toContain('Dobrucki')
  })

  test('public projection snapshots hydrate LIVE stats and plan columns', () => {
    const state = landingEnStateFromPublicSnapshots(
      { tokensServed: 1234567 },
      {
        pylonsOnlineNow: 4,
        pylonsAssignmentReadyNow: 3,
        trainingModelProgressContributors: 2,
      },
      {
        summary: 'Public catalog summary',
        plans: [
          {
            planId: 'free',
            kind: 'free',
            label: 'Free',
            tagline: 'Start with the free public lane.',
            priceLabel: '$0',
            isDefault: true,
            captureExcluded: false,
            terms: ['No card required', 'Public receipts'],
          },
        ],
      },
    )

    expect(state.stats.map((stat) => stat.value)).toEqual([
      '1,234,567',
      '4',
      '3',
      '2',
    ])
    expect(state.pricingState).toBe('ready')
    expect(state.planSummary).toBe('Public catalog summary')
    expect(state.plans[0]?.cta).toBe('Get started')

    // The live values must appear inside the rendered StatsBand/PricingTable,
    // authored from the PricingTable/PricingColumn catalog components.
    const serialized = JSON.stringify(landingEnView(state))
    expect(serialized).toContain('"_tag":"PricingTable"')
    expect(serialized).toContain('"_tag":"PricingColumn"')
    expect(serialized).toContain('1,234,567')
    expect(serialized).toContain('Public catalog summary')
  })

  test('never fabricates numbers before the first live fetch resolves', () => {
    const serialized = JSON.stringify(landingEnView(initialLandingEnState))
    // Pending placeholder, never a hardcoded metric.
    expect(serialized).toContain('—')
  })

  test('real DOM mount smoke: the EN tree renders the landing sections', async () => {
    const container = document.createElement('div')
    document.body.appendChild(container)

    // Live-data hydration is fail-soft; stub fetch to reject so the mount
    // resolves fast to the honest pending surface (no fabricated numbers).
    const originalFetch = globalThis.fetch
    globalThis.fetch = (() =>
      Promise.reject(new Error('offline in test'))) as unknown as typeof fetch

    const scope = await Effect.runPromise(Scope.make())
    const surface = await Effect.runPromise(
      Scope.provide(scope)(mountLandingEnSurface(container)),
    )

    // Marketing components emit their signature DOM markers.
    expect(container.querySelector('[data-en-navbar]')).not.toBeNull()
    expect(container.querySelector('[data-en-announcement]')).not.toBeNull()
    expect(container.querySelector('[data-en-mockup]')).not.toBeNull()
    expect(container.querySelector('[data-en-glow]')).not.toBeNull()
    expect(container.querySelector('[data-en-cta]')).not.toBeNull()
    expect(container.querySelectorAll('[data-en-stat]').length).toBe(4)
    // Verbatim hero copy landed in the DOM.
    expect(container.textContent).toContain('Software, built by agents.')

    await Effect.runPromise(surface.unmount)
    await Effect.runPromise(Scope.close(scope, Exit.void))
    container.remove()
    globalThis.fetch = originalFetch
  })

  test('source boundary uses Effect Native packages, not launch-ui components', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-landing-en-page.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).toContain("from '@effect-native/tokens'")
    expect(source).not.toContain('@/components/launch-ui')
    expect(source).not.toContain('lucide-react')
  })
})
