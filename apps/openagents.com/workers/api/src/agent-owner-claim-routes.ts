import { badRequest, notFound } from '@openagentsinc/sync-worker'
import { Effect, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  ProgrammaticAgentRegistrationRequest,
  authenticateProgrammaticAgent,
  buildProgrammaticAgentRegistrationRecord,
  createAgentToken,
  sha256Hex,
} from './agent-registration'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
  unauthorized,
} from './http/responses'
import {
  decodeUnknownWithSchema,
  optionalString,
  parseJsonRecord,
  parseJsonStringArray,
} from './json-boundary'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'
import {
  xClaimRewardEligibilityListResponse,
  xClaimRewardEligibilityStatusResponse,
} from './x-claim-reward-eligibility-routes'
import { assertXClaimRewardSettlementEvidenceRefs } from './x-claim-reward-settlement-evidence'

const CLAIM_TTL_MS = 1000 * 60 * 60 * 48
const DEFAULT_CREDENTIAL_TTL_MS = 1000 * 60 * 60 * 24 * 90

const RejectAgentOwnerClaimRequest = S.Struct({
  reason: S.optionalKey(S.Trim.check(S.isMaxLength(500))),
})

type RejectAgentOwnerClaimRequest = typeof RejectAgentOwnerClaimRequest.Type

const StartXOwnerClaimRequest = S.Struct({
  xHandle: S.optionalKey(S.Trim.check(S.isMaxLength(32))),
})

type StartXOwnerClaimRequest = typeof StartXOwnerClaimRequest.Type

const VerifyXOwnerClaimRequest = S.Struct({
  tweetUrl: S.Trim.check(S.isNonEmpty(), S.isMaxLength(500)),
})

type VerifyXOwnerClaimRequest = typeof VerifyXOwnerClaimRequest.Type

type AgentOwnerClaimStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'expired'
  | 'revoked'

type XOwnerClaimState =
  | 'pending_owner_session'
  | 'pending_x_connection'
  | 'pending_tweet'
  | 'verified'
  | 'approved'
  | 'rejected'
  | 'expired'
  | 'revoked'

type XVerificationTweetState =
  | 'visible'
  | 'deleted'
  | 'hidden'
  | 'edited'
  | 'suspended'
  | 'unavailable'

export type AgentOwnerClaimRecord = Readonly<{
  id: string
  claimTokenHash: string
  claimTokenPrefix: string
  status: AgentOwnerClaimStatus
  displayName: string
  slug: string | null
  externalId: string | null
  primaryEmail: string | null
  metadataJson: string
  ownerUserId: string | null
  agentUserId: string | null
  credentialId: string | null
  tokenPrefix: string | null
  receiptRef: string
  requestedAt: string
  expiresAt: string
  decidedAt: string | null
  tokenIssuedAt: string | null
  rejectedReason: string | null
  createdAt: string
  updatedAt: string
}>

export type XOwnerClaimChallengeRecord = Readonly<{
  id: string
  agentClaimId: string
  ownerUserId: string
  agentUserId: string | null
  xAccountRef: string
  xHandle: string
  nonce: string
  requiredText: string
  requiredUrl: string
  state: XOwnerClaimState
  receiptRef: string
  tweetRef: string | null
  tweetUrl: string | null
  policyRefsJson: string
  caveatRefsJson: string
  rejectedReason: string | null
  createdAt: string
  expiresAt: string
  verifiedAt: string | null
  updatedAt: string
}>

type XOwnerClaimChallengeRow = Readonly<{
  id: string
  agent_claim_id: string
  owner_user_id: string
  agent_user_id: string | null
  x_account_ref: string
  x_handle: string
  nonce: string
  required_text: string
  required_url: string
  state: XOwnerClaimState
  receipt_ref: string
  tweet_ref: string | null
  tweet_url: string | null
  policy_refs_json: string
  caveat_refs_json: string
  rejected_reason: string | null
  created_at: string
  expires_at: string
  verified_at: string | null
  updated_at: string
}>

export type XVerificationTweetLookup = Readonly<{
  authorHandle: string | null
  htmlText: string | null
  state: XVerificationTweetState
  tweetRef: string
  tweetUrl: string
}>

export type VerifiedPublicIdentityClaim = Readonly<{
  agentClaimRef: string
  claimRef: string
  ownerUserId: string
  provider: 'x'
  receiptRef: string
  state: 'verified' | 'approved'
  tweetRef: string
  xAccountRef: string
}>

type AgentOwnerClaimRow = Readonly<{
  id: string
  claim_token_hash: string
  claim_token_prefix: string
  status: AgentOwnerClaimStatus
  display_name: string
  slug: string | null
  external_id: string | null
  primary_email: string | null
  metadata_json: string
  owner_user_id: string | null
  agent_user_id: string | null
  credential_id: string | null
  token_prefix: string | null
  receipt_ref: string
  requested_at: string
  expires_at: string
  decided_at: string | null
  token_issued_at: string | null
  rejected_reason: string | null
  created_at: string
  updated_at: string
}>

type AgentOwnerClaimSession = Readonly<{
  user: Readonly<{
    email: string
    login?: string
    name: string
    userId: string
  }>
}>

type HttpResponse = globalThis.Response

export type XClaimRewardState =
  | 'dispatch_requested'
  | 'dispatched'
  | 'eligible'
  | 'failed'
  | 'refused'
  | 'settled'

export type XClaimRewardRecord = Readonly<{
  agentUserId: string | null
  amountSats: number
  challengeId: string
  claimId: string
  createdAt: string
  evidenceRefs: ReadonlyArray<string>
  id: string
  ownerUserId: string
  receiptRef: string
  state: XClaimRewardState
  stateReasonRef: string | null
  treasuryPaymentId: string | null
  updatedAt: string
  xAccountRef: string
}>

export const X_CLAIM_REWARD_AMOUNT_SATS = 1000
export const X_CLAIM_REWARD_CAMPAIGN_CAP = 100

export type AgentOwnerClaimStore = Readonly<{
  countXClaimRewards: () => Promise<number>
  createXClaimReward: (
    record: XClaimRewardRecord,
  ) => Promise<XClaimRewardRecord>
  listXClaimRewards: (
    limit: number,
  ) => Promise<ReadonlyArray<XClaimRewardRecord>>
  readXClaimRewardByChallengeId: (
    challengeId: string,
  ) => Promise<XClaimRewardRecord | undefined>
  readXClaimRewardById: (
    rewardId: string,
  ) => Promise<XClaimRewardRecord | undefined>
  readXClaimRewardByReceiptRef: (
    receiptRef: string,
  ) => Promise<XClaimRewardRecord | undefined>
  updateXClaimRewardState: (input: {
    evidenceRefs: ReadonlyArray<string>
    now: string
    rewardId: string
    stateReasonRef: string | null
    toState: XClaimRewardState
  }) => Promise<XClaimRewardRecord | undefined>
  approveClaim: (input: {
    claimId: string
    credentialExpiresAt: string
    decidedAt: string
    ownerUserId: string
    registration: AgentRegistrationRecord
  }) => Promise<AgentOwnerClaimRecord | undefined>
  attachClaimApproval: (input: {
    claimId: string
    decidedAt: string
    ownerUserId: string
  }) => Promise<AgentOwnerClaimRecord | undefined>
  createClaim: (record: AgentOwnerClaimRecord) => Promise<void>
  expireClaim: (
    claimId: string,
    now: string,
  ) => Promise<AgentOwnerClaimRecord | undefined>
  readClaimById: (claimId: string) => Promise<AgentOwnerClaimRecord | undefined>
  rejectClaim: (input: {
    claimId: string
    decidedAt: string
    ownerUserId: string
    reason: string | null
  }) => Promise<AgentOwnerClaimRecord | undefined>
  createXChallenge: (
    record: XOwnerClaimChallengeRecord,
  ) => Promise<XOwnerClaimChallengeRecord>
  readActiveXChallengeByClaimId: (
    claimId: string,
  ) => Promise<XOwnerClaimChallengeRecord | undefined>
  readXChallengeById: (
    challengeId: string,
  ) => Promise<XOwnerClaimChallengeRecord | undefined>
  readVerifiedPublicIdentityForAgentUserId: (
    agentUserId: string,
  ) => Promise<VerifiedPublicIdentityClaim | undefined>
  rejectXChallenge: (input: {
    challengeId: string
    now: string
    reason: string
  }) => Promise<XOwnerClaimChallengeRecord | undefined>
  verifyXChallenge: (input: {
    challengeId: string
    now: string
    tweetRef: string
    tweetUrl: string
    xAccountRef: string
    xHandle: string
  }) => Promise<XOwnerClaimChallengeRecord | undefined>
}>

type AgentOwnerClaimRouteDependencies<
  Session extends AgentOwnerClaimSession,
  Bindings,
> = Readonly<{
  agentStore?: (env: Bindings) => AgentRegistrationStore
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  rewardCampaignCap?: number
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  appOrigin: (env: Bindings) => string
  claimTtlMs?: number
  credentialTtlMs?: number
  makeStore: (env: Bindings) => AgentOwnerClaimStore
  makeToken?: () => string
  makeUuid?: () => string
  nowIso?: () => string
  resolveXVerificationTweet?: (
    input: Readonly<{ tweetUrl: string }>,
  ) => Promise<XVerificationTweetLookup>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

const rowToClaim = (row: AgentOwnerClaimRow): AgentOwnerClaimRecord => ({
  agentUserId: row.agent_user_id,
  claimTokenHash: row.claim_token_hash,
  claimTokenPrefix: row.claim_token_prefix,
  createdAt: row.created_at,
  credentialId: row.credential_id,
  decidedAt: row.decided_at,
  displayName: row.display_name,
  expiresAt: row.expires_at,
  externalId: row.external_id,
  id: row.id,
  metadataJson: row.metadata_json,
  ownerUserId: row.owner_user_id,
  primaryEmail: row.primary_email,
  receiptRef: row.receipt_ref,
  rejectedReason: row.rejected_reason,
  requestedAt: row.requested_at,
  slug: row.slug,
  status: row.status,
  tokenIssuedAt: row.token_issued_at,
  tokenPrefix: row.token_prefix,
  updatedAt: row.updated_at,
})

const rowToXChallenge = (
  row: XOwnerClaimChallengeRow,
): XOwnerClaimChallengeRecord => ({
  agentClaimId: row.agent_claim_id,
  agentUserId: row.agent_user_id,
  caveatRefsJson: row.caveat_refs_json,
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  id: row.id,
  nonce: row.nonce,
  ownerUserId: row.owner_user_id,
  policyRefsJson: row.policy_refs_json,
  receiptRef: row.receipt_ref,
  rejectedReason: row.rejected_reason,
  requiredText: row.required_text,
  requiredUrl: row.required_url,
  state: row.state,
  tweetRef: row.tweet_ref,
  tweetUrl: row.tweet_url,
  updatedAt: row.updated_at,
  verifiedAt: row.verified_at,
  xAccountRef: row.x_account_ref,
  xHandle: row.x_handle,
})

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const isUniqueConstraintError = (error: unknown): boolean =>
  errorMessage(error).includes('UNIQUE constraint failed')

const readBearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')?.trim()

  if (authorization === undefined) {
    return undefined
  }

  const match = /^Bearer\s+(.+)$/i.exec(authorization)

  return match?.[1]?.trim() || undefined
}

const readClaimToken = (request: Request): string | undefined =>
  optionalString(request.headers.get('x-openagents-claim-token')) ??
  readBearerToken(request)

const safeJsonObject = (value: string): Record<string, unknown> => {
  return parseJsonRecord(value) ?? {}
}

const publicStringArray = (value: string): ReadonlyArray<string> => {
  return parseJsonStringArray(value)
}

const claimPagePath = (claimId: string): string =>
  `/agents/claims/${encodeURIComponent(claimId)}`

const claimLoginPath = (claimId: string): string =>
  `/login/github?returnTo=${encodeURIComponent(claimPagePath(claimId))}`

const normalizeXHandle = (value: string): string =>
  value.trim().replace(/^@+/, '').toLowerCase()

const xAccountRefForHandle = (value: string): string =>
  `x:${normalizeXHandle(value)}`

const pendingXAccountRefForChallenge = (challengeId: string): string =>
  `x:pending:${challengeId}`

const friendlyXVerificationText = (input: {
  code: string
  displayName: string
}): string => `Verifying my agent ${input.displayName} is joining @OpenAgents

Code: ${input.code}`

const xIntentPostUrl = (text: string): string => {
  const url = new URL('https://x.com/intent/post')
  url.searchParams.set('text', text)

  return url.toString()
}

const tweetRefFromUrl = (value: string): string | undefined => {
  const match = /\/status\/([0-9]+)/.exec(value)

  return match?.[1] === undefined ? undefined : `x_tweet:${match[1]}`
}

const normalizeTweetUrl = (value: string): string | undefined => {
  let url: URL

  try {
    url = new URL(value)
  } catch {
    return undefined
  }

  const hostname = url.hostname.toLowerCase().replace(/^www\./, '')

  if (hostname !== 'x.com' && hostname !== 'twitter.com') {
    return undefined
  }

  const match = /^\/([^/]+)\/status\/([0-9]+)/.exec(url.pathname)

  if (match === null) {
    return undefined
  }

  return `https://x.com/${encodeURIComponent(match[1] ?? '')}/status/${
    match[2] ?? ''
  }`
}

const xClaimProjection = (record: XOwnerClaimChallengeRecord) => ({
  agentClaimRef: record.agentClaimId,
  agentUserRef:
    record.agentUserId === null ? null : `agent:${record.agentUserId}`,
  caveatRefs: publicStringArray(record.caveatRefsJson),
  claimRef: record.id,
  expiresAt: record.expiresAt,
  nonce: record.nonce,
  postIntentUrl: xIntentPostUrl(record.requiredText),
  ownerRef: `owner:${record.ownerUserId}`,
  policyRefs: publicStringArray(record.policyRefsJson),
  rejectedReason: record.rejectedReason,
  receiptRef: record.receiptRef,
  requiredText: record.requiredText,
  requiredUrl: record.requiredUrl,
  state: record.state,
  tweetRef: record.tweetRef,
  tweetUrl: record.tweetUrl,
  xAccountRef: record.xAccountRef,
  xHandle: record.xHandle,
})

const xClaimChallengeResponse = (
  record: XOwnerClaimChallengeRecord,
  status = 200,
  extra: Record<string, unknown> = {},
) =>
  noStoreJsonResponse(
    {
      ...extra,
      xClaim: xClaimProjection(record),
      verification: {
        instructions: [
          'Publish the required text from the X account you want bound to this claim.',
          'Return with the public tweet URL and OpenAgents will verify the code and bind the author from the tweet.',
          'Old-format tweets that include the nonce and claim URL still verify during the transition window.',
          'Do not send X OAuth tokens or private account material to this endpoint.',
        ],
      },
    },
    { status },
  )

const escapeHtml = (value: string): string =>
  value.replace(/[&<>"']/g, character => {
    switch (character) {
      case '&':
        return '&amp;'
      case '<':
        return '&lt;'
      case '>':
        return '&gt;'
      case '"':
        return '&quot;'
      case "'":
        return '&#39;'
      default:
        return character
    }
  })

const claimRegistrationRequest = (
  claim: AgentOwnerClaimRecord,
): ProgrammaticAgentRegistrationRequest => ({
  displayName: claim.displayName,
  ...(claim.externalId === null ? {} : { externalId: claim.externalId }),
  metadata: {
    ...safeJsonObject(claim.metadataJson),
    defaultScopes: [
      'agent.profile.read',
      'agent.notifications.read',
      'forum.bookmark',
      'forum.follow',
      'forum.watch',
    ],
    ownerClaim: {
      claimId: claim.id,
      receiptRef: claim.receiptRef,
    },
  },
  ...(claim.primaryEmail === null ? {} : { primaryEmail: claim.primaryEmail }),
  ...(claim.slug === null ? {} : { slug: claim.slug }),
})

const claimProjection = (claim: AgentOwnerClaimRecord, appOrigin: string) => ({
  agentUserRef:
    claim.agentUserId === null ? null : `agent:${claim.agentUserId}`,
  approveUrl: `${appOrigin}/api/agents/claims/${encodeURIComponent(claim.id)}/approve`,
  claimUrl: `${appOrigin}${claimPagePath(claim.id)}`,
  credential: {
    expiresAt: claim.status === 'approved' ? claim.expiresAt : null,
    issuedAt: claim.tokenIssuedAt,
    tokenPrefix: claim.tokenPrefix,
  },
  displayName: claim.displayName,
  expiresAt: claim.expiresAt,
  id: claim.id,
  ownerUserRef:
    claim.ownerUserId === null ? null : `owner:${claim.ownerUserId}`,
  receiptRef: claim.receiptRef,
  rejectUrl: `${appOrigin}/api/agents/claims/${encodeURIComponent(claim.id)}/reject`,
  rejectedReason: claim.rejectedReason,
  requestedAt: claim.requestedAt,
  slug: claim.slug,
  status: claim.status,
  statusUrl: `${appOrigin}/api/agents/claims/${encodeURIComponent(claim.id)}`,
})

const claimResponse = (
  claim: AgentOwnerClaimRecord,
  appOrigin: string,
  extra: Record<string, unknown> = {},
  status = 200,
) =>
  noStoreJsonResponse(
    {
      claim: claimProjection(claim, appOrigin),
      ...extra,
    },
    { status },
  )

const normalizeClaimStatus = async (
  store: AgentOwnerClaimStore,
  claim: AgentOwnerClaimRecord,
  now: string,
): Promise<AgentOwnerClaimRecord> => {
  if (claim.status !== 'pending' || claim.expiresAt > now) {
    return claim
  }

  return (
    (await store.expireClaim(claim.id, now)) ?? {
      ...claim,
      status: 'expired',
      updatedAt: now,
    }
  )
}

const decodeRegistrationBody = async (
  request: Request,
): Promise<ProgrammaticAgentRegistrationRequest | HttpResponse> => {
  const body = await request.json().catch(error => ({
    parseError: errorMessage(error),
  }))

  try {
    return decodeUnknownWithSchema(ProgrammaticAgentRegistrationRequest, body)
  } catch (error) {
    return badRequest(errorMessage(error))
  }
}

export const makeD1AgentOwnerClaimStore = (
  db: D1Database,
): AgentOwnerClaimStore => {
  const readClaimById = async (
    claimId: string,
  ): Promise<AgentOwnerClaimRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT *
         FROM agent_owner_claims
         WHERE id = ?`,
      )
      .bind(claimId)
      .first<AgentOwnerClaimRow>()

    return row === null ? undefined : rowToClaim(row)
  }

  const readXChallengeById = async (
    challengeId: string,
  ): Promise<XOwnerClaimChallengeRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT *
         FROM agent_owner_x_claim_challenges
         WHERE id = ?`,
      )
      .bind(challengeId)
      .first<XOwnerClaimChallengeRow>()

    return row === null ? undefined : rowToXChallenge(row)
  }

  const rewardFromRow = (row: Record<string, unknown>): XClaimRewardRecord => ({
    agentUserId: (row.agent_user_id as string | null) ?? null,
    amountSats: Number(row.amount_sats),
    challengeId: String(row.challenge_id),
    claimId: String(row.claim_id),
    createdAt: String(row.created_at),
    evidenceRefs: parseJsonStringArray(String(row.evidence_refs_json ?? '[]')),
    id: String(row.id),
    ownerUserId: String(row.owner_user_id),
    receiptRef: String(row.receipt_ref),
    state: String(row.state) as XClaimRewardState,
    stateReasonRef: (row.state_reason_ref as string | null) ?? null,
    treasuryPaymentId: (row.treasury_payment_id as string | null) ?? null,
    updatedAt: String(row.updated_at),
    xAccountRef: String(row.x_account_ref),
  })

  const readRewardByChallengeId = async (
    challengeId: string,
  ): Promise<XClaimRewardRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT * FROM x_claim_reward_ledger WHERE challenge_id = ? LIMIT 1`,
      )
      .bind(challengeId)
      .first<Record<string, unknown>>()

    return row === null ? undefined : rewardFromRow(row)
  }

  return {
    countXClaimRewards: async () => {
      const row = await db
        .prepare(
          `SELECT COUNT(*) AS reward_count
           FROM x_claim_reward_ledger
           WHERE state NOT IN ('refused', 'failed')`,
        )
        .first<Record<string, unknown>>()

      return row === null ? 0 : Number(row.reward_count)
    },
    createXClaimReward: async record => {
      await db
        .prepare(
          `INSERT OR IGNORE INTO x_claim_reward_ledger (
            id, challenge_id, claim_id, owner_user_id, agent_user_id,
            x_account_ref, amount_sats, state, state_reason_ref, receipt_ref,
            evidence_refs_json, treasury_payment_id, created_at, updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.challengeId,
          record.claimId,
          record.ownerUserId,
          record.agentUserId,
          record.xAccountRef,
          record.amountSats,
          record.state,
          record.stateReasonRef,
          record.receiptRef,
          JSON.stringify(record.evidenceRefs),
          record.treasuryPaymentId,
          record.createdAt,
          record.updatedAt,
        )
        .run()

      return (await readRewardByChallengeId(record.challengeId)) ?? record
    },
    listXClaimRewards: async limit => {
      const result = await db
        .prepare(
          `SELECT * FROM x_claim_reward_ledger
           ORDER BY created_at DESC, id DESC
           LIMIT ?`,
        )
        .bind(limit)
        .all<Record<string, unknown>>()

      return (result.results ?? []).map(rewardFromRow)
    },
    readXClaimRewardByChallengeId: readRewardByChallengeId,
    readXClaimRewardById: async rewardId => {
      const row = await db
        .prepare(`SELECT * FROM x_claim_reward_ledger WHERE id = ? LIMIT 1`)
        .bind(rewardId)
        .first<Record<string, unknown>>()

      return row === null ? undefined : rewardFromRow(row)
    },
    readXClaimRewardByReceiptRef: async receiptRef => {
      const row = await db
        .prepare(
          `SELECT * FROM x_claim_reward_ledger WHERE receipt_ref = ? LIMIT 1`,
        )
        .bind(receiptRef)
        .first<Record<string, unknown>>()

      return row === null ? undefined : rewardFromRow(row)
    },
    updateXClaimRewardState: async input => {
      await db
        .prepare(
          `UPDATE x_claim_reward_ledger
              SET state = ?,
                  state_reason_ref = ?,
                  evidence_refs_json = ?,
                  updated_at = ?
            WHERE id = ?`,
        )
        .bind(
          input.toState,
          input.stateReasonRef,
          JSON.stringify(input.evidenceRefs),
          input.now,
          input.rewardId,
        )
        .run()

      const row = await db
        .prepare(`SELECT * FROM x_claim_reward_ledger WHERE id = ? LIMIT 1`)
        .bind(input.rewardId)
        .first<Record<string, unknown>>()

      return row === null ? undefined : rewardFromRow(row)
    },
    attachClaimApproval: async input => {
      await db
        .prepare(
          `UPDATE agent_owner_claims
              SET status = 'approved',
                  owner_user_id = ?,
                  decided_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND status = 'pending'
              AND agent_user_id IS NOT NULL`,
        )
        .bind(
          input.ownerUserId,
          input.decidedAt,
          input.decidedAt,
          input.claimId,
        )
        .run()

      return readClaimById(input.claimId)
    },
    approveClaim: async input => {
      await db.batch([
        db
          .prepare(
            `INSERT INTO users
            (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.registration.user.id,
            input.registration.user.kind,
            input.registration.user.displayName,
            input.registration.user.primaryEmail,
            input.registration.user.avatarUrl,
            input.registration.user.status,
            input.registration.user.createdAt,
            input.registration.user.updatedAt,
          ),
        db
          .prepare(
            `INSERT INTO auth_identities
            (id, user_id, provider, provider_subject, email, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.registration.identity.id,
            input.registration.identity.userId,
            input.registration.identity.provider,
            input.registration.identity.providerSubject,
            input.registration.identity.email,
            input.registration.identity.createdAt,
            input.registration.identity.updatedAt,
          ),
        db
          .prepare(
            `INSERT INTO agent_profiles
            (user_id, slug, metadata_json, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?)`,
          )
          .bind(
            input.registration.profile.userId,
            input.registration.profile.slug,
            input.registration.profile.metadataJson,
            input.registration.profile.createdAt,
            input.registration.profile.updatedAt,
          ),
        db
          .prepare(
            `INSERT INTO agent_credentials
            (id, user_id, openauth_user_id, token_hash, token_prefix, name, status, created_at, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.registration.credential.id,
            input.registration.credential.userId,
            input.ownerUserId,
            input.registration.credential.tokenHash,
            input.registration.credential.tokenPrefix,
            input.registration.credential.name,
            input.registration.credential.status,
            input.registration.credential.createdAt,
            input.credentialExpiresAt,
          ),
        db
          .prepare(
            `INSERT INTO openauth_agent_links
              (id, openauth_user_id, agent_user_id, agent_credential_id,
               link_kind, status, created_at, updated_at, revoked_at)
             VALUES (?, ?, ?, ?, 'claim_approval', 'active', ?, ?, NULL)
             ON CONFLICT(openauth_user_id, agent_user_id, agent_credential_id)
             DO UPDATE SET
               link_kind = 'claim_approval',
               status = 'active',
               updated_at = excluded.updated_at,
               revoked_at = NULL`,
          )
          .bind(
            `openauth_agent_link_${input.claimId}`,
            input.ownerUserId,
            input.registration.user.id,
            input.registration.credential.id,
            input.decidedAt,
            input.decidedAt,
          ),
        db
          .prepare(
            `UPDATE agent_owner_claims
           SET status = 'approved',
               owner_user_id = ?,
               agent_user_id = ?,
               credential_id = ?,
               token_prefix = ?,
               expires_at = ?,
               decided_at = ?,
               token_issued_at = ?,
               updated_at = ?
           WHERE id = ?
             AND status = 'pending'`,
          )
          .bind(
            input.ownerUserId,
            input.registration.user.id,
            input.registration.credential.id,
            input.registration.credential.tokenPrefix,
            input.credentialExpiresAt,
            input.decidedAt,
            input.decidedAt,
            input.decidedAt,
            input.claimId,
          ),
      ])

      return readClaimById(input.claimId)
    },
    createClaim: async record => {
      await db
        .prepare(
          `INSERT INTO agent_owner_claims
          (id, claim_token_hash, claim_token_prefix, status, display_name,
           slug, external_id, primary_email, metadata_json, owner_user_id,
           agent_user_id, credential_id, token_prefix, receipt_ref,
           requested_at, expires_at, decided_at, token_issued_at,
           rejected_reason, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.claimTokenHash,
          record.claimTokenPrefix,
          record.status,
          record.displayName,
          record.slug,
          record.externalId,
          record.primaryEmail,
          record.metadataJson,
          record.ownerUserId,
          record.agentUserId,
          record.credentialId,
          record.tokenPrefix,
          record.receiptRef,
          record.requestedAt,
          record.expiresAt,
          record.decidedAt,
          record.tokenIssuedAt,
          record.rejectedReason,
          record.createdAt,
          record.updatedAt,
        )
        .run()
    },
    createXChallenge: async record => {
      await db
        .prepare(
          `INSERT INTO agent_owner_x_claim_challenges
          (id, agent_claim_id, owner_user_id, agent_user_id, x_account_ref,
           x_handle, nonce, required_text, required_url, state, receipt_ref,
           tweet_ref, tweet_url, policy_refs_json, caveat_refs_json,
           rejected_reason, created_at, expires_at, verified_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.agentClaimId,
          record.ownerUserId,
          record.agentUserId,
          record.xAccountRef,
          record.xHandle,
          record.nonce,
          record.requiredText,
          record.requiredUrl,
          record.state,
          record.receiptRef,
          record.tweetRef,
          record.tweetUrl,
          record.policyRefsJson,
          record.caveatRefsJson,
          record.rejectedReason,
          record.createdAt,
          record.expiresAt,
          record.verifiedAt,
          record.updatedAt,
        )
        .run()

      return (await readXChallengeById(record.id)) ?? record
    },
    expireClaim: async (claimId, now) => {
      await db
        .prepare(
          `UPDATE agent_owner_claims
         SET status = 'expired',
             updated_at = ?
         WHERE id = ?
           AND status = 'pending'
           AND expires_at <= ?`,
        )
        .bind(now, claimId, now)
        .run()

      return readClaimById(claimId)
    },
    readClaimById,
    readActiveXChallengeByClaimId: async claimId => {
      const row = await db
        .prepare(
          `SELECT *
           FROM agent_owner_x_claim_challenges
           WHERE agent_claim_id = ?
             AND state IN ('pending_owner_session', 'pending_x_connection', 'pending_tweet', 'verified', 'approved')
           ORDER BY created_at DESC
           LIMIT 1`,
        )
        .bind(claimId)
        .first<XOwnerClaimChallengeRow>()

      return row === null ? undefined : rowToXChallenge(row)
    },
    readXChallengeById,
    readVerifiedPublicIdentityForAgentUserId: async agentUserId => {
      const row = await db
        .prepare(
          `SELECT *
           FROM agent_owner_x_claim_challenges
           WHERE agent_user_id = ?
             AND state IN ('verified', 'approved')
           ORDER BY verified_at DESC, updated_at DESC
           LIMIT 1`,
        )
        .bind(agentUserId)
        .first<XOwnerClaimChallengeRow>()

      if (row === null || row.tweet_ref === null) {
        return undefined
      }

      return {
        agentClaimRef: row.agent_claim_id,
        claimRef: row.id,
        ownerUserId: row.owner_user_id,
        provider: 'x',
        receiptRef: row.receipt_ref,
        state: row.state === 'approved' ? 'approved' : 'verified',
        tweetRef: row.tweet_ref,
        xAccountRef: row.x_account_ref,
      }
    },
    rejectXChallenge: async input => {
      await db
        .prepare(
          `UPDATE agent_owner_x_claim_challenges
           SET state = 'rejected',
               rejected_reason = ?,
               updated_at = ?
           WHERE id = ?
             AND state IN ('pending_owner_session', 'pending_x_connection', 'pending_tweet')`,
        )
        .bind(input.reason, input.now, input.challengeId)
        .run()

      return readXChallengeById(input.challengeId)
    },
    rejectClaim: async input => {
      await db
        .prepare(
          `UPDATE agent_owner_claims
         SET status = 'rejected',
             owner_user_id = ?,
             decided_at = ?,
             rejected_reason = ?,
             updated_at = ?
         WHERE id = ?
           AND status = 'pending'`,
        )
        .bind(
          input.ownerUserId,
          input.decidedAt,
          input.reason,
          input.decidedAt,
          input.claimId,
        )
        .run()

      return readClaimById(input.claimId)
    },
    verifyXChallenge: async input => {
      await db
        .prepare(
          `UPDATE agent_owner_x_claim_challenges
           SET state = 'verified',
               x_account_ref = ?,
               x_handle = ?,
               tweet_ref = ?,
               tweet_url = ?,
               verified_at = ?,
               updated_at = ?
           WHERE id = ?
             AND state = 'pending_tweet'`,
        )
        .bind(
          input.xAccountRef,
          input.xHandle,
          input.tweetRef,
          input.tweetUrl,
          input.now,
          input.now,
          input.challengeId,
        )
        .run()

      return readXChallengeById(input.challengeId)
    },
  }
}

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const header = request.headers.get('authorization') ?? ''
  const match = /^Bearer\s+(.+)$/i.exec(header.trim())

  return match?.[1]?.trim()
}

const createClaimResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const parsed = await decodeRegistrationBody(request)

  if (parsed instanceof Response) {
    return parsed
  }

  const now = dependencies.nowIso ?? currentIsoTimestamp
  const makeUuid = dependencies.makeUuid ?? randomUuid
  const makeToken = dependencies.makeToken ?? createAgentToken
  const requestedAt = now()
  const bearerToken = bearerTokenFromRequest(request)
  let existingAgentUserId: string | null = null
  let existingAgentDisplayName: string | null = null

  if (
    bearerToken !== undefined &&
    bearerToken.startsWith(AGENT_TOKEN_PREFIX) &&
    dependencies.agentStore !== undefined
  ) {
    const session = await authenticateProgrammaticAgent(
      dependencies.agentStore(env),
      bearerToken,
      now,
    ).catch(() => undefined)

    if (session === undefined) {
      return unauthorized()
    }

    existingAgentUserId = session.user.id
    existingAgentDisplayName = session.user.displayName
  }

  const token = makeToken()

  if (!token.startsWith(AGENT_TOKEN_PREFIX)) {
    return serverError()
  }

  const claimId = `agent_claim_${makeUuid()}`
  const claim: AgentOwnerClaimRecord = {
    agentUserId: existingAgentUserId,
    claimTokenHash: await sha256Hex(token),
    claimTokenPrefix: token.slice(0, 20),
    createdAt: requestedAt,
    credentialId: null,
    decidedAt: null,
    displayName: existingAgentDisplayName ?? parsed.displayName,
    expiresAt: isoTimestampAfterIso(
      requestedAt,
      dependencies.claimTtlMs ?? CLAIM_TTL_MS,
    ),
    externalId:
      existingAgentUserId === null ? (parsed.externalId ?? null) : null,
    id: claimId,
    metadataJson: JSON.stringify(parsed.metadata ?? {}),
    ownerUserId: null,
    primaryEmail: parsed.primaryEmail ?? null,
    receiptRef: `agent_claim_receipt_${claimId}`,
    rejectedReason: null,
    requestedAt,
    slug: existingAgentUserId === null ? (parsed.slug ?? null) : null,
    status: 'pending',
    tokenIssuedAt: null,
    tokenPrefix: null,
    updatedAt: requestedAt,
  }

  try {
    await dependencies.makeStore(env).createClaim(claim)
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return noStoreJsonResponse(
        {
          error: 'agent_owner_claim_conflict',
          reason:
            'An agent with the same slug or externalId is already registered. Approval creates a new agent identity, so claims must not reuse the slug or externalId of an existing registered agent. To claim an existing agent, send its bearer token on the claim request instead.',
        },
        { status: 409 },
      )
    }

    return serverError()
  }

  return claimResponse(
    claim,
    dependencies.appOrigin(env),
    {
      oneTimePendingAgentToken: token,
      instructions:
        existingAgentUserId === null
          ? [
              'Store the one-time pending token securely. OpenAgents does not store or show it again.',
              'The token is not usable until a signed-in owner approves this claim.',
              'Ask the owner to open the claimUrl or call the approveUrl from an authenticated OpenAgents browser session.',
              'Poll the statusUrl with Authorization: Bearer <oneTimePendingAgentToken> or X-OpenAgents-Claim-Token.',
            ]
          : [
              'This claim attaches to your existing registered agent identity. Your current agent token stays active and no new credential will be issued.',
              'The one-time pending token is for claim status polling only and never becomes a usable credential.',
              'Ask the owner to open the claimUrl or call the approveUrl from an authenticated OpenAgents browser session.',
              'Poll the statusUrl with Authorization: Bearer <oneTimePendingAgentToken> or X-OpenAgents-Claim-Token.',
            ],
    },
    201,
  )
}

const statusClaimResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  claimId: string,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const token = readClaimToken(request)

  if (token === undefined || !token.startsWith(AGENT_TOKEN_PREFIX)) {
    return unauthorized()
  }

  const store = dependencies.makeStore(env)
  const claim = await store.readClaimById(claimId)

  if (claim === undefined) {
    return notFound()
  }

  const tokenHash = await sha256Hex(token)

  if (tokenHash !== claim.claimTokenHash) {
    return unauthorized()
  }

  const current = await normalizeClaimStatus(
    store,
    claim,
    (dependencies.nowIso ?? currentIsoTimestamp)(),
  )

  return claimResponse(current, dependencies.appOrigin(env), {
    usableAgentToken:
      current.status === 'approved'
        ? {
            tokenPrefix: current.tokenPrefix,
            useExistingOneTimePendingAgentToken: true,
          }
        : null,
  })
}

const startXClaimResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  claimId: string,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const body = await request.json().catch(error => ({
    parseError: errorMessage(error),
  }))
  let parsed: StartXOwnerClaimRequest

  try {
    parsed = decodeUnknownWithSchema(StartXOwnerClaimRequest, body)
  } catch (error) {
    return badRequest(errorMessage(error))
  }

  const store = dependencies.makeStore(env)
  const claim = await store.readClaimById(claimId)

  if (claim === undefined) {
    return notFound()
  }

  const now = (dependencies.nowIso ?? currentIsoTimestamp)()
  const current = await normalizeClaimStatus(store, claim, now)

  if (
    current.status !== 'approved' ||
    current.ownerUserId !== session.user.userId
  ) {
    return noStoreJsonResponse(
      { error: 'agent_owner_claim_not_approved_by_session_owner' },
      { status: 403 },
    )
  }

  const existing = await store.readActiveXChallengeByClaimId(current.id)

  if (existing !== undefined) {
    // Challenges minted before the 12-char code format that have not been
    // tweeted yet are superseded with a fresh short code instead of being
    // echoed back; once a tweet is bound or verified the nonce must not move.
    const isPreTweet =
      existing.state === 'pending_owner_session' ||
      existing.state === 'pending_x_connection' ||
      existing.state === 'pending_tweet'

    if (!isPreTweet || existing.nonce.length <= 12) {
      return xClaimChallengeResponse(existing)
    }

    await store.rejectXChallenge({
      challengeId: existing.id,
      now,
      reason: 'superseded_by_short_verification_code',
    })
  }

  const makeUuid = dependencies.makeUuid ?? randomUuid
  const challengeId = `agent_x_claim_${makeUuid()}`
  // Owner-facing verification code; keep the full string at 12 chars max so
  // the tweet copy stays human-friendly. Substring-matched against the tweet,
  // bound to one claim and one challenge, so 7 hex chars of entropy suffice.
  const nonce = `oa-x-${makeUuid().replaceAll('-', '').slice(0, 7)}`
  const requiredUrl = `${dependencies.appOrigin(env)}${claimPagePath(current.id)}`
  const normalizedHandle =
    parsed.xHandle === undefined || parsed.xHandle.trim() === ''
      ? ''
      : normalizeXHandle(parsed.xHandle)
  const requiredText = friendlyXVerificationText({
    code: nonce,
    displayName: current.displayName,
  })
  const record: XOwnerClaimChallengeRecord = {
    agentClaimId: current.id,
    agentUserId: current.agentUserId,
    caveatRefsJson: JSON.stringify([
      'caveat.public_claim.x_tweet_proof_only',
      'caveat.public_claim.reward_dispatch_separate',
    ]),
    createdAt: now,
    expiresAt: isoTimestampAfterIso(
      now,
      dependencies.claimTtlMs ?? CLAIM_TTL_MS,
    ),
    id: challengeId,
    nonce,
    ownerUserId: session.user.userId,
    policyRefsJson: JSON.stringify([
      'policy.public_identity_claim.x.v1',
      'policy.public_identity_claim.one_x_account_per_campaign.v1',
    ]),
    receiptRef: `agent_x_claim_receipt_${challengeId}`,
    rejectedReason: null,
    requiredText,
    requiredUrl,
    state: 'pending_tweet',
    tweetRef: null,
    tweetUrl: null,
    updatedAt: now,
    verifiedAt: null,
    xAccountRef:
      normalizedHandle === ''
        ? pendingXAccountRefForChallenge(challengeId)
        : xAccountRefForHandle(normalizedHandle),
    xHandle: normalizedHandle,
  }

  try {
    return xClaimChallengeResponse(await store.createXChallenge(record), 201)
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return noStoreJsonResponse(
        { error: 'agent_x_claim_conflict' },
        { status: 409 },
      )
    }

    return serverError()
  }
}

const defaultResolveXVerificationTweet = async (
  input: Readonly<{ tweetUrl: string }>,
): Promise<XVerificationTweetLookup> => {
  const normalizedUrl = normalizeTweetUrl(input.tweetUrl)
  const tweetRef = tweetRefFromUrl(input.tweetUrl)

  if (normalizedUrl === undefined || tweetRef === undefined) {
    return {
      authorHandle: null,
      htmlText: null,
      state: 'unavailable',
      tweetRef: 'x_tweet:invalid',
      tweetUrl: input.tweetUrl,
    }
  }

  const url = new URL('https://publish.x.com/oembed')
  url.searchParams.set('url', normalizedUrl)
  url.searchParams.set('omit_script', '1')

  const response = await fetch(url.toString())

  if (!response.ok) {
    return {
      authorHandle: null,
      htmlText: null,
      state: response.status === 404 ? 'deleted' : 'unavailable',
      tweetRef,
      tweetUrl: normalizedUrl,
    }
  }

  const payload = parseJsonRecord(await response.text())
  const authorUrl = optionalString(payload?.author_url)
  const authorHandle = (() => {
    if (authorUrl === undefined) {
      return null
    }

    try {
      return normalizeXHandle(new URL(authorUrl).pathname.replace('/', ''))
    } catch {
      return null
    }
  })()

  return {
    authorHandle,
    htmlText: optionalString(payload?.html) ?? null,
    state: 'visible',
    tweetRef,
    tweetUrl: normalizedUrl,
  }
}

const verifyXClaimResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  claimId: string,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const body = await request.json().catch(error => ({
    parseError: errorMessage(error),
  }))
  let parsed: VerifyXOwnerClaimRequest

  try {
    parsed = decodeUnknownWithSchema(VerifyXOwnerClaimRequest, body)
  } catch (error) {
    return badRequest(errorMessage(error))
  }

  const normalizedTweetUrl = normalizeTweetUrl(parsed.tweetUrl)

  if (normalizedTweetUrl === undefined) {
    return badRequest('X verification tweet URL must be a public X status URL.')
  }

  const store = dependencies.makeStore(env)
  const claim = await store.readClaimById(claimId)

  if (claim === undefined) {
    return notFound()
  }

  const challenge = await store.readActiveXChallengeByClaimId(claimId)

  if (challenge === undefined) {
    return noStoreJsonResponse(
      { error: 'agent_x_claim_challenge_missing' },
      { status: 404 },
    )
  }

  const now = (dependencies.nowIso ?? currentIsoTimestamp)()

  if (
    claim.ownerUserId !== session.user.userId ||
    challenge.ownerUserId !== session.user.userId
  ) {
    return noStoreJsonResponse(
      { error: 'agent_x_claim_wrong_owner' },
      { status: 403 },
    )
  }

  if (challenge.expiresAt <= now) {
    const expired = await store.rejectXChallenge({
      challengeId: challenge.id,
      now,
      reason: 'X claim challenge expired before tweet verification.',
    })

    return xClaimChallengeResponse(expired ?? challenge, 409)
  }

  if (challenge.state === 'verified' || challenge.state === 'approved') {
    return xClaimChallengeResponse(challenge)
  }

  const resolver =
    dependencies.resolveXVerificationTweet ?? defaultResolveXVerificationTweet
  const tweet = await resolver({ tweetUrl: normalizedTweetUrl })

  if (tweet.state !== 'visible') {
    const rejected = await store.rejectXChallenge({
      challengeId: challenge.id,
      now,
      reason: `X verification tweet is ${tweet.state}.`,
    })

    return xClaimChallengeResponse(rejected ?? challenge, 409)
  }

  const verifiedHandle = normalizeXHandle(tweet.authorHandle ?? '')

  if (verifiedHandle === '') {
    const rejected = await store.rejectXChallenge({
      challengeId: challenge.id,
      now,
      reason: 'X verification tweet author could not be determined.',
    })

    return xClaimChallengeResponse(rejected ?? challenge, 409)
  }

  if (challenge.xHandle !== '' && verifiedHandle !== challenge.xHandle) {
    const rejected = await store.rejectXChallenge({
      challengeId: challenge.id,
      now,
      reason:
        'X verification tweet was not published by the challenged account.',
    })

    return xClaimChallengeResponse(rejected ?? challenge, 409)
  }

  const htmlText = tweet.htmlText ?? ''
  const hasCode = htmlText.includes(challenge.nonce)
  const hasLegacyClaimUrl = htmlText.includes(challenge.requiredUrl)
  const isLegacyRequiredText = challenge.requiredText.includes(
    challenge.requiredUrl,
  )

  if (!hasCode || (isLegacyRequiredText && !hasLegacyClaimUrl)) {
    const rejected = await store.rejectXChallenge({
      challengeId: challenge.id,
      now,
      reason: isLegacyRequiredText
        ? 'X verification tweet is missing the required nonce or claim URL.'
        : 'X verification tweet is missing the required code.',
    })

    return xClaimChallengeResponse(rejected ?? challenge, 409)
  }

  try {
    const verified = await store.verifyXChallenge({
      challengeId: challenge.id,
      now,
      tweetRef: tweet.tweetRef,
      tweetUrl: tweet.tweetUrl,
      xAccountRef: xAccountRefForHandle(verifiedHandle),
      xHandle: verifiedHandle,
    })
    const resolved = verified ?? challenge
    const makeUuid = dependencies.makeUuid ?? randomUuid
    const campaignCap =
      dependencies.rewardCampaignCap ?? X_CLAIM_REWARD_CAMPAIGN_CAP
    const budgetExhausted = (await store.countXClaimRewards()) >= campaignCap
    const rewardId = `x_claim_reward_${makeUuid()}`
    const reward = await store.createXClaimReward({
      agentUserId: resolved.agentUserId,
      amountSats: X_CLAIM_REWARD_AMOUNT_SATS,
      challengeId: resolved.id,
      claimId: resolved.agentClaimId,
      createdAt: now,
      evidenceRefs: [resolved.receiptRef],
      id: rewardId,
      ownerUserId: resolved.ownerUserId,
      receiptRef: `x_claim_reward_receipt_${rewardId}`,
      xAccountRef: resolved.xAccountRef,
      state: budgetExhausted ? 'refused' : 'eligible',
      stateReasonRef: budgetExhausted
        ? 'reason.public.x_claim_reward_campaign_budget_exhausted'
        : null,
      treasuryPaymentId: null,
      updatedAt: now,
    })

    return xClaimChallengeResponse(resolved, 200, {
      reward: publicRewardProjection(reward),
    })
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return noStoreJsonResponse(
        { error: 'agent_x_claim_duplicate_tweet_or_account' },
        { status: 409 },
      )
    }

    return serverError()
  }
}

export const publicRewardProjection = (
  reward: XClaimRewardRecord,
): Record<string, unknown> => ({
  amountSats: reward.amountSats,
  authorityBoundary:
    'Reward eligibility is a promotional campaign state. It is not Forum tip settlement, accepted-work payout, or spendable balance until operator-gated dispatch settles with receipts.',
  receiptRef: reward.receiptRef,
  rewardId: reward.id,
  state: reward.state,
  stateReasonRef: reward.stateReasonRef,
})

const XClaimRewardDispatchRequest = S.Struct({
  action: S.Literals([
    'approve_dispatch',
    'mark_dispatched',
    'mark_failed',
    'mark_settled',
    'refuse',
  ]),
  evidenceRefs: S.optionalKey(S.Array(S.Trim.check(S.isMaxLength(300)))),
  stateReasonRef: S.optionalKey(S.Trim.check(S.isMaxLength(300))),
})

const rewardTransitionForAction: Record<
  string,
  Readonly<{ from: ReadonlyArray<XClaimRewardState>; to: XClaimRewardState }>
> = {
  approve_dispatch: { from: ['eligible'], to: 'dispatch_requested' },
  mark_dispatched: { from: ['dispatch_requested'], to: 'dispatched' },
  mark_failed: {
    from: ['dispatch_requested', 'dispatched'],
    to: 'failed',
  },
  mark_settled: { from: ['dispatched'], to: 'settled' },
  refuse: { from: ['eligible', 'dispatch_requested'], to: 'refused' },
}

const dispatchRewardResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  rewardId: string,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  if (
    dependencies.requireAdminApiToken === undefined ||
    !(await dependencies.requireAdminApiToken(request, env))
  ) {
    return unauthorized()
  }

  let parsed: typeof XClaimRewardDispatchRequest.Type

  try {
    parsed = decodeUnknownWithSchema(
      XClaimRewardDispatchRequest,
      await request.json().catch(() => ({})),
    )
  } catch (error) {
    return badRequest(errorMessage(error))
  }

  const store = dependencies.makeStore(env)
  const reward = await store.readXClaimRewardById(rewardId)

  if (reward === undefined) {
    return notFound()
  }

  const transition = rewardTransitionForAction[parsed.action]

  if (transition === undefined || !transition.from.includes(reward.state)) {
    return noStoreJsonResponse(
      {
        error: 'x_claim_reward_invalid_transition',
        reason: `Reward in state ${reward.state} cannot ${parsed.action}.`,
        reward: publicRewardProjection(reward),
      },
      { status: 409 },
    )
  }

  const settlementEvidenceGate =
    transition.to === 'settled'
      ? assertXClaimRewardSettlementEvidenceRefs(parsed.evidenceRefs ?? [])
      : undefined

  if (settlementEvidenceGate !== undefined && !settlementEvidenceGate.ok) {
    return noStoreJsonResponse(
      {
        blockingReasonRefs: settlementEvidenceGate.blockingReasonRefs,
        error: 'x_claim_reward_settlement_requires_evidence',
        reason:
          'mark_settled requires public-safe settlement evidence refs from the dispatch wallet path.',
        reward: publicRewardProjection(reward),
      },
      { status: 400 },
    )
  }

  const now = (dependencies.nowIso ?? currentIsoTimestamp)()
  const updated = await store.updateXClaimRewardState({
    evidenceRefs: [
      ...reward.evidenceRefs,
      ...(settlementEvidenceGate?.acceptedRefs ?? parsed.evidenceRefs ?? []),
    ],
    now,
    rewardId,
    stateReasonRef: parsed.stateReasonRef ?? reward.stateReasonRef,
    toState: transition.to,
  })

  return updated === undefined
    ? serverError()
    : noStoreJsonResponse({ reward: publicRewardProjection(updated) })
}

const approveClaimResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  claimId: string,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const store = dependencies.makeStore(env)
  const claim = await store.readClaimById(claimId)

  if (claim === undefined) {
    return notFound()
  }

  const decidedAt = (dependencies.nowIso ?? currentIsoTimestamp)()
  const current = await normalizeClaimStatus(store, claim, decidedAt)

  if (current.status !== 'pending') {
    return noStoreJsonResponse(
      {
        claim: claimProjection(current, dependencies.appOrigin(env)),
        error: 'agent_owner_claim_not_pending',
      },
      { status: 409 },
    )
  }

  if (current.agentUserId !== null) {
    let attached: AgentOwnerClaimRecord | undefined

    try {
      attached = await store.attachClaimApproval({
        claimId: current.id,
        decidedAt,
        ownerUserId: session.user.userId,
      })
    } catch {
      return serverError()
    }

    if (attached === undefined || attached.status !== 'approved') {
      return noStoreJsonResponse(
        { error: 'agent_owner_claim_not_pending' },
        { status: 409 },
      )
    }

    return dependencies.appendRefreshedSessionCookies(
      claimResponse(attached, dependencies.appOrigin(env), {
        approval: {
          attachedAgentUserRef: `agent:${current.agentUserId}`,
          ownerUserRef: `owner:${session.user.userId}`,
          tokenPrefix: null,
          tokenWasDisplayedAgain: false,
          usableTokenInstruction:
            'This claim attached owner linkage to the existing registered agent. The agent keeps its current token; no new credential was issued.',
        },
      }),
      session,
    )
  }

  const credentialExpiresAt = isoTimestampAfterIso(
    decidedAt,
    dependencies.credentialTtlMs ?? DEFAULT_CREDENTIAL_TTL_MS,
  )
  const registration = buildProgrammaticAgentRegistrationRecord(
    claimRegistrationRequest(current),
    {
      expiresAt: credentialExpiresAt,
      tokenHash: current.claimTokenHash,
      tokenPrefix: current.claimTokenPrefix,
    },
    {
      now: () => decidedAt,
      ...(dependencies.makeUuid === undefined
        ? {}
        : { makeUuid: dependencies.makeUuid }),
    },
  )

  let approved: AgentOwnerClaimRecord | undefined

  try {
    approved = await store.approveClaim({
      claimId: current.id,
      credentialExpiresAt,
      decidedAt,
      ownerUserId: session.user.userId,
      registration,
    })
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return noStoreJsonResponse(
        {
          error: 'agent_owner_claim_conflict',
          reason:
            'An agent with the same slug or externalId is already registered. Approval creates a new agent identity, so claims must not reuse the slug or externalId of an existing registered agent.',
        },
        { status: 409 },
      )
    }

    return serverError()
  }

  if (approved === undefined || approved.status !== 'approved') {
    return noStoreJsonResponse(
      { error: 'agent_owner_claim_not_pending' },
      { status: 409 },
    )
  }

  return dependencies.appendRefreshedSessionCookies(
    claimResponse(approved, dependencies.appOrigin(env), {
      approval: {
        ownerUserRef: `owner:${session.user.userId}`,
        tokenPrefix: approved.tokenPrefix,
        tokenWasDisplayedAgain: false,
        usableTokenInstruction:
          'The original one-time pending token is now active. OpenAgents did not store or redisplay the raw token.',
      },
    }),
    session,
  )
}

const rejectClaimResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  claimId: string,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const body = await request.json().catch(() => ({}))
  let parsed: RejectAgentOwnerClaimRequest

  try {
    parsed = decodeUnknownWithSchema(RejectAgentOwnerClaimRequest, body)
  } catch (error) {
    return badRequest(errorMessage(error))
  }

  const store = dependencies.makeStore(env)
  const claim = await store.readClaimById(claimId)

  if (claim === undefined) {
    return notFound()
  }

  const decidedAt = (dependencies.nowIso ?? currentIsoTimestamp)()
  const current = await normalizeClaimStatus(store, claim, decidedAt)

  if (current.status !== 'pending') {
    return noStoreJsonResponse(
      {
        claim: claimProjection(current, dependencies.appOrigin(env)),
        error: 'agent_owner_claim_not_pending',
      },
      { status: 409 },
    )
  }

  const rejected = await store.rejectClaim({
    claimId: current.id,
    decidedAt,
    ownerUserId: session.user.userId,
    reason: parsed.reason ?? null,
  })

  if (rejected === undefined || rejected.status !== 'rejected') {
    return noStoreJsonResponse(
      { error: 'agent_owner_claim_not_pending' },
      { status: 409 },
    )
  }

  return dependencies.appendRefreshedSessionCookies(
    claimResponse(rejected, dependencies.appOrigin(env)),
    session,
  )
}

const ownerClaimPageResponse = async <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  claimId: string,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const store = dependencies.makeStore(env)
  const claim = await store.readClaimById(claimId)

  if (claim === undefined) {
    return new Response('Agent claim not found.', {
      headers: {
        'cache-control': 'no-store',
        'content-type': 'text/plain; charset=utf-8',
      },
      status: 404,
    })
  }

  const current = await normalizeClaimStatus(
    store,
    claim,
    (dependencies.nowIso ?? currentIsoTimestamp)(),
  )
  const xChallenge =
    current.status === 'approved'
      ? await store.readActiveXChallengeByClaimId(current.id)
      : undefined
  const approvePath = `/api/agents/claims/${encodeURIComponent(current.id)}/approve`
  const rejectPath = `/api/agents/claims/${encodeURIComponent(current.id)}/reject`
  const xChallengePath = `/api/agents/claims/${encodeURIComponent(current.id)}/x/challenge`
  const xVerifyPath = `/api/agents/claims/${encodeURIComponent(current.id)}/x/verify`
  const loginPath = claimLoginPath(current.id)
  const statusText =
    current.status === 'pending'
      ? 'Pending owner review'
      : current.status === 'approved'
        ? 'Approved'
        : current.status === 'rejected'
          ? 'Rejected'
          : current.status === 'expired'
            ? 'Expired'
            : 'Revoked'
  const actionsHtml =
    current.status === 'pending'
      ? `<div class="actions">
      <button id="approve">Approve claim</button>
      <button class="secondary" id="reject">Reject claim</button>
      <a class="button secondary" href="${escapeHtml(loginPath)}">Sign in</a>
    </div>
    <p class="result" id="result"></p>`
      : `<p class="result" id="result">${
          current.status === 'approved'
            ? `Approved. Original pending token is active. Token prefix: ${escapeHtml(current.tokenPrefix ?? 'recorded')}.`
            : `${escapeHtml(statusText)}. This claim cannot be approved.`
        }</p>`
  const xChallengeHtml =
    current.status === 'approved'
      ? `<section class="x-claim" data-x-claim-panel>
      <h2>Verify on X</h2>
      <p>Post a public verification tweet, then paste the tweet URL here. OpenAgents binds the author from the public tweet and never asks for X OAuth tokens.</p>
      <div id="xChallengeExisting"${xChallenge === undefined ? ' hidden' : ''}>
        <pre id="xRequiredText">${escapeHtml(xChallenge?.requiredText ?? '')}</pre>
        <a class="button" id="xIntent" href="${escapeHtml(
          xChallenge === undefined
            ? '#'
            : xIntentPostUrl(xChallenge.requiredText),
        )}" target="_blank" rel="noopener">Post verification tweet</a>
      </div>
      <button id="prepareXClaim"${
        xChallenge === undefined ? '' : ' hidden'
      }>Prepare verification tweet</button>
      <div class="verify-row">
        <label for="tweetUrl">Tweet URL</label>
        <input id="tweetUrl" type="url" inputmode="url" autocomplete="off" placeholder="https://x.com/you/status/..." />
        <button id="verifyXClaim">Verify tweet</button>
      </div>
      <p class="result" id="xResult">${escapeHtml(
        xChallenge === undefined
          ? ''
          : xChallenge.state === 'verified' || xChallenge.state === 'approved'
            ? 'X proof verified.'
            : 'Verification tweet is ready to post.',
      )}</p>
    </section>`
      : ''
  const actionScript =
    current.status === 'pending'
      ? `
    const approvePath = ${JSON.stringify(approvePath)};
    const rejectPath = ${JSON.stringify(rejectPath)};
    const loginPath = ${JSON.stringify(loginPath)};
    const claimRetryKey = ${JSON.stringify(`openagents.claim.${current.id}.pendingAction`)};
    const result = document.getElementById('result');
    const rememberPendingAction = action => {
      try {
        sessionStorage.setItem(claimRetryKey, action);
      } catch (_) {}
    };
    const takePendingAction = () => {
      try {
        const action = sessionStorage.getItem(claimRetryKey);
        sessionStorage.removeItem(claimRetryKey);
        return action;
      } catch (_) {
        return null;
      }
    };
    const request = async (path, init, options = {}) => {
      const response = await fetch(path, init);
      const body = await response.json().catch(() => ({}));
      if (!response.ok) {
        if (response.status === 401) {
          if (options.rememberAction) {
            rememberPendingAction(options.rememberAction);
          }
          result.textContent = 'Opening sign in...';
          window.location.assign(loginPath);
          throw { message: 'Opening sign in. Return here to finish if the browser does not redirect automatically.' };
        }
        throw { message: body.error || 'Request failed. Sign in with the owner account, then retry.' };
      }
      return body;
    };
    const approve = async rememberAction => {
      result.textContent = 'Approving...';
      try {
        const body = await request(approvePath, { method: 'POST' }, { rememberAction });
        result.textContent = 'Approved. Token prefix: ' + (body.approval?.tokenPrefix || body.claim?.credential?.tokenPrefix || 'recorded');
        setTimeout(() => window.location.reload(), 500);
      } catch (error) {
        result.textContent = error.message || String(error);
      }
    };
    document.getElementById('approve')?.addEventListener('click', async () => approve('approve'));
    document.getElementById('reject')?.addEventListener('click', async () => {
      result.textContent = 'Rejecting...';
      try {
        await request(rejectPath, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ reason: 'Rejected from owner claim page.' })
        }, { rememberAction: 'reject' });
        result.textContent = 'Rejected.';
        setTimeout(() => window.location.reload(), 500);
      } catch (error) {
        result.textContent = error.message || String(error);
      }
    });
    if (takePendingAction() === 'approve') {
      approve(undefined);
    }`
      : ''
  const xClaimScript =
    current.status === 'approved'
      ? `
    const xChallengePath = ${JSON.stringify(xChallengePath)};
    const xVerifyPath = ${JSON.stringify(xVerifyPath)};
    const loginPath = ${JSON.stringify(loginPath)};
    const xRetryKey = ${JSON.stringify(`openagents.claim.${current.id}.xAction`)};
    const xResult = document.getElementById('xResult');
    const xRequiredText = document.getElementById('xRequiredText');
    const xIntent = document.getElementById('xIntent');
    const xChallengeExisting = document.getElementById('xChallengeExisting');
    const prepareXClaim = document.getElementById('prepareXClaim');
    const tweetUrl = document.getElementById('tweetUrl');
    const rememberXAction = action => {
      try {
        sessionStorage.setItem(xRetryKey, action);
      } catch (_) {}
    };
    const takeXAction = () => {
      try {
        const action = sessionStorage.getItem(xRetryKey);
        sessionStorage.removeItem(xRetryKey);
        return action;
      } catch (_) {
        return null;
      }
    };
    const xRequest = async (path, init, rememberAction) => {
      const response = await fetch(path, init);
      const body = await response.json().catch(() => ({}));
      if (!response.ok) {
        if (response.status === 401) {
          if (rememberAction) rememberXAction(rememberAction);
          xResult.textContent = 'Opening sign in...';
          window.location.assign(loginPath);
          throw { message: 'Opening sign in. Return here to finish if the browser does not redirect automatically.' };
        }
        throw { message: body.error || body.xClaim?.rejectedReason || 'X verification request failed.' };
      }
      return body;
    };
    const renderXClaim = body => {
      const xClaim = body.xClaim || {};
      if (xRequiredText) xRequiredText.textContent = xClaim.requiredText || '';
      if (xIntent) {
        xIntent.href = xClaim.postIntentUrl || '#';
        xIntent.hidden = !xClaim.postIntentUrl;
      }
      if (xChallengeExisting) xChallengeExisting.hidden = false;
      if (prepareXClaim) prepareXClaim.hidden = true;
      xResult.textContent = xClaim.state === 'verified' || xClaim.state === 'approved'
        ? 'X proof verified.'
        : 'Verification tweet is ready to post.';
    };
    const prepare = async rememberAction => {
      xResult.textContent = 'Preparing verification code...';
      try {
        renderXClaim(await xRequest(xChallengePath, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({})
        }, rememberAction));
      } catch (error) {
        xResult.textContent = error.message || String(error);
      }
    };
    prepareXClaim?.addEventListener('click', async () => prepare('prepare-x'));
    document.getElementById('verifyXClaim')?.addEventListener('click', async () => {
      const value = tweetUrl?.value?.trim() || '';
      if (value === '') {
        xResult.textContent = 'Paste the public tweet URL first.';
        return;
      }
      xResult.textContent = 'Verifying tweet...';
      try {
        renderXClaim(await xRequest(xVerifyPath, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ tweetUrl: value })
        }, 'verify-x'));
      } catch (error) {
        xResult.textContent = error.message || String(error);
      }
    });
    if (takeXAction() === 'prepare-x') {
      prepare(undefined);
    }`
      : ''
  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Agent claim - OpenAgents</title>
  <style>
    :root { color-scheme: dark; }
    body { margin: 0; background: #000; color: #f1efe8; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
    main { width: min(100% - 32px, 760px); margin: 12vh auto; border: 1px solid rgba(255,255,255,.14); padding: 28px; }
    p { color: rgba(241,239,232,.68); line-height: 1.6; }
    .eyebrow { color: rgba(241,239,232,.42); font-size: 12px; letter-spacing: .12em; text-transform: uppercase; }
    h1 { font-size: clamp(32px, 6vw, 58px); line-height: 1; margin: 12px 0 18px; font-weight: 600; }
    dl { display: grid; gap: 0; margin: 24px 0; }
    div.row { display: grid; grid-template-columns: 12rem 1fr; gap: 16px; border-top: 1px solid rgba(255,255,255,.1); padding: 14px 0; }
    dt { color: rgba(241,239,232,.42); text-transform: uppercase; font-size: 12px; letter-spacing: .12em; }
    dd { margin: 0; color: rgba(241,239,232,.82); overflow-wrap: anywhere; }
    button, a.button { border: 1px solid rgba(255,255,255,.2); background: #f1efe8; color: #000; padding: 11px 14px; font: inherit; cursor: pointer; text-decoration: none; display: inline-block; }
    button.secondary, a.button.secondary { background: transparent; color: rgba(241,239,232,.8); }
    button:disabled { opacity: .35; cursor: not-allowed; }
    .actions { display: flex; flex-wrap: wrap; gap: 10px; }
    .x-claim { border-top: 1px solid rgba(255,255,255,.14); margin-top: 26px; padding-top: 24px; }
    h2 { font-size: 20px; margin: 0 0 10px; }
    pre { white-space: pre-wrap; overflow-wrap: anywhere; border: 1px solid rgba(255,255,255,.14); padding: 14px; color: rgba(241,239,232,.86); }
    .verify-row { display: grid; gap: 10px; margin-top: 16px; }
    label { color: rgba(241,239,232,.42); text-transform: uppercase; font-size: 12px; letter-spacing: .12em; }
    input { border: 1px solid rgba(255,255,255,.18); background: transparent; color: #f1efe8; font: inherit; padding: 11px 12px; min-width: 0; }
    .result { min-height: 24px; margin-top: 18px; }
    @media (max-width: 640px) { div.row { grid-template-columns: 1fr; gap: 6px; } main { margin: 24px auto; padding: 20px; } }
  </style>
</head>
<body>
  <main data-agent-claim-page>
    <p class="eyebrow">OpenAgents agent claim</p>
    <h1>${escapeHtml(current.displayName)}</h1>
    <p>This agent requested an OpenAgents identity. Approval activates the original one-time pending token held by the agent. OpenAgents will not show the raw token here.</p>
    <dl>
      <div class="row"><dt>Status</dt><dd>${escapeHtml(statusText)}</dd></div>
      <div class="row"><dt>Claim</dt><dd>${escapeHtml(current.id)}</dd></div>
      <div class="row"><dt>Slug</dt><dd>${escapeHtml(current.slug ?? 'Not provided')}</dd></div>
      <div class="row"><dt>Requested</dt><dd><time data-friendly-time datetime="${escapeHtml(current.requestedAt)}">Loading...</time></dd></div>
      <div class="row"><dt>Expires</dt><dd><time data-friendly-time datetime="${escapeHtml(current.expiresAt)}">Loading...</time></dd></div>
    </dl>
    ${actionsHtml}
    ${xChallengeHtml}
  </main>
  <script>
    const friendlyTime = value => {
      const timestamp = Date.parse(value);
      if (Number.isNaN(timestamp)) return 'Unknown time';
      return new Intl.DateTimeFormat(undefined, {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(timestamp);
    };
    document.querySelectorAll('[data-friendly-time]').forEach(node => {
      node.textContent = friendlyTime(node.getAttribute('datetime'));
    });
    ${actionScript}
    ${xClaimScript}
  </script>
</body>
</html>`

  return new Response(html, {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
    },
  })
}

export const makeAgentOwnerClaimRoutes = <
  Session extends AgentOwnerClaimSession,
  Bindings,
>(
  dependencies: AgentOwnerClaimRouteDependencies<Session, Bindings>,
) => ({
  routeAgentOwnerClaimRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/agents/claims') {
      return Effect.promise(() =>
        createClaimResponse(dependencies, request, env),
      )
    }

    const claimPageMatch = /^\/agents\/claims\/([^/]+)$/.exec(url.pathname)

    if (claimPageMatch !== null) {
      const claimId = decodeURIComponent(claimPageMatch[1] ?? '')

      return Effect.promise(() =>
        ownerClaimPageResponse(dependencies, request, env, claimId),
      )
    }

    const rewardDispatchMatch =
      /^\/api\/agents\/claims\/rewards\/([^/]+)\/dispatch$/.exec(url.pathname)

    if (rewardDispatchMatch !== null) {
      const rewardId = decodeURIComponent(rewardDispatchMatch[1] ?? '')

      return Effect.promise(() =>
        dispatchRewardResponse(dependencies, request, env, rewardId),
      )
    }

    // Public-safe x_claim_reward eligibility read path (#4754): the
    // four-state lifecycle projection for the campaign ledger and for a
    // single reward by id or receipt ref.
    if (url.pathname === '/api/agents/claims/rewards') {
      return Effect.promise(() =>
        xClaimRewardEligibilityListResponse(
          {
            nowIso: dependencies.nowIso ?? currentIsoTimestamp,
            store: dependencies.makeStore(env),
          },
          request,
        ),
      )
    }

    const rewardStatusMatch = /^\/api\/agents\/claims\/rewards\/([^/]+)$/.exec(
      url.pathname,
    )

    if (rewardStatusMatch !== null) {
      const rewardRef = decodeURIComponent(rewardStatusMatch[1] ?? '')

      return Effect.promise(() =>
        xClaimRewardEligibilityStatusResponse(
          {
            nowIso: dependencies.nowIso ?? currentIsoTimestamp,
            store: dependencies.makeStore(env),
          },
          request,
          rewardRef,
        ),
      )
    }

    const xClaimActionMatch =
      /^\/api\/agents\/claims\/([^/]+)\/x\/(challenge|verify)$/.exec(
        url.pathname,
      )

    if (xClaimActionMatch !== null) {
      const claimId = decodeURIComponent(xClaimActionMatch[1] ?? '')
      const action = xClaimActionMatch[2]

      if (action === 'challenge') {
        return Effect.promise(() =>
          startXClaimResponse(dependencies, request, env, ctx, claimId),
        )
      }

      return Effect.promise(() =>
        verifyXClaimResponse(dependencies, request, env, ctx, claimId),
      )
    }

    const claimActionMatch =
      /^\/api\/agents\/claims\/([^/]+)(?:\/(approve|reject))?$/.exec(
        url.pathname,
      )

    if (claimActionMatch === null) {
      return undefined
    }

    const claimId = decodeURIComponent(claimActionMatch[1] ?? '')
    const action = claimActionMatch[2]

    if (action === 'approve') {
      return Effect.promise(() =>
        approveClaimResponse(dependencies, request, env, ctx, claimId),
      )
    }

    if (action === 'reject') {
      return Effect.promise(() =>
        rejectClaimResponse(dependencies, request, env, ctx, claimId),
      )
    }

    return Effect.promise(() =>
      statusClaimResponse(dependencies, request, env, claimId),
    )
  },
})
