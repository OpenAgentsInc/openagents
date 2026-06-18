import { describe, expect, it } from 'vitest'

import {
  FIRST_REAL_SETTLEMENT_CHALLENGE_REF,
  FIRST_REAL_SETTLEMENT_RECEIPT_REF,
  LAUNCH_RECOGNITION_BUNDLE_SLUG,
  ProofReplayBundleSchemaVersion,
  buildFirstRealSettlementReplayBundle,
  buildLaunchRecognitionReplayBundle,
  buildPublicProofReplayBundleForRequest,
  type ReplayEvent,
} from './public-proof-replay-routes'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

const runRef = 'run.tassadar.executor.20260615'
const windowRef = 'training.window.tassadar.executor.20260615.w1'
const contributorRef = 'pylon.448ba824b5fc879f3a59'
const validatorRef = 'pylon.public.validator'
const simulationReceiptRef = 'receipt.nexus.tassadar_run_settlement.simulation_5'

const req = (
  url = `https://openagents.com/api/public/tassadar-replays/first-real-settlement?receiptRef=${FIRST_REAL_SETTLEMENT_RECEIPT_REF}`,
) => new Request(url)

const fakeStore = (overrides: Record<string, unknown> = {}) =>
  ({
    readRun: async () => undefined,
    listWindowsForRun: async () => [],
    listWindowLeasesForRun: async () => [],
    listVerificationChallengesForRun: async () => [],
    ...overrides,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  }) as any

const runRecord: TrainingRunRecord = {
  createdAt: '2026-06-16T10:00:00.000Z',
  id: 'run-proof-replay-test',
  manifest: {
    artifactDigestRefs: [],
    blockerRefs: [],
    spendCapSats: 100_000,
  },
  maxAllowedStale: 5,
  promiseRef: 'training.decentralized_training_launch.v1',
  publicProjectionJson: '{}',
  receiptRefs: [simulationReceiptRef, FIRST_REAL_SETTLEMENT_RECEIPT_REF],
  sealInFlightAt: null,
  sealPublicationCadenceWindows: 1,
  sourceRefs: ['issue.github.openagents.5298'],
  state: 'active',
  trainingRunRef: runRef,
  updatedAt: '2026-06-16T10:00:00.000Z',
}

const windowRecord: TrainingWindowRecord = {
  activatedAt: '2026-06-16T10:00:00.000Z',
  datasetRefs: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'window-proof-replay-test',
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
  id: 'lease-proof-replay-test',
  leaseExpiresAt: '2026-06-16T12:01:00.000Z',
  leaseRef: 'training.lease.proof_replay_test',
  publicProjectionJson: '{}',
  pylonRef: contributorRef,
  receiptRefs: [],
  state: 'active',
  trainingRunRef: runRef,
  windowRef,
}

const verifiedChallenge: TrainingVerificationChallengeRecord = {
  challengeRef: FIRST_REAL_SETTLEMENT_CHALLENGE_REF,
  commitmentRefs: [],
  contributionRef: 'contribution.tassadar.proof_replay_test',
  createdAt: '2026-06-16T10:02:00.000Z',
  failureCodes: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'challenge-proof-replay-test',
  leaseExpiresAt: null,
  leaseRef: leaseRecord.leaseRef,
  leasedToRef: validatorRef,
  maxAttempts: 3,
  payloadJson: JSON.stringify({
    pylonDeviceRef: contributorRef,
    validatorDeviceRef: validatorRef,
  }),
  publicProjectionJson: '{}',
  rejectedAt: null,
  samplingPolicy: 'per_contribution',
  state: 'Verified',
  timedOutAt: null,
  trainingRunRef: runRef,
  updatedAt: '2026-06-16T10:03:00.000Z',
  verdictRefs: ['verdict.training.exact_trace_replay.proof_replay_test'],
  verificationClass: 'exact_trace_replay',
  verifiedAt: '2026-06-16T10:03:00.000Z',
  windowRef,
}

const fakeTrainingStore = () =>
  fakeStore({
    readRun: async () => runRecord,
    listWindowsForRun: async () => [windowRecord],
    listWindowLeasesForRun: async () => [leaseRecord],
    listVerificationChallengesForRun: async () => [verifiedChallenge],
  })

const fakePayoutLedgerStore = () =>
  ({
    readPaymentAuthorityReceiptByRef: async (receiptRef: string) => {
      if (receiptRef === simulationReceiptRef) {
        return {
          eventRef: 'reconciliation.tassadar.simulation_5',
          payoutAttemptRef: 'payout_attempt.tassadar.simulation_5',
          payoutIntentRef: 'payout_intent.tassadar.simulation_5',
          publicProjectionJson: JSON.stringify({
            amountSats: 5,
            contributorRef,
            moneyMovement: 'simulation',
            movementMode: 'simulation',
            realBitcoinMoved: false,
            state: 'settled',
            trainingRunRef: runRef,
            verificationChallengeRef: FIRST_REAL_SETTLEMENT_CHALLENGE_REF,
          }),
          receiptKind: 'settlement_recorded',
          receiptRef,
        }
      }

      if (receiptRef === FIRST_REAL_SETTLEMENT_RECEIPT_REF) {
        return {
          eventRef:
            'reconciliation.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
          payoutAttemptRef:
            'payout_attempt.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
          payoutIntentRef:
            'payout_intent.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618',
          publicProjectionJson: JSON.stringify({
            adapter: 'spark_treasury',
            amountSats: 1_000,
            contributorRef,
            moneyMovement: 'real_bitcoin',
            movementMode: 'real_bitcoin',
            realBitcoinMoved: true,
            state: 'settled',
            trainingRunRef: runRef,
            verificationChallengeRef: FIRST_REAL_SETTLEMENT_CHALLENGE_REF,
          }),
          receiptKind: 'settlement_recorded',
          receiptRef,
        }
      }

      return undefined
    },
    readReconciliationEventByRef: async () => ({ status: 'matched' }),
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  }) as any

const eventByKind = (
  events: ReadonlyArray<ReplayEvent>,
  kind: ReplayEvent['kind'],
): ReplayEvent => {
  const event = events.find(item => item.kind === kind)
  expect(event).toBeDefined()

  if (event === undefined) {
    throw new Error(`missing event ${kind}`)
  }

  return event
}

describe('public proof replay bundle resolver (#5298)', () => {
  it('returns a deterministic public-safe bundle for the first real settlement receipt', async () => {
    const first = await buildPublicProofReplayBundleForRequest(req(), {} as never, {
      makePayoutLedgerStore: fakePayoutLedgerStore,
      makeStore: fakeTrainingStore,
      now: () => '2026-06-18T02:00:00.000Z',
    })
    const second = await buildPublicProofReplayBundleForRequest(req(), {} as never, {
      makePayoutLedgerStore: fakePayoutLedgerStore,
      makeStore: fakeTrainingStore,
      now: () => '2026-06-18T02:05:00.000Z',
    })

    expect(first.schemaVersion).toBe(ProofReplayBundleSchemaVersion)
    expect(first.privacyLevel).toBe('public_safe')
    expect(first.claimScope).toBe('evidence_presentation_only')
    expect(first.sourceAuthority).toBe('worker_d1_public')
    expect(first.bundleRef).toBe(second.bundleRef)
    expect(first.generatedAt).toBe('2026-06-18T02:00:00.000Z')
    expect(second.generatedAt).toBe('2026-06-18T02:05:00.000Z')
    expect(first.socialDisplayTime).toBe('8:38pm, June 17')
    expect(first.sourceRefs.map(source => source.ref)).toContain(
      FIRST_REAL_SETTLEMENT_RECEIPT_REF,
    )

    const settlementRecorded = eventByKind(first.events, 'settlement_recorded')
    const confirmedZap = eventByKind(first.events, 'payment_zap_confirmed')
    const simulatedZap = eventByKind(first.events, 'payment_zap_simulated')

    expect(settlementRecorded.sequenceIndex).toBeLessThan(
      confirmedZap.sequenceIndex,
    )
    expect(confirmedZap.amountSats).toBe(1_000)
    expect(confirmedZap.rail).toBe('spark_treasury')
    expect(confirmedZap.stateAfter).toBe('realBitcoinMoved:true')
    expect(confirmedZap.sourceRefs).toContain(FIRST_REAL_SETTLEMENT_RECEIPT_REF)
    expect(simulatedZap.amountSats).toBe(5)
    expect(simulatedZap.caveat).toContain('not confirmed Bitcoin movement')
    expect(simulatedZap.sourceRefs).toContain(simulationReceiptRef)

    const failedClosedEvents = first.events.filter(
      event => event.kind === 'settlement_blocked_closed',
    )
    expect(failedClosedEvents).toHaveLength(2)
    expect(failedClosedEvents.every(event => event.amountSats === undefined)).toBe(
      true,
    )
    expect(failedClosedEvents.every(event => event.observedAt === undefined)).toBe(
      true,
    )
    expect(first.gaps[0]?.reason).toContain('ordered by replay sequence')
  })

  it('keeps source refs on every event, flow, and caption', async () => {
    const bundle = await buildPublicProofReplayBundleForRequest(
      req(),
      {} as never,
      {
        makePayoutLedgerStore: fakePayoutLedgerStore,
        makeStore: fakeTrainingStore,
        now: () => '2026-06-18T02:00:00.000Z',
      },
    )

    expect(bundle.events.every(event => event.sourceRefs.length > 0)).toBe(true)
    expect(bundle.flows.every(flow => flow.sourceRefs.length > 0)).toBe(true)
    expect(bundle.captions.every(caption => caption.sourceRefs.length > 0)).toBe(
      true,
    )
  })

  it('rejects private payment/operator material before returning a bundle', () => {
    expect(() =>
      buildFirstRealSettlementReplayBundle({
        appUrl: 'https://openagents.com',
        generatedAt: '2026-06-18T02:00:00.000Z',
        requestedRefs: [FIRST_REAL_SETTLEMENT_RECEIPT_REF],
        summary: {
          runRef,
          settlementRows: [
            {
              amountSats: 1_000,
              apiUrl: `https://openagents.com/api/public/nexus-pylon/receipts/${FIRST_REAL_SETTLEMENT_RECEIPT_REF}`,
              contributorRef,
              movementMode: 'real_bitcoin',
              realBitcoinMoved: true,
              receiptKind: 'settlement_recorded',
              receiptPageUrl: `https://openagents.com/nexus-pylon/receipts/${FIRST_REAL_SETTLEMENT_RECEIPT_REF}`,
              receiptRef: FIRST_REAL_SETTLEMENT_RECEIPT_REF,
              sourceRefs: [
                FIRST_REAL_SETTLEMENT_RECEIPT_REF,
                'spark1rawpublicaddresslookingmaterial000000000000',
                'mnemonic should never enter public replay fixtures',
              ],
              state: 'settled',
              trainingRunRef: runRef,
              verificationChallengeRef: FIRST_REAL_SETTLEMENT_CHALLENGE_REF,
            },
          ],
        },
      }),
    ).toThrow(/private material/)
  })

  it('builds a launch-recognition replay with separate recipient lanes and overpayment accounting', async () => {
    const bundle = await buildPublicProofReplayBundleForRequest(
      req(`https://openagents.com/api/public/proof-replays?ref=${LAUNCH_RECOGNITION_BUNDLE_SLUG}`),
      {} as never,
      {
        now: () => '2026-06-18T03:00:00.000Z',
      },
    )

    expect(bundle.schemaVersion).toBe(ProofReplayBundleSchemaVersion)
    expect(bundle.title).toContain('Launch Recognition Payments')
    expect(bundle.bundleRef).toMatch(/^proof_replay_bundle\.launch_recognition\./)
    expect(bundle.actors.map(actor => actor.displayName)).toEqual(
      expect.arrayContaining(['Trigger', 'Whitefang', 'Orrery']),
    )

    const intendedRewards = bundle.events.filter(
      event => event.kind === 'recognition_reward_recorded',
    )
    expect(intendedRewards).toHaveLength(3)
    expect(intendedRewards.every(event => event.amountSats === 50_000)).toBe(true)
    expect(intendedRewards.every(event => event.caveat?.includes('Intended'))).toBe(
      true,
    )

    const confirmedZaps = bundle.events.filter(
      event => event.kind === 'payment_zap_confirmed',
    )
    expect(confirmedZaps).toHaveLength(3)
    expect(
      confirmedZaps.every(event =>
        event.sourceRefs.some(ref => ref.startsWith('recipient_confirmation.')) ||
        event.sourceRefs.some(ref => ref.includes('JUNE17_ROADMAP.md')),
      ),
    ).toBe(true)

    const blocked = bundle.events.filter(
      event => event.kind === 'settlement_blocked_closed',
    )
    expect(blocked).toHaveLength(3)
    expect(blocked.every(event => event.amountSats === undefined)).toBe(true)
    expect(blocked.map(event => event.stateAfter)).toEqual(
      expect.arrayContaining([
        'historical_pending_snapshot',
        'failed_before_dispatch',
        'expired_or_pending_snapshot',
      ]),
    )

    const overpayment = eventByKind(bundle.events, 'overpayment_detected')
    expect(overpayment.amountSats).toBe(109_239)
    expect(overpayment.caveat).toContain('159,239')
    expect(overpayment.sourceRefs).toEqual(
      expect.arrayContaining([
        'recipient_confirmation.launch_recognition.orrery.visible_159239_sats',
        'recognition_ledger.launch_recognition.orrery.hazard_pay_owner_decision',
      ]),
    )
    expect(bundle.gaps.map(gap => gap.gapRef)).toEqual(
      expect.arrayContaining([
        'proof_replay_gap.launch_recognition.whitefang_snapshot_change',
        'proof_replay_gap.launch_recognition.orrery_accounting_snapshot_change',
      ]),
    )
  })

  it('does not echo unsafe requested refs into the launch-recognition replay', () => {
    const bundle = buildLaunchRecognitionReplayBundle({
      appUrl: 'https://openagents.com',
      generatedAt: '2026-06-18T03:00:00.000Z',
      requestedRefs: [
        LAUNCH_RECOGNITION_BUNDLE_SLUG,
        'bolt11 private invoice should be ignored',
      ],
    })

    expect(JSON.stringify(bundle)).not.toMatch(
      /bolt11|preimage|payment_hash|mnemonic|service[_ -]?token|spark[_ -]?api[_ -]?key/i,
    )
    expect(bundle.sourceRefs.map(source => source.ref)).not.toContain(
      'bolt11 private invoice should be ignored',
    )
  })
})
