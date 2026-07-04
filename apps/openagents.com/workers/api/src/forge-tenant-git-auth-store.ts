import {
  decodeForgeGitAccessTokenRow,
  decodeForgeGitAccessTokenScopeRow,
  decodeForgeTenantRow,
  type ForgeGitAccessScope,
  type ForgeGitAccessTokenRow,
  type ForgeGitAccessTokenScopeRow,
  type ForgeTenantConfidentialWorkspaceMode,
  type ForgeTenantRow,
  type ForgeTenantState,
} from '@openagentsinc/forge-protocol'
import {
  decideAgentDefinitionCompiledToolAuthority,
  type AgentDefinitionCompiledToolRuntimePolicy,
  type AgentDefinitionToolAuthorityDecision,
} from '@openagentsinc/agent-runtime-schema'

import { parseJsonStringArray } from './json-boundary'
import { randomUuid } from './runtime-primitives'

export const FORGE_GIT_TOKEN_PREFIX = 'oa_forge_git_'

export type ForgeTenantInput = Readonly<{
  tenantRef: string
  displayName: string
  state?: ForgeTenantState
  confidentialWorkspaceMode?: ForgeTenantConfidentialWorkspaceMode | null
  attestationRef?: string | null
  encryptedKnowledgePackRef?: string | null
  refusalReason?: string | null
  retentionPolicyRef?: string | null
  nowIso: string
}>

export type ForgeGitAccessTokenMintInput = Readonly<{
  tenantRef: string
  tokenRef: string
  subjectRef: string
  repositoryRef: string
  scopes: ReadonlyArray<ForgeGitAccessScope>
  refRestrictions?: ReadonlyArray<string>
  agentDefinitionToolPolicy?: AgentDefinitionCompiledToolRuntimePolicy
  expiresAt: string
  sourceRefs: ReadonlyArray<string>
  nowIso: string
}>

export type ForgeGitAccessTokenMintResult = Readonly<{
  token: string
  record: ForgeGitAccessTokenRow
  scopes: ReadonlyArray<ForgeGitAccessTokenScopeRow>
}>

export type ForgeGitAccessTokenSession = Readonly<{
  tenantRef: string
  tokenRef: string
  subjectRef: string
  repositoryRef: string
  tokenPrefix: string
  scopes: ReadonlyArray<ForgeGitAccessScope>
  refRestrictions: ReadonlyArray<string>
  authenticatedAt: string
}>

export type ForgeGitScopePolicyDecision = Readonly<{
  scope: ForgeGitAccessScope
  toolRef: string
  decision: AgentDefinitionToolAuthorityDecision
}>

export type ForgeGitAccessScopeCompilationResult =
  | Readonly<{
      status: 'allowed'
      scopes: ReadonlyArray<ForgeGitAccessScope>
      decisions: ReadonlyArray<ForgeGitScopePolicyDecision>
      blockerRefs: []
      escalationRefs: []
    }>
  | Readonly<{
      status: 'denied'
      scopes: []
      decisions: ReadonlyArray<ForgeGitScopePolicyDecision>
      blockerRefs: ReadonlyArray<string>
      escalationRefs: ReadonlyArray<string>
      reasonRef: string
    }>
  | Readonly<{
      status: 'operator_escalation_required'
      scopes: []
      decisions: ReadonlyArray<ForgeGitScopePolicyDecision>
      blockerRefs: ReadonlyArray<string>
      escalationRefs: ReadonlyArray<string>
      reasonRef: string
    }>

export type ForgeTenantGitAuthStore = Readonly<{
  upsertTenant: (input: ForgeTenantInput) => Promise<ForgeTenantRow>
  mintGitAccessToken: (
    input: ForgeGitAccessTokenMintInput,
    options?: Readonly<{ makeToken?: () => string }>,
  ) => Promise<ForgeGitAccessTokenMintResult>
  authenticateGitAccessToken: (
    input: Readonly<{
      token: string
      repositoryRef: string
      requiredScope: ForgeGitAccessScope
      nowIso: string
    }>,
  ) => Promise<ForgeGitAccessTokenSession | undefined>
  revokeGitAccessToken: (
    tenantRef: string,
    tokenRef: string,
    revokedAt: string,
  ) => Promise<ForgeGitAccessTokenRow | undefined>
  readGitAccessToken: (
    tenantRef: string,
    tokenRef: string,
  ) => Promise<ForgeGitAccessTokenRow | undefined>
}>

class ForgeTenantGitAuthStoreError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForgeTenantGitAuthStoreError'
  }
}

const textEncoder = new TextEncoder()
const sha256Pattern = /^[0-9a-f]{64}$/i

const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')

const jsonArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...values])

const rowOrFail = <T>(row: T | null, label: string): T => {
  if (row === null) {
    throw new ForgeTenantGitAuthStoreError(`${label} was not persisted`)
  }
  return row
}

const assertToken = (token: string): string => {
  if (!token.startsWith(FORGE_GIT_TOKEN_PREFIX)) {
    throw new ForgeTenantGitAuthStoreError(
      'forge git access token has an invalid prefix',
    )
  }
  return token
}

const assertTokenHash = (tokenHash: string): string => {
  if (!sha256Pattern.test(tokenHash)) {
    throw new ForgeTenantGitAuthStoreError(
      'forge git access token hash must be a SHA-256 hex digest',
    )
  }
  return tokenHash.toLowerCase()
}

const normalizeScopes = (
  scopes: ReadonlyArray<ForgeGitAccessScope>,
): ReadonlyArray<ForgeGitAccessScope> => {
  const unique = [...new Set(scopes)]
  if (unique.length === 0) {
    throw new ForgeTenantGitAuthStoreError(
      'forge git access token must grant at least one scope',
    )
  }
  return unique
}

const normalizeRefRestrictions = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(
    (refs ?? []).map(ref => ref.trim()).filter(ref => ref !== ''),
  )

  const invalid = normalized.find(ref =>
    ref.length > 260 || !/^refs\/[A-Za-z0-9._/-]+$/.test(ref)
  )
  if (invalid !== undefined) {
    throw new ForgeTenantGitAuthStoreError(
      `forge git access token ref restriction is invalid: ${invalid}`,
    )
  }

  return normalized
}

export const FORGE_GIT_SCOPE_TOOL_REFS: Record<ForgeGitAccessScope, string> = {
  'git:admin': 'tool.openagents.forge.git.admin',
  'git:receive-pack': 'tool.openagents.forge.git.receive_pack',
  'git:upload-pack': 'tool.openagents.forge.git.upload_pack',
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].sort()

export const compileAgentDefinitionForgeGitAccessScopes = (input: Readonly<{
  policy: AgentDefinitionCompiledToolRuntimePolicy
  requestedScopes: ReadonlyArray<ForgeGitAccessScope>
  invocationRef?: string
}>): ForgeGitAccessScopeCompilationResult => {
  const requestedScopes = normalizeScopes(input.requestedScopes)
  const decisions = requestedScopes.map((scope): ForgeGitScopePolicyDecision => {
    const toolRef = FORGE_GIT_SCOPE_TOOL_REFS[scope]
    return {
      scope,
      toolRef,
      decision: decideAgentDefinitionCompiledToolAuthority({
        policy: input.policy,
        toolRef,
        invocationRef: input.invocationRef === undefined
          ? `forge_git_scope:${scope}`
          : `${input.invocationRef}:${scope}`,
      }),
    }
  })

  const denied = decisions.find(item => item.decision.status === 'denied')
  if (denied !== undefined) {
    return {
      status: 'denied',
      scopes: [],
      decisions,
      blockerRefs: uniqueRefs(decisions.flatMap(item => item.decision.blockerRefs)),
      escalationRefs: [],
      reasonRef: denied.decision.reasonRef,
    }
  }

  const escalations = decisions
    .map(item => item.decision.escalation?.escalationRef)
    .filter((ref): ref is string => ref !== undefined)
  if (escalations.length > 0) {
    return {
      status: 'operator_escalation_required',
      scopes: [],
      decisions,
      blockerRefs: uniqueRefs(decisions.flatMap(item => item.decision.blockerRefs)),
      escalationRefs: uniqueRefs(escalations),
      reasonRef: 'reason.agent_definition.forge_git_scope_requires_operator',
    }
  }

  return {
    status: 'allowed',
    scopes: requestedScopes,
    decisions,
    blockerRefs: [],
    escalationRefs: [],
  }
}

export const createForgeGitAccessToken = (
  makeUuid: () => string = randomUuid,
): string =>
  `${FORGE_GIT_TOKEN_PREFIX}${makeUuid().replaceAll('-', '')}${makeUuid().replaceAll(
    '-',
    '',
  )}`

export const forgeGitAccessTokenPrefix = (token: string): string =>
  assertToken(token).slice(0, FORGE_GIT_TOKEN_PREFIX.length + 16)

export const forgeGitAccessTokenHash = async (
  token: string,
): Promise<string> =>
  bytesToHex(
    new Uint8Array(
      await crypto.subtle.digest(
        'SHA-256',
        textEncoder.encode(assertToken(token)),
      ),
    ),
  )

const readToken = async (
  db: D1Database,
  tenantRef: string,
  tokenRef: string,
): Promise<ForgeGitAccessTokenRow | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_git_access_tokens
        WHERE tenant_ref = ? AND token_ref = ?
        LIMIT 1
      `,
    )
    .bind(tenantRef, tokenRef)
    .first()

  return row === null ? undefined : decodeForgeGitAccessTokenRow(row)
}

const readTenant = async (
  db: D1Database,
  tenantRef: string,
): Promise<ForgeTenantRow | undefined> => {
  const row = await db
    .prepare(
      `
        SELECT *
        FROM forge_tenants
        WHERE tenant_ref = ?
        LIMIT 1
      `,
    )
    .bind(tenantRef)
    .first()

  return row === null ? undefined : decodeForgeTenantRow(row)
}

const readTokenScopes = async (
  db: D1Database,
  tenantRef: string,
  tokenRef: string,
): Promise<ReadonlyArray<ForgeGitAccessTokenScopeRow>> => {
  const rows = await db
    .prepare(
      `
        SELECT *
        FROM forge_git_access_token_scopes
        WHERE tenant_ref = ? AND token_ref = ?
        ORDER BY scope ASC
      `,
    )
    .bind(tenantRef, tokenRef)
    .all()
  return (rows.results ?? []).map(row =>
    decodeForgeGitAccessTokenScopeRow(row),
  )
}

const requiredScopePredicate = (
  requiredScope: ForgeGitAccessScope,
): ReadonlyArray<ForgeGitAccessScope> =>
  requiredScope === 'git:admin'
    ? ['git:admin']
    : [requiredScope, 'git:admin']

export const makeD1ForgeTenantGitAuthStore = (
  db: D1Database,
): ForgeTenantGitAuthStore => ({
  async upsertTenant(input) {
    await db
      .prepare(
        `
          INSERT INTO forge_tenants (
            tenant_ref,
            display_name,
            state,
            confidential_workspace_mode,
            attestation_ref,
            encrypted_knowledge_pack_ref,
            refusal_reason,
            retention_policy_ref,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT (tenant_ref) DO UPDATE SET
            display_name = excluded.display_name,
            state = excluded.state,
            confidential_workspace_mode = excluded.confidential_workspace_mode,
            attestation_ref = excluded.attestation_ref,
            encrypted_knowledge_pack_ref = excluded.encrypted_knowledge_pack_ref,
            refusal_reason = excluded.refusal_reason,
            retention_policy_ref = excluded.retention_policy_ref,
            updated_at = excluded.updated_at
        `,
      )
      .bind(
        input.tenantRef,
        input.displayName,
        input.state ?? 'active',
        input.confidentialWorkspaceMode ?? null,
        input.attestationRef ?? null,
        input.encryptedKnowledgePackRef ?? null,
        input.refusalReason ?? null,
        input.retentionPolicyRef ?? null,
        input.nowIso,
        input.nowIso,
      )
      .run()

    return decodeForgeTenantRow(
      rowOrFail(
        await db
          .prepare(
            `
              SELECT *
              FROM forge_tenants
              WHERE tenant_ref = ?
            `,
          )
          .bind(input.tenantRef)
          .first(),
        'forge tenant',
      ),
    )
  },

  async mintGitAccessToken(input, options) {
    const tenant = await readTenant(db, input.tenantRef)
    if (tenant === undefined || tenant.state !== 'active') {
      throw new ForgeTenantGitAuthStoreError(
        'forge git access token requires an active tenant',
      )
    }
    if (Date.parse(input.expiresAt) <= Date.parse(input.nowIso)) {
      throw new ForgeTenantGitAuthStoreError(
        'forge git access token expiry must be in the future',
      )
    }

    const token = assertToken(options?.makeToken?.() ?? createForgeGitAccessToken())
    const tokenHash = assertTokenHash(await forgeGitAccessTokenHash(token))
    const tokenPrefix = forgeGitAccessTokenPrefix(token)
    const requestedScopes = normalizeScopes(input.scopes)
    const refRestrictions = normalizeRefRestrictions(input.refRestrictions)
    const compiledScopes = input.agentDefinitionToolPolicy === undefined
      ? {
          status: 'allowed' as const,
          scopes: requestedScopes,
          decisions: [],
          blockerRefs: [] as const,
          escalationRefs: [] as const,
        }
      : compileAgentDefinitionForgeGitAccessScopes({
          policy: input.agentDefinitionToolPolicy,
          requestedScopes,
          invocationRef: input.tokenRef,
        })
    if (compiledScopes.status !== 'allowed') {
      throw new ForgeTenantGitAuthStoreError(
        `forge git access token scope rejected by compiled agent definition policy: ${compiledScopes.reasonRef}`,
      )
    }
    const scopes = compiledScopes.scopes

    await db
      .prepare(
        `
          INSERT INTO forge_git_access_tokens (
            tenant_ref,
            token_ref,
            subject_ref,
            repository_ref,
            token_hash,
            token_prefix,
            state,
            created_at,
            expires_at,
            last_used_at,
            revoked_at,
            source_refs_json,
            ref_restrictions_json
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `,
      )
      .bind(
        input.tenantRef,
        input.tokenRef,
        input.subjectRef,
        input.repositoryRef,
        tokenHash,
        tokenPrefix,
        'active',
        input.nowIso,
        input.expiresAt,
        null,
        null,
        jsonArray(input.sourceRefs),
        jsonArray(refRestrictions),
      )
      .run()

    for (const scope of scopes) {
      await db
        .prepare(
          `
            INSERT INTO forge_git_access_token_scopes (
              tenant_ref,
              token_ref,
              scope,
              created_at
            ) VALUES (?, ?, ?, ?)
          `,
        )
        .bind(input.tenantRef, input.tokenRef, scope, input.nowIso)
        .run()
    }

    const record = await readToken(db, input.tenantRef, input.tokenRef)
    return {
      token,
      record: rowOrFail(record ?? null, 'forge git access token'),
      scopes: await readTokenScopes(db, input.tenantRef, input.tokenRef),
    }
  },

  async authenticateGitAccessToken(input) {
    if (!input.token.startsWith(FORGE_GIT_TOKEN_PREFIX)) {
      return undefined
    }

    const tokenHash = await forgeGitAccessTokenHash(input.token)
    const row = await db
      .prepare(
        `
          SELECT tokens.*
          FROM forge_git_access_tokens tokens
          INNER JOIN forge_tenants tenants
            ON tenants.tenant_ref = tokens.tenant_ref
          WHERE tokens.token_hash = ?
            AND tokens.state = 'active'
            AND tenants.state = 'active'
          LIMIT 1
        `,
      )
      .bind(tokenHash)
      .first()
    if (row === null) {
      return undefined
    }

    const tokenRecord = decodeForgeGitAccessTokenRow(row)
    if (tokenRecord.repository_ref !== input.repositoryRef) {
      return undefined
    }
    if (Date.parse(tokenRecord.expires_at) <= Date.parse(input.nowIso)) {
      await db
        .prepare(
          `
            UPDATE forge_git_access_tokens
            SET state = 'expired'
            WHERE tenant_ref = ? AND token_ref = ? AND state = 'active'
          `,
        )
        .bind(tokenRecord.tenant_ref, tokenRecord.token_ref)
        .run()
      return undefined
    }

    const allowedScopes = requiredScopePredicate(input.requiredScope)
    const scopeRow = await db
      .prepare(
        `
          SELECT *
          FROM forge_git_access_token_scopes
          WHERE tenant_ref = ?
            AND token_ref = ?
            AND scope IN (${allowedScopes.map(() => '?').join(', ')})
          LIMIT 1
        `,
      )
      .bind(tokenRecord.tenant_ref, tokenRecord.token_ref, ...allowedScopes)
      .first()
    if (scopeRow === null) {
      return undefined
    }

    await db
      .prepare(
        `
          UPDATE forge_git_access_tokens
          SET last_used_at = ?
          WHERE tenant_ref = ? AND token_ref = ?
        `,
      )
      .bind(input.nowIso, tokenRecord.tenant_ref, tokenRecord.token_ref)
      .run()

    const scopeRows = await readTokenScopes(
      db,
      tokenRecord.tenant_ref,
      tokenRecord.token_ref,
    )
    return {
      tenantRef: tokenRecord.tenant_ref,
      tokenRef: tokenRecord.token_ref,
      subjectRef: tokenRecord.subject_ref,
      repositoryRef: tokenRecord.repository_ref,
      tokenPrefix: tokenRecord.token_prefix,
      scopes: scopeRows.map(scope => scope.scope),
      refRestrictions: parseJsonStringArray(tokenRecord.ref_restrictions_json),
      authenticatedAt: input.nowIso,
    }
  },

  async revokeGitAccessToken(tenantRef, tokenRef, revokedAt) {
    await db
      .prepare(
        `
          UPDATE forge_git_access_tokens
          SET state = 'revoked', revoked_at = ?
          WHERE tenant_ref = ? AND token_ref = ? AND state = 'active'
        `,
      )
      .bind(revokedAt, tenantRef, tokenRef)
      .run()
    return readToken(db, tenantRef, tokenRef)
  },

  readGitAccessToken: (tenantRef, tokenRef) =>
    readToken(db, tenantRef, tokenRef),
})
