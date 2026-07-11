import { createHash } from "node:crypto"

import { Schema as S } from "effect"

export const CLAUDE_OWNER_LOCAL_PERMISSION_AUTHORITY_SCHEMA =
  "openagents.pylon.claude_owner_local_permission_authority.v1" as const
export const CLAUDE_OWNER_LOCAL_PERMISSION_POLICY_REF =
  "policy.pylon.claude.owner_local_bypass.v1" as const
export const CLAUDE_OWNER_LOCAL_PERMISSION_TTL_MS = 30 * 60 * 1_000

const boundedRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(181),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u),
)
const isoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
)

export const ClaudeOwnerLocalPermissionAuthoritySchema = S.Struct({
  schema: S.Literal(CLAUDE_OWNER_LOCAL_PERMISSION_AUTHORITY_SCHEMA),
  authorityRef: S.String.check(
    S.isPattern(/^authority\.pylon\.claude_owner_local\.[0-9a-f]{24}$/u),
  ),
  authorizationRef: S.String.check(
    S.isPattern(/^authorization\.pylon\.claude_owner_local\.[0-9a-f]{24}$/u),
  ),
  pylonRef: boundedRef,
  runRef: boundedRef,
  operationRef: boundedRef,
  accountRefHash: boundedRef,
  issuedAt: isoTimestamp,
  expiresAt: isoTimestamp,
  revokedAt: S.NullOr(isoTimestamp),
})

export type ClaudeOwnerLocalPermissionAuthority =
  typeof ClaudeOwnerLocalPermissionAuthoritySchema.Type

export type ClaudeOwnerLocalPermissionControl = Readonly<{
  authority: ClaudeOwnerLocalPermissionAuthority
  /** Process-local cancellation only; never serialized or projected. */
  signal?: AbortSignal
}>

export type ClaudePermissionMode = "acceptEdits" | "bypassPermissions"

export type ClaudePermissionAdmission =
  | Readonly<{
      kind: "bounded"
      permissionMode: "acceptEdits"
      authorityRef: null
      auditReceiptRef: null
    }>
  | Readonly<{
      kind: "owner_local"
      permissionMode: "bypassPermissions"
      authorityRef: string
      auditReceiptRef: string
    }>
  | Readonly<{
      kind: "refused"
      permissionMode: "acceptEdits"
      authorityRef: string | null
      auditReceiptRef: string | null
      blockerRef:
        | "blocker.pylon.claude_owner_local_permission.invalid"
        | "blocker.pylon.claude_owner_local_permission.scope_mismatch"
        | "blocker.pylon.claude_owner_local_permission.expired"
        | "blocker.pylon.claude_owner_local_permission.revoked"
    }>

export type ClaudePermissionAudit = Readonly<{
  authorityRef: string
  auditReceiptRef: string
  policyRef: typeof CLAUDE_OWNER_LOCAL_PERMISSION_POLICY_REF
}>

// The schema makes scope auditable; this process-local issuer set makes the
// capability non-forgeable from wire/config JSON and naturally invalid after
// restart. A restarted trusted composition must mint a fresh scoped grant.
const issuedAuthorities = new WeakSet<object>()
const revokedAuthorities = new WeakSet<object>()

const digest = (value: string): string =>
  createHash("sha256").update(value).digest("hex").slice(0, 24)

const authorityAuditReceiptRef = (
  authority: ClaudeOwnerLocalPermissionAuthority,
): string =>
  `proof.pylon.claude_owner_local_permission.${digest([
    authority.authorityRef,
    authority.pylonRef,
    authority.runRef,
    authority.operationRef,
    authority.accountRefHash,
  ].join(":"))}`

export const decodeClaudeOwnerLocalPermissionAuthority =
  S.decodeUnknownSync(ClaudeOwnerLocalPermissionAuthoritySchema)

export const issueClaudeOwnerLocalPermissionAuthority = (input: Readonly<{
  /** Public-safe durable ref already accepted by the trusted composition. */
  authorizationRef: string
  pylonRef: string
  runRef: string
  operationRef: string
  accountRefHash: string
  now: Date
  ttlMs?: number
}>): ClaudeOwnerLocalPermissionAuthority => {
  const ttlMs = Math.min(
    CLAUDE_OWNER_LOCAL_PERMISSION_TTL_MS,
    Math.max(1, Math.trunc(input.ttlMs ?? CLAUDE_OWNER_LOCAL_PERMISSION_TTL_MS)),
  )
  const issuedAt = input.now.toISOString()
  const expiresAt = new Date(input.now.getTime() + ttlMs).toISOString()
  const scope = [
    input.authorizationRef,
    input.pylonRef,
    input.runRef,
    input.operationRef,
    input.accountRefHash,
    issuedAt,
    expiresAt,
  ].join(":")
  const authority = decodeClaudeOwnerLocalPermissionAuthority({
    schema: CLAUDE_OWNER_LOCAL_PERMISSION_AUTHORITY_SCHEMA,
    authorityRef: `authority.pylon.claude_owner_local.${digest(scope)}`,
    authorizationRef: input.authorizationRef,
    pylonRef: input.pylonRef,
    runRef: input.runRef,
    operationRef: input.operationRef,
    accountRefHash: input.accountRefHash,
    issuedAt,
    expiresAt,
    revokedAt: null,
  })
  issuedAuthorities.add(authority)
  return authority
}

export const projectClaudePermissionAudit = (
  admission: ClaudePermissionAdmission,
): ClaudePermissionAudit | null =>
  admission.kind === "owner_local"
    ? {
        authorityRef: admission.authorityRef,
        auditReceiptRef: admission.auditReceiptRef,
        policyRef: CLAUDE_OWNER_LOCAL_PERMISSION_POLICY_REF,
      }
    : null

export const revokeClaudeOwnerLocalPermissionAuthority = (
  authority: ClaudeOwnerLocalPermissionAuthority,
  revokedAt: Date,
): ClaudeOwnerLocalPermissionAuthority => {
  revokedAuthorities.add(authority)
  const revoked = decodeClaudeOwnerLocalPermissionAuthority({
    ...authority,
    revokedAt: revokedAt.toISOString(),
  })
  issuedAuthorities.add(revoked)
  revokedAuthorities.add(revoked)
  return revoked
}

export const admitClaudePermission = (input: Readonly<{
  control?: ClaudeOwnerLocalPermissionControl
  expected: Readonly<{
    pylonRef: string
    operationRef: string
    accountRefHash: string
    runRef?: string
  }>
  now: Date
}>): ClaudePermissionAdmission => {
  if (input.control === undefined) {
    return {
      kind: "bounded",
      permissionMode: "acceptEdits",
      authorityRef: null,
      auditReceiptRef: null,
    }
  }

  if (!issuedAuthorities.has(input.control.authority)) {
    return {
      kind: "refused",
      permissionMode: "acceptEdits",
      authorityRef: null,
      auditReceiptRef: null,
      blockerRef: "blocker.pylon.claude_owner_local_permission.invalid",
    }
  }

  let authority: ClaudeOwnerLocalPermissionAuthority
  try {
    authority = decodeClaudeOwnerLocalPermissionAuthority(input.control.authority)
  } catch {
    return {
      kind: "refused",
      permissionMode: "acceptEdits",
      authorityRef: null,
      auditReceiptRef: null,
      blockerRef: "blocker.pylon.claude_owner_local_permission.invalid",
    }
  }
  const auditReceiptRef = authorityAuditReceiptRef(authority)
  if (
    authority.pylonRef !== input.expected.pylonRef ||
    authority.operationRef !== input.expected.operationRef ||
    authority.accountRefHash !== input.expected.accountRefHash ||
    (input.expected.runRef !== undefined && authority.runRef !== input.expected.runRef)
  ) {
    return {
      kind: "refused",
      permissionMode: "acceptEdits",
      authorityRef: authority.authorityRef,
      auditReceiptRef,
      blockerRef: "blocker.pylon.claude_owner_local_permission.scope_mismatch",
    }
  }
  if (
    authority.revokedAt !== null ||
    revokedAuthorities.has(input.control.authority) ||
    input.control.signal?.aborted === true
  ) {
    return {
      kind: "refused",
      permissionMode: "acceptEdits",
      authorityRef: authority.authorityRef,
      auditReceiptRef,
      blockerRef: "blocker.pylon.claude_owner_local_permission.revoked",
    }
  }
  const issuedAt = Date.parse(authority.issuedAt)
  const expiresAt = Date.parse(authority.expiresAt)
  const now = input.now.getTime()
  if (
    !Number.isFinite(issuedAt) ||
    !Number.isFinite(expiresAt) ||
    issuedAt > now ||
    expiresAt <= now ||
    expiresAt - issuedAt > CLAUDE_OWNER_LOCAL_PERMISSION_TTL_MS
  ) {
    return {
      kind: "refused",
      permissionMode: "acceptEdits",
      authorityRef: authority.authorityRef,
      auditReceiptRef,
      blockerRef: "blocker.pylon.claude_owner_local_permission.expired",
    }
  }
  return {
    kind: "owner_local",
    permissionMode: "bypassPermissions",
    authorityRef: authority.authorityRef,
    auditReceiptRef,
  }
}
