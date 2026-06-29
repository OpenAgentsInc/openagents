import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { sha256Hex } from './agent-registration'
import { deriveDebtReceiptKey } from './debt-receipt-key'
import {
  type DebtReceiptSettlementInput,
  type DebtReceiptSettlementProjection,
  projectDebtReceiptSettlement,
} from './debt-receipt-policy'
import { makeHygieneLaneSettlementRoutes } from './hygiene-lane-settlement-routes'
import { nexusPylonPublicReceiptDetailFromLedger } from './nexus-pylon-visibility'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerStore,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import {
  type TreasuryPaymentAuthorityAdapter,
  makeTreasuryPaymentAuthority,
} from './treasury-payment-authority'

/**
 * Honest hygiene-lane settlement DISPATCH route tests (openagents #5372).
 *
 * These assert the route reuses the proven Spark money rail and the SAME owner
 * gate, settles on an HONEST `hygiene_merged_reviewed` basis (NEVER an
 * exact_trace_replay verdict), is one-settlement-per-DebtReceiptKey + idempotent,
 * and is public-projection-safe (no spark1.../preimages/raw destination).
 */

const HYGIENE_RUN_REF = 'run.hygiene.lane.20260618'
const CONTRIBUTOR_REF = 'pylon.public.contributor.trigger'

const payableDebtReceiptInput: DebtReceiptSettlementInput = {
  acceptedWorkRefs: ['accepted_work.public.debt_receipt.5334.fixture_dedup'],
  baselineMetricRefs: ['metric.public.debt_receipt.5334.baseline'],
  budgetCapSats: 100,
  debtReceiptKeyInput: {
    debtReceiptRef: 'receipt.public.debt.5334',
    objectiveDigest: 'objective.public.debt_receipt.5334.dual_format_to_zero',
    repoBaselineRef: 'baseline.public.commit.c43992567',
    scopeDigest: 'scope.public.debt_receipt.5334.tassadar_fixture_pairs',
  },
  fundingApprovalRefs: ['approval.public.debt_receipt.5334.funded'],
  fundingAuthorityActorRef: 'actor.public.owner.allocator',
  fundingAuthorityRefs: ['authority.public.debt_receipt.allocator_route'],
  hygieneDeltaRefs: ['delta.public.debt_receipt.5334.dual_format_removed'],
  noNewEqualOrWorseDebtRefs: ['check.public.debt_receipt.5334.no_worse_debt'],
  payableSats: 80,
  proposerActorRef: 'actor.public.orrery.churn_probe',
  reviewDecisionRefs: ['review.public.debt_receipt.5334.accepted'],
  reviewerActorRef: 'actor.public.reviewer.trigger',
  scopeRefs: ['scope.public.debt_receipt.5334.tassadar_fixture_pairs'],
  settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
  settlementAuthorityActorRef: 'actor.public.treasury.policy',
  sourceRefs: ['issue.public.github.openagentsinc_openagents.5334'],
  stopConditionRefs: ['stop.public.debt_receipt.5334.retire_once'],
  targetMetricRefs: ['metric.public.debt_receipt.5334.target.churn_0'],
  verificationCommandRefs: ['command.public.debt_receipt.5334.regen_and_diff'],
  workerActorRef: 'actor.public.worker.codex_loop',
}

const payableProjection = projectDebtReceiptSettlement(payableDebtReceiptInput)
const DEBT_RECEIPT_KEY = payableProjection.debtReceiptKey!

const goodSignals = {
  behaviorReceiptGreen: true,
  changedWeightedLines: 1_200,
  debtReducedWeightedUnits: 8,
  duplicateReplay: false,
  filesTouched: 4,
  newDebtWeightedUnits: 0,
}

const settlementBody = (overrides: Record<string, unknown> = {}) => ({
  adapterKind: 'spark_treasury',
  contributorRef: CONTRIBUTOR_REF,
  debtReceiptKeyRef: DEBT_RECEIPT_KEY,
  idempotencyRef: 'idem.hygiene.5334',
  mergedPrRef: 'pr.public.github.openagentsinc_openagents.5334',
  operatorApprovalRef: 'operator.approval.hygiene.5334',
  payoutTargetApprovalRef: 'payout.target.approval.hygiene.5334',
  payoutTargetRef: 'payout.target.hygiene.5334',
  reviewerAcceptanceRef: 'review.public.debt_receipt.5334.accepted',
  signals: goodSignals,
  trainingRunRef: HYGIENE_RUN_REF,
  ...overrides,
})

const enabledGateEnv = (overrides: Record<string, unknown> = {}) => ({
  OPENAGENTS_REAL_SETTLEMENT_GATE: JSON.stringify({
    allowedAdapterKind: 'spark_treasury',
    allowedContributorRefs: [CONTRIBUTOR_REF],
    allowedRunRefs: [HYGIENE_RUN_REF],
    enabled: true,
    maxDailyPayoutSats: 5_000,
    maxPayoutSats: 100,
    ...overrides,
  }),
})

const jsonRequest = (body: Record<string, unknown>): Request =>
  new Request('https://openagents.test/api/hygiene-lane/settlement-receipt', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const runRoute = async (
  route: Effect.Effect<Response> | undefined,
): Promise<Response> => {
  expect(route).toBeDefined()

  return Effect.runPromise(route!)
}

const makeLedgerStore = (): NexusTreasuryPayoutLedgerStore & {
  readonly receipts: Map<string, NexusPaymentAuthorityReceiptRecord>
} => {
  const intents = new Map<string, NexusTreasuryPayoutIntentRecord>()
  const intentsByIdem = new Map<string, NexusTreasuryPayoutIntentRecord>()
  const attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  const attemptsByIdem = new Map<string, NexusTreasuryPayoutAttemptRecord>()
  const events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>()
  const receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>()
  const notImplemented = async (): Promise<never> => {
    throw new Error('not implemented in hygiene ledger store')
  }

  return {
    createPaymentAuthorityReceipt: async record => {
      receipts.set(record.receiptRef, record)
    },
    createPayoutAttempt: async record => {
      attempts.set(record.payoutAttemptRef, record)
      attemptsByIdem.set(record.idempotencyKeyHash, record)
    },
    createPayoutIntent: async record => {
      intents.set(record.payoutIntentRef, record)
      intentsByIdem.set(record.idempotencyKeyHash, record)
    },
    createPayoutTargetApproval: async () => {},
    createReconciliationEvent: async record => {
      events.set(record.eventRef, record)
    },
    createReleaseGate: async () => {},
    listPaymentAuthorityReceipts: async () => [...receipts.values()],
    readPaymentAuthorityReceiptByRef: async receiptRef =>
      receipts.get(receiptRef),
    readPayoutAttemptByIdempotencyKeyHash: async idempotencyKeyHash =>
      attemptsByIdem.get(idempotencyKeyHash),
    readPayoutAttemptByRef: async payoutAttemptRef =>
      attempts.get(payoutAttemptRef),
    readPayoutIntentByBuyerPaymentRef: notImplemented,
    readPayoutIntentByIdempotencyKeyHash: async idempotencyKeyHash =>
      intentsByIdem.get(idempotencyKeyHash),
    readPayoutIntentByRef: async payoutIntentRef => intents.get(payoutIntentRef),
    readReconciliationEventByRef: async eventRef => events.get(eventRef),
    receipts,
  }
}

class CountingSparkAdapter {
  dispatchCalls = 0

  adapter: TreasuryPaymentAuthorityAdapter = {
    adapterKind: 'spark_treasury',
    dispatch: input =>
      Effect.suspend(() => {
        this.dispatchCalls += 1

        return Effect.succeed({
          ...input.attempt,
          adapterKind: 'spark_treasury' as const,
          publicProjectionJson: JSON.stringify({
            adapter: 'spark_treasury',
            moneyMovement: 'real_bitcoin',
            rawMaterialStored: false,
            state: 'dispatch_reported',
          }),
          redactedPaymentRef: 'payment.redacted.spark_treasury.test',
          status: 'dispatched' as const,
        })
      }),
    preview: input =>
      Effect.succeed({
        adapterKind: 'spark_treasury',
        amount: input.intent.amount,
        dispatchAllowed: true,
        payoutIntentRef: input.intent.payoutIntentRef,
        payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
        policySnapshotRef: input.intent.policySnapshotRef,
        spendCap: input.intent.spendCap,
      }),
    reconcile: input =>
      Effect.succeed({
        ...input.event,
        adapterKind: 'spark_treasury',
        publicProjectionJson: JSON.stringify({
          adapter: 'spark_treasury',
          moneyMovement: 'real_bitcoin',
          state: 'reconciliation_matched',
        }),
        status: 'matched' as const,
      }),
  }
}

const makeRoutes = (
  overrides: Partial<
    Parameters<typeof makeHygieneLaneSettlementRoutes>[0]
  > = {},
  ledger = makeLedgerStore(),
  spark = new CountingSparkAdapter(),
) =>
  makeHygieneLaneSettlementRoutes<Record<string, unknown>>({
    makePayoutLedgerStore: () => ledger,
    makeSettlementPaymentAuthority: (_env, context) =>
      makeTreasuryPaymentAuthority({
        adapters: [spark.adapter],
        ledgerStore: context.ledgerStore,
      }),
    nowIso: () => '2026-06-18T10:05:00.000Z',
    readSettlementWalletReadiness: async () => 'ready',
    requireAdminApiToken: async () => true,
    resolveDebtReceiptProjection: async (): Promise<
      DebtReceiptSettlementProjection | undefined
    > => payableProjection,
    resolveSettlementPayoutDestination: async () => 'destination.test',
    ...overrides,
  })

const settlementReceiptFromStore = (
  ledger: ReturnType<typeof makeLedgerStore>,
): NexusPaymentAuthorityReceiptRecord | undefined =>
  [...ledger.receipts.values()].find(
    receipt => receipt.receiptKind === 'settlement_recorded',
  )

describe('POST /api/hygiene-lane/settlement-receipt (honest dispatch, #5372)', () => {
  it('rejects non-admin callers', async () => {
    const routes = makeRoutes({ requireAdminApiToken: async () => false })

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(jsonRequest(settlementBody()), {}),
    )

    expect(response.status).toBe(401)
  })

  it('gate OFF (default) => honest simulation, no dispatch, realBitcoinMoved:false', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(jsonRequest(settlementBody()), {}),
    )

    expect(response.status).toBe(200)
    expect(spark.dispatchCalls).toBe(0)

    const json = (await response.json()) as {
      settlement: {
        moneyMovement: string
        realAuthorized: boolean
        verificationBasis: string
      }
    }
    expect(json.settlement.realAuthorized).toBe(false)
    expect(json.settlement.moneyMovement).toBe('none')
    expect(json.settlement.verificationBasis).toBe('hygiene_merged_reviewed')

    const receipt = settlementReceiptFromStore(ledger)
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson) as {
      moneyMovement: string
      verificationBasis: string
    }
    expect(projection.moneyMovement).toBe('none')
    // Honest basis even on the simulation chain; NEVER exact_trace_replay.
    expect(projection.verificationBasis).toBe('hygiene_merged_reviewed')
    expect(receipt!.publicProjectionJson).not.toContain('exact_trace_replay')
    expect(receipt!.publicProjectionJson).not.toContain(
      'verificationChallengeRef',
    )
  })

  it('gate ON + payable + under cap => exactly one dispatch, realBitcoinMoved:true', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(200)
    expect(spark.dispatchCalls).toBe(1)

    const json = (await response.json()) as {
      settlement: { moneyMovement: string; realAuthorized: boolean }
    }
    expect(json.settlement.realAuthorized).toBe(true)
    expect(json.settlement.moneyMovement).toBe('real_bitcoin')

    const receipt = settlementReceiptFromStore(ledger)
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson) as {
      moneyMovement: string
      state: string
      verificationBasis: string
    }
    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(projection.state).toBe('settled')
    expect(projection.verificationBasis).toBe('hygiene_merged_reviewed')

    // The public derivation must report real bitcoin moved.
    const detail = nexusPylonPublicReceiptDetailFromLedger({
      appUrl: 'https://openagents.com',
      attempt: await ledger.readPayoutAttemptByRef(receipt!.payoutAttemptRef!),
      event:
        receipt!.eventRef === null
          ? undefined
          : await ledger.readReconciliationEventByRef(receipt!.eventRef),
      intent: await ledger.readPayoutIntentByRef(receipt!.payoutIntentRef),
      nowIso: '2026-06-18T10:05:00.000Z',
      receipt: receipt!,
    })
    expect(detail.realBitcoinMoved).toBe(true)
  })

  it('retry of the same idempotencyRef dispatches at most once', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const first = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )
    const second = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    // The deterministic receipt ref short-circuits the second dispatch.
    expect(spark.dispatchCalls).toBe(1)
    expect(
      [...ledger.receipts.values()].filter(
        receipt => receipt.receiptKind === 'settlement_recorded',
      ),
    ).toHaveLength(1)
  })

  it('hashes idempotency refs before deriving settlement receipt refs', async () => {
    const commonPrefix = `idem.${'a'.repeat(160)}`
    const firstIdempotencyRef = `${commonPrefix}.first`
    const secondIdempotencyRef = `${commonPrefix}.second`

    const receiptRefFor = async (idempotencyRef: string): Promise<string> => {
      const routes = makeRoutes()
      const response = await runRoute(
        routes.routeHygieneLaneSettlementRequest(
          jsonRequest(settlementBody({ idempotencyRef })),
          {},
        ),
      )

      expect(response.status).toBe(200)
      const json = (await response.json()) as {
        settlement: { settlementReceiptRef: string }
      }

      return json.settlement.settlementReceiptRef
    }

    const firstReceiptRef = await receiptRefFor(firstIdempotencyRef)
    const secondReceiptRef = await receiptRefFor(secondIdempotencyRef)

    expect(firstReceiptRef).not.toBe(secondReceiptRef)
    expect(firstReceiptRef).toContain(
      `sha256_${(await sha256Hex(firstIdempotencyRef)).slice(0, 32)}`,
    )
    expect(secondReceiptRef).toContain(
      `sha256_${(await sha256Hex(secondIdempotencyRef)).slice(0, 32)}`,
    )
    expect(firstReceiptRef).not.toContain(commonPrefix.slice(0, 120))
  })

  it('a duplicate-replay debt receipt is never payable (one settlement per key, #5340)', async () => {
    const duplicateProjection = projectDebtReceiptSettlement({
      ...payableDebtReceiptInput,
      retiredDebtReceiptKeys: [
        deriveDebtReceiptKey(payableDebtReceiptInput.debtReceiptKeyInput!),
      ],
    })
    expect(duplicateProjection.duplicateReplay).toBe(true)

    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes(
      { resolveDebtReceiptProjection: async () => duplicateProjection },
      ledger,
      spark,
    )

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(409)
    expect(spark.dispatchCalls).toBe(0)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('duplicate_replay')
  })

  it('a missing debt-receipt projection fails closed (not found, no dispatch)', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes(
      { resolveDebtReceiptProjection: async () => undefined },
      ledger,
      spark,
    )

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(404)
    expect(spark.dispatchCalls).toBe(0)
  })

  it('a non-hygiene run-ref fails closed (validation error, not settleable)', async () => {
    const routes = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody({ trainingRunRef: 'run.cs336.a1.demo' })),
        enabledGateEnv({ allowedRunRefs: ['run.cs336.a1.demo'] }),
      ),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('not_hygiene_lane_run_ref')
  })

  it('a denied amount (no measured debt reduction) fails closed (not settleable)', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(
          settlementBody({
            signals: { ...goodSignals, debtReducedWeightedUnits: 0 },
          }),
        ),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(400)
    expect(spark.dispatchCalls).toBe(0)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('amount_denied')
  })

  it('no registered Spark destination => real branch fails closed (no send)', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes(
      { resolveSettlementPayoutDestination: async () => undefined },
      ledger,
      spark,
    )

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(409)
    expect(spark.dispatchCalls).toBe(0)
    expect(settlementReceiptFromStore(ledger)).toBeUndefined()
  })

  it('amount over the gate cap does NOT move real money (falls to honest simulation)', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(
          settlementBody({
            signals: {
              ...goodSignals,
              changedWeightedLines: 4_000,
              debtReducedWeightedUnits: 40,
              filesTouched: 12,
            },
          }),
        ),
        // The formula would produce 100 sats; the gate cap of 5 binds and the
        // real branch is not authorized, so NO real money moves.
        enabledGateEnv({ maxPayoutSats: 5 }),
      ),
    )

    expect(response.status).toBe(200)
    expect(spark.dispatchCalls).toBe(0)
    const json = (await response.json()) as {
      settlement: { moneyMovement: string; realAuthorized: boolean }
    }
    expect(json.settlement.realAuthorized).toBe(false)
    expect(json.settlement.moneyMovement).toBe('none')
  })

  it('the persisted receipt projection is public-projection-safe (no raw destination/preimage)', async () => {
    const ledger = makeLedgerStore()
    const routes = makeRoutes({}, ledger, new CountingSparkAdapter())

    await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    const receipt = settlementReceiptFromStore(ledger)!
    const serialized = JSON.stringify(receipt)
    expect(serialized).not.toMatch(/spark1[a-z0-9]/i)
    expect(serialized).not.toMatch(/lnbc|preimage|mnemonic|sk-/i)
    expect(serialized).not.toContain('destination.test')
    // The only destination ref present is the redacted form.
    expect(receipt.publicProjectionJson).not.toContain('redacted')
  })
})
