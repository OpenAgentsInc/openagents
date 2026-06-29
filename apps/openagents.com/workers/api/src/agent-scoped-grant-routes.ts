import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { badRequest, notFound } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import { sha256Hex } from './agent-registration'
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
  parseJsonStringArray,
  readJsonObject,
  recordFromUnknown,
  stringArrayFromUnknown,
} from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

type HttpResponse = globalThis.Response

type AgentScopedGrantKind = 'agent_sites' | 'customer_orders'
type AgentScopedGrantAction = 'grant' | 'revoke'

type AgentScopedGrantSession = Readonly<{
  user: Readonly<{
    email: string
    login?: string | undefined
    name?: string | undefined
    userId: string
  }>
}>

export type OwnerAgentRecord = Readonly<{
  avatarUrl: string | null
  createdAt: string
  credentialExpiresAt: string | null
  credentialId: string | null
  credentialLastUsedAt: string | null
  credentialStatus: string | null
  displayName: string
  primaryEmail: string | null
  profileMetadataJson: string
  slug: string | null
  tokenPrefix: string | null
  updatedAt: string
  userId: string
}>

export type OwnerClaimRecord = Readonly<{
  agentUserId: string | null
  claimId: string
  displayName: string
  receiptRef: string
  requestedAt: string
  status: string
  tokenPrefix: string | null
}>

export type AgentScopedGrantReceiptRecord = Readonly<{
  action: AgentScopedGrantAction
  agentCredentialId: string | null
  agentUserId: string
  createdAt: string
  expiresAt: string | null
  grantId: string
  grantKind: AgentScopedGrantKind
  id: string
  idempotencyKeyHash: string
  ownerUserId: string
  reason: string | null
  receiptRef: string
  scopesJson: string
  status: 'applied' | 'idempotent_replay'
  targetJson: string
}>

type AgentScopedGrantReceiptRow = Readonly<{
  action: AgentScopedGrantAction
  agent_credential_id: string | null
  agent_user_id: string
  created_at: string
  expires_at: string | null
  grant_id: string
  grant_kind: AgentScopedGrantKind
  id: string
  idempotency_key_hash: string
  owner_user_id: string
  reason: string | null
  receipt_ref: string
  scopes_json: string
  status: 'applied' | 'idempotent_replay'
  target_json: string
}>

export type AgentScopedGrantStore = Readonly<{
  createReceipt: (
    receipt: AgentScopedGrantReceiptRecord,
  ) => Promise<AgentScopedGrantReceiptRecord>
  listAgentClaimsForOwner: (
    ownerUserId: string,
  ) => Promise<ReadonlyArray<OwnerClaimRecord>>
  listAgents: () => Promise<ReadonlyArray<OwnerAgentRecord>>
  listReceiptsForOwner: (
    ownerUserId: string,
    limit: number,
  ) => Promise<ReadonlyArray<AgentScopedGrantReceiptRecord>>
  readAgent: (agentUserId: string) => Promise<OwnerAgentRecord | undefined>
  readReceiptByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<AgentScopedGrantReceiptRecord | undefined>
  updateAgentMetadata: (
    agentUserId: string,
    metadataJson: string,
    updatedAt: string,
  ) => Promise<void>
}>

type ValidationResult<A> =
  | Readonly<{
      ok: true
      value: A
    }>
  | Readonly<{
      ok: false
      reason: string
    }>

type AgentScopedGrantRouteDependencies<
  Session extends AgentScopedGrantSession,
  Bindings,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  appOrigin: (env: Bindings) => string
  makeStore: (env: Bindings) => AgentScopedGrantStore
  makeUuid?: () => string
  nowIso?: () => string
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

const CustomerOrderScopes = new Set([
  'customer_orders.feedback',
  'customer_orders.read',
  'customer_orders.write',
])

const AgentSiteScopes = new Set([
  'sites:builder-session:create',
  'sites:deploy:request',
  'sites:preview:request',
  'sites:project:create',
  'sites:version:save',
])

const GrantMetadataKeys: Record<AgentScopedGrantKind, string> = {
  agent_sites: 'agentSiteGrants',
  customer_orders: 'customerOrderGrants',
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const isUniqueConstraintError = (error: unknown): boolean =>
  errorMessage(error).includes('UNIQUE constraint failed')

const rowToReceipt = (
  row: AgentScopedGrantReceiptRow,
): AgentScopedGrantReceiptRecord => ({
  action: row.action,
  agentCredentialId: row.agent_credential_id,
  agentUserId: row.agent_user_id,
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  grantId: row.grant_id,
  grantKind: row.grant_kind,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  ownerUserId: row.owner_user_id,
  reason: row.reason,
  receiptRef: row.receipt_ref,
  scopesJson: row.scopes_json,
  status: row.status,
  targetJson: row.target_json,
})

const jsonRecord = (value: string): Record<string, unknown> =>
  parseJsonRecord(value) ?? {}

const jsonRecordValue = (value: unknown): Record<string, unknown> =>
  recordFromUnknown(value) ?? {}

const jsonArrayValue = (value: unknown): ReadonlyArray<unknown> =>
  arrayFromUnknown(value) ?? []

const safeJson = (value: unknown): string => JSON.stringify(value)

const sameStringSet = (
  left: ReadonlyArray<string>,
  right: ReadonlyArray<string>,
): boolean => {
  const sortedLeft = Array.from(new Set(left)).sort()
  const sortedRight = Array.from(new Set(right)).sort()

  return (
    sortedLeft.length === sortedRight.length &&
    sortedLeft.every((value, index) => value === sortedRight[index])
  )
}

const normalizedScopes = (
  grantKind: AgentScopedGrantKind,
  scopes: ReadonlyArray<string>,
): ValidationResult<ReadonlyArray<string>> => {
  const allowed =
    grantKind === 'customer_orders'
      ? CustomerOrderScopes
      : AgentSiteScopes
  const normalized = Array.from(new Set(scopes.map(scope => scope.trim())))
    .filter(scope => scope !== '')
    .sort()

  if (normalized.length === 0) {
    return {
      ok: false,
      reason: 'At least one scope is required.',
    }
  }

  const unknown = normalized.find(scope => !allowed.has(scope))

  if (unknown !== undefined) {
    return {
      ok: false,
      reason: `Unsupported ${grantKind} scope: ${unknown}.`,
    }
  }

  return {
    ok: true,
    value: normalized,
  }
}

const targetForGrant = (
  grantKind: AgentScopedGrantKind,
  body: Record<string, unknown>,
): ValidationResult<Record<string, unknown>> => {
  if (grantKind === 'agent_sites') {
    const siteId = optionalString(body.siteId)

    return {
      ok: true,
      value: siteId === undefined ? {} : { siteId },
    }
  }

  return {
    ok: true,
    value: {},
  }
}

const targetMatches = (
  grantKind: AgentScopedGrantKind,
  grant: Record<string, unknown>,
  target: Record<string, unknown>,
): boolean => {
  if (grantKind === 'agent_sites') {
    return optionalString(grant.siteId) === optionalString(target.siteId)
  }

  return true
}

const grantProjection = (
  grantKind: AgentScopedGrantKind,
  grant: Record<string, unknown>,
) => ({
  expiresAt: optionalString(grant.expiresAt) ?? null,
  grantId: optionalString(grant.grantId) ?? null,
  grantKind,
  ownerUserId: optionalString(grant.ownerUserId) ?? null,
  scopes: stringArrayFromUnknown(grant.scopes),
  siteId:
    grantKind === 'agent_sites' ? optionalString(grant.siteId) ?? null : undefined,
  status: optionalString(grant.status) ?? 'active',
})

const allGrantRecords = (
  metadata: Record<string, unknown>,
): ReadonlyArray<Readonly<{ grant: Record<string, unknown>; kind: AgentScopedGrantKind }>> =>
  (['customer_orders', 'agent_sites'] as const).flatMap(kind =>
    jsonArrayValue(metadata[GrantMetadataKeys[kind]])
      .map(jsonRecordValue)
      .map(grant => ({ grant, kind })),
  )

const grantsForOwner = (
  metadata: Record<string, unknown>,
  ownerUserId: string,
): ReadonlyArray<ReturnType<typeof grantProjection>> =>
  allGrantRecords(metadata)
    .filter(({ grant }) => optionalString(grant.ownerUserId) === ownerUserId)
    .map(({ grant, kind }) => grantProjection(kind, grant))

const activeDuplicateGrant = (
  metadata: Record<string, unknown>,
  input: Readonly<{
    expiresAt: string | null
    grantKind: AgentScopedGrantKind
    nowIso: string
    ownerUserId: string
    scopes: ReadonlyArray<string>
    target: Record<string, unknown>
  }>,
): Record<string, unknown> | undefined =>
  jsonArrayValue(metadata[GrantMetadataKeys[input.grantKind]])
    .map(jsonRecordValue)
    .find(grant => {
      const expiresAt = optionalString(grant.expiresAt)

      return (
        optionalString(grant.status) === 'active' &&
        optionalString(grant.ownerUserId) === input.ownerUserId &&
        (expiresAt === undefined || expiresAt > input.nowIso) &&
        sameStringSet(stringArrayFromUnknown(grant.scopes), input.scopes) &&
        targetMatches(input.grantKind, grant, input.target)
      )
    })

const appendGrant = (
  metadata: Record<string, unknown>,
  grantKind: AgentScopedGrantKind,
  grant: Record<string, unknown>,
): Record<string, unknown> => ({
  ...metadata,
  [GrantMetadataKeys[grantKind]]: [
    ...jsonArrayValue(metadata[GrantMetadataKeys[grantKind]]),
    grant,
  ],
})

type RevokeGrantResult =
  | Readonly<{
      kind: 'not_found'
    }>
  | Readonly<{
      kind: 'wrong_owner'
    }>
  | Readonly<{
      grant: Record<string, unknown>
      grantKind: AgentScopedGrantKind
      kind: 'revoked'
      metadata: Record<string, unknown>
    }>

const revokeGrant = (
  metadata: Record<string, unknown>,
  input: Readonly<{
    grantId: string
    nowIso: string
    ownerUserId: string
  }>,
): RevokeGrantResult =>
  (['customer_orders', 'agent_sites'] as const)
    .map(grantKind => {
      const key = GrantMetadataKeys[grantKind]
      const grants = jsonArrayValue(metadata[key]).map(jsonRecordValue)
      const index = grants.findIndex(
        grant => optionalString(grant.grantId) === input.grantId,
      )
      const grant = index === -1 ? undefined : grants[index]

      if (grant === undefined) {
        return { kind: 'not_found' } as const
      }

      if (optionalString(grant.ownerUserId) !== input.ownerUserId) {
        return { kind: 'wrong_owner' } as const
      }

      const revoked = {
        ...grant,
        revokedAt: input.nowIso,
        status: 'revoked',
      }
      const nextGrants = grants.map((candidate, candidateIndex) =>
        candidateIndex === index ? revoked : candidate,
      )

      return {
        grant: revoked,
        grantKind,
        kind: 'revoked',
        metadata: {
          ...metadata,
          [key]: nextGrants,
        },
      } as const
    })
    .find(result => result.kind !== 'not_found') ?? { kind: 'not_found' }

const idempotencyKeyFromRequest = (request: Request): string | HttpResponse => {
  const key = optionalString(request.headers.get('idempotency-key'))

  if (key === undefined || key.length < 8 || key.length > 200) {
    return badRequest('Idempotency-Key header must be 8-200 characters.')
  }

  return key
}

const receiptProjection = (receipt: AgentScopedGrantReceiptRecord) => ({
  action: receipt.action,
  agentCredentialId: receipt.agentCredentialId,
  agentUserId: receipt.agentUserId,
  createdAt: receipt.createdAt,
  expiresAt: receipt.expiresAt,
  grantId: receipt.grantId,
  grantKind: receipt.grantKind,
  id: receipt.id,
  ownerUserId: receipt.ownerUserId,
  reason: receipt.reason,
  receiptRef: receipt.receiptRef,
  scopes: parseJsonStringArray(receipt.scopesJson),
  scopesJson: receipt.scopesJson,
  status: receipt.status,
  target: jsonRecord(receipt.targetJson),
})

const receiptResponse = (
  receipt: AgentScopedGrantReceiptRecord,
  status = 200,
) => noStoreJsonResponse({ receipt: receiptProjection(receipt) }, { status })

const agentProjection = (agent: OwnerAgentRecord, ownerUserId: string) => {
  const metadata = jsonRecord(agent.profileMetadataJson)

  return {
    agentUserId: agent.userId,
    avatarUrl: agent.avatarUrl,
    createdAt: agent.createdAt,
    displayName: agent.displayName,
    grants: grantsForOwner(metadata, ownerUserId),
    primaryEmail: agent.primaryEmail,
    slug: agent.slug,
    token: {
      expiresAt: agent.credentialExpiresAt,
      lastUsedAt: agent.credentialLastUsedAt,
      status: agent.credentialStatus,
      tokenPrefix: agent.tokenPrefix,
    },
    updatedAt: agent.updatedAt,
  }
}

const scopeCatalog = () => ({
  agent_sites: Array.from(AgentSiteScopes).sort(),
  customer_orders: Array.from(CustomerOrderScopes).sort(),
})

const grantKindFromBody = (
  value: unknown,
): AgentScopedGrantKind | undefined => {
  const grantKind = optionalString(value)

  return grantKind === 'agent_sites' ||
    grantKind === 'customer_orders'
    ? grantKind
    : undefined
}

const listResponse = async <Session extends AgentScopedGrantSession, Bindings>(
  dependencies: AgentScopedGrantRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const store = dependencies.makeStore(env)
  const [agents, claims, receipts] = await Promise.all([
    store.listAgents(),
    store.listAgentClaimsForOwner(session.user.userId),
    store.listReceiptsForOwner(session.user.userId, 20),
  ])

  return dependencies.appendRefreshedSessionCookies(
    noStoreJsonResponse({
      agents: agents.map(agent => agentProjection(agent, session.user.userId)),
      claims,
      ownerUserId: session.user.userId,
      receipts: receipts.map(receiptProjection),
      scopeCatalog: scopeCatalog(),
    }),
    session,
  )
}

const createGrantResponse = async <
  Session extends AgentScopedGrantSession,
  Bindings,
>(
  dependencies: AgentScopedGrantRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey instanceof Response) {
    return idempotencyKey
  }

  const body = await readJsonObject(request).catch(error =>
    jsonRecordValue({ error: errorMessage(error) }),
  )
  const agentUserId = optionalString(body.agentUserId)
  const grantKind = grantKindFromBody(body.grantKind)

  if (agentUserId === undefined) {
    return badRequest('agentUserId is required.')
  }

  if (grantKind === undefined) {
    return badRequest('grantKind must be customer_orders or agent_sites.')
  }

  if (containsProviderSecretMaterial(safeJson(body))) {
    return badRequest('Grant request cannot contain provider secret material.')
  }

  return applyCreateGrant(dependencies, env, {
    agentUserId,
    body,
    grantKind,
    idempotencyKey,
    ownerUserId: session.user.userId,
    wrapResponse: response =>
      dependencies.appendRefreshedSessionCookies(response, session),
  })
}

const applyCreateGrant = async <
  Session extends AgentScopedGrantSession,
  Bindings,
>(
  dependencies: AgentScopedGrantRouteDependencies<Session, Bindings>,
  env: Bindings,
  input: Readonly<{
    agentUserId: string
    body: Record<string, unknown>
    grantKind: AgentScopedGrantKind
    idempotencyKey: string
    ownerUserId: string
    wrapResponse?: (response: HttpResponse) => HttpResponse
  }>,
) => {
  const { agentUserId, body, grantKind, idempotencyKey, ownerUserId } = input
  const wrapResponse = input.wrapResponse ?? (response => response)
  const scopesResult = normalizedScopes(
    grantKind,
    stringArrayFromUnknown(body.scopes),
  )
  const targetResult = targetForGrant(grantKind, body)

  if (!scopesResult.ok) {
    return badRequest(scopesResult.reason)
  }

  if (!targetResult.ok) {
    return badRequest(targetResult.reason)
  }

  const scopes = scopesResult.value
  const target = targetResult.value
  const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
  const expiresAt = optionalString(body.expiresAt) ?? null

  if (expiresAt !== null && expiresAt <= nowIso) {
    return badRequest('expiresAt must be in the future.')
  }

  const store = dependencies.makeStore(env)
  const idempotencyKeyHash = await sha256Hex(
    `${ownerUserId}\n${idempotencyKey}`,
  )
  const existingReceipt =
    await store.readReceiptByIdempotencyKeyHash(idempotencyKeyHash)

  if (existingReceipt !== undefined) {
    return wrapResponse(
      receiptResponse({ ...existingReceipt, status: 'idempotent_replay' }),
    )
  }

  const agent = await store.readAgent(agentUserId)

  if (agent === undefined) {
    return notFound()
  }

  const metadata = jsonRecord(agent.profileMetadataJson)
  const duplicate = activeDuplicateGrant(metadata, {
    expiresAt,
    grantKind,
    nowIso,
    ownerUserId,
    scopes,
    target,
  })

  if (duplicate !== undefined) {
    return noStoreJsonResponse(
      {
        error: 'agent_scoped_grant_duplicate',
        existingGrant: grantProjection(grantKind, duplicate),
      },
      { status: 409 },
    )
  }

  const makeUuid = dependencies.makeUuid ?? randomUuid
  const grantId = `agent_grant_${makeUuid()}`
  const grant =
    grantKind === 'customer_orders'
      ? {
          expiresAt,
          grantId,
          ownerUserId,
          scopes,
          status: 'active',
        }
      : {
          expiresAt,
          grantId,
          ownerUserId,
          scopes,
          status: 'active',
          ...target,
        }
  const nextMetadata = appendGrant(metadata, grantKind, grant)
  const receiptId = `agent_scoped_grant_receipt_${makeUuid()}`
  const receipt: AgentScopedGrantReceiptRecord = {
    action: 'grant',
    agentCredentialId: agent.credentialId,
    agentUserId: agent.userId,
    createdAt: nowIso,
    expiresAt,
    grantId,
    grantKind,
    id: receiptId,
    idempotencyKeyHash,
    ownerUserId,
    reason: optionalString(body.reason) ?? null,
    receiptRef: `agent_scoped_grant_receipt_${grantId}`,
    scopesJson: safeJson(scopes),
    status: 'applied',
    targetJson: safeJson(target),
  }

  try {
    await store.updateAgentMetadata(agent.userId, safeJson(nextMetadata), nowIso)
    const savedReceipt = await store.createReceipt(receipt)

    return wrapResponse(
      noStoreJsonResponse(
        {
          agent: agentProjection(
            { ...agent, profileMetadataJson: safeJson(nextMetadata) },
            ownerUserId,
          ),
          grant: grantProjection(grantKind, grant),
          receipt: receiptProjection(savedReceipt),
        },
        { status: 201 },
      ),
    )
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return noStoreJsonResponse(
        { error: 'agent_scoped_grant_conflict' },
        { status: 409 },
      )
    }

    return serverError()
  }
}

const operatorCreateGrantResponse = async <
  Session extends AgentScopedGrantSession,
  Bindings,
>(
  dependencies: AgentScopedGrantRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
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

  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey instanceof Response) {
    return idempotencyKey
  }

  const body = await readJsonObject(request).catch(error =>
    jsonRecordValue({ error: errorMessage(error) }),
  )
  const agentUserId = optionalString(body.agentUserId)
  const grantKind = grantKindFromBody(body.grantKind)
  const ownerUserId = optionalString(body.ownerUserId)

  if (agentUserId === undefined) {
    return badRequest('agentUserId is required.')
  }

  if (ownerUserId === undefined) {
    return badRequest(
      'ownerUserId is required for operator-issued grants; use the owner linked by an approved agent claim.',
    )
  }

  if (grantKind === undefined) {
    return badRequest('grantKind must be customer_orders or agent_sites.')
  }

  if (containsProviderSecretMaterial(safeJson(body))) {
    return badRequest('Grant request cannot contain provider secret material.')
  }

  return applyCreateGrant(dependencies, env, {
    agentUserId,
    body,
    grantKind,
    idempotencyKey,
    ownerUserId,
  })
}

const revokeGrantResponse = async <
  Session extends AgentScopedGrantSession,
  Bindings,
>(
  dependencies: AgentScopedGrantRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  grantId: string,
) => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey instanceof Response) {
    return idempotencyKey
  }

  const store = dependencies.makeStore(env)
  const idempotencyKeyHash = await sha256Hex(
    `${session.user.userId}\n${idempotencyKey}`,
  )
  const existingReceipt =
    await store.readReceiptByIdempotencyKeyHash(idempotencyKeyHash)

  if (existingReceipt !== undefined) {
    return dependencies.appendRefreshedSessionCookies(
      receiptResponse({ ...existingReceipt, status: 'idempotent_replay' }),
      session,
    )
  }

  const agents = await store.listAgents()
  const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const reason = optionalString(body.reason) ?? null
  const matches = agents
    .map(agent => ({
      agent,
      result: revokeGrant(jsonRecord(agent.profileMetadataJson), {
        grantId,
        nowIso,
        ownerUserId: session.user.userId,
      }),
    }))
    .filter(match => match.result.kind !== 'not_found')

  if (matches.some(match => match.result.kind === 'wrong_owner')) {
    return noStoreJsonResponse(
      { error: 'agent_scoped_grant_wrong_owner' },
      { status: 403 },
    )
  }

  if (matches.length === 0) {
    return notFound()
  }

  const match = matches.find(candidate => candidate.result.kind === 'revoked')

  if (match === undefined || match.result.kind !== 'revoked') {
    return notFound()
  }

  const { agent, result } = match

  const receiptId = `agent_scoped_grant_receipt_${(dependencies.makeUuid ?? randomUuid)()}`
  const receipt: AgentScopedGrantReceiptRecord = {
    action: 'revoke',
    agentCredentialId: agent.credentialId,
    agentUserId: agent.userId,
    createdAt: nowIso,
    expiresAt: optionalString(result.grant.expiresAt) ?? null,
    grantId,
    grantKind: result.grantKind,
    id: receiptId,
    idempotencyKeyHash,
    ownerUserId: session.user.userId,
    reason,
    receiptRef: `agent_scoped_grant_revoke_receipt_${grantId}`,
    scopesJson: safeJson(stringArrayFromUnknown(result.grant.scopes)),
    status: 'applied',
    targetJson: safeJson(
      result.grantKind === 'agent_sites'
          ? { siteId: optionalString(result.grant.siteId) ?? null }
          : {},
    ),
  }

  try {
    await store.updateAgentMetadata(
      agent.userId,
      safeJson(result.metadata),
      nowIso,
    )
    const savedReceipt = await store.createReceipt(receipt)

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        agent: agentProjection(
          { ...agent, profileMetadataJson: safeJson(result.metadata) },
          session.user.userId,
        ),
        grant: grantProjection(result.grantKind, result.grant),
        receipt: receiptProjection(savedReceipt),
      }),
      session,
    )
  } catch {
    return serverError()
  }
}

const scopedGrantPageResponse = async <
  Session extends AgentScopedGrantSession,
  Bindings,
>(
  dependencies: AgentScopedGrantRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await dependencies.requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return unauthorized()
  }

  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Agent grants - OpenAgents</title>
  <link rel="stylesheet" href="/assets/openagents.css">
</head>
<body class="bg-black text-[#f1efe8] font-mono">
  <main class="mx-auto my-12 w-[min(100%-32px,920px)] border border-white/15 p-7">
    <p class="text-xs uppercase tracking-[0.12em] text-white/45">OpenAgents</p>
    <h1 class="mt-3 text-4xl font-semibold leading-none">Agent grants</h1>
    <p class="mt-4 max-w-3xl text-white/65">Grant narrow, revocable agent authority. Raw tokens are never shown here.</p>
    <section class="mt-8 border-t border-white/10 pt-6">
      <h2 class="text-xl">Current grants</h2>
      <pre id="grants" class="mt-4 overflow-auto whitespace-pre-wrap border border-white/10 p-4 text-sm text-white/75">Loading...</pre>
    </section>
  </main>
  <script>
    fetch('/api/agents/scoped-grants')
      .then(response => response.json())
      .then(body => {
        document.getElementById('grants').textContent = JSON.stringify(body, null, 2);
      })
      .catch(error => {
        document.getElementById('grants').textContent = error.message || String(error);
      });
  </script>
</body>
</html>`

  return dependencies.appendRefreshedSessionCookies(
    new Response(html, {
      headers: {
        'cache-control': 'no-store',
        'content-type': 'text/html; charset=utf-8',
      },
    }),
    session,
  )
}

export const makeD1AgentScopedGrantStore = (
  db: D1Database,
): AgentScopedGrantStore => ({
  createReceipt: async receipt => {
    await db
      .prepare(
        `INSERT INTO agent_scoped_grant_receipts
          (id, receipt_ref, idempotency_key_hash, owner_user_id, agent_user_id,
           agent_credential_id, grant_id, grant_kind, action, status,
           scopes_json, target_json, expires_at, reason, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        receipt.id,
        receipt.receiptRef,
        receipt.idempotencyKeyHash,
        receipt.ownerUserId,
        receipt.agentUserId,
        receipt.agentCredentialId,
        receipt.grantId,
        receipt.grantKind,
        receipt.action,
        receipt.status,
        receipt.scopesJson,
        receipt.targetJson,
        receipt.expiresAt,
        receipt.reason,
        receipt.createdAt,
      )
      .run()

    return receipt
  },
  listAgentClaimsForOwner: async ownerUserId => {
    const rows = await db
      .prepare(
        `SELECT id, status, display_name, agent_user_id, token_prefix, receipt_ref, requested_at
         FROM agent_owner_claims
         WHERE owner_user_id = ?
         ORDER BY requested_at DESC
         LIMIT 50`,
      )
      .bind(ownerUserId)
      .all<{
        agent_user_id: string | null
        display_name: string
        id: string
        receipt_ref: string
        requested_at: string
        status: string
        token_prefix: string | null
      }>()

    return rows.results.map(row => ({
      agentUserId: row.agent_user_id,
      claimId: row.id,
      displayName: row.display_name,
      receiptRef: row.receipt_ref,
      requestedAt: row.requested_at,
      status: row.status,
      tokenPrefix: row.token_prefix,
    }))
  },
  listAgents: async () => {
    const rows = await db
      .prepare(
        `SELECT
            users.id AS user_id,
            users.display_name,
            users.primary_email,
            users.avatar_url,
            users.created_at,
            users.updated_at,
            agent_profiles.slug,
            agent_profiles.metadata_json,
            agent_credentials.id AS credential_id,
            agent_credentials.token_prefix,
            agent_credentials.status AS credential_status,
            agent_credentials.last_used_at,
            agent_credentials.expires_at
         FROM users
         LEFT JOIN agent_profiles ON agent_profiles.user_id = users.id
         LEFT JOIN agent_credentials ON agent_credentials.user_id = users.id
         WHERE users.kind = 'agent'
           AND users.status = 'active'
           AND users.deleted_at IS NULL
         ORDER BY users.created_at DESC
         LIMIT 200`,
      )
      .all<{
        avatar_url: string | null
        created_at: string
        credential_id: string | null
        credential_status: string | null
        display_name: string
        expires_at: string | null
        last_used_at: string | null
        metadata_json: string | null
        primary_email: string | null
        slug: string | null
        token_prefix: string | null
        updated_at: string
        user_id: string
      }>()

    return rows.results.map(row => ({
      avatarUrl: row.avatar_url,
      createdAt: row.created_at,
      credentialExpiresAt: row.expires_at,
      credentialId: row.credential_id,
      credentialLastUsedAt: row.last_used_at,
      credentialStatus: row.credential_status,
      displayName: row.display_name,
      primaryEmail: row.primary_email,
      profileMetadataJson: row.metadata_json ?? '{}',
      slug: row.slug,
      tokenPrefix: row.token_prefix,
      updatedAt: row.updated_at,
      userId: row.user_id,
    }))
  },
  listReceiptsForOwner: async (ownerUserId, limit) => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM agent_scoped_grant_receipts
         WHERE owner_user_id = ?
         ORDER BY created_at DESC
         LIMIT ?`,
      )
      .bind(ownerUserId, limit)
      .all<AgentScopedGrantReceiptRow>()

    return rows.results.map(rowToReceipt)
  },
  readAgent: async agentUserId => {
    const agents = await makeD1AgentScopedGrantStore(db).listAgents()

    return agents.find(agent => agent.userId === agentUserId)
  },
  readReceiptByIdempotencyKeyHash: async idempotencyKeyHash => {
    const row = await db
      .prepare(
        `SELECT *
         FROM agent_scoped_grant_receipts
         WHERE idempotency_key_hash = ?`,
      )
      .bind(idempotencyKeyHash)
      .first<AgentScopedGrantReceiptRow>()

    return row === null ? undefined : rowToReceipt(row)
  },
  updateAgentMetadata: async (agentUserId, metadataJson, updatedAt) => {
    await db
      .prepare(
        `UPDATE agent_profiles
         SET metadata_json = ?,
             updated_at = ?
         WHERE user_id = ?`,
      )
      .bind(metadataJson, updatedAt, agentUserId)
      .run()
  },
})

export const makeAgentScopedGrantRoutes = <
  Session extends AgentScopedGrantSession,
  Bindings,
>(
  dependencies: AgentScopedGrantRouteDependencies<Session, Bindings>,
) => ({
  routeAgentScopedGrantRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/agents/scoped-grants') {
      return Effect.promise(() =>
        scopedGrantPageResponse(dependencies, request, env, ctx),
      )
    }

    if (url.pathname === '/api/operator/agents/scoped-grants') {
      return Effect.promise(() =>
        operatorCreateGrantResponse(dependencies, request, env),
      )
    }

    if (url.pathname === '/api/agents/scoped-grants') {
      return request.method === 'GET'
        ? Effect.promise(() => listResponse(dependencies, request, env, ctx))
        : Effect.promise(() =>
            createGrantResponse(dependencies, request, env, ctx),
          )
    }

    const revokeMatch =
      /^\/api\/agents\/scoped-grants\/([^/]+)\/revoke$/.exec(url.pathname)

    if (revokeMatch === null) {
      return undefined
    }

    const grantId = decodeURIComponent(revokeMatch[1] ?? '')

    return Effect.promise(() =>
      revokeGrantResponse(dependencies, request, env, ctx, grantId),
    )
  },
})
