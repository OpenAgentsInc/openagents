import { Effect } from 'effect'

import { serializeCookie } from './auth-cookies'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  redirectResponse,
} from './http/responses'
import { openAgentsDatabase } from './runtime'
import {
  compactRandomId,
  currentDate,
  currentIsoTimestamp,
  isoTimestampAfter,
} from './runtime-primitives'
import {
  type ReferralAttributionRecord,
  type ReferralAttributionTarget,
  type ReferralInviteAudiencePath,
  type ReferralInviteRecord,
  PENDING_REFERRAL_COOKIE,
  PENDING_REFERRAL_MAX_AGE_SECONDS,
  type SiteReferralSourceRecord,
  publicReferralAttribution,
  referralCaptureRedirectLocation,
  referralInviteTarget,
  referralInviteUnavailableReason,
  referralSourceUnavailableReason,
} from './site-referrals'

type SiteReferralRouteEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

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

type ReferralInviteRow = Readonly<{
  archived_at: string | null
  audience_path: ReferralInviteAudiencePath
  created_at: string
  expires_at: string | null
  id: string
  policy_state: ReferralInviteRecord['policyState']
  public_invite_ref: string
  referral_source_id: string
  scope: ReferralInviteRecord['scope']
  token_hash: string
  updated_at: string
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

const sourceRecordFromRow = (row: SiteReferralRow): SiteReferralSourceRecord => ({
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

const inviteRecordFromRow = (row: ReferralInviteRow): ReferralInviteRecord => ({
  archivedAt: row.archived_at,
  audiencePath: row.audience_path,
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  id: row.id,
  policyState: row.policy_state,
  publicInviteRef: row.public_invite_ref,
  referralSourceId: row.referral_source_id,
  scope: row.scope,
  tokenHash: row.token_hash,
  updatedAt: row.updated_at,
})

const firstText = (value: string | null): string | undefined => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

const capturePathFromUrl = (url: URL): ReferralInviteAudiencePath =>
  firstText(url.searchParams.get('path')) === 'agent' ? 'agent' : 'human'

const targetFromUrl = (
  url: URL,
  fallback: ReferralAttributionTarget,
): ReferralAttributionTarget => {
  const target = firstText(url.searchParams.get('target'))

  if (target === 'order') {
    return 'order'
  }

  if (target === 'agent' || target === 'agent_claim') {
    return 'agent_claim'
  }

  if (target === 'home') {
    return 'home'
  }

  return fallback
}

const expiresAtFromNow = (): string =>
  isoTimestampAfter(currentDate(), PENDING_REFERRAL_MAX_AGE_SECONDS * 1000)

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

const findSourceById = async (
  db: D1Database,
  sourceId: string,
): Promise<SiteReferralSourceRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
       FROM site_referral_sources
       WHERE id = ?
       LIMIT 1`,
    )
    .bind(sourceId)
    .first<SiteReferralRow>()

  return row === null ? null : sourceRecordFromRow(row)
}

const findInviteByPublicRef = async (
  db: D1Database,
  publicInviteRef: string,
): Promise<ReferralInviteRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
       FROM referral_invites
       WHERE public_invite_ref = ?
       LIMIT 1`,
    )
    .bind(publicInviteRef)
    .first<ReferralInviteRow>()

  return row === null ? null : inviteRecordFromRow(row)
}

const createPendingAttribution = async (
  db: D1Database,
  input: Readonly<{
    capturePath: ReferralInviteAudiencePath
    expiresAt: string
    invite: ReferralInviteRecord | null
    nowIso: string
    source: SiteReferralSourceRecord
    target: ReferralAttributionTarget
  }>,
): Promise<ReferralAttributionRecord> => {
  const attribution = {
    archivedAt: null,
    capturePath: input.capturePath,
    claimedUserId: null,
    createdAt: input.nowIso,
    expiresAt: input.expiresAt,
    firstVerifiedAt: null,
    id: compactRandomId('referral_attribution'),
    policyState: 'pending',
    publicInviteRef: input.invite?.publicInviteRef ?? null,
    publicSourceRef: input.source.publicSourceRef,
    referralInviteId: input.invite?.id ?? null,
    referralSourceId: input.source.id,
    target: input.target,
    updatedAt: input.nowIso,
  } satisfies ReferralAttributionRecord

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

  return attribution
}

const unavailableResponse = (
  kind: 'source' | 'invite',
  reason: string,
): HttpResponse =>
  noStoreJsonResponse(
    {
      error: `referral_${kind}_unavailable`,
      reason,
    },
    { status: reason === 'unknown' ? 404 : 410 },
  )

const captureResponse = (
  attribution: ReferralAttributionRecord,
): HttpResponse =>
  redirectResponse(referralCaptureRedirectLocation(attribution.target), [
    serializeCookie(
      PENDING_REFERRAL_COOKIE,
      attribution.id,
      PENDING_REFERRAL_MAX_AGE_SECONDS,
    ),
  ])

const sourceCaptureResponse = async (
  request: Request,
  env: SiteReferralRouteEnv,
  publicSourceRef: string,
): Promise<HttpResponse> => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return methodNotAllowed(['GET', 'HEAD'])
  }

  if (!SAFE_REF_PATTERN.test(publicSourceRef)) {
    return unavailableResponse('source', 'unknown')
  }

  const url = new URL(request.url)
  const nowIso = currentIsoTimestamp()
  const database = openAgentsDatabase(env)

  const source = await findSourceByPublicRef(database, publicSourceRef)
  const sourceUnavailable = referralSourceUnavailableReason(source)

  if (sourceUnavailable !== undefined || source === null) {
    return unavailableResponse('source', sourceUnavailable ?? 'unknown')
  }

  return captureResponse(
    await createPendingAttribution(database, {
      capturePath: capturePathFromUrl(url),
      expiresAt: expiresAtFromNow(),
      invite: null,
      nowIso,
      source,
      target: targetFromUrl(url, 'home'),
    }),
  )
}

const inviteCaptureResponse = async (
  request: Request,
  env: SiteReferralRouteEnv,
  publicInviteRef: string,
): Promise<HttpResponse> => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return methodNotAllowed(['GET', 'HEAD'])
  }

  if (!SAFE_REF_PATTERN.test(publicInviteRef)) {
    return unavailableResponse('invite', 'unknown')
  }

  const nowIso = currentIsoTimestamp()
  const database = openAgentsDatabase(env)

  const invite = await findInviteByPublicRef(database, publicInviteRef)
  const inviteUnavailable = referralInviteUnavailableReason(invite, nowIso)

  if (inviteUnavailable !== undefined || invite === null) {
    return unavailableResponse('invite', inviteUnavailable ?? 'unknown')
  }

  const source = await findSourceById(database, invite.referralSourceId)

  if (source === null) {
    return unavailableResponse('source', 'unknown')
  }

  const sourceUnavailable = referralSourceUnavailableReason(source)

  if (sourceUnavailable !== undefined) {
    return unavailableResponse('source', sourceUnavailable)
  }

  return captureResponse(
    await createPendingAttribution(database, {
      capturePath: invite.audiencePath,
      expiresAt: expiresAtFromNow(),
      invite,
      nowIso,
      source,
      target: referralInviteTarget(invite.scope),
    }),
  )
}

export const makeSiteReferralRoutes = () => ({
  routeSiteReferralRequest: (
    request: Request,
    env: SiteReferralRouteEnv,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const sourceMatch = /^\/r\/site\/([^/]+)$/.exec(url.pathname)

    if (sourceMatch !== null) {
      return Effect.promise(() =>
        sourceCaptureResponse(
          request,
          env,
          decodeURIComponent(sourceMatch[1] ?? ''),
        ),
      )
    }

    const inviteMatch = /^\/r\/invite\/([^/]+)$/.exec(url.pathname)

    if (inviteMatch === null) {
      return undefined
    }

    return Effect.promise(() =>
      inviteCaptureResponse(
        request,
        env,
        decodeURIComponent(inviteMatch[1] ?? ''),
      ),
    )
  },

  referralCookieName: PENDING_REFERRAL_COOKIE,

  publicReferralAttribution,
})
