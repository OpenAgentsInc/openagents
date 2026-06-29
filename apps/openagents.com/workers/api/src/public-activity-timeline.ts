import {
  PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION,
  assertPublicActivityTimelineEnvelopeSafe,
  assertPublicActivityTimelineEventSafe,
  orderPublicActivityTimelineEvents,
  publicActivityTimelineCursorForEvent,
  publicActivityTimelineEventKinds,
  publicActivityTimelineSourceKinds,
  type PublicActivityTimelineEnvelope,
  type PublicActivityTimelineEvent,
  type PublicActivityTimelineEventKind,
  type PublicActivityTimelineRange,
  type PublicActivityTimelineSourceKind,
  type PublicActivityTimelineSourceLag,
} from '@openagentsinc/public-activity-timeline'

import { parseJsonRecord } from './json-boundary'
import {
  publicInferenceReceiptFromRecord,
  type InferenceReceiptRecord,
  type InferenceReceiptStore,
} from './inference-receipts'
import { parseInferenceChargeContextRef } from './inference/metering-hook'
import { isKhalaModel } from './inference/pricing'
import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusTreasuryPayoutLedgerStore,
} from './nexus-treasury-payout-ledger'
import { liveAtReadStaleness } from './public-projection-staleness'
import {
  PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
} from './public-pylon-stats'
import {
  pylonClientVersionMeetsMinimum,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
} from './pylon-api'
import { currentIsoTimestamp } from './runtime-primitives'
import type {
  TrainingAuthorityStore,
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

const DEFAULT_ACTIVITY_TIMELINE_LIMIT = 50
const MAX_ACTIVITY_TIMELINE_LIMIT = 200
const SOURCE_LOOKUP_LIMIT = 48
const DEFAULT_TRAINING_RUN_LIMIT = 8
const OPENAGENTS_PUBLIC_APP_URL = 'https://openagents.com'

const timelineRebuildRefs = [
  'public_activity_timeline_read',
  'pylon_registration_recorded',
  'pylon_heartbeat_ingested',
  'training_window_state_transition_recorded',
  'training_window_lease_claimed',
  'training_trace_contribution_recorded',
  'training_verification_challenge_state_transition_recorded',
  'pay_ins.public_receipt_ref',
  'payment_authority_receipt_recorded',
  'forum_topic_created',
  'forum_post_created',
  'artanis_admin_tick_decision_recorded',
  'pylon_capacity_funnel_snapshot_recorded',
] as const

export type PublicActivityTimelineQuery = Readonly<{
  from: string | undefined
  to: string | undefined
  since: string | undefined
  limit: number
  filterKinds: ReadonlyArray<PublicActivityTimelineEventKind>
  filterSources: ReadonlyArray<PublicActivityTimelineSourceKind>
}>

export type PublicActivityTimelinePylonStore = Pick<
  PylonApiStore,
  'listRegistrations'
>

export type PublicActivityTimelineTrainingStore = Pick<
  TrainingAuthorityStore,
  | 'listRuns'
  | 'listVerificationChallengesForRun'
  | 'listWindowLeasesForRun'
  | 'listWindowsForRun'
>

export type PublicActivityTimelineReceiptStore = Pick<
  NexusTreasuryPayoutLedgerStore,
  'listPaymentAuthorityReceipts' | 'readReconciliationEventByRef'
>

export type PublicActivityTimelineInferenceReceiptStore = Pick<
  InferenceReceiptStore,
  'listRecentInferenceReceipts'
>

export type PublicActivityTimelineForumRecord = Readonly<{
  actorRef: string | null
  createdAt: string
  eventRef: string
  kind: 'topic' | 'post'
  postRef: string | null
  sourceRefs: ReadonlyArray<string>
  state: string
  title: string | null
  topicRef: string
}>

export type PublicActivityTimelineForumStore = Readonly<{
  listRecentActivity: (
    limit: number,
  ) => Promise<ReadonlyArray<PublicActivityTimelineForumRecord>>
}>

export type PublicActivityTimelineArtanisTickRecord = Readonly<{
  assignmentRef: string | null
  createdAt: string
  decisionRef: string
  sourceRefs: ReadonlyArray<string>
  state: string
}>

export type PublicActivityTimelineArtanisStore = Readonly<{
  listRecentTicks: (
    limit: number,
  ) => Promise<ReadonlyArray<PublicActivityTimelineArtanisTickRecord>>
}>

export type PublicActivityTimelineCapacitySnapshotRecord = Readonly<{
  aggregateState: string
  snapshotAt: string
  snapshotRef: string
  sourceRefs: ReadonlyArray<string>
}>

export type PublicActivityTimelineCapacityStore = Readonly<{
  listRecentSnapshots: (
    limit: number,
  ) => Promise<ReadonlyArray<PublicActivityTimelineCapacitySnapshotRecord>>
}>

export type PublicActivityTimelineSourceInput = Readonly<{
  artanisStore?: PublicActivityTimelineArtanisStore
  capacityStore?: PublicActivityTimelineCapacityStore
  forumStore?: PublicActivityTimelineForumStore
  nowIso?: () => string
  pylonStore?: PublicActivityTimelinePylonStore
  query?: Partial<PublicActivityTimelineQuery>
  inferenceReceiptStore?: PublicActivityTimelineInferenceReceiptStore
  receiptStore?: PublicActivityTimelineReceiptStore
  trainingStore?: PublicActivityTimelineTrainingStore
}>

class PublicActivityTimelineProjectionUnsafe extends Error {
  override readonly name = 'PublicActivityTimelineProjectionUnsafe'
}

const nonEmpty = (value: string | null | undefined): value is string =>
  typeof value === 'string' && value.trim() !== ''

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter(nonEmpty).map(ref => ref.trim()))].sort()

const slugPart = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9_.:-]+/g, '_').slice(0, 160)

const sourceLagSeconds = (
  observedAt: string,
  latestSourceEventAt: string,
): number | null => {
  const observedMillis = Date.parse(observedAt)
  const latestMillis = Date.parse(latestSourceEventAt)

  if (!Number.isFinite(observedMillis) || !Number.isFinite(latestMillis)) {
    return null
  }

  return Math.max(0, Math.floor((observedMillis - latestMillis) / 1000))
}

const eventWithCursor = (
  input: Omit<PublicActivityTimelineEvent, 'cursor'>,
): PublicActivityTimelineEvent => {
  const event = {
    ...input,
    blockerRefs: uniqueRefs(input.blockerRefs),
    caveatRefs: uniqueRefs(input.caveatRefs),
    refs: uniqueRefs(input.refs),
    sourceRefs: uniqueRefs(input.sourceRefs),
  }

  return {
    ...event,
    cursor: publicActivityTimelineCursorForEvent(event),
  }
}

const sourceLag = (input: {
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
  events: ReadonlyArray<PublicActivityTimelineEvent>
  maxStalenessSeconds: number
  observedAt: string
  sourceKind: PublicActivityTimelineSourceKind
  sourceRefs: ReadonlyArray<string>
  status?: PublicActivityTimelineSourceLag['status']
}): PublicActivityTimelineSourceLag => {
  const latestSourceEventAt =
    input.events.length === 0
      ? null
      : [...input.events].sort((left, right) => right.ts.localeCompare(left.ts))[0]
          ?.ts ?? null
  const sourceRefs = uniqueRefs(input.sourceRefs)
  const blockerRefs = uniqueRefs(input.blockerRefs ?? [])
  const lagSeconds =
    latestSourceEventAt === null
      ? null
      : sourceLagSeconds(input.observedAt, latestSourceEventAt)
  const sourceIsStale =
    lagSeconds !== null && lagSeconds > input.maxStalenessSeconds
  const status =
    input.status ??
    (blockerRefs.length > 0 && input.events.length === 0
      ? 'projection_gap'
      : sourceIsStale
        ? 'stale'
      : 'current')
  const caveatRefs = uniqueRefs([
    ...(input.caveatRefs ?? []),
    ...(sourceIsStale
      ? ['caveat.public.activity_timeline.source_lag_exceeds_contract']
      : []),
  ])

  return {
    blockerRefs,
    caveatRefs,
    lagSeconds,
    latestSourceEventAt,
    maxStalenessSeconds: input.maxStalenessSeconds,
    observedAt: input.observedAt,
    sourceKind: input.sourceKind,
    sourceRefs,
    status,
  }
}

const projectionGapEvent = (input: {
  blockerRef: string
  observedAt: string
  sourceKind: PublicActivityTimelineSourceKind
  text: string
}): PublicActivityTimelineEvent =>
  eventWithCursor({
    blockerRefs: [input.blockerRef],
    caveatRefs: ['caveat.public.activity_timeline.projection_gap_no_guessing'],
    eventRef: `event.public.projection_gap.${input.sourceKind}.${slugPart(
      input.observedAt,
    )}`,
    kind: 'projection_gap',
    refs: [`source.${input.sourceKind}`],
    sourceKind: 'projection_gap',
    sourceRefs: [],
    text: input.text,
    ts: input.observedAt,
  })

const pylonIsOnline = (registration: PylonApiRegistrationRecord): boolean => {
  const status = registration.latestHeartbeatStatus?.trim().toLowerCase() ?? ''
  return ['available', 'healthy', 'idle', 'online', 'ready'].includes(status)
}

const pylonHasAssignmentEvidence = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  registration.capabilityRefs.length > 0 &&
  registration.latestCapacityRefs.length > 0 &&
  registration.latestHealthRefs.length > 0 &&
  registration.latestLoadRefs.length > 0

const pylonIsAssignmentReady = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  registration.status === 'active' &&
  registration.walletReady &&
  pylonIsOnline(registration) &&
  pylonHasAssignmentEvidence(registration) &&
  pylonClientVersionMeetsMinimum(
    registration.clientVersion,
    PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
  )

const pylonEvents = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
): ReadonlyArray<PublicActivityTimelineEvent> =>
  registrations.flatMap(registration => {
    const refs = uniqueRefs([
      registration.pylonRef,
      ...registration.capabilityRefs,
      ...registration.latestCapacityRefs,
      ...registration.latestHealthRefs,
      ...registration.latestLoadRefs,
    ])
    const baseSourceRefs = uniqueRefs([
      registration.pylonRef,
      'route:/api/public/pylon-stats',
    ])
    const events: PublicActivityTimelineEvent[] = [
      eventWithCursor({
        actorRef: registration.pylonRef,
        blockerRefs: [],
        caveatRefs: [
          'caveat.public.activity_timeline.pylon_refs_are_public_projection_only',
        ],
        eventRef: `event.public.pylon_registered.${registration.pylonRef}`,
        kind: 'pylon_registered',
        refs,
        sourceKind: 'pylon_api',
        sourceRefs: baseSourceRefs,
        state: registration.status,
        text: 'Pylon registered in the public activity timeline.',
        ts: registration.createdAt,
      }),
    ]

    if (registration.latestHeartbeatAt !== null) {
      events.push(
        eventWithCursor({
          actorRef: registration.pylonRef,
          blockerRefs: [],
          caveatRefs: [
            'caveat.public.activity_timeline.heartbeat_is_presence_not_paid_work',
          ],
          eventRef: `event.public.pylon_heartbeat.${registration.pylonRef}.${slugPart(
            registration.latestHeartbeatAt,
          )}`,
          kind: 'pylon_heartbeat',
          refs,
          sourceKind: 'pylon_presence',
          sourceRefs: baseSourceRefs,
          state: registration.latestHeartbeatStatus ?? 'unknown',
          text: 'Pylon heartbeat observed.',
          ts: registration.latestHeartbeatAt,
        }),
      )
    }

    if (registration.walletReady) {
      events.push(
        eventWithCursor({
          actorRef: registration.pylonRef,
          blockerRefs: [],
          caveatRefs: [
            'caveat.public.activity_timeline.wallet_ready_public_projection',
          ],
          eventRef: `event.public.wallet_ready.${registration.pylonRef}.${slugPart(
            registration.latestHeartbeatAt ?? registration.updatedAt,
          )}`,
          kind: 'wallet_ready',
          refs: uniqueRefs([...refs, registration.walletRef]),
          sourceKind: 'pylon_presence',
          sourceRefs: baseSourceRefs,
          state: 'ready',
          text: 'Pylon reports public wallet readiness with private receive data omitted.',
          ts: registration.latestHeartbeatAt ?? registration.updatedAt,
        }),
      )
    }

    if (pylonIsAssignmentReady(registration)) {
      events.push(
        eventWithCursor({
          actorRef: registration.pylonRef,
          blockerRefs: [],
          caveatRefs: [
            'caveat.public.activity_timeline.assignment_ready_is_not_assignment_or_payout',
          ],
          eventRef: `event.public.assignment_ready.${registration.pylonRef}.${slugPart(
            registration.latestHeartbeatAt ?? registration.updatedAt,
          )}`,
          kind: 'assignment_ready',
          refs,
          sourceKind: 'pylon_presence',
          sourceRefs: baseSourceRefs,
          state: 'ready',
          text: 'Pylon has public assignment-readiness evidence.',
          ts: registration.latestHeartbeatAt ?? registration.updatedAt,
        }),
      )
    }

    return events
  })

const windowEventKind = (
  window: TrainingWindowRecord,
): 'window_opened' | 'window_closed' =>
  window.state === 'active' || window.sealedAt === null
    ? 'window_opened'
    : 'window_closed'

const trainingEventsForRun = (input: {
  challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
  leases: ReadonlyArray<TrainingWindowLeaseRecord>
  run: TrainingRunRecord
  windows: ReadonlyArray<TrainingWindowRecord>
}): ReadonlyArray<PublicActivityTimelineEvent> => {
  const runSourceRefs = uniqueRefs([
    input.run.trainingRunRef,
    `route:/api/public/training/runs/${input.run.trainingRunRef}`,
    'route:/api/public/tassadar-run-summary',
    ...input.run.sourceRefs,
  ])
  const windowEvents = input.windows.map(window => {
    const kind = windowEventKind(window)
    const ts =
      kind === 'window_closed'
        ? window.reconciledAt ?? window.sealedAt ?? window.updatedAt
        : window.activatedAt ?? window.plannedAt

    return eventWithCursor({
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.activity_timeline.training_window_is_projection_only',
      ],
      eventRef: `event.public.${kind}.${window.windowRef}`,
      kind,
      refs: uniqueRefs([
        input.run.trainingRunRef,
        window.windowRef,
        ...window.sourceRefs,
        ...window.receiptRefs,
      ]),
      runRef: input.run.trainingRunRef,
      sourceKind: 'training_window',
      sourceRefs: uniqueRefs([
        window.windowRef,
        ...window.sourceRefs,
        ...runSourceRefs,
      ]),
      state: window.state,
      targetRef: window.windowRef,
      text:
        kind === 'window_closed'
          ? 'Training window closed in the public timeline.'
          : 'Training window opened in the public timeline.',
      ts,
      windowRef: window.windowRef,
    })
  })

  const leaseEvents = input.leases.map(lease =>
    eventWithCursor({
      actorRef: lease.pylonRef,
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.activity_timeline.claimed_work_is_not_accepted_or_paid',
      ],
      eventRef: `event.public.work_claimed.${lease.leaseRef}`,
      kind: 'work_claimed',
      refs: uniqueRefs([
        input.run.trainingRunRef,
        lease.windowRef,
        lease.leaseRef,
        lease.pylonRef,
        ...lease.receiptRefs,
      ]),
      runRef: input.run.trainingRunRef,
      sourceKind: 'training_window',
      sourceRefs: uniqueRefs([lease.leaseRef, lease.windowRef, ...runSourceRefs]),
      state: lease.state,
      targetRef: lease.windowRef,
      text: 'Training work lease claimed by a public Pylon ref.',
      ts: lease.claimedAt,
      windowRef: lease.windowRef,
    }),
  )

  const challengeEvents = input.challenges.flatMap(challenge => {
    const sourceRefs = uniqueRefs([
      challenge.challengeRef,
      `route:/api/public/training/verification-challenges/${challenge.challengeRef}`,
      ...runSourceRefs,
    ])
    const refs = uniqueRefs([
      challenge.challengeRef,
      challenge.contributionRef,
      challenge.windowRef,
      challenge.leasedToRef,
      ...challenge.commitmentRefs,
      ...challenge.verdictRefs,
    ])
    const events: PublicActivityTimelineEvent[] = []

    if (challenge.contributionRef !== null) {
      events.push(
        eventWithCursor({
          actorRef: challenge.leasedToRef ?? undefined,
          blockerRefs: [],
          caveatRefs: [
            'caveat.public.activity_timeline.trace_event_is_digest_ref_only',
          ],
          eventRef: `event.public.trace_submitted.${challenge.contributionRef}`,
          kind: 'trace_submitted',
          refs,
          runRef: challenge.trainingRunRef,
          sourceKind: 'training_trace',
          sourceRefs: uniqueRefs([challenge.contributionRef, ...sourceRefs]),
          targetRef: challenge.challengeRef,
          text: 'Training trace contribution submitted as public digest refs.',
          ts: challenge.createdAt,
          windowRef: challenge.windowRef ?? undefined,
        }),
      )
    }

    const verificationKind: PublicActivityTimelineEventKind =
      challenge.state === 'Verified'
        ? 'verification_verified'
        : challenge.state === 'Rejected'
          ? 'verification_rejected'
          : 'verification_queued'
    const verificationTs =
      verificationKind === 'verification_verified'
        ? challenge.verifiedAt ?? challenge.updatedAt
        : verificationKind === 'verification_rejected'
          ? challenge.rejectedAt ?? challenge.updatedAt
          : challenge.createdAt

    events.push(
      eventWithCursor({
        actorRef: challenge.leasedToRef ?? undefined,
        blockerRefs: [],
        caveatRefs: [
          'caveat.public.activity_timeline.verification_payloads_omitted',
        ],
        eventRef: `event.public.${verificationKind}.${challenge.challengeRef}`,
        kind: verificationKind,
        refs,
        runRef: challenge.trainingRunRef,
        sourceKind: 'training_verification',
        sourceRefs,
        state: challenge.state,
        targetRef: challenge.challengeRef,
        text: 'Training verification challenge state projected publicly.',
        ts: verificationTs,
        windowRef: challenge.windowRef ?? undefined,
      }),
    )

    return events
  })

  return [...windowEvents, ...leaseEvents, ...challengeEvents]
}

const receiptAmountSats = (
  record: NexusPaymentAuthorityReceiptRecord,
  projection: Record<string, unknown> | undefined,
): number | undefined =>
  typeof projection?.amountSats === 'number'
    ? projection.amountSats
    : typeof projection?.amountMsats === 'number'
      ? Math.floor(projection.amountMsats / 1000)
      : record.receiptKind === 'settlement_recorded'
        ? undefined
        : undefined

const receiptEvents = async (
  receiptStore: PublicActivityTimelineReceiptStore,
): Promise<ReadonlyArray<PublicActivityTimelineEvent>> => {
  const receipts = await receiptStore.listPaymentAuthorityReceipts(
    SOURCE_LOOKUP_LIMIT,
  )

  return (
    await Promise.all(
      receipts.map(async receipt => {
        const projection = parseJsonRecord(receipt.publicProjectionJson)
        const event =
          receipt.eventRef === null
            ? undefined
            : await receiptStore.readReconciliationEventByRef(receipt.eventRef)
        const movementMode =
          projection?.movementMode === 'real_bitcoin' ||
          projection?.moneyMovement === 'real_bitcoin'
            ? 'real_bitcoin'
            : 'simulation'
        const realBitcoinMoved =
          receipt.receiptKind === 'settlement_recorded' &&
          movementMode === 'real_bitcoin' &&
          (projection?.realBitcoinMoved === true || event?.status === 'matched')
        const amountSats = receiptAmountSats(receipt, projection)
        const runRef =
          typeof projection?.trainingRunRef === 'string'
            ? projection.trainingRunRef
            : undefined
        const windowRef =
          typeof projection?.windowRef === 'string'
            ? projection.windowRef
            : undefined
        const contributorRef =
          typeof projection?.contributorRef === 'string'
            ? projection.contributorRef
            : undefined
        const challengeRef =
          typeof projection?.verificationChallengeRef === 'string'
            ? projection.verificationChallengeRef
            : undefined
        const apiSourceRef = `${OPENAGENTS_PUBLIC_APP_URL}/api/public/nexus-pylon/receipts/${encodeURIComponent(
          receipt.receiptRef,
        )}`
        const baseRefs = uniqueRefs([
          receipt.receiptRef,
          receipt.eventRef,
          receipt.payoutAttemptRef,
          receipt.payoutIntentRef,
          contributorRef,
          runRef,
          windowRef,
          challengeRef,
          ...receipt.metadataRefs,
        ])
        const baseEvent = eventWithCursor({
          amountSats,
          actorRef: 'treasury.public.receipt',
          blockerRefs: [],
          caveatRefs:
            realBitcoinMoved === true
              ? []
              : ['caveat.public.activity_timeline.simulation_not_real_bitcoin'],
          eventRef: `event.public.settlement_recorded.${receipt.receiptRef}`,
          kind: 'settlement_recorded',
          realBitcoinMoved,
          refs: baseRefs,
          runRef,
          sourceKind: 'settlement_receipt',
          sourceRefs: uniqueRefs([receipt.receiptRef, apiSourceRef]),
          state:
            receipt.receiptKind === 'settlement_recorded'
              ? realBitcoinMoved
                ? 'settled'
                : 'settled_simulation'
              : receipt.receiptKind,
          targetRef: contributorRef,
          text:
            realBitcoinMoved === true
              ? 'Receipt-backed settlement recorded.'
              : 'Settlement receipt recorded without real Bitcoin movement.',
          ts: receipt.createdAt,
          windowRef,
        })

        return realBitcoinMoved !== true
          ? [baseEvent]
          : [
              baseEvent,
              eventWithCursor({
                amountSats,
                actorRef: 'treasury.public.receipt',
                blockerRefs: [],
                caveatRefs: [],
                eventRef: `event.public.real_bitcoin_moved.${receipt.receiptRef}`,
                kind: 'real_bitcoin_moved',
                realBitcoinMoved: true,
                refs: baseRefs,
                runRef,
                sourceKind: 'settlement_receipt',
                sourceRefs: uniqueRefs([receipt.receiptRef, apiSourceRef]),
                state: event?.status ?? 'confirmed',
                targetRef: contributorRef,
                text: 'Receipt-backed real Bitcoin movement confirmed.',
                ts: event?.createdAt ?? receipt.createdAt,
                windowRef,
              }),
            ]
      }),
    )
  ).flat()
}

const inferenceRequestRefFromReceiptRef = (receiptRef: string): string => {
  const prefix = 'receipt.inference.charge.'
  return receiptRef.startsWith(prefix) && receiptRef.length > prefix.length
    ? `request.khala.${slugPart(receiptRef.slice(prefix.length))}`
    : `request.khala.${slugPart(receiptRef)}`
}

const gatewayRefFromAdapterId = (adapterId: string): string =>
  adapterId.trim().length === 0
    ? 'gateway.khala.public'
    : `gateway.${slugPart(adapterId)}`

const inferenceReceiptEvents = (
  records: ReadonlyArray<InferenceReceiptRecord>,
): ReadonlyArray<PublicActivityTimelineEvent> =>
  records.flatMap(record => {
    const context = parseInferenceChargeContextRef(record.contextRef ?? '')
    if (context === undefined) {
      return []
    }

    const requestedModel = context.requestedModel
    if (requestedModel === undefined || !isKhalaModel(requestedModel)) {
      return []
    }

    const receipt = publicInferenceReceiptFromRecord(
      record,
      record.stateChangedAt,
    )
    if (receipt === null || receipt.kind !== 'charge') {
      return []
    }

    const receiptRoute = `${OPENAGENTS_PUBLIC_APP_URL}/api/public/inference/receipts/${encodeURIComponent(
      record.receiptRef,
    )}`
    const gatewayRef = gatewayRefFromAdapterId(context.adapterId)

    return [
      eventWithCursor({
        actorRef: gatewayRef,
        blockerRefs: [],
        caveatRefs: [
          'caveat.public.activity_timeline.inference_receipt_public_projection_only',
        ],
        eventRef: `event.public.khala_inference_served.${slugPart(
          record.receiptRef,
        )}`,
        kind: 'khala_inference_served',
        refs: uniqueRefs([
          record.receiptRef,
          inferenceRequestRefFromReceiptRef(record.receiptRef),
          requestedModel,
          gatewayRef,
        ]),
        sourceKind: 'inference_receipt',
        sourceRefs: uniqueRefs([
          record.receiptRef,
          receiptRoute,
          ...receipt.sourceRefs,
        ]),
        state: requestedModel,
        targetRef: record.receiptRef,
        text: 'Khala inference served with a public ledger receipt.',
        ts: record.stateChangedAt,
      }),
    ]
  })

const forumEvents = (
  records: ReadonlyArray<PublicActivityTimelineForumRecord>,
): ReadonlyArray<PublicActivityTimelineEvent> =>
  records.map(record =>
    eventWithCursor({
      actorRef: record.actorRef ?? undefined,
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.activity_timeline.forum_body_text_omitted',
      ],
      eventRef: `event.public.forum_${record.kind}.${record.eventRef}`,
      kind: record.kind === 'topic' ? 'forum_topic_created' : 'forum_posted',
      refs: uniqueRefs([record.topicRef, record.postRef]),
      sourceKind: 'forum',
      sourceRefs: record.sourceRefs,
      state: record.state,
      targetRef: record.postRef ?? record.topicRef,
      text:
        record.kind === 'topic'
          ? 'Public Forum topic created.'
          : 'Public Forum post created.',
      ts: record.createdAt,
    }),
  )

const artanisEvents = (
  records: ReadonlyArray<PublicActivityTimelineArtanisTickRecord>,
): ReadonlyArray<PublicActivityTimelineEvent> =>
  records.map(record =>
    eventWithCursor({
      actorRef: 'artanis.public.admin',
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.activity_timeline.artanis_tick_is_decision_log_only',
      ],
      eventRef: `event.public.artanis_tick.${record.decisionRef}`,
      kind: 'artanis_tick',
      refs: uniqueRefs([record.decisionRef, record.assignmentRef]),
      sourceKind: 'artanis',
      sourceRefs: record.sourceRefs,
      state: record.state,
      targetRef: record.assignmentRef ?? undefined,
      text: 'Artanis administrator tick decision recorded.',
      ts: record.createdAt,
    }),
  )

const capacityEvents = (
  records: ReadonlyArray<PublicActivityTimelineCapacitySnapshotRecord>,
): ReadonlyArray<PublicActivityTimelineEvent> =>
  records.map(record =>
    eventWithCursor({
      blockerRefs: [],
      caveatRefs: [
        'caveat.public.activity_timeline.capacity_snapshot_counts_only',
      ],
      eventRef: `event.public.capacity_snapshot.${record.snapshotRef}`,
      kind: 'capacity_snapshot',
      refs: [record.snapshotRef],
      sourceKind: 'capacity_funnel',
      sourceRefs: record.sourceRefs,
      state: record.aggregateState,
      text: 'Capacity funnel snapshot recorded.',
      ts: record.snapshotAt,
    }),
  )

const normalizeQuery = (
  query: Partial<PublicActivityTimelineQuery> | undefined,
): PublicActivityTimelineQuery => ({
  filterKinds: query?.filterKinds ?? [],
  filterSources: query?.filterSources ?? [],
  from: query?.from,
  limit:
    query?.limit === undefined
      ? DEFAULT_ACTIVITY_TIMELINE_LIMIT
      : Math.min(Math.max(1, Math.trunc(query.limit)), MAX_ACTIVITY_TIMELINE_LIMIT),
  since: query?.since,
  to: query?.to,
})

const applyQuery = (
  events: ReadonlyArray<PublicActivityTimelineEvent>,
  query: PublicActivityTimelineQuery,
): ReadonlyArray<PublicActivityTimelineEvent> => {
  const kindFilter = new Set(query.filterKinds)
  const sourceFilter = new Set(query.filterSources)

  return events.filter(event => {
    if (query.since !== undefined && event.cursor <= query.since) return false
    if (query.from !== undefined && event.ts < query.from) return false
    if (query.to !== undefined && event.ts > query.to) return false
    if (kindFilter.size > 0 && !kindFilter.has(event.kind)) return false
    if (sourceFilter.size > 0 && !sourceFilter.has(event.sourceKind)) return false
    return true
  })
}

const rangeForQuery = (
  query: PublicActivityTimelineQuery,
  observedAt: string,
): PublicActivityTimelineRange | undefined =>
  query.from === undefined &&
  query.to === undefined &&
  query.since === undefined &&
  query.filterKinds.length === 0
    ? undefined
    : {
        filterKinds: [...query.filterKinds],
        from: query.from ?? '0000-01-01T00:00:00.000Z',
        limit: query.limit,
        since: query.since ?? null,
        to: query.to ?? observedAt,
      }

const loadOrGap = async <T>(input: {
  load: () => Promise<ReadonlyArray<T>>
  observedAt: string
  sourceKind: PublicActivityTimelineSourceKind
}): Promise<
  | Readonly<{ ok: true; records: ReadonlyArray<T> }>
  | Readonly<{ gap: PublicActivityTimelineEvent; ok: false }>
> => {
  try {
    return { ok: true, records: await input.load() }
  } catch {
    return {
      gap: projectionGapEvent({
        blockerRef: `blocker.public.activity_timeline.${input.sourceKind}_unavailable`,
        observedAt: input.observedAt,
        sourceKind: input.sourceKind,
        text: `Public activity timeline could not read ${input.sourceKind}; emitted a projection gap instead of guessing.`,
      }),
      ok: false,
    }
  }
}

export const buildPublicActivityTimelineEnvelope = async (
  input: PublicActivityTimelineSourceInput,
): Promise<PublicActivityTimelineEnvelope> => {
  const observedAt = input.nowIso?.() ?? currentIsoTimestamp()
  const query = normalizeQuery(input.query)
  const allEvents: PublicActivityTimelineEvent[] = []
  const sourceLagItems: PublicActivityTimelineSourceLag[] = []

  if (input.pylonStore === undefined) {
    const gap = projectionGapEvent({
      blockerRef: 'blocker.public.activity_timeline.pylon_store_missing',
      observedAt,
      sourceKind: 'pylon_api',
      text: 'Pylon source store is not configured for this activity timeline read.',
    })
    allEvents.push(gap)
    sourceLagItems.push(
      sourceLag({
        blockerRefs: gap.blockerRefs,
        events: [],
        maxStalenessSeconds: 300,
        observedAt,
        sourceKind: 'pylon_api',
        sourceRefs: [],
        status: 'unavailable',
      }),
    )
  } else {
    const loaded = await loadOrGap({
      load: () => input.pylonStore!.listRegistrations(SOURCE_LOOKUP_LIMIT),
      observedAt,
      sourceKind: 'pylon_api',
    })
    if (loaded.ok) {
      const events = pylonEvents(loaded.records)
      allEvents.push(...events)
      sourceLagItems.push(
        sourceLag({
          events: events.filter(event => event.sourceKind === 'pylon_api'),
          maxStalenessSeconds: 300,
          observedAt,
          sourceKind: 'pylon_api',
          sourceRefs: ['route:/api/public/pylon-stats'],
        }),
        sourceLag({
          events: events.filter(event => event.sourceKind === 'pylon_presence'),
          maxStalenessSeconds: 300,
          observedAt,
          sourceKind: 'pylon_presence',
          sourceRefs: ['route:/api/public/pylon-stats'],
        }),
      )
    } else {
      allEvents.push(loaded.gap)
      sourceLagItems.push(
        sourceLag({
          blockerRefs: loaded.gap.blockerRefs,
          events: [],
          maxStalenessSeconds: 300,
          observedAt,
          sourceKind: 'pylon_api',
          sourceRefs: [],
          status: 'unavailable',
        }),
      )
    }
  }

  if (input.trainingStore === undefined) {
    const gap = projectionGapEvent({
      blockerRef: 'blocker.public.activity_timeline.training_store_missing',
      observedAt,
      sourceKind: 'training_window',
      text: 'Training source store is not configured for this activity timeline read.',
    })
    allEvents.push(gap)
    sourceLagItems.push(
      sourceLag({
        blockerRefs: gap.blockerRefs,
        events: [],
        maxStalenessSeconds: 0,
        observedAt,
        sourceKind: 'training_window',
        sourceRefs: [],
        status: 'unavailable',
      }),
    )
  } else {
    const loaded = await loadOrGap({
      load: async () => {
        const runs = await input.trainingStore!.listRuns(DEFAULT_TRAINING_RUN_LIMIT)
        const nested = await Promise.all(
          runs.map(async run => ({
            challenges:
              await input.trainingStore!.listVerificationChallengesForRun(
                run.trainingRunRef,
                SOURCE_LOOKUP_LIMIT,
              ),
            leases: await input.trainingStore!.listWindowLeasesForRun(
              run.trainingRunRef,
              SOURCE_LOOKUP_LIMIT,
            ),
            run,
            windows: await input.trainingStore!.listWindowsForRun(
              run.trainingRunRef,
              SOURCE_LOOKUP_LIMIT,
            ),
          })),
        )
        return nested
      },
      observedAt,
      sourceKind: 'training_window',
    })
    if (loaded.ok) {
      const events = loaded.records.flatMap(trainingEventsForRun)
      allEvents.push(...events)
      for (const sourceKind of [
        'training_window',
        'training_trace',
        'training_verification',
      ] as const) {
        sourceLagItems.push(
          sourceLag({
            events: events.filter(event => event.sourceKind === sourceKind),
            maxStalenessSeconds: 0,
            observedAt,
            sourceKind,
            sourceRefs: ['route:/api/public/tassadar-run-summary'],
          }),
        )
      }
    } else {
      allEvents.push(loaded.gap)
      sourceLagItems.push(
        sourceLag({
          blockerRefs: loaded.gap.blockerRefs,
          events: [],
          maxStalenessSeconds: 0,
          observedAt,
          sourceKind: 'training_window',
          sourceRefs: [],
          status: 'unavailable',
        }),
      )
    }
  }

  if (input.receiptStore === undefined) {
    const gap = projectionGapEvent({
      blockerRef: 'blocker.public.activity_timeline.settlement_receipt_store_missing',
      observedAt,
      sourceKind: 'settlement_receipt',
      text: 'Settlement receipt source store is not configured for this activity timeline read.',
    })
    allEvents.push(gap)
    sourceLagItems.push(
      sourceLag({
        blockerRefs: gap.blockerRefs,
        events: [],
        maxStalenessSeconds: 0,
        observedAt,
        sourceKind: 'settlement_receipt',
        sourceRefs: [],
        status: 'unavailable',
      }),
    )
  } else {
    const loaded = await loadOrGap({
      load: () => receiptEvents(input.receiptStore!),
      observedAt,
      sourceKind: 'settlement_receipt',
    })
    if (loaded.ok) {
      allEvents.push(...loaded.records)
      sourceLagItems.push(
        sourceLag({
          events: loaded.records,
          maxStalenessSeconds: 0,
          observedAt,
          sourceKind: 'settlement_receipt',
          sourceRefs: ['route:/api/public/nexus-pylon/receipts/{receiptRef}'],
        }),
      )
    } else {
      allEvents.push(loaded.gap)
      sourceLagItems.push(
        sourceLag({
          blockerRefs: loaded.gap.blockerRefs,
          events: [],
          maxStalenessSeconds: 0,
          observedAt,
          sourceKind: 'settlement_receipt',
          sourceRefs: [],
          status: 'unavailable',
        }),
      )
    }
  }

  if (input.inferenceReceiptStore === undefined) {
    const gap = projectionGapEvent({
      blockerRef: 'blocker.public.activity_timeline.inference_receipt_store_missing',
      observedAt,
      sourceKind: 'inference_receipt',
      text: 'Inference receipt source store is not configured for this activity timeline read.',
    })
    allEvents.push(gap)
    sourceLagItems.push(
      sourceLag({
        blockerRefs: gap.blockerRefs,
        events: [],
        maxStalenessSeconds: 0,
        observedAt,
        sourceKind: 'inference_receipt',
        sourceRefs: [],
        status: 'unavailable',
      }),
    )
  } else {
    const loaded = await loadOrGap({
      load: () =>
        input.inferenceReceiptStore!.listRecentInferenceReceipts(
          SOURCE_LOOKUP_LIMIT,
        ),
      observedAt,
      sourceKind: 'inference_receipt',
    })
    if (loaded.ok) {
      const events = inferenceReceiptEvents(loaded.records)
      allEvents.push(...events)
      sourceLagItems.push(
        sourceLag({
          events,
          maxStalenessSeconds: 0,
          observedAt,
          sourceKind: 'inference_receipt',
          sourceRefs: ['route:/api/public/inference/receipts/{receiptRef}'],
        }),
      )
    } else {
      allEvents.push(loaded.gap)
      sourceLagItems.push(
        sourceLag({
          blockerRefs: loaded.gap.blockerRefs,
          events: [],
          maxStalenessSeconds: 0,
          observedAt,
          sourceKind: 'inference_receipt',
          sourceRefs: [],
          status: 'unavailable',
        }),
      )
    }
  }

  if (input.forumStore === undefined) {
    const gap = projectionGapEvent({
      blockerRef: 'blocker.public.activity_timeline.forum_store_missing',
      observedAt,
      sourceKind: 'forum',
      text: 'Forum source store is not configured for this activity timeline read.',
    })
    allEvents.push(gap)
    sourceLagItems.push(
      sourceLag({
        blockerRefs: gap.blockerRefs,
        events: [],
        maxStalenessSeconds: 600,
        observedAt,
        sourceKind: 'forum',
        sourceRefs: [],
        status: 'unavailable',
      }),
    )
  } else {
    const loaded = await loadOrGap({
      load: () => input.forumStore!.listRecentActivity(SOURCE_LOOKUP_LIMIT),
      observedAt,
      sourceKind: 'forum',
    })
    if (loaded.ok) {
      const events = forumEvents(loaded.records)
      allEvents.push(...events)
      sourceLagItems.push(
        sourceLag({
          events,
          maxStalenessSeconds: 600,
          observedAt,
          sourceKind: 'forum',
          sourceRefs: ['route:/api/forum/posts', 'route:/api/forum'],
        }),
      )
    } else {
      allEvents.push(loaded.gap)
      sourceLagItems.push(
        sourceLag({
          blockerRefs: loaded.gap.blockerRefs,
          events: [],
          maxStalenessSeconds: 600,
          observedAt,
          sourceKind: 'forum',
          sourceRefs: [],
          status: 'unavailable',
        }),
      )
    }
  }

  if (input.artanisStore === undefined) {
    const gap = projectionGapEvent({
      blockerRef: 'blocker.public.activity_timeline.artanis_store_missing',
      observedAt,
      sourceKind: 'artanis',
      text: 'Artanis source store is not configured for this activity timeline read.',
    })
    allEvents.push(gap)
    sourceLagItems.push(
      sourceLag({
        blockerRefs: gap.blockerRefs,
        events: [],
        maxStalenessSeconds: 300,
        observedAt,
        sourceKind: 'artanis',
        sourceRefs: [],
        status: 'unavailable',
      }),
    )
  } else {
    const loaded = await loadOrGap({
      load: () => input.artanisStore!.listRecentTicks(SOURCE_LOOKUP_LIMIT),
      observedAt,
      sourceKind: 'artanis',
    })
    if (loaded.ok) {
      const events = artanisEvents(loaded.records)
      allEvents.push(...events)
      sourceLagItems.push(
        sourceLag({
          events,
          maxStalenessSeconds: 300,
          observedAt,
          sourceKind: 'artanis',
          sourceRefs: ['route:/api/public/artanis/admin-ticks'],
        }),
      )
    } else {
      allEvents.push(loaded.gap)
      sourceLagItems.push(
        sourceLag({
          blockerRefs: loaded.gap.blockerRefs,
          events: [],
          maxStalenessSeconds: 300,
          observedAt,
          sourceKind: 'artanis',
          sourceRefs: [],
          status: 'unavailable',
        }),
      )
    }
  }

  if (input.capacityStore === undefined) {
    const gap = projectionGapEvent({
      blockerRef: 'blocker.public.activity_timeline.capacity_funnel_store_missing',
      observedAt,
      sourceKind: 'capacity_funnel',
      text: 'Capacity funnel source store is not configured for this activity timeline read.',
    })
    allEvents.push(gap)
    sourceLagItems.push(
      sourceLag({
        blockerRefs: gap.blockerRefs,
        events: [],
        maxStalenessSeconds: 3600,
        observedAt,
        sourceKind: 'capacity_funnel',
        sourceRefs: [],
        status: 'unavailable',
      }),
    )
  } else {
    const loaded = await loadOrGap({
      load: () => input.capacityStore!.listRecentSnapshots(SOURCE_LOOKUP_LIMIT),
      observedAt,
      sourceKind: 'capacity_funnel',
    })
    if (loaded.ok) {
      const events = capacityEvents(loaded.records)
      allEvents.push(...events)
      sourceLagItems.push(
        sourceLag({
          events,
          maxStalenessSeconds: 3600,
          observedAt,
          sourceKind: 'capacity_funnel',
          sourceRefs: ['route:/api/public/pylon-capacity-funnel/history'],
        }),
      )
    } else {
      allEvents.push(loaded.gap)
      sourceLagItems.push(
        sourceLag({
          blockerRefs: loaded.gap.blockerRefs,
          events: [],
          maxStalenessSeconds: 3600,
          observedAt,
          sourceKind: 'capacity_funnel',
          sourceRefs: [],
          status: 'unavailable',
        }),
      )
    }
  }

  const ordered = orderPublicActivityTimelineEvents(allEvents)
  const queryFiltered = applyQuery(ordered, query)
  // Final per-event safety net: a single event that slips past the per-source
  // sanitizers (residual unsafe material, missing source/blocker refs, a
  // receipt-source requirement, or a cursor mismatch) must NEVER 500 the whole
  // public feed. Drop the offending event(s) and keep serving the rest of the
  // timeline. Dropping can only make the projection safer, and it preserves the
  // "honest, public-safe envelope, never a hard failure" contract for this
  // surface. The envelope-level assert below remains as a cross-event backstop.
  const filtered = queryFiltered.filter(event => {
    try {
      assertPublicActivityTimelineEventSafe(event)
      return true
    } catch {
      return false
    }
  })
  const page = filtered.slice(0, query.limit)
  const envelope: PublicActivityTimelineEnvelope = {
    events: page,
    generatedAt: observedAt,
    nextCursor:
      filtered.length > page.length ? page[page.length - 1]?.cursor ?? null : null,
    range: rangeForQuery(query, observedAt),
    schemaVersion: PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION,
    sourceLag: sourceLagItems,
    staleness: liveAtReadStaleness(timelineRebuildRefs),
  }

  try {
    return assertPublicActivityTimelineEnvelopeSafe(envelope)
  } catch (error) {
    throw new PublicActivityTimelineProjectionUnsafe(
      error instanceof Error
        ? error.message
        : 'Public activity timeline projection failed safety validation.',
    )
  }
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

export const publicActivityTimelineQueryFromUrl = (
  url: URL,
): PublicActivityTimelineQuery | Response => {
  const rawLimit = url.searchParams.get('limit')
  const parsedLimit =
    rawLimit === null ? DEFAULT_ACTIVITY_TIMELINE_LIMIT : Number(rawLimit)

  if (!Number.isFinite(parsedLimit)) {
    return Response.json({ error: 'invalid_limit' }, { status: 400 })
  }

  const filterKinds = valuesFromParams(url.searchParams, 'kind')
  const invalidKind = filterKinds.find(
    kind =>
      !publicActivityTimelineEventKinds.includes(
        kind as PublicActivityTimelineEventKind,
      ),
  )

  if (invalidKind !== undefined) {
    return Response.json(
      { error: 'invalid_event_kind', value: invalidKind },
      { status: 400 },
    )
  }

  const filterSources = valuesFromParams(url.searchParams, 'source')
  const invalidSource = filterSources.find(
    source =>
      !publicActivityTimelineSourceKinds.includes(
        source as PublicActivityTimelineSourceKind,
      ),
  )

  if (invalidSource !== undefined) {
    return Response.json(
      { error: 'invalid_source_kind', value: invalidSource },
      { status: 400 },
    )
  }

  return {
    filterKinds: filterKinds as ReadonlyArray<PublicActivityTimelineEventKind>,
    filterSources:
      filterSources as ReadonlyArray<PublicActivityTimelineSourceKind>,
    from: url.searchParams.get('from') ?? undefined,
    limit: Math.min(
      Math.max(1, Math.trunc(parsedLimit)),
      MAX_ACTIVITY_TIMELINE_LIMIT,
    ),
    since: url.searchParams.get('since') ?? undefined,
    to: url.searchParams.get('to') ?? undefined,
  }
}
