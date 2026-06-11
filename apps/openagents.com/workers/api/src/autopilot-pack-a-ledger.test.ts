import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PackAArtifactRecord,
  PackABudgetPolicy,
  PackALedgerProjection,
  PackALedgerUnsafe,
  PackAReceiptRecord,
  PackAUsageEventRecord,
  PackAUsageProjection,
  PayoutVisibilityCell,
  SettlementLedgerReceipt,
  TeamBudgetRecord,
  TeamSpendRecord,
  appendPackAReceiptIdempotent,
  assertPackAArtifactPublicSafe,
  checkPackAClaimRequirements,
  decidePackABudget,
  joinTeamSpendEvidence,
  payoutVisibilityMatrix,
  projectPackALedger,
  projectPackAUsage,
  projectSettlementChain,
  settlementStageAuthorityLabel,
} from './autopilot-pack-a-ledger'

const nowIso = '2026-06-11T23:20:00.000Z'

const artifact = (
  input: Partial<PackAArtifactRecord> = {},
): PackAArtifactRecord =>
  new PackAArtifactRecord({
    artifactRef: 'artifact.pack_a.task_output.public_1',
    createdAt: nowIso,
    digest: 'sha256.public_artifact_1',
    kind: 'task_output',
    mediaType: 'application/json',
    payloadRef: 'payload.pack_a.task_output.public_1',
    producerAdapter: 'adapter.openagents.pack_a',
    redactionClass: 'public_ref',
    relatedReceiptRefs: ['receipt.pack_a.task_completed.1'],
    retentionPolicy: 'proof_retained',
    sizeBytes: 128,
    visibility: 'public',
    ...input,
  })

const receipt = (input: Partial<PackAReceiptRecord> = {}): PackAReceiptRecord =>
  new PackAReceiptRecord({
    artifactRefs: ['artifact.pack_a.task_output.public_1'],
    createdAt: nowIso,
    idempotencyKey: 'idempotency.pack_a.task_completed.1',
    kind: 'task_completed',
    previousReceiptRefs: [],
    receiptRef: 'receipt.pack_a.task_completed.1',
    subjectRef: 'task.pack_a.1',
    ...input,
  })

const usage = (
  input: Partial<PackAUsageEventRecord> = {},
): PackAUsageEventRecord =>
  new PackAUsageEventRecord({
    budgetRef: 'budget.pack_a.task_1',
    cacheReadTokens: 25,
    cacheWriteTokens: 15,
    contextEstimateTokens: 8_000,
    costMicros: 4_000,
    createdAt: nowIso,
    currency: 'usd',
    eventRef: 'usage.pack_a.task_1.1',
    externalAdapterSpendMicros: null,
    inputTokens: 1_000,
    kind: 'provider_response_usage',
    maxOutputReservationTokens: 2_000,
    outputTokens: 500,
    paymentState: 'not_charged',
    pricingState: 'known',
    processRuntimeMs: 10_000,
    providerRef: 'provider.openagents.shc',
    rateLimitResetRef: null,
    receiptRefs: ['receipt.pack_a.usage.1'],
    scheduleRef: 'schedule.pack_a.1',
    taskRef: 'task.pack_a.1',
    toolCallTokens: 40,
    userRequestedTokenTarget: 12_000,
    wallClockMs: 12_000,
    ...input,
  })

const policy = new PackABudgetPolicy({
  askAtPercent: 80,
  budgetRef: 'budget.pack_a.task_1',
  compactAtContextPercent: 70,
  hardStopAtPercent: 100,
  maxCostMicros: 10_000,
  maxRetryCount: 2,
  pauseAtPercent: 90,
  warnAtPercent: 60,
})

describe('Pack A Ledger contracts', () => {
  test('projects public artifact and receipt refs with generatedAt and staleness metadata', () => {
    const projection = projectPackALedger({
      artifacts: [
        artifact(),
        artifact({
          artifactRef: 'artifact.pack_a.operator.private_1',
          payloadRef: 'payload.pack_a.operator.private_1',
          redactionClass: 'operator_summary',
          visibility: 'operator',
        }),
      ],
      generatedAt: nowIso,
      receipts: [receipt()],
      visibility: 'public',
    })

    expect(() =>
      S.decodeUnknownSync(PackALedgerProjection)(projection),
    ).not.toThrow()
    expect(projection.artifactRefs).toEqual([
      'artifact.pack_a.task_output.public_1',
    ])
    expect(projection.caveatRefs).toContain(
      'caveat.pack_a_ledger.visibility_narrowed',
    )
    expect(projection.generatedAt).toBe(nowIso)
    expect(projection.staleness.maxStalenessSeconds).toBe(0)
  })

  test('rejects unsafe public artifacts and keeps duplicate receipt append idempotent', () => {
    expect(() =>
      assertPackAArtifactPublicSafe(
        artifact({
          payloadRef: '/Users/operator/private/raw_prompt.txt',
        }),
      ),
    ).toThrow(PackALedgerUnsafe)

    const first = appendPackAReceiptIdempotent([], receipt())
    const second = appendPackAReceiptIdempotent(first.receipts, receipt())

    expect(first.inserted).toBe(true)
    expect(second.inserted).toBe(false)
    expect(second.receipts).toHaveLength(1)
  })

  test('requires distinct lifecycle receipt kinds for proof claims', () => {
    const result = checkPackAClaimRequirements('m10_overnight_unattended', [
      receipt({ kind: 'schedule_fired', receiptRef: 'receipt.schedule_fired' }),
      receipt({ kind: 'task_completed', receiptRef: 'receipt.task_completed' }),
      receipt({
        kind: 'notification_delivered',
        receiptRef: 'receipt.notification_delivered',
      }),
      receipt({
        kind: 'review_recorded',
        receiptRef: 'receipt.review_recorded',
      }),
    ])

    expect(result.ready).toBe(false)
    expect(result.missingReceiptKinds).toEqual([
      'verification_passed',
      'delivery_recorded',
    ])
    expect(
      checkPackAClaimRequirements('p4_settlement_bridge', [
        receipt({ kind: 'delivery_recorded' }),
        receipt({ kind: 'acceptance_recorded' }),
        receipt({ kind: 'settlement_recorded' }),
      ]).ready,
    ).toBe(true)
  })

  test('decides usage budgets from cost, context, unknown pricing, free own-Pylon, and rate limits', () => {
    expect(
      decidePackABudget({
        contextWindowTokens: 10_000,
        policy,
        retryCount: 0,
        usage: usage({ contextEstimateTokens: 8_000 }),
      }),
    ).toMatchObject({
      decision: 'compact',
      receiptKind: 'usage_threshold_crossed',
    })
    expect(
      decidePackABudget({
        contextWindowTokens: 50_000,
        policy,
        retryCount: 0,
        usage: usage({ costMicros: null, pricingState: 'unknown' }),
      }),
    ).toMatchObject({
      decision: 'continue_with_caveat',
      reasonRef: 'budget.pricing_unknown_not_zero',
    })
    expect(
      decidePackABudget({
        contextWindowTokens: 50_000,
        policy,
        retryCount: 2,
        usage: usage({ kind: 'rate_limit_observed' }),
      }),
    ).toMatchObject({
      blockerRefs: ['blocker.pack_a_usage.retry_budget_exhausted'],
      decision: 'stop',
      receiptKind: 'usage_budget_stop',
    })

    const projection = projectPackAUsage(
      usage({
        costMicros: null,
        currency: null,
        paymentState: 'free_own_pylon',
        pricingState: 'unknown',
      }),
      nowIso,
    )

    expect(() =>
      S.decodeUnknownSync(PackAUsageProjection)(projection),
    ).not.toThrow()
    expect(projection.costMicros).toBeNull()
    expect(projection.paymentState).toBe('free_own_pylon')
    expect(projection.caveatRefs).toContain(
      'caveat.pack_a_usage.unknown_pricing_not_zero',
    )
  })

  test('joins team spend to mission, ledger, artifact, and receipt evidence while blocking over cap', () => {
    const join = joinTeamSpendEvidence({
      budget: new TeamBudgetRecord({
        budgetRef: 'team_budget.alpha.month',
        currency: 'usd',
        maxMicros: 10_000,
        perMissionCapMicros: 5_000,
        periodRef: 'period.2026_06',
        teamRef: 'team.alpha',
      }),
      generatedAt: nowIso,
      missionRef: 'mission.alpha.1',
      proposedSpendMicros: 3_000,
      spendRecords: [
        new TeamSpendRecord({
          amountMicros: 3_000,
          artifactRefs: ['artifact.pack_a.task_output.public_1'],
          ledgerEntryRef: 'ledger.team.alpha.1',
          missionRef: 'mission.alpha.1',
          receiptRefs: ['receipt.pack_a.task_completed.1'],
          spendRef: 'spend.alpha.1',
          teamRef: 'team.alpha',
        }),
      ],
    })

    expect(join.spendAllowed).toBe(false)
    expect(join.blockedReasonRefs).toEqual([
      'blocker.team_budget.per_mission_cap',
    ])
    expect(join.ledgerEntryRefs).toEqual(['ledger.team.alpha.1'])
    expect(join.artifactRefs).toEqual(['artifact.pack_a.task_output.public_1'])
    expect(join.generatedAt).toBe(nowIso)
  })

  test('keeps payment, acceptance, conversion, escrow, and settlement receipts distinct', () => {
    const chain = projectSettlementChain({
      generatedAt: nowIso,
      receipts: [
        settlementReceipt('buyer_credit_debit', {
          asset: 'usd',
          receiptRef: 'receipt.usd_debit.1',
          settlementAuthority: 'payment_only',
        }),
        settlementReceipt('conversion', {
          asset: 'sats',
          conversionRef: 'conversion.usd_to_sats.1',
          receiptRef: 'receipt.conversion.1',
          settlementAuthority: 'conversion_only',
        }),
        settlementReceipt('escrow_hold', {
          asset: 'sats',
          receiptRef: 'receipt.escrow.1',
          settlementAuthority: 'escrow_only',
        }),
        settlementReceipt('accepted_work', {
          asset: 'sats',
          payoutEligible: true,
          receiptRef: 'receipt.accepted_work.1',
          settlementAuthority: 'accepted_work_only',
        }),
      ],
      workOrderRef: 'work_order.ledger.1',
    })

    expect(chain.payoutEligible).toBe(true)
    expect(chain.settledBitcoinClaimAllowed).toBe(false)
    expect(chain.caveatRefs).toContain('caveat.settlement.not_settled')
    expect(settlementStageAuthorityLabel('buyer_credit_debit')).toBe(
      'USD credit debit only',
    )

    const settled = projectSettlementChain({
      generatedAt: nowIso,
      receipts: [
        ...chain.receiptRefs.map((receiptRef, index) =>
          settlementReceipt(
            [
              'buyer_credit_debit',
              'conversion',
              'escrow_hold',
              'accepted_work',
            ][index] as SettlementLedgerReceipt['stage'],
            { receiptRef },
          ),
        ),
        settlementReceipt('settlement', {
          asset: 'sats',
          receiptRef: 'receipt.settlement.1',
          settlementAuthority: 'settled_bitcoin',
        }),
      ],
      workOrderRef: 'work_order.ledger.1',
    })

    expect(settled.settledBitcoinClaimAllowed).toBe(true)
  })

  test('opens payout visibility gate only when every rung and surface is green', () => {
    const partial = payoutVisibilityMatrix({
      cells: [
        visibilityCell('direct_settlement', 'recipient_view', 'green'),
        visibilityCell('direct_settlement', 'public_receipt', 'typed_absent'),
      ],
      generatedAt: nowIso,
    })

    expect(partial.releaseGateOpen).toBe(false)
    expect(partial.staleness.rebuildsOn).toContain('payout_receipt_published')

    const complete = payoutVisibilityMatrix({
      cells: [
        'conversion',
        'credited_and_swept',
        'direct_settlement',
        'escrow_hold',
      ].flatMap(rung =>
        ['auditor_aggregate', 'public_receipt', 'recipient_view'].map(surface =>
          visibilityCell(
            rung as PayoutVisibilityCell['rung'],
            surface as PayoutVisibilityCell['surface'],
            'green',
          ),
        ),
      ),
      generatedAt: nowIso,
    })

    expect(complete.releaseGateOpen).toBe(true)
  })
})

const settlementReceipt = (
  stage: SettlementLedgerReceipt['stage'],
  input: Partial<SettlementLedgerReceipt> = {},
): SettlementLedgerReceipt =>
  new SettlementLedgerReceipt({
    amountMinorUnits: 1_000,
    asset: 'sats',
    conversionRef: null,
    createdAt: nowIso,
    duplicateKey: `duplicate.${stage}.${input.receiptRef ?? 'default'}`,
    fromReceiptRefs: [],
    payoutEligible: false,
    providerRef: 'provider.public.ledger_1',
    receiptRef: `receipt.${stage}.default`,
    settlementAuthority: 'accepted_work_only',
    spendCapRef: 'spend_cap.ledger_1',
    stage,
    workOrderRef: 'work_order.ledger.1',
    ...input,
  })

const visibilityCell = (
  rung: PayoutVisibilityCell['rung'],
  surface: PayoutVisibilityCell['surface'],
  state: PayoutVisibilityCell['state'],
): PayoutVisibilityCell =>
  new PayoutVisibilityCell({
    generatedAt: nowIso,
    reasonRef:
      state === 'green'
        ? `visibility.${rung}.${surface}.ready`
        : `visibility.${rung}.${surface}.absent`,
    receiptRef:
      state === 'green' ? `receipt.visibility.${rung}.${surface}` : null,
    rung,
    state,
    staleness: {
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
      rebuildsOn: ['settlement_visibility_cell_recorded'],
    },
    surface,
  })
