import {
  Accordion,
  AnnouncementBadge,
  Button,
  Card,
  ComponentValueBinding,
  CtaSection,
  Footer,
  Glow,
  Hero,
  IntentRef,
  LogoRow,
  MockupFrame,
  NavBar,
  PricingColumn,
  PricingTable,
  Section,
  Stack,
  StaticPayload,
  StatsBand,
  Text,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type ButtonView,
  type IntentHandlers,
  type IntentReporter,
  type View,
} from '@effect-native/core'
import { makeDomRenderer } from '@effect-native/render-dom'
import { Effect, Exit, Schema, Scope, SubscriptionRef } from '@effect-native/core/effect'
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
import { stage1EffectNativeTheme } from './-stage1-effect-native-theme'

// WEB-1-EN (#8595): full landing authored from the marketing catalog (v20+),
// not a grey-box Stack/Card approximation. Section order mirrors the React
// launch-ui replica at `/new`. Decorative copy is preserved from that
// replica; live counters + plan columns bind real public projections (never
// fabricated static marketing numbers).

type Stage1Stat = Readonly<{
  key: string
  label: string
  value: string
  description: string
}>

type Stage1Plan = Readonly<{
  key: string
  label: string
  tagline: string
  priceLabel: string
  cta: string
  href: string
  terms: ReadonlyArray<string>
  highlighted: boolean
}>

export type Stage1LandingState = Readonly<{
  stats: ReadonlyArray<Stage1Stat>
  pricingState: 'pending' | 'ready' | 'unavailable'
  planSummary: string
  plans: ReadonlyArray<Stage1Plan>
  expandedFaqIds: ReadonlyArray<string>
  navCollapsed: boolean
}>

class Stage1PublicSnapshotError extends Schema.TaggedErrorClass<Stage1PublicSnapshotError>()(
  'Stage1PublicSnapshotError',
  { message: Schema.String },
) {}

const Navigated = defineIntent(
  'Stage1Navigated',
  Schema.Struct({ href: Schema.String }),
)

const FaqToggled = defineIntent('Stage1FaqToggled', Schema.String)

const MenuToggled = defineIntent('Stage1MenuToggled', Schema.Null)

const stage1Intents = [Navigated, FaqToggled, MenuToggled] as const

const emptyStats = (): ReadonlyArray<Stage1Stat> => [
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

export const initialStage1LandingState: Stage1LandingState = {
  stats: emptyStats(),
  pricingState: 'pending',
  planSummary:
    'Live plan catalog loads from the public Khala Code plans projection.',
  plans: [],
  expandedFaqIds: [],
  navCollapsed: false,
}

const planFromProjection = (
  plan: KhalaCodePlanCatalogProjection['plans'][number],
  index: number,
): Stage1Plan => ({
  key: plan.planId,
  label: plan.label,
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
  highlighted: plan.isDefault === true || index === 1,
})

export const stage1StateFromPublicSnapshots = (
  tokens: Awaited<ReturnType<typeof fetchKhalaTokensServed>>,
  pylons: PylonStatsSnapshot | null,
  catalog: KhalaCodePlanCatalogProjection | null,
): Pick<
  Stage1LandingState,
  'stats' | 'pricingState' | 'planSummary' | 'plans'
> => ({
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
})

const navigateIntent = (href: string) =>
  IntentRef('Stage1Navigated', StaticPayload({ href }))

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

// Feature items — launch-ui sections/items/default.tsx DEFAULT_ITEMS titles +
// descriptions (copy freeze for WEB-1). There is no dedicated Items catalog
// tag; Section + Card is the honest composition over the marketing primitives.
const FEATURE_ITEMS: ReadonlyArray<Readonly<{ id: string; title: string; description: string }>> = [
  {
    id: 'a11y',
    title: 'Accessibility first',
    description: 'Fully WCAG 2.0 compliant, made with best a11y practices',
  },
  {
    id: 'responsive',
    title: 'Responsive design',
    description: 'Looks and works great on any device and screen size',
  },
  {
    id: 'themes',
    title: 'Light and dark mode',
    description: 'Seamless switching between color schemes, 6 themes included',
  },
  {
    id: 'customize',
    title: 'Easy to customize',
    description: 'Flexible options to match your product or brand',
  },
  {
    id: 'perf',
    title: 'Top-level performance',
    description: 'Made for lightning-fast load times and smooth interactions',
  },
  {
    id: 'prod',
    title: 'Production ready',
    description: 'Thoroughly tested and launch-prepared',
  },
  {
    id: 'i18n',
    title: 'Made for localization',
    description: 'Easy to implement support for multiple languages and regions',
  },
  {
    id: 'cms',
    title: 'CMS friendly',
    description: 'Built to work with any headless content management system',
  },
]

// FAQ copy from launch-ui sections/faq/default.tsx (plain-text extraction).
const FAQ_ITEMS: ReadonlyArray<Readonly<{ id: string; question: string; answer: string }>> = [
  {
    id: 'why-landing',
    question: 'Why is building a great landing page critical for your business?',
    answer:
      "In today's AI-driven world, standing out is harder than ever. While anyone can build a product, a professional landing page makes the difference between success and failure. Launch UI helps you ship faster without compromising on quality.",
  },
  {
    id: 'why-not-nocode',
    question: 'Why use Launch UI instead of a no-code tool?',
    answer:
      'No-code tools lock you into their ecosystem with recurring fees and limited control. Launch UI gives you full control of your code while maintaining professional quality.',
  },
  {
    id: 'how-different',
    question:
      'How is Launch UI different from other component libraries and templates?',
    answer:
      'Launch UI stands out with premium design quality and delightful touches of custom animations and illustrations. All components are carefully crafted to help position your product as a professional tool.',
  },
  {
    id: 'code-yours',
    question: 'What exactly does it mean that "The code is yours"?',
    answer:
      'The basic version of Launch UI is open-source and free forever, under a do-whatever-you-want license. The pro version is a one-time purchase with lifetime access — no recurring fees or restrictions.',
  },
  {
    id: 'figma',
    question: 'Are Figma files included?',
    answer:
      'Yes! The complete Launch UI template is available for free on the Figma community.',
  },
  {
    id: 'discount',
    question: 'Can I get a discount?',
    answer:
      "Actually, yes! I'm always actively looking for beta testers of new features. If you are interested in exchanging feedback for a discount, please contact via email.",
  },
]

// LogoRow.source is schema-gated as a URI (`^[a-z][a-z0-9+.-]*:`). Use
// https placeholders for the tool names; real asset URLs can replace these
// without changing the typed tree shape.
const LOGO_ITEMS = [
  { id: 'figma', source: 'https://cdn.simpleicons.org/figma/A259FF', alt: 'Figma' },
  { id: 'react', source: 'https://cdn.simpleicons.org/react/61DAFB', alt: 'React' },
  {
    id: 'typescript',
    source: 'https://cdn.simpleicons.org/typescript/3178C6',
    alt: 'TypeScript',
  },
  {
    id: 'shadcn',
    source: 'https://cdn.simpleicons.org/shadcnui/FFFFFF',
    alt: 'Shadcn/ui',
  },
  {
    id: 'tailwind',
    source: 'https://cdn.simpleicons.org/tailwindcss/06B6D4',
    alt: 'Tailwind',
  },
] as const

export const stage1LandingView = (state: Stage1LandingState): View =>
  Stack(
    {
      key: 'stage1-root',
      direction: 'column',
      gap: '0',
      style: {
        backgroundColor: 'background',
        minHeight: 'full',
        width: 'full',
      },
    },
    [
      Section(
        {
          key: 'stage1-banner-section',
          width: 'full',
          paddingY: '2',
          background: 'surface',
        },
        [
          Text({
            key: 'stage1-banner-copy',
            content:
              'stage1 — Effect Native marketing-catalog landing (WEB-1-EN #8595), not the live homepage',
            variant: 'caption',
            color: 'textMuted',
            style: { width: 'full' },
          }),
        ],
      ),

      // 1. Navbar
      NavBar({
        key: 'stage1-navbar',
        brand: Text({
          key: 'stage1-brand',
          content: 'Launch UI',
          variant: 'title',
        }),
        links: [
          {
            id: 'docs',
            label: 'Docs',
            onPress: navigateIntent(SALES_LANDING_LINKS.docs),
          },
          {
            id: 'promises',
            label: 'Promises',
            onPress: navigateIntent(SALES_LANDING_LINKS.promises),
          },
          {
            id: 'stats',
            label: 'Stats',
            onPress: navigateIntent(SALES_LANDING_LINKS.stats),
          },
          {
            id: 'github',
            label: 'GitHub',
            onPress: navigateIntent(SALES_LANDING_LINKS.github),
          },
        ],
        sticky: true,
        collapsed: state.navCollapsed,
        onToggleMenu: IntentRef('Stage1MenuToggled', StaticPayload(null)),
        actions: [
          actionButton(
            'stage1-nav-cta',
            'Get Started',
            SALES_LANDING_LINKS.khala,
            'primary',
          ),
        ],
      }),

      // 2–3. Announcement + Hero (+ mockup/glow centerpiece)
      Section(
        {
          key: 'stage1-hero-section',
          width: 'contained',
          paddingY: '8',
          background: 'background',
        },
        [
          AnnouncementBadge({
            key: 'stage1-announce',
            label: 'Launch UI v2 is out!',
            actionLabel: 'Read more',
            onPress: navigateIntent(SALES_LANDING_LINKS.docs),
          }),
          Hero({
            key: 'stage1-hero',
            align: 'center',
            headline: 'Give your big idea the design it deserves',
            subhead:
              'Professionally designed blocks and templates built with React, Shadcn/ui and Tailwind that will help your product stand out.',
            headlineTone: 'gradient',
            actions: [
              actionButton(
                'stage1-hero-primary',
                'Get Started',
                SALES_LANDING_LINKS.khala,
                'primary',
              ),
              actionButton(
                'stage1-hero-github',
                'Github',
                SALES_LANDING_LINKS.github,
                'secondary',
              ),
            ],
            media: MockupFrame(
              { key: 'stage1-hero-mockup', variant: 'browser', tilt: 'left' },
              [
                Glow({ key: 'stage1-hero-glow', intensity: 'md' }, [
                  Text({
                    key: 'stage1-hero-mockup-label',
                    content: 'Launch UI app screenshot',
                    variant: 'body',
                    color: 'textMuted',
                  }),
                ]),
              ],
            ),
          }),
        ],
      ),

      // 4. Logos
      Section(
        {
          key: 'stage1-logos-section',
          width: 'contained',
          paddingY: '6',
        },
        [
          Text({
            key: 'stage1-logos-title',
            content: 'Built with industry-standard tools and best practices',
            variant: 'title',
            style: { width: 'full' },
          }),
          LogoRow({
            key: 'stage1-logos',
            logos: LOGO_ITEMS.map((logo) => ({ ...logo })),
          }),
        ],
      ),

      // 5. Items / features
      Section(
        {
          key: 'stage1-items-section',
          width: 'contained',
          paddingY: '6',
        },
        [
          Text({
            key: 'stage1-items-title',
            content: "Everything you need. Nothing you don't.",
            variant: 'heading',
            style: { width: 'full' },
          }),
          Stack(
            {
              key: 'stage1-items-grid',
              direction: 'row',
              gap: '3',
              style: { width: 'full' },
            },
            FEATURE_ITEMS.map((item) =>
              Card(
                {
                  key: `feature-${item.id}`,
                  padding: '4',
                  radius: 'lg',
                  style: {
                    backgroundColor: 'surface',
                    borderColor: 'border',
                    borderWidth: 1,
                    flex: 1,
                    minWidth: 'sm',
                  },
                },
                [
                  Text({
                    key: `feature-${item.id}-title`,
                    content: item.title,
                    variant: 'label',
                  }),
                  Text({
                    key: `feature-${item.id}-body`,
                    content: item.description,
                    variant: 'body',
                    color: 'textMuted',
                  }),
                ],
              ),
            ),
          ),
        ],
      ),

      // 6. Stats — live public projections (standing rule: never static fakes)
      Section(
        {
          key: 'stage1-stats-section',
          width: 'contained',
          paddingY: '6',
        },
        [
          Text({
            key: 'stage1-stats-title',
            content: 'Live network activity',
            variant: 'title',
            style: { width: 'full' },
          }),
          Text({
            key: 'stage1-stats-copy',
            content:
              'Public projections load client-side; pending or unavailable values stay explicit.',
            variant: 'body',
            color: 'textMuted',
            style: { width: 'full' },
          }),
          StatsBand({
            key: 'stage1-stats',
            stats: state.stats.map((stat) => ({
              id: stat.key,
              label: `${stat.label} — ${stat.description}`,
              value: stat.value,
              tone: 'info' as const,
            })),
          }),
        ],
      ),

      // 7. Pricing — live Khala Code plan catalog
      Section(
        {
          key: 'stage1-pricing-section',
          width: 'contained',
          paddingY: '6',
        },
        [
          Text({
            key: 'stage1-pricing-title',
            content: 'Build your dream landing page, today.',
            variant: 'heading',
            style: { width: 'full' },
          }),
          Text({
            key: 'stage1-pricing-summary',
            content: state.planSummary,
            variant: 'body',
            color: 'textMuted',
            style: { width: 'full' },
          }),
          state.pricingState === 'ready' && state.plans.length > 0
            ? PricingTable({
                key: 'stage1-pricing-table',
                columns: state.plans.map((plan) =>
                  PricingColumn({
                    key: `plan-${plan.key}`,
                    name: plan.label,
                    price: plan.priceLabel,
                    features: plan.terms.map((term, index) => ({
                      id: `${plan.key}-term-${index}`,
                      label: term,
                      included: true,
                    })),
                    highlighted: plan.highlighted,
                    ctaLabel: plan.cta,
                    onCta: navigateIntent(plan.href),
                  }),
                ),
              })
            : Card(
                {
                  key: 'stage1-pricing-pending',
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
                  Text({
                    key: 'stage1-pricing-pending-copy',
                    content:
                      state.pricingState === 'pending'
                        ? `${LIVE_VALUE_PENDING} loading live plan catalog...`
                        : 'Plan catalog unavailable.',
                    variant: 'body',
                    color: 'textMuted',
                  }),
                ],
              ),
        ],
      ),

      // 8. FAQ
      Section(
        {
          key: 'stage1-faq-section',
          width: 'contained',
          paddingY: '6',
        },
        [
          Text({
            key: 'stage1-faq-title',
            content: 'Questions and Answers',
            variant: 'heading',
            style: { width: 'full' },
          }),
          Accordion({
            key: 'stage1-faq',
            mode: 'single',
            expandedIds: state.expandedFaqIds,
            onToggle: IntentRef('Stage1FaqToggled', ComponentValueBinding()),
            items: FAQ_ITEMS.map((item) => ({
              id: item.id,
              header: item.question,
              content: [
                Text({
                  key: `faq-${item.id}-body`,
                  content: item.answer,
                  variant: 'body',
                  color: 'textMuted',
                }),
              ],
            })),
          }),
        ],
      ),

      // 9. CTA
      CtaSection({
        key: 'stage1-cta',
        headline: 'Start building',
        body: 'Get started with professionally designed blocks and templates.',
        tone: 'info',
        actions: [
          actionButton(
            'stage1-cta-primary',
            'Get Started',
            SALES_LANDING_LINKS.khala,
            'primary',
          ),
          actionButton(
            'stage1-cta-secondary',
            'Talk to Sarah',
            SALES_LANDING_LINKS.talkToSarah,
            'secondary',
          ),
        ],
      }),

      // 10. Footer
      Footer({
        key: 'stage1-footer',
        brand: Text({
          key: 'stage1-footer-brand',
          content: 'Launch UI',
          variant: 'title',
        }),
        columns: [
          {
            id: 'product',
            title: 'Product',
            links: [
              actionButton(
                'footer-docs',
                'Documentation',
                SALES_LANDING_LINKS.docs,
                'ghost',
              ),
              actionButton(
                'footer-promises',
                'Promises',
                SALES_LANDING_LINKS.promises,
                'ghost',
              ),
            ],
          },
          {
            id: 'company',
            title: 'Company',
            links: [
              actionButton(
                'footer-stats',
                'Stats',
                SALES_LANDING_LINKS.stats,
                'ghost',
              ),
              actionButton(
                'footer-forum',
                'Forum',
                SALES_LANDING_LINKS.forum,
                'ghost',
              ),
            ],
          },
          {
            id: 'contact',
            title: 'Contact',
            links: [
              actionButton(
                'footer-github',
                'GitHub',
                SALES_LANDING_LINKS.github,
                'ghost',
              ),
              actionButton(
                'footer-sarah',
                'Talk to Sarah',
                SALES_LANDING_LINKS.talkToSarah,
                'ghost',
              ),
            ],
          },
        ],
        legal: Text({
          key: 'stage1-footer-legal',
          content:
            '© 2026 OpenAgents. Stage1 Effect Native surface — not the live homepage.',
          variant: 'caption',
          color: 'textMuted',
        }),
      }),
    ],
  )

const updatePublicSnapshot = (
  state: SubscriptionRef.SubscriptionRef<Stage1LandingState>,
) => {
  const fetchSnapshot = Effect.tryPromise({
    try: async () => {
      const [tokens, pylons, catalog] = await Promise.all([
        fetchKhalaTokensServed(),
        fetchPylonStats(),
        fetchKhalaCodePlans(),
      ])
      return stage1StateFromPublicSnapshots(tokens, pylons, catalog)
    },
    catch: (error) =>
      new Stage1PublicSnapshotError({
        message: error instanceof Error ? error.message : String(error),
      }),
  })

  return fetchSnapshot.pipe(
    Effect.flatMap((next) =>
      SubscriptionRef.update(state, (current) => ({
        ...current,
        ...next,
      })),
    ),
  )
}

export const mountStage1EffectNativeSurface = (container: HTMLElement) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialStage1LandingState)
    const program = makeViewProgramFromState(state, stage1LandingView)
    const handlers: IntentHandlers<typeof stage1Intents> = {
      Stage1Navigated: ({ href }) =>
        Effect.sync(() => {
          window.location.assign(href)
        }),
      Stage1FaqToggled: (id) =>
        SubscriptionRef.update(state, (current) => {
          const open = current.expandedFaqIds.includes(id)
          return {
            ...current,
            // single-mode accordion: open only the toggled id, or close all
            expandedFaqIds: open ? [] : [id],
          }
        }),
      Stage1MenuToggled: () =>
        SubscriptionRef.update(state, (current) => ({
          ...current,
          navCollapsed: !current.navCollapsed,
        })),
    }
    const registry = yield* makeIntentRegistry(stage1Intents, handlers)
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))
    const surface = yield* makeDomRenderer({
      theme: stage1EffectNativeTheme,
    }).mount(container, program.viewStream, report)

    yield* updatePublicSnapshot(state).pipe(Effect.catch(() => Effect.void))

    return {
      state,
      unmount: surface.unmount,
    }
  })

export function Stage1EffectNativePage() {
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
          Scope.provide(scope)(mountStage1EffectNativeSurface(root)),
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
      aria-label="OpenAgents stage1 Effect Native landing"
      className="stage1-effect-native-host"
      data-route="stage1-effect-native"
      data-stage1-effect-native=""
      data-web1-en-marketing-catalog=""
    >
      <div ref={rootRef} data-stage1-effect-native-root="" />
    </main>
  )
}
