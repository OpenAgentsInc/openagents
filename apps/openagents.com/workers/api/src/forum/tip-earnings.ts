import { containsProviderSecretMaterial } from '@openagents/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonUnknown } from '../json-boundary'
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
} from './schemas'
import { forumTipSettlementProjectionForReceipt } from './tip-settlement'

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

type CountRow = Readonly<{ count: number | null }>

type ForumTipLeaderboardPostRow = Readonly<{
  actor_json: string
  post_id: string
  tip_count: number | null
  topic_id: string
  total_paid_sats: number | null
  total_settled_sats: number | null
}>

type ForumTipLeaderboardCreatorRow = Readonly<{
  actor_json: string
  tip_count: number | null
  total_paid_sats: number | null
  total_settled_sats: number | null
}>

type ForumTipEarningsRuntime = Readonly<{
  nowIso: () => string
}>

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

const summarizeEarnings = (
  earnings: ReadonlyArray<ForumCreatorEarning>,
): ForumCreatorEarningsSummary => {
  const totalPaidSats = earnings
    .filter(earning => earning.amount.asset === 'sats')
    .filter(earning => earning.paymentState === 'confirmed')
    .reduce((sum, earning) => sum + earning.amount.amount, 0)
  const totalSettledSats = earnings
    .filter(earning => earning.amount.asset === 'sats')
    .filter(earning => earning.settlementState === 'settled')
    .reduce((sum, earning) => sum + earning.amount.amount, 0)

  return {
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
    totalCount: earnings.length,
    totalPaidSats,
    totalSettledSats,
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

const postLeaderboardFromRow = (
  row: ForumTipLeaderboardPostRow,
): ForumTipLeaderboardPost =>
  decodeTipLeaderboardPost({
    author: actorFromJson(row.actor_json),
    postId: row.post_id,
    postPermalink: forumPostPublicUrl(row.topic_id, row.post_id),
    tipCount: Math.max(0, Number(row.tip_count ?? 0)),
    topicId: row.topic_id,
    totalPaidSats: Math.max(0, Number(row.total_paid_sats ?? 0)),
    totalSettledSats: Math.max(0, Number(row.total_settled_sats ?? 0)),
  })

const creatorLeaderboardFromRow = (
  row: ForumTipLeaderboardCreatorRow,
): ForumTipLeaderboardCreator =>
  decodeTipLeaderboardCreator({
    actor: actorFromJson(row.actor_json),
    tipCount: Math.max(0, Number(row.tip_count ?? 0)),
    totalPaidSats: Math.max(0, Number(row.total_paid_sats ?? 0)),
    totalSettledSats: Math.max(0, Number(row.total_settled_sats ?? 0)),
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
            AND ma.target_post_id IS NOT NULL
            AND ma.target_topic_id IS NOT NULL
            AND ma.archived_at IS NULL
          GROUP BY ma.target_post_id, ma.target_topic_id, p.actor_json
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

const assertProjectionSafe = (
  value:
    | ForumCreatorEarningsResponse
    | ForumTipLeaderboardsResponse
    | ForumTipReconciliationResponse,
): void => {
  const json = JSON.stringify(value)

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
  const json = JSON.stringify(value)

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
    const projection = decodeTipLeaderboardsResponse({
      creators: creatorRows.map(creatorLeaderboardFromRow),
      generatedAt: runtime.nowIso(),
      posts: postRows.map(postLeaderboardFromRow),
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
    const [rows, totalCount] = yield* Effect.all([
      readEarningRows(db, { actorRef: input.actorRef, limit }),
      countEarningRows(db, input.actorRef),
    ])
    const earnings = rows.map(earningFromRow)
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
    const [rows, totalCount] = yield* Effect.all([
      readEarningRows(db, { actorRef: input.actorRef, limit }),
      countEarningRows(db, input.actorRef),
    ])
    const earnings = rows.map(earningFromRow)
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
      summary: summarizeEarnings(earnings),
    })

    assertProjectionSafe(projection)

    return projection
  })
