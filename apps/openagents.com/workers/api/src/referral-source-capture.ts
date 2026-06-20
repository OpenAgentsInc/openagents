// Shared referral-source capture: resolve a public referral source ref and
// create a pending referral_attributions row, reusing the existing referral
// spine (site_referral_sources -> referral_attributions). This is the same
// capture the /r/<ref> route performs; it is factored out so other inbound
// surfaces (e.g. the /business intake) can capture an inbound ?ref=<code>
// without rebuilding a parallel referral path.
//
// It does NOT consume the attribution. Consumption (consume-once binding to a
// user, order, agent, or business signup) stays in
// site-referral-attribution-consumption.ts.

import {
  type ReferralAttributionRecord,
  type ReferralAttributionTarget,
  type ReferralInviteAudiencePath,
  type SiteReferralSourceRecord,
  referralSourceUnavailableReason,
} from './site-referrals'

export type ReferralSourceCaptureRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
  // Expiry timestamp for a freshly captured pending attribution.
  expiresAtFromNow: () => string
}>

type SiteReferralRow = Readonly<{
  archived_at: string | null
  campaign_ref: string | null
  created_at: string
  id: string
  policy_state: SiteReferralSourceRecord['policyState']
  public_slug: string
  public_source_ref: string
  referrer_user_id: string
  site_id: string
  site_version_id: string | null
  source_label: string | null
  updated_at: string
}>

// Bounded, safe public-source-ref shape (mirrors SAFE_REF_PATTERN in
// site-referral-routes.ts) so a hostile ?ref= value can never widen the query.
const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

export const isSafeReferralSourceRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value)

const sourceRecordFromRow = (
  row: SiteReferralRow,
): SiteReferralSourceRecord => ({
  archivedAt: row.archived_at,
  campaignRef: row.campaign_ref,
  createdAt: row.created_at,
  id: row.id,
  policyState: row.policy_state,
  publicSlug: row.public_slug,
  publicSourceRef: row.public_source_ref,
  referrerUserId: row.referrer_user_id,
  siteId: row.site_id,
  siteVersionId: row.site_version_id,
  sourceLabel: row.source_label,
  updatedAt: row.updated_at,
})

const findSourceByPublicRef = async (
  db: D1Database,
  publicSourceRef: string,
): Promise<SiteReferralSourceRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
       FROM site_referral_sources
       WHERE public_source_ref = ?
       LIMIT 1`,
    )
    .bind(publicSourceRef)
    .first<SiteReferralRow>()

  return row === null ? null : sourceRecordFromRow(row)
}

const insertPendingAttribution = async (
  db: D1Database,
  attribution: ReferralAttributionRecord,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO referral_attributions (
         id,
         referral_source_id,
         referral_invite_id,
         public_source_ref,
         public_invite_ref,
         capture_path,
         target,
         policy_state,
         first_verified_at,
         claimed_user_id,
         expires_at,
         created_at,
         updated_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      attribution.id,
      attribution.referralSourceId,
      attribution.referralInviteId,
      attribution.publicSourceRef,
      attribution.publicInviteRef,
      attribution.capturePath,
      attribution.target,
      attribution.policyState,
      attribution.firstVerifiedAt,
      attribution.claimedUserId,
      attribution.expiresAt,
      attribution.createdAt,
      attribution.updatedAt,
      attribution.archivedAt,
    )
    .run()
}

export type ReferralSourceCaptureResult =
  | Readonly<{ _tag: 'unavailable'; reason: string }>
  | Readonly<{
      _tag: 'captured'
      attribution: ReferralAttributionRecord
    }>

// Resolve a public referral source ref and, if it is active, create a new
// pending attribution. Used by inbound surfaces (e.g. /business) that receive a
// bare ?ref=<public_source_ref> rather than the /r/<ref> redirect-cookie flow.
export const capturePendingReferralBySourceRef = async (
  db: D1Database,
  runtime: ReferralSourceCaptureRuntime,
  input: Readonly<{
    publicSourceRef: string
    capturePath?: ReferralInviteAudiencePath
    target?: ReferralAttributionTarget
  }>,
): Promise<ReferralSourceCaptureResult> => {
  if (!isSafeReferralSourceRef(input.publicSourceRef)) {
    return { _tag: 'unavailable', reason: 'unknown' }
  }

  const source = await findSourceByPublicRef(db, input.publicSourceRef)
  const unavailable = referralSourceUnavailableReason(source)

  if (unavailable !== undefined || source === null) {
    return { _tag: 'unavailable', reason: unavailable ?? 'unknown' }
  }

  const nowIso = runtime.nowIso()
  const attribution = {
    archivedAt: null,
    capturePath: input.capturePath ?? 'human',
    claimedUserId: null,
    createdAt: nowIso,
    expiresAt: runtime.expiresAtFromNow(),
    firstVerifiedAt: null,
    id: runtime.makeId('referral_attribution'),
    policyState: 'pending',
    publicInviteRef: null,
    publicSourceRef: source.publicSourceRef,
    referralInviteId: null,
    referralSourceId: source.id,
    target: input.target ?? 'order',
    updatedAt: nowIso,
  } satisfies ReferralAttributionRecord

  await insertPendingAttribution(db, attribution)

  return { _tag: 'captured', attribution }
}
