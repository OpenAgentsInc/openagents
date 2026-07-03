// =============================================================================
// COORDINATOR WIRING (#4977 / WS-A client-delivery workroom page)
//
// This module is a SELF-CONTAINED Foldkit Submodel (Model / Msg / init /
// update / view / commands) for the client-delivery workroom page. It is
// intentionally isolated so it can be unit-tested without the shared logged-in
// Model. A coordinator integrates it into the shared logged-in loop with the
// following edits (do NOT make these edits in this fanout lane):
//
// 1. apps/web/src/route.ts
//      // Route ADT + router (omni workroom + per-tab deep link)
//      export const WorkroomRoute = r('Workroom', { workroomId: S.String })
//      export type WorkroomRoute = typeof WorkroomRoute.Type
//      export const WorkroomTabRoute = r('WorkroomTab', {
//        tab: S.String,
//        workroomId: S.String,
//      })
//      export type WorkroomTabRoute = typeof WorkroomTabRoute.Type
//      export const workroomRouter = pipe(
//        literal('workrooms'),
//        slash(string('workroomId')),
//        Route.mapTo(WorkroomRoute),
//      )
//      export const workroomTabRouter = pipe(
//        literal('workrooms'),
//        slash(string('workroomId')),
//        slash(string('tab')),
//        Route.mapTo(WorkroomTabRoute),
//      )
//      // add WorkroomRoute and WorkroomTabRoute to the LoggedInRoute S.Union([...])
//
// 2. apps/web/src/page/loggedIn/view.ts
//      import * as Workroom from './page/workroom'
//      // inside the routeView M.tagsExhaustive dispatch:
//      Workroom: ({ workroomId }) =>
//        Ui.workroomScrollableRoute<Message>([
//          Workroom.embeddedView(model, workroomId, Workroom.OverviewTab),
//        ]),
//      WorkroomTab: ({ tab, workroomId }) =>
//        Ui.workroomScrollableRoute<Message>([
//          Workroom.embeddedView(model, workroomId, Workroom.tabFromRef(tab)),
//        ]),
//      // (embeddedView projects the shared Model slice into this submodel's
//      //  Model; the coordinator owns the Model.omniWorkroom slice, the Msg
//      //  forwarding, and update wiring via Workroom.update / Workroom.init.)
//
// 3. Sidebar nav item (apps/web/src/page/loggedIn/model.ts initSidebar.primaryItems):
//      { href: workroomRouter({ workroomId: '<active-workroom-id>' }), label: 'Workroom' },
// =============================================================================

import { Match as M, Schema as S } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'
import { m } from 'foldkit/message'
import { ts } from 'foldkit/schema'

import * as Ui from '../../../ui'

// -----------------------------------------------------------------------------
// Surface projection schemas (customer surface of GET /api/omni/workrooms/{id})
// These mirror the server projections in
// workers/api/src/omni-workroom-surface-projections.ts and are intentionally
// loose (optional keys) because projection shape narrows per surface.
// -----------------------------------------------------------------------------

export const OmniWorkroomStatus = S.Literals([
  'open',
  'active',
  'blocked',
  'delivered',
  'accepted',
  'closed',
  'unavailable',
])
export type OmniWorkroomStatus = typeof OmniWorkroomStatus.Type

const OmniWorkroomProjection = S.Struct({
  acceptedOutcomeContractId: S.optionalKey(S.NullOr(S.String)),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  classificationCaveatRef: S.optionalKey(S.String),
  customerIntentRef: S.optionalKey(S.String),
  dataClassification: S.optionalKey(S.String),
  emailRefs: S.optionalKey(S.Array(S.String)),
  publicReceiptRef: S.optionalKey(S.String),
  receiptRefs: S.optionalKey(S.Array(S.String)),
  siteId: S.optionalKey(S.NullOr(S.String)),
  softwareOrderId: S.optionalKey(S.String),
  sourceRefs: S.optionalKey(S.Array(S.String)),
  status: S.optionalKey(S.String),
  trustTier: S.optionalKey(S.String),
  visibility: S.optionalKey(S.String),
  workKind: S.optionalKey(S.String),
})
export type OmniWorkroomProjection = typeof OmniWorkroomProjection.Type

const OmniEvidenceEntryProjection = S.Struct({
  caveatRef: S.optionalKey(S.NullOr(S.String)),
  entryKind: S.optionalKey(S.String),
  redactionState: S.optionalKey(S.String),
  ref: S.optionalKey(S.String),
  sourceAuthority: S.optionalKey(S.String),
  summaryRef: S.optionalKey(S.String),
  visibility: S.optionalKey(S.String),
})
export type OmniEvidenceEntryProjection =
  typeof OmniEvidenceEntryProjection.Type

const OmniEvidenceBundleProjection = S.Struct({
  artifactRefs: S.optionalKey(S.Array(S.String)),
  entries: S.optionalKey(S.Array(OmniEvidenceEntryProjection)),
  id: S.optionalKey(S.String),
  publicReceiptRef: S.optionalKey(S.String),
  receiptRefs: S.optionalKey(S.Array(S.String)),
  sourceRefs: S.optionalKey(S.Array(S.String)),
  sourceAuthorityCaveatRef: S.optionalKey(S.NullOr(S.String)),
  status: S.optionalKey(S.String),
  summaryRef: S.optionalKey(S.String),
  workKind: S.optionalKey(S.String),
  workroomId: S.optionalKey(S.String),
})
export type OmniEvidenceBundleProjection =
  typeof OmniEvidenceBundleProjection.Type

const OmniLifecycleDecisionProjection = S.Struct({
  artifactRef: S.optionalKey(S.NullOr(S.String)),
  customerSafeExplanationRef: S.optionalKey(S.String),
  decisionKind: S.optionalKey(S.String),
  followupRequestRef: S.optionalKey(S.NullOr(S.String)),
  noSettlementImplication: S.optionalKey(S.Boolean),
  receiptRef: S.optionalKey(S.String),
  resultingState: S.optionalKey(S.String),
  siteRevisionFeedbackRef: S.optionalKey(S.NullOr(S.String)),
  workKind: S.optionalKey(S.String),
  workroomId: S.optionalKey(S.String),
})
export type OmniLifecycleDecisionProjection =
  typeof OmniLifecycleDecisionProjection.Type

const OmniRouteScorecardProjection = S.Struct({
  decisionReasonRefs: S.optionalKey(S.Array(S.String)),
  observedResultKind: S.optionalKey(S.String),
  observedResultRef: S.optionalKey(S.String),
  postCloseoutScore: S.optionalKey(S.NullOr(S.Number)),
  privacyTier: S.optionalKey(S.String),
  publicCaveatRef: S.optionalKey(S.String),
  selectedModelRef: S.optionalKey(S.String),
  selectedRouteRef: S.optionalKey(S.String),
  selectedRuntimeRef: S.optionalKey(S.String),
  trustTier: S.optionalKey(S.String),
  workKind: S.optionalKey(S.String),
  workroomId: S.optionalKey(S.String),
})
export type OmniRouteScorecardProjection =
  typeof OmniRouteScorecardProjection.Type

const OmniEconomicsProjection = S.Struct({
  fundingMode: S.optionalKey(S.String),
  noSettlementImplication: S.optionalKey(S.Boolean),
  publicCaveatRef: S.optionalKey(S.String),
  workKind: S.optionalKey(S.String),
  workroomId: S.optionalKey(S.String),
})
export type OmniEconomicsProjection = typeof OmniEconomicsProjection.Type

export const OmniWorkroomSurfaceResponse = S.Struct({
  generatedAt: S.optionalKey(S.String),
  surface: S.String,
  workroom: OmniWorkroomProjection,
  economics: S.optionalKey(S.Array(OmniEconomicsProjection)),
  evidenceBundles: S.optionalKey(S.Array(OmniEvidenceBundleProjection)),
  lifecycleDecisions: S.optionalKey(S.Array(OmniLifecycleDecisionProjection)),
  routeScorecards: S.optionalKey(S.Array(OmniRouteScorecardProjection)),
})
export type OmniWorkroomSurfaceResponse =
  typeof OmniWorkroomSurfaceResponse.Type

export const OmniLifecycleHistoryResponse = S.Struct({
  audience: S.optionalKey(S.String),
  decisions: S.Array(OmniLifecycleDecisionProjection),
  directEffectPermitted: S.optionalKey(S.Boolean),
  workroomId: S.optionalKey(S.String),
})
export type OmniLifecycleHistoryResponse =
  typeof OmniLifecycleHistoryResponse.Type

export const OmniLifecycleDecisionResponse = S.Struct({
  decision: OmniLifecycleDecisionProjection,
  directEffectPermitted: S.optionalKey(S.Boolean),
})
export type OmniLifecycleDecisionResponse =
  typeof OmniLifecycleDecisionResponse.Type

// Customer-safe lifecycle decision kinds the page can POST.
export const OmniLifecycleDecisionKind = S.Literals([
  'accept',
  'reject',
  'provisionally_accept',
  'reopen',
  'request_revision',
  'mark_unavailable',
])
export type OmniLifecycleDecisionKind = typeof OmniLifecycleDecisionKind.Type

// -----------------------------------------------------------------------------
// Tabs
// -----------------------------------------------------------------------------

export const WorkroomTabRef = S.Literals([
  'overview',
  'intake',
  'brand_story',
  'style',
  'offer',
  'site',
  'lead_magnet',
  'email',
  'social',
  'assets',
  'approvals',
  'handoff',
])
export type WorkroomTabRef = typeof WorkroomTabRef.Type

export const OverviewTab: WorkroomTabRef = 'overview'

type TabConfig = Readonly<{ ref: WorkroomTabRef; label: string }>

const TABS: ReadonlyArray<TabConfig> = [
  { label: 'Overview', ref: 'overview' },
  { label: 'Intake', ref: 'intake' },
  { label: 'Brand Story', ref: 'brand_story' },
  { label: 'Style', ref: 'style' },
  { label: 'Offer', ref: 'offer' },
  { label: 'Site', ref: 'site' },
  { label: 'Lead Magnet', ref: 'lead_magnet' },
  { label: 'Email', ref: 'email' },
  { label: 'Social', ref: 'social' },
  { label: 'Assets', ref: 'assets' },
  { label: 'Approvals', ref: 'approvals' },
  { label: 'Handoff', ref: 'handoff' },
]

export const tabFromRef = (value: string): WorkroomTabRef =>
  TABS.some(tab => tab.ref === value) ? (value as WorkroomTabRef) : 'overview'

const tabLabel = (ref: WorkroomTabRef): string =>
  TABS.find(tab => tab.ref === ref)?.label ?? 'Overview'

// -----------------------------------------------------------------------------
// Load-state ADTs (Idle / Loading / Loaded / Failed) following decisions.ts.
// -----------------------------------------------------------------------------

const WorkroomSurfaceIdle = ts('WorkroomSurfaceIdle', {})
const WorkroomSurfaceLoading = ts('WorkroomSurfaceLoading', {})
const WorkroomSurfaceLoaded = ts('WorkroomSurfaceLoaded', {
  response: OmniWorkroomSurfaceResponse,
})
const WorkroomSurfaceFailed = ts('WorkroomSurfaceFailed', { error: S.String })
export const WorkroomSurfaceState = S.Union([
  WorkroomSurfaceIdle,
  WorkroomSurfaceLoading,
  WorkroomSurfaceLoaded,
  WorkroomSurfaceFailed,
])
export type WorkroomSurfaceState = typeof WorkroomSurfaceState.Type

const WorkroomLifecycleIdle = ts('WorkroomLifecycleIdle', {})
const WorkroomLifecycleLoading = ts('WorkroomLifecycleLoading', {})
const WorkroomLifecycleLoaded = ts('WorkroomLifecycleLoaded', {
  response: OmniLifecycleHistoryResponse,
})
const WorkroomLifecycleFailed = ts('WorkroomLifecycleFailed', {
  error: S.String,
})
export const WorkroomLifecycleState = S.Union([
  WorkroomLifecycleIdle,
  WorkroomLifecycleLoading,
  WorkroomLifecycleLoaded,
  WorkroomLifecycleFailed,
])
export type WorkroomLifecycleState = typeof WorkroomLifecycleState.Type

const WorkroomDecisionActIdle = ts('WorkroomDecisionActIdle', {})
const WorkroomDecisionActSubmitting = ts('WorkroomDecisionActSubmitting', {
  decisionKind: OmniLifecycleDecisionKind,
})
const WorkroomDecisionActSucceeded = ts('WorkroomDecisionActSucceeded', {
  response: OmniLifecycleDecisionResponse,
})
const WorkroomDecisionActFailed = ts('WorkroomDecisionActFailed', {
  error: S.String,
})
export const WorkroomDecisionActState = S.Union([
  WorkroomDecisionActIdle,
  WorkroomDecisionActSubmitting,
  WorkroomDecisionActSucceeded,
  WorkroomDecisionActFailed,
])
export type WorkroomDecisionActState = typeof WorkroomDecisionActState.Type

// -----------------------------------------------------------------------------
// Model
// -----------------------------------------------------------------------------

export const Model = S.Struct({
  workroomId: S.String,
  activeTab: WorkroomTabRef,
  surface: WorkroomSurfaceState,
  lifecycle: WorkroomLifecycleState,
  decisionAct: WorkroomDecisionActState,
})
export type Model = typeof Model.Type

export const init = (workroomId: string, activeTab: WorkroomTabRef): Model => ({
  activeTab,
  decisionAct: WorkroomDecisionActIdle(),
  lifecycle: WorkroomLifecycleIdle(),
  surface: WorkroomSurfaceIdle(),
  workroomId,
})

// -----------------------------------------------------------------------------
// Messages (RequestedLoad... / Succeeded... / Failed... / Submitted...).
// -----------------------------------------------------------------------------

export const SelectedWorkroomTab = m('SelectedWorkroomTab', {
  tab: WorkroomTabRef,
})
export const RequestedLoadWorkroomSurface = m('RequestedLoadWorkroomSurface')
export const SucceededLoadWorkroomSurface = m('SucceededLoadWorkroomSurface', {
  response: OmniWorkroomSurfaceResponse,
})
export const FailedLoadWorkroomSurface = m('FailedLoadWorkroomSurface', {
  error: S.String,
})
export const RequestedLoadWorkroomLifecycle = m(
  'RequestedLoadWorkroomLifecycle',
)
export const SucceededLoadWorkroomLifecycle = m(
  'SucceededLoadWorkroomLifecycle',
  { response: OmniLifecycleHistoryResponse },
)
export const FailedLoadWorkroomLifecycle = m('FailedLoadWorkroomLifecycle', {
  error: S.String,
})
export const SubmittedWorkroomLifecycleDecision = m(
  'SubmittedWorkroomLifecycleDecision',
  {
    customerSafeExplanationRef: S.String,
    decisionKind: OmniLifecycleDecisionKind,
    receiptRef: S.String,
    workKind: S.String,
  },
)
export const SucceededWorkroomLifecycleDecision = m(
  'SucceededWorkroomLifecycleDecision',
  { response: OmniLifecycleDecisionResponse },
)
export const FailedWorkroomLifecycleDecision = m(
  'FailedWorkroomLifecycleDecision',
  { error: S.String },
)

export const Msg = S.Union([
  SelectedWorkroomTab,
  RequestedLoadWorkroomSurface,
  SucceededLoadWorkroomSurface,
  FailedLoadWorkroomSurface,
  RequestedLoadWorkroomLifecycle,
  SucceededLoadWorkroomLifecycle,
  FailedLoadWorkroomLifecycle,
  SubmittedWorkroomLifecycleDecision,
  SucceededWorkroomLifecycleDecision,
  FailedWorkroomLifecycleDecision,
])
export type Msg = typeof Msg.Type

// -----------------------------------------------------------------------------
// Commands. The submodel emits a small command ADT instead of issuing fetches
// directly so update() stays pure and unit-testable. The coordinator maps these
// to foldkit Command.define handlers (see runCommand for the request shapes,
// including the required Idempotency-Key header on POST).
// -----------------------------------------------------------------------------

export type Cmd =
  | Readonly<{ _tag: 'None' }>
  | Readonly<{ _tag: 'LoadSurface'; workroomId: string }>
  | Readonly<{ _tag: 'LoadLifecycle'; workroomId: string }>
  | Readonly<{
      _tag: 'SubmitLifecycleDecision'
      customerSafeExplanationRef: string
      decisionKind: OmniLifecycleDecisionKind
      idempotencyKey: string
      receiptRef: string
      workKind: string
      workroomId: string
    }>

const cmdNone: Cmd = { _tag: 'None' }

const decisionIdempotencyKey = (
  workroomId: string,
  decisionKind: OmniLifecycleDecisionKind,
): string => `browser-omni-lifecycle:${workroomId}:${decisionKind}`

// -----------------------------------------------------------------------------
// Update
// -----------------------------------------------------------------------------

export const update = (model: Model, message: Msg): readonly [Model, Cmd] =>
  M.value(message).pipe(
    M.withReturnType<readonly [Model, Cmd]>(),
    M.tags({
      SelectedWorkroomTab: ({ tab }) => [{ ...model, activeTab: tab }, cmdNone],
      RequestedLoadWorkroomSurface: () => [
        {
          ...model,
          lifecycle: WorkroomLifecycleLoading(),
          surface: WorkroomSurfaceLoading(),
        },
        { _tag: 'LoadSurface', workroomId: model.workroomId },
      ],
      SucceededLoadWorkroomSurface: ({ response }) => [
        { ...model, surface: WorkroomSurfaceLoaded({ response }) },
        cmdNone,
      ],
      FailedLoadWorkroomSurface: ({ error }) => [
        { ...model, surface: WorkroomSurfaceFailed({ error }) },
        cmdNone,
      ],
      RequestedLoadWorkroomLifecycle: () => [
        { ...model, lifecycle: WorkroomLifecycleLoading() },
        { _tag: 'LoadLifecycle', workroomId: model.workroomId },
      ],
      SucceededLoadWorkroomLifecycle: ({ response }) => [
        { ...model, lifecycle: WorkroomLifecycleLoaded({ response }) },
        cmdNone,
      ],
      FailedLoadWorkroomLifecycle: ({ error }) => [
        { ...model, lifecycle: WorkroomLifecycleFailed({ error }) },
        cmdNone,
      ],
      SubmittedWorkroomLifecycleDecision: ({
        customerSafeExplanationRef,
        decisionKind,
        receiptRef,
        workKind,
      }) => [
        { ...model, decisionAct: WorkroomDecisionActSubmitting({ decisionKind }) },
        {
          _tag: 'SubmitLifecycleDecision',
          customerSafeExplanationRef,
          decisionKind,
          idempotencyKey: decisionIdempotencyKey(model.workroomId, decisionKind),
          receiptRef,
          workKind,
          workroomId: model.workroomId,
        },
      ],
      SucceededWorkroomLifecycleDecision: ({ response }) => [
        {
          ...model,
          decisionAct: WorkroomDecisionActSucceeded({ response }),
          lifecycle: WorkroomLifecycleLoading(),
        },
        { _tag: 'LoadLifecycle', workroomId: model.workroomId },
      ],
      FailedWorkroomLifecycleDecision: ({ error }) => [
        { ...model, decisionAct: WorkroomDecisionActFailed({ error }) },
        cmdNone,
      ],
    }),
    M.exhaustive,
  )

// -----------------------------------------------------------------------------
// Command runner. The coordinator wires these into foldkit Command.define using
// requestJson (apps/web/src/page/loggedIn/commands/api.ts). Kept here so the
// exact request shapes (paths, surface query, Idempotency-Key header) live with
// the page that owns them. This function is provided for reference and reuse by
// the coordinator's transition wiring.
// -----------------------------------------------------------------------------

export const surfaceRequestInfo = (
  workroomId: string,
): Readonly<{ init: RequestInit; request: string }> => ({
  init: {
    cache: 'no-store',
    credentials: 'include',
    headers: { accept: 'application/json' },
  },
  request: `/api/omni/workrooms/${encodeURIComponent(workroomId)}?surface=customer`,
})

export const lifecycleRequestInfo = (
  workroomId: string,
): Readonly<{ init: RequestInit; request: string }> => ({
  init: {
    cache: 'no-store',
    credentials: 'include',
    headers: { accept: 'application/json' },
  },
  request: `/api/omni/workrooms/${encodeURIComponent(workroomId)}/lifecycle-decisions?audience=customer`,
})

export const lifecycleDecisionRequestInfo = (input: {
  customerSafeExplanationRef: string
  decisionKind: OmniLifecycleDecisionKind
  idempotencyKey: string
  receiptRef: string
  workKind: string
  workroomId: string
}): Readonly<{ init: RequestInit; request: string }> => ({
  init: {
    body: JSON.stringify({
      actorKind: 'customer',
      customerSafeExplanationRef: input.customerSafeExplanationRef,
      decisionKind: input.decisionKind,
      idempotencyKey: input.idempotencyKey,
      receiptRef: input.receiptRef,
      workKind: input.workKind,
    }),
    cache: 'no-store',
    credentials: 'include',
    headers: {
      accept: 'application/json',
      'content-type': 'application/json',
      'idempotency-key': input.idempotencyKey,
    },
    method: 'POST',
  },
  request: `/api/omni/workrooms/${encodeURIComponent(input.workroomId)}/lifecycle-decisions`,
})

// -----------------------------------------------------------------------------
// View helpers
// -----------------------------------------------------------------------------

type Tone = 'accent' | 'positive' | 'warning' | 'negative' | 'info'

const statusTone = (status: string | undefined): Tone =>
  M.value(status ?? '').pipe(
    M.when('accepted', () => 'positive' as const),
    M.when('delivered', () => 'positive' as const),
    M.when('closed', () => 'positive' as const),
    M.when('blocked', () => 'negative' as const),
    M.when('unavailable', () => 'negative' as const),
    M.when('active', () => 'accent' as const),
    M.orElse(() => 'info' as const),
  )

const humanize = (value: string): string => value.replaceAll('_', ' ')

const refChips = (refs: ReadonlyArray<string>): ReadonlyArray<Html> => {
  const h = html<Msg>()

  return refs.map(ref =>
    h.span(
      [
        Ui.className<Msg>(
          'min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap border border-[#222] px-2 py-1 text-xs text-white/55',
        ),
      ],
      [ref],
    ),
  )
}

const valueOrPending = (value: string | null | undefined): string =>
  value === null || value === undefined || value === '' ? 'Not recorded' : value

const metricStrip = (
  metrics: ReadonlyArray<Readonly<{ label: string; value: string }>>,
): Html => {
  const h = html<Msg>()

  return h.div(
    [
      Ui.className<Msg>(
        'grid gap-px overflow-hidden border border-[#222] bg-[#222] sm:grid-cols-2 xl:grid-cols-4',
      ),
    ],
    metrics.map(metric =>
      h.div([Ui.className<Msg>('grid gap-1 bg-[#080808] p-3')], [
        h.div([Ui.className<Msg>(Ui.eyebrowClass)], [metric.label]),
        h.div(
          [
            Ui.className<Msg>(
              'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm text-white/75',
            ),
          ],
          [metric.value],
        ),
      ]),
    ),
  )
}

const sectionHeading = (title: string, detail?: string): Html => {
  const h = html<Msg>()

  return h.div([Ui.className<Msg>('grid gap-1')], [
    h.h2(
      [Ui.className<Msg>('m-0 text-base font-medium text-white/85')],
      [title],
    ),
    detail === undefined
      ? hiddenSpan()
      : h.p([Ui.className<Msg>('m-0 text-sm/6 text-white/50')], [detail]),
  ])
}

const hiddenSpan = (): Html =>
  html<Msg>().span([Ui.className<Msg>('hidden')], [])

const placeholderTab = (label: string, detail: string): Html => {
  const h = html<Msg>()

  return h.div(
    [Ui.className<Msg>('grid gap-3 border border-[#222] bg-[#080808] p-4')],
    [sectionHeading(label, detail)],
  )
}

const refListSection = (
  title: string,
  refs: ReadonlyArray<string> | undefined,
  emptyText: string,
): Html => {
  const h = html<Msg>()
  const items = refs ?? []

  return h.div(
    [Ui.className<Msg>('grid gap-2 border border-[#222] bg-[#080808] p-4')],
    [
      h.div([Ui.className<Msg>(Ui.eyebrowClass)], [title]),
      items.length === 0
        ? h.p([Ui.className<Msg>('m-0 text-sm/6 text-white/45')], [emptyText])
        : h.div([Ui.className<Msg>('flex flex-wrap gap-2')], refChips(items)),
    ],
  )
}

// -----------------------------------------------------------------------------
// Tab views
// -----------------------------------------------------------------------------

const tabBarView = (model: Model): Html => {
  const h = html<Msg>()

  return h.div(
    [
      Ui.className<Msg>(
        'flex flex-wrap items-center justify-start gap-2 border-b border-[#222] pb-3',
      ),
    ],
    TABS.map(tab =>
      h.button(
        [
          h.Type('button'),
          h.OnClick(SelectedWorkroomTab({ tab: tab.ref })),
          Ui.className<Msg>(
            tab.ref === model.activeTab
              ? 'min-h-[30px] cursor-pointer border border-[#ffb400]/70 bg-[#080808] px-2.5 font-[inherit] text-[0.75rem] text-[#f1efe8]'
              : 'min-h-[30px] cursor-pointer border border-[#333] bg-transparent px-2.5 font-[inherit] text-[0.75rem] text-white/60 hover:bg-[#080808] hover:text-[#f1efe8]',
          ),
        ],
        [tab.label],
      ),
    ),
  )
}

const overviewTabView = (workroom: OmniWorkroomProjection): Html => {
  const h = html<Msg>()
  const detailRow = (label: string, value: string): Html =>
    h.div(
      [
        Ui.className<Msg>(
          'grid gap-1 border-b border-[#222] py-3 last:border-b-0 sm:grid-cols-[10rem_minmax(0,1fr)] sm:gap-4',
        ),
      ],
      [
        h.div([Ui.className<Msg>(Ui.eyebrowClass)], [label]),
        h.div(
          [Ui.className<Msg>('min-w-0 text-sm/6 text-white/75')],
          [value],
        ),
      ],
    )

  return h.div([Ui.className<Msg>('grid gap-3')], [
    h.div(
      [Ui.className<Msg>('flex flex-wrap items-center gap-2')],
      [
        h.span(
          [Ui.className<Msg>(Ui.statusDotClass(statusTone(workroom.status)))],
          [],
        ),
        h.span(
          [
            Ui.className<Msg>(
              'inline-flex min-h-7 items-center border border-[#333] px-2 text-[0.6875rem] uppercase text-white/65',
            ),
          ],
          [humanize(workroom.status ?? 'unknown')],
        ),
        workroom.workKind === undefined
          ? hiddenSpan()
          : h.span(
              [
                Ui.className<Msg>(
                  'inline-flex min-h-7 items-center border border-[#333] px-2 text-[0.6875rem] uppercase text-white/65',
                ),
              ],
              [humanize(workroom.workKind)],
            ),
        workroom.visibility === undefined
          ? hiddenSpan()
          : h.span(
              [
                Ui.className<Msg>(
                  'inline-flex min-h-7 items-center border border-[#333] px-2 text-[0.6875rem] uppercase text-white/65',
                ),
              ],
              [`${humanize(workroom.visibility)} visibility`],
            ),
      ],
    ),
    visibilityTierView(workroom.visibility),
    h.div(
      [Ui.className<Msg>('border-y border-[#222]')],
      [
        detailRow('Order', workroom.softwareOrderId ?? 'Not recorded'),
        detailRow(
          'Customer intent',
          workroom.customerIntentRef ?? 'Not recorded',
        ),
        detailRow('Data class', workroom.dataClassification ?? 'Not recorded'),
        detailRow('Trust tier', workroom.trustTier ?? 'Not recorded'),
        detailRow('Visibility', valueOrPending(workroom.visibility)),
        detailRow('Site', workroom.siteId ?? 'No site linked'),
        detailRow(
          'Accepted contract',
          valueOrPending(workroom.acceptedOutcomeContractId),
        ),
        detailRow(
          'Public receipt',
          workroom.publicReceiptRef ?? 'Not recorded',
        ),
      ],
    ),
    refListSection('Blockers', workroom.blockerRefs, 'No blockers recorded.'),
  ])
}

const visibilityTierView = (activeVisibility: string | undefined): Html => {
  const h = html<Msg>()
  const tiers = ['private', 'customer', 'team', 'public']

  return h.div(
    [
      Ui.className<Msg>(
        'grid gap-2 border border-[#222] bg-[#080808] p-4',
      ),
    ],
    [
      h.div([Ui.className<Msg>(Ui.eyebrowClass)], ['Visibility tiers']),
      h.div(
        [Ui.className<Msg>('grid gap-2 sm:grid-cols-4')],
        tiers.map(tier =>
          h.div(
            [
              Ui.className<Msg>(
                tier === activeVisibility
                  ? 'border border-[#ffb400]/70 bg-black p-3 text-[#f1efe8]'
                  : 'border border-[#222] bg-black p-3 text-white/50',
              ),
            ],
            [
              h.div(
                [Ui.className<Msg>('text-[0.6875rem] uppercase')],
                [humanize(tier)],
              ),
              h.div(
                [Ui.className<Msg>('mt-1 text-xs/5 text-white/45')],
                [
                  tier === activeVisibility
                    ? 'Current projection'
                    : 'Available policy tier',
                ],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

const evidenceEntryRow = (entry: OmniEvidenceEntryProjection): Html => {
  const h = html<Msg>()

  return h.div(
    [
      Ui.className<Msg>(
        'grid gap-2 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Msg>('flex flex-wrap items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Msg>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [humanize(entry.entryKind ?? 'evidence')],
          ),
          entry.visibility === undefined
            ? hiddenSpan()
            : h.span(
                [Ui.className<Msg>('text-xs text-white/40')],
                [humanize(entry.visibility)],
              ),
          entry.redactionState === undefined
            ? hiddenSpan()
            : h.span(
                [Ui.className<Msg>('text-xs text-white/40')],
                [humanize(entry.redactionState)],
              ),
        ],
      ),
      h.div(
        [
          Ui.className<Msg>(
            'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm text-white/70',
          ),
        ],
        [entry.ref ?? entry.summaryRef ?? 'evidence ref pending'],
      ),
      entry.caveatRef === undefined || entry.caveatRef === null
        ? hiddenSpan()
        : h.div([Ui.className<Msg>('text-xs text-white/45')], [
            entry.caveatRef,
          ]),
    ],
  )
}

const evidenceBundleRow = (bundle: OmniEvidenceBundleProjection): Html => {
  const h = html<Msg>()
  const entries = bundle.entries ?? []

  return h.div(
    [
      Ui.className<Msg>(
        'grid gap-2 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Msg>('flex flex-wrap items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Msg>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [humanize(bundle.status ?? 'bundle')],
          ),
          h.span(
            [
              Ui.className<Msg>(
                'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/60',
              ),
            ],
            [bundle.id ?? bundle.publicReceiptRef ?? 'bundle'],
          ),
        ],
      ),
      bundle.summaryRef === undefined
        ? hiddenSpan()
        : h.p(
            [Ui.className<Msg>('m-0 text-sm/6 text-white/55')],
            [bundle.summaryRef],
          ),
      bundle.artifactRefs === undefined || bundle.artifactRefs.length === 0
        ? hiddenSpan()
        : h.div(
            [Ui.className<Msg>('flex flex-wrap gap-2')],
            refChips(bundle.artifactRefs),
          ),
      entries.length === 0
        ? hiddenSpan()
        : h.div(
            [Ui.className<Msg>('border-y border-[#222]')],
            entries.map(evidenceEntryRow),
          ),
    ],
  )
}

const assetsTabView = (response: OmniWorkroomSurfaceResponse): Html => {
  const h = html<Msg>()
  const bundles = response.evidenceBundles ?? []

  return h.div([Ui.className<Msg>('grid gap-3')], [
    sectionHeading(
      'Assets',
      'Evidence and proof bundle references attached to this delivery.',
    ),
    refListSection(
      'Artifacts',
      response.workroom.artifactRefs,
      'No artifacts recorded yet.',
    ),
    h.div(
      [Ui.className<Msg>('grid gap-2 border border-[#222] bg-[#080808] p-4')],
      [
        h.div([Ui.className<Msg>(Ui.eyebrowClass)], ['Evidence bundles']),
        bundles.length === 0
          ? h.p(
              [Ui.className<Msg>('m-0 text-sm/6 text-white/45')],
              ['No evidence bundles recorded yet.'],
            )
          : h.div(
              [Ui.className<Msg>('border-y border-[#222]')],
              bundles.map(evidenceBundleRow),
            ),
      ],
    ),
  ])
}

const handoffTabView = (response: OmniWorkroomSurfaceResponse): Html => {
  const h = html<Msg>()

  return h.div([Ui.className<Msg>('grid gap-3')], [
    sectionHeading(
      'Handoff',
      'Receipts and proof references that travel with the delivered work.',
    ),
    refListSection(
      'Receipts',
      response.workroom.receiptRefs,
      'No receipts recorded yet.',
    ),
    refListSection(
      'Sources',
      response.workroom.sourceRefs,
      'No source references recorded yet.',
    ),
    response.workroom.publicReceiptRef === undefined
      ? hiddenSpan()
      : refListSection(
          'Public receipt',
          [response.workroom.publicReceiptRef],
          'No public receipt.',
        ),
  ])
}

const routeScorecardsView = (
  scorecards: ReadonlyArray<OmniRouteScorecardProjection>,
): Html => {
  const h = html<Msg>()

  return h.div([Ui.className<Msg>('grid gap-3')], [
    sectionHeading(
      'Route scorecards',
      'Selected route, runtime, trust, and observed closeout signals.',
    ),
    scorecards.length === 0
      ? h.p([Ui.className<Msg>('m-0 text-sm/6 text-white/45')], [
          'No route scorecards recorded yet.',
        ])
      : h.div(
          [Ui.className<Msg>('grid gap-3')],
          scorecards.map(scorecard =>
            h.div(
              [
                Ui.className<Msg>(
                  'grid gap-3 border border-[#222] bg-[#080808] p-4',
                ),
              ],
              [
                metricStrip([
                  {
                    label: 'Result',
                    value: valueOrPending(scorecard.observedResultKind),
                  },
                  {
                    label: 'Trust',
                    value: valueOrPending(scorecard.trustTier),
                  },
                  {
                    label: 'Privacy',
                    value: valueOrPending(scorecard.privacyTier),
                  },
                  {
                    label: 'Runtime',
                    value: valueOrPending(scorecard.selectedRuntimeRef),
                  },
                ]),
                refListSection(
                  'Decision refs',
                  scorecard.decisionReasonRefs,
                  'No decision refs recorded.',
                ),
                refListSection(
                  'Observed result',
                  scorecard.observedResultRef === undefined
                    ? []
                    : [scorecard.observedResultRef],
                  'No observed result ref.',
                ),
                refListSection(
                  'Public caveat',
                  scorecard.publicCaveatRef === undefined
                    ? []
                    : [scorecard.publicCaveatRef],
                  'No public caveat.',
                ),
              ],
            ),
          ),
        ),
  ])
}

const economicsView = (
  economics: ReadonlyArray<OmniEconomicsProjection>,
): Html => {
  const h = html<Msg>()

  return h.div([Ui.className<Msg>('grid gap-3')], [
    sectionHeading(
      'Economics',
      'Funding and caveat refs are informational here; this page does not settle or pay out.',
    ),
    economics.length === 0
      ? h.p([Ui.className<Msg>('m-0 text-sm/6 text-white/45')], [
          'No economics records projected yet.',
        ])
      : h.div(
          [Ui.className<Msg>('grid gap-3')],
          economics.map(record =>
            h.div(
              [
                Ui.className<Msg>(
                  'grid gap-3 border border-[#222] bg-[#080808] p-4',
                ),
              ],
              [
                metricStrip([
                  {
                    label: 'Funding',
                    value: valueOrPending(record.fundingMode),
                  },
                  {
                    label: 'Work kind',
                    value: valueOrPending(record.workKind),
                  },
                  {
                    label: 'Settlement',
                    value:
                      record.noSettlementImplication === true
                        ? 'No settlement implication'
                        : 'Not recorded',
                  },
                  {
                    label: 'Workroom',
                    value: valueOrPending(record.workroomId),
                  },
                ]),
                refListSection(
                  'Public caveat',
                  record.publicCaveatRef === undefined
                    ? []
                    : [record.publicCaveatRef],
                  'No public caveat.',
                ),
              ],
            ),
          ),
        ),
  ])
}

const operationsTabView = (response: OmniWorkroomSurfaceResponse): Html => {
  const h = html<Msg>()

  return h.div([Ui.className<Msg>('grid gap-5')], [
    routeScorecardsView(response.routeScorecards ?? []),
    economicsView(response.economics ?? []),
  ])
}

const decisionActStatusView = (model: Model): Html => {
  const h = html<Msg>()

  return M.value(model.decisionAct).pipe(
    M.tags({
      WorkroomDecisionActIdle: () => hiddenSpan(),
      WorkroomDecisionActSubmitting: ({ decisionKind }) =>
        h.p(
          [Ui.className<Msg>('m-0 text-sm text-white/45')],
          [`Recording ${humanize(decisionKind)}...`],
        ),
      WorkroomDecisionActSucceeded: () =>
        h.p(
          [Ui.className<Msg>('m-0 text-sm text-[#7ccf8a]')],
          ['Decision recorded.'],
        ),
      WorkroomDecisionActFailed: ({ error }) =>
        h.p([Ui.className<Msg>('m-0 text-sm text-[#ff8a80]')], [error]),
    }),
    M.exhaustive,
  )
}

const decisionRow = (decision: OmniLifecycleDecisionProjection): Html => {
  const h = html<Msg>()

  return h.div(
    [
      Ui.className<Msg>(
        'grid gap-2 border-b border-[#222] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Msg>('flex flex-wrap items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Msg>(
                'inline-flex border border-[#333] px-2 py-1 text-[0.6875rem] uppercase text-white/55',
              ),
            ],
            [humanize(decision.decisionKind ?? 'decision')],
          ),
          decision.resultingState === undefined
            ? hiddenSpan()
            : h.span(
                [Ui.className<Msg>('text-xs text-white/40')],
                [humanize(decision.resultingState)],
              ),
        ],
      ),
      h.p(
        [Ui.className<Msg>('m-0 text-sm/6 text-white/70')],
        [decision.customerSafeExplanationRef ?? 'No explanation reference.'],
      ),
    ],
  )
}

const APPROVAL_ACTIONS: ReadonlyArray<
  Readonly<{
    kind: OmniLifecycleDecisionKind
    label: string
    variant: Ui.ButtonVariant
  }>
> = [
  { kind: 'accept', label: 'Approve', variant: 'primary' },
  { kind: 'request_revision', label: 'Request revision', variant: 'secondary' },
  { kind: 'reject', label: 'Reject', variant: 'danger' },
]

const approvalsTabView = (model: Model): Html => {
  const h = html<Msg>()
  const submitting = model.decisionAct._tag === 'WorkroomDecisionActSubmitting'
  const workKind =
    model.surface._tag === 'WorkroomSurfaceLoaded'
      ? (model.surface.response.workroom.workKind ?? 'software_delivery')
      : 'software_delivery'
  const receiptRef =
    model.surface._tag === 'WorkroomSurfaceLoaded'
      ? (model.surface.response.workroom.publicReceiptRef ??
        `receipt:${model.workroomId}`)
      : `receipt:${model.workroomId}`

  const decisionsBody = M.value(model.lifecycle).pipe(
    M.tags({
      WorkroomLifecycleIdle: () => [
        h.p(
          [Ui.className<Msg>('m-0 text-sm/6 text-white/45')],
          ['Lifecycle history has not loaded.'],
        ),
      ],
      WorkroomLifecycleLoading: () => [
        h.p(
          [Ui.className<Msg>('m-0 text-sm/6 text-white/45')],
          ['Loading lifecycle history...'],
        ),
      ],
      WorkroomLifecycleFailed: ({ error }) => [
        h.p([Ui.className<Msg>('m-0 text-sm/6 text-[#ff8a80]')], [error]),
      ],
      WorkroomLifecycleLoaded: ({ response }) =>
        response.decisions.length === 0
          ? [
              h.p(
                [Ui.className<Msg>('m-0 text-sm/6 text-white/45')],
                ['No lifecycle decisions recorded yet.'],
              ),
            ]
          : [
              h.div(
                [Ui.className<Msg>('border-y border-[#222]')],
                response.decisions.map(decisionRow),
              ),
            ],
    }),
    M.orElse(() => [hiddenSpan()]),
  )

  return h.div([Ui.className<Msg>('grid gap-4')], [
    sectionHeading(
      'Approvals',
      'Record a customer-safe lifecycle decision for this delivery. The server enforces no direct settlement effect.',
    ),
    h.div(
      [Ui.className<Msg>('flex flex-wrap gap-2')],
      APPROVAL_ACTIONS.map(action =>
        Ui.button<Msg>({
          attrs: [
            h.Type('button'),
            ...(submitting
              ? [h.Disabled(true)]
              : [
                  h.OnClick(
                    SubmittedWorkroomLifecycleDecision({
                      customerSafeExplanationRef: `explanation.${action.kind}`,
                      decisionKind: action.kind,
                      receiptRef,
                      workKind,
                    }),
                  ),
                ]),
          ],
          label: action.label,
          size: 'sm',
          variant: action.variant,
        }),
      ),
    ),
    decisionActStatusView(model),
    h.div(
      [Ui.className<Msg>('grid gap-2 border border-[#222] bg-[#080808] p-4')],
      [
        h.div([Ui.className<Msg>(Ui.eyebrowClass)], ['Decision history']),
        ...decisionsBody,
      ],
    ),
  ])
}

const loadedTabView = (
  model: Model,
  response: OmniWorkroomSurfaceResponse,
): Html =>
  M.value(model.activeTab).pipe(
    M.when('overview', () => overviewTabView(response.workroom)),
    M.when('assets', () => assetsTabView(response)),
    M.when('handoff', () => handoffTabView(response)),
    M.when('approvals', () => approvalsTabView(model)),
    M.when('site', () => operationsTabView(response)),
    M.orElse(() =>
      placeholderTab(
        tabLabel(model.activeTab),
        `Accumulated ${tabLabel(model.activeTab)} state will project here from the workroom surface.`,
      ),
    ),
  )

const surfaceBody = (model: Model): Html => {
  const h = html<Msg>()

  // Approvals tab is usable even before the surface loads.
  if (model.activeTab === 'approvals') {
    return approvalsTabView(model)
  }

  return M.value(model.surface).pipe(
    M.tags({
      WorkroomSurfaceIdle: () =>
        h.p(
          [Ui.className<Msg>('m-0 text-sm text-white/45')],
          ['Workroom has not loaded.'],
        ),
      WorkroomSurfaceLoading: () =>
        h.p(
          [Ui.className<Msg>('m-0 text-sm text-white/45')],
          ['Loading workroom...'],
        ),
      WorkroomSurfaceFailed: ({ error }) =>
        h.p([Ui.className<Msg>('m-0 text-sm text-[#ff8a80]')], [error]),
      WorkroomSurfaceLoaded: ({ response }) => loadedTabView(model, response),
    }),
    M.exhaustive,
  )
}

// -----------------------------------------------------------------------------
// View (self-contained Msg). The coordinator's embeddedView re-maps the shared
// logged-in Message to this submodel's Msg; see COORDINATOR WIRING above.
// -----------------------------------------------------------------------------

export const view = (model: Model): Html => {
  const h = html<Msg>()

  return h.section([Ui.className<Msg>('grid gap-4')], [
    h.div(
      [Ui.className<Msg>('flex flex-wrap items-end justify-between gap-3')],
      [
        h.div([Ui.className<Msg>('grid gap-1')], [
          h.h1(
            [Ui.className<Msg>('m-0 text-2xl font-semibold text-white')],
            ['Workroom'],
          ),
          h.p(
            [
              Ui.className<Msg>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm/6 text-white/50',
              ),
            ],
            [model.workroomId],
          ),
        ]),
        Ui.button<Msg>({
          attrs: [h.Type('button'), h.OnClick(RequestedLoadWorkroomSurface())],
          label: 'Refresh',
          size: 'sm',
          variant: 'secondary',
        }),
      ],
    ),
    tabBarView(model),
    surfaceBody(model),
  ])
}
