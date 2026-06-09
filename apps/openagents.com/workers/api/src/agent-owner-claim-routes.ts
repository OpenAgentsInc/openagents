import { badRequest, notFound } from '@openagents/sync-worker'
import { Effect, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationRecord,
  ProgrammaticAgentRegistrationRequest,
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
} from './json-boundary'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'

const CLAIM_TTL_MS = 1000 * 60 * 60 * 48
const DEFAULT_CREDENTIAL_TTL_MS = 1000 * 60 * 60 * 24 * 90

const RejectAgentOwnerClaimRequest = S.Struct({
  reason: S.optionalKey(S.Trim.check(S.isMaxLength(500))),
})

type RejectAgentOwnerClaimRequest = typeof RejectAgentOwnerClaimRequest.Type

type AgentOwnerClaimStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'expired'
  | 'revoked'

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
    login: string
    name: string
    userId: string
  }>
}>

type HttpResponse = globalThis.Response

export type AgentOwnerClaimStore = Readonly<{
  approveClaim: (input: {
    claimId: string
    credentialExpiresAt: string
    decidedAt: string
    ownerUserId: string
    registration: AgentRegistrationRecord
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
}>

type AgentOwnerClaimRouteDependencies<
  Session extends AgentOwnerClaimSession,
  Bindings,
> = Readonly<{
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

const claimPagePath = (claimId: string): string =>
  `/agents/claims/${encodeURIComponent(claimId)}`

const claimLoginPath = (claimId: string): string =>
  `/login/github?returnTo=${encodeURIComponent(claimPagePath(claimId))}`

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

  return {
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
            (id, user_id, token_hash, token_prefix, name, status, created_at, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.registration.credential.id,
            input.registration.credential.userId,
            input.registration.credential.tokenHash,
            input.registration.credential.tokenPrefix,
            input.registration.credential.name,
            input.registration.credential.status,
            input.registration.credential.createdAt,
            input.credentialExpiresAt,
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
  }
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
  const token = makeToken()

  if (!token.startsWith(AGENT_TOKEN_PREFIX)) {
    return serverError()
  }

  const claimId = `agent_claim_${makeUuid()}`
  const claim: AgentOwnerClaimRecord = {
    agentUserId: null,
    claimTokenHash: await sha256Hex(token),
    claimTokenPrefix: token.slice(0, 20),
    createdAt: requestedAt,
    credentialId: null,
    decidedAt: null,
    displayName: parsed.displayName,
    expiresAt: isoTimestampAfterIso(
      requestedAt,
      dependencies.claimTtlMs ?? CLAIM_TTL_MS,
    ),
    externalId: parsed.externalId ?? null,
    id: claimId,
    metadataJson: JSON.stringify(parsed.metadata ?? {}),
    ownerUserId: null,
    primaryEmail: parsed.primaryEmail ?? null,
    receiptRef: `agent_claim_receipt_${claimId}`,
    rejectedReason: null,
    requestedAt,
    slug: parsed.slug ?? null,
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
        { error: 'agent_owner_claim_conflict' },
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
      instructions: [
        'Store the one-time pending token securely. OpenAgents does not store or show it again.',
        'The token is not usable until a signed-in owner approves this claim.',
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
        { error: 'agent_owner_claim_conflict' },
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
  const approvePath = `/api/agents/claims/${encodeURIComponent(current.id)}/approve`
  const rejectPath = `/api/agents/claims/${encodeURIComponent(current.id)}/reject`
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
