import { describe, expect, it } from 'vitest'

import {
  DEFAULT_TASSADAR_RUN_REF,
  PublicTassadarRunSummarySchemaVersion,
  buildPublicTassadarRunSummaryEnvelopeForRequest,
} from './public-tassadar-run-summary-routes'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

// Minimal fake store; tests never hit D1. readRun defaults to "not found".
const fakeStore = (overrides: Record<string, unknown> = {}) =>
  ({
    readRun: async () => undefined,
    listWindowsForRun: async () => [],
    listWindowLeasesForRun: async () => [],
    listVerificationChallengesForRun: async () => [],
    ...overrides,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  }) as any

const now = () => '2026-06-16T12:00:00.000Z'
const req = (url = 'https://openagents.com/api/public/tassadar-run-summary') =>
  new Request(url)
const runRef = 'run.tassadar.executor.20260615'
const windowRef = 'training.window.tassadar.executor.20260615.w1'
const pylonRef = 'pylon.public.contributor'
const settlementReceiptRef =
  'receipt.nexus.tassadar_run_settlement.public_summary_test'

const runRecord: TrainingRunRecord = {
  createdAt: '2026-06-16T10:00:00.000Z',
  id: 'run-public-summary-test',
  manifest: {
    artifactDigestRefs: [],
    blockerRefs: [],
    spendCapSats: 100_000,
  },
  maxAllowedStale: 5,
  promiseRef: 'training.decentralized_training_launch.v1',
  publicProjectionJson: '{}',
  receiptRefs: [settlementReceiptRef],
  sealInFlightAt: null,
  sealPublicationCadenceWindows: 1,
  sourceRefs: ['issue.github.openagents.5006'],
  state: 'active',
  trainingRunRef: runRef,
  updatedAt: '2026-06-16T10:00:00.000Z',
}

const windowRecord: TrainingWindowRecord = {
  activatedAt: '2026-06-16T10:00:00.000Z',
  datasetRefs: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'window-public-summary-test',
  plannedAt: '2026-06-16T10:00:00.000Z',
  priority: 100,
  publicProjectionJson: '{}',
  receiptRefs: [],
  reconciledAt: null,
  sealMetadata: null,
  sealedAt: null,
  sourceRefs: ['workload.tassadar_executor.alm_numeric_trace.v1'],
  state: 'active',
  trainingRunRef: runRef,
  updatedAt: '2026-06-16T10:00:00.000Z',
  windowRef,
}

const leaseRecord: TrainingWindowLeaseRecord = {
  claimedAt: '2026-06-16T10:01:00.000Z',
  id: 'lease-public-summary-test',
  leaseExpiresAt: '2026-06-16T12:01:00.000Z',
  leaseRef: 'training.lease.public_summary_test',
  publicProjectionJson: '{}',
  pylonRef,
  receiptRefs: [],
  state: 'active',
  trainingRunRef: runRef,
  windowRef,
}

const verifiedChallenge: TrainingVerificationChallengeRecord = {
  challengeRef: 'training.verification.challenge.public_summary_test',
  commitmentRefs: [],
  contributionRef: 'contribution.tassadar.public_summary_test',
  createdAt: '2026-06-16T10:02:00.000Z',
  failureCodes: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'challenge-public-summary-test',
  leaseExpiresAt: null,
  leaseRef: leaseRecord.leaseRef,
  leasedToRef: 'pylon.public.validator',
  maxAttempts: 3,
  payloadJson: JSON.stringify({
    pylonDeviceRef: pylonRef,
    validatorDeviceRef: 'pylon.public.validator',
  }),
  publicProjectionJson: '{}',
  rejectedAt: null,
  samplingPolicy: 'per_contribution',
  state: 'Verified',
  timedOutAt: null,
  trainingRunRef: runRef,
  updatedAt: '2026-06-16T10:03:00.000Z',
  verdictRefs: ['verdict.training.exact_trace_replay.public_summary_test'],
  verificationClass: 'exact_trace_replay',
  verifiedAt: '2026-06-16T10:03:00.000Z',
  windowRef,
}

const rejectedChallenge: TrainingVerificationChallengeRecord = {
  ...verifiedChallenge,
  challengeRef: 'training.verification.challenge.public_summary_rejected',
  failureCodes: ['DigestMismatch'],
  id: 'challenge-public-summary-rejected',
  payloadJson: JSON.stringify({
    pylonDeviceRef: 'pylon.public.rejected_worker',
    validatorDeviceRef: 'pylon.public.validator',
  }),
  rejectedAt: '2026-06-16T10:04:00.000Z',
  state: 'Rejected',
  updatedAt: '2026-06-16T10:04:00.000Z',
  verdictRefs: ['verdict.training.exact_trace_replay.public_summary_rejected'],
  verifiedAt: null,
}

const fakePayoutLedgerStore = () =>
  ({
    readPaymentAuthorityReceiptByRef: async (receiptRef: string) =>
      receiptRef === settlementReceiptRef
        ? {
            eventRef: 'reconciliation.tassadar.public_summary_test',
            payoutAttemptRef: 'payout_attempt.tassadar.public_summary_test',
            payoutIntentRef: 'payout_intent.tassadar.public_summary_test',
            publicProjectionJson: JSON.stringify({
              amountSats: 21,
              contributorRef: pylonRef,
              moneyMovement: 'simulation',
              state: 'settled',
              trainingRunRef: runRef,
              verificationChallengeRef: verifiedChallenge.challengeRef,
            }),
            receiptKind: 'settlement_recorded',
            receiptRef: settlementReceiptRef,
          }
        : undefined,
    readReconciliationEventByRef: async (eventRef: string) =>
      eventRef === 'reconciliation.tassadar.public_summary_test'
        ? { status: 'matched' }
        : undefined,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  }) as any

describe('buildPublicTassadarRunSummaryEnvelopeForRequest (public read, #5114)', () => {
  it('returns an honest idle envelope when the run is not found (receipt-first)', async () => {
    const body = await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(),
      {} as never,
      {
        makeStore: () => fakeStore(),
        now,
      },
    )
    expect(body.schemaVersion).toBe(PublicTassadarRunSummarySchemaVersion)
    expect(body.runRef).toBe(DEFAULT_TASSADAR_RUN_REF)
    expect(body.runState).toBe('planned')
    expect((body.emptyState as { idle: boolean }).idle).toBe(true)
    expect(body.metrics).toEqual({})
    expect(body.generatedAt).toBe('2026-06-16T12:00:00.000Z')
    expect((body.staleness as { composition?: unknown }).composition).toBe(
      'live_at_read',
    )
    expect(
      (body.staleness as { maxStalenessSeconds?: unknown }).maxStalenessSeconds,
    ).toBe(0)
    const bulletin = body.bulletin as {
      headline: string
      metrics: { totalPylonCount: number; activePylonCount: number }
      onBoardLines: ReadonlyArray<string>
      summary: string
    }
    expect(bulletin.headline).toBe('Tassadar is planned: 0 pylons, 0 active.')
    expect(bulletin.metrics.totalPylonCount).toBe(0)
    expect(bulletin.metrics.activePylonCount).toBe(0)
    expect(bulletin.onBoardLines).toContain('Status: planned')
    expect(bulletin.summary).toContain(
      'No active training window is visible in the public projection right now.',
    )
  })

  it('honors the ?run= query param when choosing which run to read', async () => {
    let asked = ''
    await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(
        'https://openagents.com/api/public/tassadar-run-summary?run=run.custom.test',
      ),
      {} as never,
      {
        makeStore: () =>
          fakeStore({
            readRun: async (ref: string) => {
              asked = ref
              return undefined
            },
          }),
        now,
      },
    )
    expect(asked).toBe('run.custom.test')
  })

  it('requires no admin auth to build the public read envelope', async () => {
    const body = await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(),
      {} as never,
      {
        makeStore: () => fakeStore(),
        now,
      },
    )
    expect(body.runRef).toBe(DEFAULT_TASSADAR_RUN_REF)
  })

  it('resolves provider-confirmed settlement receipts like the canonical public training-run endpoint', async () => {
    const body = await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(),
      {} as never,
      {
        makePayoutLedgerStore: fakePayoutLedgerStore,
        makeStore: () =>
          fakeStore({
            readRun: async () => runRecord,
            listWindowsForRun: async () => [windowRecord],
            listWindowLeasesForRun: async () => [leaseRecord],
            listVerificationChallengesForRun: async () => [
              verifiedChallenge,
              rejectedChallenge,
            ],
          }),
        now,
      },
    )
    const metrics = body.metrics as {
      providerConfirmedSettledPayoutSats: { value: number }
      qualifiedContributorCount: {
        sourceRefs: ReadonlyArray<string>
        value: number
      }
    }

    // The only settled receipt here is a SIMULATION (movementMode:simulation,
    // realBitcoinMoved:false). The real-money settled total must exclude it
    // (Orrery's sim-vs-real conflation fix) — so it is 0, even though the
    // simulation row still appears below, flagged. A real_bitcoin receipt would
    // count; a settled-state sim must not inflate the real total.
    expect(metrics.providerConfirmedSettledPayoutSats.value).toBe(0)
    expect(metrics.qualifiedContributorCount.value).toBe(1)
    expect(metrics.qualifiedContributorCount.sourceRefs).toContain(pylonRef)
    expect(metrics.qualifiedContributorCount.sourceRefs).toContain(
      settlementReceiptRef,
    )

    const settlementRows = body.settlementRows as ReadonlyArray<{
      amountSats: number
      apiUrl: string
      contributorRef: string
      movementMode: string
      realBitcoinMoved: boolean
      receiptKind: string
      receiptPageUrl: string
      receiptRef: string
      sourceRefs: ReadonlyArray<string>
      state: string
      trainingRunRef: string
      verificationChallengeRef: string
    }>
    expect(settlementRows).toEqual([
      expect.objectContaining({
        amountSats: 21,
        apiUrl: `https://openagents.com/api/public/nexus-pylon/receipts/${settlementReceiptRef}`,
        contributorRef: pylonRef,
        movementMode: 'simulation',
        realBitcoinMoved: false,
        receiptKind: 'settlement_recorded',
        receiptPageUrl: `https://openagents.com/nexus-pylon/receipts/${settlementReceiptRef}`,
        receiptRef: settlementReceiptRef,
        state: 'settled',
        trainingRunRef: runRef,
        verificationChallengeRef: verifiedChallenge.challengeRef,
      }),
    ])
    expect(settlementRows[0]?.sourceRefs).toContain(pylonRef)
    expect(settlementRows[0]?.sourceRefs).toContain(
      verifiedChallenge.challengeRef,
    )

    const realGradient = body.realGradient as {
      rejectedReplayPairs: ReadonlyArray<{
        challengeRef: string
        failureCodes: ReadonlyArray<string>
        validatorRef: string | null
        verdictRefs: ReadonlyArray<string>
        workerRef: string
      }>
      verifiedReplayPairs: ReadonlyArray<{ challengeRef: string }>
    }
    expect(realGradient.verifiedReplayPairs).toEqual([
      expect.objectContaining({
        challengeRef: verifiedChallenge.challengeRef,
      }),
    ])
    expect(realGradient.rejectedReplayPairs).toEqual([
      expect.objectContaining({
        challengeRef: rejectedChallenge.challengeRef,
        failureCodes: ['DigestMismatch'],
        validatorRef: 'pylon.public.validator',
        verdictRefs: [
          'verdict.training.exact_trace_replay.public_summary_rejected',
        ],
        workerRef: 'pylon.public.rejected_worker',
      }),
    ])
    const bulletin = body.bulletin as {
      headline: string
      latestActivity: ReadonlyArray<{ label: string; text: string }>
      metrics: {
        acceptedTraceCount: number
        activePylonCount: number
        activeWindowCount: number
        settledSats: number
        totalPylonCount: number
      }
      onBoardLines: ReadonlyArray<string>
      summary: string
    }
    expect(bulletin.headline).toBe('Tassadar is active: 1 pylon, 1 active.')
    expect(bulletin.metrics).toEqual(
      expect.objectContaining({
        acceptedTraceCount: 1,
        activePylonCount: 1,
        activeWindowCount: 1,
        settledSats: 0,
        totalPylonCount: 1,
      }),
    )
    expect(bulletin.onBoardLines).toEqual([
      'Status: active',
      '1 pylon, 1 active',
      'settlement pending',
    ])
    expect(bulletin.summary).toContain('1 training window active right now.')
    expect(bulletin.latestActivity[0]?.label).toBe('latest update')
  })

  // #5403 gap 1 (honesty): the real settled total must be the sum of ONLY the
  // real_bitcoin receipts, with the settled-state simulation row excluded — so
  // 1000 + 5 = 1005, NOT 1010. The simulation row still appears in
  // settlementRows, flagged movementMode:simulation / realBitcoinMoved:false,
  // so a skeptic can see it was not counted. This locks the per-run truth that
  // /api/public/tassadar-run-summary reports (1005 / 2 real receipts) and that
  // the per-run settlements feed mirrors.
  it('real settled total excludes the simulation row (1005 real, not 1010, with the sim row still flagged)', async () => {
    const canaryReceiptRef =
      'receipt.nexus.tassadar_run_settlement.mixed_test.canary1k'
    const selfServeReceiptRef =
      'receipt.nexus.tassadar_run_settlement.mixed_test.selfserve5'
    const simReceiptRef =
      'receipt.nexus.tassadar_run_settlement.mixed_test.sim5'
    const mixedRunRecord: TrainingRunRecord = {
      ...runRecord,
      receiptRefs: [canaryReceiptRef, selfServeReceiptRef, simReceiptRef],
    }
    const receiptRecords: Record<
      string,
      {
        amountSats: number
        contributorRef: string
        movementMode: 'real_bitcoin' | 'simulation'
        realBitcoinMoved: boolean
      }
    > = {
      [canaryReceiptRef]: {
        amountSats: 1000,
        contributorRef: 'pylon.public.worker_canary',
        movementMode: 'real_bitcoin',
        realBitcoinMoved: true,
      },
      [selfServeReceiptRef]: {
        amountSats: 5,
        contributorRef: 'pylon.public.worker_selfserve',
        movementMode: 'real_bitcoin',
        realBitcoinMoved: true,
      },
      [simReceiptRef]: {
        amountSats: 5,
        contributorRef: 'pylon.public.worker_sim',
        movementMode: 'simulation',
        realBitcoinMoved: false,
      },
    }
    const mixedPayoutLedgerStore = () =>
      ({
        readPaymentAuthorityReceiptByRef: async (receiptRef: string) => {
          const spec = receiptRecords[receiptRef]
          if (spec === undefined) return undefined
          return {
            eventRef: `reconciliation.${receiptRef}`,
            payoutAttemptRef: `payout_attempt.${receiptRef}`,
            payoutIntentRef: `payout_intent.${receiptRef}`,
            publicProjectionJson: JSON.stringify({
              amountSats: spec.amountSats,
              contributorRef: spec.contributorRef,
              moneyMovement: spec.movementMode,
              movementMode: spec.movementMode,
              realBitcoinMoved: spec.realBitcoinMoved,
              state: 'settled',
              trainingRunRef: runRef,
            }),
            receiptKind: 'settlement_recorded',
            receiptRef,
          }
        },
        readReconciliationEventByRef: async () => ({ status: 'matched' }),
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any

    const body = await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(),
      {} as never,
      {
        makePayoutLedgerStore: mixedPayoutLedgerStore,
        makeStore: () =>
          fakeStore({
            readRun: async () => mixedRunRecord,
            listWindowsForRun: async () => [windowRecord],
            listWindowLeasesForRun: async () => [leaseRecord],
            listVerificationChallengesForRun: async () => [verifiedChallenge],
          }),
        now,
      },
    )

    const metrics = body.metrics as {
      providerConfirmedSettledPayoutSats: { value: number }
      qualifiedContributorCount: { value: number }
    }
    // 1000 + 5 real = 1005; the 5-sat simulation row is excluded.
    expect(metrics.providerConfirmedSettledPayoutSats.value).toBe(1005)

    const settlement = body.settlement as {
      reconciledState: string
      settledPayoutSats: number
      settledReceiptCount: number
    }
    expect(settlement.settledPayoutSats).toBe(1005)
    expect(settlement.settledReceiptCount).toBe(2)
    expect(settlement.reconciledState).toBe('settling')

    const settlementRows = body.settlementRows as ReadonlyArray<{
      amountSats: number
      movementMode: string
      realBitcoinMoved: boolean
      receiptRef: string
    }>
    // All three rows are present (the sim row is flagged, not hidden).
    expect(settlementRows).toHaveLength(3)
    const realSatsFromRows = settlementRows
      .filter(row => row.realBitcoinMoved)
      .reduce((total, row) => total + row.amountSats, 0)
    expect(realSatsFromRows).toBe(1005)
    const simRow = settlementRows.find(row => row.receiptRef === simReceiptRef)
    expect(simRow?.movementMode).toBe('simulation')
    expect(simRow?.realBitcoinMoved).toBe(false)
    // The sim row's 5 sats must NOT push the real total to 1010.
    const allRowsSats = settlementRows.reduce(
      (total, row) => total + row.amountSats,
      0,
    )
    expect(allRowsSats).toBe(1010)
    expect(metrics.providerConfirmedSettledPayoutSats.value).not.toBe(
      allRowsSats,
    )
  })
})
