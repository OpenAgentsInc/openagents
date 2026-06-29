import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  SiteReferralRewardGate,
  projectSiteReferralRewardGate,
} from './site-referral-reward-gate'
import type {
  ReferralAttributionPolicyState,
  ReferralAttributionTarget,
  ReferralInviteAudiencePath,
  SiteReferralSourcePolicyState,
} from './site-referrals'

export const SiteReferralSourceMetrics = S.Struct({
  agentClaimCount: S.Number,
  cappedPolicyCount: S.Number,
  captureCount: S.Number,
  claimedCaptureCount: S.Number,
  disputedCaptureCount: S.Number,
  disputedPolicyCount: S.Number,
  expiredCaptureCount: S.Number,
  heldPolicyCount: S.Number,
  latestCaptureAt: S.NullOr(S.String),
  latestVerifiedAt: S.NullOr(S.String),
  linkedOrderCount: S.Number,
  operatorOverrideCount: S.Number,
  paidWorkflowCount: S.Number,
  pendingCaptureCount: S.Number,
  reversedPolicyCount: S.Number,
  verifiedUserCount: S.Number,
})
export type SiteReferralSourceMetrics = typeof SiteReferralSourceMetrics.Type

export const SiteReferralOwnerSourceSummary = S.Struct({
  campaignRef: S.NullOr(S.String),
  metrics: SiteReferralSourceMetrics,
  policyState: S.String,
  publicSlug: S.String,
  publicSourceRef: S.String,
  referralSourceId: S.String,
  rewardGate: SiteReferralRewardGate,
  siteId: S.String,
  siteSlug: S.String,
  siteTitle: S.String,
  sourceLabel: S.NullOr(S.String),
})
export type SiteReferralOwnerSourceSummary =
  typeof SiteReferralOwnerSourceSummary.Type

export const SiteReferralOwnerOverview = S.Struct({
  sources: S.Array(SiteReferralOwnerSourceSummary),
  totals: SiteReferralSourceMetrics,
})
export type SiteReferralOwnerOverview = typeof SiteReferralOwnerOverview.Type

export const OperatorReferralAttributionInspection = S.Struct({
  capturePath: S.String,
  claimedUserId: S.NullOr(S.String),
  createdAt: S.String,
  expiresAt: S.String,
  firstVerifiedAt: S.NullOr(S.String),
  linkedOrderCount: S.Number,
  policyState: S.String,
  publicInviteRef: S.NullOr(S.String),
  publicSourceRef: S.String,
  referralAttributionId: S.String,
  referralInviteId: S.NullOr(S.String),
  referralSourceId: S.String,
  siteId: S.String,
  siteSlug: S.String,
  siteTitle: S.String,
  target: S.String,
  updatedAt: S.String,
})
export type OperatorReferralAttributionInspection =
  typeof OperatorReferralAttributionInspection.Type

export const OperatorSiteReferralInspection = S.Struct({
  attributions: S.Array(OperatorReferralAttributionInspection),
  sources: S.Array(
    S.Struct({
      campaignRef: S.NullOr(S.String),
      metrics: SiteReferralSourceMetrics,
      policyState: S.String,
      publicSlug: S.String,
      publicSourceRef: S.String,
      referralSourceId: S.String,
      referrerUserId: S.String,
      rewardGate: SiteReferralRewardGate,
      siteId: S.String,
      siteOwnerUserId: S.String,
      siteSlug: S.String,
      siteTitle: S.String,
      sourceLabel: S.NullOr(S.String),
    }),
  ),
  totals: SiteReferralSourceMetrics,
})
export type OperatorSiteReferralInspection =
  typeof OperatorSiteReferralInspection.Type

export const OperatorConsumedReferralAttributions = S.Struct({
  attributions: S.Array(OperatorReferralAttributionInspection),
})
export type OperatorConsumedReferralAttributions =
  typeof OperatorConsumedReferralAttributions.Type

type SourceMetricsRow = Readonly<{
  agent_claim_count: number | null
  capped_policy_count: number | null
  campaign_ref: string | null
  capture_count: number | null
  claimed_capture_count: number | null
  disputed_capture_count: number | null
  disputed_policy_count: number | null
  expired_capture_count: number | null
  held_policy_count: number | null
  latest_capture_at: string | null
  latest_verified_at: string | null
  linked_order_count: number | null
  operator_override_count: number | null
  paid_workflow_count: number | null
  pending_capture_count: number | null
  policy_state: SiteReferralSourcePolicyState
  public_slug: string
  public_source_ref: string
  referral_source_id: string
  referrer_user_id: string
  reversed_policy_count: number | null
  site_id: string
  site_owner_user_id: string
  site_slug: string
  site_title: string
  source_label: string | null
  verified_user_count: number | null
}>

type AttributionInspectionRow = Readonly<{
  capture_path: ReferralInviteAudiencePath
  claimed_user_id: string | null
  created_at: string
  expires_at: string
  first_verified_at: string | null
  linked_order_count: number | null
  policy_state: ReferralAttributionPolicyState
  public_invite_ref: string | null
  public_source_ref: string
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  site_id: string
  site_slug: string
  site_title: string
  target: ReferralAttributionTarget
  updated_at: string
}>

const SAFE_PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
const SAFE_PUBLIC_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{0,120}$/
const PROHIBITED_PUBLIC_TEXT_PATTERN =
  /\b(provider[_ -]?account|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic)\b/i

const countFromRow = (value: number | null): number =>
  Number.isFinite(value) ? Number(value) : 0

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const textIsPublicSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_PUBLIC_TEXT_PATTERN.test(value)

const publicText = (
  value: string | null | undefined,
  maxLength: number,
): string | null => {
  if (value === null || value === undefined) {
    return null
  }

  const compact = compactText(value, maxLength)

  return compact === '' || !textIsPublicSafe(compact) ? null : compact
}

const publicRef = (value: string): string =>
  SAFE_PUBLIC_REF_PATTERN.test(value) && textIsPublicSafe(value)
    ? value
    : 'source'

const publicSlug = (value: string): string =>
  SAFE_PUBLIC_SLUG_PATTERN.test(value) && textIsPublicSafe(value)
    ? value
    : 'site'

const metricsFromRow = (row: SourceMetricsRow): SiteReferralSourceMetrics => ({
  agentClaimCount: countFromRow(row.agent_claim_count),
  cappedPolicyCount: countFromRow(row.capped_policy_count),
  captureCount: countFromRow(row.capture_count),
  claimedCaptureCount: countFromRow(row.claimed_capture_count),
  disputedCaptureCount: countFromRow(row.disputed_capture_count),
  disputedPolicyCount: countFromRow(row.disputed_policy_count),
  expiredCaptureCount: countFromRow(row.expired_capture_count),
  heldPolicyCount: countFromRow(row.held_policy_count),
  latestCaptureAt: publicText(row.latest_capture_at, 80),
  latestVerifiedAt: publicText(row.latest_verified_at, 80),
  linkedOrderCount: countFromRow(row.linked_order_count),
  operatorOverrideCount: countFromRow(row.operator_override_count),
  paidWorkflowCount: countFromRow(row.paid_workflow_count),
  pendingCaptureCount: countFromRow(row.pending_capture_count),
  reversedPolicyCount: countFromRow(row.reversed_policy_count),
  verifiedUserCount: countFromRow(row.verified_user_count),
})

const emptyMetrics = (): SiteReferralSourceMetrics => ({
  agentClaimCount: 0,
  cappedPolicyCount: 0,
  captureCount: 0,
  claimedCaptureCount: 0,
  disputedCaptureCount: 0,
  disputedPolicyCount: 0,
  expiredCaptureCount: 0,
  heldPolicyCount: 0,
  latestCaptureAt: null,
  latestVerifiedAt: null,
  linkedOrderCount: 0,
  operatorOverrideCount: 0,
  paidWorkflowCount: 0,
  pendingCaptureCount: 0,
  reversedPolicyCount: 0,
  verifiedUserCount: 0,
})

const combineLatest = (
  left: string | null,
  right: string | null,
): string | null => {
  if (left === null) {
    return right
  }

  if (right === null) {
    return left
  }

  return left >= right ? left : right
}

const addMetrics = (
  left: SiteReferralSourceMetrics,
  right: SiteReferralSourceMetrics,
): SiteReferralSourceMetrics => ({
  agentClaimCount: left.agentClaimCount + right.agentClaimCount,
  cappedPolicyCount: left.cappedPolicyCount + right.cappedPolicyCount,
  captureCount: left.captureCount + right.captureCount,
  claimedCaptureCount: left.claimedCaptureCount + right.claimedCaptureCount,
  disputedCaptureCount: left.disputedCaptureCount + right.disputedCaptureCount,
  disputedPolicyCount: left.disputedPolicyCount + right.disputedPolicyCount,
  expiredCaptureCount: left.expiredCaptureCount + right.expiredCaptureCount,
  heldPolicyCount: left.heldPolicyCount + right.heldPolicyCount,
  latestCaptureAt: combineLatest(left.latestCaptureAt, right.latestCaptureAt),
  latestVerifiedAt: combineLatest(
    left.latestVerifiedAt,
    right.latestVerifiedAt,
  ),
  linkedOrderCount: left.linkedOrderCount + right.linkedOrderCount,
  operatorOverrideCount:
    left.operatorOverrideCount + right.operatorOverrideCount,
  paidWorkflowCount: left.paidWorkflowCount + right.paidWorkflowCount,
  pendingCaptureCount: left.pendingCaptureCount + right.pendingCaptureCount,
  reversedPolicyCount: left.reversedPolicyCount + right.reversedPolicyCount,
  verifiedUserCount: left.verifiedUserCount + right.verifiedUserCount,
})

const refsIf = (
  condition: boolean,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => (condition ? refs : [])

const sourceRewardGateFromRow = (
  row: SourceMetricsRow,
  metrics: SiteReferralSourceMetrics,
): SiteReferralRewardGate => {
  const referralSourceRef = publicRef(row.referral_source_id)
  const sourcePolicyBlockerRefs =
    row.policy_state === 'active'
      ? []
      : [`blocker.public.site_referral.source_${row.policy_state}`]

  return projectSiteReferralRewardGate({
    attributionRefs: refsIf(metrics.captureCount > 0, [
      `attribution.public.site_referral.${referralSourceRef}`,
    ]),
    paidActivityRefs: refsIf(metrics.paidWorkflowCount > 0, [
      `workflow.public.site_referral.${referralSourceRef}.paid_activity`,
    ]),
    policyBlockerRefs: [
      ...sourcePolicyBlockerRefs,
      ...refsIf(metrics.heldPolicyCount > 0, [
        'blocker.public.site_referral.policy_held',
      ]),
      ...refsIf(metrics.disputedCaptureCount > 0, [
        'blocker.public.site_referral.capture_disputed',
      ]),
      ...refsIf(metrics.disputedPolicyCount > 0, [
        'blocker.public.site_referral.dispute_hold',
      ]),
      ...refsIf(metrics.cappedPolicyCount > 0, [
        'blocker.public.site_referral.cap_exceeded',
      ]),
      ...refsIf(metrics.reversedPolicyCount > 0, [
        'blocker.public.site_referral.chargeback_refund_or_clawback',
      ]),
      ...refsIf(metrics.operatorOverrideCount > 0, [
        'blocker.public.site_referral.operator_review',
      ]),
    ],
    settlementReceiptRefs: [],
  })
}

const ownerSourceFromRow = (
  row: SourceMetricsRow,
): SiteReferralOwnerSourceSummary => {
  const metrics = metricsFromRow(row)

  return {
    campaignRef: publicText(row.campaign_ref, 120),
    metrics,
    policyState: row.policy_state,
    publicSlug: publicSlug(row.public_slug),
    publicSourceRef: publicRef(row.public_source_ref),
    referralSourceId: publicRef(row.referral_source_id),
    rewardGate: sourceRewardGateFromRow(row, metrics),
    siteId: publicRef(row.site_id),
    siteSlug: publicSlug(row.site_slug),
    siteTitle: publicText(row.site_title, 160) ?? 'OpenAgents Site',
    sourceLabel: publicText(row.source_label, 160),
  }
}

const operatorSourceFromRow = (row: SourceMetricsRow) => ({
  ...ownerSourceFromRow(row),
  referrerUserId: publicRef(row.referrer_user_id),
  siteOwnerUserId: publicRef(row.site_owner_user_id),
})

const attributionFromRow = (
  row: AttributionInspectionRow,
): OperatorReferralAttributionInspection => ({
  capturePath: row.capture_path,
  claimedUserId:
    row.claimed_user_id === null ? null : publicRef(row.claimed_user_id),
  createdAt: publicText(row.created_at, 80) ?? '',
  expiresAt: publicText(row.expires_at, 80) ?? '',
  firstVerifiedAt: publicText(row.first_verified_at, 80),
  linkedOrderCount: countFromRow(row.linked_order_count),
  policyState: row.policy_state,
  publicInviteRef:
    row.public_invite_ref === null ? null : publicRef(row.public_invite_ref),
  publicSourceRef: publicRef(row.public_source_ref),
  referralAttributionId: publicRef(row.referral_attribution_id),
  referralInviteId:
    row.referral_invite_id === null ? null : publicRef(row.referral_invite_id),
  referralSourceId: publicRef(row.referral_source_id),
  siteId: publicRef(row.site_id),
  siteSlug: publicSlug(row.site_slug),
  siteTitle: publicText(row.site_title, 160) ?? 'OpenAgents Site',
  target: row.target,
  updatedAt: publicText(row.updated_at, 80) ?? '',
})

const assertPublicSafeProjection = (label: string, value: unknown): void => {
  const json = JSON.stringify(value)

  if (
    containsProviderSecretMaterial(json) ||
    PROHIBITED_PUBLIC_TEXT_PATTERN.test(json) ||
    /"email"\s*:|"primary_email"\s*:|"user_email"\s*:/i.test(json)
  ) {
    throw new SiteReferralInspectionUnsafePayload({
      reason: `${label} contains private or secret-shaped material.`,
    })
  }
}

const sourceMetricsQuery = (whereClause: string): string =>
  `SELECT site_referral_sources.id AS referral_source_id,
          site_referral_sources.referrer_user_id,
          site_referral_sources.public_source_ref,
          site_referral_sources.public_slug,
          site_referral_sources.campaign_ref,
          site_referral_sources.source_label,
          site_referral_sources.policy_state,
          site_projects.id AS site_id,
          site_projects.owner_user_id AS site_owner_user_id,
          site_projects.slug AS site_slug,
          site_projects.title AS site_title,
          COUNT(DISTINCT referral_attributions.id) AS capture_count,
          COUNT(DISTINCT CASE
            WHEN referral_attributions.policy_state = 'pending'
            THEN referral_attributions.id
          END) AS pending_capture_count,
          COUNT(DISTINCT CASE
            WHEN referral_attributions.policy_state = 'claimed'
            THEN referral_attributions.id
          END) AS claimed_capture_count,
          COUNT(DISTINCT CASE
            WHEN referral_attributions.policy_state = 'disputed'
            THEN referral_attributions.id
          END) AS disputed_capture_count,
          COUNT(DISTINCT CASE
            WHEN referral_attributions.policy_state = 'expired'
            THEN referral_attributions.id
          END) AS expired_capture_count,
          COUNT(DISTINCT user_referral_attributions.user_id)
            AS verified_user_count,
          COUNT(DISTINCT order_referral_attributions.software_order_id)
            AS linked_order_count,
          COUNT(DISTINCT agent_referral_attributions.agent_user_id)
            AS agent_claim_count,
          COUNT(DISTINCT CASE
            WHEN referral_workflow_events.event_kind IN (
              'paid_usage',
              'site_checkout',
              'l402_redemption',
              'accepted_outcome'
            )
             AND referral_workflow_events.policy_state IN (
              'recorded',
              'eligible'
            )
            THEN referral_workflow_events.id
          END) AS paid_workflow_count,
          COUNT(DISTINCT CASE
            WHEN site_referral_policy_events.decision_state = 'held'
            THEN site_referral_policy_events.id
          END) AS held_policy_count,
          COUNT(DISTINCT CASE
            WHEN site_referral_policy_events.decision_state = 'disputed'
            THEN site_referral_policy_events.id
          END) AS disputed_policy_count,
          COUNT(DISTINCT CASE
            WHEN site_referral_policy_events.decision_state = 'capped'
            THEN site_referral_policy_events.id
          END) AS capped_policy_count,
          COUNT(DISTINCT CASE
            WHEN site_referral_policy_events.decision_state = 'reversed'
            THEN site_referral_policy_events.id
          END) AS reversed_policy_count,
          COUNT(DISTINCT CASE
            WHEN site_referral_policy_events.decision_state =
                 'operator_overridden'
            THEN site_referral_policy_events.id
          END) AS operator_override_count,
          MAX(referral_attributions.created_at) AS latest_capture_at,
          MAX(user_referral_attributions.first_verified_at)
            AS latest_verified_at
     FROM site_referral_sources
     JOIN site_projects
       ON site_projects.id = site_referral_sources.site_id
      AND site_projects.archived_at IS NULL
     LEFT JOIN referral_attributions
       ON referral_attributions.referral_source_id = site_referral_sources.id
      AND referral_attributions.archived_at IS NULL
     LEFT JOIN user_referral_attributions
       ON user_referral_attributions.referral_source_id =
          site_referral_sources.id
      AND user_referral_attributions.archived_at IS NULL
     LEFT JOIN order_referral_attributions
       ON order_referral_attributions.referral_source_id =
          site_referral_sources.id
      AND order_referral_attributions.archived_at IS NULL
     LEFT JOIN agent_referral_attributions
       ON agent_referral_attributions.referral_source_id =
          site_referral_sources.id
      AND agent_referral_attributions.archived_at IS NULL
     LEFT JOIN referral_workflow_events
       ON referral_workflow_events.referral_source_id =
          site_referral_sources.id
      AND referral_workflow_events.archived_at IS NULL
     LEFT JOIN site_referral_policy_events
       ON site_referral_policy_events.referral_source_id =
          site_referral_sources.id
      AND site_referral_policy_events.archived_at IS NULL
    WHERE site_referral_sources.archived_at IS NULL
      ${whereClause}
    GROUP BY site_referral_sources.id
    ORDER BY site_referral_sources.created_at DESC
    LIMIT ?`

const operatorAttributionsQuery = (whereClause: string): string => `
  SELECT referral_attributions.id AS referral_attribution_id,
         referral_attributions.referral_source_id,
         referral_attributions.referral_invite_id,
         referral_attributions.public_source_ref,
         referral_attributions.public_invite_ref,
         referral_attributions.capture_path,
         referral_attributions.target,
         referral_attributions.policy_state,
         referral_attributions.first_verified_at,
         referral_attributions.claimed_user_id,
         referral_attributions.expires_at,
         referral_attributions.created_at,
         referral_attributions.updated_at,
         site_projects.id AS site_id,
         site_projects.slug AS site_slug,
         site_projects.title AS site_title,
         (
           SELECT COUNT(*)
             FROM order_referral_attributions
            WHERE order_referral_attributions.referral_attribution_id =
                  referral_attributions.id
              AND order_referral_attributions.archived_at IS NULL
         ) AS linked_order_count
    FROM referral_attributions
    JOIN site_referral_sources
      ON site_referral_sources.id = referral_attributions.referral_source_id
     AND site_referral_sources.archived_at IS NULL
    JOIN site_projects
      ON site_projects.id = site_referral_sources.site_id
     AND site_projects.archived_at IS NULL
   WHERE referral_attributions.archived_at IS NULL
     ${whereClause}
   ORDER BY referral_attributions.created_at DESC
   LIMIT ?`

export class SiteReferralInspectionUnsafePayload extends S.TaggedErrorClass<SiteReferralInspectionUnsafePayload>()(
  'SiteReferralInspectionUnsafePayload',
  {
    reason: S.String,
  },
) {}

export const readSiteReferralOwnerOverview = async (
  db: D1Database,
  ownerUserId: string,
  limit = 100,
): Promise<SiteReferralOwnerOverview> => {
  const rows = await db
    .prepare(
      sourceMetricsQuery(`AND site_referral_sources.referrer_user_id = ?`),
    )
    .bind(ownerUserId, limit)
    .all<SourceMetricsRow>()

  const sources = (rows.results ?? []).map(ownerSourceFromRow)
  const overview = {
    sources,
    totals: sources.reduce(
      (total, source) => addMetrics(total, source.metrics),
      emptyMetrics(),
    ),
  } satisfies SiteReferralOwnerOverview

  assertPublicSafeProjection('Site referral owner overview', overview)

  return overview
}

export const readOperatorSiteReferralInspection = async (
  db: D1Database,
  limit = 100,
): Promise<OperatorSiteReferralInspection> => {
  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const [sourceRows, attributionRows] = await Promise.all([
    db.prepare(sourceMetricsQuery('')).bind(safeLimit).all<SourceMetricsRow>(),
    db
      .prepare(operatorAttributionsQuery(''))
      .bind(safeLimit)
      .all<AttributionInspectionRow>(),
  ])
  const sources = (sourceRows.results ?? []).map(operatorSourceFromRow)
  const inspection = {
    attributions: (attributionRows.results ?? []).map(attributionFromRow),
    sources,
    totals: sources.reduce(
      (total, source) => addMetrics(total, source.metrics),
      emptyMetrics(),
    ),
  } satisfies OperatorSiteReferralInspection

  assertPublicSafeProjection('Operator Site referral inspection', inspection)

  return inspection
}

export const readOperatorConsumedReferralAttributions = async (
  db: D1Database,
  limit = 100,
): Promise<OperatorConsumedReferralAttributions> => {
  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      operatorAttributionsQuery(
        `AND referral_attributions.policy_state = 'claimed'
         AND referral_attributions.first_verified_at IS NOT NULL`,
      ),
    )
    .bind(safeLimit)
    .all<AttributionInspectionRow>()
  const projection = {
    attributions: (rows.results ?? []).map(attributionFromRow),
  } satisfies OperatorConsumedReferralAttributions

  assertPublicSafeProjection('Operator consumed referral attributions', projection)

  return projection
}
