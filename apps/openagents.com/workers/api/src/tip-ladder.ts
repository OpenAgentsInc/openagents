import { Effect } from 'effect'

import type { PaymentsLedgerDb } from './payments-ledger-db'
import { sha256Hex } from './agent-registration'
import {
  type LedgerStatement,
  type PayInPlan,
  createPayInStatements,
  markPayInFailedStatements,
  markPayInForwardingStatements,
  markPayInPaidStatements,
  readAgentBalance,
  runLedgerStatements,
} from './payments-ledger'
import type { BufferPayFn } from './tips-sweep'

// The tip receive ladder (issue #4706; design:
// docs/payments/reliable-tips.md §2). A tip never fails; only its form
// varies. Rungs, in order:
//   1. below-threshold or no reachable direct path -> credited (instant,
//      atomic balance move on the #4705 ledger)
//   2. direct Lightning -> only when the recipient has a registered destination
//      AND the tips buffer (issue #4708) is configured to pay it
// The rung that served is recorded on the pay-in row and in the
// response. Until #4708 lands, the direct rung reports
// 'tips_buffer_unconfigured' and the ladder lands on credited.

export const TIP_LADDER_DEFAULT_SEND_CREDITS_BELOW_SAT = 10
export const TIP_LADDER_DEFAULT_RECEIVE_CREDITS_BELOW_SAT = 10
export const TIP_LADDER_RECEIPT_REF_PREFIX = 'receipt.forum.tip_ladder.'
export const PYLON_TIP_LADDER_RECEIPT_REF_PREFIX = 'receipt.pylon.tip_ladder.'

const tipLadderReceiptRefPattern =
  /^receipt\.(forum|pylon)\.tip_ladder\.[A-Za-z0-9_.:-]{8,180}$/
const creditedFallbackSuffix = ':credited_fallback'

export type TipLadderRung = 'credited' | 'direct_bolt12' | 'direct_lightning'

export const tipLadderCanonicalIdempotencyKey = (
  idempotencyKey: string,
): string =>
  idempotencyKey.endsWith(creditedFallbackSuffix)
    ? idempotencyKey.slice(0, -creditedFallbackSuffix.length)
    : idempotencyKey

export const tipLadderReceiptRefFromDigest = (
  digestHex: string,
  prefix: string = TIP_LADDER_RECEIPT_REF_PREFIX,
): string => `${prefix}sha256.${digestHex.slice(0, 32)}`

export const tipLadderReceiptRefFromIdempotencyKey = async (
  idempotencyKey: string,
): Promise<string> =>
  tipLadderReceiptRefFromDigest(
    await sha256Hex(tipLadderCanonicalIdempotencyKey(idempotencyKey)),
  )

export const pylonTipLadderReceiptRefFromIdempotencyKey = async (
  idempotencyKey: string,
): Promise<string> =>
  tipLadderReceiptRefFromDigest(
    await sha256Hex(tipLadderCanonicalIdempotencyKey(idempotencyKey)),
    PYLON_TIP_LADDER_RECEIPT_REF_PREFIX,
  )

export const isTipLadderReceiptRef = (receiptRef: string): boolean =>
  tipLadderReceiptRefPattern.test(receiptRef)

export const artanisResponderTipReceiptRef = (topicId: string): string =>
  `${TIP_LADDER_RECEIPT_REF_PREFIX}artanis_responder.${topicId}`

export type TipLadderDecision =
  | Readonly<{
      kind: 'credited'
      reason:
        | 'below_send_threshold'
        | 'below_receive_threshold'
        | 'recipient_destination_missing'
        | 'tips_buffer_unconfigured'
        | 'direct_attempt_failed'
    }>
  | Readonly<{ kind: 'direct_lightning' }>
  | Readonly<{
      kind: 'refused'
      reason: 'insufficient_sender_balance' | 'self_tip' | 'invalid_amount'
    }>

export const tipLadderDecision = (
  input: Readonly<{
    amountSat: number
    senderRef: string
    recipientRef: string
    senderBalanceMsat: number
    senderSendCreditsBelowSat: number
    recipientReceiveCreditsBelowSat: number
    recipientHasPaymentDestination: boolean
    tipsBufferConfigured: boolean
  }>,
): TipLadderDecision => {
  if (!Number.isInteger(input.amountSat) || input.amountSat <= 0) {
    return { kind: 'refused', reason: 'invalid_amount' }
  }

  if (input.senderRef === input.recipientRef) {
    return { kind: 'refused', reason: 'self_tip' }
  }

  if (input.senderBalanceMsat < input.amountSat * 1000) {
    return { kind: 'refused', reason: 'insufficient_sender_balance' }
  }

  if (input.amountSat < input.senderSendCreditsBelowSat) {
    return { kind: 'credited', reason: 'below_send_threshold' }
  }

  if (input.amountSat < input.recipientReceiveCreditsBelowSat) {
    return { kind: 'credited', reason: 'below_receive_threshold' }
  }

  if (!input.recipientHasPaymentDestination) {
    return { kind: 'credited', reason: 'recipient_destination_missing' }
  }

  if (!input.tipsBufferConfigured) {
    return { kind: 'credited', reason: 'tips_buffer_unconfigured' }
  }

  return { kind: 'direct_lightning' }
}

export type CreditedTipPlanInput = Readonly<{
  payInId: string
  fundingLegId: string
  payoutLegId: string
  senderRef: string
  recipientRef: string
  amountSat: number
  postId: string
  contextRef?: string
  idempotencyKey: string
  publicReceiptRef: string | null
  ladderReason: string
}>

// The credited rung is one atomic batch: create the pay-in (debiting the
// sender under the balance CHECK constraint) and mark it paid (crediting
// the recipient and stamping resulting balances). A tip credited this
// way is settled-on-ledger the moment the batch commits.
export const creditedTipStatements = (
  input: CreditedTipPlanInput,
  nowIso: string,
): ReadonlyArray<LedgerStatement> => {
  const amountMsat = input.amountSat * 1000

  const plan: PayInPlan = {
    contextRef: input.contextRef ?? `forum.post.${input.postId}`,
    costMsat: amountMsat,
    genesisId: null,
    idempotencyKey: input.idempotencyKey,
    publicReceiptRef: input.publicReceiptRef,
    legs: [
      {
        amountMsat,
        direction: 'in',
        externalRef: null,
        kind: 'balance',
        legId: input.fundingLegId,
        partyRef: input.senderRef,
      },
      {
        amountMsat,
        direction: 'out',
        externalRef: `ladder.${input.ladderReason}`,
        kind: 'balance',
        legId: input.payoutLegId,
        partyRef: input.recipientRef,
      },
    ],
    payInId: input.payInId,
    payInType: 'tip',
    payerRef: input.senderRef,
    rung: 'credited',
  }

  return [
    ...createPayInStatements(plan, nowIso),
    ...markPayInPaidStatements(
      {
        balancePayoutLegs: [
          {
            amountMsat,
            legId: input.payoutLegId,
            partyRef: input.recipientRef,
          },
        ],
        payInId: input.payInId,
        rung: 'credited',
      },
      nowIso,
    ),
  ]
}

export type TipLadderResult =
  | Readonly<{
      kind: 'tipped'
      rung: TipLadderRung
      ladderReason: string
      payInId: string
      receiptRef: string
      amountSat: number
      senderBalanceMsatAfter: number
    }>
  | Readonly<{
      kind: 'refused'
      reason: 'insufficient_sender_balance' | 'self_tip' | 'invalid_amount'
      senderBalanceMsat: number
    }>

export class TipLadderError extends Error {
  override readonly name = 'TipLadderError'

  constructor(
    readonly reason: string,
    message: string,
  ) {
    super(message)
  }
}

export const executeTipLadder = (
  // CFG-4 (#8519): the credits domain (pay_ins/pay_in_legs/agent_balances)
  // is Cloud SQL Postgres-authoritative; every ladder read/write goes
  // through the PaymentsLedgerDb handle. There is no D1 branch and no
  // mirror for these tables anymore.
  ledgerDb: PaymentsLedgerDb,
  input: Readonly<{
    amountSat: number
    senderRef: string
    recipientRef: string
    recipientHasPaymentDestination: boolean
    recipientPaymentDestination: string | null
    payFromBuffer: BufferPayFn | null
    tipsBufferConfigured: boolean
    postId: string
    idempotencyKey: string
    publicReceiptRef: string
    makeId: () => string
    nowIso: string
    contextRef?: string
    directPayoutExternalRef?: string
  }>,
): Effect.Effect<TipLadderResult, TipLadderError> =>
  Effect.gen(function* () {
    if (!isTipLadderReceiptRef(input.publicReceiptRef)) {
      return yield* Effect.fail(
        new TipLadderError(
          'invalid_public_receipt_ref',
          'Tip ladder public receipt ref is not public-safe.',
        ),
      )
    }

    const senderBalance = yield* Effect.promise(() =>
      readAgentBalance(ledgerDb, input.senderRef),
    )
    const recipientBalance = yield* Effect.promise(() =>
      readAgentBalance(ledgerDb, input.recipientRef),
    )

    const senderBalanceMsat = senderBalance?.availableMsat ?? 0
    const decision = tipLadderDecision({
      amountSat: input.amountSat,
      recipientHasPaymentDestination: input.recipientHasPaymentDestination,
      recipientReceiveCreditsBelowSat:
        recipientBalance?.receiveCreditsBelowSat ??
        TIP_LADDER_DEFAULT_RECEIVE_CREDITS_BELOW_SAT,
      recipientRef: input.recipientRef,
      senderBalanceMsat,
      senderRef: input.senderRef,
      senderSendCreditsBelowSat:
        senderBalance?.sendCreditsBelowSat ??
        TIP_LADDER_DEFAULT_SEND_CREDITS_BELOW_SAT,
      tipsBufferConfigured: input.tipsBufferConfigured,
    })

    if (decision.kind === 'refused') {
      return {
        kind: 'refused' as const,
        reason: decision.reason,
        senderBalanceMsat,
      }
    }

    // The direct rung: the sender's balance is debited and the tips
    // buffer (#4708) pays the recipient's registered Lightning destination.
    // A failed direct attempt refunds atomically and falls back to the
    // credited rung - the tip still never fails.
    if (
      decision.kind === 'direct_lightning' &&
      input.payFromBuffer !== null &&
      input.recipientPaymentDestination !== null
    ) {
      const amountMsat = input.amountSat * 1000
      const directPayInId = input.makeId()
      const directFundingLegId = input.makeId()
      const directPayoutLegId = input.makeId()

      yield* Effect.tryPromise({
        catch: error =>
          new TipLadderError(
            'ledger_batch_failed',
            error instanceof Error ? error.message : String(error),
          ),
        try: () =>
          runLedgerStatements(ledgerDb, [
            ...createPayInStatements(
              {
                contextRef: input.contextRef ?? `forum.post.${input.postId}`,
                costMsat: amountMsat,
                genesisId: null,
                idempotencyKey: input.idempotencyKey,
                publicReceiptRef: input.publicReceiptRef,
                legs: [
                  {
                    amountMsat,
                    direction: 'in',
                    externalRef: null,
                    kind: 'balance',
                    legId: directFundingLegId,
                    partyRef: input.senderRef,
                  },
                  {
                    amountMsat,
                    direction: 'out',
                    externalRef:
                      input.directPayoutExternalRef ??
                      'forum.tip_recipient_claim',
                    kind: 'lightning',
                    legId: directPayoutLegId,
                    partyRef: input.recipientRef,
                  },
                ],
                payInId: directPayInId,
                payInType: 'tip',
                payerRef: input.senderRef,
                rung: 'direct_lightning',
              },
              input.nowIso,
            ),
            ...markPayInForwardingStatements(directPayInId, input.nowIso),
          ]),
      })

      const payResult = yield* Effect.promise(() =>
        input.payFromBuffer!({
          amountSat: input.amountSat,
          destination: input.recipientPaymentDestination!,
        }),
      )

      if (!payResult.ok && payResult.pending === true) {
        // #4710: pending holds the debit in forwarding; the
        // reconciliation pass settles or refunds-with-fallback later.
        yield* Effect.tryPromise({
          catch: error =>
            new TipLadderError(
              'ledger_batch_failed',
              error instanceof Error ? error.message : String(error),
            ),
          try: () =>
            runLedgerStatements(ledgerDb, [
              {
                params: [`pending:${payResult.paymentId}`, directPayoutLegId],
                sql: `UPDATE pay_in_legs
                      SET external_ref = external_ref || '|' || ?
                      WHERE id = ?`,
              },
            ]),
        })

        return {
          amountSat: input.amountSat,
          kind: 'tipped' as const,
          ladderReason: 'direct_forwarding',
          payInId: directPayInId,
          receiptRef: input.publicReceiptRef,
          rung: 'direct_lightning' as const,
          senderBalanceMsatAfter: senderBalanceMsat - amountMsat,
        }
      }

      if (payResult.ok) {
        yield* Effect.tryPromise({
          catch: error =>
            new TipLadderError(
              'ledger_batch_failed',
              error instanceof Error ? error.message : String(error),
            ),
          try: () =>
            runLedgerStatements(ledgerDb, [
              ...markPayInPaidStatements(
                { balancePayoutLegs: [], payInId: directPayInId },
                input.nowIso,
              ),
              {
                params: [payResult.paymentRef, directPayoutLegId],
                sql: `UPDATE pay_in_legs
                      SET external_ref = external_ref || '|' || ?
                      WHERE id = ?`,
              },
            ]),
        })

        return {
          amountSat: input.amountSat,
          kind: 'tipped' as const,
          ladderReason: 'direct_settled',
          payInId: directPayInId,
          receiptRef: input.publicReceiptRef,
          rung: 'direct_lightning' as const,
          senderBalanceMsatAfter: senderBalanceMsat - amountMsat,
        }
      }

      yield* Effect.tryPromise({
        catch: error =>
          new TipLadderError(
            'ledger_batch_failed',
            error instanceof Error ? error.message : String(error),
          ),
        try: () =>
          runLedgerStatements(
            ledgerDb,
            markPayInFailedStatements(
              {
                balanceFundingLegs: [
                  {
                    amountMsat,
                    legId: directFundingLegId,
                    partyRef: input.senderRef,
                    refundLegId: input.makeId(),
                  },
                ],
                failureReason: `direct_pay_failed:${payResult.reason}`.slice(
                  0,
                  120,
                ),
                payInId: directPayInId,
              },
              input.nowIso,
            ),
          ),
      })
    }

    const ladderReason =
      decision.kind === 'credited' ? decision.reason : 'direct_attempt_failed'

    const payInId = input.makeId()
    const statements = creditedTipStatements(
      {
        amountSat: input.amountSat,
        fundingLegId: input.makeId(),
        idempotencyKey:
          ladderReason === 'direct_attempt_failed'
            ? `${input.idempotencyKey}:credited_fallback`
            : input.idempotencyKey,
        ladderReason,
        payInId,
        payoutLegId: input.makeId(),
        postId: input.postId,
        ...(input.contextRef === undefined
          ? {}
          : { contextRef: input.contextRef }),
        publicReceiptRef: input.publicReceiptRef,
        recipientRef: input.recipientRef,
        senderRef: input.senderRef,
      },
      input.nowIso,
    )

    yield* Effect.tryPromise({
      catch: error =>
        new TipLadderError(
          'ledger_batch_failed',
          error instanceof Error ? error.message : String(error),
        ),
      try: () => runLedgerStatements(ledgerDb, statements),
    })

    return {
      amountSat: input.amountSat,
      kind: 'tipped' as const,
      ladderReason,
      payInId,
      receiptRef: input.publicReceiptRef,
      rung: 'credited' as const,
      senderBalanceMsatAfter: senderBalanceMsat - input.amountSat * 1000,
    }
  })

// Credited tip totals for post tipStats (the settled-vs-credited split
// the design requires): sums paid credited-tip pay-ins by post context.
export const readCreditedTipTotals = async (
  ledgerDb: PaymentsLedgerDb,
  postIds: ReadonlyArray<string>,
): Promise<ReadonlyMap<string, number>> => {
  const uniquePostIds = [...new Set(postIds)].filter(postId => postId !== '')

  if (uniquePostIds.length === 0) {
    return new Map()
  }

  const placeholders = uniquePostIds.map(() => '?').join(', ')
  const contextRefs = uniquePostIds.map(postId => `forum.post.${postId}`)

  const rows = await ledgerDb.query(
    `SELECT context_ref, COALESCE(SUM(cost_msat), 0) AS credited_msat
       FROM pay_ins
      WHERE pay_in_type = 'tip'
        AND rung = 'credited'
        AND state = 'paid'
        AND context_ref IN (${placeholders})
      GROUP BY context_ref`,
    contextRefs,
  )

  const totals = new Map<string, number>()
  for (const row of rows as Array<{
    context_ref: string
    // Postgres returns bigint SUMs as strings; Number(...) below decodes.
    credited_msat: number | string
  }>) {
    const postId = row.context_ref.replace('forum.post.', '')
    totals.set(postId, Math.floor(Number(row.credited_msat) / 1000))
  }

  return totals
}
