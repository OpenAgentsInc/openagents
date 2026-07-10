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

/** Full marketing-catalog tag set the WEB-1-EN landing must author from. */
const MARKETING_CATALOG_TAGS = [
  'NavBar',
  'AnnouncementBadge',
  'Hero',
  'MockupFrame',
  'Glow',
  'LogoRow',
  'StatsBand',
  'Accordion',
  'CtaSection',
  'Footer',
  'Section',
] as const

const LIVE_HYDRATED_STATE = landingEnStateFromPublicSnapshots(
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
      {
        planId: 'privacy',
        kind: 'paid',
        label: 'Privacy',
        tagline: 'Private work lane.',
        priceLabel: 'Talk to us',
        isDefault: false,
        captureExcluded: false,
        terms: ['Private receipts', 'Operator review'],
        purchase: { armed: true },
      },
    ],
  },
)

describe('WEB-1-EN landing Effect Native route (#8595)', () => {
  test('server render is only a thin mount shim, not landing-content React', () => {
    const html = renderToStaticMarkup(<LandingEnPage />)

    expect(html).toContain('data-route="landing-en"')
    expect(html).toContain('data-landing-en-root=""')
    // Landing content lives in the EN tree, never in the React shim.
    expect(html).not.toContain('Software, built by agents.')
    expect(html).not.toContain('Live network activity')
  })

  test('authored landing content is one typed Effect Native tree from the full marketing catalog', () => {
    const tree = landingEnView(initialLandingEnState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({ tag: 'Stack', key: 'landing-en-root' })
    // Pinned catalog version (guarded by the vendor-freshness test / vendor.json).
    expect(serialized).toContain('"catalogVersion":"effect-native/v29"')
    // Every marketing catalog component the landing must be authored from.
    // (PricingTable/PricingColumn render in the hydrated `ready` state — see
    // the live-projection test below.)
    for (const tag of MARKETING_CATALOG_TAGS) {
      expect(serialized).toContain(`"_tag":"${tag}"`)
    }
    // LogoRow uses URI-schema https placeholders (stage1 pattern).
    expect(serialized).toContain('https://cdn.simpleicons.org/')
    expect(serialized).toContain('"alt":"Khala Code"')
    // Existing OpenAgents copy preserved verbatim; no React className leakage.
    expect(serialized).toContain('Software, built by agents.')
    expect(serialized).toContain('LandingEnNavigated')
    expect(serialized).toContain('LandingEnFaqToggled')
    expect(serialized).not.toContain('className')
    // Copy freeze: launch-ui template author copy must NOT ship here.
    expect(serialized).not.toContain('Launch UI')
    expect(serialized).not.toContain('Dobrucki')
    expect(serialized).not.toContain('designwithcode')
  })

  test('public projection snapshots hydrate LIVE stats and plan columns', () => {
    expect(LIVE_HYDRATED_STATE.stats.map((stat) => stat.value)).toEqual([
      '1,234,567',
      '4',
      '3',
      '2',
    ])
    expect(LIVE_HYDRATED_STATE.pricingState).toBe('ready')
    expect(LIVE_HYDRATED_STATE.planSummary).toBe('Public catalog summary')
    expect(LIVE_HYDRATED_STATE.plans[0]?.cta).toBe('Get started')
    expect(LIVE_HYDRATED_STATE.plans[1]?.cta).toBe('Talk to us')
    expect(LIVE_HYDRATED_STATE.plans[1]?.highlighted).toBe(true)

    // Live values must appear inside the rendered StatsBand/PricingTable,
    // authored from the PricingTable/PricingColumn catalog components.
    const serialized = JSON.stringify(landingEnView(LIVE_HYDRATED_STATE))
    expect(serialized).toContain('"_tag":"PricingTable"')
    expect(serialized).toContain('"_tag":"PricingColumn"')
    expect(serialized).toContain('"_tag":"StatsBand"')
    expect(serialized).toContain('1,234,567')
    expect(serialized).toContain('Public catalog summary')
    expect(serialized).toContain('"value":"1,234,567"')
    expect(serialized).toContain('"value":"4"')
    expect(serialized).toContain('Get started')
    expect(serialized).toContain('Talk to us')
    // Hydrated tree still carries the rest of the marketing catalog.
    for (const tag of MARKETING_CATALOG_TAGS) {
      expect(serialized).toContain(`"_tag":"${tag}"`)
    }
  })

  test('never fabricates numbers before the first live fetch resolves', () => {
    const serialized = JSON.stringify(landingEnView(initialLandingEnState))
    // Pending placeholder, never a hardcoded metric.
    expect(serialized).toContain('—')
    // Pricing stays pending (no PricingTable until live catalog is ready).
    expect(serialized).not.toContain('"_tag":"PricingTable"')
    expect(serialized).toContain('loading live plan catalog')
  })

  test('unavailable plan catalog stays honest (no fabricated pricing)', () => {
    const unavailable = landingEnStateFromPublicSnapshots(
      { tokensServed: 99 },
      null,
      null,
    )
    expect(unavailable.pricingState).toBe('unavailable')
    expect(unavailable.plans).toEqual([])
    expect(unavailable.planSummary).toContain('no pricing value is fabricated')
    const serialized = JSON.stringify(landingEnView(unavailable))
    expect(serialized).not.toContain('"_tag":"PricingTable"')
    expect(serialized).toContain('Plan catalog unavailable.')
    // Stats still hydrate when partial data is present.
    expect(serialized).toContain('99')
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
    // LogoRow renders per-logo img markers (URI-schema https placeholders).
    expect(container.querySelectorAll('[data-en-logo]').length).toBe(4)
    expect(container.querySelector('[data-en-tag="LogoRow"]')).not.toBeNull()
    expect(container.querySelectorAll('[data-en-stat]').length).toBe(4)
    // FAQ accordion present (mode + items).
    expect(container.querySelector('[data-en-accordion-mode]')).not.toBeNull()
    expect(container.querySelectorAll('[data-en-accordion-item]').length).toBe(2)
    // Verbatim hero copy landed in the DOM.
    expect(container.textContent).toContain('Software, built by agents.')
    expect(container.textContent).toContain('Live network activity')
    expect(container.textContent).toContain('Four work surfaces, one receipt discipline')

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
    const routeSource = readFileSync(
      join(process.cwd(), 'src/routes/landing-en.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).toContain("from '@effect-native/tokens'")
    // Full marketing catalog consumption (not just base primitives).
    for (const symbol of [
      'NavBar',
      'AnnouncementBadge',
      'Hero',
      'LogoRow',
      'StatsBand',
      'PricingTable',
      'PricingColumn',
      'Accordion',
      'CtaSection',
      'Footer',
      'Glow',
      'MockupFrame',
    ]) {
      expect(source).toContain(symbol)
    }
    // Live projection path (stats + pricing), not static numbers.
    expect(source).toContain('fetchKhalaTokensServed')
    expect(source).toContain('fetchPylonStats')
    expect(source).toContain('fetchKhalaCodePlans')
    expect(source).toContain('landingEnStateFromPublicSnapshots')
    // Boundary: no launch-ui React section components.
    expect(source).not.toContain('@/components/launch-ui')
    expect(source).not.toContain('lucide-react')
    expect(source).not.toContain('from "lucide-react"')
    expect(routeSource).not.toContain('@/components/launch-ui')
    // Root route is NOT flipped here — this page only mounts at /landing-en.
    expect(routeSource).toContain("createFileRoute('/landing-en')")
    expect(routeSource).not.toContain("createFileRoute('/')")
  })
})
