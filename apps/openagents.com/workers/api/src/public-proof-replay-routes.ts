import {
  buildProofReplayBundleFromPublicActivityTimeline,
  type ProofReplayBundle as SharedProofReplayBundle,
} from '@openagentsinc/proof-replay'
import type {
  PublicActivityTimelineEnvelope,
  PublicActivityTimelineEvent,
} from '@openagentsinc/public-activity-timeline'
import {
  buildPublicActivityTimelineEnvelope,
  publicActivityTimelineQueryFromUrl,
  type PublicActivityTimelineQuery,
} from './public-activity-timeline'
import {
  makeD1PublicActivityTimelineArtanisStore,
  makeD1PublicActivityTimelineCapacityStore,
  makeD1PublicActivityTimelineForumStore,
} from './public-activity-timeline-routes'
import {
  buildPublicTassadarRunSummaryEnvelope,
  DEFAULT_TASSADAR_RUN_REF,
  type PublicTassadarSettlementRow,
} from './public-tassadar-run-summary-routes'
import { liveAtReadStaleness } from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import type { NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  makeD1NexusTreasuryPayoutLedgerStore,
} from './nexus-treasury-payout-ledger'
import {
  makeD1TrainingAuthorityStore,
  type TrainingAuthorityStore,
} from './training-run-window-authority'
import { makeD1PylonApiStore } from './pylon-api'
import { noStoreJsonResponse } from './http/responses'

export const ProofReplayBundleSchemaVersion = 'proof_replay_bundle.v1'
export const FIRST_REAL_SETTLEMENT_BUNDLE_SLUG = 'first-real-settlement'
export const FIRST_REAL_SETTLEMENT_TITLE =
  'Tassadar Run 1: First Real Bitcoin Settlement'
export const FIRST_REAL_SETTLEMENT_LOCAL_DISPLAY_TIME = '8:38pm, June 17'
export const FIRST_REAL_SETTLEMENT_OBSERVED_AT = '2026-06-18T01:38:00.000Z'

export const FIRST_REAL_SETTLEMENT_RECEIPT_REF =
  'receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618'
export const FIRST_REAL_SETTLEMENT_CHALLENGE_REF =
  'training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c'
export const FIRST_REAL_SETTLEMENT_WINDOW_REF =
  'training.window.tassadar.executor.20260615.w1'
export const FIRST_REAL_SETTLEMENT_CONTRIBUTOR_REF =
  'pylon.448ba824b5fc879f3a59'
export const FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL =
  'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-1dce5715-ec37-4850-a484-e7fe329417aa'
export const FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL =
  'https://openagents.com/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9#post-a8df2265-547a-4a18-9398-3e7412a6859a'
export const LAUNCH_RECOGNITION_BUNDLE_SLUG = 'launch-recognition-payments'
export const LAUNCH_RECOGNITION_TITLE =
  'Launch Recognition Payments: Trigger, Whitefang, Orrery'
export const LAUNCH_RECOGNITION_LOCAL_DISPLAY_TIME = 'June 17 closeout'
export const LAUNCH_ROADMAP_DOC_URL =
  'https://github.com/OpenAgentsInc/openagents/blob/main/docs/launch/JUNE17_ROADMAP.md'
export const LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL =
  'https://github.com/OpenAgentsInc/openagents/blob/main/docs/payments/2026-06-17-launch-recognition-closeout.md'
export const LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL =
  'https://github.com/OpenAgentsInc/openagents/blob/main/docs/payments/2026-06-17-launch-recognition-spark-recipient-status.md'

const PUBLIC_PROOF_REPLAY_METHODS = ['GET', 'HEAD', 'OPTIONS'] as const

const publicProofReplayCorsHeaders = (headers: Headers): Headers => {
  headers.set('access-control-allow-origin', '*')
  headers.set(
    'access-control-allow-methods',
    PUBLIC_PROOF_REPLAY_METHODS.join(', '),
  )
  headers.set('access-control-allow-headers', 'accept, content-type')
  headers.set('access-control-max-age', '86400')
  return headers
}

export const publicProofReplayJsonResponse = (
  value: unknown,
  init: ResponseInit = {},
) => {
  const headers = publicProofReplayCorsHeaders(new Headers(init.headers))
  return noStoreJsonResponse(value, { ...init, headers })
}

export const publicProofReplayOptionsResponse = () => {
  const headers = publicProofReplayCorsHeaders(
    new Headers({ 'cache-control': 'no-store' }),
  )
  return new Response(null, { headers, status: 204 })
}

const publicProofReplayMethodNotAllowedResponse = () =>
  publicProofReplayJsonResponse(
    { error: 'method_not_allowed' },
    {
      headers: { allow: PUBLIC_PROOF_REPLAY_METHODS.join(', ') },
      status: 405,
    },
  )

type SourceKind =
  | 'api'
  | 'doc'
  | 'forum_post'
  | 'pylon'
  | 'receipt'
  | 'run'
  | 'window'
  | 'verification_challenge'
  | 'payment_authority'
  | 'recipient_confirmation'
  | 'operator_context'

type ReplayEventKind =
  | 'actor_entered_region'
  | 'proof_submitted'
  | 'proof_verified'
  | 'claim_boundary_shown'
  | 'settlement_blocked_closed'
  | 'payout_intent_persisted'
  | 'settlement_recorded'
  | 'payment_zap_confirmed'
  | 'payment_zap_simulated'
  | 'recognition_reward_recorded'
  | 'recipient_confirmation_recorded'
  | 'overpayment_detected'
  | 'forum_announcement_posted'

type ReplaySourceRef = Readonly<{
  ref: string
  kind: SourceKind
  url?: string
  observedAt?: string
}>

type ReplayActor = Readonly<{
  actorRef: string
  avatarRole:
    | 'contributor'
    | 'validator'
    | 'settlement_terminal'
    | 'recognition_terminal'
    | 'recognition_lane'
    | 'overpayment_branch'
    | 'operator_gate'
    | 'announcer'
    | 'recipient'
  displayName: string
  pylonRef?: string
  fallbackAssetId: string
}>

type ReplayStage = Readonly<{
  stageRef: string
  stageKind:
    | 'run_core'
    | 'pylon_station'
    | 'proof_gate'
    | 'settlement_terminal'
    | 'recognition_terminal'
    | 'recognition_lane'
    | 'overpayment_branch'
    | 'registry_marker'
    | 'replay_gap'
  label: string
  sourceRefs: ReadonlyArray<string>
}>

export type ReplayEvent = Readonly<{
  eventRef: string
  kind: ReplayEventKind
  sequenceIndex: number
  timelineSecond: number
  observedAt?: string
  actorRefs: ReadonlyArray<string>
  targetRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  displayText: string
  stateBefore?: string
  stateAfter?: string
  amountSats?: number
  rail?: string
  caveat?: string
}>

type ReplayFlow = Readonly<{
  flowRef: string
  flowKind:
    | 'work_handoff'
    | 'verification_check'
    | 'receipt_emission'
    | 'payment_movement'
    | 'recognition_reward'
    | 'overpayment_branch'
    | 'pending_marker'
    | 'simulation_marker'
  fromRef: string
  toRef: string
  sourceRefs: ReadonlyArray<string>
  amountSats?: number
  rail?: string
}>

type ReplayCameraCue = Readonly<{
  cueRef: string
  mode:
    | 'overview'
    | 'follow_actor'
    | 'orbit_proof'
    | 'zap_focus'
    | 'director_track'
  startSecond: number
  durationSecond: number
  focusRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

type ReplayCaption = Readonly<{
  captionRef: string
  sequenceIndex: number
  timelineSecond: number
  text: string
  sourceRefs: ReadonlyArray<string>
}>

type ReplayGap = Readonly<{
  gapRef: string
  reason: string
  affectedRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

export type ProofReplayBundle = Readonly<{
  bundleRef: string
  schemaVersion: typeof ProofReplayBundleSchemaVersion
  generatedAt: string
  title: string
  socialDisplayTime: string
  sourceRefs: ReadonlyArray<ReplaySourceRef>
  sourceAuthority: 'worker_d1_public'
  staleness: ReturnType<typeof liveAtReadStaleness>
  privacyLevel: 'public_safe'
  claimScope: 'evidence_presentation_only'
  actors: ReadonlyArray<ReplayActor>
  stages: ReadonlyArray<ReplayStage>
  events: ReadonlyArray<ReplayEvent>
  flows: ReadonlyArray<ReplayFlow>
  cameraCues: ReadonlyArray<ReplayCameraCue>
  captions: ReadonlyArray<ReplayCaption>
  gaps: ReadonlyArray<ReplayGap>
}>

type PublicTassadarSummaryEnvelope = Readonly<{
  runRef?: unknown
  settlementRows?: unknown
}>

type Deps = Readonly<{
  buildActivityTimelineEnvelope?: (
    query: PublicActivityTimelineQuery,
    request: Request,
    generatedAt: string,
  ) => Promise<PublicActivityTimelineEnvelope>
  makePayoutLedgerStore?: (
    env: Parameters<typeof openAgentsDatabase>[0],
  ) => NexusTreasuryPayoutLedgerStore
  makeStore?: (
    env: Parameters<typeof openAgentsDatabase>[0],
  ) => TrainingAuthorityStore
  now?: () => string
}>

class ProofReplayPublicProjectionUnsafe extends Error {
  override readonly name = 'ProofReplayPublicProjectionUnsafe'
}

class PublicProofReplayRequestError extends Error {
  readonly payload: Record<string, unknown>
  readonly status: number

  constructor(status: number, payload: Record<string, unknown>) {
    super(typeof payload.error === 'string' ? payload.error : 'bad_request')
    this.name = 'PublicProofReplayRequestError'
    this.payload = payload
    this.status = status
  }
}

const replayBundleStaleness = () =>
  liveAtReadStaleness([
    'training_run_state_transition_recorded',
    'training_window_state_transition_recorded',
    'training_verification_challenge_recorded',
    'nexus_payment_authority_receipt_recorded',
    'nexus_reconciliation_event_recorded',
  ])

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values.map(value => value.trim()).filter(value => value !== ''))]
    .sort()

const optionalRef = (value: string | null | undefined): ReadonlyArray<string> =>
  typeof value === 'string' && value.trim() !== '' ? [value] : []

const sourceKindForRef = (ref: string): SourceKind => {
  if (ref.startsWith('https://openagents.com/forum/')) {
    return 'forum_post'
  }

  if (ref.startsWith('https://openagents.com/api/')) {
    return 'api'
  }

  if (
    ref.startsWith('https://github.com/OpenAgentsInc/openagents/blob/main/docs/')
  ) {
    return 'doc'
  }

  if (ref.startsWith('receipt.')) {
    return 'receipt'
  }

  if (ref.startsWith('recipient_confirmation.')) {
    return 'recipient_confirmation'
  }

  if (
    ref.startsWith('payout_intent.') ||
    ref.startsWith('payout_attempt.') ||
    ref.startsWith('reconciliation.') ||
    ref.startsWith('external_event.') ||
    ref.startsWith('recognition_ledger.')
  ) {
    return 'payment_authority'
  }

  if (ref.startsWith('training.verification.challenge.')) {
    return 'verification_challenge'
  }

  if (ref.startsWith('training.window.')) {
    return 'window'
  }

  if (ref.startsWith('run.')) {
    return 'run'
  }

  if (ref.startsWith('pylon.')) {
    return 'pylon'
  }

  return 'operator_context'
}

const sourceRecord = (ref: string, observedAt?: string): ReplaySourceRef => ({
  kind: sourceKindForRef(ref),
  ref,
  ...(observedAt === undefined ? {} : { observedAt }),
  ...(ref.startsWith('https://') ? { url: ref } : {}),
})

const asSettlementRows = (
  value: unknown,
): ReadonlyArray<PublicTassadarSettlementRow> =>
  Array.isArray(value)
    ? value.filter((row): row is PublicTassadarSettlementRow => {
        const maybeRow = row as Partial<PublicTassadarSettlementRow>
        return (
          typeof maybeRow.receiptRef === 'string' &&
          typeof maybeRow.amountSats === 'number' &&
          typeof maybeRow.realBitcoinMoved === 'boolean' &&
          (maybeRow.movementMode === 'real_bitcoin' ||
            maybeRow.movementMode === 'simulation') &&
          typeof maybeRow.state === 'string' &&
          Array.isArray(maybeRow.sourceRefs)
        )
      })
    : []

const requestedRefsFor = (request: Request): ReadonlyArray<string> => {
  const url = new URL(request.url)
  return uniqueSorted([
    ...url.searchParams.getAll('refs').flatMap(refs => refs.split(',')),
    ...url.searchParams.getAll('ref'),
    ...url.searchParams.getAll('receiptRef'),
    ...url.searchParams.getAll('run'),
  ])
}

const requestedRunRefFor = (request: Request): string => {
  const runRef = new URL(request.url).searchParams.get('run')?.trim()
  return runRef === undefined || runRef === '' ? DEFAULT_TASSADAR_RUN_REF : runRef
}

const valuesFromParams = (
  searchParams: URLSearchParams,
  key: string,
): ReadonlyArray<string> =>
  searchParams
    .getAll(key)
    .flatMap(value => value.split(','))
    .map(value => value.trim())
    .filter(value => value !== '')

const isGeneratedActivityReplayRequest = (request: Request): boolean => {
  const url = new URL(request.url)
  const refs = valuesFromParams(url.searchParams, 'ref')
  return (
    url.searchParams.get('mode') === 'activity-timeline' ||
    url.searchParams.get('generated') === 'activity-timeline' ||
    refs.includes('activity-timeline') ||
    refs.includes('public-activity-timeline') ||
    url.searchParams.has('from') ||
    url.searchParams.has('to') ||
    url.searchParams.has('runRef') ||
    url.searchParams.has('windowRef') ||
    url.searchParams.has('actorRef') ||
    url.searchParams.has('kind')
  )
}

type GeneratedActivityReplayFilters = Readonly<{
  actorRefs: ReadonlyArray<string>
  runRefs: ReadonlyArray<string>
  windowRefs: ReadonlyArray<string>
}>

const generatedActivityReplayFiltersFromUrl = (
  url: URL,
): GeneratedActivityReplayFilters => ({
  actorRefs: valuesFromParams(url.searchParams, 'actorRef'),
  runRefs: uniqueSorted([
    ...valuesFromParams(url.searchParams, 'runRef'),
    ...valuesFromParams(url.searchParams, 'run').filter(ref =>
      ref.startsWith('run.') || ref.startsWith('training.run.'),
    ),
  ]),
  windowRefs: valuesFromParams(url.searchParams, 'windowRef'),
})

const generatedActivityReplayFilterMatches = (
  event: PublicActivityTimelineEvent,
  filters: GeneratedActivityReplayFilters,
): boolean => {
  if (
    filters.actorRefs.length > 0 &&
    (event.actorRef === undefined || !filters.actorRefs.includes(event.actorRef))
  ) {
    return false
  }
  if (
    filters.runRefs.length > 0 &&
    (event.runRef === undefined || !filters.runRefs.includes(event.runRef))
  ) {
    return false
  }
  if (
    filters.windowRefs.length > 0 &&
    (event.windowRef === undefined ||
      !filters.windowRefs.includes(event.windowRef))
  ) {
    return false
  }
  return true
}

const routeUrlWithQuery = (request: Request): string => {
  const url = new URL(request.url)
  url.pathname = '/api/public/activity-timeline'
  return url.toString()
}

const generatedActivityReplayManifest = (input: {
  envelope: PublicActivityTimelineEnvelope
  filters: GeneratedActivityReplayFilters
  request: Request
  timelineQuery: PublicActivityTimelineQuery
}) => ({
  authority: 'evidence_presentation_only',
  caveatRefs: [
    'caveat.public.proof_replay.generated_from_activity_timeline_observation_only',
  ],
  input: {
    actorRefs: input.filters.actorRefs,
    filterKinds: input.timelineQuery.filterKinds,
    filterSources: input.timelineQuery.filterSources,
    from: input.timelineQuery.from ?? null,
    limit: input.timelineQuery.limit,
    runRefs: input.filters.runRefs,
    since: input.timelineQuery.since ?? null,
    to: input.timelineQuery.to ?? null,
    windowRefs: input.filters.windowRefs,
  },
  route: '/api/public/proof-replays',
  schemaVersion: 'openagents.public_activity_generated_replay.v1',
  source: {
    route: '/api/public/activity-timeline',
    url: routeUrlWithQuery(input.request),
  },
  sourceLag: input.envelope.sourceLag,
  staleness: input.envelope.staleness,
})

const buildActivityTimelineEnvelopeForReplay = async (
  input: {
    deps: Deps
    env: Parameters<typeof openAgentsDatabase>[0]
    generatedAt: string
    query: PublicActivityTimelineQuery
    request: Request
  },
): Promise<PublicActivityTimelineEnvelope> => {
  if (input.deps.buildActivityTimelineEnvelope !== undefined) {
    return input.deps.buildActivityTimelineEnvelope(
      input.query,
      input.request,
      input.generatedAt,
    )
  }

  const db = openAgentsDatabase(input.env)
  return buildPublicActivityTimelineEnvelope({
    artanisStore: makeD1PublicActivityTimelineArtanisStore(db, input.generatedAt),
    capacityStore: makeD1PublicActivityTimelineCapacityStore(db),
    forumStore: makeD1PublicActivityTimelineForumStore(db),
    nowIso: () => input.generatedAt,
    pylonStore: makeD1PylonApiStore(db),
    query: input.query,
    receiptStore: makeD1NexusTreasuryPayoutLedgerStore(db),
    trainingStore: makeD1TrainingAuthorityStore(db),
  })
}

const buildGeneratedActivityReplayBundleForRequest = async (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
  deps: Deps,
  generatedAt: string,
): Promise<ProofReplayBundle> => {
  const url = new URL(request.url)
  if (!url.searchParams.has('from') || !url.searchParams.has('to')) {
    throw new PublicProofReplayRequestError(400, {
      error: 'generated_replay_requires_bounded_range',
      required: ['from', 'to'],
    })
  }
  const parsedQuery = publicActivityTimelineQueryFromUrl(url)
  if (parsedQuery instanceof Response) {
    throw new PublicProofReplayRequestError(parsedQuery.status, {
      error: 'invalid_activity_timeline_query',
    })
  }

  const filters = generatedActivityReplayFiltersFromUrl(url)
  const envelope = await buildActivityTimelineEnvelopeForReplay({
    deps,
    env,
    generatedAt,
    query: parsedQuery,
    request,
  })
  const filteredEnvelope: PublicActivityTimelineEnvelope = {
    ...envelope,
    events: envelope.events.filter(event =>
      generatedActivityReplayFilterMatches(event, filters),
    ),
  }
  const sharedBundle: SharedProofReplayBundle =
    buildProofReplayBundleFromPublicActivityTimeline(filteredEnvelope, {
      bundleRef: `proof_replay_bundle.public_activity.${stableHash(
        JSON.stringify({
          filters,
          from: parsedQuery.from,
          kind: parsedQuery.filterKinds,
          nextCursor: filteredEnvelope.nextCursor,
          since: parsedQuery.since,
          to: parsedQuery.to,
        }),
      )}`,
      generatedAt,
      origin: url.origin,
      sourceAuthority: 'worker_d1_public',
      title: 'Generated Public Activity Replay',
    })
  const manifest = generatedActivityReplayManifest({
    envelope: filteredEnvelope,
    filters,
    request,
    timelineQuery: parsedQuery,
  })
  const sourceRefs = [
    ...sharedBundle.sourceRefs,
    sourceRecord(routeUrlWithQuery(request), generatedAt),
  ]
  const bundle = {
    ...sharedBundle,
    generatedFrom: manifest,
    socialDisplayTime: 'Generated from public activity timeline',
    sourceAuthority: 'worker_d1_public',
    sourceRefs,
    staleness: filteredEnvelope.staleness,
  } as unknown as ProofReplayBundle

  assertPublicSafe(bundle)
  return bundle
}

const isLaunchRecognitionReplayRequest = (
  refs: ReadonlyArray<string>,
): boolean =>
  refs.some(
    ref =>
      ref === LAUNCH_RECOGNITION_BUNDLE_SLUG ||
      ref === 'launch-recognition' ||
      ref === 'recognition-payments' ||
      ref === 'recognition_ledger.launch.june17.closeout.v1',
  )

const selectRealSettlementRow = (
  rows: ReadonlyArray<PublicTassadarSettlementRow>,
  requestedRefs: ReadonlyArray<string>,
): PublicTassadarSettlementRow | undefined =>
  rows.find(row => requestedRefs.includes(row.receiptRef) && row.realBitcoinMoved) ??
  rows.find(row => row.receiptRef === FIRST_REAL_SETTLEMENT_RECEIPT_REF) ??
  rows.find(row => row.realBitcoinMoved)

const privateMaterialPatterns = [
  /\b(?:lnbc|lntb|lnbcrt|lno1)[a-z0-9]{12,}/i,
  /\bspark1[a-z0-9]{12,}/i,
  /\bbc1[ac-hj-np-z02-9]{20,}/i,
  /\bxprv[a-z0-9]{12,}/i,
  /mnemonic/i,
  /preimage/i,
  /bolt11/i,
  /spark[_-]?api[_-]?key/i,
  /breez[_-]?api[_-]?key/i,
  /service[_-]?token/i,
  /bearer\s+[a-z0-9._-]{12,}/i,
  /payment[_-]?hash/i,
  /provider[_-]?payload/i,
  /raw[_-]?prompt/i,
  /private[_-]?log/i,
  /wallet[_-]?path/i,
  /customer[_-]?(?:data|email|record)/i,
]

const assertPublicSafe = (bundle: Omit<ProofReplayBundle, 'bundleRef'>): void => {
  const serialized = JSON.stringify(bundle)
  const matchedPattern = privateMaterialPatterns.find(pattern =>
    pattern.test(serialized),
  )

  if (matchedPattern !== undefined) {
    throw new ProofReplayPublicProjectionUnsafe(
      `proof replay bundle rejected private material pattern ${matchedPattern}`,
    )
  }
}

const stableHash = (value: string): string => {
  const hash = [...value].reduce(
    (accumulator, char) =>
      Math.imul(accumulator ^ char.charCodeAt(0), 16_777_619) >>> 0,
    2_166_136_261,
  )
  return hash.toString(16).padStart(8, '0')
}

const canonicalReceiptApiUrl = (
  appUrl: string,
  receiptRef: string,
): string =>
  `${appUrl}/api/public/nexus-pylon/receipts/${encodeURIComponent(receiptRef)}`

const refFromSourceRefs = (
  refs: ReadonlyArray<string>,
  prefix: string,
  fallback: string,
): string => refs.find(ref => ref.startsWith(prefix)) ?? fallback

const makeEvent = (
  input: Omit<ReplayEvent, 'eventRef'>,
): ReplayEvent => ({
  ...input,
  eventRef: `proof_replay_event.tassadar.first_real_settlement.${String(input.sequenceIndex).padStart(2, '0')}.${input.kind}`,
})

const makeLaunchRecognitionEvent = (
  input: Omit<ReplayEvent, 'eventRef'>,
): ReplayEvent => ({
  ...input,
  eventRef: `proof_replay_event.launch_recognition.${String(input.sequenceIndex).padStart(2, '0')}.${input.kind}`,
})

export const buildLaunchRecognitionReplayBundle = (
  input: Readonly<{
    appUrl: string
    generatedAt: string
    requestedRefs: ReadonlyArray<string>
  }>,
): ProofReplayBundle => {
  void input.appUrl
  const docs = [
    LAUNCH_ROADMAP_DOC_URL,
    LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
    LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
  ] as const
  const recognitionLedgerRef = 'recognition_ledger.launch.june17.closeout.v1'
  const triggerConfirmationRef =
    'recipient_confirmation.launch_recognition.trigger.visible_50000_sats'
  const whitefangConfirmationRef =
    'recipient_confirmation.launch_recognition.whitefang.visible_51030_sats'
  const orreryConfirmationRef =
    'recipient_confirmation.launch_recognition.orrery.visible_159239_sats'
  const hazardPayDecisionRef =
    'recognition_ledger.launch_recognition.orrery.hazard_pay_owner_decision'
  const whitefangHistoricalPendingRef =
    'recognition_ledger.launch_recognition.whitefang.historical_pending_snapshot'
  const orreryPendingRowsRef =
    'recognition_ledger.launch_recognition.orrery.pending_rows_expired'
  const orreryFailedRowsRef =
    'recognition_ledger.launch_recognition.orrery.failed_before_dispatch_rows'

  const actors: ReadonlyArray<ReplayActor> = [
    {
      actorRef: 'actor.launch_recognition_terminal',
      avatarRole: 'recognition_terminal',
      displayName: 'Launch recognition terminal',
      fallbackAssetId: 'procedural.recognition_terminal.v1',
    },
    {
      actorRef: 'actor.trigger',
      avatarRole: 'recipient',
      displayName: 'Trigger',
      fallbackAssetId: 'procedural.pylon_avatar.recipient.trigger.v1',
    },
    {
      actorRef: 'actor.whitefang',
      avatarRole: 'recipient',
      displayName: 'Whitefang',
      fallbackAssetId: 'procedural.pylon_avatar.recipient.whitefang.v1',
    },
    {
      actorRef: 'actor.orrery',
      avatarRole: 'recipient',
      displayName: 'Orrery',
      fallbackAssetId: 'procedural.pylon_avatar.recipient.orrery.v1',
    },
    {
      actorRef: 'actor.owner_decision',
      avatarRole: 'operator_gate',
      displayName: 'Owner decision',
      fallbackAssetId: 'procedural.operator_gate.v1',
    },
  ]
  const stages: ReadonlyArray<ReplayStage> = [
    {
      label: 'Launch recognition ledger',
      sourceRefs: [recognitionLedgerRef, LAUNCH_ROADMAP_DOC_URL],
      stageKind: 'recognition_terminal',
      stageRef: 'stage.launch_recognition.terminal',
    },
    {
      label: 'Trigger lane',
      sourceRefs: [triggerConfirmationRef, LAUNCH_ROADMAP_DOC_URL],
      stageKind: 'recognition_lane',
      stageRef: 'stage.launch_recognition.trigger',
    },
    {
      label: 'Whitefang lane',
      sourceRefs: [
        whitefangConfirmationRef,
        whitefangHistoricalPendingRef,
        LAUNCH_ROADMAP_DOC_URL,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
      ],
      stageKind: 'recognition_lane',
      stageRef: 'stage.launch_recognition.whitefang',
    },
    {
      label: 'Orrery lane',
      sourceRefs: [
        orreryConfirmationRef,
        hazardPayDecisionRef,
        LAUNCH_ROADMAP_DOC_URL,
      ],
      stageKind: 'recognition_lane',
      stageRef: 'stage.launch_recognition.orrery',
    },
    {
      label: 'Orrery overpayment branch',
      sourceRefs: [
        hazardPayDecisionRef,
        orreryPendingRowsRef,
        orreryFailedRowsRef,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
        LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
      ],
      stageKind: 'overpayment_branch',
      stageRef: 'stage.launch_recognition.orrery_overpayment',
    },
  ]
  const events: ReadonlyArray<ReplayEvent> = [
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal'],
      displayText:
        'Recognition ledger opens with three intended 50,000-sat lanes.',
      kind: 'actor_entered_region',
      sequenceIndex: 0,
      sourceRefs: [recognitionLedgerRef, LAUNCH_ROADMAP_DOC_URL],
      targetRefs: ['stage.launch_recognition.terminal'],
      timelineSecond: 0,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.trigger'],
      amountSats: 50_000,
      caveat: 'Intended recognition amount; not a payment by itself.',
      displayText: 'Trigger lane records the intended 50,000-sat recognition reward.',
      kind: 'recognition_reward_recorded',
      rail: 'spark_backup_recipient_confirmation',
      sequenceIndex: 1,
      sourceRefs: [recognitionLedgerRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'intended_reward_recorded',
      targetRefs: ['stage.launch_recognition.trigger'],
      timelineSecond: 4,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.trigger'],
      amountSats: 50_000,
      displayText:
        'Trigger recipient-side Spark backup status confirms 50,000 sats visible.',
      kind: 'recipient_confirmation_recorded',
      rail: 'spark_backup',
      sequenceIndex: 2,
      sourceRefs: [
        triggerConfirmationRef,
        LAUNCH_ROADMAP_DOC_URL,
        LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
      ],
      stateAfter: 'recipient_confirmed',
      targetRefs: ['stage.launch_recognition.trigger'],
      timelineSecond: 8,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal', 'actor.trigger'],
      amountSats: 50_000,
      displayText:
        'Trigger recognition ribbon animates from recipient-confirmed public evidence.',
      kind: 'payment_zap_confirmed',
      rail: 'spark_backup_recipient_confirmation',
      sequenceIndex: 3,
      sourceRefs: [triggerConfirmationRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'recipient_confirmed',
      targetRefs: ['actor.trigger', 'stage.launch_recognition.trigger'],
      timelineSecond: 10,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.whitefang'],
      amountSats: 50_000,
      caveat: 'Intended recognition amount; older snapshots still showed blockers.',
      displayText:
        'Whitefang lane records the intended 50,000-sat recognition reward.',
      kind: 'recognition_reward_recorded',
      rail: 'spark_treasury',
      sequenceIndex: 4,
      sourceRefs: [recognitionLedgerRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'intended_reward_recorded',
      targetRefs: ['stage.launch_recognition.whitefang'],
      timelineSecond: 14,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal'],
      caveat:
        'Historical snapshot only: failed before a full-size recognition dispatch.',
      displayText:
        'Whitefang earlier closeout snapshot stays blocked: full recognition was still pending funding.',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 5,
      sourceRefs: [
        whitefangHistoricalPendingRef,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
        LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
      ],
      stateAfter: 'historical_pending_snapshot',
      targetRefs: ['stage.launch_recognition.whitefang'],
      timelineSecond: 18,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal', 'actor.whitefang'],
      amountSats: 50_000,
      displayText:
        'Whitefang recognition settles later as one Spark-treasury payment.',
      kind: 'payment_zap_confirmed',
      rail: 'spark_treasury',
      sequenceIndex: 6,
      sourceRefs: [whitefangConfirmationRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'recipient_confirmed',
      targetRefs: ['actor.whitefang', 'stage.launch_recognition.whitefang'],
      timelineSecond: 22,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.whitefang'],
      amountSats: 51_030,
      caveat:
        'Recipient-visible total includes the 1,000-sat canary plus 30-sat smokes.',
      displayText:
        'Whitefang recipient-side status reports 51,030 sats visible.',
      kind: 'recipient_confirmation_recorded',
      rail: 'spark_backup',
      sequenceIndex: 7,
      sourceRefs: [whitefangConfirmationRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'recipient_confirmed',
      targetRefs: ['stage.launch_recognition.whitefang'],
      timelineSecond: 26,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.orrery'],
      amountSats: 50_000,
      caveat: 'Intended recognition amount before the overpayment incident.',
      displayText: 'Orrery lane records the intended 50,000-sat recognition reward.',
      kind: 'recognition_reward_recorded',
      rail: 'split_lightning_address',
      sequenceIndex: 8,
      sourceRefs: [recognitionLedgerRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'intended_reward_recorded',
      targetRefs: ['stage.launch_recognition.orrery'],
      timelineSecond: 30,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal', 'actor.orrery'],
      amountSats: 50_000,
      caveat:
        'Recognition amount covered by documented split settled sends, not a single clean full-size send.',
      displayText:
        'Orrery intended recognition amount is covered by split settled sends.',
      kind: 'payment_zap_confirmed',
      rail: 'split_lightning_address',
      sequenceIndex: 9,
      sourceRefs: [
        orreryConfirmationRef,
        LAUNCH_ROADMAP_DOC_URL,
        LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
      ],
      stateAfter: 'recognition_amount_covered',
      targetRefs: ['actor.orrery', 'stage.launch_recognition.orrery'],
      timelineSecond: 34,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal'],
      caveat:
        'The failed retry rows had no durable payment id and no settled row; they are not received money.',
      displayText:
        'Orrery large retry failures stay failed-before-dispatch: 0 sats moved for those rows.',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 10,
      sourceRefs: [
        orreryFailedRowsRef,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
        LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
      ],
      stateAfter: 'failed_before_dispatch',
      targetRefs: ['stage.launch_recognition.orrery_overpayment'],
      timelineSecond: 38,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal'],
      caveat:
        'Pending/orphaned local rows are not recipient receipts; final roadmap says they were expired.',
      displayText:
        'Orrery orphaned pending rows render as accounting cards, not moving sats.',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 11,
      sourceRefs: [
        orreryPendingRowsRef,
        LAUNCH_ROADMAP_DOC_URL,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
      ],
      stateAfter: 'expired_or_pending_snapshot',
      targetRefs: ['stage.launch_recognition.orrery_overpayment'],
      timelineSecond: 42,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.launch_recognition_terminal', 'actor.orrery'],
      amountSats: 109_239,
      caveat:
        'Overage uses the latest roadmap recipient-confirmed 159,239-sat balance minus the intended 50,000-sat reward.',
      displayText:
        'Orrery overpayment detected: 109,239 sats above the intended reward.',
      kind: 'overpayment_detected',
      rail: 'split_lightning_address',
      sequenceIndex: 12,
      sourceRefs: [
        hazardPayDecisionRef,
        orreryConfirmationRef,
        LAUNCH_ROADMAP_DOC_URL,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
      ],
      stateAfter: 'overpayment_detected',
      targetRefs: ['stage.launch_recognition.orrery_overpayment'],
      timelineSecond: 46,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.orrery'],
      amountSats: 159_239,
      caveat:
        'Recipient-visible total is shown as closeout evidence, not as the originally intended amount.',
      displayText:
        'Orrery recipient-side status confirms 159,239 sats visible.',
      kind: 'recipient_confirmation_recorded',
      rail: 'spark_backup',
      sequenceIndex: 13,
      sourceRefs: [orreryConfirmationRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'recipient_confirmed',
      targetRefs: ['stage.launch_recognition.orrery'],
      timelineSecond: 50,
    }),
    makeLaunchRecognitionEvent({
      actorRefs: ['actor.owner_decision'],
      caveat:
        'This is a documented owner decision after the incident, not original payout intent.',
      displayText:
        'Owner decision: Orrery keeps the overage as hazard pay; do not resend.',
      kind: 'claim_boundary_shown',
      sequenceIndex: 14,
      sourceRefs: [hazardPayDecisionRef, LAUNCH_ROADMAP_DOC_URL],
      stateAfter: 'closed_do_not_resend',
      targetRefs: ['stage.launch_recognition.orrery_overpayment'],
      timelineSecond: 55,
    }),
  ]
  const flows: ReadonlyArray<ReplayFlow> = [
    ...(['trigger', 'whitefang', 'orrery'] as const).map(
      (recipient): ReplayFlow => ({
        amountSats: 50_000,
        flowKind: 'recognition_reward',
        flowRef: `proof_replay_flow.launch_recognition.${recipient}.intended_reward`,
        fromRef: 'actor.launch_recognition_terminal',
        rail:
          recipient === 'whitefang'
            ? 'spark_treasury'
            : recipient === 'orrery'
              ? 'split_lightning_address'
              : 'spark_backup_recipient_confirmation',
        sourceRefs: [recognitionLedgerRef, LAUNCH_ROADMAP_DOC_URL],
        toRef: `actor.${recipient}`,
      }),
    ),
    {
      amountSats: 109_239,
      flowKind: 'overpayment_branch',
      flowRef: 'proof_replay_flow.launch_recognition.orrery.overpayment',
      fromRef: 'actor.launch_recognition_terminal',
      rail: 'split_lightning_address',
      sourceRefs: [hazardPayDecisionRef, orreryConfirmationRef, LAUNCH_ROADMAP_DOC_URL],
      toRef: 'stage.launch_recognition.orrery_overpayment',
    },
    {
      flowKind: 'pending_marker',
      flowRef: 'proof_replay_flow.launch_recognition.orrery.pending_not_payment',
      fromRef: 'actor.launch_recognition_terminal',
      sourceRefs: [orreryPendingRowsRef, LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL],
      toRef: 'stage.launch_recognition.orrery_overpayment',
    },
  ]
  const cameraCues: ReadonlyArray<ReplayCameraCue> = [
    {
      cueRef: 'proof_replay_camera.launch_recognition.open',
      durationSecond: 10,
      focusRefs: ['stage.launch_recognition.terminal'],
      mode: 'overview',
      sourceRefs: [LAUNCH_ROADMAP_DOC_URL],
      startSecond: 0,
    },
    {
      cueRef: 'proof_replay_camera.launch_recognition.lanes',
      durationSecond: 22,
      focusRefs: [
        'stage.launch_recognition.trigger',
        'stage.launch_recognition.whitefang',
        'stage.launch_recognition.orrery',
      ],
      mode: 'director_track',
      sourceRefs: [recognitionLedgerRef],
      startSecond: 10,
    },
    {
      cueRef: 'proof_replay_camera.launch_recognition.overpayment',
      durationSecond: 20,
      focusRefs: ['stage.launch_recognition.orrery_overpayment'],
      mode: 'zap_focus',
      sourceRefs: [hazardPayDecisionRef, LAUNCH_ROADMAP_DOC_URL],
      startSecond: 34,
    },
    {
      cueRef: 'proof_replay_camera.launch_recognition.final',
      durationSecond: 8,
      focusRefs: ['stage.launch_recognition.terminal'],
      mode: 'overview',
      sourceRefs: docs,
      startSecond: 55,
    },
  ]
  const captions: ReadonlyArray<ReplayCaption> = [
    {
      captionRef: 'proof_replay_caption.launch_recognition.title',
      sequenceIndex: 0,
      sourceRefs: [LAUNCH_ROADMAP_DOC_URL],
      text: LAUNCH_RECOGNITION_TITLE,
      timelineSecond: 0,
    },
    {
      captionRef: 'proof_replay_caption.launch_recognition.intent',
      sequenceIndex: 1,
      sourceRefs: [recognitionLedgerRef, LAUNCH_ROADMAP_DOC_URL],
      text: 'Each lane starts at the intended 50,000-sat recognition reward.',
      timelineSecond: 4,
    },
    {
      captionRef: 'proof_replay_caption.launch_recognition.blockers',
      sequenceIndex: 2,
      sourceRefs: [LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL],
      text: 'Pending, failed, and timeout rows stay ledger cards, not received money.',
      timelineSecond: 18,
    },
    {
      captionRef: 'proof_replay_caption.launch_recognition.overpayment',
      sequenceIndex: 3,
      sourceRefs: [hazardPayDecisionRef, LAUNCH_ROADMAP_DOC_URL],
      text: 'Orrery overpayment is an exception lane: hazard pay, not original intent.',
      timelineSecond: 46,
    },
  ]
  const gaps: ReadonlyArray<ReplayGap> = [
    {
      affectedRefs: [whitefangHistoricalPendingRef, whitefangConfirmationRef],
      gapRef: 'proof_replay_gap.launch_recognition.whitefang_snapshot_change',
      reason:
        'Earlier payment docs showed Whitefang full recognition pending; the roadmap end-of-day update supersedes that with a closed, recipient-confirmed state.',
      sourceRefs: [
        LAUNCH_ROADMAP_DOC_URL,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
        LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
      ],
    },
    {
      affectedRefs: [orreryConfirmationRef, hazardPayDecisionRef],
      gapRef: 'proof_replay_gap.launch_recognition.orrery_accounting_snapshot_change',
      reason:
        'Orrery sender-side closeout snapshots and later recipient-visible closeout differ; the replay renders both as historical accounting states instead of rewriting them.',
      sourceRefs: [
        LAUNCH_ROADMAP_DOC_URL,
        LAUNCH_RECOGNITION_CLOSEOUT_DOC_URL,
        LAUNCH_RECOGNITION_SPARK_STATUS_DOC_URL,
      ],
    },
  ]
  const allSourceRefs = uniqueSorted([
    ...docs,
    ...stages.flatMap(stage => stage.sourceRefs),
    ...events.flatMap(event => event.sourceRefs),
    ...flows.flatMap(flow => flow.sourceRefs),
    ...captions.flatMap(caption => caption.sourceRefs),
    ...gaps.flatMap(gap => gap.sourceRefs),
  ])
  const bundleWithoutRef: Omit<ProofReplayBundle, 'bundleRef'> = {
    actors,
    cameraCues,
    captions,
    claimScope: 'evidence_presentation_only',
    events,
    flows,
    gaps,
    generatedAt: input.generatedAt,
    privacyLevel: 'public_safe',
    schemaVersion: ProofReplayBundleSchemaVersion,
    socialDisplayTime: LAUNCH_RECOGNITION_LOCAL_DISPLAY_TIME,
    sourceAuthority: 'worker_d1_public',
    sourceRefs: allSourceRefs.map(ref => sourceRecord(ref)),
    stages,
    staleness: replayBundleStaleness(),
    title: LAUNCH_RECOGNITION_TITLE,
  }
  assertPublicSafe(bundleWithoutRef)

  const deterministicRefSeed = JSON.stringify({
    events: bundleWithoutRef.events.map(event => ({
      kind: event.kind,
      sequenceIndex: event.sequenceIndex,
      sourceRefs: event.sourceRefs,
    })),
    sourceRefs: allSourceRefs,
    title: bundleWithoutRef.title,
  })

  return {
    ...bundleWithoutRef,
    bundleRef: `proof_replay_bundle.launch_recognition.${stableHash(
      deterministicRefSeed,
    )}`,
  }
}

export const buildFirstRealSettlementReplayBundle = (
  input: Readonly<{
    appUrl: string
    generatedAt: string
    requestedRefs: ReadonlyArray<string>
    summary: PublicTassadarSummaryEnvelope
  }>,
): ProofReplayBundle => {
  const settlementRows = asSettlementRows(input.summary.settlementRows)
  const realSettlementRow = selectRealSettlementRow(
    settlementRows,
    input.requestedRefs,
  )
  const simulationRows = settlementRows.filter(
    row => row.movementMode === 'simulation' || !row.realBitcoinMoved,
  )
  const receiptRef =
    realSettlementRow?.receiptRef ?? FIRST_REAL_SETTLEMENT_RECEIPT_REF
  const contributorRef =
    realSettlementRow?.contributorRef ?? FIRST_REAL_SETTLEMENT_CONTRIBUTOR_REF
  const challengeRef =
    realSettlementRow?.verificationChallengeRef ??
    FIRST_REAL_SETTLEMENT_CHALLENGE_REF
  const runRef =
    typeof input.summary.runRef === 'string'
      ? input.summary.runRef
      : realSettlementRow?.trainingRunRef ?? DEFAULT_TASSADAR_RUN_REF
  const receiptApiUrl =
    realSettlementRow?.apiUrl ?? canonicalReceiptApiUrl(input.appUrl, receiptRef)
  const payoutIntentRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'payout_intent.',
    'payout_intent.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const payoutAttemptRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'payout_attempt.',
    'payout_attempt.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const reconciliationRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'reconciliation.',
    'reconciliation.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const externalEventRef = refFromSourceRefs(
    realSettlementRow?.sourceRefs ?? [],
    'external_event.',
    'external_event.tassadar_run_settlement.spark_treasury.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
  )
  const realSourceRefs = uniqueSorted([
    runRef,
    FIRST_REAL_SETTLEMENT_WINDOW_REF,
    challengeRef,
    ...optionalRef(contributorRef),
    receiptRef,
    receiptApiUrl,
    payoutIntentRef,
    payoutAttemptRef,
    reconciliationRef,
    externalEventRef,
    FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL,
    FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL,
    ...(realSettlementRow?.sourceRefs ?? []),
  ])
  const simulationSourceRefs = uniqueSorted(
    simulationRows.flatMap(row => [
      row.receiptRef,
      row.apiUrl,
      ...row.sourceRefs,
    ]),
  )
  const allSourceRefs = uniqueSorted([...realSourceRefs, ...simulationSourceRefs])
  const actors: ReadonlyArray<ReplayActor> = [
    {
      actorRef: `actor.${contributorRef}`,
      avatarRole: 'contributor',
      displayName: 'Contributor pylon',
      fallbackAssetId: 'procedural.pylon_avatar.contributor.v1',
      pylonRef: contributorRef,
    },
    {
      actorRef: 'actor.tassadar.validator',
      avatarRole: 'validator',
      displayName: 'Independent validator',
      fallbackAssetId: 'procedural.pylon_avatar.validator.v1',
    },
    {
      actorRef: 'actor.owner_gate',
      avatarRole: 'operator_gate',
      displayName: 'Owner gate',
      fallbackAssetId: 'procedural.operator_gate.v1',
    },
    {
      actorRef: 'actor.spark_treasury_terminal',
      avatarRole: 'settlement_terminal',
      displayName: 'Spark treasury terminal',
      fallbackAssetId: 'procedural.settlement_terminal.spark.v1',
    },
    {
      actorRef: 'actor.forum_announcer',
      avatarRole: 'announcer',
      displayName: 'Forum announcement',
      fallbackAssetId: 'procedural.forum_announcer.v1',
    },
  ]
  const stages: ReadonlyArray<ReplayStage> = [
    {
      label: 'Tassadar',
      sourceRefs: [runRef],
      stageKind: 'run_core',
      stageRef: 'stage.tassadar.run_core',
    },
    {
      label: 'Contributor station',
      sourceRefs: optionalRef(contributorRef),
      stageKind: 'pylon_station',
      stageRef: `stage.${contributorRef}.station`,
    },
    {
      label: 'Exact replay proof gate',
      sourceRefs: [challengeRef],
      stageKind: 'proof_gate',
      stageRef: 'stage.tassadar.proof_gate',
    },
    {
      label: 'Spark settlement terminal',
      sourceRefs: [receiptRef, payoutIntentRef, reconciliationRef],
      stageKind: 'settlement_terminal',
      stageRef: 'stage.tassadar.settlement_terminal',
    },
    {
      label: 'Simulation rehearsal lane',
      sourceRefs: simulationSourceRefs,
      stageKind: 'registry_marker',
      stageRef: 'stage.tassadar.simulation_rehearsal',
    },
  ]
  const simulationEvents = simulationRows.map((row, index) =>
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      amountSats: row.amountSats,
      caveat:
        'Simulation-backed settlement record; this is not confirmed Bitcoin movement.',
      displayText: `${row.amountSats} sats simulation rehearsal remains non-payment.`,
      kind: 'payment_zap_simulated',
      sequenceIndex: 4 + index,
      sourceRefs: uniqueSorted([row.receiptRef, row.apiUrl, ...row.sourceRefs]),
      stateAfter: row.state,
      stateBefore: 'simulation_recorded',
      targetRefs: ['stage.tassadar.simulation_rehearsal'],
      timelineSecond: 18 + index,
    }),
  )
  const eventOffset = simulationEvents.length
  const events: ReadonlyArray<ReplayEvent> = [
    makeEvent({
      actorRefs: [`actor.${contributorRef}`],
      displayText: 'Contributor pylon enters the Tassadar replay stage.',
      kind: 'actor_entered_region',
      sequenceIndex: 0,
      sourceRefs: [runRef, ...optionalRef(contributorRef)],
      targetRefs: ['stage.tassadar.run_core'],
      timelineSecond: 0,
    }),
    makeEvent({
      actorRefs: [`actor.${contributorRef}`],
      displayText: 'Contributor submits exact-trace work to the proof gate.',
      kind: 'proof_submitted',
      sequenceIndex: 1,
      sourceRefs: [challengeRef, runRef],
      targetRefs: ['stage.tassadar.proof_gate'],
      timelineSecond: 5,
    }),
    makeEvent({
      actorRefs: ['actor.tassadar.validator'],
      displayText: 'Independent validator replay matches the challenge digest.',
      kind: 'proof_verified',
      sequenceIndex: 2,
      sourceRefs: [challengeRef],
      stateAfter: 'Verified',
      targetRefs: ['stage.tassadar.proof_gate'],
      timelineSecond: 10,
    }),
    makeEvent({
      actorRefs: ['actor.owner_gate'],
      caveat: 'Authorization opens a bounded settlement path; it is not payment.',
      displayText: 'Owner gate authorizes the bounded 1,000-sat settlement branch.',
      kind: 'claim_boundary_shown',
      sequenceIndex: 3,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL, runRef, challengeRef],
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 15,
    }),
    ...simulationEvents,
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      caveat: 'Failed closed before dispatch; 0 sats moved.',
      displayText: 'Payout intent lookup failed closed: 0 sats moved.',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 4 + eventOffset,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL],
      stateAfter: 'blocked_closed',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 22,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      displayText: 'Durable payout intent persisted for the real canary settlement.',
      kind: 'payout_intent_persisted',
      sequenceIndex: 5 + eventOffset,
      sourceRefs: [payoutIntentRef, FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL],
      stateAfter: 'persisted',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 27,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      caveat: 'Treasury adapter unavailable; failed closed before sats moved.',
      displayText: 'Spark treasury adapter was unavailable: 0 sats moved.',
      kind: 'settlement_blocked_closed',
      sequenceIndex: 6 + eventOffset,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL, externalEventRef],
      stateAfter: 'blocked_closed',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 32,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      amountSats: realSettlementRow?.amountSats ?? 1_000,
      displayText: 'Settlement receipt recorded from the Spark treasury rail.',
      kind: 'settlement_recorded',
      observedAt: FIRST_REAL_SETTLEMENT_OBSERVED_AT,
      rail: 'spark_treasury',
      sequenceIndex: 7 + eventOffset,
      sourceRefs: [receiptRef, receiptApiUrl, reconciliationRef, payoutAttemptRef],
      stateAfter: realSettlementRow?.state ?? 'settled',
      targetRefs: ['stage.tassadar.settlement_terminal'],
      timelineSecond: 36,
    }),
    makeEvent({
      actorRefs: ['actor.spark_treasury_terminal'],
      amountSats: realSettlementRow?.amountSats ?? 1_000,
      displayText: '1,000 sats zap to the contributor pylon is receipt-backed.',
      kind: 'payment_zap_confirmed',
      observedAt: FIRST_REAL_SETTLEMENT_OBSERVED_AT,
      rail: 'spark_treasury',
      sequenceIndex: 8 + eventOffset,
      sourceRefs: [receiptRef, receiptApiUrl, externalEventRef],
      stateAfter:
        realSettlementRow?.realBitcoinMoved === true
          ? 'realBitcoinMoved:true'
          : 'awaiting_public_projection',
      targetRefs: [`actor.${contributorRef}`, `stage.${contributorRef}.station`],
      timelineSecond: 38,
    }),
    makeEvent({
      actorRefs: ['actor.forum_announcer'],
      displayText: 'Public Forum announcement links the settled receipt.',
      kind: 'forum_announcement_posted',
      sequenceIndex: 9 + eventOffset,
      sourceRefs: [FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL, receiptRef],
      targetRefs: ['stage.tassadar.run_core'],
      timelineSecond: 44,
    }),
  ]
  const flows: ReadonlyArray<ReplayFlow> = [
    {
      flowKind: 'work_handoff',
      flowRef: 'proof_replay_flow.tassadar.first_real_settlement.work_handoff',
      fromRef: `actor.${contributorRef}`,
      sourceRefs: [challengeRef, runRef],
      toRef: 'stage.tassadar.proof_gate',
    },
    {
      flowKind: 'verification_check',
      flowRef:
        'proof_replay_flow.tassadar.first_real_settlement.verification_check',
      fromRef: 'actor.tassadar.validator',
      sourceRefs: [challengeRef],
      toRef: 'stage.tassadar.proof_gate',
    },
    {
      amountSats: realSettlementRow?.amountSats ?? 1_000,
      flowKind: 'payment_movement',
      flowRef: 'proof_replay_flow.tassadar.first_real_settlement.spark_zap',
      fromRef: 'actor.spark_treasury_terminal',
      rail: 'spark_treasury',
      sourceRefs: [receiptRef, receiptApiUrl, externalEventRef],
      toRef: `actor.${contributorRef}`,
    },
    ...simulationRows.map((row): ReplayFlow => ({
      amountSats: row.amountSats,
      flowKind: 'simulation_marker',
      flowRef: `proof_replay_flow.tassadar.first_real_settlement.simulation.${stableHash(row.receiptRef)}`,
      fromRef: 'actor.spark_treasury_terminal',
      sourceRefs: [row.receiptRef, row.apiUrl, ...row.sourceRefs],
      toRef: 'stage.tassadar.simulation_rehearsal',
    })),
  ]
  const cameraCues: ReadonlyArray<ReplayCameraCue> = [
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.overview',
      durationSecond: 8,
      focusRefs: ['stage.tassadar.run_core'],
      mode: 'overview',
      sourceRefs: [runRef],
      startSecond: 0,
    },
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.orbit_proof',
      durationSecond: 10,
      focusRefs: ['stage.tassadar.proof_gate', challengeRef],
      mode: 'orbit_proof',
      sourceRefs: [challengeRef],
      startSecond: 8,
    },
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.zap_focus',
      durationSecond: 8,
      focusRefs: ['stage.tassadar.settlement_terminal', receiptRef],
      mode: 'zap_focus',
      sourceRefs: [receiptRef, receiptApiUrl],
      startSecond: 34,
    },
    {
      cueRef: 'proof_replay_camera.tassadar.first_real_settlement.final',
      durationSecond: 10,
      focusRefs: ['stage.tassadar.run_core', `actor.${contributorRef}`, receiptRef],
      mode: 'director_track',
      sourceRefs: [receiptRef, FIRST_REAL_SETTLEMENT_SETTLED_FORUM_URL],
      startSecond: 42,
    },
  ]
  const captions: ReadonlyArray<ReplayCaption> = [
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.title',
      sequenceIndex: 0,
      sourceRefs: [runRef],
      text: FIRST_REAL_SETTLEMENT_TITLE,
      timelineSecond: 0,
    },
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.verify',
      sequenceIndex: 1,
      sourceRefs: [challengeRef],
      text: 'Verified work -> owner gate -> Spark zap -> public receipt',
      timelineSecond: 9,
    },
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.failed_closed',
      sequenceIndex: 2,
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL],
      text: 'Two real dispatch blockers failed closed before any sats moved.',
      timelineSecond: 22,
    },
    {
      captionRef: 'proof_replay_caption.tassadar.first_real_settlement.zap',
      sequenceIndex: 3,
      sourceRefs: [receiptRef, receiptApiUrl],
      text: '1,000 sats settled, realBitcoinMoved:true',
      timelineSecond: 38,
    },
  ]
  const gaps: ReadonlyArray<ReplayGap> = [
    {
      affectedRefs: [
        'payout_intent_not_found',
        'adapter_unavailable',
        FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL,
      ],
      gapRef: 'proof_replay_gap.tassadar.first_real_settlement.operational_history_sequence',
      reason:
        'Intermediate failed-closed timestamps are Forum-announced operational history and are ordered by replay sequence.',
      sourceRefs: [FIRST_REAL_SETTLEMENT_FAILED_FORUM_URL],
    },
  ]
  const bundleWithoutRef: Omit<ProofReplayBundle, 'bundleRef'> = {
    actors,
    cameraCues,
    captions,
    claimScope: 'evidence_presentation_only',
    events,
    flows,
    gaps,
    generatedAt: input.generatedAt,
    privacyLevel: 'public_safe',
    schemaVersion: ProofReplayBundleSchemaVersion,
    socialDisplayTime: FIRST_REAL_SETTLEMENT_LOCAL_DISPLAY_TIME,
    sourceAuthority: 'worker_d1_public',
    sourceRefs: allSourceRefs.map(ref =>
      sourceRecord(
        ref,
        ref === receiptRef || ref === receiptApiUrl
          ? FIRST_REAL_SETTLEMENT_OBSERVED_AT
          : undefined,
      ),
    ),
    stages,
    staleness: replayBundleStaleness(),
    title: FIRST_REAL_SETTLEMENT_TITLE,
  }
  assertPublicSafe(bundleWithoutRef)

  const deterministicRefSeed = JSON.stringify({
    events: bundleWithoutRef.events.map(event => ({
      kind: event.kind,
      sequenceIndex: event.sequenceIndex,
      sourceRefs: event.sourceRefs,
    })),
    receiptRef,
    sourceRefs: allSourceRefs,
    title: bundleWithoutRef.title,
  })

  return {
    ...bundleWithoutRef,
    bundleRef: `proof_replay_bundle.tassadar.first_real_settlement.${stableHash(
      deterministicRefSeed,
    )}`,
  }
}

export const buildPublicProofReplayBundleForRequest = async (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
  deps: Deps = {},
): Promise<ProofReplayBundle> => {
  const generatedAt = (deps.now ?? currentIsoTimestamp)()
  if (isGeneratedActivityReplayRequest(request)) {
    return buildGeneratedActivityReplayBundleForRequest(
      request,
      env,
      deps,
      generatedAt,
    )
  }

  const requestedRefs = requestedRefsFor(request)
  if (isLaunchRecognitionReplayRequest(requestedRefs)) {
    return buildLaunchRecognitionReplayBundle({
      appUrl: new URL(request.url).origin,
      generatedAt,
      requestedRefs,
    })
  }

  const makeStore =
    deps.makeStore ?? (e => makeD1TrainingAuthorityStore(openAgentsDatabase(e)))
  const makePayoutLedgerStore =
    deps.makePayoutLedgerStore ??
    (e => makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(e)))
  const runRef = requestedRunRefFor(request)
  const appUrl = new URL(request.url).origin
  const summary = await buildPublicTassadarRunSummaryEnvelope(
    makeStore(env),
    runRef,
    generatedAt,
    makePayoutLedgerStore(env),
    appUrl,
  )

  return buildFirstRealSettlementReplayBundle({
    appUrl,
    generatedAt,
    requestedRefs,
    summary,
  })
}

export const handlePublicProofReplayBundleRequest = async (
  request: Request,
  env: Parameters<typeof openAgentsDatabase>[0],
  deps: Deps = {},
) => {
  const method = request.method.toUpperCase()
  if (method === 'OPTIONS') return publicProofReplayOptionsResponse()
  if (method !== 'GET' && method !== 'HEAD') {
    return publicProofReplayMethodNotAllowedResponse()
  }

  try {
    const bundle = await buildPublicProofReplayBundleForRequest(request, env, deps)
    return publicProofReplayJsonResponse(bundle)
  } catch (error) {
    if (error instanceof PublicProofReplayRequestError) {
      return publicProofReplayJsonResponse(error.payload, { status: error.status })
    }
    throw error
  }
}
