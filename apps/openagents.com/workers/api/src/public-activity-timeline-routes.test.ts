import {
  assertPublicActivityTimelineEnvelopeSafe,
  publicActivityTimelineEventKinds,
  publicActivityTimelineHasUnsafeMaterial,
  publicActivityTimelineSourceKinds,
  type PublicActivityTimelineEnvelope,
} from '@openagentsinc/public-activity-timeline'
import type { SyncSql, SyncTransactionSql } from '@openagentsinc/khala-sync-server'
import { projectActivityTimelineSnapshotBestEffort } from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'
import { beforeEach, describe, expect, test } from 'vitest'

import { invalidateActivityTimelineSnapshotCacheForTests } from './khala-sync-public-activity-timeline'
import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'

import type {
  InferenceReceiptRecord,
} from './inference-receipts'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import type {
  PublicActivityTimelineArtanisStore,
  PublicActivityTimelineCapacityStore,
  PublicActivityTimelineForumStore,
  PublicActivityTimelineInferenceReceiptStore,
  PublicActivityTimelinePylonStore,
  PublicActivityTimelineReceiptStore,
  PublicActivityTimelineTrainingStore,
} from './public-activity-timeline'
import {
  handlePublicActivityTimelineApi,
  handlePublicActivityTimelineStreamApi,
} from './public-activity-timeline-routes'
import type { PylonApiRegistrationRecord } from './pylon-api'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

const nowIso = '2026-06-18T18:10:00.000Z'
const runRef = 'run.tassadar.executor.20260615'
const windowRef = 'training.window.public.timeline.w1'
const pylonRef = 'pylon.public.timeline.worker'
const validatorRef = 'pylon.public.timeline.validator'
const realReceiptRef = 'receipt.nexus.public_activity_timeline.real.1'
const simulationReceiptRef =
  'receipt.nexus.public_activity_timeline.simulation.1'
const khalaReceiptRef = 'receipt.inference.charge.chatcmpl_public_timeline_1'

const request = (path = '/api/public/activity-timeline') =>
  new Request(`https://openagents.com${path}`)

const streamRequest = (
  path = '/api/public/activity-timeline/stream',
  init?: RequestInit,
) => new Request(`https://openagents.com${path}`, init)

const route = async (
  path: string,
  input: Parameters<typeof handlePublicActivityTimelineApi>[1] = fullInput(),
): Promise<Response> =>
  Effect.runPromise(handlePublicActivityTimelineApi(request(path), input))

const streamRoute = async (
  path: string,
  input: Parameters<typeof handlePublicActivityTimelineStreamApi>[1] = fullInput(),
  init?: RequestInit,
): Promise<Response> =>
  Effect.runPromise(
    handlePublicActivityTimelineStreamApi(streamRequest(path, init), input),
  )

const decode = async (response: Response) =>
  assertPublicActivityTimelineEnvelopeSafe(
    (await response.json()) as PublicActivityTimelineEnvelope,
  )

type SseFrame = Readonly<{
  data?: unknown
  event?: string
  id?: string
  retry?: string
}>

const parseSseFrames = (text: string): SseFrame[] =>
  text
    .split(/\n\n/)
    .map(frame => frame.trim())
    .filter(frame => frame !== '')
    .map(frame => {
      const parsed: { dataLines: string[]; event?: string; id?: string; retry?: string } = {
        dataLines: [],
      }
      for (const line of frame.split('\n')) {
        if (line.startsWith(':')) continue
        const separator = line.indexOf(':')
        const key = separator === -1 ? line : line.slice(0, separator)
        const value =
          separator === -1 ? '' : line.slice(separator + 1).replace(/^ /, '')
        if (key === 'data') parsed.dataLines.push(value)
        if (key === 'event') parsed.event = value
        if (key === 'id') parsed.id = value
        if (key === 'retry') parsed.retry = value
      }
      const dataText = parsed.dataLines.join('\n')
      return {
        ...(parsed.event === undefined ? {} : { event: parsed.event }),
        ...(parsed.id === undefined ? {} : { id: parsed.id }),
        ...(parsed.retry === undefined ? {} : { retry: parsed.retry }),
        ...(dataText === '' ? {} : { data: JSON.parse(dataText) as unknown }),
      }
    })

const frameDataRecord = (frame: SseFrame): Record<string, unknown> => {
  expect(frame.data).toBeTypeOf('object')
  expect(frame.data).not.toBeNull()
  return frame.data as Record<string, unknown>
}

const eventFromFrame = (frame: SseFrame): PublicActivityTimelineEnvelope['events'][number] => {
  const data = frameDataRecord(frame)
  expect(data.event).toBeTypeOf('object')
  return data.event as PublicActivityTimelineEnvelope['events'][number]
}

const envelopeFromSseFrames = (frames: readonly SseFrame[]) => {
  const meta = frames.find(frame => frame.event === 'activity_timeline_meta')
  if (meta === undefined) throw new Error('missing activity timeline meta frame')
  const metaData = frameDataRecord(meta)
  const nextCursor = metaData.nextCursor
  const envelope = {
    schemaVersion: metaData.schemaVersion,
    generatedAt: metaData.generatedAt,
    staleness: metaData.staleness,
    range: metaData.range,
    sourceLag: metaData.sourceLag,
    nextCursor: typeof nextCursor === 'string' ? nextCursor : null,
    events: frames
      .filter(frame => frame.id !== undefined && frame.data !== undefined)
      .map(eventFromFrame),
  } as PublicActivityTimelineEnvelope
  assertPublicActivityTimelineEnvelopeSafe(envelope)
  return envelope
}

const registration = (): PylonApiRegistrationRecord => ({
  capabilityRefs: ['capability.public.tassadar_executor_trace'],
  clientProtocolVersion: '1.0.0',
  clientVersion: 'openagents.pylon@1.0.0',
  createdAt: '2026-06-18T18:00:00.000Z',
  displayName: 'Timeline Worker',
  id: 'registration-public-activity-timeline',
  latestCapacityRefs: ['capacity.public.gpu_available'],
  latestHeartbeatAt: '2026-06-18T18:00:02.000Z',
  latestHeartbeatStatus: 'online',
  latestHealthRefs: ['health.public.ok'],
  latestLoadRefs: ['load.public.low'],
  latestResourceMode: 'background_20',
  ownerAgentCredentialId: 'credential_public_timeline',
  ownerAgentTokenPrefix: 'oa_agent_test',
  ownerAgentUserId: 'agent_public_timeline',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef,
  resourceMode: 'background_20',
  status: 'active',
  updatedAt: '2026-06-18T18:00:02.000Z',
  walletReady: true,
  walletRef: 'wallet.public.timeline.ready',
})

const runRecord: TrainingRunRecord = {
  createdAt: '2026-06-18T17:59:00.000Z',
  id: 'run-public-activity-timeline',
  manifest: null,
  maxAllowedStale: 0,
  promiseRef: 'training.decentralized_training_launch.v1',
  publicProjectionJson: '{}',
  receiptRefs: [realReceiptRef, simulationReceiptRef],
  sealInFlightAt: null,
  sealPublicationCadenceWindows: 1,
  sourceRefs: ['source.public.timeline.run'],
  state: 'active',
  trainingRunRef: runRef,
  updatedAt: '2026-06-18T18:00:00.000Z',
}

const activeWindow: TrainingWindowRecord = {
  activatedAt: '2026-06-18T18:00:05.000Z',
  datasetRefs: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'window-public-activity-timeline',
  plannedAt: '2026-06-18T18:00:04.000Z',
  priority: 100,
  publicProjectionJson: '{}',
  receiptRefs: [],
  reconciledAt: null,
  sealMetadata: null,
  sealedAt: null,
  sourceRefs: ['source.public.timeline.window'],
  state: 'active',
  trainingRunRef: runRef,
  updatedAt: '2026-06-18T18:00:05.000Z',
  windowRef,
}

const sealedWindow: TrainingWindowRecord = {
  ...activeWindow,
  activatedAt: '2026-06-18T18:00:03.000Z',
  id: 'window-public-activity-timeline-sealed',
  reconciledAt: null,
  sealedAt: '2026-06-18T18:00:13.000Z',
  state: 'sealed',
  updatedAt: '2026-06-18T18:00:13.000Z',
  windowRef: 'training.window.public.timeline.w0',
}

const leaseRecord: TrainingWindowLeaseRecord = {
  claimedAt: '2026-06-18T18:00:06.000Z',
  id: 'lease-public-activity-timeline',
  leaseExpiresAt: '2026-06-18T19:00:06.000Z',
  leaseRef: 'training.lease.public.timeline.1',
  publicProjectionJson: '{}',
  pylonRef,
  receiptRefs: [],
  state: 'active',
  trainingRunRef: runRef,
  windowRef,
}

const challenge = (
  state: 'Queued' | 'Verified' | 'Rejected',
): TrainingVerificationChallengeRecord => {
  const stateSlug = state.toLowerCase()

  return {
    challengeRef: `training.verification.challenge.public.timeline.${stateSlug}`,
    commitmentRefs: [`trace.commitment.public.timeline.${stateSlug}`],
    contributionRef:
      state === 'Queued'
        ? null
        : `trace.contribution.public.timeline.${stateSlug}`,
    createdAt:
      state === 'Queued'
        ? '2026-06-18T18:00:06.500Z'
        : state === 'Verified'
          ? '2026-06-18T18:00:07.000Z'
          : '2026-06-18T18:00:08.000Z',
    failureCodes: state === 'Rejected' ? ['ExecutorTraceMismatch'] : [],
    homeworkKind: 'tassadar_executor_trace',
    id: `challenge-public-activity-timeline-${state}`,
    leaseExpiresAt: null,
    leaseRef: null,
    leasedToRef: state === 'Queued' ? null : validatorRef,
    maxAttempts: 1,
    payloadJson: '{}',
    publicProjectionJson: '{}',
    rejectedAt: state === 'Rejected' ? '2026-06-18T18:00:12.000Z' : null,
    samplingPolicy: 'per_contribution',
    state,
    timedOutAt: null,
    trainingRunRef: runRef,
    updatedAt:
      state === 'Queued'
        ? '2026-06-18T18:00:06.500Z'
        : state === 'Verified'
          ? '2026-06-18T18:00:11.000Z'
          : '2026-06-18T18:00:12.000Z',
    verdictRefs:
      state === 'Queued' ? [] : [`verdict.public.timeline.${stateSlug}`],
    verificationClass: 'exact_trace_replay',
    verifiedAt: state === 'Verified' ? '2026-06-18T18:00:11.000Z' : null,
    windowRef,
  }
}

const receipt = (
  input: Readonly<{
    amountSats: number
    eventRef: string
    movementMode: 'real_bitcoin' | 'simulation'
    receiptRef: string
  }>,
): NexusPaymentAuthorityReceiptRecord => ({
  archivedAt: null,
  audience: 'public',
  createdAt:
    input.movementMode === 'real_bitcoin'
      ? '2026-06-18T18:00:14.000Z'
      : '2026-06-18T18:00:09.000Z',
  eventRef: input.eventRef,
  id: `receipt_${input.receiptRef}`,
  metadataRefs: ['metadata.public.timeline.receipt'],
  payoutAttemptRef: `attempt.public.timeline.${input.receiptRef}`,
  payoutIntentRef: `intent.public.timeline.${input.receiptRef}`,
  publicProjectionJson: JSON.stringify({
    amountSats: input.amountSats,
    contributorRef: pylonRef,
    movementMode: input.movementMode,
    realBitcoinMoved: input.movementMode === 'real_bitcoin',
    state: 'settled',
    trainingRunRef: runRef,
    verificationChallengeRef:
      'training.verification.challenge.public.timeline.verified',
    windowRef,
  }),
  receiptKind: 'settlement_recorded',
  receiptRef: input.receiptRef,
})

const reconciliationEvent = (
  eventRef: string,
  createdAt: string,
): NexusTreasuryPayoutReconciliationEventRecord => ({
  adapterKind: 'spark_treasury',
  archivedAt: null,
  createdAt,
  eventRef,
  externalEventRef: `external.public.timeline.${eventRef}`,
  id: `event_${eventRef}`,
  idempotencyKeyHash: `idempotency.${eventRef}`,
  metadataRefs: ['metadata.public.timeline.reconciliation'],
  payoutAttemptRef: `attempt.public.timeline.${eventRef}`,
  payoutIntentRef: `intent.public.timeline.${eventRef}`,
  providerRef: 'provider.public.spark_treasury',
  publicProjectionJson: '{}',
  resultRef: `result.public.timeline.${eventRef}`,
  status: 'matched',
})

const pylonStore = (): PublicActivityTimelinePylonStore => ({
  listRegistrations: async () => [registration()],
})

const trainingStore = (): PublicActivityTimelineTrainingStore => ({
  listRuns: async () => [runRecord],
  listVerificationChallengesForRun: async () => [
    challenge('Queued'),
    challenge('Verified'),
    challenge('Rejected'),
  ],
  listWindowLeasesForRun: async () => [leaseRecord],
  listWindowsForRun: async () => [activeWindow, sealedWindow],
})

const receiptStore = (): PublicActivityTimelineReceiptStore => {
  const receipts = [
    receipt({
      amountSats: 5,
      eventRef: 'event.public.timeline.simulation',
      movementMode: 'simulation',
      receiptRef: simulationReceiptRef,
    }),
    receipt({
      amountSats: 1000,
      eventRef: 'event.public.timeline.real',
      movementMode: 'real_bitcoin',
      receiptRef: realReceiptRef,
    }),
  ]
  const events = new Map([
    [
      'event.public.timeline.real',
      reconciliationEvent(
        'event.public.timeline.real',
        '2026-06-18T18:00:15.000Z',
      ),
    ],
    [
      'event.public.timeline.simulation',
      reconciliationEvent(
        'event.public.timeline.simulation',
        '2026-06-18T18:00:10.000Z',
      ),
    ],
  ])

  return {
    listPaymentAuthorityReceipts: async () => receipts,
    readReconciliationEventByRef: async eventRef => events.get(eventRef),
  }
}

const inferenceReceiptStore = (): PublicActivityTimelineInferenceReceiptStore => ({
  listRecentInferenceReceipts: async () => [
    {
      contextRef:
        'inference:fireworks:served:accounts%2Ffireworks%2Fmodels%2Fqwen3:tokens:42:requested:openagents%2Fkhala',
      createdAt: '2026-06-18T18:00:08.500Z',
      payInType: 'adjustment',
      receiptRef: khalaReceiptRef,
      state: 'paid',
      stateChangedAt: '2026-06-18T18:00:08.750Z',
    } satisfies InferenceReceiptRecord,
    {
      contextRef:
        'inference:fireworks:served:accounts%2Ffireworks%2Fmodels%2Fqwen3:tokens:42:requested:accounts%2Ffireworks%2Fmodels%2Fqwen3',
      createdAt: '2026-06-18T18:00:08.000Z',
      payInType: 'adjustment',
      receiptRef: 'receipt.inference.charge.non_khala',
      state: 'paid',
      stateChangedAt: '2026-06-18T18:00:08.250Z',
    } satisfies InferenceReceiptRecord,
  ],
})

const forumStore = (): PublicActivityTimelineForumStore => ({
  listRecentActivity: async () => [
    {
      actorRef: 'agent.public.forum.author',
      createdAt: '2026-06-18T18:00:16.000Z',
      eventRef: 'forum.topic.public.timeline.1',
      kind: 'topic',
      postRef: null,
      sourceRefs: ['forum.topic.public.timeline.1', 'route:/api/forum'],
      state: 'open',
      title: 'Timeline topic',
      topicRef: 'forum.topic.public.timeline.1',
    },
    {
      actorRef: 'agent.public.forum.author',
      createdAt: '2026-06-18T18:00:17.000Z',
      eventRef: 'forum.post.public.timeline.1',
      kind: 'post',
      postRef: 'forum.post.public.timeline.1',
      sourceRefs: [
        'forum.topic.public.timeline.1',
        'forum.post.public.timeline.1',
        'route:/api/forum/posts',
      ],
      state: 'visible',
      title: 'Timeline topic',
      topicRef: 'forum.topic.public.timeline.1',
    },
  ],
})

const artanisStore = (): PublicActivityTimelineArtanisStore => ({
  listRecentTicks: async () => [
    {
      assignmentRef: null,
      createdAt: '2026-06-18T18:00:18.000Z',
      decisionRef: 'tick_decision.public.timeline.1',
      sourceRefs: [
        'tick_decision.public.timeline.1',
        'route:/api/public/artanis/admin-ticks',
      ],
      state: 'no_action',
    },
  ],
})

const capacityStore = (): PublicActivityTimelineCapacityStore => ({
  listRecentSnapshots: async () => [
    {
      aggregateState: 'total:3',
      snapshotAt: '2026-06-18T18:00:19.000Z',
      snapshotRef: 'pylon_capacity_funnel_snapshot_public_timeline_1',
      sourceRefs: [
        'pylon_capacity_funnel_snapshot_public_timeline_1',
        'route:/api/public/pylon-capacity-funnel/history',
      ],
    },
  ],
})

const fullInput = () => ({
  artanisStore: artanisStore(),
  capacityStore: capacityStore(),
  forumStore: forumStore(),
  inferenceReceiptStore: inferenceReceiptStore(),
  nowIso: () => nowIso,
  pylonStore: pylonStore(),
  receiptStore: receiptStore(),
  trainingStore: trainingStore(),
})

describe('public activity timeline route', () => {
  test('unions public-safe source families into a cursor ordered timeline', async () => {
    const response = await route('/api/public/activity-timeline?limit=200')
    const body = await decode(response)
    const kinds = body.events.map(event => event.kind)
    const nonGapEventKinds = publicActivityTimelineEventKinds.filter(
      kind => kind !== 'projection_gap',
    )
    const nonGapSourceKinds = publicActivityTimelineSourceKinds.filter(
      kind => kind !== 'projection_gap',
    )

    expect(response.status).toBe(200)
    expect(body.schemaVersion).toBe('openagents.public_activity_timeline.v1')
    expect([...new Set(kinds)].sort()).toEqual([...nonGapEventKinds].sort())
    expect([...new Set(body.sourceLag.map(item => item.sourceKind))].sort()).toEqual(
      [...nonGapSourceKinds].sort(),
    )
    expect(body.events.every(event => event.cursor.includes(event.eventRef))).toBe(
      true,
    )
    const khala = body.events.find(event => event.kind === 'khala_inference_served')
    expect(khala).toMatchObject({
      actorRef: 'gateway.fireworks',
      sourceKind: 'inference_receipt',
      state: 'openagents/khala',
      targetRef: khalaReceiptRef,
    })
    expect(khala?.sourceRefs).toContain(khalaReceiptRef)
    expect(body.events.some(event => event.refs.includes('receipt.inference.charge.non_khala'))).toBe(
      false,
    )
    expect(
      body.events.every(
        event => event.sourceRefs.length > 0 || event.blockerRefs.length > 0,
      ),
    ).toBe(true)
    expect(JSON.stringify(body)).not.toMatch(
      /raw_(trace|payload|log)|secret|token_secret|sk-[a-z0-9]/i,
    )
  })

  test('omits private source payload material from the public projection', async () => {
    const unsafeProjectionReceipt = receipt({
      amountSats: 1000,
      eventRef: 'event.public.timeline.real',
      movementMode: 'real_bitcoin',
      receiptRef: realReceiptRef,
    })
    const response = await route('/api/public/activity-timeline?limit=200', {
      ...fullInput(),
      forumStore: {
        listRecentActivity: async () =>
          (await forumStore().listRecentActivity(48)).map(record => ({
            ...record,
            title: 'raw_prompt customer_email@example.com sk-test-private',
          })),
      },
      pylonStore: {
        listRegistrations: async () => [
          {
            ...registration(),
            displayName: 'private customer email@example.com token_secret',
            ownerAgentTokenPrefix: 'token_secret_not_projected',
          },
        ],
      },
      receiptStore: {
        listPaymentAuthorityReceipts: async () => [
          {
            ...unsafeProjectionReceipt,
            publicProjectionJson: JSON.stringify({
              amountSats: 1000,
              contributorRef: pylonRef,
              movementMode: 'real_bitcoin',
              paymentPreimage: 'payment_preimage_not_projected',
              providerToken: 'provider_token_not_projected',
              rawPrompt: 'raw_prompt_not_projected',
              realBitcoinMoved: true,
              state: 'settled',
              trainingRunRef: runRef,
              verificationChallengeRef:
                'training.verification.challenge.public.timeline.verified',
              windowRef,
            }),
          },
        ],
        readReconciliationEventByRef: receiptStore().readReconciliationEventByRef,
      },
      trainingStore: {
        ...trainingStore(),
        listRuns: async () => [
          {
            ...runRecord,
            publicProjectionJson: JSON.stringify({
              localPath: '/Users/example/private-source',
              rawTrace: 'raw_trace_not_projected',
            }),
          },
        ],
        listVerificationChallengesForRun: async () => [
          {
            ...challenge('Verified'),
            payloadJson: JSON.stringify({
              paymentPreimage: 'payment_preimage_not_projected',
              rawPayload: 'raw_payload_not_projected',
            }),
          },
        ],
      },
    })
    const body = await decode(response)

    expect(response.status).toBe(200)
    expect(publicActivityTimelineHasUnsafeMaterial(body)).toBe(false)
    expect(JSON.stringify(body)).not.toMatch(
      /@|\/Users\/|payment_preimage|provider_token|raw_(payload|prompt|trace)|sk-[a-z0-9]|token_secret/i,
    )
  })

  test('drops a single unsafe event instead of failing the whole feed', async () => {
    // Regression: a recent forum event whose actorRef carries an "@" (matched
    // by the public-safe material guard) must NOT 500 the entire activity feed.
    // The offending event is dropped; the rest of the timeline still serves a
    // safe 200 envelope.
    const response = await route('/api/public/activity-timeline?limit=200', {
      ...fullInput(),
      forumStore: {
        listRecentActivity: async () =>
          (await forumStore().listRecentActivity(48)).map((record, index) =>
            index === 0
              ? { ...record, actorRef: 'ops@example.com' }
              : record,
          ),
      },
    })
    const body = await decode(response)

    expect(response.status).toBe(200)
    expect(publicActivityTimelineHasUnsafeMaterial(body)).toBe(false)
    // The unsafe forum topic event is excluded.
    expect(
      body.events.some(
        event =>
          event.eventRef ===
          'event.public.forum_topic.forum.topic.public.timeline.1',
      ),
    ).toBe(false)
    // The remaining safe forum post event still projects.
    expect(
      body.events.some(
        event =>
          event.eventRef ===
          'event.public.forum_post.forum.post.public.timeline.1',
      ),
    ).toBe(true)
  })

  test('stream omits private source payload material from public frames', async () => {
    const response = await streamRoute('/api/public/activity-timeline/stream?limit=200', {
      ...fullInput(),
      forumStore: {
        listRecentActivity: async () =>
          (await forumStore().listRecentActivity(48)).map(record => ({
            ...record,
            title: 'raw_prompt customer_email@example.com sk-test-private',
          })),
      },
      pylonStore: {
        listRegistrations: async () => [
          {
            ...registration(),
            displayName: 'private customer email@example.com token_secret',
            ownerAgentTokenPrefix: 'token_secret_not_projected',
          },
        ],
      },
      trainingStore: {
        ...trainingStore(),
        listVerificationChallengesForRun: async () => [
          {
            ...challenge('Verified'),
            payloadJson: JSON.stringify({
              paymentPreimage: 'payment_preimage_not_projected',
              rawPayload: 'raw_payload_not_projected',
            }),
          },
        ],
      },
    })
    const text = await response.text()
    const streamed = envelopeFromSseFrames(parseSseFrames(text))

    expect(response.status).toBe(200)
    expect(publicActivityTimelineHasUnsafeMaterial(streamed)).toBe(false)
    expect(text).not.toMatch(
      /@|payment_preimage|raw_(payload|prompt)|sk-[a-z0-9]|token_secret/i,
    )
  })

  test('surfaces stale source lag instead of hiding it behind a fresh read timestamp', async () => {
    const body = await decode(
      await route('/api/public/activity-timeline?limit=200'),
    )
    const lag = body.sourceLag.find(item => item.sourceKind === 'pylon_presence')

    expect(lag).toMatchObject({
      latestSourceEventAt: '2026-06-18T18:00:02.000Z',
      lagSeconds: 598,
      maxStalenessSeconds: 300,
      observedAt: nowIso,
      status: 'stale',
    })
    expect(lag?.sourceRefs).toContain('route:/api/public/pylon-stats')
    expect(lag?.caveatRefs).toContain(
      'caveat.public.activity_timeline.source_lag_exceeds_contract',
    )
  })

  test('supports live-tail cursor and bounded range filters', async () => {
    const first = await decode(
      await route('/api/public/activity-timeline?limit=3'),
    )

    expect(first.events).toHaveLength(3)
    expect(first.nextCursor).toBe(first.events[2]?.cursor)

    const second = await decode(
      await route(
        `/api/public/activity-timeline?limit=200&since=${encodeURIComponent(
          first.nextCursor ?? '',
        )}`,
      ),
    )
    const bounded = await decode(
      await route(
        '/api/public/activity-timeline?from=2026-06-18T18:00:08.700Z&to=2026-06-18T18:00:18.000Z&kind=khala_inference_served,real_bitcoin_moved,forum_posted,artanis_tick&source=inference_receipt,settlement_receipt,forum,artanis&limit=20',
      ),
    )

    expect(second.events[0]?.cursor).not.toBe(first.events[0]?.cursor)
    expect(bounded.range).toMatchObject({
      filterKinds: [
        'khala_inference_served',
        'real_bitcoin_moved',
        'forum_posted',
        'artanis_tick',
      ],
      from: '2026-06-18T18:00:08.700Z',
      limit: 20,
      to: '2026-06-18T18:00:18.000Z',
    })
    expect(bounded.events.map(event => event.kind)).toEqual([
      'khala_inference_served',
      'real_bitcoin_moved',
      'forum_posted',
      'artanis_tick',
    ])
  })

  test('streams timeline events as SSE frames with source-lag metadata', async () => {
    const json = await decode(
      await route('/api/public/activity-timeline?limit=3'),
    )
    const response = await streamRoute(
      '/api/public/activity-timeline/stream?limit=3',
    )
    const text = await response.text()
    const frames = parseSseFrames(text)
    const streamed = envelopeFromSseFrames(frames)
    const retry = frames.find(frame => frame.retry !== undefined)
    const meta = frames.find(frame => frame.event === 'activity_timeline_meta')

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/event-stream')
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('x-accel-buffering')).toBe('no')
    expect(response.headers.get('x-openagents-authority')).toBe(
      'observation_only',
    )
    expect(response.headers.get('x-openagents-polling-fallback')).toContain(
      '/api/public/activity-timeline?',
    )
    expect(retry?.retry).toBe('15000')
    expect(meta).toBeDefined()
    expect(frameDataRecord(meta!).sourceLag).toEqual(json.sourceLag)
    expect(frameDataRecord(meta!).staleness).toEqual(json.staleness)
    expect(streamed.events).toEqual(json.events)
    expect(
      frames
        .filter(frame => frame.id !== undefined)
        .map(frame => [frame.id, frame.event]),
    ).toEqual(json.events.map(event => [event.cursor, event.kind]))
  })

  test('stream reconnect resumes from Last-Event-ID cursor', async () => {
    const first = await streamRoute(
      '/api/public/activity-timeline/stream?limit=2',
    )
    const firstFrames = parseSseFrames(await first.text())
    const lastEventId = firstFrames
      .filter(frame => frame.id !== undefined)
      .at(-1)?.id
    if (lastEventId === undefined) throw new Error('missing first stream cursor')

    const resumed = await streamRoute(
      '/api/public/activity-timeline/stream?limit=200',
      fullInput(),
      { headers: { 'Last-Event-ID': lastEventId } },
    )
    const expected = await decode(
      await route(
        `/api/public/activity-timeline?limit=200&since=${encodeURIComponent(
          lastEventId,
        )}`,
      ),
    )
    const resumedEnvelope = envelopeFromSseFrames(
      parseSseFrames(await resumed.text()),
    )

    expect(resumed.status).toBe(200)
    expect(resumedEnvelope.events.map(event => event.cursor)).toEqual(
      expected.events.map(event => event.cursor),
    )
    expect(
      resumedEnvelope.events.every(event => event.cursor > lastEventId),
    ).toBe(true)
  })

  test('paginates deterministically across equal timestamps', async () => {
    const first = await decode(
      await route('/api/public/activity-timeline?limit=2'),
    )
    const second = await decode(
      await route(
        `/api/public/activity-timeline?limit=2&since=${encodeURIComponent(
          first.nextCursor ?? '',
        )}`,
      ),
    )
    const replay = await decode(
      await route('/api/public/activity-timeline?limit=4'),
    )
    const tiedPresenceRefs = replay.events
      .filter(
        event =>
          event.ts === '2026-06-18T18:00:02.000Z' &&
          event.sourceKind === 'pylon_presence',
      )
      .map(event => event.eventRef)

    expect([...first.events, ...second.events].map(event => event.cursor)).toEqual(
      replay.events.map(event => event.cursor),
    )
    expect(tiedPresenceRefs).toEqual([...tiedPresenceRefs].sort())
  })

  test('emits real bitcoin movement only for receipt-backed real movement', async () => {
    const body = await decode(
      await route('/api/public/activity-timeline?source=settlement_receipt&limit=20'),
    )
    const realMovement = body.events.filter(
      event => event.kind === 'real_bitcoin_moved',
    )
    const simulationSettlement = body.events.find(
      event =>
        event.kind === 'settlement_recorded' &&
        event.refs.includes(simulationReceiptRef),
    )

    expect(realMovement).toHaveLength(1)
    expect(
      realMovement.some(event => event.refs.includes(simulationReceiptRef)),
    ).toBe(false)
    expect(realMovement[0]).toMatchObject({
      amountSats: 1000,
      realBitcoinMoved: true,
      refs: expect.arrayContaining([realReceiptRef]),
      sourceRefs: expect.arrayContaining([realReceiptRef]),
    })
    expect(simulationSettlement).toMatchObject({
      amountSats: 5,
      realBitcoinMoved: false,
      refs: expect.arrayContaining([simulationReceiptRef]),
    })
  })

  test('emits projection gaps instead of guessing when source coverage is missing', async () => {
    const response = await route('/api/public/activity-timeline?limit=20', {
      nowIso: () => nowIso,
    })
    const body = await decode(response)
    const gap = body.events.find(event =>
      event.blockerRefs.includes(
        'blocker.public.activity_timeline.pylon_store_missing',
      ),
    )

    expect(response.status).toBe(200)
    expect(gap).toMatchObject({
      blockerRefs: expect.arrayContaining([
        'blocker.public.activity_timeline.pylon_store_missing',
      ]),
      sourceKind: 'projection_gap',
    })
  })

  test('rejects invalid filters and methods', async () => {
    const invalidKind = await route(
      '/api/public/activity-timeline?kind=raw_private_trace',
    )
    const invalidSource = await route(
      '/api/public/activity-timeline?source=private_wallet',
    )
    const method = await Effect.runPromise(
      handlePublicActivityTimelineApi(
        new Request('https://openagents.com/api/public/activity-timeline', {
          method: 'POST',
        }),
        fullInput(),
      ),
    )

    expect(invalidKind.status).toBe(400)
    expect(invalidSource.status).toBe(400)
    expect(method.status).toBe(405)
  })
})

// ---------------------------------------------------------------------------
// KS-6.7b (#8421): scope.public.activity-timeline stored-snapshot projection
// read path — projection-hit serves a stored_snapshot-labeled envelope from
// ONE cached Postgres row; any miss (no binding, unprojected, broken client)
// fails open to the exact same live merge exercised by the tests above.
// ---------------------------------------------------------------------------

type ChangelogRow = {
  scope: string
  entityType: string
  entityId: string
  version: number
  postImageJson: string | null
}

const makeFakePg = (): { sql: SyncSql; rows: Array<ChangelogRow> } => {
  const rows: Array<ChangelogRow> = []
  let lastVersion = 0

  const run = async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): Promise<any> => {
    const text = strings.join('?')
    if (text.includes('INSERT INTO khala_sync_scopes')) {
      lastVersion += 1
      return [{ last_version: lastVersion }]
    }
    if (text.includes('INSERT INTO khala_sync_changelog')) {
      rows.push({
        entityId: String(values[3]),
        entityType: String(values[2]),
        postImageJson: values[5] === null ? null : String(values[5]),
        scope: String(values[0]),
        version: Number(values[1]),
      })
      return [{ committed_at: nowIso }]
    }
    if (text.includes('SELECT post_image_json')) {
      const entityType = String(values[1])
      const entityId = String(values[2])
      const matches = rows
        .filter(r => r.entityType === entityType && r.entityId === entityId)
        .sort((a, b) => b.version - a.version)
      const top = matches[0]
      return top === undefined ? [] : [{ post_image_json: top.postImageJson }]
    }
    throw new Error(`fake pg: unscripted statement: ${text.slice(0, 120)}`)
  }

  const sql = run as unknown as SyncSql & {
    begin: <A>(fn: (tx: SyncTransactionSql) => Promise<A>) => Promise<A>
  }
  ;(sql as { begin: unknown }).begin = async <A>(
    fn: (tx: SyncTransactionSql) => Promise<A>,
  ): Promise<A> => fn(run as unknown as SyncTransactionSql)

  return { rows, sql: sql as SyncSql }
}

const clientFor = (sql: SyncSql): KhalaSyncPushSqlClient => ({
  end: async () => undefined,
  sql,
})

const binding = { connectionString: 'postgres://hyperdrive-fake' }

describe('public activity timeline route — projection read path', () => {
  beforeEach(() => {
    invalidateActivityTimelineSnapshotCacheForTests()
  })

  test('serves the stored snapshot with a stored_snapshot staleness label on a projection hit', async () => {
    const { sql } = makeFakePg()
    const seeded = await projectActivityTimelineSnapshotBestEffort(sql, {
      events: [
        {
          blockerRefs: [],
          caveatRefs: [],
          cursor: `${nowIso}:forum:event.public.forum_topic.projected`,
          eventRef: 'event.public.forum_topic.projected',
          kind: 'forum_topic_created',
          refs: ['topic:projected'],
          sourceKind: 'forum',
          sourceRefs: ['topic:projected', 'route:/api/forum'],
          text: 'Public Forum topic created.',
          ts: nowIso,
        },
      ],
      generatedAt: nowIso,
      sourceLag: [
        {
          blockerRefs: [],
          caveatRefs: [],
          lagSeconds: 0,
          latestSourceEventAt: nowIso,
          maxStalenessSeconds: 600,
          observedAt: nowIso,
          sourceKind: 'forum',
          sourceRefs: ['route:/api/forum'],
          status: 'current',
        },
      ],
    })
    expect(seeded.ok).toBe(true)

    const response = await route('/api/public/activity-timeline', {
      KHALA_SYNC_DB: binding,
      projectionReadDeps: {
        makeSqlClient: async () => clientFor(sql),
        nowIso: () => nowIso,
      },
    })
    const body = await decode(response)

    expect(response.status).toBe(200)
    expect(body.staleness.composition).toBe('stored_snapshot')
    expect(body.staleness.maxStalenessSeconds).toBe(90)
    expect(
      body.events.some(
        event => event.eventRef === 'event.public.forum_topic.projected',
      ),
    ).toBe(true)
    // A projection-served envelope carries ONLY the stored events — none of
    // the live-merge projection-gap noise from the missing D1 stores.
    expect(
      body.events.some(event => event.sourceKind === 'projection_gap'),
    ).toBe(false)
  })

  test('fails open to the live merge (with all its own store coverage) when nothing has been projected yet', async () => {
    const { sql } = makeFakePg()

    const response = await route('/api/public/activity-timeline?limit=20', {
      ...fullInput(),
      KHALA_SYNC_DB: binding,
      projectionReadDeps: { makeSqlClient: async () => clientFor(sql) },
    })
    const body = await decode(response)

    expect(response.status).toBe(200)
    expect(body.staleness.composition).toBe('live_at_read')
  })

  test('fails open to the live merge when the Postgres client itself is broken', async () => {
    const response = await route('/api/public/activity-timeline?limit=20', {
      ...fullInput(),
      KHALA_SYNC_DB: binding,
      projectionReadDeps: {
        makeSqlClient: async () => {
          throw new Error('connection refused')
        },
      },
    })
    const body = await decode(response)

    expect(response.status).toBe(200)
    expect(body.staleness.composition).toBe('live_at_read')
  })
})
