import {
  Button,
  Card,
  IntentRef,
  List,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type ButtonView,
  type CardView,
  type IntentHandlers,
  type IntentReporter,
  type KeyedView,
  type StackView,
  type TextView,
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
}>

export type Stage1LandingState = Readonly<{
  stats: ReadonlyArray<Stage1Stat>
  pricingState: 'pending' | 'ready' | 'unavailable'
  planSummary: string
  plans: ReadonlyArray<Stage1Plan>
}>

class Stage1PublicSnapshotError extends Schema.TaggedErrorClass<Stage1PublicSnapshotError>()(
  'Stage1PublicSnapshotError',
  { message: Schema.String },
) {}

const Navigated = defineIntent(
  'Stage1Navigated',
  Schema.Struct({ href: Schema.String }),
)

const stage1Intents = [Navigated] as const

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
}

const planFromProjection = (
  plan: KhalaCodePlanCatalogProjection['plans'][number],
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
  terms: plan.terms.slice(0, 3),
})

export const stage1StateFromPublicSnapshots = (
  tokens: Awaited<ReturnType<typeof fetchKhalaTokensServed>>,
  pylons: PylonStatsSnapshot | null,
  catalog: KhalaCodePlanCatalogProjection | null,
): Stage1LandingState => ({
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

const keyed = <V extends View>(view: V): V & KeyedView => view as V & KeyedView

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
    style: {
      width: 'full',
    },
  })

const section = (key: string, children: ReadonlyArray<View>): StackView =>
  Stack(
    {
      key,
      direction: 'column',
      gap: '6',
      padding: '6',
      style: {
        width: 'full',
        maxWidth: 1280,
        alignSelf: 'center',
      },
    },
    children,
  )

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

const statCard = (stat: Stage1Stat): CardView =>
  Card(
    {
      key: `stat-${stat.key}`,
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
      text(`stat-${stat.key}-value`, stat.value, 'title', 'textPrimary'),
      text(`stat-${stat.key}-label`, stat.label, 'label', 'accent'),
      text(`stat-${stat.key}-description`, stat.description, 'caption', 'textMuted'),
    ],
  )

const suiteCards: ReadonlyArray<CardView> = [
  Card(
    {
      key: 'suite-khala-code',
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
      text('suite-khala-code-title', 'Khala Code', 'title'),
      text(
        'suite-khala-code-copy',
        'Open-source coding console, one inbox, exact token accounting, and swarm delegation.',
        'body',
        'textMuted',
      ),
    ],
  ),
  Card(
    {
      key: 'suite-business',
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
      text('suite-business-title', 'Business work', 'title'),
      text(
        'suite-business-copy',
        'Bounded software outcomes scoped as quick wins, reviewed by humans before publish, send, or spend.',
        'body',
        'textMuted',
      ),
    ],
  ),
  Card(
    {
      key: 'suite-network',
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
      text('suite-network-title', 'Network evidence', 'title'),
      text(
        'suite-network-copy',
        'Pylons, Forum activity, receipts, and stats stay public-safe and explicit about what is unavailable.',
        'body',
        'textMuted',
      ),
    ],
  ),
  Card(
    {
      key: 'suite-promises',
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
      text('suite-promises-title', 'Product promises', 'title'),
      text(
        'suite-promises-copy',
        'Claims route through the promise registry: green, operator-assisted, or roadmap-labeled.',
        'body',
        'textMuted',
      ),
    ],
  ),
]

const planCard = (plan: Stage1Plan): CardView =>
  Card(
    {
      key: `plan-${plan.key}`,
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
      text(`plan-${plan.key}-label`, plan.label, 'title'),
      text(`plan-${plan.key}-tagline`, plan.tagline, 'body', 'textMuted'),
      text(`plan-${plan.key}-price`, plan.priceLabel, 'heading', 'accent'),
      List(
        {
          key: `plan-${plan.key}-terms`,
          style: {
            backgroundColor: 'surface',
            borderColor: 'surface',
            borderWidth: 0,
            gap: '2',
          },
        },
        plan.terms.map((term, index) =>
          keyed(
            Text({
              key: `plan-${plan.key}-term-${index}`,
              content: term,
              variant: 'caption',
              color: 'textMuted',
            }),
          ),
        ),
      ),
      actionButton(`plan-${plan.key}-cta`, plan.cta, plan.href, 'secondary'),
    ],
  )

const faqItems: ReadonlyArray<CardView> = [
  Card(
    {
      key: 'faq-build',
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
      text('faq-build-q', 'TODO(owner-copy): How does OpenAgents build software?', 'label'),
      text('faq-build-a', 'TODO(owner-copy) - placeholder pending owner copy sign-off (#8565).', 'body', 'textMuted'),
    ],
  ),
  Card(
    {
      key: 'faq-price',
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
      text('faq-price-q', 'TODO(owner-copy): What do I pay, and how?', 'label'),
      text('faq-price-a', 'TODO(owner-copy) - placeholder pending owner copy sign-off (#8565).', 'body', 'textMuted'),
    ],
  ),
]

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
      Stack(
        {
          key: 'stage1-banner',
          direction: 'row',
          justify: 'center',
          padding: '2',
          style: {
            backgroundColor: 'surface',
            borderColor: 'border',
            borderWidth: 1,
            width: 'full',
          },
        },
        [
          text(
            'stage1-banner-copy',
            'stage1 - Effect Native surface, not the live homepage - copy pending owner sign-off (#8565)',
            'caption',
            'textMuted',
          ),
        ],
      ),
      section('stage1-nav', [
        Stack(
          {
            key: 'stage1-nav-row',
            direction: 'row',
            align: 'center',
            justify: 'between',
            gap: '4',
            style: { width: 'full' },
          },
          [
            text('stage1-brand', 'OpenAgents', 'title'),
            Stack(
              {
                key: 'stage1-nav-actions',
                direction: 'row',
                align: 'center',
                gap: '2',
              },
              [
                actionButton('stage1-nav-promises', 'Promises', SALES_LANDING_LINKS.promises, 'ghost'),
                actionButton('stage1-nav-stats', 'Stats', SALES_LANDING_LINKS.stats, 'ghost'),
                actionButton('stage1-nav-sarah', 'Talk to Sarah', SALES_LANDING_LINKS.talkToSarah, 'primary'),
              ],
            ),
          ],
        ),
      ]),
      section('stage1-hero', [
        text('stage1-hero-title', 'Software, built by agents.', 'heading'),
        text(
          'stage1-hero-copy',
          'One open network where coding agents do real work - yours, or ours. Every outcome lands with verifiable receipts.',
          'body',
          'textMuted',
        ),
        Stack(
          {
            key: 'stage1-hero-actions',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          [
            actionButton('stage1-hero-sarah', 'Talk to Sarah', SALES_LANDING_LINKS.talkToSarah, 'primary'),
            actionButton('stage1-hero-business', 'Start a project', SALES_LANDING_LINKS.businessIntake, 'secondary'),
          ],
        ),
      ]),
      section('stage1-stats', [
        text('stage1-stats-title', 'Live network activity', 'title'),
        text(
          'stage1-stats-copy',
          'Public projections load client-side; pending or unavailable values stay explicit.',
          'body',
          'textMuted',
        ),
        Stack(
          {
            key: 'stage1-stat-grid',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          state.stats.map(statCard),
        ),
      ]),
      section('stage1-suite', [
        text('stage1-suite-title', 'Four work surfaces, one receipt discipline', 'title'),
        Stack(
          {
            key: 'stage1-suite-grid',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          suiteCards,
        ),
      ]),
      section('stage1-pricing', [
        text('stage1-pricing-title', 'Plans', 'title'),
        text('stage1-pricing-summary', state.planSummary, 'body', 'textMuted'),
        Stack(
          {
            key: 'stage1-pricing-grid',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          state.pricingState === 'ready'
            ? state.plans.map(planCard)
            : [
                Card(
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
                    text(
                      'stage1-pricing-pending-copy',
                      state.pricingState === 'pending'
                        ? `${LIVE_VALUE_PENDING} loading live plan catalog...`
                        : 'Plan catalog unavailable.',
                      'body',
                      'textMuted',
                    ),
                  ],
                ),
              ],
        ),
      ]),
      section('stage1-faq', [
        text('stage1-faq-title', 'Questions and answers', 'title'),
        Stack(
          {
            key: 'stage1-faq-grid',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          faqItems,
        ),
      ]),
      section('stage1-cta', [
        text(
          'stage1-cta-title',
          'TODO(owner-copy): closing call-to-action headline',
          'title',
        ),
        Stack(
          {
            key: 'stage1-cta-actions',
            direction: 'row',
            gap: '3',
            style: { width: 'full' },
          },
          [
            actionButton('stage1-cta-sarah', 'Talk to Sarah', SALES_LANDING_LINKS.talkToSarah, 'primary'),
            actionButton('stage1-cta-business', 'Start a project', SALES_LANDING_LINKS.businessIntake, 'secondary'),
          ],
        ),
      ]),
      section('stage1-footer', [
        text(
          'stage1-footer-copy',
          'OpenAgents - Product promises, Forum, stats, privacy, and terms remain the canonical public surfaces.',
          'caption',
          'textMuted',
        ),
      ]),
      Spacer({ key: 'stage1-bottom-space', size: '6' }),
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
    Effect.flatMap((next) => SubscriptionRef.set(state, next)),
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
    >
      <div ref={rootRef} data-stage1-effect-native-root="" />
    </main>
  )
}
