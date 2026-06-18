import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import type { TassadarAutoSettlementDeps } from './tassadar-auto-settlement'
import {
  TassadarAdversarialDivergenceRewardSats,
  TassadarAdversarialVerificationUnsafe,
  projectTassadarAdversarialVerificationReleaseGate,
  settleConfirmedTassadarDivergenceDefect,
  verifyTassadarAdversarialDivergenceClaim,
  type TassadarAdversarialDivergenceClaim,
  type TassadarAdversarialDivergenceReproduction,
} from './tassadar-adversarial-verification-market'
import type { TassadarRealSettlementGate } from './tassadar-run-settlement-gate'
import { buildTrainingRunRecord } from './training-run-window-authority'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

const nowIso = '2026-06-18T14:30:00.000Z'
const digest = (char: string) => `sha256:${char.repeat(64)}`

class MemoryLedgerStore implements NexusTreasuryPayoutLedgerStore {
  attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  attemptsByIdempotency = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  intentsByIdempotency = new Map<string, NexusTreasuryPayoutIntentRecord>()
  receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()

  createPayoutAttempt = async (record: NexusTreasuryPayoutAttemptRecord) => {
    this.attempts.set(record.payoutAttemptRef, record)
    this.attemptsByIdempotency.set(record.idempotencyKeyHash, record)
  }

  createPayoutIntent = async (record: NexusTreasuryPayoutIntentRecord) => {
    this.intents.set(record.payoutIntentRef, record)
    this.intentsByIdempotency.set(record.idempotencyKeyHash, record)
  }

  createPayoutTargetApproval = async () => {}

  createPaymentAuthorityReceipt = async (
    record: NexusPaymentAuthorityReceiptRecord,
  ) => {
    this.receipts.set(record.receiptRef, record)
  }

  createReconciliationEvent = async (
    record: NexusTreasuryPayoutReconciliationEventRecord,
  ) => {
    this.events.set(record.eventRef, record)
  }

  createReleaseGate = async () => {}

  listPaymentAuthorityReceipts = async (limit: number) =>
    [...this.receipts.values()].slice(0, limit)

  readPayoutAttemptByRef = async (ref: string) => this.attempts.get(ref)

  readPayoutAttemptByIdempotencyKeyHash = async (hash: string) =>
    this.attemptsByIdempotency.get(hash)

  readPayoutIntentByIdempotencyKeyHash = async (hash: string) =>
    this.intentsByIdempotency.get(hash)

  readPayoutIntentByBuyerPaymentRef = async (buyerPaymentRef: string) =>
    [...this.intents.values()].find(i => i.buyerPaymentRef === buyerPaymentRef)

  readPayoutIntentByRef = async (ref: string) => this.intents.get(ref)

  readPaymentAuthorityReceiptByRef = async (ref: string) =>
    this.receipts.get(ref)

  readReconciliationEventByRef = async (ref: string) => this.events.get(ref)
}

const run = (): TrainingRunRecord => ({
  ...buildTrainingRunRecord({
    makeId: () => 'run',
    nowIso,
    request: {
      manifest: { artifactDigestRefs: [], blockerRefs: [], spendCapSats: 100 },
      promiseRef: 'training.decentralized_training_launch.v1',
      trainingRunRef: 'run.tassadar.executor.20260615',
    },
  }),
  state: 'active',
})

const verifiedChallenge = (): TrainingVerificationChallengeRecord => ({
  challengeRef: 'challenge.tassadar.adversarial_divergence.1',
  commitmentRefs: ['commitment.tassadar.adversarial_divergence.1'],
  contributionRef: 'claim.public.tassadar_adversarial.divergence_001',
  createdAt: nowIso,
  failureCodes: [],
  homeworkKind: 'tassadar_adversarial_divergence',
  id: 'challenge_tassadar_adversarial_divergence_1',
  leaseExpiresAt: null,
  leaseRef: null,
  leasedToRef: null,
  maxAttempts: 3,
  payloadJson: '{}',
  publicProjectionJson: '{}',
  rejectedAt: null,
  samplingPolicy: 'per_contribution',
  state: 'Verified',
  timedOutAt: null,
  trainingRunRef: 'run.tassadar.executor.20260615',
  updatedAt: nowIso,
  verdictRefs: ['receipt.public.tassadar_adversarial_divergence.confirmed.1'],
  verificationClass: 'exact_trace_replay',
  verifiedAt: nowIso,
  windowRef: 'training.window.tassadar.executor.20260615.w-adversarial-1',
})

const lease = (): TrainingWindowLeaseRecord => ({
  claimedAt: nowIso,
  id: 'lease_tassadar_adversarial_1',
  leaseExpiresAt: '2026-06-18T16:00:00.000Z',
  leaseRef: 'lease.tassadar.adversarial_divergence.1',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.worker.adversarial_hunter',
  receiptRefs: [],
  state: 'active',
  trainingRunRef: 'run.tassadar.executor.20260615',
  windowRef: 'training.window.tassadar.executor.20260615.w-adversarial-1',
})

const gate = (
  overrides: Partial<TassadarRealSettlementGate> = {},
): TassadarRealSettlementGate => ({
  allowedAdapterKind: 'spark_treasury',
  allowedContributorRefs: [],
  allowedRunRefs: ['run.tassadar.executor.20260615'],
  enabled: false,
  maxDailyPayoutSats: 50_000,
  maxPayoutSats: 100,
  runScopedStreaming: true,
  ...overrides,
})

const deps = (
  ledger: MemoryLedgerStore,
  dispatchCount: { value: number },
): TassadarAutoSettlementDeps<Record<string, unknown>> => ({
  dispatchRealSettlement: input =>
    Effect.gen(function* () {
      dispatchCount.value += 1
      yield* Effect.promise(() =>
        ledger.createPaymentAuthorityReceipt(input.settlement.settlementReceipt),
      )
    }),
  ledger,
  nowIso,
  readGate: () => gate(),
  resolvePayoutDestination: async () => undefined,
  run: run(),
})

const claim = (
  overrides: Partial<TassadarAdversarialDivergenceClaim> = {},
): TassadarAdversarialDivergenceClaim => ({
  claimRef: 'claim.public.tassadar_adversarial.divergence_001',
  claimantActorRef: 'agent:adversarial-verifier',
  claimantDeviceRef: 'device.pylon.adversarial_hunter',
  divergenceKind: 'trace_digest_mismatch',
  expectedBehaviorDigest: digest('1'),
  implementationRefs: [
    'implementation.public.tassadar.reference_linear',
    'implementation.public.tassadar.hull_cache',
  ],
  inputDigest: digest('3'),
  inputRef: 'input.public.tassadar_adversarial.divergence_001',
  moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  moduleKind: TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
  moduleRef: 'module.public.tassadar.linked_dense.canonical',
  observedBehaviorDigest: digest('2'),
  psionicEvidenceRefs: [
    'report.public.psionic.tassadar_exactness_refusal.step_mismatch',
  ],
  sourceRefs: ['source.public.psionic.tassadar_trace_diff_report'],
  specRef: 'spec.public.tassadar.linked_dense.w3_100m',
  workRequestId: 'work_request_tassadar_adversarial_001',
  ...overrides,
})

const reproduction = (
  overrides: Partial<TassadarAdversarialDivergenceReproduction> = {},
): TassadarAdversarialDivergenceReproduction => ({
  blockerRefs: [],
  expectedBehaviorDigest: digest('1'),
  inputDigest: digest('3'),
  observedBehaviorDigest: digest('2'),
  psionicEvidenceRefs: [
    'report.public.psionic.tassadar_exactness_refusal.reproduced',
  ],
  reproduced: true,
  reproductionRef: 'reproduction.public.tassadar_adversarial.divergence_001',
  validatorActorRef: 'agent:independent-validator',
  validatorDeviceRef: 'device.pylon.independent_validator',
  validatorReceiptRefs: [
    'receipt.public.tassadar_adversarial.validator_replay_001',
  ],
  ...overrides,
})

describe('Tassadar adversarial-verification market', () => {
  test('confirms a reproducible independent divergence and settles through the V1 path in simulation mode by default', async () => {
    const verdict = verifyTassadarAdversarialDivergenceClaim({
      claim: claim(),
      reproduction: reproduction(),
    })
    const releaseGate =
      projectTassadarAdversarialVerificationReleaseGate(verdict)
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlement = await Effect.runPromise(
      settleConfirmedTassadarDivergenceDefect(deps(ledger, dispatchCount), {
        challenge: verifiedChallenge(),
        lease: lease(),
        verdict,
      }),
    )

    expect(verdict).toMatchObject({
      reproducible: true,
      settlementEligible: true,
      status: 'confirmed_defect',
      verificationClass: 'e3_adversarial_divergence',
    })
    expect(releaseGate).toMatchObject({
      releaseAllowed: true,
      status: 'accepted',
    })
    expect(settlement).toMatchObject({
      kind: 'settlement_attempted',
      realBitcoinMoved: false,
      skipped: null,
    })
    expect(settlement.settlement?.mode).toBe('unpaid_smoke_simulation')
    expect(settlement.settlement?.amountSats).toBe(
      TassadarAdversarialDivergenceRewardSats,
    )
    expect(dispatchCount.value).toBe(0)
    expect(ledger.receipts.size).toBe(1)

    const receipt = ledger.receipts.get(settlement.settlementReceiptRef!)!
    const projection = JSON.parse(receipt.publicProjectionJson) as {
      constructionContributionRef: string
      moneyMovement: string
      settlementSource: string
    }
    expect(projection.constructionContributionRef).toBe(
      verdict.defectContributionRef,
    )
    expect(projection.moneyMovement).toBe('none')
    expect(projection.settlementSource).toBe('compiled_module_construction')
    expect(receipt.publicProjectionJson).not.toMatch(/spark1|invoice|preimage/i)
  })

  test('rejects non-reproducible divergence claims and pays nothing', async () => {
    const verdict = verifyTassadarAdversarialDivergenceClaim({
      claim: claim(),
      reproduction: reproduction({
        reproduced: false,
        validatorReceiptRefs: [
          'receipt.public.tassadar_adversarial.validator_false_claim_001',
        ],
      }),
    })
    const ledger = new MemoryLedgerStore()
    const dispatchCount = { value: 0 }
    const settlement = await Effect.runPromise(
      settleConfirmedTassadarDivergenceDefect(deps(ledger, dispatchCount), {
        challenge: verifiedChallenge(),
        lease: lease(),
        verdict,
      }),
    )

    expect(verdict.status).toBe('rejected_false_claim')
    expect(verdict.blockerRefs).toContain(
      'blocker.public.tassadar_adversarial.validator_did_not_reproduce',
    )
    expect(projectTassadarAdversarialVerificationReleaseGate(verdict)).toMatchObject({
      releaseAllowed: false,
      status: 'rejected',
    })
    expect(settlement).toMatchObject({
      kind: 'not_confirmed',
      realBitcoinMoved: false,
      settlement: null,
      skipped: 'not_confirmed',
    })
    expect(dispatchCount.value).toBe(0)
    expect(ledger.receipts.size).toBe(0)
  })

  test('requires explicit psionic near-miss refusal evidence and public-safe refs', () => {
    const nearMiss = verifyTassadarAdversarialDivergenceClaim({
      claim: claim({
        divergenceKind: 'near_miss_refusal_missing',
      }),
      reproduction: reproduction(),
    })

    expect(nearMiss.status).toBe('rejected_false_claim')
    expect(nearMiss.blockerRefs).toContain(
      'blocker.public.tassadar_adversarial.near_miss_refusal_ref_missing',
    )
    expect(() =>
      verifyTassadarAdversarialDivergenceClaim({
        claim: claim({
          sourceRefs: ['source.raw_private_trace'],
        }),
        reproduction: reproduction(),
      }),
    ).toThrow(TassadarAdversarialVerificationUnsafe)
  })
})
