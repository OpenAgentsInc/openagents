import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { deriveDebtReceiptKey } from './debt-receipt-key'
import { makeInMemoryHygieneDebtReceiptStore } from './hygiene-debt-receipt-store'
import { makeHygieneLaneSettlementRoutes } from './hygiene-lane-settlement-routes'
import { makeTreasuryPaymentAuthority } from './treasury-payment-authority'

/**
 * Create-side route tests (#5372, EPIC #5335, process step 1):
 *   POST /api/hygiene-lane/debt-receipts
 *
 * Admin-only. The requester/settlement-authority creates a PAYABLE funded debt
 * receipt for a merged+reviewed PR; it is persisted keyed by its DebtReceiptKey
 * (#5340), idempotent. Then the existing settle route resolves payability from
 * the SAME store; once real bitcoin moves, the key retires so a 2nd settle is a
 * duplicate replay.
 */

const HYGIENE_RUN_REF = 'run.hygiene.lane.20260618'
const CONTRIBUTOR_REF = 'pylon.public.contributor.trigger'

const KEY_INPUT = {
  debtReceiptRef: 'receipt.public.debt.5358',
  objectiveDigest: 'objective.public.debt_receipt.5358.dedup_to_zero',
  repoBaselineRef: 'baseline.public.commit.3f636c133',
  scopeDigest: 'scope.public.debt_receipt.5358.target',
}
const DEBT_RECEIPT_KEY = deriveDebtReceiptKey(KEY_INPUT)

const createBody = (overrides: Record<string, unknown> = {}) => ({
  acceptedWorkRefs: ['accepted_work.public.debt_receipt.5358.dedup'],
  baselineMetricRefs: ['metric.public.debt_receipt.5358.baseline'],
  budgetCapSats: 100,
  debtReceiptKeyInput: KEY_INPUT,
  fundingApprovalRefs: ['approval.public.debt_receipt.5358.funded'],
  fundingAuthorityActorRef: 'actor.public.owner.allocator',
  fundingAuthorityRefs: ['authority.public.debt_receipt.allocator_route'],
  hygieneDeltaRefs: ['delta.public.debt_receipt.5358.removed'],
  mergedPrRef: 'pr.public.github.openagentsinc_openagents.5358',
  noNewEqualOrWorseDebtRefs: ['check.public.debt_receipt.5358.no_worse_debt'],
  payableSats: 40,
  proposerActorRef: 'actor.public.orrery.churn_probe',
  reviewDecisionRefs: ['review.public.debt_receipt.5358.accepted'],
  reviewerAcceptanceRef: 'review.public.debt_receipt.5358.accepted',
  reviewerActorRef: 'actor.public.reviewer.trigger',
  scopeRefs: ['scope.public.debt_receipt.5358.target'],
  settlementApprovalRefs: ['approval.public.debt_receipt.5358.settlement'],
  settlementAuthorityActorRef: 'actor.public.treasury.policy',
  sourceRefs: ['issue.public.github.openagentsinc_openagents.5358'],
  stopConditionRefs: ['stop.public.debt_receipt.5358.retire_once'],
  targetMetricRefs: ['metric.public.debt_receipt.5358.target.zero'],
  verificationCommandRefs: ['command.public.debt_receipt.5358.regen_and_diff'],
  workerActorRef: 'actor.public.worker.codex_loop',
  ...overrides,
})

const createRequest = (body: Record<string, unknown>): Request =>
  new Request('https://openagents.test/api/hygiene-lane/debt-receipts', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const settlementBody = (overrides: Record<string, unknown> = {}) => ({
  adapterKind: 'spark_treasury',
  contributorRef: CONTRIBUTOR_REF,
  debtReceiptKeyRef: DEBT_RECEIPT_KEY,
  idempotencyRef: 'idem.hygiene.5358',
  mergedPrRef: 'pr.public.github.openagentsinc_openagents.5358',
  operatorApprovalRef: 'operator.approval.hygiene.5358',
  payoutTargetApprovalRef: 'payout.target.approval.hygiene.5358',
  payoutTargetRef: 'payout.target.hygiene.5358',
  reviewerAcceptanceRef: 'review.public.debt_receipt.5358.accepted',
  signals: {
    behaviorReceiptGreen: true,
    changedWeightedLines: 1_200,
    debtReducedWeightedUnits: 8,
    duplicateReplay: false,
    filesTouched: 4,
    newDebtWeightedUnits: 0,
  },
  trainingRunRef: HYGIENE_RUN_REF,
  ...overrides,
})

const settlementRequest = (body: Record<string, unknown>): Request =>
  new Request('https://openagents.test/api/hygiene-lane/settlement-receipt', {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
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

const runRoute = async (
  route: Effect.Effect<Response> | undefined,
): Promise<Response> => {
  expect(route).toBeDefined()

  return Effect.runPromise(route!)
}

// A minimal Spark adapter + ledger so the real settle branch can complete.
const makeLedgerStore = () => {
  const receipts = new Map<string, unknown>()
  const intents = new Map<string, unknown>()
  const attempts = new Map<string, unknown>()
  const attemptsByIdem = new Map<string, unknown>()
  const intentsByIdem = new Map<string, unknown>()
  const events = new Map<string, unknown>()
  const notImplemented = async (): Promise<never> => {
    throw new Error('not implemented')
  }

  return {
    createPaymentAuthorityReceipt: async (record: { receiptRef: string }) => {
      receipts.set(record.receiptRef, record)
    },
    createPayoutAttempt: async (record: {
      payoutAttemptRef: string
      idempotencyKeyHash: string
    }) => {
      attempts.set(record.payoutAttemptRef, record)
      attemptsByIdem.set(record.idempotencyKeyHash, record)
    },
    createPayoutIntent: async (record: {
      payoutIntentRef: string
      idempotencyKeyHash: string
    }) => {
      intents.set(record.payoutIntentRef, record)
      intentsByIdem.set(record.idempotencyKeyHash, record)
    },
    createPayoutTargetApproval: async () => {},
    createReconciliationEvent: async (record: { eventRef: string }) => {
      events.set(record.eventRef, record)
    },
    createReleaseGate: async () => {},
    listPaymentAuthorityReceipts: async () => [...receipts.values()],
    readPaymentAuthorityReceiptByRef: async (ref: string) => receipts.get(ref),
    readPayoutAttemptByIdempotencyKeyHash: async (h: string) =>
      attemptsByIdem.get(h),
    readPayoutAttemptByRef: async (ref: string) => attempts.get(ref),
    readPayoutIntentByBuyerPaymentRef: notImplemented,
    readPayoutIntentByIdempotencyKeyHash: async (h: string) =>
      intentsByIdem.get(h),
    readPayoutIntentByRef: async (ref: string) => intents.get(ref),
    readReconciliationEventByRef: async (ref: string) => events.get(ref),
    receipts,
  }
}

const sparkAdapter = (counter: { dispatchCalls: number }) => ({
  adapterKind: 'spark_treasury' as const,
  dispatch: (input: { attempt: Record<string, unknown> }) =>
    Effect.suspend(() => {
      counter.dispatchCalls += 1

      return Effect.succeed({
        ...input.attempt,
        adapterKind: 'spark_treasury' as const,
        publicProjectionJson: JSON.stringify({
          adapter: 'spark_treasury',
          moneyMovement: 'real_bitcoin',
          state: 'dispatch_reported',
        }),
        redactedPaymentRef: 'payment.redacted.spark_treasury.test',
        status: 'dispatched' as const,
      })
    }),
  preview: (input: { intent: Record<string, unknown> }) =>
    Effect.succeed({
      adapterKind: 'spark_treasury' as const,
      amount: input.intent.amount,
      dispatchAllowed: true,
      payoutIntentRef: input.intent.payoutIntentRef,
      payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
      policySnapshotRef: input.intent.policySnapshotRef,
      spendCap: input.intent.spendCap,
    }),
  reconcile: (input: { event: Record<string, unknown> }) =>
    Effect.succeed({
      ...input.event,
      adapterKind: 'spark_treasury' as const,
      publicProjectionJson: JSON.stringify({
        adapter: 'spark_treasury',
        moneyMovement: 'real_bitcoin',
        state: 'reconciliation_matched',
      }),
      status: 'matched' as const,
    }),
})

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const makeRoutes = (store = makeInMemoryHygieneDebtReceiptStore(), opts: any = {}) => {
  const ledger = makeLedgerStore()
  const counter = { dispatchCalls: 0 }

  const routes = makeHygieneLaneSettlementRoutes<Record<string, unknown>>({
    makeDebtReceiptStore: () => store,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    makePayoutLedgerStore: () => ledger as any,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    makeSettlementPaymentAuthority: (_env, context: any) =>
      makeTreasuryPaymentAuthority({
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        adapters: [sparkAdapter(counter) as any],
        ledgerStore: context.ledgerStore,
      }),
    nowIso: () => '2026-06-18T12:05:00.000Z',
    readSettlementWalletReadiness: async () => 'ready',
    requireAdminApiToken: async () => true,
    resolveDebtReceiptProjection: (_env, ref) => store.resolveProjection(ref),
    resolveSettlementPayoutDestination: async () => 'destination.test',
    ...opts,
  })

  return { counter, ledger, routes, store }
}

describe('POST /api/hygiene-lane/debt-receipts (create payable receipt, #5335 step 1)', () => {
  it('rejects non-admin callers', async () => {
    const { routes } = makeRoutes(makeInMemoryHygieneDebtReceiptStore(), {
      requireAdminApiToken: async () => false,
    })

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(createRequest(createBody()), {}),
    )

    expect(response.status).toBe(401)
  })

  it('creates a payable receipt (201) keyed by the DebtReceiptKey', async () => {
    const { routes, store } = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(createRequest(createBody()), {}),
    )

    expect(response.status).toBe(201)
    const json = (await response.json()) as {
      debtReceipt: { debtReceiptKey: string; payableSats: number; state: string }
    }
    expect(json.debtReceipt.debtReceiptKey).toBe(DEBT_RECEIPT_KEY)
    expect(json.debtReceipt.state).toBe('payable')
    expect(json.debtReceipt.payableSats).toBe(40)

    expect((await store.read(DEBT_RECEIPT_KEY))?.state).toBe('payable')
  })

  it('is idempotent on the key (second create => 200 idempotent)', async () => {
    const { routes, store } = makeRoutes()

    const first = await runRoute(
      routes.routeHygieneLaneSettlementRequest(createRequest(createBody()), {}),
    )
    const second = await runRoute(
      routes.routeHygieneLaneSettlementRequest(createRequest(createBody()), {}),
    )

    expect(first.status).toBe(201)
    expect(second.status).toBe(200)
    const json = (await second.json()) as { debtReceipt: { idempotent: boolean } }
    expect(json.debtReceipt.idempotent).toBe(true)
    expect(store.rows.size).toBe(1)
  })

  it('refuses a non-payable input (missing settlement approval) with a typed error', async () => {
    const { routes, store } = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        createRequest(createBody({ settlementApprovalRefs: [] })),
        {},
      ),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('not_payable')
    expect(store.rows.size).toBe(0)
  })

  it('rejects malformed debt-receipt key input refs before persisting', async () => {
    const { routes, store } = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        createRequest(
          createBody({
            debtReceiptKeyInput: {
              ...KEY_INPUT,
              scopeDigest: '/Users/trigger/private-scope',
            },
          }),
        ),
        {},
      ),
    )

    expect(response.status).toBe(400)
    expect(store.rows.size).toBe(0)
  })

  it('refuses documentation or journal credit before it can become payable', async () => {
    const { routes, store } = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        createRequest(createBody({ workClass: 'documentation_or_journal' })),
        {},
      ),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as { reason: string }
    expect(json.reason).toContain('not_payable:credit_class')
    expect(store.rows.size).toBe(0)
  })

  it('rejects an unsafe ref (payment/wallet material) before persisting', async () => {
    const { routes, store } = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        createRequest(createBody({ sourceRefs: ['issue.public.5358'] })),
        {},
      ),
    )
    // A clean ref still creates; switch to an unsafe one to prove rejection.
    expect(response.status).toBe(201)
    store.rows.clear()

    const unsafe = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        createRequest(
          createBody({ verificationCommandRefs: ['raw_log.private.run'] }),
        ),
        {},
      ),
    )
    expect(unsafe.status).toBe(400)
    expect(store.rows.size).toBe(0)
  })

  it('rejects a GET (method not allowed)', async () => {
    const { routes } = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        new Request('https://openagents.test/api/hygiene-lane/debt-receipts', {
          method: 'GET',
        }),
        {},
      ),
    )

    expect(response.status).toBe(405)
  })
})

describe('create -> settle -> retire end-to-end (#5335 / #5372)', () => {
  it('a created payable receipt is settleable once (real), then a 2nd settle is duplicate_replay', async () => {
    const { counter, ledger, routes, store } = makeRoutes()

    // 1. Create the payable receipt.
    const created = await runRoute(
      routes.routeHygieneLaneSettlementRequest(createRequest(createBody()), {}),
    )
    expect(created.status).toBe(201)

    // 2. Settle it with the gate ARMED => real bitcoin moves, exactly one dispatch.
    const settle1 = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        settlementRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )
    expect(settle1.status).toBe(200)
    const settle1Json = (await settle1.json()) as {
      settlement: { moneyMovement: string; realAuthorized: boolean }
    }
    expect(settle1Json.settlement.realAuthorized).toBe(true)
    expect(settle1Json.settlement.moneyMovement).toBe('real_bitcoin')
    expect(counter.dispatchCalls).toBe(1)

    // The receipt is now retired in the durable store.
    expect((await store.read(DEBT_RECEIPT_KEY))?.state).toBe('retired')

    // 3. A SECOND settle on the same key is a duplicate replay (409), no dispatch.
    const settle2 = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        settlementRequest(settlementBody({ idempotencyRef: 'idem.hygiene.5358.retry' })),
        enabledGateEnv(),
      ),
    )
    expect(settle2.status).toBe(409)
    const settle2Json = (await settle2.json()) as { reason: string }
    expect(settle2Json.reason).toContain('duplicate_replay')
    expect(counter.dispatchCalls).toBe(1)

    // Exactly one settled receipt was written.
    expect(
      [...ledger.receipts.values()].filter(
        (receipt: unknown) =>
          (receipt as { receiptKind?: string }).receiptKind ===
          'settlement_recorded',
      ),
    ).toHaveLength(1)
  })

  it('a settle before any create fails closed (debt_receipt_not_found)', async () => {
    const { counter, routes } = makeRoutes()

    const response = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        settlementRequest(settlementBody()),
        enabledGateEnv(),
      ),
    )

    expect(response.status).toBe(404)
    expect(counter.dispatchCalls).toBe(0)
  })

  it('the simulation chain (gate OFF) does NOT retire the receipt', async () => {
    const { routes, store } = makeRoutes()

    await runRoute(
      routes.routeHygieneLaneSettlementRequest(createRequest(createBody()), {}),
    )

    // Gate OFF => honest simulation, realAuthorized:false.
    const settle = await runRoute(
      routes.routeHygieneLaneSettlementRequest(
        settlementRequest(settlementBody()),
        {},
      ),
    )
    expect(settle.status).toBe(200)
    const json = (await settle.json()) as {
      settlement: { realAuthorized: boolean }
    }
    expect(json.settlement.realAuthorized).toBe(false)

    // The receipt stays PAYABLE so the real settle can pay it once the gate arms.
    expect((await store.read(DEBT_RECEIPT_KEY))?.state).toBe('payable')
  })
})
