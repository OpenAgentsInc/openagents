import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonUnknown } from '../json-boundary'
import { liveAtReadStaleness } from '../public-projection-staleness'
import { ForumStorageError } from './repository'
import {
  type ForumActorSummary,
  ForumActorSummary as ForumActorSummarySchema,
  type ForumCreatorEarning,
  type ForumCreatorEarningPaymentState,
  ForumCreatorEarning as ForumCreatorEarningSchema,
  type ForumCreatorEarningsResponse,
  ForumCreatorEarningsResponse as ForumCreatorEarningsResponseSchema,
  type ForumCreatorEarningsSummary,
  type ForumMoneyAmount,
  type ForumPaidActionKind,
  type ForumPaymentEventProjection,
  ForumPaymentEventProjection as ForumPaymentEventProjectionSchema,
  type ForumTipLeaderboardCreator,
  ForumTipLeaderboardCreator as ForumTipLeaderboardCreatorSchema,
  type ForumTipLeaderboardPost,
  ForumTipLeaderboardPost as ForumTipLeaderboardPostSchema,
  type ForumTipLeaderboardsResponse,
  ForumTipLeaderboardsResponse as ForumTipLeaderboardsResponseSchema,
  type ForumTipReconciliationResponse,
  ForumTipReconciliationResponse as ForumTipReconciliationResponseSchema,
  type ForumTipSettlementClaimProjection,
  ForumTipSettlementClaimProjection as ForumTipSettlementClaimProjectionSchema,
  type ForumTipSettlementState,
} from './schemas'
import {
  forumTipSettlementProjectionForReceipt,
  forumTipSettlementProjectionForState,
} from './tip-settlement'

type ForumTipEarningRow = Readonly<{
  action_kind: ForumPaidActionKind
  amount_asset: 'credits' | 'sats' | 'usd'
  amount_value: number
  earning_actor_ref: string
  money_action_created_at: string
  money_action_id: string
  payment_event_id: string | null
  payment_event_projection_json: string | null
  receipt_ref: string
  recipient_actor_ref: string | null
  settlement_claim_projection_json: string | null
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
}>

type TipLadderEarningRow = Readonly<{
  cost_msat: number
  created_at: string
  // Cumulative credited msat for this recipient through this row
  // (oldest-credited-first), used for the swept-coverage convention.
  credited_through_msat: number | null
  pay_in_id: string
  public_receipt_ref: string
  payer_ref: string
  recipient_actor_ref: string
  // Total settled sweep payout msat for this recipient.
  recipient_swept_msat: number | null
  rung: 'credited' | 'direct_bolt12' | 'direct_lightning' | null
  state_changed_at: string
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
}>

const tipLadderRungIsDirectWallet = (
  rung: TipLadderEarningRow['rung'],
): boolean => rung === 'direct_bolt12' || rung === 'direct_lightning'

type CountRow = Readonly<{ count: number | null }>

type ForumTipLeaderboardPostRow = Readonly<{
  actor_json: string
  post_id: string
  post_subject: string | null
  tip_count: number | null
  topic_id: string
  total_paid_sats: number | null
  total_settled_sats: number | null
}>

type ForumTipLeaderboardCreatorRow = Readonly<{
  actor_json: string
  earning_actor_ref: string
  tip_count: number | null
  total_paid_sats: number | null
  total_settled_sats: number | null
}>

type ForumTipLadderCreatorTotals = Readonly<{
  totalCreditedSats: number
  totalSweptSats: number
}>

const zeroLadderCreatorTotals: ForumTipLadderCreatorTotals = {
  totalCreditedSats: 0,
  totalSweptSats: 0,
}

type ForumTipEarningsRuntime = Readonly<{
  nowIso: () => string
}>

/**
 * Declared staleness contract for the tip read surfaces (epic #4751,
 * the #4753 remainder): every payload composes live at read from the
 * receipt-backed money actions and the tip-ladder pay-in ledger, so
 * the bound is zero and the rebuild set names the write transitions.
 */
export const FORUM_TIP_PROJECTION_STALENESS = liveAtReadStaleness([
  'forum_payment_event_confirmed',
  'forum_tip_settlement_claimed',
  'tip_ladder_pay_in_paid',
  'tip_sweep_settled',
])

/**
 * Leaderboard honesty caveats: ranking still keys on settled
 * receipt-backed tips; ladder credited/swept sats are listed for
 * ranked creators but do not admit creators or posts by themselves.
 */
export const FORUM_TIP_LEADERBOARD_CAVEAT_REFS: ReadonlyArray<string> = [
  'caveat.public.forum_tip.leaderboards_rank_by_settled_receipt_tips_only',
  'caveat.public.forum_tip.ladder_credited_swept_sats_listed_for_ranked_creators_only',
]

const decodeActorSummary = S.decodeUnknownSync(ForumActorSummarySchema)
const decodeCreatorEarning = S.decodeUnknownSync(ForumCreatorEarningSchema)
const decodeCreatorEarningsResponse = S.decodeUnknownSync(
  ForumCreatorEarningsResponseSchema,
)
const decodeTipLeaderboardCreator = S.decodeUnknownSync(
  ForumTipLeaderboardCreatorSchema,
)
const decodeTipLeaderboardPost = S.decodeUnknownSync(
  ForumTipLeaderboardPostSchema,
)
const decodeTipLeaderboardsResponse = S.decodeUnknownSync(
  ForumTipLeaderboardsResponseSchema,
)
const decodeTipReconciliationResponse = S.decodeUnknownSync(
  ForumTipReconciliationResponseSchema,
)
const decodePaymentEventProjection = S.decodeUnknownSync(
  ForumPaymentEventProjectionSchema,
)
const decodeSettlementClaimProjection = S.decodeUnknownSync(
  ForumTipSettlementClaimProjectionSchema,
)

const privateMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|balance[._-]?sats|bearer|bolt11|bolt12|channel[_-]?monitor|checkout[_-]?secret|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof=|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|backup|balance|channel|invoice|liquidity|payment|payload|payout|target|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|state))/i

const forumTopicPublicUrl = (topicId: string): string =>
  `https://openagents.com/forum/t/${encodeURIComponent(topicId)}`

const forumPostPublicUrl = (topicId: string, postId: string): string =>
  `${forumTopicPublicUrl(topicId)}#post-${encodeURIComponent(postId)}`

const targetPostPermalink = (row: ForumTipEarningRow): string | null =>
  row.target_topic_id === null || row.target_post_id === null
    ? null
    : forumPostPublicUrl(row.target_topic_id, row.target_post_id)

const paymentEventFromRow = (
  row: ForumTipEarningRow,
): ForumPaymentEventProjection | null =>
  row.payment_event_projection_json === null
    ? null
    : decodePaymentEventProjection(
        parseJsonUnknown(row.payment_event_projection_json),
      )

const settlementClaimFromRow = (
  row: ForumTipEarningRow,
): ForumTipSettlementClaimProjection | null =>
  row.settlement_claim_projection_json == null
    ? null
    : decodeSettlementClaimProjection(
        parseJsonUnknown(row.settlement_claim_projection_json),
      )

const paymentStateForEvent = (
  paymentEvent: ForumPaymentEventProjection | null,
): ForumCreatorEarningPaymentState =>
  paymentEvent === null ? 'unverified' : paymentEvent.status

const rowAmount = (row: ForumTipEarningRow): ForumMoneyAmount => ({
  amount: row.amount_value,
  asset: row.amount_asset,
})

const actorFromJson = (value: string): ForumActorSummary =>
  decodeActorSummary(parseJsonUnknown(value))

const earningFromRow = (row: ForumTipEarningRow): ForumCreatorEarning => {
  const paymentEvent = paymentEventFromRow(row)
  const tipSettlement = forumTipSettlementProjectionForReceipt(
    paymentEvent,
    settlementClaimFromRow(row),
  )

  return decodeCreatorEarning({
    acceptedWorkPayoutEvidence: false,
    actionKind: row.action_kind,
    amount: rowAmount(row),
    createdAt: row.money_action_created_at,
    creatorReceivedSpendableValue: tipSettlement.creatorReceivedSpendableValue,
    earningActorRef: row.earning_actor_ref,
    earningRef: `earning.forum_tip.${row.money_action_id}`,
    moneyActionRef: `forum_money_action:${row.money_action_id}`,
    paymentEventRef: row.payment_event_id,
    paymentState: paymentStateForEvent(paymentEvent),
    receiptRef: row.receipt_ref,
    recipientActorRef: row.recipient_actor_ref,
    settlementState: tipSettlement.state,
    target: {
      forumId: row.target_forum_id,
      postId: row.target_post_id,
      topicId: row.target_topic_id,
    },
    targetPostPermalink: targetPostPermalink(row),
    tipSettlement,
  })
}

const tipLadderTargetPostPermalink = (
  row: TipLadderEarningRow,
): string | null =>
  row.target_topic_id === null || row.target_post_id === null
    ? null
    : forumPostPublicUrl(row.target_topic_id, row.target_post_id)

const tipLadderPaymentEventFromRow = (
  row: TipLadderEarningRow,
): ForumPaymentEventProjection =>
  decodePaymentEventProjection({
    actionKind: 'post_reward',
    amount: {
      amount: Math.max(0, Math.floor(Number(row.cost_msat) / 1000)),
      asset: 'sats',
    },
    challengeId: row.pay_in_id,
    createdAt: row.state_changed_at,
    externalRef:
      tipLadderRungIsDirectWallet(row.rung)
        ? `payment.forum.tip_ladder.${row.pay_in_id}`
        : `ledger.forum.tip_ladder.${row.pay_in_id}`,
    payerActorRef: row.payer_ref,
    paymentEventRef: `payment_event.forum.tip_ladder.${row.pay_in_id}`,
    paymentMode: 'live',
    providerRef: 'provider.openagents.tip_ladder',
    receiptRef: row.public_receipt_ref,
    recipientActorRef: row.recipient_actor_ref,
    redactedEvidenceRef: `evidence.forum.tip_ladder.${row.pay_in_id}`,
    settlementAuthority:
      tipLadderRungIsDirectWallet(row.rung)
        ? 'recipient_wallet_direct'
        : 'openagents_ledger_credited',
    status: 'confirmed',
  })

// Swept coverage (#4753): a credited tip counts as swept once settled
// sweep payouts to the recipient's registered receive code cover its
// cumulative credited value, oldest-credited-first. The ledger amount
// is fungible, so the attribution order is a documented projection
// convention rather than a per-sat trace.
export const tipLadderCreditedTipIsSwept = (
  input: Readonly<{
    creditedThroughMsat: number
    recipientSweptMsat: number
  }>,
): boolean =>
  input.creditedThroughMsat > 0 &&
  input.recipientSweptMsat >= input.creditedThroughMsat

const tipLadderSettlementForRow = (
  row: TipLadderEarningRow,
  paymentEvent: ForumPaymentEventProjection,
) =>
  row.rung === 'credited' &&
  tipLadderCreditedTipIsSwept({
    creditedThroughMsat: Math.max(0, Number(row.credited_through_msat ?? 0)),
    recipientSweptMsat: Math.max(0, Number(row.recipient_swept_msat ?? 0)),
  })
    ? forumTipSettlementProjectionForState('swept')
    : forumTipSettlementProjectionForReceipt(paymentEvent, null)

const tipLadderEarningFromRow = (
  row: TipLadderEarningRow,
): ForumCreatorEarning => {
  const paymentEvent = tipLadderPaymentEventFromRow(row)
  const tipSettlement = tipLadderSettlementForRow(row, paymentEvent)

  return decodeCreatorEarning({
    acceptedWorkPayoutEvidence: false,
    actionKind: 'post_reward',
    amount: {
      amount: Math.max(0, Math.floor(Number(row.cost_msat) / 1000)),
      asset: 'sats',
    },
    createdAt: row.created_at,
    creatorReceivedSpendableValue: tipSettlement.creatorReceivedSpendableValue,
    earningActorRef: row.recipient_actor_ref,
    earningRef: `earning.forum_tip_ladder.${row.pay_in_id}`,
    moneyActionRef: `pay_in:${row.pay_in_id}`,
    paymentEventRef: paymentEvent.paymentEventRef,
    paymentState: paymentStateForEvent(paymentEvent),
    receiptRef: row.public_receipt_ref,
    recipientActorRef: row.recipient_actor_ref,
    settlementState: tipSettlement.state,
    target: {
      forumId: row.target_forum_id,
      postId: row.target_post_id,
      topicId: row.target_topic_id,
    },
    targetPostPermalink: tipLadderTargetPostPermalink(row),
    tipSettlement,
  })
}

const summarizeEarnings = (
  earnings: ReadonlyArray<ForumCreatorEarning>,
): ForumCreatorEarningsSummary => {
  const satsTotalForState = (state: ForumTipSettlementState): number =>
    earnings
      .filter(earning => earning.amount.asset === 'sats')
      .filter(earning => earning.settlementState === state)
      .reduce((sum, earning) => sum + earning.amount.amount, 0)
  const totalPaidSats = earnings
    .filter(earning => earning.amount.asset === 'sats')
    .filter(earning => earning.paymentState === 'confirmed')
    .reduce((sum, earning) => sum + earning.amount.amount, 0)

  return {
    creditedCount: earnings.filter(
      earning => earning.settlementState === 'credited',
    ).length,
    failedCount: earnings.filter(
      earning => earning.settlementState === 'failed',
    ).length,
    paidCount: earnings.filter(earning => earning.settlementState === 'paid')
      .length,
    pendingCount: earnings.filter(earning =>
      ['evidence_only', 'payment_required', 'recipient_pending'].includes(
        earning.settlementState,
      ),
    ).length,
    refundedCount: earnings.filter(
      earning => earning.settlementState === 'refunded',
    ).length,
    reversedCount: earnings.filter(
      earning => earning.settlementState === 'reversed',
    ).length,
    settledCount: earnings.filter(
      earning => earning.settlementState === 'settled',
    ).length,
    sweptCount: earnings.filter(
      earning => earning.settlementState === 'swept',
    ).length,
    totalCount: earnings.length,
    totalCreditedSats: satsTotalForState('credited'),
    totalPaidSats,
    totalSettledSats: satsTotalForState('settled'),
    totalSweptSats: satsTotalForState('swept'),
  }
}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ForumStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new ForumStorageError({
        operation,
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: run,
  })

const readEarningRows = (
  db: D1Database,
  input: Readonly<{ actorRef: string | null; limit: number }>,
): Effect.Effect<ReadonlyArray<ForumTipEarningRow>, ForumStorageError> => {
  const baseQuery = `SELECT ma.id AS money_action_id,
                           ma.action_kind AS action_kind,
                           ma.target_forum_id AS target_forum_id,
                           ma.target_topic_id AS target_topic_id,
                           ma.target_post_id AS target_post_id,
                           ma.amount_asset AS amount_asset,
                           ma.amount_value AS amount_value,
                           ma.payment_event_id AS payment_event_id,
                           ma.earning_actor_ref AS earning_actor_ref,
                           ma.created_at AS money_action_created_at,
                           r.receipt_ref AS receipt_ref,
                           r.recipient_actor_ref AS recipient_actor_ref,
                           pe.public_projection_json AS payment_event_projection_json,
                           sc.public_projection_json AS settlement_claim_projection_json
                      FROM forum_money_actions ma
                      JOIN forum_receipts r
                        ON r.id = ma.receipt_id
                       AND r.archived_at IS NULL
                 LEFT JOIN forum_payment_events pe
                        ON pe.id = ma.payment_event_id
                       AND pe.archived_at IS NULL
                 LEFT JOIN forum_tip_settlement_claims sc
                        ON sc.receipt_id = r.id
                       AND sc.archived_at IS NULL
                     WHERE ma.action_kind = 'post_reward'
                       AND ma.earning_actor_ref IS NOT NULL
                       AND json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                       AND ma.archived_at IS NULL`
  const scopedQuery =
    input.actorRef === null
      ? `${baseQuery}
                  ORDER BY ma.created_at DESC, ma.id DESC
                     LIMIT ?`
      : `${baseQuery}
                       AND ma.earning_actor_ref = ?
                  ORDER BY ma.created_at DESC, ma.id DESC
                     LIMIT ?`

  return d1Effect('forumTipEarnings.readRows', () =>
    (input.actorRef === null
      ? db.prepare(scopedQuery).bind(input.limit)
      : db.prepare(scopedQuery).bind(input.actorRef, input.limit)
    ).all<ForumTipEarningRow>(),
  ).pipe(Effect.map(result => result.results ?? []))
}

const countEarningRows = (
  db: D1Database,
  actorRef: string | null,
): Effect.Effect<number, ForumStorageError> => {
  const baseQuery = `SELECT COUNT(*) AS count
                      FROM forum_money_actions ma
                      JOIN forum_receipts r
                        ON r.id = ma.receipt_id
                       AND r.archived_at IS NULL
                      JOIN forum_payment_events pe
                        ON pe.id = ma.payment_event_id
                       AND pe.archived_at IS NULL
                     WHERE ma.action_kind = 'post_reward'
                       AND ma.earning_actor_ref IS NOT NULL
                       AND json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                       AND ma.archived_at IS NULL`
  const scopedQuery =
    actorRef === null ? baseQuery : `${baseQuery} AND ma.earning_actor_ref = ?`

  return d1Effect('forumTipEarnings.countRows', () =>
    (actorRef === null
      ? db.prepare(scopedQuery)
      : db.prepare(scopedQuery).bind(actorRef)
    ).first<CountRow>(),
  ).pipe(Effect.map(row => Math.max(0, Number(row?.count ?? 0))))
}

const readTipLadderEarningRows = (
  db: D1Database,
  input: Readonly<{ actorRef: string | null; limit: number }>,
): Effect.Effect<ReadonlyArray<TipLadderEarningRow>, ForumStorageError> => {
  // Every paid ladder tip projects a receipt ref the recipient can cite
  // (#4753): rows written before the public-receipt column (or by the
  // credited reconciliation fallback) get the deterministic
  // receipt-equivalent ref 'receipt.forum.tip_ladder.payin.<payInId>'.
  const baseQuery = `SELECT p.id AS pay_in_id,
                           COALESCE(
                             p.public_receipt_ref,
                             'receipt.forum.tip_ladder.payin.' || p.id
                           ) AS public_receipt_ref,
                           p.payer_ref AS payer_ref,
                           p.cost_msat AS cost_msat,
                           p.rung AS rung,
                           p.created_at AS created_at,
                           p.state_changed_at AS state_changed_at,
                           payout.party_ref AS recipient_actor_ref,
                           CASE WHEN p.rung = 'credited' THEN (
                             SELECT COALESCE(SUM(p2.cost_msat), 0)
                               FROM pay_ins p2
                               JOIN pay_in_legs payout2
                                 ON payout2.pay_in_id = p2.id
                                AND payout2.direction = 'out'
                                AND payout2.party_ref = payout.party_ref
                              WHERE p2.pay_in_type = 'tip'
                                AND p2.rung = 'credited'
                                AND p2.state = 'paid'
                                AND p2.context_ref LIKE 'forum.post.%'
                                AND (p2.created_at < p.created_at
                                     OR (p2.created_at = p.created_at
                                         AND p2.id <= p.id))
                           ) ELSE 0 END AS credited_through_msat,
                           (SELECT COALESCE(SUM(s.cost_msat), 0)
                              FROM pay_ins s
                             WHERE s.pay_in_type = 'sweep'
                               AND s.state = 'paid'
                               AND s.payer_ref = payout.party_ref
                           ) AS recipient_swept_msat,
                           forum_posts.id AS target_post_id,
                           forum_posts.topic_id AS target_topic_id,
                           forum_posts.forum_id AS target_forum_id
                      FROM pay_ins p
                      JOIN pay_in_legs payout
                        ON payout.pay_in_id = p.id
                       AND payout.direction = 'out'
                 LEFT JOIN forum_posts
                        ON forum_posts.id = substr(
                             p.context_ref,
                             length('forum.post.') + 1
                           )
                       AND forum_posts.archived_at IS NULL
                     WHERE p.pay_in_type = 'tip'
                       AND p.state = 'paid'
                       AND p.context_ref LIKE 'forum.post.%'
                       AND payout.party_ref IS NOT NULL`
  const scopedQuery =
    input.actorRef === null
      ? `${baseQuery}
                  ORDER BY p.created_at DESC, p.id DESC
                     LIMIT ?`
      : `${baseQuery}
                       AND payout.party_ref = ?
                  ORDER BY p.created_at DESC, p.id DESC
                     LIMIT ?`

  return d1Effect('forumTipEarnings.readTipLadderRows', () =>
    (input.actorRef === null
      ? db.prepare(scopedQuery).bind(input.limit)
      : db.prepare(scopedQuery).bind(input.actorRef, input.limit)
    ).all<TipLadderEarningRow>(),
  ).pipe(Effect.map(result => result.results ?? []))
}

const countTipLadderEarningRows = (
  db: D1Database,
  actorRef: string | null,
): Effect.Effect<number, ForumStorageError> => {
  const baseQuery = `SELECT COUNT(*) AS count
                      FROM pay_ins p
                      JOIN pay_in_legs payout
                        ON payout.pay_in_id = p.id
                       AND payout.direction = 'out'
                     WHERE p.pay_in_type = 'tip'
                       AND p.state = 'paid'
                       AND p.context_ref LIKE 'forum.post.%'
                       AND payout.party_ref IS NOT NULL`
  const scopedQuery =
    actorRef === null ? baseQuery : `${baseQuery} AND payout.party_ref = ?`

  return d1Effect('forumTipEarnings.countTipLadderRows', () =>
    (actorRef === null
      ? db.prepare(scopedQuery)
      : db.prepare(scopedQuery).bind(actorRef)
    ).first<CountRow>(),
  ).pipe(Effect.map(row => Math.max(0, Number(row?.count ?? 0))))
}

const sortEarningsNewestFirst = (
  earnings: ReadonlyArray<ForumCreatorEarning>,
): ReadonlyArray<ForumCreatorEarning> =>
  [...earnings].sort((left, right) => {
    const byCreatedAt = right.createdAt.localeCompare(left.createdAt)
    return byCreatedAt === 0
      ? right.earningRef.localeCompare(left.earningRef)
      : byCreatedAt
  })

const publicTextIsUnsafe = (value: string): boolean =>
  containsProviderSecretMaterial(value) || privateMaterialPattern.test(value)

export const safeLeaderboardPostTitle = (
  subject: string | null,
): string | null => {
  if (subject === null || subject.trim() === '') {
    return null
  }

  return publicTextIsUnsafe(subject) ? null : subject
}

export const safeActorSummary = (
  actor: ForumActorSummary,
): ForumActorSummary =>
  publicTextIsUnsafe(actor.displayName)
    ? {
        ...actor,
        displayName: publicTextIsUnsafe(actor.slug) ? 'agent' : actor.slug,
      }
    : actor

const postLeaderboardFromRow = (
  row: ForumTipLeaderboardPostRow,
): ForumTipLeaderboardPost =>
  decodeTipLeaderboardPost({
    author: safeActorSummary(actorFromJson(row.actor_json)),
    postId: row.post_id,
    postPermalink: forumPostPublicUrl(row.topic_id, row.post_id),
    postTitle: safeLeaderboardPostTitle(row.post_subject ?? null),
    tipCount: Math.max(0, Number(row.tip_count ?? 0)),
    topicId: row.topic_id,
    totalPaidSats: Math.max(0, Number(row.total_paid_sats ?? 0)),
    totalSettledSats: Math.max(0, Number(row.total_settled_sats ?? 0)),
  })

const creatorLeaderboardFromRow = (
  row: ForumTipLeaderboardCreatorRow,
  ladderTotals: ForumTipLadderCreatorTotals,
): ForumTipLeaderboardCreator =>
  decodeTipLeaderboardCreator({
    actor: safeActorSummary(actorFromJson(row.actor_json)),
    tipCount: Math.max(0, Number(row.tip_count ?? 0)),
    totalCreditedSats: ladderTotals.totalCreditedSats,
    totalPaidSats: Math.max(0, Number(row.total_paid_sats ?? 0)),
    totalSettledSats: Math.max(0, Number(row.total_settled_sats ?? 0)),
    totalSweptSats: ladderTotals.totalSweptSats,
  })

const readPostLeaderboardRows = (
  db: D1Database,
  limit: number,
): Effect.Effect<
  ReadonlyArray<ForumTipLeaderboardPostRow>,
  ForumStorageError
> =>
  d1Effect('forumTipLeaderboards.readPosts', () =>
    db
      .prepare(
        `SELECT ma.target_post_id AS post_id,
                ma.target_topic_id AS topic_id,
                p.actor_json AS actor_json,
                t.title AS post_subject,
                COUNT(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                  THEN 1
                END) AS tip_count,
                COALESCE(SUM(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                  THEN ma.amount_value
                  ELSE 0
                END), 0) AS total_paid_sats,
                COALESCE(SUM(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                   AND json_extract(pe.public_projection_json, '$.settlementAuthority') = 'recipient_wallet_direct'
                  THEN ma.amount_value
                  ELSE 0
                END), 0) AS total_settled_sats
           FROM forum_money_actions ma
           JOIN forum_posts p
             ON p.id = ma.target_post_id
            AND p.archived_at IS NULL
            AND p.state IN ('visible', 'edited')
           JOIN forum_topics t
             ON t.id = ma.target_topic_id
            AND t.archived_at IS NULL
           JOIN forum_receipts r
             ON r.id = ma.receipt_id
            AND r.archived_at IS NULL
      LEFT JOIN forum_payment_events pe
             ON pe.id = ma.payment_event_id
            AND pe.archived_at IS NULL
      LEFT JOIN forum_tip_settlement_claims sc
             ON sc.receipt_id = r.id
            AND sc.archived_at IS NULL
          WHERE ma.action_kind = 'post_reward'
            AND ma.amount_asset = 'sats'
            AND ma.target_post_id IS NOT NULL
            AND ma.target_topic_id IS NOT NULL
            AND ma.archived_at IS NULL
          GROUP BY ma.target_post_id, ma.target_topic_id, p.actor_json, t.title
         HAVING total_settled_sats > 0
          ORDER BY total_settled_sats DESC, tip_count DESC, ma.target_post_id ASC
          LIMIT ?`,
      )
      .bind(limit)
      .all<ForumTipLeaderboardPostRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

const readCreatorLeaderboardRows = (
  db: D1Database,
  limit: number,
): Effect.Effect<
  ReadonlyArray<ForumTipLeaderboardCreatorRow>,
  ForumStorageError
> =>
  d1Effect('forumTipLeaderboards.readCreators', () =>
    db
      .prepare(
        `SELECT p.actor_json AS actor_json,
                ma.earning_actor_ref AS earning_actor_ref,
                COUNT(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                  THEN 1
                END) AS tip_count,
                COALESCE(SUM(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                  THEN ma.amount_value
                  ELSE 0
                END), 0) AS total_paid_sats,
                COALESCE(SUM(CASE
                  WHEN json_extract(pe.public_projection_json, '$.status') = 'confirmed'
                   AND json_extract(pe.public_projection_json, '$.settlementAuthority') = 'recipient_wallet_direct'
                  THEN ma.amount_value
                  ELSE 0
                END), 0) AS total_settled_sats
           FROM forum_money_actions ma
           JOIN forum_posts p
             ON p.id = ma.target_post_id
            AND p.archived_at IS NULL
            AND p.state IN ('visible', 'edited')
           JOIN forum_receipts r
             ON r.id = ma.receipt_id
            AND r.archived_at IS NULL
      LEFT JOIN forum_payment_events pe
             ON pe.id = ma.payment_event_id
            AND pe.archived_at IS NULL
      LEFT JOIN forum_tip_settlement_claims sc
             ON sc.receipt_id = r.id
            AND sc.archived_at IS NULL
          WHERE ma.action_kind = 'post_reward'
            AND ma.amount_asset = 'sats'
            AND ma.earning_actor_ref IS NOT NULL
            AND ma.archived_at IS NULL
          GROUP BY ma.earning_actor_ref, p.actor_json
         HAVING total_settled_sats > 0
          ORDER BY total_settled_sats DESC, tip_count DESC, ma.earning_actor_ref ASC
          LIMIT ?`,
      )
      .bind(limit)
      .all<ForumTipLeaderboardCreatorRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

type ForumTipLadderCreatorTotalsRow = Readonly<{
  credited_msat: number | null
  recipient_actor_ref: string
  swept_msat: number | null
}>

// Ladder credited/swept sats for creators already ranked on the
// settled leaderboard (#4751, the #4753 remainder). Swept coverage
// follows the documented oldest-credited-first convention: settled
// sweep payouts cover credited value in aggregate, so the swept
// portion is min(sweptMsat, creditedMsat) and the rest stays credited.
const readLadderCreatorTotals = (
  db: D1Database,
  actorRefs: ReadonlyArray<string>,
): Effect.Effect<
  ReadonlyMap<string, ForumTipLadderCreatorTotals>,
  ForumStorageError
> => {
  const uniqueActorRefs = [...new Set(actorRefs)].filter(ref => ref !== '')

  if (uniqueActorRefs.length === 0) {
    return Effect.succeed(new Map())
  }

  const placeholders = uniqueActorRefs.map(() => '?').join(', ')

  return d1Effect('forumTipLeaderboards.readLadderCreatorTotals', () =>
    db
      .prepare(
        `SELECT payout.party_ref AS recipient_actor_ref,
                COALESCE(SUM(CASE
                  WHEN p.rung = 'credited' THEN p.cost_msat
                  ELSE 0
                END), 0) AS credited_msat,
                (SELECT COALESCE(SUM(s.cost_msat), 0)
                   FROM pay_ins s
                  WHERE s.pay_in_type = 'sweep'
                    AND s.state = 'paid'
                    AND s.payer_ref = payout.party_ref) AS swept_msat
           FROM pay_ins p
           JOIN pay_in_legs payout
             ON payout.pay_in_id = p.id
            AND payout.direction = 'out'
          WHERE p.pay_in_type = 'tip'
            AND p.state = 'paid'
            AND p.context_ref LIKE 'forum.post.%'
            AND payout.party_ref IN (${placeholders})
          GROUP BY payout.party_ref`,
      )
      .bind(...uniqueActorRefs)
      .all<ForumTipLadderCreatorTotalsRow>(),
  ).pipe(
    Effect.map(
      result =>
        new Map(
          (result.results ?? []).map(row => {
            const creditedMsat = Math.max(0, Number(row.credited_msat ?? 0))
            const sweptCoverageMsat = Math.min(
              creditedMsat,
              Math.max(0, Number(row.swept_msat ?? 0)),
            )

            return [
              row.recipient_actor_ref,
              {
                totalCreditedSats: Math.floor(
                  (creditedMsat - sweptCoverageMsat) / 1000,
                ),
                totalSweptSats: Math.floor(sweptCoverageMsat / 1000),
              },
            ] as const
          }),
        ),
    ),
  )
}

// Arbitrary public content (titles, names, slugs) is sanitized per-field at
// row construction; the projection-wide throwing scan below must only fire
// for structural fields (refs, ids, evidence), where unsafe content means a
// real leak rather than a user who typed "@" or "mnemonic". Stripping these
// keys from the probe keeps user content from 500ing whole endpoints.
const arbitraryPublicContentKeys = new Set(['displayName', 'postTitle', 'slug'])

const structuralProjectionProbe = (value: unknown): string =>
  JSON.stringify(value, (key, probed: unknown) =>
    arbitraryPublicContentKeys.has(key) ? undefined : probed,
  )

const assertProjectionSafe = (
  value:
    | ForumCreatorEarningsResponse
    | ForumTipLeaderboardsResponse
    | ForumTipReconciliationResponse,
): void => {
  const json = structuralProjectionProbe(value)

  if (
    containsProviderSecretMaterial(json) ||
    privateMaterialPattern.test(json)
  ) {
    throw new Error('Forum tip earnings projection contains private material.')
  }
}

export const forumTipEarningsProjectionHasPrivateMaterial = (
  value:
    | ForumCreatorEarningsResponse
    | ForumTipLeaderboardsResponse
    | ForumTipReconciliationResponse,
): boolean => {
  const json = structuralProjectionProbe(value)

  return (
    containsProviderSecretMaterial(json) || privateMaterialPattern.test(json)
  )
}

export const readForumTipLeaderboards = (
  db: D1Database,
  input: Readonly<{ limit?: number }>,
  runtime: ForumTipEarningsRuntime,
): Effect.Effect<ForumTipLeaderboardsResponse, ForumStorageError> =>
  Effect.gen(function* () {
    const limit = Math.min(Math.max(input.limit ?? 50, 1), 100)
    const [postRows, creatorRows] = yield* Effect.all([
      readPostLeaderboardRows(db, limit),
      readCreatorLeaderboardRows(db, limit),
    ])
    const ladderTotals = yield* readLadderCreatorTotals(
      db,
      creatorRows.map(row => row.earning_actor_ref),
    )
    const projection = decodeTipLeaderboardsResponse({
      caveatRefs: FORUM_TIP_LEADERBOARD_CAVEAT_REFS,
      creators: creatorRows.map(row =>
        creatorLeaderboardFromRow(
          row,
          ladderTotals.get(row.earning_actor_ref) ?? zeroLadderCreatorTotals,
        ),
      ),
      generatedAt: runtime.nowIso(),
      posts: postRows.map(postLeaderboardFromRow),
      staleness: FORUM_TIP_PROJECTION_STALENESS,
    })

    assertProjectionSafe(projection)

    return projection
  })

export const readForumCreatorEarnings = (
  db: D1Database,
  input: Readonly<{ actorRef: string; limit?: number }>,
  runtime: ForumTipEarningsRuntime,
): Effect.Effect<ForumCreatorEarningsResponse, ForumStorageError> =>
  Effect.gen(function* () {
    const limit = Math.min(Math.max(input.limit ?? 50, 1), 100)
    const [rows, ladderRows, receiptCount, ladderCount] = yield* Effect.all([
      readEarningRows(db, { actorRef: input.actorRef, limit }),
      readTipLadderEarningRows(db, { actorRef: input.actorRef, limit }),
      countEarningRows(db, input.actorRef),
      countTipLadderEarningRows(db, input.actorRef),
    ])
    const totalCount = receiptCount + ladderCount
    const earnings = sortEarningsNewestFirst([
      ...rows.map(earningFromRow),
      ...ladderRows.map(tipLadderEarningFromRow),
    ]).slice(0, limit)
    const projection = decodeCreatorEarningsResponse({
      actorRef: input.actorRef,
      earnings,
      generatedAt: runtime.nowIso(),
      pagination: {
        cursor: null,
        hasMore: totalCount > earnings.length,
        limit,
        nextCursor: null,
      },
      staleness: FORUM_TIP_PROJECTION_STALENESS,
      summary: summarizeEarnings(earnings),
    })

    assertProjectionSafe(projection)

    return projection
  })

export const readForumTipReconciliation = (
  db: D1Database,
  input: Readonly<{ actorRef: string | null; limit?: number }>,
  runtime: ForumTipEarningsRuntime,
): Effect.Effect<ForumTipReconciliationResponse, ForumStorageError> =>
  Effect.gen(function* () {
    const limit = Math.min(Math.max(input.limit ?? 50, 1), 100)
    const [rows, ladderRows, receiptCount, ladderCount] = yield* Effect.all([
      readEarningRows(db, { actorRef: input.actorRef, limit }),
      readTipLadderEarningRows(db, { actorRef: input.actorRef, limit }),
      countEarningRows(db, input.actorRef),
      countTipLadderEarningRows(db, input.actorRef),
    ])
    const totalCount = receiptCount + ladderCount
    const earnings = sortEarningsNewestFirst([
      ...rows.map(earningFromRow),
      ...ladderRows.map(tipLadderEarningFromRow),
    ]).slice(0, limit)
    const projection = decodeTipReconciliationResponse({
      acceptedWorkPayoutBoundary: 'ordinary_forum_tips_are_not_accepted_work',
      actorRef: input.actorRef,
      earnings,
      generatedAt: runtime.nowIso(),
      operatorCaveatRefs: [
        'caveat.public.forum_tip.reconciliation_redacted',
        'caveat.public.forum_tip.not_accepted_work_payout',
      ],
      pagination: {
        cursor: null,
        hasMore: totalCount > earnings.length,
        limit,
        nextCursor: null,
      },
      staleness: FORUM_TIP_PROJECTION_STALENESS,
      summary: summarizeEarnings(earnings),
    })

    assertProjectionSafe(projection)

    return projection
  })
