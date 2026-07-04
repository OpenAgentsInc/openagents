import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { badRequest, notFound } from '@openagentsinc/sync-worker'
import { Effect, Option, Schema as S } from 'effect'

import {
  AgentRateLimitMoneyAmount,
  AgentRateLimitRecoveryError,
  PublicAgentProposalRecoveryRoute,
  activeAgentRateLimitRecoveryGrant,
  agentRateLimitActorRef,
  agentRateLimitSubmissionIdempotencyKeyHash,
  computeAgentRateLimitRequestBodyDigest,
  previewAgentRateLimitRecovery,
  redeemAgentRateLimitRecovery,
  type AgentRateLimitEntitlementRecord,
  type AgentRateLimitRecoveryRuntime,
  type AgentRateLimitRecoveryStore,
} from './agent-rate-limit-recovery'
import { sha256Hex } from './agent-registration'
import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  makeAgentRuntimeRemainderMirrorForEnv,
  type AgentRuntimeRemainderMirror,
  type AgentRuntimeRemainderStoreEnv,
} from './agent-runtime-remainder-store'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  serverError,
  unauthorized,
} from './http/responses'
import {
  arrayFromUnknown,
  optionalString,
  parseJsonRecord,
  parseJsonUnknown,
  readJsonObject,
  recordFromUnknown,
} from './json-boundary'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'
import { openAgentsDatabase } from './runtime'

const PROPOSAL_RATE_LIMIT = 5
const PROPOSAL_RATE_WINDOW_MS = 1000 * 60 * 60

type HttpResponse = globalThis.Response

type AgentProposalKind =
  | 'forum_topic_draft'
  | 'order_request_draft'
  | 'other'
  | 'public_proof_note'
  | 'site_improvement'
  | 'workroom_artifact_draft'

type AgentProposalStatus = 'pending' | 'promoted' | 'rejected'

type AgentProposalPromotionKind =
  | 'customer_order'
  | 'forum_topic'
  | 'manual_review'
  | 'site_feedback'
  | 'workroom_artifact'

export type AgentProposalRecord = Readonly<{
  authorJson: string
  bodyText: string
  clientFingerprintHash: string
  createdAt: string
  decidedAt: string | null
  id: string
  idempotencyKeyHash: string
  kind: AgentProposalKind
  operatorNote: string | null
  operatorUserId: string | null
  promotedTargetRef: string | null
  promotionKind: AgentProposalPromotionKind | null
  receiptRef: string
  sourceUrlsJson: string
  status: AgentProposalStatus
  summary: string
  targetJson: string
  title: string
  updatedAt: string
}>

type AgentProposalRow = Readonly<{
  author_json: string
  body_text: string
  client_fingerprint_hash: string
  created_at: string
  decided_at: string | null
  id: string
  idempotency_key_hash: string
  kind: AgentProposalKind
  operator_note: string | null
  operator_user_id: string | null
  promoted_target_ref: string | null
  promotion_kind: AgentProposalPromotionKind | null
  receipt_ref: string
  source_urls_json: string
  status: AgentProposalStatus
  summary: string
  target_json: string
  title: string
  updated_at: string
}>

type AgentProposalSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type AgentProposalRouteDependencies<
  Session extends AgentProposalSession,
  Bindings,
> = Readonly<{
  agentStore?: (env: Bindings) => AgentRegistrationStore
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  appOrigin: (env: Bindings) => string
  isOpenAgentsAdminEmail: (email: string) => boolean
  makeStore: (env: Bindings) => AgentProposalStore
  makeUuid?: () => string
  nowIso?: () => string
  proposalRateLimit?: number
  proposalRateWindowMs?: number
  rateLimitRecoveryRuntime?: AgentRateLimitRecoveryRuntime
  recoveryStore?: (env: Bindings) => AgentRateLimitRecoveryStore
  requireAdminApiToken?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

export type AgentProposalStore = Readonly<{
  countRecentByClientFingerprint: (
    clientFingerprintHash: string,
    sinceIso: string,
  ) => Promise<number>
  createProposal: (record: AgentProposalRecord) => Promise<void>
  listProposals: (input: {
    limit: number
    status: AgentProposalStatus | 'all'
  }) => Promise<ReadonlyArray<AgentProposalRecord>>
  readById: (proposalId: string) => Promise<AgentProposalRecord | undefined>
  readByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<AgentProposalRecord | undefined>
  transitionProposal: (input: {
    decidedAt: string
    note: string | null
    operatorUserId: string
    promotedTargetRef?: string | null
    promotionKind?: AgentProposalPromotionKind | null
    proposalId: string
    status: Extract<AgentProposalStatus, 'promoted' | 'rejected'>
  }) => Promise<AgentProposalRecord | undefined>
}>

const proposalKinds: ReadonlySet<string> = new Set([
  'forum_topic_draft',
  'order_request_draft',
  'other',
  'public_proof_note',
  'site_improvement',
  'workroom_artifact_draft',
])

const promotionKinds: ReadonlySet<string> = new Set([
  'customer_order',
  'forum_topic',
  'manual_review',
  'site_feedback',
  'workroom_artifact',
])

const rowToProposal = (row: AgentProposalRow): AgentProposalRecord => ({
  authorJson: row.author_json,
  bodyText: row.body_text,
  clientFingerprintHash: row.client_fingerprint_hash,
  createdAt: row.created_at,
  decidedAt: row.decided_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  kind: row.kind,
  operatorNote: row.operator_note,
  operatorUserId: row.operator_user_id,
  promotedTargetRef: row.promoted_target_ref,
  promotionKind: row.promotion_kind,
  receiptRef: row.receipt_ref,
  sourceUrlsJson: row.source_urls_json,
  status: row.status,
  summary: row.summary,
  targetJson: row.target_json,
  title: row.title,
  updatedAt: row.updated_at,
})

const jsonRecord = (value: string): Record<string, unknown> => {
  return parseJsonRecord(value) ?? {}
}

const jsonArray = (value: string): ReadonlyArray<unknown> => {
  try {
    const parsed = parseJsonUnknown(value)

    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const isUniqueConstraintError = (error: unknown): boolean =>
  errorMessage(error).includes('UNIQUE constraint failed')

const boundedText = (
  value: unknown,
  field: string,
  minLength: number,
  maxLength: number,
): string => {
  const text = optionalString(value)

  if (text === undefined || text.length < minLength) {
    throw new AgentProposalValidationError({
      reason: `${field} must be at least ${minLength} characters.`,
    })
  }

  if (text.length > maxLength) {
    throw new AgentProposalValidationError({
      reason: `${field} must be at most ${maxLength} characters.`,
    })
  }

  return text
}

const checkedKind = (value: unknown): AgentProposalKind => {
  const kind = optionalString(value)

  if (kind === undefined || !proposalKinds.has(kind)) {
    return 'other'
  }

  return kind as AgentProposalKind
}

const checkedPromotionKind = (value: unknown): AgentProposalPromotionKind => {
  const kind = optionalString(value)

  if (kind === undefined || !promotionKinds.has(kind)) {
    return 'manual_review'
  }

  return kind as AgentProposalPromotionKind
}

const checkedSourceUrls = (value: unknown): ReadonlyArray<string> => {
  const rawUrls = arrayFromUnknown(value)?.slice(0, 8) ?? []
  const urls = rawUrls.flatMap(rawUrl => {
    const urlText = optionalString(rawUrl)

    if (urlText === undefined || urlText.length > 500) {
      return []
    }

    try {
      const url = new URL(urlText)

      return url.protocol === 'https:' || url.protocol === 'http:'
        ? [url.toString()]
        : []
    } catch {
      return []
    }
  })

  return Array.from(new Set(urls))
}

const clientFingerprintInput = (request: Request): string => {
  const ip =
    request.headers.get('cf-connecting-ip') ??
    request.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ??
    'unknown-ip'
  const userAgent = request.headers.get('user-agent') ?? 'unknown-agent'

  return `${ip}|${userAgent}`
}

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const key = request.headers.get('idempotency-key')?.trim()

  if (key === undefined || key.length < 8 || key.length > 200) {
    return undefined
  }

  return key
}

const idempotencyKeyFromValue = (value: unknown): string | undefined => {
  const key = optionalString(value)?.trim()

  if (key === undefined || key.length < 8 || key.length > 200) {
    return undefined
  }

  return key
}

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  return scheme?.toLowerCase() === 'bearer' &&
    token !== undefined &&
    token.startsWith(AGENT_TOKEN_PREFIX)
    ? token
    : undefined
}

const paidEntitlementRefFromRequest = (request: Request): string | undefined => {
  const entitlementRef = request.headers
    .get('x-openagents-rate-limit-entitlement')
    ?.trim()

  return entitlementRef === undefined || entitlementRef.length === 0
    ? undefined
    : entitlementRef
}

const decodeMoneyAmount = S.decodeUnknownOption(AgentRateLimitMoneyAmount)

const checkedSpendCap = (value: unknown) => {
  const decoded = Option.getOrUndefined(decodeMoneyAmount(value))

  if (decoded === undefined) {
    throw new AgentProposalValidationError({
      reason:
        'spendCap must include amount, asset, and denomination for the recovery preview.',
    })
  }

  return decoded
}

class AgentProposalValidationError extends Error {
  readonly reason: string

  constructor(input: { reason: string }) {
    super(input.reason)
    this.reason = input.reason
  }
}

const parseProposalBody = (body: Record<string, unknown>) => {
  const title = boundedText(body.title, 'title', 3, 160)
  const summary = boundedText(body.summary, 'summary', 10, 700)
  const bodyText = boundedText(body.bodyText, 'bodyText', 20, 5000)
  const kind = checkedKind(body.kind)
  const sourceUrls = checkedSourceUrls(body.sourceUrls)
  const target = recordFromUnknown(body.target) ?? {}
  const author = recordFromUnknown(body.author) ?? {}
  const safetyPayload = JSON.stringify({
    author,
    bodyText,
    sourceUrls,
    summary,
    target,
    title,
  })

  if (containsProviderSecretMaterial(safetyPayload)) {
    throw new AgentProposalValidationError({
      reason: 'proposal appears to contain secret material',
    })
  }

  return {
    author,
    bodyText,
    kind,
    sourceUrls,
    summary,
    target,
    title,
  }
}

const parseProposalRequest = async (request: Request) =>
  parseProposalBody(await readJsonObject(request))

const parseTransitionRequest = async (request: Request) => {
  const body = await readJsonObject(request)
  const note = optionalString(body.note ?? body.reason) ?? null
  const promotedTargetRef = optionalString(body.promotedTargetRef) ?? null
  const promotionKind = checkedPromotionKind(body.promotionKind)

  return {
    note: note === null ? null : note.slice(0, 1000),
    promotedTargetRef:
      promotedTargetRef === null ? null : promotedTargetRef.slice(0, 300),
    promotionKind,
  }
}

const proposalProjection = (
  proposal: AgentProposalRecord,
  appOrigin: string,
) => ({
  author: jsonRecord(proposal.authorJson),
  bodyText: proposal.bodyText,
  createdAt: proposal.createdAt,
  decidedAt: proposal.decidedAt,
  id: proposal.id,
  kind: proposal.kind,
  operatorNote: proposal.operatorNote,
  promotedTargetRef: proposal.promotedTargetRef,
  promotionKind: proposal.promotionKind,
  receiptRef: proposal.receiptRef,
  sourceUrls: jsonArray(proposal.sourceUrlsJson).filter(
    (url): url is string => typeof url === 'string',
  ),
  status: proposal.status,
  summary: proposal.summary,
  target: jsonRecord(proposal.targetJson),
  title: proposal.title,
  updatedAt: proposal.updatedAt,
  url: `${appOrigin}/api/agents/proposals/${encodeURIComponent(proposal.id)}`,
})

const proposalResponse = (
  proposal: AgentProposalRecord,
  appOrigin: string,
  extra: Record<string, unknown> = {},
  status = 200,
) =>
  noStoreJsonResponse(
    {
      proposal: proposalProjection(proposal, appOrigin),
      ...extra,
    },
    { status },
  )

const listResponse = (
  proposals: ReadonlyArray<AgentProposalRecord>,
  appOrigin: string,
) =>
  noStoreJsonResponse({
    proposals: proposals.map(proposal => proposalProjection(proposal, appOrigin)),
  })

const withProposalRateLimitHeaders = (
  response: HttpResponse,
  limit: number,
  windowSeconds: number,
  input: Readonly<{
    paidRecovery:
      | 'available_l402'
      | 'planned_not_live'
      | 'wait_only'
    price?: typeof PublicAgentProposalRecoveryRoute.price
  }> = { paidRecovery: 'planned_not_live' },
) => {
  response.headers.set('ratelimit-limit', String(limit))
  response.headers.set('ratelimit-policy', `${limit};w=${windowSeconds}`)
  response.headers.set('ratelimit-reset', String(windowSeconds))
  response.headers.set('x-openagents-paid-recovery', input.paidRecovery)
  response.headers.set('x-openagents-payment-preview-required', 'true')
  response.headers.set('x-openagents-spend-cap-required', 'true')
  response.headers.set(
    'x-openagents-recovery-modes',
    input.paidRecovery === 'available_l402'
      ? 'wait, l402'
      : 'wait, operator_review',
  )

  if (response.status === 429) {
    response.headers.set('retry-after', String(windowSeconds))
  }

  if (input.paidRecovery === 'available_l402') {
    response.headers.set(
      'x-openagents-rate-limit-preview-url',
      PublicAgentProposalRecoveryRoute.previewPath,
    )
    response.headers.set(
      'x-openagents-rate-limit-redeem-url',
      PublicAgentProposalRecoveryRoute.redeemPath,
    )
  }

  if (input.price !== undefined) {
    response.headers.set(
      'x-openagents-recovery-price',
      `${input.price.asset}:${input.price.amount}:${input.price.denomination}`,
    )
  }

  return response
}

export const makeD1AgentProposalStore = (
  db: D1Database,
): AgentProposalStore => {
  const readById = async (
    proposalId: string,
  ): Promise<AgentProposalRecord | undefined> => {
    const row = await db
      .prepare(
        `SELECT *
         FROM agent_proposals
         WHERE id = ?`,
      )
      .bind(proposalId)
      .first<AgentProposalRow>()

    return row === null ? undefined : rowToProposal(row)
  }

  return {
    countRecentByClientFingerprint: async (clientFingerprintHash, sinceIso) => {
      const row = await db
        .prepare(
          `SELECT COUNT(*) AS count
         FROM agent_proposals
         WHERE client_fingerprint_hash = ?
           AND created_at >= ?`,
        )
        .bind(clientFingerprintHash, sinceIso)
        .first<{ count: number }>()

      return Number(row?.count ?? 0)
    },
    createProposal: async record => {
      await db
        .prepare(
          `INSERT INTO agent_proposals
          (id, receipt_ref, status, kind, title, summary, body_text,
           source_urls_json, target_json, author_json, client_fingerprint_hash,
           idempotency_key_hash, promotion_kind, promoted_target_ref,
           operator_note, operator_user_id, decided_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.receiptRef,
          record.status,
          record.kind,
          record.title,
          record.summary,
          record.bodyText,
          record.sourceUrlsJson,
          record.targetJson,
          record.authorJson,
          record.clientFingerprintHash,
          record.idempotencyKeyHash,
          record.promotionKind,
          record.promotedTargetRef,
          record.operatorNote,
          record.operatorUserId,
          record.decidedAt,
          record.createdAt,
          record.updatedAt,
        )
        .run()
    },
    listProposals: async input => {
      const query =
        input.status === 'all'
          ? db
              .prepare(
                `SELECT *
               FROM agent_proposals
               ORDER BY created_at DESC
               LIMIT ?`,
              )
              .bind(input.limit)
          : db
              .prepare(
                `SELECT *
               FROM agent_proposals
               WHERE status = ?
               ORDER BY created_at DESC
               LIMIT ?`,
              )
              .bind(input.status, input.limit)
      const result = await query.all<AgentProposalRow>()

      return result.results.map(rowToProposal)
    },
    readById,
    readByIdempotencyKeyHash: async idempotencyKeyHash => {
      const row = await db
        .prepare(
          `SELECT *
         FROM agent_proposals
         WHERE idempotency_key_hash = ?`,
        )
        .bind(idempotencyKeyHash)
        .first<AgentProposalRow>()

      return row === null ? undefined : rowToProposal(row)
    },
    transitionProposal: async input => {
      await db
        .prepare(
          `UPDATE agent_proposals
         SET status = ?,
             promotion_kind = ?,
             promoted_target_ref = ?,
             operator_note = ?,
             operator_user_id = ?,
             decided_at = ?,
             updated_at = ?
         WHERE id = ?
           AND status = 'pending'`,
        )
        .bind(
          input.status,
          input.promotionKind ?? null,
          input.promotedTargetRef ?? null,
          input.note,
          input.operatorUserId,
          input.decidedAt,
          input.decidedAt,
          input.proposalId,
        )
        .run()

      return readById(input.proposalId)
    },
  }
}

const mirrorAgentProposal = (
  mirror: AgentRuntimeRemainderMirror,
  proposalId: string,
): Promise<void> => mirror.mirrorRowsByPk('agent_proposals', [proposalId])

export const makeMirroredAgentProposalStore = (
  d1: AgentProposalStore,
  mirror: AgentRuntimeRemainderMirror | undefined,
): AgentProposalStore => {
  if (mirror === undefined) {
    return d1
  }

  return {
    countRecentByClientFingerprint: (clientFingerprintHash, sinceIso) =>
      d1.countRecentByClientFingerprint(clientFingerprintHash, sinceIso),
    createProposal: async record => {
      await d1.createProposal(record)
      await mirrorAgentProposal(mirror, record.id)
    },
    listProposals: input => d1.listProposals(input),
    readById: proposalId => d1.readById(proposalId),
    readByIdempotencyKeyHash: idempotencyKeyHash =>
      d1.readByIdempotencyKeyHash(idempotencyKeyHash),
    transitionProposal: async input => {
      const proposal = await d1.transitionProposal(input)
      if (proposal !== undefined) {
        await mirrorAgentProposal(mirror, proposal.id)
      }
      return proposal
    },
  }
}

export const makeAgentProposalStoreForEnv = (
  env: AgentRuntimeRemainderStoreEnv,
): AgentProposalStore =>
  makeMirroredAgentProposalStore(
    makeD1AgentProposalStore(openAgentsDatabase(env)),
    makeAgentRuntimeRemainderMirrorForEnv(env),
  )

const requireAdminSession = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Session | undefined> => {
  if (dependencies.requireAdminApiToken !== undefined) {
    const hasAdminToken = await dependencies.requireAdminApiToken(request, env)

    if (hasAdminToken) {
      return {
        user: {
          email: 'chris@openagents.com',
          userId: 'github:14167547',
        },
      } as Session
    }
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (
    session === undefined ||
    !dependencies.isOpenAgentsAdminEmail(session.user.email)
  ) {
    return undefined
  }

  return session
}

const authenticateRecoveryAgent = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
): Promise<ProgrammaticAgentSession | undefined> => {
  const token = bearerTokenFromRequest(request)
  const store = dependencies.agentStore?.(env)

  return token === undefined || store === undefined
    ? undefined
    : authenticateProgrammaticAgent(store, token)
}

const proposalRecoveryStore = <Session extends AgentProposalSession, Bindings>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  env: Bindings,
): AgentRateLimitRecoveryStore | undefined =>
  dependencies.recoveryStore?.(env)

const proposalBodyDigest = (body: Record<string, unknown>) =>
  computeAgentRateLimitRequestBodyDigest(
    PublicAgentProposalRecoveryRoute.routeKey,
    PublicAgentProposalRecoveryRoute.method,
    PublicAgentProposalRecoveryRoute.path,
    body,
  )

const activeProposalRecoveryGrant = (
  session: ProgrammaticAgentSession,
  nowIso: string,
) =>
  activeAgentRateLimitRecoveryGrant(
    session,
    PublicAgentProposalRecoveryRoute.routeKey,
    PublicAgentProposalRecoveryRoute.price,
    nowIso,
  )

const errorResponseForRecovery = (
  error: unknown,
  status = 403,
): HttpResponse =>
  error instanceof AgentRateLimitRecoveryError
    ? noStoreJsonResponse(
        {
          error: `agent_rate_limit_recovery_${error.kind}`,
          reason: error.message,
        },
        { status },
      )
    : serverError()

const recoveryHeadersForSession = (
  response: HttpResponse,
  session: ProgrammaticAgentSession | undefined,
  nowIso: string,
  limit: number,
  windowSeconds: number,
) =>
  withProposalRateLimitHeaders(response, limit, windowSeconds, {
    paidRecovery:
      session !== undefined &&
      activeProposalRecoveryGrant(session, nowIso) !== undefined
        ? 'available_l402'
        : 'wait_only',
    price: PublicAgentProposalRecoveryRoute.price,
  })

const consumeProposalRecoveryEntitlement = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  input: Readonly<{
    clientFingerprintHash: string
    idempotencyKey: string
    limit: number
    nowIso: string
    windowSeconds: number
  }>,
): Promise<
  | Readonly<{
      entitlement: AgentRateLimitEntitlementRecord
      parsed: Awaited<ReturnType<typeof parseProposalRequest>>
    }>
  | HttpResponse
  | undefined
> => {
  const entitlementRef = paidEntitlementRefFromRequest(request)
  const session = await authenticateRecoveryAgent(dependencies, request, env)

  if (entitlementRef === undefined) {
    return recoveryHeadersForSession(
      noStoreJsonResponse(
        {
          error: 'agent_proposal_rate_limited',
          paidRecovery:
            session !== undefined &&
            activeProposalRecoveryGrant(session, input.nowIso) !== undefined
              ? 'available_l402'
              : 'wait_only',
        },
        { status: 429 },
      ),
      session,
      input.nowIso,
      input.limit,
      input.windowSeconds,
    )
  }

  const recoveryStore = proposalRecoveryStore(dependencies, env)

  if (session === undefined || recoveryStore === undefined) {
    return recoveryHeadersForSession(
      noStoreJsonResponse(
        {
          error: 'agent_rate_limit_recovery_not_authorized',
          paidRecovery: 'wait_only',
        },
        { status: 403 },
      ),
      session,
      input.nowIso,
      input.limit,
      input.windowSeconds,
    )
  }

  if (activeProposalRecoveryGrant(session, input.nowIso) === undefined) {
    return recoveryHeadersForSession(
      noStoreJsonResponse(
        {
          error: 'agent_rate_limit_recovery_grant_missing',
          paidRecovery: 'wait_only',
        },
        { status: 403 },
      ),
      session,
      input.nowIso,
      input.limit,
      input.windowSeconds,
    )
  }

  let body: Record<string, unknown>
  let parsed: Awaited<ReturnType<typeof parseProposalRequest>>

  try {
    body = recordFromUnknown(await request.clone().json()) ?? {}
    parsed = parseProposalBody(body)
  } catch (error) {
    if (error instanceof AgentProposalValidationError) {
      return badRequest(error.reason)
    }

    return badRequest(errorMessage(error))
  }

  const actorRef = agentRateLimitActorRef(session)
  const requestBodyDigest = await proposalBodyDigest(body)
  const submissionIdempotencyKeyHash =
    await agentRateLimitSubmissionIdempotencyKeyHash(
      actorRef,
      PublicAgentProposalRecoveryRoute.routeKey,
      input.idempotencyKey,
    )

  try {
    const entitlement = await recoveryStore.consumeEntitlement({
      actorRef,
      clientFingerprintHash: input.clientFingerprintHash,
      entitlementRef,
      method: PublicAgentProposalRecoveryRoute.method,
      nowIso: input.nowIso,
      path: PublicAgentProposalRecoveryRoute.path,
      requestBodyDigest,
      routeKey: PublicAgentProposalRecoveryRoute.routeKey,
      submissionIdempotencyKeyHash,
    })

    return entitlement === undefined
      ? noStoreJsonResponse(
          {
            error: 'agent_rate_limit_entitlement_not_found',
            paidRecovery: 'available_l402',
          },
          { status: 403 },
        )
      : { entitlement, parsed }
  } catch (error) {
    return errorResponseForRecovery(error)
  }
}

const createProposalResponse = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey === undefined) {
    return badRequest('Idempotency-Key header of 8-200 characters is required')
  }

  const now = (dependencies.nowIso ?? currentIsoTimestamp)()
  const makeUuid = dependencies.makeUuid ?? randomUuid
  const store = dependencies.makeStore(env)
  const clientFingerprintHash = await sha256Hex(clientFingerprintInput(request))
  const idempotencyKeyHash = await sha256Hex(
    `${clientFingerprintHash}:${idempotencyKey}`,
  )
  const existing = await store.readByIdempotencyKeyHash(idempotencyKeyHash)
  const limit = dependencies.proposalRateLimit ?? PROPOSAL_RATE_LIMIT
  const windowMs =
    dependencies.proposalRateWindowMs ?? PROPOSAL_RATE_WINDOW_MS

  if (existing !== undefined) {
    return withProposalRateLimitHeaders(
      proposalResponse(existing, dependencies.appOrigin(env), {
        idempotentReplay: true,
      }),
      limit,
      Math.round(windowMs / 1000),
    )
  }

  const sinceIso = isoTimestampAfterIso(now, -windowMs)
  const recentCount = await store.countRecentByClientFingerprint(
    clientFingerprintHash,
    sinceIso,
  )
  let paidRecoveryEntitlement: AgentRateLimitEntitlementRecord | undefined
  let paidRecoveryParsed:
    | Awaited<ReturnType<typeof parseProposalRequest>>
    | undefined

  if (recentCount >= limit) {
    const recoveryAttempt = await consumeProposalRecoveryEntitlement(
      dependencies,
      request,
      env,
      {
        clientFingerprintHash,
        idempotencyKey,
        limit,
        nowIso: now,
        windowSeconds: Math.round(windowMs / 1000),
      },
    )

    if (recoveryAttempt instanceof Response) {
      return recoveryAttempt
    }

    if (recoveryAttempt === undefined) {
      return withProposalRateLimitHeaders(
        noStoreJsonResponse(
          {
            error: 'agent_proposal_rate_limited',
            paidRecovery: 'wait_only',
          },
          { status: 429 },
        ),
        limit,
        Math.round(windowMs / 1000),
        { paidRecovery: 'wait_only' },
      )
    }

    paidRecoveryEntitlement = recoveryAttempt.entitlement
    paidRecoveryParsed = recoveryAttempt.parsed
  }

  let parsed: Awaited<ReturnType<typeof parseProposalRequest>>

  if (paidRecoveryParsed !== undefined) {
    parsed = paidRecoveryParsed
  } else {
    try {
      parsed = await parseProposalRequest(request)
    } catch (error) {
      if (error instanceof AgentProposalValidationError) {
        return badRequest(error.reason)
      }

      return badRequest(errorMessage(error))
    }
  }

  const proposalId = `agent_proposal_${makeUuid()}`
  const record: AgentProposalRecord = {
    authorJson: JSON.stringify(parsed.author),
    bodyText: parsed.bodyText,
    clientFingerprintHash,
    createdAt: now,
    decidedAt: null,
    id: proposalId,
    idempotencyKeyHash,
    kind: parsed.kind,
    operatorNote: null,
    operatorUserId: null,
    promotedTargetRef: null,
    promotionKind: null,
    receiptRef: `agent_proposal_receipt_${proposalId}`,
    sourceUrlsJson: JSON.stringify(parsed.sourceUrls),
    status: 'pending',
    summary: parsed.summary,
    targetJson: JSON.stringify(parsed.target),
    title: parsed.title,
    updatedAt: now,
  }

  try {
    await store.createProposal(record)
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      const duplicate = await store.readByIdempotencyKeyHash(idempotencyKeyHash)

      if (duplicate !== undefined) {
        return proposalResponse(duplicate, dependencies.appOrigin(env), {
          idempotentReplay: true,
        })
      }
    }

    return serverError()
  }

  return withProposalRateLimitHeaders(
    proposalResponse(
      record,
      dependencies.appOrigin(env),
      {
        authority: {
          createsCustomerOrder: false,
          deploysSite: false,
          postsPublicly: false,
          sendsEmail: false,
          spendsMoney: false,
          status: 'pending_operator_review',
        },
        idempotentReplay: false,
        paidRecovery:
          paidRecoveryEntitlement === undefined
            ? null
            : {
                entitlementRef: paidRecoveryEntitlement.entitlementRef,
                receiptRef: paidRecoveryEntitlement.receiptRef,
                status: 'consumed',
              },
      },
      201,
    ),
    limit,
    Math.round(windowMs / 1000),
    {
      paidRecovery:
        paidRecoveryEntitlement === undefined ? 'planned_not_live' : 'available_l402',
    },
  )
}

const readPublicProposalResponse = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  proposalId: string,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const proposal = await dependencies.makeStore(env).readById(proposalId)

  if (proposal === undefined) {
    return notFound()
  }

  return proposalResponse(proposal, dependencies.appOrigin(env))
}

const previewProposalRateLimitRecoveryResponse = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const previewIdempotencyKey = idempotencyKeyFromRequest(request)

  if (previewIdempotencyKey === undefined) {
    return badRequest('Idempotency-Key header of 8-200 characters is required')
  }

  const session = await authenticateRecoveryAgent(dependencies, request, env)
  const recoveryStore = proposalRecoveryStore(dependencies, env)

  if (session === undefined || recoveryStore === undefined) {
    return unauthorized()
  }

  const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
  const grant = activeProposalRecoveryGrant(session, nowIso)

  if (grant === undefined) {
    return noStoreJsonResponse(
      {
        error: 'agent_rate_limit_recovery_grant_missing',
        paidRecovery: 'wait_only',
      },
      { status: 403 },
    )
  }

  let body: Record<string, unknown>
  let proposalBody: Record<string, unknown>
  let submitIdempotencyKey: string
  let spendCap: typeof AgentRateLimitMoneyAmount.Type

  try {
    body = await readJsonObject(request)
    proposalBody = recordFromUnknown(body.proposal) ?? {}
    parseProposalBody(proposalBody)
    submitIdempotencyKey =
      idempotencyKeyFromValue(body.idempotencyKey) ??
      (() => {
        throw new AgentProposalValidationError({
          reason:
            'idempotencyKey must match the Idempotency-Key that will be used for the paid retry.',
        })
      })()
    spendCap = checkedSpendCap(body.spendCap)
  } catch (error) {
    if (error instanceof AgentProposalValidationError) {
      return badRequest(error.reason)
    }

    return badRequest(errorMessage(error))
  }

  if (
    spendCap.asset !== grant.spendCap.asset ||
    spendCap.denomination !== grant.spendCap.denomination ||
    spendCap.amount > grant.spendCap.amount
  ) {
    return noStoreJsonResponse(
      {
        error: 'agent_rate_limit_recovery_over_grant_spend_cap',
        paidRecovery: 'wait_only',
      },
      { status: 403 },
    )
  }

  const actorRef = agentRateLimitActorRef(session)
  const clientFingerprintHash = await sha256Hex(clientFingerprintInput(request))
  const requestBodyDigest = await proposalBodyDigest(proposalBody)
  const submissionIdempotencyKeyHash =
    await agentRateLimitSubmissionIdempotencyKeyHash(
      actorRef,
      PublicAgentProposalRecoveryRoute.routeKey,
      submitIdempotencyKey,
    )

  try {
    const challenge = await previewAgentRateLimitRecovery(
      recoveryStore,
      {
        actorRef,
        clientFingerprintHash,
        idempotencyKey: previewIdempotencyKey,
        ownerUserId: grant.ownerUserId,
        requestBodyDigest,
        routeKey: PublicAgentProposalRecoveryRoute.routeKey,
        spendCap,
        submissionIdempotencyKeyHash,
      },
      dependencies.rateLimitRecoveryRuntime,
    )

    return withProposalRateLimitHeaders(
      noStoreJsonResponse({
        challenge: {
          actorRef: challenge.actorRef,
          challengeId: challenge.id,
          entitlementKind: challenge.entitlementKind,
          expiresAt: challenge.expiresAt,
          method: challenge.method,
          path: challenge.path,
          price: challenge.price,
          requestBodyDigest: challenge.requestBodyDigest,
          routeKey: challenge.routeKey,
          spendCap: challenge.spendCap,
        },
        entitlementRef: null,
        paymentRequired: true,
        paidRecovery: 'available_l402',
      }),
      dependencies.proposalRateLimit ?? PROPOSAL_RATE_LIMIT,
      Math.round(
        (dependencies.proposalRateWindowMs ?? PROPOSAL_RATE_WINDOW_MS) / 1000,
      ),
      {
        paidRecovery: 'available_l402',
        price: PublicAgentProposalRecoveryRoute.price,
      },
    )
  } catch (error) {
    return errorResponseForRecovery(error)
  }
}

const redeemProposalRateLimitRecoveryResponse = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const redemptionIdempotencyKey = idempotencyKeyFromRequest(request)

  if (redemptionIdempotencyKey === undefined) {
    return badRequest('Idempotency-Key header of 8-200 characters is required')
  }

  const session = await authenticateRecoveryAgent(dependencies, request, env)
  const recoveryStore = proposalRecoveryStore(dependencies, env)

  if (session === undefined || recoveryStore === undefined) {
    return unauthorized()
  }

  const body = await readJsonObject(request)
  const challengeId = optionalString(body.challengeId)
  const l402ProofRef = optionalString(body.l402ProofRef)

  if (challengeId === undefined || l402ProofRef === undefined) {
    return badRequest('challengeId and l402ProofRef are required')
  }

  const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()

  if (activeProposalRecoveryGrant(session, nowIso) === undefined) {
    return noStoreJsonResponse(
      {
        error: 'agent_rate_limit_recovery_grant_missing',
        paidRecovery: 'wait_only',
      },
      { status: 403 },
    )
  }

  try {
    const redeemed = await redeemAgentRateLimitRecovery(
      recoveryStore,
      {
        actorRef: agentRateLimitActorRef(session),
        challengeId,
        idempotencyKey: redemptionIdempotencyKey,
        l402ProofRef,
      },
      dependencies.rateLimitRecoveryRuntime,
    )

    return withProposalRateLimitHeaders(
      noStoreJsonResponse({
        ...redeemed,
        paidRecovery: 'available_l402',
        retry: {
          entitlementHeader: 'X-OpenAgents-Rate-Limit-Entitlement',
          entitlementRef: redeemed.entitlementRef,
          method: PublicAgentProposalRecoveryRoute.method,
          path: PublicAgentProposalRecoveryRoute.path,
        },
      }),
      dependencies.proposalRateLimit ?? PROPOSAL_RATE_LIMIT,
      Math.round(
        (dependencies.proposalRateWindowMs ?? PROPOSAL_RATE_WINDOW_MS) / 1000,
      ),
      {
        paidRecovery: 'available_l402',
        price: PublicAgentProposalRecoveryRoute.price,
      },
    )
  } catch (error) {
    return errorResponseForRecovery(error)
  }
}

const listOperatorProposalsResponse = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await requireAdminSession(dependencies, request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const url = new URL(request.url)
  const status = optionalString(url.searchParams.get('status')) ?? 'pending'
  const normalizedStatus: AgentProposalStatus | 'all' =
    status === 'all' || status === 'promoted' || status === 'rejected'
      ? status
      : 'pending'
  const proposals = await dependencies.makeStore(env).listProposals({
    limit: 100,
    status: normalizedStatus,
  })

  return dependencies.appendRefreshedSessionCookies(
    listResponse(proposals, dependencies.appOrigin(env)),
    session,
  )
}

const readOperatorProposalResponse = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  proposalId: string,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await requireAdminSession(dependencies, request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const proposal = await dependencies.makeStore(env).readById(proposalId)

  if (proposal === undefined) {
    return notFound()
  }

  return dependencies.appendRefreshedSessionCookies(
    proposalResponse(proposal, dependencies.appOrigin(env)),
    session,
  )
}

const transitionOperatorProposalResponse = async <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  proposalId: string,
  status: Extract<AgentProposalStatus, 'promoted' | 'rejected'>,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await requireAdminSession(dependencies, request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const input = await parseTransitionRequest(request)
  const proposal = await dependencies.makeStore(env).transitionProposal({
    decidedAt: (dependencies.nowIso ?? currentIsoTimestamp)(),
    note: input.note,
    operatorUserId: session.user.userId,
    promotedTargetRef:
      status === 'promoted' ? input.promotedTargetRef : null,
    promotionKind: status === 'promoted' ? input.promotionKind : null,
    proposalId,
    status,
  })

  if (proposal === undefined) {
    return notFound()
  }

  if (proposal.status !== status) {
    return noStoreJsonResponse(
      { error: 'agent_proposal_not_pending' },
      { status: 409 },
    )
  }

  return dependencies.appendRefreshedSessionCookies(
    proposalResponse(proposal, dependencies.appOrigin(env), {
      downstreamEffect:
        'No public post, order, deploy, email, repository connection, or payment was created by this transition.',
    }),
    session,
  )
}

export const makeAgentProposalRoutes = <
  Session extends AgentProposalSession,
  Bindings,
>(
  dependencies: AgentProposalRouteDependencies<Session, Bindings>,
) => ({
  routeAgentProposalRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/agents/proposals') {
      return Effect.promise(() =>
        createProposalResponse(dependencies, request, env),
      )
    }

    if (url.pathname === PublicAgentProposalRecoveryRoute.previewPath) {
      return Effect.promise(() =>
        previewProposalRateLimitRecoveryResponse(dependencies, request, env),
      )
    }

    if (url.pathname === PublicAgentProposalRecoveryRoute.redeemPath) {
      return Effect.promise(() =>
        redeemProposalRateLimitRecoveryResponse(dependencies, request, env),
      )
    }

    const publicReadMatch =
      /^\/api\/agents\/proposals\/([^/]+)$/.exec(url.pathname)

    if (publicReadMatch !== null) {
      return Effect.promise(() =>
        readPublicProposalResponse(
          dependencies,
          request,
          env,
          decodeURIComponent(publicReadMatch[1] ?? ''),
        ),
      )
    }

    if (url.pathname === '/api/operator/agent-proposals') {
      return Effect.promise(() =>
        listOperatorProposalsResponse(dependencies, request, env, ctx),
      )
    }

    const operatorMatch =
      /^\/api\/operator\/agent-proposals\/([^/]+)(?:\/(promote|reject))?$/.exec(
        url.pathname,
      )

    if (operatorMatch === null) {
      return undefined
    }

    const proposalId = decodeURIComponent(operatorMatch[1] ?? '')
    const action = operatorMatch[2]

    if (action === 'promote' || action === 'reject') {
      return Effect.promise(() =>
        transitionOperatorProposalResponse(
          dependencies,
          request,
          env,
          ctx,
          proposalId,
          action === 'promote' ? 'promoted' : 'rejected',
        ),
      )
    }

    return Effect.promise(() =>
      readOperatorProposalResponse(
        dependencies,
        request,
        env,
        ctx,
        proposalId,
      ),
    )
  },
})
