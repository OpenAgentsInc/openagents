/**
 * Scoped MCP grants + principals for the CRM MCP server (epic #5991, #5995).
 *
 * Server-authoritative: a scoped credential is a `crm_mcp_grants` row binding a
 * set of authority classes + a single tenant to a hashed token. The transport
 * authenticates a request to exactly one `McpPrincipal` — the admin token (full
 * CRM authority on a header/default tenant) or a scoped grant — and the catalog
 * filters tools/resources by the principal's grants and reads ONLY its bound
 * tenant. Raw tokens are never stored (only SHA-256) and shown once at mint.
 */
import {
  type OpenAgentsMcpAuthorityClass,
  type OpenAgentsMcpGrant,
} from '@openagentsinc/mcp-contract'

// KS-8.11 (#8322): CrmEmailDatabase union — grant mint/revoke mirror their
// crm_mcp_grants rows to Postgres fail-soft (token hashes only, never tokens).
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  mirrorCrmEmailRows,
} from './crm-email-domain-store'
import type { McpPrincipal } from './crm-mcp-routes'
import {
  type CrmRuntime,
  DEFAULT_CRM_TENANT_REF,
  defaultCrmRuntime,
} from './crm-store'
import { parseJsonStringArray } from './json-boundary'

/** Authority classes the CRM MCP tools use across all waves. */
export const CRM_MCP_AUTHORITY_CLASSES: ReadonlyArray<OpenAgentsMcpAuthorityClass> =
  ['operator_read', 'approval_resolution', 'workspace_write']

const MCP_TOKEN_PREFIX = 'oa_mcp_'

/** Typed grant error (named subclass, not a generic thrown Error). */
class CrmMcpGrantError extends Error {}

export const readMcpBearerToken = (request: Request): string | undefined => {
  const header = request.headers.get('authorization')
  if (header === null) return undefined
  const [scheme, token] = header.split(' ')
  return scheme?.toLowerCase() === 'bearer' && token !== undefined
    ? token
    : undefined
}

export const mcpTenantHeader = (request: Request): string => {
  const value = request.headers.get('x-openagents-tenant')
  return value === null || value.trim() === ''
    ? DEFAULT_CRM_TENANT_REF
    : value.trim()
}

/** SHA-256 hex of a token (Web Crypto; available in the Worker runtime). */
export const hashCrmMcpToken = async (token: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(token),
  )
  return [...new Uint8Array(digest)]
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
}

const grantsFor = (
  grantRef: string,
  subjectRef: string,
  authorities: ReadonlyArray<OpenAgentsMcpAuthorityClass>,
  grantedAt: string,
): ReadonlyArray<OpenAgentsMcpGrant> =>
  authorities.map(authorityClass => ({
    authorityClass,
    decision: 'granted' as const,
    grantRef,
    grantedAt,
    scopeRefs: [],
    sourceRefs: ['crm_mcp_grants'],
    subjectRef,
  }))

/** The admin principal: full CRM authority on the requested/default tenant. */
export const crmMcpAdminPrincipal = (
  tenantRef: string,
  nowIso: string,
): McpPrincipal => ({
  grants: grantsFor('grant.admin', 'admin', CRM_MCP_AUTHORITY_CLASSES, nowIso),
  subjectRef: 'admin',
  tenantRef,
})

// --- store ------------------------------------------------------------------

export type CrmMcpGrantSummary = Readonly<{
  grantRef: string
  tenantRef: string
  authorities: ReadonlyArray<string>
  label: string | null
  status: string
  createdAt: string
  expiresAt: string | null
}>

const VALID_AUTHORITIES = new Set<string>(CRM_MCP_AUTHORITY_CLASSES)

export const mintCrmMcpGrant = async (
  db: CrmEmailDatabase,
  input: Readonly<{
    tenantRef: string
    authorities: ReadonlyArray<string>
    label?: string | null
    expiresAt?: string | null
  }>,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<
  Readonly<{ grantRef: string; token: string; summary: CrmMcpGrantSummary }>
> => {
  const authorities = input.authorities.filter(a => VALID_AUTHORITIES.has(a))
  if (authorities.length === 0) {
    throw new CrmMcpGrantError('at least one valid authority class is required')
  }
  const grantRef = runtime.makeId('crm_mcp_grant')
  const token = runtime.makeId(MCP_TOKEN_PREFIX.replace(/_$/, ''))
  const tokenHash = await hashCrmMcpToken(token)
  const now = runtime.nowIso()
  await crmEmailAuthorityDb(db)
    .prepare(
      `INSERT INTO crm_mcp_grants (
         id, grant_ref, token_hash, tenant_ref, authority_classes_json, label,
         status, created_at, expires_at
       ) VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?)`,
    )
    .bind(
      runtime.makeId('crm_mcp_grant_row'),
      grantRef,
      tokenHash,
      input.tenantRef,
      JSON.stringify(authorities),
      input.label ?? null,
      now,
      input.expiresAt ?? null,
    )
    .run()
  await mirrorCrmEmailRows(db, 'crm_mcp_grants', 'grant_ref', [grantRef])
  return {
    grantRef,
    summary: {
      authorities,
      createdAt: now,
      expiresAt: input.expiresAt ?? null,
      grantRef,
      label: input.label ?? null,
      status: 'active',
      tenantRef: input.tenantRef,
    },
    token,
  }
}

type GrantRow = Readonly<{
  grant_ref: string
  tenant_ref: string
  authority_classes_json: string
  label: string | null
  status: string
  created_at: string
  expires_at: string | null
}>

const validAuthority = (value: string): value is OpenAgentsMcpAuthorityClass =>
  VALID_AUTHORITIES.has(value)

/** Resolve a scoped bearer token to a principal, or null if invalid/expired/revoked. */
export const resolveCrmMcpGrantPrincipal = async (
  db: CrmEmailDatabase,
  token: string,
  nowIso: string,
): Promise<McpPrincipal | null> => {
  const tokenHash = await hashCrmMcpToken(token)
  const row = await crmEmailAuthorityDb(db)
    .prepare(
      `SELECT grant_ref, tenant_ref, authority_classes_json, label, status, created_at, expires_at
         FROM crm_mcp_grants WHERE token_hash = ? AND status = 'active' LIMIT 1`,
    )
    .bind(tokenHash)
    .first<GrantRow>()
  if (row === null) return null
  if (row.expires_at !== null && row.expires_at <= nowIso) return null
  const authorities = parseJsonStringArray(row.authority_classes_json).filter(
    validAuthority,
  )
  if (authorities.length === 0) return null
  return {
    grants: grantsFor(
      row.grant_ref,
      row.grant_ref,
      authorities,
      row.created_at,
    ),
    subjectRef: row.grant_ref,
    tenantRef: row.tenant_ref,
  }
}

export const listCrmMcpGrants = async (
  db: CrmEmailDatabase,
  tenantRef: string,
): Promise<ReadonlyArray<CrmMcpGrantSummary>> => {
  const result = await crmEmailAuthorityDb(db)
    .prepare(
      `SELECT grant_ref, tenant_ref, authority_classes_json, label, status, created_at, expires_at
         FROM crm_mcp_grants WHERE tenant_ref = ? ORDER BY created_at DESC LIMIT 200`,
    )
    .bind(tenantRef)
    .all<GrantRow>()
  return (result.results ?? []).map(row => ({
    authorities: parseJsonStringArray(row.authority_classes_json),
    createdAt: row.created_at,
    expiresAt: row.expires_at,
    grantRef: row.grant_ref,
    label: row.label,
    status: row.status,
    tenantRef: row.tenant_ref,
  }))
}

export const revokeCrmMcpGrant = async (
  db: CrmEmailDatabase,
  tenantRef: string,
  grantRef: string,
  runtime: CrmRuntime = defaultCrmRuntime,
): Promise<boolean> => {
  void runtime
  const result = await crmEmailAuthorityDb(db)
    .prepare(
      `UPDATE crm_mcp_grants SET status = 'revoked' WHERE tenant_ref = ? AND grant_ref = ? AND status = 'active'`,
    )
    .bind(tenantRef, grantRef)
    .run()
  await mirrorCrmEmailRows(db, 'crm_mcp_grants', 'grant_ref', [grantRef])
  return (result.meta?.changes ?? 0) > 0
}
