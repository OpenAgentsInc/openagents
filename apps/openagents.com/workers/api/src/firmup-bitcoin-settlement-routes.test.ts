import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { sha256Hex } from './agent-registration'
import {
  type FirmupSettleableEscrowProjection,
  makeFirmupBitcoinSettlementRoutes,
} from './firmup-bitcoin-settlement-routes'
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
 * Firm-up escrow -> Bitcoin settlement DISPATCH route tests (openagents #5459).
 *
 * MONEY-SAFETY: every test drives a MOCK Spark adapter (`CountingSparkAdapter`)
 * and an in-memory ledger. No real payout is ever triggered and no receipt is
 * ever faked — the mock adapter returns dispatch/reconcile records the route's
 * own builder shaped, exactly as the proven hygiene-lane route test does.
 *
 * These assert the route:
 *   - reuses the proven Spark money rail + the SAME owner gate,
 *   - clears real money ONLY against an EXECUTED + verified verdict (a manual
 *     attestation or a rejected verification is refused),
 *   - enforces worker != validator,
 *   - is idempotent per escrow release (one payout per release),
 *   - records an honest `firmup_executed_verification` basis citing the executed
 *     verdict + command, never a manual attestation, and
 *   - keeps the public projection secret-safe.
 */

const FIRMUP_RUN_REF = 'run.firmup.lane.20260618'
const WORKER_REF = 'pylon.public.worker.orrery'
const VALIDATOR_REF = 'pylon.public.validator.whitefang'
const ESCROW_REF = 'labor_escrow.public.escrow_5459'
const VERIFICATION_COMMAND_REF = 'command.public.firmup.5459.bun_test'

const settleableEscrow: FirmupSettleableEscrowProjection = {
  amountSats: 50,
  escrowRef: ESCROW_REF,
  providerActorRef: WORKER_REF,
  verificationCommandRef: VERIFICATION_COMMAND_REF,
  workRequestRef: 'work_request.public.firmup_5459',
}

// An EXECUTED, verified verdict: it carries the executed-trace digest that a
// manual attestation cannot supply.
const executedVerifiedVerdict = {
  executedTraceDigestPrefix: 'sha256_a1b2c3d4e5f6',
  outcome: 'verified',
  validatorActorRef: VALIDATOR_REF,
  verdictRef: 'verdict.public.firmup.5459.executed_verified',
  verificationCommandRef: VERIFICATION_COMMAND_REF,
}

const settlementBody = (overrides: Record<string, unknown> = {}) => ({
  adapterKind: 'spark_treasury',
  escrowRef: ESCROW_REF,
  operatorApprovalRef: 'operator.approval.firmup.5459',
  payoutTargetApprovalRef: 'payout.target.approval.firmup.5459',
  payoutTargetRef: 'payout.target.firmup.5459',
  trainingRunRef: FIRMUP_RUN_REF,
  verdict: executedVerifiedVerdict,
  ...overrides,
})

const enabledGateEnv = (overrides: Record<string, unknown> = {}) => ({
  OPENAGENTS_REAL_SETTLEMENT_GATE: JSON.stringify({
    allowedAdapterKind: 'spark_treasury',
    allowedContributorRefs: [WORKER_REF],
    allowedRunRefs: [FIRMUP_RUN_REF],
    enabled: true,
    maxDailyPayoutSats: 5_000,
    maxPayoutSats: 100,
    ...overrides,
  }),
})

const jsonRequest = (body: Record<string, unknown>): Request =>
  new Request('https://openagents.test/api/firmup-lane/settlement-receipt', {
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
    throw new Error('not implemented in firm-up ledger store')
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
    Parameters<typeof makeFirmupBitcoinSettlementRoutes>[0]
  > = {},
  ledger = makeLedgerStore(),
  spark = new CountingSparkAdapter(),
) =>
  makeFirmupBitcoinSettlementRoutes<Record<string, unknown>>({
    makePayoutLedgerStore: () => ledger,
    makeSettlementPaymentAuthority: (_env, context) =>
      makeTreasuryPaymentAuthority({
        adapters: [spark.adapter],
        ledgerStore: context.ledgerStore,
      }),
    nowIso: () => '2026-06-18T10:05:00.000Z',
    readSettlementWalletReadiness: async () => 'ready',
    requireAdminApiToken: async () => true,
    resolveSettleableEscrow: async (): Promise<
      FirmupSettleableEscrowProjection | undefined
    > => settleableEscrow,
    resolveSettlementPayoutDestination: async () => 'destination.test',
    ...overrides,
  })

const settlementReceiptFromStore = (
  ledger: ReturnType<typeof makeLedgerStore>,
): NexusPaymentAuthorityReceiptRecord | undefined =>
  [...ledger.receipts.values()].find(
    receipt => receipt.receiptKind === 'settlement_recorded',
  )

describe('POST /api/firmup-lane/settlement-receipt (firm-up -> bitcoin, #5459)', () => {
  it('rejects non-admin callers', async () => {
    const routes = makeRoutes({ requireAdminApiToken: async () => false })

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(jsonRequest(settlementBody()), {}),
    )

    expect(response.status).toBe(401)
  })

  it('gate OFF (default) => honest simulation, no dispatch, realBitcoinMoved:false', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(jsonRequest(settlementBody()), {}),
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
    expect(json.settlement.verificationBasis).toBe('firmup_executed_verification')

    const receipt = settlementReceiptFromStore(ledger)
    expect(receipt).toBeDefined()
    const projection = JSON.parse(receipt!.publicProjectionJson) as {
      moneyMovement: string
      verificationBasis: string
    }
    expect(projection.moneyMovement).toBe('none')
    // Honest basis even on the simulation chain; the receipt names the EXECUTED
    // verdict + command, NEVER a manual attestation.
    expect(projection.verificationBasis).toBe('firmup_executed_verification')
    expect(receipt!.publicProjectionJson).toContain(
      executedVerifiedVerdict.verdictRef,
    )
    expect(receipt!.publicProjectionJson).not.toContain('manual_attestation')
  })

  it('gate ON + executed-verified verdict + under cap => exactly one dispatch, realBitcoinMoved:true', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
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
      validatorActorRef: string
      verdictRef: string
      verificationBasis: string
    }
    expect(projection.moneyMovement).toBe('real_bitcoin')
    expect(projection.state).toBe('settled')
    expect(projection.verificationBasis).toBe('firmup_executed_verification')
    // The receipt references the EXECUTED verification outcome (verdict +
    // distinct validator), the trust anchor for paying real money.
    expect(projection.verdictRef).toBe(executedVerifiedVerdict.verdictRef)
    expect(projection.validatorActorRef).toBe(VALIDATOR_REF)

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

  it('retry of the same escrow release dispatches at most once (one payout per release)', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const first = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )
    const second = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    // The deterministic receipt ref (keyed on escrowRef) short-circuits the
    // second dispatch.
    expect(spark.dispatchCalls).toBe(1)
    expect(
      [...ledger.receipts.values()].filter(
        receipt => receipt.receiptKind === 'settlement_recorded',
      ),
    ).toHaveLength(1)
  })

  it('a MANUAL attestation (no executed-trace digest) is refused — never pays real money', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        // A bare attestation string, the OLD release shape — not an executed
        // verdict. The schema decode rejects it as malformed.
        jsonRequest(
          settlementBody({ verdict: 'verdict.public.live.validator_passed' }),
        ),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(400)
    expect(spark.dispatchCalls).toBe(0)
  })

  it('an EXECUTED but REJECTED verification is refused — never pays', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(
          settlementBody({
            verdict: { ...executedVerifiedVerdict, outcome: 'rejected' },
          }),
        ),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(409)
    expect(spark.dispatchCalls).toBe(0)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('verification_rejected')
  })

  it('a self-verified firm-up (worker == validator) is refused', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes({}, ledger, spark)

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(
          settlementBody({
            verdict: {
              ...executedVerifiedVerdict,
              validatorActorRef: WORKER_REF,
            },
          }),
        ),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(409)
    expect(spark.dispatchCalls).toBe(0)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('worker_validator_not_distinct')
  })

  it('a verdict for a different verification command is refused', async () => {
    const routes = makeRoutes()

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(
          settlementBody({
            verdict: {
              ...executedVerifiedVerdict,
              verificationCommandRef: 'command.public.firmup.other',
            },
          }),
        ),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('verification_command_ref_mismatch')
  })

  it('a missing settleable escrow fails closed (not found, no dispatch)', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes(
      { resolveSettleableEscrow: async () => undefined },
      ledger,
      spark,
    )

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(404)
    expect(spark.dispatchCalls).toBe(0)
  })

  it('a non-firmup run-ref fails closed (validation error, not settleable)', async () => {
    const routes = makeRoutes()

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(settlementBody({ trainingRunRef: 'run.cs336.a1.demo' })),
        enabledGateEnv({ allowedRunRefs: ['run.cs336.a1.demo'] }),
      ),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('not_firmup_lane_run_ref')
  })

  it('RL-3 boundary: a credit-funded escrow may NOT settle to withdrawable Bitcoin (refused, no dispatch)', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes(
      {
        resolveSettleableEscrow: async () => ({
          ...settleableEscrow,
          // A credit-funded escrow: the shared credit<->Bitcoin boundary refuses
          // turning it into a withdrawable Bitcoin payout.
          revenueAsset: 'credit',
        }),
      },
      ledger,
      spark,
    )

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(409)
    expect(spark.dispatchCalls).toBe(0)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('asset_boundary_violation')
  })

  it('RL-3 boundary: a Bitcoin-funded escrow (default) settles normally — the valid crossing', async () => {
    const ledger = makeLedgerStore()
    const spark = new CountingSparkAdapter()
    const routes = makeRoutes(
      {
        resolveSettleableEscrow: async () => ({
          ...settleableEscrow,
          revenueAsset: 'bitcoin',
        }),
      },
      ledger,
      spark,
    )

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(
        jsonRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(200)
    expect(spark.dispatchCalls).toBe(1)
    const json = (await response.json()) as {
      settlement: { realAuthorized: boolean }
    }
    expect(json.settlement.realAuthorized).toBe(true)
  })

  it('hashes escrow refs before deriving settlement receipt refs (no raw ref leakage)', async () => {
    const routes = makeRoutes()

    const response = await runRoute(
      routes.routeFirmupLaneSettlementRequest(jsonRequest(settlementBody()), {}),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as {
      settlement: { settlementReceiptRef: string }
    }
    const digest = await sha256Hex(`firmup_lane_settlement:${ESCROW_REF}`)
    expect(json.settlement.settlementReceiptRef).toContain(
      `sha256_${digest.slice(0, 32)}`,
    )
  })
})
