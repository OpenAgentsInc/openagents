// WEB-1-EN landing conversion (OpenAgentsInc/openagents#8595).
//
// This route (`/landing-en`) re-authors the whole landing as ONE typed Effect
// Native view tree built from the vendored marketing catalog components
// (NavBar, AnnouncementBadge, Hero, StatsBand, PricingTable/PricingColumn,
// Accordion, CtaSection, Footer, Glow, MockupFrame) plus the base primitives.
// React remains ONLY as the thin route-shell host that mounts the EN tree
// through the DOM renderer (the EN adapter rule) — there are zero React section
// components in the landing content.
//
// Reference reconciliation (honest note for the owner review on #8565/#8595):
//   * `/new` is the Launch UI *replica* — it supplies the VISUAL STRUCTURE and
//     launch-ui-grade polish (navbar, announcement, hero, logos, items, stats,
//     pricing, faq, cta, footer, mockup+glow centerpiece), but its strings are
//     the template author's copy ("Launch UI v2 is out!", "$99 Pro", the
//     "Mikołaj Dobrucki" copyright, designwithcode.dev). Those are NOT
//     OpenAgents copy and must not ship on an OpenAgents route.
//   * `/stage1` is the existing Effect Native OpenAgents landing — it supplies
//     the CONTENT and the LIVE public-projection data path.
// So this page pairs `/new`'s section structure + polish with `/stage1`'s
// existing OpenAgents copy (VERBATIM — no new words invented; owner-copy TODO
// placeholders preserved as-is) and its live-data path. Final landing copy is
// still owner-gated on #8565.
//
// Live data stays live: stats bind to the LIVE public counters and pricing
// renders the LIVE Khala Code plan catalog, both via the same fail-soft public
// projection fetchers used by `/stage1`. Because the DOM renderer resolves
// marketing `Bound<string>` fields to state only through a full view re-render
// (a Binding literal renders empty), live values flow as plain strings held in
// the SubscriptionRef and re-emitted by makeViewProgramFromState on each fetch
// — never fabricated/static numbers.

import {
  Button,
  Card,
  IntentRef,
  StaticPayload,
  Stack,
  Text,
  AnnouncementBadge,
  Accordion,
  CtaSection,
  Footer,
  Glow,
  Hero,
  MockupFrame,
  NavBar,
  PricingColumn,
  PricingTable,
  Section,
  StatsBand,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type AccordionItem,
  type ButtonView,
  type FooterColumn,
  type IntentHandlers,
  type IntentReporter,
  type NavBarLink,
  type PricingColumnView,
  type StatsBandItem,
  type TextView,
  type View,
} from '@effect-native/core'
import { makeDomRenderer } from '@effect-native/render-dom'
import { Effect, Exit, Schema, Scope, SubscriptionRef } from '@effect-native/core/effect'
import { khalaTheme } from '@effect-native/tokens'
import { useEffect, useRef } from 'react'

import {
  SALES_LANDING_LINKS,
  fetchKhalaCodePlans,
  fetchKhalaTokensServed,
  formatCount,
  LIVE_VALUE_PENDING,
  type KhalaCodePlanCatalogProjection,
} from './-sales-landing-data'
import {
  fetchPylonStats,
  type PylonStatsSnapshot,
} from './-pylon-network'

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

type LandingEnStat = Readonly<{
  key: string
  label: string
  value: string
  description: string
}>

type LandingEnPlan = Readonly<{
  key: string
  name: string
  tagline: string
  priceLabel: string
  cta: string
  href: string
  terms: ReadonlyArray<string>
  highlighted: boolean
}>

export type LandingEnState = Readonly<{
  stats: ReadonlyArray<LandingEnStat>
  pricingState: 'pending' | 'ready' | 'unavailable'
  planSummary: string
  plans: ReadonlyArray<LandingEnPlan>
  faqExpandedId: string | null
}>

class LandingEnSnapshotError extends Schema.TaggedErrorClass<LandingEnSnapshotError>()(
  'LandingEnSnapshotError',
  { message: Schema.String },
) {}

const Navigated = defineIntent(
  'LandingEnNavigated',
  Schema.Struct({ href: Schema.String }),
)

const FaqToggled = defineIntent(
  'LandingEnFaqToggled',
  Schema.Struct({ id: Schema.String }),
)

const landingEnIntents = [Navigated, FaqToggled] as const

const emptyStats = (): ReadonlyArray<LandingEnStat> => [
  {
    key: 'tokens',
    label: 'served',
    value: LIVE_VALUE_PENDING,
    description: 'real input + output tokens across OpenAgents products',
  },
  {
    key: 'online',
    label: 'online now',
    value: LIVE_VALUE_PENDING,
    description: 'Pylon nodes with a live heartbeat',
  },
  {
    key: 'ready',
    label: 'work-ready',
    value: LIVE_VALUE_PENDING,
    description: 'Pylons ready to accept assignments now',
  },
  {
    key: 'contributors',
    label: 'contributing',
    value: LIVE_VALUE_PENDING,
    description: 'contributors to live training-model progress',
  },
]

export const initialLandingEnState: LandingEnState = {
  stats: emptyStats(),
  pricingState: 'pending',
  planSummary:
    'Live plan catalog loads from the public Khala Code plans projection.',
  plans: [],
  faqExpandedId: 'faq-build',
}

const planFromProjection = (
  plan: KhalaCodePlanCatalogProjection['plans'][number],
): LandingEnPlan => ({
  key: plan.planId,
  name: plan.label,
  tagline: plan.tagline,
  priceLabel: plan.priceLabel,
  cta:
    plan.kind === 'free'
      ? 'Get started'
      : plan.purchase?.armed === true
        ? 'Talk to us'
        : 'Not yet purchasable',
  href:
    plan.kind === 'free'
      ? SALES_LANDING_LINKS.khala
      : SALES_LANDING_LINKS.businessIntake,
  terms: plan.terms.slice(0, 4),
  highlighted: plan.kind !== 'free',
})

export const landingEnStateFromPublicSnapshots = (
  tokens: Awaited<ReturnType<typeof fetchKhalaTokensServed>>,
  pylons: PylonStatsSnapshot | null,
  catalog: KhalaCodePlanCatalogProjection | null,
  previous: LandingEnState = initialLandingEnState,
): LandingEnState => ({
  stats: [
    {
      key: 'tokens',
      label: 'served',
      value: formatCount(tokens?.tokensServed ?? null),
      description: 'real input + output tokens across OpenAgents products',
    },
    {
      key: 'online',
      label: 'online now',
      value: formatCount(pylons?.pylonsOnlineNow ?? null),
      description: 'Pylon nodes with a live heartbeat',
    },
    {
      key: 'ready',
      label: 'work-ready',
      value: formatCount(pylons?.pylonsAssignmentReadyNow ?? null),
      description: 'Pylons ready to accept assignments now',
    },
    {
      key: 'contributors',
      label: 'contributing',
      value: formatCount(pylons?.trainingModelProgressContributors ?? null),
      description: 'contributors to live training-model progress',
    },
  ],
  pricingState: catalog === null ? 'unavailable' : 'ready',
  planSummary:
    catalog?.summary ??
    'Public plan catalog unavailable; no pricing value is fabricated.',
  plans: catalog?.plans.map(planFromProjection) ?? [],
  faqExpandedId: previous.faqExpandedId,
})

// ---------------------------------------------------------------------------
// View helpers
// ---------------------------------------------------------------------------

const navigateIntent = (href: string) =>
  IntentRef('LandingEnNavigated', StaticPayload({ href }))

const toggleFaqIntent = IntentRef('LandingEnFaqToggled')

const text = (
  key: string,
  content: string,
  variant: TextView['variant'] = 'body',
  color: TextView['color'] = 'textPrimary',
): TextView =>
  Text({
    key,
    content,
    variant,
    color,
    style: { width: 'full' },
  })

const actionButton = (
  key: string,
  label: string,
  href: string,
  variant: ButtonView['variant'] = 'secondary',
): ButtonView =>
  Button({
    key,
    label,
    variant,
    onPress: navigateIntent(href),
    style: {
      backgroundColor: variant === 'primary' ? 'accent' : 'surface',
      borderColor: variant === 'ghost' ? 'surface' : 'border',
      borderRadius: 'md',
      borderWidth: variant === 'ghost' ? 0 : 1,
      color: 'textPrimary',
      fontWeight: 'semibold',
      paddingTop: '3',
      paddingRight: '4',
      paddingBottom: '3',
      paddingLeft: '4',
      typeScale: 'label',
    },
  })

const bandSection = (
  key: string,
  children: ReadonlyArray<View>,
): View =>
  Section(
    {
      key,
      width: 'contained',
      paddingY: '20',
      style: {
        backgroundColor: 'background',
        gap: '8',
        width: 'full',
      },
    },
    children,
  )

// Suite / "items" cards — VERBATIM OpenAgents copy from /stage1.
const suiteCard = (
  key: string,
  title: string,
  copy: string,
): View =>
  Card(
    {
      key,
      padding: '4',
      radius: 'lg',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
        flex: 1,
        minWidth: 'md',
      },
    },
    [
      text(`${key}-title`, title, 'title'),
      text(`${key}-copy`, copy, 'body', 'textMuted'),
    ],
  )

const suiteCards: ReadonlyArray<View> = [
  suiteCard(
    'suite-khala-code',
    'Khala Code',
    'Open-source coding console, one inbox, exact token accounting, and swarm delegation.',
  ),
  suiteCard(
    'suite-business',
    'Business work',
    'Bounded software outcomes scoped as quick wins, reviewed by humans before publish, send, or spend.',
  ),
  suiteCard(
    'suite-network',
    'Network evidence',
    'Pylons, Forum activity, receipts, and stats stay public-safe and explicit about what is unavailable.',
  ),
  suiteCard(
    'suite-promises',
    'Product promises',
    'Claims route through the promise registry: green, operator-assisted, or roadmap-labeled.',
  ),
]

// FAQ — owner-copy TODO placeholders preserved VERBATIM from /stage1 (#8565).
const faqAccordionItems: ReadonlyArray<AccordionItem> = [
  {
    id: 'faq-build',
    header: 'TODO(owner-copy): How does OpenAgents build software?',
    content: [
      text(
        'faq-build-a',
        'TODO(owner-copy) - placeholder pending owner copy sign-off (#8565).',
        'body',
        'textMuted',
      ),
    ],
  },
  {
    id: 'faq-price',
    header: 'TODO(owner-copy): What do I pay, and how?',
    content: [
      text(
        'faq-price-a',
        'TODO(owner-copy) - placeholder pending owner copy sign-off (#8565).',
        'body',
        'textMuted',
      ),
    ],
  },
]

const statsBandItems = (stats: ReadonlyArray<LandingEnStat>): ReadonlyArray<StatsBandItem> =>
  stats.map((stat) => ({
    id: `stat-${stat.key}`,
    label: stat.label,
    value: stat.value,
    tone: 'info' as const,
  }))

const pricingColumn = (plan: LandingEnPlan): PricingColumnView =>
  PricingColumn({
    key: `plan-${plan.key}`,
    name: plan.name,
    price: plan.priceLabel,
    highlighted: plan.highlighted,
    features: plan.terms.map((term, index) => ({
      id: `plan-${plan.key}-term-${index}`,
      label: term,
      included: true,
    })),
    ctaLabel: plan.cta,
    onCta: navigateIntent(plan.href),
    style: {
      backgroundColor: 'surface',
      borderColor: 'border',
      borderWidth: 1,
      borderRadius: 'lg',
      flex: 1,
      minWidth: 'md',
    },
  })

const navBarLinks: ReadonlyArray<NavBarLink> = [
  { id: 'nav-promises', label: 'Promises', onPress: navigateIntent(SALES_LANDING_LINKS.promises) },
  { id: 'nav-stats', label: 'Stats', onPress: navigateIntent(SALES_LANDING_LINKS.stats) },
  { id: 'nav-forum', label: 'Forum', onPress: navigateIntent(SALES_LANDING_LINKS.forum) },
]

const footerColumns: ReadonlyArray<FooterColumn> = [
  {
    id: 'footer-product',
    title: 'Product',
    links: [
      actionButton('footer-promises', 'Promises', SALES_LANDING_LINKS.promises, 'ghost'),
      actionButton('footer-stats', 'Stats', SALES_LANDING_LINKS.stats, 'ghost'),
      actionButton('footer-docs', 'Docs', SALES_LANDING_LINKS.docs, 'ghost'),
    ],
  },
  {
    id: 'footer-network',
    title: 'Network',
    links: [
      actionButton('footer-forum', 'Forum', SALES_LANDING_LINKS.forum, 'ghost'),
      actionButton('footer-github', 'GitHub', SALES_LANDING_LINKS.github, 'ghost'),
      actionButton('footer-khala', 'Khala', SALES_LANDING_LINKS.khala, 'ghost'),
    ],
  },
]

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

export const landingEnView = (state: LandingEnState): View =>
  Stack(
    {
      key: 'landing-en-root',
      direction: 'column',
      gap: '0',
      style: {
        backgroundColor: 'background',
        minHeight: 'full',
        width: 'full',
      },
    },
    [
      // Navbar (marketing top-nav).
      Section(
        {
          key: 'landing-en-nav',
          width: 'contained',
          paddingY: '4',
          style: { backgroundColor: 'background', width: 'full' },
        },
        [
          NavBar({
            key: 'landing-en-navbar',
            sticky: true,
            brand: text('landing-en-brand', 'OpenAgents', 'title'),
            links: navBarLinks,
            actions: [
              actionButton(
                'landing-en-nav-sarah',
                'Talk to Sarah',
                SALES_LANDING_LINKS.talkToSarah,
                'primary',
              ),
            ],
            style: { width: 'full' },
          }),
        ],
      ),

      // Hero + announcement + mockup/glow centerpiece.
      Section(
        {
          key: 'landing-en-hero',
          width: 'contained',
          paddingY: '24',
          style: { backgroundColor: 'background', gap: '8', width: 'full' },
        },
        [
          AnnouncementBadge({
            key: 'landing-en-announcement',
            label:
              'Effect Native landing preview — copy pending owner sign-off (#8565)',
            actionLabel: 'Product promises',
            onPress: navigateIntent(SALES_LANDING_LINKS.promises),
          }),
          Hero({
            key: 'landing-en-hero-block',
            align: 'start',
            headline: 'Software, built by agents.',
            headlineTone: 'gradient',
            subhead:
              'One open network where coding agents do real work - yours, or ours. Every outcome lands with verifiable receipts.',
            actions: [
              actionButton(
                'landing-en-hero-sarah',
                'Talk to Sarah',
                SALES_LANDING_LINKS.talkToSarah,
                'primary',
              ),
              actionButton(
                'landing-en-hero-business',
                'Start a project',
                SALES_LANDING_LINKS.businessIntake,
                'secondary',
              ),
            ],
            // Mockup + glow centerpiece (the launch-ui signature visual).
            // NOTE(EN-2 gap #8572): the vendored Image schema requires an
            // absolute URI scheme for `source`, so it cannot express a
            // same-origin relative asset like `/dashboard-dark.png` (what the
            // /new launch-ui replica uses). Until that gap is addressed
            // upstream, the mockup frames a representative product panel built
            // from base primitives rather than a broken/blocked <img>.
            media: Glow(
              { key: 'landing-en-hero-glow', intensity: 'lg' },
              [
                MockupFrame(
                  { key: 'landing-en-hero-mockup', variant: 'browser', tilt: 'left' },
                  [
                    Stack(
                      {
                        key: 'landing-en-hero-mockup-body',
                        direction: 'column',
                        gap: '3',
                        padding: '6',
                        style: {
                          backgroundColor: 'surface',
                          minWidth: 'lg',
                          width: 'full',
                        },
                      },
                      [
                        text('landing-en-hero-mockup-title', 'Khala Code', 'title'),
                        text(
                          'landing-en-hero-mockup-copy',
                          'One inbox for agent work with exact token accounting and public receipts.',
                          'body',
                          'textMuted',
                        ),
                        Stack(
                          {
                            key: 'landing-en-hero-mockup-row',
                            direction: 'row',
                            gap: '3',
                            style: { width: 'full' },
                          },
                          [
                            suiteCard(
                              'landing-en-hero-mockup-card-a',
                              'Assignments',
                              'Bounded outcomes, reviewed before publish, send, or spend.',
                            ),
                            suiteCard(
                              'landing-en-hero-mockup-card-b',
                              'Receipts',
                              'Every result lands with verifiable, public-safe evidence.',
                            ),
                          ],
                        ),
                      ],
                    ),
                  ],
                ),
              ],
            ),
          }),
        ],
      ),

      // Logos / "built on" trust band.
      // NOTE(content-gap): the launch-ui logos row uses tech-logo image assets;
      // OpenAgents has no vetted partner/tech logo image set, so this renders a
      // text surface strip. Swapping in the LogoRow catalog component is a
      // pure content/asset task once real logo assets are approved (not a code
      // gap — LogoRow is vendored and renderer-ready).
      bandSection('landing-en-logos', [
        text(
          'landing-en-logos-title',
          'Four work surfaces, one receipt discipline',
          'title',
          'textMuted',
        ),
        Stack(
          {
            key: 'landing-en-logos-row',
            direction: 'row',
            gap: '6',
            align: 'center',
            style: { width: 'full' },
          },
          [
            text('landing-en-logo-khala', 'Khala Code', 'label', 'textMuted'),
            text('landing-en-logo-business', 'Business work', 'label', 'textMuted'),
            text('landing-en-logo-network', 'Network evidence', 'label', 'textMuted'),
            text('landing-en-logo-promises', 'Product promises', 'label', 'textMuted'),
          ],
        ),
      ]),

      // Items / features.
      bandSection('landing-en-suite', [
        text(
          'landing-en-suite-title',
          'Four work surfaces, one receipt discipline',
          'heading',
        ),
        Stack(
          {
            key: 'landing-en-suite-grid',
            direction: 'row',
            gap: '4',
            style: { width: 'full' },
          },
          suiteCards,
        ),
      ]),

      // Stats — LIVE public counters.
      bandSection('landing-en-stats', [
        text('landing-en-stats-title', 'Live network activity', 'heading'),
        text(
          'landing-en-stats-copy',
          'Public projections load client-side; pending or unavailable values stay explicit.',
          'body',
          'textMuted',
        ),
        StatsBand({
          key: 'landing-en-stats-band',
          stats: statsBandItems(state.stats),
          style: { width: 'full' },
        }),
      ]),

      // Pricing — LIVE Khala Code plan catalog.
      bandSection('landing-en-pricing', [
        text('landing-en-pricing-title', 'Plans', 'heading'),
        text('landing-en-pricing-summary', state.planSummary, 'body', 'textMuted'),
        state.pricingState === 'ready' && state.plans.length > 0
          ? PricingTable({
              key: 'landing-en-pricing-table',
              columns: state.plans.map(pricingColumn),
              style: { width: 'full' },
            })
          : Card(
              {
                key: 'landing-en-pricing-pending',
                padding: '4',
                radius: 'lg',
                style: {
                  backgroundColor: 'surface',
                  borderColor: 'border',
                  borderWidth: 1,
                  width: 'full',
                },
              },
              [
                text(
                  'landing-en-pricing-pending-copy',
                  state.pricingState === 'pending'
                    ? `${LIVE_VALUE_PENDING} loading live plan catalog...`
                    : 'Plan catalog unavailable.',
                  'body',
                  'textMuted',
                ),
              ],
            ),
      ]),

      // FAQ.
      bandSection('landing-en-faq', [
        text('landing-en-faq-title', 'Questions and answers', 'heading'),
        Accordion({
          key: 'landing-en-faq-accordion',
          mode: 'single',
          items: faqAccordionItems,
          expandedIds: state.faqExpandedId === null ? [] : [state.faqExpandedId],
          onToggle: toggleFaqIntent,
          style: { width: 'full' },
        }),
      ]),

      // Closing CTA (with glow).
      Section(
        {
          key: 'landing-en-cta',
          width: 'contained',
          paddingY: '24',
          style: { backgroundColor: 'background', width: 'full' },
        },
        [
          Glow(
            { key: 'landing-en-cta-glow', intensity: 'md', style: { width: 'full' } },
            [
              CtaSection({
                key: 'landing-en-cta-block',
                headline: 'TODO(owner-copy): closing call-to-action headline',
                tone: 'info',
                actions: [
                  actionButton(
                    'landing-en-cta-sarah',
                    'Talk to Sarah',
                    SALES_LANDING_LINKS.talkToSarah,
                    'primary',
                  ),
                  actionButton(
                    'landing-en-cta-business',
                    'Start a project',
                    SALES_LANDING_LINKS.businessIntake,
                    'secondary',
                  ),
                ],
              }),
            ],
          ),
        ],
      ),

      // Footer.
      Section(
        {
          key: 'landing-en-footer-section',
          width: 'contained',
          paddingY: '16',
          style: { backgroundColor: 'background', width: 'full' },
        },
        [
          Footer({
            key: 'landing-en-footer',
            brand: text('landing-en-footer-brand', 'OpenAgents', 'title'),
            columns: footerColumns,
            legal: text(
              'landing-en-footer-legal',
              'OpenAgents - Product promises, Forum, stats, privacy, and terms remain the canonical public surfaces.',
              'caption',
              'textMuted',
            ),
            style: { width: 'full' },
          }),
        ],
      ),
    ],
  )

// ---------------------------------------------------------------------------
// Live data + mount
// ---------------------------------------------------------------------------

const updatePublicSnapshot = (
  state: SubscriptionRef.SubscriptionRef<LandingEnState>,
) => {
  const fetchSnapshot = Effect.tryPromise({
    try: async () => {
      const [tokens, pylons, catalog] = await Promise.all([
        fetchKhalaTokensServed(),
        fetchPylonStats(),
        fetchKhalaCodePlans(),
      ])
      return { tokens, pylons, catalog }
    },
    catch: (error) =>
      new LandingEnSnapshotError({
        message: error instanceof Error ? error.message : String(error),
      }),
  })

  return fetchSnapshot.pipe(
    Effect.flatMap(({ tokens, pylons, catalog }) =>
      SubscriptionRef.update(state, (previous) =>
        landingEnStateFromPublicSnapshots(tokens, pylons, catalog, previous),
      ),
    ),
  )
}

export const mountLandingEnSurface = (container: HTMLElement) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialLandingEnState)
    const program = makeViewProgramFromState(state, landingEnView)
    const handlers: IntentHandlers<typeof landingEnIntents> = {
      LandingEnNavigated: ({ href }) =>
        Effect.sync(() => {
          window.location.assign(href)
        }),
      LandingEnFaqToggled: ({ id }) =>
        SubscriptionRef.update(state, (previous) => ({
          ...previous,
          faqExpandedId: previous.faqExpandedId === id ? null : id,
        })),
    }
    const registry = yield* makeIntentRegistry(landingEnIntents, handlers)
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))
    const surface = yield* makeDomRenderer({
      theme: khalaTheme,
    }).mount(container, program.viewStream, report)

    // Hydrate the LIVE public counters + plan catalog. Fail-soft: any
    // network/parse error leaves the honest pending/unavailable state rather
    // than fabricating a number. The SubscriptionRef update re-emits the view
    // with real values through makeViewProgramFromState.
    yield* updatePublicSnapshot(state).pipe(Effect.catch(() => Effect.void))

    return {
      state,
      unmount: surface.unmount,
    }
  })

export function LandingEnPage() {
  const rootRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const root = rootRef.current
    if (root === null) {
      return undefined
    }

    let disposed = false
    let closeScope: (() => void) | undefined

    void Effect.runPromise(Scope.make())
      .then((scope) => {
        const close = () => {
          void Effect.runPromise(Scope.close(scope, Exit.void))
        }
        closeScope = close
        if (disposed) {
          close()
          return undefined
        }
        return Effect.runPromise(
          Scope.provide(scope)(mountLandingEnSurface(root)),
        )
      })
      .catch(() => undefined)

    return () => {
      disposed = true
      closeScope?.()
    }
  }, [])

  return (
    <main
      aria-label="OpenAgents Effect Native landing"
      className="landing-en-host"
      data-route="landing-en"
      data-landing-en=""
    >
      <div ref={rootRef} data-landing-en-root="" />
    </main>
  )
}
