import { Effect } from 'effect'

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
//   2. direct BOLT 12 -> only when the recipient has a registered offer
//      AND the tips buffer (issue #4708) is configured to pay it
// The rung that served is recorded on the pay-in row and in the
// response. Until #4708 lands, the direct rung reports
// 'tips_buffer_unconfigured' and the ladder lands on credited.

export const TIP_LADDER_DEFAULT_SEND_CREDITS_BELOW_SAT = 10
export const TIP_LADDER_DEFAULT_RECEIVE_CREDITS_BELOW_SAT = 10

export type TipLadderRung = 'credited' | 'direct_bolt12'

export type TipLadderDecision =
  | Readonly<{
      kind: 'credited'
      reason:
        | 'below_send_threshold'
        | 'below_receive_threshold'
        | 'recipient_offer_missing'
        | 'tips_buffer_unconfigured'
        | 'direct_attempt_failed'
    }>
  | Readonly<{ kind: 'direct_bolt12' }>
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
    recipientHasRegisteredOffer: boolean
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

  if (!input.recipientHasRegisteredOffer) {
    return { kind: 'credited', reason: 'recipient_offer_missing' }
  }

  if (!input.tipsBufferConfigured) {
    return { kind: 'credited', reason: 'tips_buffer_unconfigured' }
  }

  return { kind: 'direct_bolt12' }
}

export type CreditedTipPlanInput = Readonly<{
  payInId: string
  fundingLegId: string
  payoutLegId: string
  senderRef: string
  recipientRef: string
  amountSat: number
  postId: string
  idempotencyKey: string
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
    contextRef: `forum.post.${input.postId}`,
    costMsat: amountMsat,
    genesisId: null,
    idempotencyKey: input.idempotencyKey,
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
  db: D1Database,
  input: Readonly<{
    amountSat: number
    senderRef: string
    recipientRef: string
    recipientHasRegisteredOffer: boolean
    recipientBolt12Offer: string | null
    payFromBuffer: BufferPayFn | null
    tipsBufferConfigured: boolean
    postId: string
    idempotencyKey: string
    makeId: () => string
    nowIso: string
  }>,
): Effect.Effect<TipLadderResult, TipLadderError> =>
  Effect.gen(function* () {
    const senderBalance = yield* Effect.promise(() =>
      readAgentBalance(db, input.senderRef),
    )
    const recipientBalance = yield* Effect.promise(() =>
      readAgentBalance(db, input.recipientRef),
    )

    const senderBalanceMsat = senderBalance?.balanceMsat ?? 0
    const decision = tipLadderDecision({
      amountSat: input.amountSat,
      recipientHasRegisteredOffer: input.recipientHasRegisteredOffer,
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
    // buffer (#4708) pays the recipient's registered offer over BOLT 12.
    // A failed direct attempt refunds atomically and falls back to the
    // credited rung - the tip still never fails.
    if (
      decision.kind === 'direct_bolt12' &&
      input.payFromBuffer !== null &&
      input.recipientBolt12Offer !== null
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
          runLedgerStatements(db, [
            ...createPayInStatements(
              {
                contextRef: `forum.post.${input.postId}`,
                costMsat: amountMsat,
                genesisId: null,
                idempotencyKey: input.idempotencyKey,
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
                    externalRef: 'forum.tip_recipient_claim',
                    kind: 'lightning',
                    legId: directPayoutLegId,
                    partyRef: input.recipientRef,
                  },
                ],
                payInId: directPayInId,
                payInType: 'tip',
                payerRef: input.senderRef,
                rung: 'direct_bolt12',
              },
              input.nowIso,
            ),
            ...markPayInForwardingStatements(directPayInId, input.nowIso),
          ]),
      })

      const payResult = yield* Effect.promise(() =>
        input.payFromBuffer!({
          amountSat: input.amountSat,
          bolt12Offer: input.recipientBolt12Offer!,
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
            runLedgerStatements(db, [
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
          rung: 'direct_bolt12' as const,
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
            runLedgerStatements(db, [
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
          rung: 'direct_bolt12' as const,
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
            db,
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
      try: () => runLedgerStatements(db, statements),
    })

    return {
      amountSat: input.amountSat,
      kind: 'tipped' as const,
      ladderReason,
      payInId,
      rung: 'credited' as const,
      senderBalanceMsatAfter: senderBalanceMsat - input.amountSat * 1000,
    }
  })

// Credited tip totals for post tipStats (the settled-vs-credited split
// the design requires): sums paid credited-tip pay-ins by post context.
export const readCreditedTipTotals = async (
  db: D1Database,
  postIds: ReadonlyArray<string>,
): Promise<ReadonlyMap<string, number>> => {
  const uniquePostIds = [...new Set(postIds)].filter(postId => postId !== '')

  if (uniquePostIds.length === 0) {
    return new Map()
  }

  const placeholders = uniquePostIds.map(() => '?').join(', ')
  const contextRefs = uniquePostIds.map(postId => `forum.post.${postId}`)

  const result = await db
    .prepare(
      `SELECT context_ref, COALESCE(SUM(cost_msat), 0) AS credited_msat
         FROM pay_ins
        WHERE pay_in_type = 'tip'
          AND rung = 'credited'
          AND state = 'paid'
          AND context_ref IN (${placeholders})
        GROUP BY context_ref`,
    )
    .bind(...contextRefs)
    .all()

  const totals = new Map<string, number>()
  for (const row of (result.results ?? []) as Array<{
    context_ref: string
    credited_msat: number
  }>) {
    const postId = row.context_ref.replace('forum.post.', '')
    totals.set(postId, Math.floor(Number(row.credited_msat) / 1000))
  }

  return totals
}
