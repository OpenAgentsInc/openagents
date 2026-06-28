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
  authenticatedAt: string
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
    const scopes = normalizeScopes(input.scopes)

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
            source_refs_json
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
