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

type BusinessSignupAttributionRow = Readonly<{
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

const existingBusinessSignupAttribution = (
  db: D1Database,
  businessSignupRequestId: string,
): Promise<BusinessSignupAttributionRow | null> =>
  storage('siteReferralConsumption.businessSignupAttribution.read', () =>
    db
      .prepare(
        `SELECT referral_attribution_id
           FROM business_signup_referral_attributions
          WHERE business_signup_request_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(businessSignupRequestId)
      .first<BusinessSignupAttributionRow>(),
  )

const userAttributionStatement = (
  db: D1Database,
  input: Readonly<{
    attribution: ReferralAttributionRow
    nowIso: string
    userId: string
  }>,
): D1PreparedStatement =>
  db
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

const orderAttributionStatement = (
  db: D1Database,
  input: Readonly<{
    attribution: ReferralAttributionRow
    nowIso: string
    orderId: string
    userId: string
  }>,
): D1PreparedStatement =>
  db
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

const agentAttributionStatement = (
  db: D1Database,
  input: Readonly<{
    agentUserId: string
    attribution: ReferralAttributionRow
    nowIso: string
    ownerUserId: string | null
  }>,
): D1PreparedStatement =>
  db
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

const businessSignupAttributionStatement = (
  db: D1Database,
  input: Readonly<{
    attribution: ReferralAttributionRow
    businessSignupRequestId: string
    nowIso: string
  }>,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT OR IGNORE INTO business_signup_referral_attributions
           (business_signup_request_id,
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
         VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL)`,
    )
    .bind(
      input.businessSignupRequestId,
      input.attribution.id,
      input.attribution.referral_source_id,
      input.attribution.referral_invite_id,
      input.attribution.capture_path,
      input.attribution.target,
      input.nowIso,
      input.nowIso,
      input.nowIso,
    )

const markClaimedStatement = (
  db: D1Database,
  input: Readonly<{
    attributionId: string
    nowIso: string
    userId: string
  }>,
): D1PreparedStatement =>
  db
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

// A converted business signup is a pre-account lead: there is no users.id to
// credit yet, so the pending attribution is flipped to 'claimed' without
// setting claimed_user_id. The referral source still earns the credit via the
// referral_source_id recorded on the consume-once business-signup row; if the
// same lead later creates an account, the standard user-path consumption is a
// no-op because this attribution is already claimed (consume-once holds).
const markClaimedNoUserStatement = (
  db: D1Database,
  input: Readonly<{
    attributionId: string
    nowIso: string
  }>,
): D1PreparedStatement =>
  db
    .prepare(
      `UPDATE referral_attributions
            SET policy_state = 'claimed',
                first_verified_at = COALESCE(first_verified_at, ?),
                updated_at = ?
          WHERE id = ?
            AND policy_state = 'pending'
            AND archived_at IS NULL`,
    )
    .bind(input.nowIso, input.nowIso, input.attributionId)

const batchReferralConsumption = (
  db: D1Database,
  operation: string,
  statements: Array<D1PreparedStatement>,
): Promise<void> =>
  storage(operation, async () => {
    await db.batch(statements)
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

  // Attribution window: pending captures are valid until expires_at, currently
  // thirty days from capture. Last touch is represented by the pending cookie:
  // whichever unconsumed attribution id is present at signup/order claim wins,
  // and this batch locks that attribution exactly once with the qualifying row.
  await batchReferralConsumption(db, 'siteReferralConsumption.user.batch', [
    userAttributionStatement(db, {
      attribution,
      nowIso,
      userId: input.userId,
    }),
    markClaimedStatement(db, {
      attributionId: attribution.id,
      nowIso,
      userId: input.userId,
    }),
  ])

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

  await batchReferralConsumption(db, 'siteReferralConsumption.order.batch', [
    orderAttributionStatement(db, {
      attribution,
      nowIso,
      orderId: input.orderId,
      userId: input.userId,
    }),
  ])

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

  await batchReferralConsumption(db, 'siteReferralConsumption.agent.batch', [
    agentAttributionStatement(db, {
      agentUserId: input.agentUserId,
      attribution,
      nowIso,
      ownerUserId: input.ownerUserId,
    }),
    markClaimedStatement(db, {
      attributionId: attribution.id,
      nowIso,
      userId: input.ownerUserId ?? input.agentUserId,
    }),
  ])

  return { _tag: 'consumed', attributionId: attribution.id }
}

// Bind a converted business signup to the referral spine. Mirrors the
// agent-claim path: the binding is keyed on the business_signup_request_id, the
// pending attribution is consumed exactly once (PRIMARY KEY on the consume-once
// table + the pending->claimed guard prevent double-credit), and no users.id is
// required because a business signup is a pre-account lead.
export const linkPendingReferralToBusinessSignup = async (
  db: D1Database,
  runtime: ReferralConsumptionRuntime,
  input: Readonly<{
    businessSignupRequestId: string
    pendingAttributionId: string | undefined
  }>,
): Promise<ReferralConsumptionResult> => {
  const nowIso = runtime.nowIso()
  const existing = await existingBusinessSignupAttribution(
    db,
    input.businessSignupRequestId,
  )

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

  await batchReferralConsumption(
    db,
    'siteReferralConsumption.businessSignup.batch',
    [
      businessSignupAttributionStatement(db, {
        attribution,
        businessSignupRequestId: input.businessSignupRequestId,
        nowIso,
      }),
      markClaimedNoUserStatement(db, {
        attributionId: attribution.id,
        nowIso,
      }),
    ],
  )

  return { _tag: 'consumed', attributionId: attribution.id }
}
