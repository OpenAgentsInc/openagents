// PORTAL-1 (#8652): /portal client-portal surface, authored as ONE typed
// Effect Native view tree (catalog v29) with typed intents. React appears
// only in the thin route-shell host (-portal-page.tsx); this module is
// host-free so the Cloud Run monolith can bundle it directly
// (src/portal-entry.ts) and serve openagents.com/portal.
//
// Login-gated: the surface probes /api/auth/session on mount. Logged-out
// renders the login gate (never the engagement body); logged-in loads the
// caller's OWN engagement from the owner-scoped Worker API. Approve/reject
// buttons dispatch typed intents -> POST /api/portal/content/:id/decision ->
// optimistic item state + the decision receipt ref rendered inline.
//
// KPI tiles are HONEST placeholders: values render as an em dash with the
// server-provided "placeholder until live funnel wiring" note. Nothing is
// fabricated client-side.

import {
  Badge,
  Button,
  Card,
  Chip,
  Divider,
  IntentRef,
  Section,
  Stack,
  StatTile,
  StaticPayload,
  StatusBanner,
  Text,
  defineIntent,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type ButtonView,
  type IntentHandlers,
  type IntentReporter,
  type TextView,
  type View,
} from '@effect-native/core'
import { makeDomRenderer } from '@effect-native/render-dom'
import { Effect, Schema, SubscriptionRef } from '@effect-native/core/effect'
import { khalaTheme } from '@effect-native/tokens'

import {
  fetchPortalAuthMode,
  fetchPortalEngagement,
  portalLoginHref,
  submitPortalDecision,
  type PortalContentItem,
  type PortalDecision,
  type PortalEngagementSummary,
  type PortalKpi,
} from './-portal-data'

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export type PortalDecisionPanel = Readonly<{
  phase: 'sending' | 'decided' | 'failed'
  message: string
  receiptRef?: string
}>

export type PortalPageState = Readonly<{
  phase: 'loading' | 'logged_out' | 'empty' | 'ready' | 'unavailable'
  engagement: PortalEngagementSummary | null
  items: ReadonlyArray<PortalContentItem>
  kpis: ReadonlyArray<PortalKpi>
  decisionPanels: Readonly<Record<string, PortalDecisionPanel>>
}>

export const initialPortalPageState: PortalPageState = {
  phase: 'loading',
  engagement: null,
  items: [],
  kpis: [],
  decisionPanels: {},
}

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

const PortalDecisionSubmitted = defineIntent(
  'PortalDecisionSubmitted',
  Schema.Struct({ itemId: Schema.String, decision: Schema.String }),
)

const PortalNavigated = defineIntent(
  'PortalNavigated',
  Schema.Struct({ href: Schema.String }),
)

export const portalIntents = [PortalDecisionSubmitted, PortalNavigated] as const

const navigateIntent = (href: string) =>
  IntentRef('PortalNavigated', StaticPayload({ href }))

const decisionIntent = (itemId: string, decision: PortalDecision) =>
  IntentRef('PortalDecisionSubmitted', StaticPayload({ itemId, decision }))

// ---------------------------------------------------------------------------
// View helpers
// ---------------------------------------------------------------------------

const text = (
  key: string,
  content: string,
  variant: TextView['variant'] = 'body',
  color: TextView['color'] = 'textPrimary',
): TextView =>
  Text({ key, content, variant, color, style: { width: 'full' } })

const actionButton = (
  key: string,
  label: string,
  onPress: ReturnType<typeof navigateIntent>,
  variant: ButtonView['variant'] = 'secondary',
): ButtonView =>
  Button({
    key,
    label,
    variant,
    onPress,
    style: {
      backgroundColor: variant === 'primary' ? 'accent' : 'surface',
      borderColor: 'border',
      borderRadius: 'md',
      borderWidth: 1,
      color: 'textPrimary',
      fontWeight: 'semibold',
      paddingTop: '2',
      paddingRight: '4',
      paddingBottom: '2',
      paddingLeft: '4',
      typeScale: 'label',
    },
  })

const surfaceCard = (
  key: string,
  children: ReadonlyArray<View>,
): View =>
  Card(
    {
      key,
      padding: '6',
      radius: 'lg',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
        gap: '3',
        width: 'full',
      },
    },
    children,
  )

const engagementStatusTone = (
  status: string,
): 'neutral' | 'info' | 'success' | 'warn' => {
  if (status === 'active') return 'success'
  if (status === 'preparing') return 'info'
  if (status === 'paused') return 'warn'
  return 'neutral'
}

const itemStateTone = (
  state: string,
): 'neutral' | 'info' | 'success' | 'warn' | 'danger' => {
  if (state === 'approved') return 'success'
  if (state === 'rejected') return 'danger'
  if (state === 'published') return 'info'
  return 'neutral'
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

const loginGateView = (): View =>
  surfaceCard('portal-login-gate', [
    text('portal-login-title', 'Client portal', 'heading'),
    StatusBanner({
      key: 'portal-login-banner',
      tone: 'info',
      message: 'Log in to view your engagement.',
      style: { width: 'full' },
    }),
    text(
      'portal-login-copy',
      'Your engagement dashboard, content calendar, and approval queue are private to your account.',
      'body',
      'textMuted',
    ),
    actionButton(
      'portal-login-button',
      'Log in with GitHub',
      navigateIntent(portalLoginHref('/portal')),
      'primary',
    ),
  ])

const emptyStateView = (): View =>
  surfaceCard('portal-empty', [
    text('portal-empty-title', 'Your setup is being prepared', 'heading'),
    text(
      'portal-empty-copy',
      'Your engagement has not been provisioned yet. Once your OpenAgents team activates it, your funnel status and content calendar appear here.',
      'body',
      'textMuted',
    ),
    StatusBanner({
      key: 'portal-empty-banner',
      tone: 'info',
      message: 'No engagement is linked to this account yet.',
      style: { width: 'full' },
    }),
  ])

const unavailableView = (): View =>
  surfaceCard('portal-unavailable', [
    text('portal-unavailable-title', 'Portal unavailable', 'heading'),
    StatusBanner({
      key: 'portal-unavailable-banner',
      tone: 'warn',
      message: 'The portal API is unreachable right now. Nothing is shown rather than showing stale or fabricated data.',
      style: { width: 'full' },
    }),
  ])

const kpiTilesView = (kpis: ReadonlyArray<PortalKpi>): View =>
  Stack(
    {
      key: 'portal-kpis',
      direction: 'column',
      gap: '3',
      style: { width: 'full' },
    },
    [
      text('portal-kpis-title', 'Funnel KPIs', 'title'),
      Stack(
        {
          key: 'portal-kpis-row',
          direction: 'row',
          gap: '3',
          style: { width: 'full' },
        },
        kpis.map((kpi) =>
          StatTile({
            key: `portal-kpi-${kpi.key}`,
            label: kpi.label,
            value: kpi.value === null ? '—' : String(kpi.value),
            tone: 'info',
            style: {
              backgroundColor: 'surface',
              borderColor: 'border',
              borderWidth: 1,
              flex: 1,
            },
          }),
        ),
      ),
      text(
        'portal-kpis-note',
        kpis.every((kpi) => kpi.value === null)
          ? 'Honest placeholders: KPI values appear once the live funnel wiring exists — no fabricated numbers.'
          : 'Live values where wired; placeholders stay explicit.',
        'caption',
        'textMuted',
      ),
    ],
  )

const decisionPanelView = (
  item: PortalContentItem,
  panel: PortalDecisionPanel | undefined,
): ReadonlyArray<View> => {
  if (panel?.phase === 'sending') {
    return [
      text(`portal-item-${item.id}-panel`, panel.message, 'caption', 'textMuted'),
    ]
  }
  if (panel?.phase === 'failed') {
    return [
      StatusBanner({
        key: `portal-item-${item.id}-panel`,
        tone: 'danger',
        message: panel.message,
        style: { width: 'full' },
      }),
    ]
  }
  const receiptRef = panel?.receiptRef ?? item.decisionReceiptRef
  if (item.state !== 'draft' && receiptRef !== null && receiptRef !== undefined) {
    return [
      text(
        `portal-item-${item.id}-receipt`,
        `receipt: ${receiptRef}`,
        'caption',
        'textMuted',
      ),
    ]
  }
  return []
}

const contentItemCard = (
  item: PortalContentItem,
  panel: PortalDecisionPanel | undefined,
): View =>
  Card(
    {
      key: `portal-item-${item.id}`,
      padding: '4',
      radius: 'lg',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
        flex: 1,
        gap: '2',
        minWidth: 'md',
      },
    },
    [
      Stack(
        {
          key: `portal-item-${item.id}-tags`,
          direction: 'row',
          gap: '2',
          style: { width: 'full' },
        },
        [
          Chip({
            key: `portal-item-${item.id}-channel`,
            label: 'channel',
            value: item.channel,
            tone: 'info',
          }),
          Badge({
            key: `portal-item-${item.id}-variant`,
            label: `variant ${item.variant.toUpperCase()}`,
            tone: 'neutral',
          }),
          Badge({
            key: `portal-item-${item.id}-state`,
            label: item.state,
            tone: itemStateTone(item.state),
          }),
        ],
      ),
      text(`portal-item-${item.id}-title`, item.title, 'title'),
      text(`portal-item-${item.id}-body`, item.body, 'body', 'textMuted'),
      ...(item.state === 'draft' && panel?.phase !== 'sending'
        ? [
            Stack(
              {
                key: `portal-item-${item.id}-actions`,
                direction: 'row',
                gap: '2',
                style: { width: 'full' },
              },
              [
                actionButton(
                  `portal-item-${item.id}-approve`,
                  'Approve',
                  decisionIntent(item.id, 'approve'),
                  'primary',
                ),
                actionButton(
                  `portal-item-${item.id}-reject`,
                  'Reject',
                  decisionIntent(item.id, 'reject'),
                  'secondary',
                ),
              ],
            ),
          ]
        : []),
      ...decisionPanelView(item, panel),
    ],
  )

/** Group items into A/B pair rows (pairRef), unpaired items render alone. */
export const portalContentPairs = (
  items: ReadonlyArray<PortalContentItem>,
): ReadonlyArray<ReadonlyArray<PortalContentItem>> => {
  const byPair = new Map<string, Array<PortalContentItem>>()
  const rows: Array<ReadonlyArray<PortalContentItem>> = []
  for (const item of items) {
    if (item.pairRef === null) {
      rows.push([item])
      continue
    }
    const existing = byPair.get(item.pairRef)
    if (existing === undefined) {
      const group: Array<PortalContentItem> = [item]
      byPair.set(item.pairRef, group)
      rows.push(group)
      continue
    }
    existing.push(item)
  }
  return rows
}

const contentCalendarView = (state: PortalPageState): View =>
  Stack(
    {
      key: 'portal-calendar',
      direction: 'column',
      gap: '3',
      style: { width: 'full' },
    },
    [
      text('portal-calendar-title', 'Content calendar', 'title'),
      text(
        'portal-calendar-copy',
        'Agent-drafted posts awaiting your decision. A/B variants render side by side; every approve or reject mints a receipt.',
        'body',
        'textMuted',
      ),
      ...(state.items.length === 0
        ? [
            StatusBanner({
              key: 'portal-calendar-empty',
              tone: 'info',
              message: 'No content items yet — drafts appear here as your team publishes the calendar.',
              style: { width: 'full' },
            }),
          ]
        : portalContentPairs(state.items).map((pair, index) =>
            Stack(
              {
                key: `portal-pair-${pair[0]?.pairRef ?? pair[0]?.id ?? index}`,
                direction: 'row',
                gap: '3',
                style: { width: 'full' },
              },
              pair.map((item) =>
                contentItemCard(item, state.decisionPanels[item.id]),
              ),
            ),
          )),
    ],
  )

const readyView = (state: PortalPageState): ReadonlyArray<View> => {
  const engagement = state.engagement
  if (engagement === null) return [emptyStateView()]
  return [
    surfaceCard('portal-header', [
      Stack(
        {
          key: 'portal-header-row',
          direction: 'row',
          gap: '3',
          style: { width: 'full' },
        },
        [
          text('portal-header-name', engagement.name, 'heading'),
          Badge({
            key: 'portal-header-status',
            label: engagement.status,
            tone: engagementStatusTone(engagement.status),
          }),
        ],
      ),
      text(
        'portal-header-copy',
        'Your engagement at a glance: funnel status, the content calendar, and your approval queue.',
        'body',
        'textMuted',
      ),
    ]),
    kpiTilesView(state.kpis),
    Divider({ key: 'portal-divider' }),
    contentCalendarView(state),
  ]
}

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

export const portalPageView = (state: PortalPageState): View =>
  Stack(
    {
      key: 'portal-root',
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
          key: 'portal-section',
          width: 'contained',
          paddingY: '12',
          style: { backgroundColor: 'background', gap: '6', width: 'full' },
        },
        state.phase === 'loading'
          ? [
              text('portal-loading', 'Loading your portal…', 'body', 'textMuted'),
            ]
          : state.phase === 'logged_out'
            ? [loginGateView()]
            : state.phase === 'empty'
              ? [emptyStateView()]
              : state.phase === 'unavailable'
                ? [unavailableView()]
                : readyView(state),
      ),
    ],
  )

// ---------------------------------------------------------------------------
// Mount
// ---------------------------------------------------------------------------

export type PortalSurfaceDependencies = Readonly<{
  fetchFn?: typeof fetch
  assignLocation?: (href: string) => void
}>

export const mountPortalSurface = (
  container: HTMLElement,
  deps: PortalSurfaceDependencies = {},
) =>
  Effect.gen(function* () {
    const fetchFn = deps.fetchFn ?? fetch
    const assignLocation =
      deps.assignLocation ??
      ((href: string) => {
        window.location.assign(href)
      })

    const state = yield* SubscriptionRef.make(initialPortalPageState)
    const program = makeViewProgramFromState(state, portalPageView)

    const updatePanel = (itemId: string, panel: PortalDecisionPanel) =>
      SubscriptionRef.update(state, (previous) => ({
        ...previous,
        decisionPanels: { ...previous.decisionPanels, [itemId]: panel },
      }))

    const setItemState = (
      itemId: string,
      itemState: string,
      receiptRef?: string,
    ) =>
      SubscriptionRef.update(state, (previous) => ({
        ...previous,
        items: previous.items.map((item) =>
          item.id === itemId
            ? {
                ...item,
                state: itemState,
                decisionReceiptRef: receiptRef ?? item.decisionReceiptRef,
              }
            : item,
        ),
      }))

    const handlers: IntentHandlers<typeof portalIntents> = {
      PortalDecisionSubmitted: ({ itemId, decision }) =>
        Effect.gen(function* () {
          const current = yield* SubscriptionRef.get(state)
          const item = current.items.find((entry) => entry.id === itemId)
          if (item === undefined || item.state !== 'draft') return
          if (decision !== 'approve' && decision !== 'reject') return
          const optimisticState =
            decision === 'approve' ? 'approved' : 'rejected'

          // Optimistic: flip the card immediately, roll back on failure.
          yield* setItemState(itemId, optimisticState)
          yield* updatePanel(itemId, {
            phase: 'sending',
            message:
              decision === 'approve' ? 'Approving…' : 'Rejecting…',
          })

          const result = yield* Effect.promise(() =>
            submitPortalDecision({ itemId, decision }, fetchFn),
          )
          if (result.ok) {
            yield* setItemState(itemId, result.state, result.receiptRef)
            yield* updatePanel(itemId, {
              phase: 'decided',
              message: 'Decision recorded',
              receiptRef: result.receiptRef,
            })
            return
          }
          yield* setItemState(itemId, 'draft')
          yield* updatePanel(itemId, {
            phase: 'failed',
            message: `Decision failed · ${result.errorMessage}`,
          })
        }),
      PortalNavigated: ({ href }) =>
        Effect.sync(() => {
          assignLocation(href)
        }),
    }

    const registry = yield* makeIntentRegistry(portalIntents, handlers)
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))
    const surface = yield* makeDomRenderer({ theme: khalaTheme }).mount(
      container,
      program.viewStream,
      report,
    )

    // Login gate first, then the owner-scoped engagement read. Fail-soft:
    // any failure renders the honest unavailable state.
    yield* Effect.promise(async () => {
      const authMode = await fetchPortalAuthMode(fetchFn)
      if (authMode === 'LoggedOut') {
        return { phase: 'logged_out' as const }
      }
      const snapshot = await fetchPortalEngagement(fetchFn)
      if (snapshot === null) {
        return { phase: 'unavailable' as const }
      }
      if (snapshot.kind === 'unauthorized') {
        return { phase: 'logged_out' as const }
      }
      if (snapshot.kind === 'none') {
        return { phase: 'empty' as const }
      }
      return {
        phase: 'ready' as const,
        engagement: snapshot.engagement,
        items: snapshot.items,
        kpis: snapshot.kpis,
      }
    }).pipe(
      Effect.flatMap((loaded) =>
        SubscriptionRef.update(state, (previous) => ({
          ...previous,
          ...loaded,
        })),
      ),
      Effect.catch(() =>
        SubscriptionRef.update(state, (previous) => ({
          ...previous,
          phase: 'unavailable' as const,
        })),
      ),
    )

    return { state, unmount: surface.unmount }
  })
