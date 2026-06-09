import { Schema as S } from 'effect'

import type {
  ReferralAttributionRecord,
  ReferralAttributionTarget,
  ReferralInviteAudiencePath,
} from './site-referrals'

export type ReferralConsumptionRuntime = Readonly<{
  nowIso: () => string
}>

export type ReferralConsumptionResult =
  | Readonly<{
      _tag: 'none'
    }>
  | Readonly<{
      _tag: 'expired'
      attributionId: string
    }>
  | Readonly<{
      _tag: 'already_verified'
      attributionId: string
    }>
  | Readonly<{
      _tag: 'consumed'
      attributionId: string
    }>

export class SiteReferralConsumptionStorageError extends S.TaggedErrorClass<SiteReferralConsumptionStorageError>()(
  'SiteReferralConsumptionStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

type ReferralAttributionRow = Readonly<{
  archived_at: string | null
  capture_path: ReferralInviteAudiencePath
  claimed_user_id: string | null
  created_at: string
  expires_at: string
  first_verified_at: string | null
  id: string
  policy_state: ReferralAttributionRecord['policyState']
  public_invite_ref: string | null
  public_source_ref: string
  referral_invite_id: string | null
  referral_source_id: string
  target: ReferralAttributionTarget
  updated_at: string
}>

type UserAttributionRow = Readonly<{
  referral_attribution_id: string
}>

type OrderAttributionRow = Readonly<{
  referral_attribution_id: string
}>

type AgentAttributionRow = Readonly<{
  referral_attribution_id: string
}>

const SAFE_ATTRIBUTION_ID_PATTERN =
  /^referral_attribution_[A-Za-z0-9_-]{1,190}$/

const storage = async <T>(
  operation: string,
  run: () => Promise<T>,
): Promise<T> => {
  try {
    return await run()
  } catch (error) {
    throw new SiteReferralConsumptionStorageError({ operation, error })
  }
}

const pendingAttribution = async (
  db: D1Database,
  attributionId: string | undefined,
  nowIso: string,
): Promise<ReferralAttributionRow | null> => {
  if (
    attributionId === undefined ||
    !SAFE_ATTRIBUTION_ID_PATTERN.test(attributionId)
  ) {
    return null
  }

  return storage('siteReferralConsumption.pendingAttribution.read', () =>
    db
      .prepare(
        `SELECT *
           FROM referral_attributions
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(attributionId)
      .first<ReferralAttributionRow>(),
  )
}

const existingUserAttribution = (
  db: D1Database,
  userId: string,
): Promise<UserAttributionRow | null> =>
  storage('siteReferralConsumption.userAttribution.read', () =>
    db
      .prepare(
        `SELECT referral_attribution_id
           FROM user_referral_attributions
          WHERE user_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(userId)
      .first<UserAttributionRow>(),
  )

const existingOrderAttribution = (
  db: D1Database,
  orderId: string,
): Promise<OrderAttributionRow | null> =>
  storage('siteReferralConsumption.orderAttribution.read', () =>
    db
      .prepare(
        `SELECT referral_attribution_id
           FROM order_referral_attributions
          WHERE software_order_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(orderId)
      .first<OrderAttributionRow>(),
  )

const existingAgentAttribution = (
  db: D1Database,
  agentUserId: string,
): Promise<AgentAttributionRow | null> =>
  storage('siteReferralConsumption.agentAttribution.read', () =>
    db
      .prepare(
        `SELECT referral_attribution_id
           FROM agent_referral_attributions
          WHERE agent_user_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(agentUserId)
      .first<AgentAttributionRow>(),
  )

const insertUserAttribution = (
  db: D1Database,
  input: Readonly<{
    attribution: ReferralAttributionRow
    nowIso: string
    userId: string
  }>,
): Promise<void> =>
  storage('siteReferralConsumption.userAttribution.insert', async () => {
    await db
      .prepare(
        `INSERT OR IGNORE INTO user_referral_attributions
           (user_id,
            referral_attribution_id,
            referral_source_id,
            referral_invite_id,
            capture_path,
            target,
            first_verified_at,
            policy_state,
            created_at,
            updated_at,
            archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)`,
      )
      .bind(
        input.userId,
        input.attribution.id,
        input.attribution.referral_source_id,
        input.attribution.referral_invite_id,
        input.attribution.capture_path,
        input.attribution.target,
        input.nowIso,
        input.nowIso,
        input.nowIso,
      )
      .run()
  })

const insertOrderAttribution = (
  db: D1Database,
  input: Readonly<{
    attribution: ReferralAttributionRow
    nowIso: string
    orderId: string
    userId: string
  }>,
): Promise<void> =>
  storage('siteReferralConsumption.orderAttribution.insert', async () => {
    await db
      .prepare(
        `INSERT OR IGNORE INTO order_referral_attributions
           (software_order_id,
            user_id,
            referral_attribution_id,
            referral_source_id,
            referral_invite_id,
            capture_path,
            target,
            linked_at,
            policy_state,
            created_at,
            updated_at,
            archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)`,
      )
      .bind(
        input.orderId,
        input.userId,
        input.attribution.id,
        input.attribution.referral_source_id,
        input.attribution.referral_invite_id,
        input.attribution.capture_path,
        input.attribution.target,
        input.nowIso,
        input.nowIso,
        input.nowIso,
      )
      .run()
  })

const insertAgentAttribution = (
  db: D1Database,
  input: Readonly<{
    agentUserId: string
    attribution: ReferralAttributionRow
    nowIso: string
    ownerUserId: string | null
  }>,
): Promise<void> =>
  storage('siteReferralConsumption.agentAttribution.insert', async () => {
    await db
      .prepare(
        `INSERT OR IGNORE INTO agent_referral_attributions
           (agent_user_id,
            owner_user_id,
            referral_attribution_id,
            referral_source_id,
            referral_invite_id,
            capture_path,
            target,
            claimed_at,
            policy_state,
            created_at,
            updated_at,
            archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)`,
      )
      .bind(
        input.agentUserId,
        input.ownerUserId,
        input.attribution.id,
        input.attribution.referral_source_id,
        input.attribution.referral_invite_id,
        input.attribution.capture_path,
        input.attribution.target,
        input.nowIso,
        input.nowIso,
        input.nowIso,
      )
      .run()
  })

const markClaimed = (
  db: D1Database,
  input: Readonly<{
    attributionId: string
    nowIso: string
    userId: string
  }>,
): Promise<void> =>
  storage('siteReferralConsumption.attribution.claim', async () => {
    await db
      .prepare(
        `UPDATE referral_attributions
            SET policy_state = 'claimed',
                claimed_user_id = COALESCE(claimed_user_id, ?),
                first_verified_at = COALESCE(first_verified_at, ?),
                updated_at = ?
          WHERE id = ?
            AND policy_state = 'pending'
            AND archived_at IS NULL`,
      )
      .bind(input.userId, input.nowIso, input.nowIso, input.attributionId)
      .run()
  })

export const consumePendingReferralForUser = async (
  db: D1Database,
  runtime: ReferralConsumptionRuntime,
  input: Readonly<{
    pendingAttributionId: string | undefined
    userId: string
  }>,
): Promise<ReferralConsumptionResult> => {
  const nowIso = runtime.nowIso()
  const existing = await existingUserAttribution(db, input.userId)

  if (existing !== null) {
    return {
      _tag: 'already_verified',
      attributionId: existing.referral_attribution_id,
    }
  }

  const attribution = await pendingAttribution(
    db,
    input.pendingAttributionId,
    nowIso,
  )

  if (attribution === null) {
    return { _tag: 'none' }
  }

  if (
    attribution.policy_state !== 'pending' ||
    attribution.expires_at <= nowIso
  ) {
    return { _tag: 'expired', attributionId: attribution.id }
  }

  await insertUserAttribution(db, {
    attribution,
    nowIso,
    userId: input.userId,
  })
  await markClaimed(db, {
    attributionId: attribution.id,
    nowIso,
    userId: input.userId,
  })

  return { _tag: 'consumed', attributionId: attribution.id }
}

export const linkPendingReferralToOrder = async (
  db: D1Database,
  runtime: ReferralConsumptionRuntime,
  input: Readonly<{
    orderId: string
    pendingAttributionId: string | undefined
    userId: string
  }>,
): Promise<ReferralConsumptionResult> => {
  const nowIso = runtime.nowIso()
  const existingOrder = await existingOrderAttribution(db, input.orderId)

  if (existingOrder !== null) {
    return {
      _tag: 'already_verified',
      attributionId: existingOrder.referral_attribution_id,
    }
  }

  const userResult = await consumePendingReferralForUser(db, runtime, input)
  const userAttribution = await existingUserAttribution(db, input.userId)

  if (userAttribution === null) {
    return userResult
  }

  const attribution = await pendingAttribution(
    db,
    userAttribution.referral_attribution_id,
    nowIso,
  )

  if (attribution === null) {
    return userResult
  }

  await insertOrderAttribution(db, {
    attribution,
    nowIso,
    orderId: input.orderId,
    userId: input.userId,
  })

  return userResult._tag === 'none'
    ? {
        _tag: 'already_verified',
        attributionId: userAttribution.referral_attribution_id,
      }
    : userResult
}

export const linkPendingReferralToAgentClaim = async (
  db: D1Database,
  runtime: ReferralConsumptionRuntime,
  input: Readonly<{
    agentUserId: string
    ownerUserId: string | null
    pendingAttributionId: string | undefined
  }>,
): Promise<ReferralConsumptionResult> => {
  const nowIso = runtime.nowIso()
  const existingAgent = await existingAgentAttribution(db, input.agentUserId)

  if (existingAgent !== null) {
    return {
      _tag: 'already_verified',
      attributionId: existingAgent.referral_attribution_id,
    }
  }

  const attribution = await pendingAttribution(
    db,
    input.pendingAttributionId,
    nowIso,
  )

  if (attribution === null) {
    return { _tag: 'none' }
  }

  if (
    attribution.policy_state !== 'pending' ||
    attribution.expires_at <= nowIso
  ) {
    return { _tag: 'expired', attributionId: attribution.id }
  }

  await insertAgentAttribution(db, {
    agentUserId: input.agentUserId,
    attribution,
    nowIso,
    ownerUserId: input.ownerUserId,
  })
  await markClaimed(db, {
    attributionId: attribution.id,
    nowIso,
    userId: input.ownerUserId ?? input.agentUserId,
  })

  return { _tag: 'consumed', attributionId: attribution.id }
}
