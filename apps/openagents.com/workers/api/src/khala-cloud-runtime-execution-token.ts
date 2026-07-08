// Seam A (#8503, AC-1) — mint-at-dispatch execution-token seam.
//
// THE PROBLEM. The org-cloud-runtime microVM turn makes ONE hosted
// `/v1/chat/completions` call under a programmatic AGENT bearer, and posts the
// EXACT `token_usage_events` receipt attributed to the mobile turn's OWNER.
// That call must authenticate as a real, active `agent_credentials` row and be
// owner-linked, but it must NOT be a long-lived standing secret sitting in a
// work-context blob. This module mints a FRESH, short-lived, owner-linked agent
// credential AT DISPATCH TIME, hands the raw token to the caller exactly once
// (to bake into the microVM's work-context `inference` block), and revokes it
// the instant the turn completes.
//
// STORAGE. `agent_credentials` is Postgres-authoritative in the same khala_sync
// database the auth gate reads on every request (CFG D1 evacuation, #8515 /
// KS-8.5 #8334): columns `id, user_id, openauth_user_id, token_hash,
// token_prefix, name, status ('active'|'revoked'), created_at, last_used_at,
// revoked_at, expires_at` (all text). The auth gate admits a token iff
// `status = 'active' AND revoked_at IS NULL AND (expires_at IS NULL OR
// expires_at > now)`, so a short `expires_at` bounds blast radius even if the
// explicit revoke is skipped, and the revoke flips BOTH `status` and
// `revoked_at` so a re-presented token fails closed immediately.
//
// OWNER LINKAGE. Both `user_id` and `openauth_user_id` are set to the mobile
// turn's `ownerUserId`, so the gate resolves the authenticated principal to the
// owner (there is no FK on `user_id`, so this is a pure attribution link, not a
// second agent identity). The single billable row for the turn is still the
// `/api/khala/cloud/runtime-turn-usage` receipt (owner-attributed) — this token
// only authenticates the internal org-capacity inference call.
//
// SECRET DISCIPLINE. The raw token is returned once and never logged, never
// re-read from storage (only its hash is stored), and never serialized outside
// the caller's in-memory work-context build.

import type { SyncSql } from '@openagentsinc/khala-sync-server'

import { createAgentToken, sha256Hex } from './agent-registration'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'

/** Default TTL for a minted execution token (10 minutes). Bounds blast radius
 * to well under a single microVM turn's hard timeout. */
export const DEFAULT_EXECUTION_TOKEN_TTL_SECONDS = 600

/** Credential `name` stamped on minted Seam A execution tokens. */
export const CLOUD_RUNTIME_EXECUTION_TOKEN_NAME =
  'seam-a.cloud-runtime.execution-token'

/** Stable id prefix for minted Seam A execution credentials. */
export const CLOUD_RUNTIME_EXECUTION_TOKEN_ID_PREFIX =
  'agentcred.seam-a.cloud-runtime'

/**
 * Stable system AGENT user the minted credential's `user_id` points at.
 *
 * WHY A SYSTEM AGENT USER, NOT THE OWNER. The inference-gateway auth gate
 * (`findAgentByTokenHash`) resolves a bearer's `agent_credentials.user_id`
 * against the `users` table filtered to `kind='agent' AND status='active'`, and
 * REJECTS (401) when it does not resolve. The turn OWNER is a human/OpenAuth
 * user (`github:…`), NOT an agent-kind user, so a credential minted with
 * `user_id = ownerUserId` can never authenticate the microVM's inference call.
 * So `user_id` is this stable service-agent identity (ensured to exist at mint),
 * while OWNER ATTRIBUTION rides `openauth_user_id = ownerUserId`: the single
 * billable `token_usage_events` receipt is attributed by the receipt BODY's
 * `ownerUserId`, gated by `requireOwnerPostingAuthority` matching this
 * credential's `openauth_user_id`. Attribution is unchanged; auth now resolves.
 */
export const CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_USER_ID =
  'agent.openagents-agent-computer'

/** Display name for the {@link CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_USER_ID} row. */
export const CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_DISPLAY_NAME =
  'OpenAgents Agent Computer'

/** The raw-token half of a mint, returned to the caller EXACTLY ONCE. */
export type MintedExecutionToken = Readonly<{
  /** `agent_credentials.id` — the handle to revoke after the turn. */
  credentialId: string
  /** The raw bearer, in memory only. NEVER log or persist this. */
  rawToken: string
  /** Non-secret display prefix (first 20 chars), safe for events/logs. */
  tokenPrefix: string
  /** Owner the token is linked to (== input.ownerUserId). */
  ownerUserId: string
  /** ISO mint time. */
  createdAt: string
  /** ISO expiry (createdAt + ttlSeconds). */
  expiresAt: string
}>

export type MintExecutionTokenInput = Readonly<{
  /** The mobile turn owner the token attributes to (e.g. `github:123`). */
  ownerUserId: string
  /** TTL in seconds. Default {@link DEFAULT_EXECUTION_TOKEN_TTL_SECONDS}. */
  ttlSeconds?: number | undefined
  /** Clock seam (default real wall clock). */
  now?: (() => string) | undefined
  /** Id seam (default `crypto.randomUUID`). */
  uuid?: (() => string) | undefined
  /** Raw-token seam (default `createAgentToken`). */
  createToken?: (() => string) | undefined
  /** Hash seam (default `sha256Hex`). */
  hash?: ((value: string) => Promise<string>) | undefined
}>

const isoPlusSeconds = (iso: string, seconds: number): string =>
  isoTimestampAfterIso(iso, seconds * 1000)

const resolveTtl = (ttl: number | undefined): number =>
  ttl !== undefined && Number.isSafeInteger(ttl) && ttl > 0
    ? ttl
    : DEFAULT_EXECUTION_TOKEN_TTL_SECONDS

/**
 * Mint a fresh, short-lived, owner-linked agent credential and INSERT it into
 * `agent_credentials`. Returns the raw token exactly once so the caller can
 * bake it into the microVM work-context; only its SHA-256 hash is persisted.
 */
export const mintCloudRuntimeExecutionToken = async (
  sql: SyncSql,
  input: MintExecutionTokenInput,
): Promise<MintedExecutionToken> => {
  const now = input.now ?? currentIsoTimestamp
  const uuid = input.uuid ?? randomUuid
  const createToken = input.createToken ?? createAgentToken
  const hash = input.hash ?? sha256Hex

  const createdAt = now()
  const expiresAt = isoPlusSeconds(createdAt, resolveTtl(input.ttlSeconds))
  const credentialId = `${CLOUD_RUNTIME_EXECUTION_TOKEN_ID_PREFIX}.${uuid()}`
  const rawToken = createToken()
  const tokenHash = await hash(rawToken)
  const tokenPrefix = rawToken.slice(0, 20)

  // Ensure the stable service-agent user exists so the inference-gateway auth
  // gate (`kind='agent' AND status='active'`) resolves this credential's
  // `user_id`. Idempotent — a concurrent mint just no-ops on conflict.
  await sql`
    INSERT INTO users
      (id, kind, display_name, status, created_at, updated_at)
    VALUES
      (${CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_USER_ID}, 'agent',
       ${CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_DISPLAY_NAME}, 'active',
       ${createdAt}, ${createdAt})
    ON CONFLICT (id) DO NOTHING
  `

  await sql`
    INSERT INTO agent_credentials
      (id, user_id, openauth_user_id, token_hash, token_prefix, name,
       status, created_at, expires_at)
    VALUES
      (${credentialId}, ${CLOUD_RUNTIME_EXECUTION_TOKEN_AGENT_USER_ID},
       ${input.ownerUserId}, ${tokenHash}, ${tokenPrefix},
       ${CLOUD_RUNTIME_EXECUTION_TOKEN_NAME}, 'active', ${createdAt}, ${expiresAt})
  `

  return {
    createdAt,
    credentialId,
    expiresAt,
    ownerUserId: input.ownerUserId,
    rawToken,
    tokenPrefix,
  }
}

export type RevokeExecutionTokenInput = Readonly<{
  credentialId: string
  now?: (() => string) | undefined
}>

/**
 * Revoke a minted execution token by id: flip BOTH `status` to `'revoked'` and
 * set `revoked_at`, so the auth gate (which checks both) fails the token closed
 * immediately. Idempotent — revoking an already-revoked/absent id is a no-op.
 * Returns the number of rows updated (0 or 1).
 */
export const revokeCloudRuntimeExecutionToken = async (
  sql: SyncSql,
  input: RevokeExecutionTokenInput,
): Promise<number> => {
  const now = input.now ?? currentIsoTimestamp
  const revokedAt = now()
  const rows = (await sql`
    UPDATE agent_credentials
    SET status = 'revoked', revoked_at = ${revokedAt}
    WHERE id = ${input.credentialId}
      AND status = 'active'
    RETURNING id
  `) as ReadonlyArray<{ id: string }>
  return rows.length
}
