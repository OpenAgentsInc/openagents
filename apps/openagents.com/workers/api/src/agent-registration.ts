import { Schema as S } from 'effect'

import {
  type AgentRuntimeRemainderMirror,
  type AgentRuntimeRemainderStoreEnv,
  makeAgentRuntimeRemainderMirrorForEnv,
} from './agent-runtime-remainder-store'
import {
  type IdentityAuthMirror,
  identityAuthMirrorFromEnv,
} from './identity-auth-domain-store'
import { type IdentityDb, identityDbForEnv } from './identity-db'
import { logWorkerRouteWarning } from './observability'
import type { LedgerParam } from './payments-ledger-db'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

export const AGENT_TOKEN_PREFIX = 'oa_agent_'
const SIMPLE_EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())

// Single source of truth for an agent user displayName constraint. Registration
// and the self-serve rename endpoint (#5333) must validate identically so a
// rename can never persist a name registration would have rejected.
export const AgentDisplayName = NonEmptyTrimmedString.check(S.isMaxLength(120))

export const ProgrammaticAgentRegistrationRequest = S.Struct({
  displayName: AgentDisplayName,
  slug: S.optionalKey(
    TrimmedString.check(
      S.isMinLength(3),
      S.isMaxLength(80),
      S.isPattern(/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/),
    ),
  ),
  externalId: S.optionalKey(NonEmptyTrimmedString.check(S.isMaxLength(200))),
  primaryEmail: S.optionalKey(
    NonEmptyTrimmedString.check(S.isPattern(SIMPLE_EMAIL_PATTERN)),
  ),
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  bolt12Offer: S.optionalKey(
    TrimmedString.check(
      S.isMaxLength(4096),
      S.isPattern(/^lno1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{16,4092}$/i),
    ),
  ),
  sparkAddress: S.optionalKey(
    TrimmedString.check(
      S.isMaxLength(600),
      S.isPattern(
        /^(?:spark|sparkt|sparkrt|sparks|sp|spt|sprt|sps)1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{16,512}$/i,
      ),
    ),
  ),
  // Optional static Lightning Address (LNURL-pay), e.g. one hosted by a Spark
  // wallet's LSP. Mirrors bolt12Offer as a public payment destination.
  lightningAddress: S.optionalKey(
    TrimmedString.check(
      S.isMaxLength(512),
      S.isPattern(
        /^[a-z0-9][a-z0-9._%+-]{0,127}@[a-z0-9](?:[a-z0-9-]{0,62}\.)+[a-z]{2,24}$/i,
      ),
    ),
  ),
})

export type ProgrammaticAgentRegistrationRequest =
  typeof ProgrammaticAgentRegistrationRequest.Type

// #6370: admin-only token re-issue for an EXISTING forum agent identity. The
// caller supplies exactly one selector (slug or externalId); the endpoint mints
// a fresh credential bound to the SAME agent entity so a dead agent token can be
// recovered without creating a new agent or changing its slug/displayName. The
// "exactly one selector" rule is enforced by the route handler so it can return
// a clear bad-request message.
export const ReissueAgentTokenRequest = S.Struct({
  slug: S.optionalKey(
    TrimmedString.check(
      S.isMinLength(3),
      S.isMaxLength(80),
      S.isPattern(/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/),
    ),
  ),
  externalId: S.optionalKey(NonEmptyTrimmedString.check(S.isMaxLength(200))),
})

export type ReissueAgentTokenRequest = typeof ReissueAgentTokenRequest.Type

// The single existing-agent selector the reissue store lookup accepts. Exactly
// one of slug/externalId is provided.
export type AgentReissueSelector =
  | Readonly<{ slug: string }>
  | Readonly<{ externalId: string }>

// The existing agent entity a reissue binds the fresh credential to. `slug` is
// the public Forum/profile slug (may be null for slug-less agents looked up by
// externalId); `userId` is the stable agent user entity.
export type AgentReissueTarget = Readonly<{
  userId: string
  slug: string | null
  displayName: string
}>

export type AgentTokenReissue = Readonly<{
  token: string
  tokenPrefix: string
  slug: string | null
  actorRef: string
  userId: string
  credentialId: string
}>

// The narrowed store capability `reissueProgrammaticAgentToken` needs: both
// methods are REQUIRED here so the reissue path is statically guaranteed the
// lookup + additive-insert capability without runtime guards. The full
// `AgentRegistrationStore` keeps these optional so the many other store
// implementers do not have to provide them; the D1 store satisfies both.
export type AgentReissueStore = Readonly<{
  findAgentForReissue: (
    selector: AgentReissueSelector,
  ) => Promise<AgentReissueTarget | undefined>
  addAgentCredential: (record: AgentCredentialRecord) => Promise<void>
}>

export type AgentForumIdentityTarget = Readonly<{
  session: ProgrammaticAgentSession
  slug: string | null
}>

export type AgentForumIdentityStore = Readonly<{
  findAgentForumIdentity: (
    selector: AgentReissueSelector,
    now: string,
  ) => Promise<AgentForumIdentityTarget | undefined>
}>

export type AgentUserRecord = Readonly<{
  id: string
  kind: 'agent'
  displayName: string
  primaryEmail: string | null
  avatarUrl: string | null
  status: 'active'
  createdAt: string
  updatedAt: string
}>

export type AgentIdentityRecord = Readonly<{
  id: string
  userId: string
  provider: 'agent_programmatic'
  providerSubject: string
  email: string | null
  createdAt: string
  updatedAt: string
}>

export type AgentProfileRecord = Readonly<{
  userId: string
  slug: string | null
  metadataJson: string
  createdAt: string
  updatedAt: string
}>

export type AgentCredentialRecord = Readonly<{
  id: string
  userId: string
  openauthUserId: string | null
  tokenHash: string
  tokenPrefix: string
  name: string
  status: 'active'
  createdAt: string
  expiresAt: string | null
}>

export type AgentRegistrationRecord = Readonly<{
  user: AgentUserRecord
  identity: AgentIdentityRecord
  profile: AgentProfileRecord
  credential: AgentCredentialRecord
}>

export type AgentCredentialLookup = Readonly<{
  user: AgentUserRecord
  credentialId: string
  openauthUserId?: string | null
  profileMetadataJson: string
  tokenPrefix: string
}>

export type OpenAuthAgentLinkKind =
  | 'claim_approval'
  | 'credential_anchor'
  | 'manual'

export type OpenAuthAgentLinkRecord = Readonly<{
  id: string
  openauthUserId: string
  agentUserId: string
  agentCredentialId: string | null
  linkKind: OpenAuthAgentLinkKind
  status: 'active' | 'revoked'
  createdAt: string
  updatedAt: string
  revokedAt: string | null
}>

export type LinkedAgentOwnerRecord = Readonly<{
  agentUserId: string
  credentialId: string | null
  displayName: string
  linkKind: OpenAuthAgentLinkKind
  openauthUserId: string
  tokenPrefix: string | null
}>

export type AgentRegistrationStore = Readonly<{
  createAgentRegistration: (record: AgentRegistrationRecord) => Promise<void>
  findAgentByTokenHash: (
    tokenHash: string,
    now: string,
  ) => Promise<AgentCredentialLookup | undefined>
  touchAgentCredential: (
    credentialId: string,
    lastUsedAt: string,
  ) => Promise<void>
  // #5333: self-serve agent displayName rename. Updates the agent user row that
  // `session.user.displayName` reads from, which Pylon registration/heartbeat
  // projections and Forum actor context derive from. Self-only: the caller
  // passes its own authenticated userId. Returns the number of rows updated so
  // callers can distinguish a real self-update from a no-op/missing row.
  updateAgentDisplayName: (
    userId: string,
    displayName: string,
    updatedAt: string,
  ) => Promise<number>
  linkOpenAuthAgent?: (record: OpenAuthAgentLinkRecord) => Promise<void>
  listLinkedAgentsForOpenAuthUser?: (
    openauthUserId: string,
    limit: number,
  ) => Promise<ReadonlyArray<LinkedAgentOwnerRecord>>
  // #6370: admin token re-issue. Find an existing active agent entity by its
  // public slug or its external id (auth_identities.provider_subject) so a
  // fresh credential can be bound to the SAME user. Returns undefined when no
  // matching active agent exists.
  findAgentForReissue?: (
    selector: AgentReissueSelector,
  ) => Promise<AgentReissueTarget | undefined>
  // #6370: additive credential insert for an existing agent user. Prior
  // credentials are left intact (a dead token simply stops being used); this
  // adds a new active credential so the recovered token authenticates as the
  // same entity.
  addAgentCredential?: (record: AgentCredentialRecord) => Promise<void>
  // Registered Forum identity lookup. Internal Forum writers use this to speak
  // as the canonical registered agent actor, never a synthetic duplicate actor.
  findAgentForumIdentity?: (
    selector: AgentReissueSelector,
    now: string,
  ) => Promise<AgentForumIdentityTarget | undefined>
}>

export type ProgrammaticAgentRegistration = Readonly<{
  user: AgentUserRecord
  identity: Readonly<{
    id: string
    provider: 'agent_programmatic'
    providerSubject: string
  }>
  credential: Readonly<{
    id: string
    token: string
    tokenPrefix: string
    createdAt: string
    expiresAt: string | null
  }>
}>

export type ProgrammaticAgentSession = Readonly<{
  user: AgentUserRecord
  credential: Readonly<{
    id: string
    openauthUserId?: string | null
    profileMetadataJson: string
    tokenPrefix: string
    lastUsedAt: string
  }>
}>

type AgentCredentialRow = Readonly<{
  user_id: string
  credential_id: string
  openauth_user_id: string | null
  metadata_json: string | null
  token_prefix: string
}>

type LinkedAgentOwnerRow = Readonly<{
  agent_user_id: string
  credential_id: string | null
  link_kind: OpenAuthAgentLinkKind
  openauth_user_id: string
  token_prefix: string | null
}>

// ---------------------------------------------------------------------------
// CFG-4 Domain 2 (#8519): Postgres-authoritative `users` reads
// ---------------------------------------------------------------------------

/** The `users` predicate every agent auth/identity path shares:
 * kind='agent', status='active', not deleted. */
const ACTIVE_AGENT_USERS_SQL = `SELECT id, display_name, primary_email, avatar_url, created_at, updated_at
   FROM users
  WHERE kind = 'agent'
    AND status = 'active'
    AND deleted_at IS NULL
    AND id IN`

const toActiveAgentUser = (
  row: Readonly<Record<string, unknown>>,
): AgentUserRecord => ({
  id: String(row.id),
  kind: 'agent',
  displayName: String(row.display_name),
  primaryEmail: row.primary_email === null ? null : String(row.primary_email),
  avatarUrl: row.avatar_url === null ? null : String(row.avatar_url),
  status: 'active',
  createdAt: String(row.created_at),
  updatedAt: String(row.updated_at),
})

const readActiveAgentUsers = async (
  identityDb: IdentityDb,
  userIds: ReadonlyArray<string>,
): Promise<ReadonlyMap<string, AgentUserRecord>> => {
  if (userIds.length === 0) return new Map()
  const placeholders = userIds.map(() => '?').join(', ')
  const rows = await identityDb.query(
    `${ACTIVE_AGENT_USERS_SQL} (${placeholders})`,
    [...userIds],
  )
  return new Map(rows.map(row => [String(row.id), toActiveAgentUser(row)]))
}

const readActiveAgentUser = async (
  identityDb: IdentityDb,
  userId: string,
): Promise<AgentUserRecord | undefined> =>
  (await readActiveAgentUsers(identityDb, [userId])).get(userId)

// CFG D1 evacuation (#8515): the Postgres-mirror twin of the agent auth-gate
// credential/profile SELECT. `agent_credentials`/`agent_profiles` are KS-8.5
// (#8334) read-back mirrors in the SAME khala_sync Postgres database the
// identity core (`users`/`auth_identities`, CFG-4 #8519) is authoritative in,
// so `identityDb` (a generic KHALA_SYNC_DB executor) can serve them directly.
// Used only as the fail-soft fallback when the D1 `d1-http` bridge 401s
// account-wide, so the OpenAI-compatible inference gateway (and every other
// authenticated agent request) does not 500 on a dead D1. All mirror columns
// are `text`, so the ISO-timestamp `expires_at > ?` comparison is exact.
const AGENT_CREDENTIAL_BY_TOKEN_HASH_PG_SQL = `SELECT
    agent_credentials.user_id,
    agent_credentials.id AS credential_id,
    agent_credentials.openauth_user_id,
    agent_profiles.metadata_json,
    agent_credentials.token_prefix
 FROM agent_credentials
 LEFT JOIN agent_profiles ON agent_profiles.user_id = agent_credentials.user_id
 WHERE agent_credentials.token_hash = ?
   AND agent_credentials.status = 'active'
   AND agent_credentials.revoked_at IS NULL
   AND (
     agent_credentials.expires_at IS NULL
     OR agent_credentials.expires_at > ?
   )
 LIMIT 1`

const readAgentCredentialByTokenHashFromPostgres = async (
  identityDb: IdentityDb,
  tokenHash: string,
  now: string,
): Promise<AgentCredentialRow | null> => {
  const rows = await identityDb.query(AGENT_CREDENTIAL_BY_TOKEN_HASH_PG_SQL, [
    tokenHash,
    now,
  ])
  const row = rows[0]
  if (row === undefined) {
    return null
  }
  return {
    user_id: String(row.user_id),
    credential_id: String(row.credential_id),
    openauth_user_id:
      row.openauth_user_id === null || row.openauth_user_id === undefined
        ? null
        : String(row.openauth_user_id),
    metadata_json:
      row.metadata_json === null || row.metadata_json === undefined
        ? null
        : String(row.metadata_json),
    token_prefix: String(row.token_prefix),
  }
}

const textEncoder = new TextEncoder()

const bytesToBase64Url = (bytes: Uint8Array): string =>
  btoa(Array.from(bytes, byte => String.fromCharCode(byte)).join(''))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')

const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')

const randomBase64Url = (byteLength: number): string => {
  const bytes = new Uint8Array(byteLength)
  crypto.getRandomValues(bytes)

  return bytesToBase64Url(bytes)
}

export const createAgentToken = (): string =>
  `${AGENT_TOKEN_PREFIX}${randomBase64Url(32)}`

export const sha256Hex = async (value: string): Promise<string> =>
  bytesToHex(
    new Uint8Array(
      await crypto.subtle.digest('SHA-256', textEncoder.encode(value)),
    ),
  )

export const timingSafeEqual = async (
  actual: string,
  expected: string,
): Promise<boolean> => {
  const [actualHash, expectedHash] = await Promise.all([
    crypto.subtle.digest('SHA-256', textEncoder.encode(actual)),
    crypto.subtle.digest('SHA-256', textEncoder.encode(expected)),
  ])
  const expectedBytes = new Uint8Array(expectedHash)
  const mismatch = Array.from(new Uint8Array(actualHash)).reduce(
    (accumulator, byte, index) =>
      accumulator | (byte ^ (expectedBytes[index] ?? 0)),
    0,
  )

  return mismatch === 0
}

// CFG D1 evacuation (#8515): the Cloudflare D1 `d1-http` bridge 401s
// account-wide (Workers Paid plan cancelled — no token fix). The agent
// bearer-token auth GATE reads `agent_credentials`/`agent_profiles` on EVERY
// authenticated request; against dead D1 that was the dominant residual
// d1-http 401 source (~30 401s/35s) AND it broke `POST /api/agents/register`
// (the credential/profile writes went to dead D1). `agent_credentials` and
// `agent_profiles` are KS-8.5 (#8334) read-back twins in the SAME khala_sync
// Postgres database the identity core (`users`/`auth_identities`, CFG-4 #8519)
// is already authoritative in, so `identityDb` (a generic KHALA_SYNC_DB
// executor) can serve BOTH tables' reads AND writes directly — no cross-store
// JOIN boundary remains between the four tables.
//
// `KHALA_SYNC_AGENT_CREDENTIALS_WRITES` is the cutover lever (mirrors the
// `KHALA_SYNC_PYLON_WRITES` / `KHALA_SYNC_FORGE_GIT_CANONICAL_WRITES`
// posture). Default 'postgres': the agent credential/profile READS and WRITES
// are Postgres-authoritative and never touch the dead D1 bridge. Any value
// other than an explicit 'd1' resolves to 'postgres' — the inverse-typo
// posture the other WRITE cutovers use, so a typo can never route the auth
// gate back onto the 401-dead bridge. 'd1' restores the legacy path (reads
// D1-first with a Postgres fail-soft fallback; writes to D1) as an emergency
// escape hatch only.
export type AgentCredentialsWritesMode = 'd1' | 'postgres'

export type AgentCredentialsFlagEnv = Readonly<{
  KHALA_SYNC_AGENT_CREDENTIALS_WRITES?: string | undefined
}>

export const agentCredentialsWritesModeFromEnv = (
  env: AgentCredentialsFlagEnv,
): AgentCredentialsWritesMode =>
  env.KHALA_SYNC_AGENT_CREDENTIALS_WRITES?.trim().toLowerCase() === 'd1'
    ? 'd1'
    : 'postgres'

// The Postgres-authoritative twin of the D1 credential/profile INSERTs.
// `agent_credentials`/`agent_profiles` live in the same khala_sync database as
// `users`/`auth_identities`, so the whole registration lands in ONE atomic
// Postgres transaction — the UNIQUE (provider, provider_subject) dedupe and
// the slug UNIQUE both enforce together, and there is no D1 batch that can
// half-fail behind a committed identity row.
const insertAgentRegistrationOnPostgres = (
  identityDb: IdentityDb,
  record: AgentRegistrationRecord,
  credentialOpenauthUserId: string | null,
): Promise<void> =>
  identityDb.batch([
    {
      sql: `INSERT INTO users
          (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      params: [
        record.user.id,
        record.user.kind,
        record.user.displayName,
        record.user.primaryEmail,
        record.user.avatarUrl,
        record.user.status,
        record.user.createdAt,
        record.user.updatedAt,
      ],
    },
    {
      sql: `INSERT INTO auth_identities
          (id, user_id, provider, provider_subject, email, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)`,
      params: [
        record.identity.id,
        record.identity.userId,
        record.identity.provider,
        record.identity.providerSubject,
        record.identity.email,
        record.identity.createdAt,
        record.identity.updatedAt,
      ],
    },
    {
      sql: `INSERT INTO agent_profiles
          (user_id, slug, metadata_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)`,
      params: [
        record.profile.userId,
        record.profile.slug,
        record.profile.metadataJson,
        record.profile.createdAt,
        record.profile.updatedAt,
      ],
    },
    {
      sql: `INSERT INTO agent_credentials
          (id, user_id, openauth_user_id, token_hash, token_prefix, name, status, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      params: [
        record.credential.id,
        record.credential.userId,
        credentialOpenauthUserId,
        record.credential.tokenHash,
        record.credential.tokenPrefix,
        record.credential.name,
        record.credential.status,
        record.credential.createdAt,
        record.credential.expiresAt,
      ],
    },
  ])

export const makeD1AgentRegistrationStore = (
  db: D1Database,
  identityDb: IdentityDb,
  mode: AgentCredentialsWritesMode = 'postgres',
): AgentRegistrationStore & AgentReissueStore & AgentForumIdentityStore => {
  // Default 'postgres': agent credential/profile reads + writes are
  // Postgres-authoritative (D1 is 401-dead). 'd1' is the emergency escape
  // hatch that restores the legacy D1 path.
  const credentialsOnPostgres = mode !== 'd1'
  return {
    createAgentRegistration: async record => {
      if (credentialsOnPostgres) {
        // Postgres-authoritative: one atomic transaction across all four tables
        // (identity core + credential/profile), never the dead D1 bridge.
        await insertAgentRegistrationOnPostgres(
          identityDb,
          record,
          record.credential.openauthUserId,
        )
        return
      }
      // CFG-4 Domain 2 (#8519) cross-store seam: `users`/`auth_identities`
      // are Postgres-authoritative; `agent_profiles`/`agent_credentials`
      // stay D1. Identity first — the Postgres UNIQUE
      // (provider, provider_subject) refuses a duplicate externalId BEFORE
      // any D1 row lands (same dedupe D1 enforced when this was one batch).
      // If the D1 batch then fails (e.g. slug UNIQUE), the orphaned agent
      // user has no credential/profile, can never authenticate, and the
      // registration surfaces the error to the caller — no broken slug is
      // ever claimed.
      await identityDb.batch([
        {
          sql: `INSERT INTO users
            (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
          params: [
            record.user.id,
            record.user.kind,
            record.user.displayName,
            record.user.primaryEmail,
            record.user.avatarUrl,
            record.user.status,
            record.user.createdAt,
            record.user.updatedAt,
          ],
        },
        {
          sql: `INSERT INTO auth_identities
            (id, user_id, provider, provider_subject, email, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)`,
          params: [
            record.identity.id,
            record.identity.userId,
            record.identity.provider,
            record.identity.providerSubject,
            record.identity.email,
            record.identity.createdAt,
            record.identity.updatedAt,
          ],
        },
      ])
      await db.batch([
        db
          .prepare(
            `INSERT INTO agent_profiles
            (user_id, slug, metadata_json, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?)`,
          )
          .bind(
            record.profile.userId,
            record.profile.slug,
            record.profile.metadataJson,
            record.profile.createdAt,
            record.profile.updatedAt,
          ),
        db
          .prepare(
            `INSERT INTO agent_credentials
            (id, user_id, openauth_user_id, token_hash, token_prefix, name, status, created_at, expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            record.credential.id,
            record.credential.userId,
            record.credential.openauthUserId,
            record.credential.tokenHash,
            record.credential.tokenPrefix,
            record.credential.name,
            record.credential.status,
            record.credential.createdAt,
            record.credential.expiresAt,
          ),
      ])
    },

    findAgentByTokenHash: async (tokenHash, now) => {
      // CFG-4 Domain 2 (#8519): THE agent auth-gate read — runs on EVERY
      // authenticated agent request.
      //
      // CFG D1 evacuation (#8515): the D1 `d1-http` bridge 401s account-wide, so
      // this read is Postgres-PRIMARY by default (`credentialsOnPostgres`). The
      // credential/profile SELECT serves from the khala_sync Postgres twin — the
      // same database `users`/`auth_identities` are authoritative in — so a
      // normal authed request never touches the 401-dead bridge (this removed
      // the dominant ~30 401s/35s d1-http source). A Postgres error fails CLOSED
      // (undefined -> 401 — never grant, never 500). The legacy 'd1' escape
      // hatch keeps the old D1-first read with a Postgres fail-soft fallback.
      let row: AgentCredentialRow | null
      if (credentialsOnPostgres) {
        try {
          row = await readAgentCredentialByTokenHashFromPostgres(
            identityDb,
            tokenHash,
            now,
          )
        } catch {
          return undefined
        }
      } else {
        try {
          row = await db
            .prepare(
              `SELECT
            agent_credentials.user_id,
            agent_credentials.id AS credential_id,
            agent_credentials.openauth_user_id,
            agent_profiles.metadata_json,
            agent_credentials.token_prefix
         FROM agent_credentials
         LEFT JOIN agent_profiles ON agent_profiles.user_id = agent_credentials.user_id
         WHERE agent_credentials.token_hash = ?
           AND agent_credentials.status = 'active'
           AND agent_credentials.revoked_at IS NULL
           AND (
             agent_credentials.expires_at IS NULL
             OR agent_credentials.expires_at > ?
           )`,
            )
            .bind(tokenHash, now)
            .first<AgentCredentialRow>()
        } catch (error) {
          logWorkerRouteWarning(
            'khala_sync_agent_auth_postgres_read_fallback',
            {
              messageSafe: error instanceof Error ? error.name : 'error',
            },
          )
          try {
            row = await readAgentCredentialByTokenHashFromPostgres(
              identityDb,
              tokenHash,
              now,
            )
          } catch {
            return undefined
          }
        }
      }

      if (row === null) {
        return undefined
      }

      const user = await readActiveAgentUser(identityDb, row.user_id)
      if (user === undefined) {
        return undefined
      }

      return {
        user,
        credentialId: row.credential_id,
        openauthUserId: row.openauth_user_id,
        profileMetadataJson: row.metadata_json ?? '{}',
        tokenPrefix: row.token_prefix,
      }
    },

    touchAgentCredential: async (credentialId, lastUsedAt) => {
      // CFG D1 evacuation (#8515): `last_used_at` is non-authoritative
      // bookkeeping. Writes go to the Postgres twin by default so the touch
      // actually lands (and never hits the 401-dead bridge); a store error must
      // never fail an otherwise-valid authentication, so this is best-effort.
      try {
        if (credentialsOnPostgres) {
          await identityDb.query(
            `UPDATE agent_credentials
         SET last_used_at = ?
         WHERE id = ?`,
            [lastUsedAt, credentialId],
          )
        } else {
          await db
            .prepare(
              `UPDATE agent_credentials
         SET last_used_at = ?
         WHERE id = ?`,
            )
            .bind(lastUsedAt, credentialId)
            .run()
        }
      } catch {
        // Swallow: touch bookkeeping is never allowed to 500 an auth request.
      }
    },

    updateAgentDisplayName: async (userId, displayName, updatedAt) => {
      // CFG-4 Domain 2 (#8519): Postgres-authoritative `users` write.
      // RETURNING gives the touched-row count on both engines.
      const rows = await identityDb.query(
        `UPDATE users
         SET display_name = ?, updated_at = ?
       WHERE id = ?
         AND kind = 'agent'
         AND status = 'active'
         AND deleted_at IS NULL
       RETURNING id`,
        [displayName, updatedAt, userId],
      )

      return rows.length
    },

    linkOpenAuthAgent: async record => {
      // RESIDUAL D1 (out of scope for the #8515 agent-credentials cutover):
      // `openauth_agent_links` is a DIFFERENT, still-D1-authoritative domain
      // (identity-auth-domain-store), and this write is an atomic D1 batch with
      // the `agent_credentials.openauth_user_id` linkage UPDATE. It cannot move
      // to Postgres without cutting the openauth_agent_links domain too, so it
      // stays on D1 here; the owner-link/claim lane is blocked independently.
      await db.batch([
        db
          .prepare(
            `INSERT INTO openauth_agent_links
            (id, openauth_user_id, agent_user_id, agent_credential_id,
             link_kind, status, created_at, updated_at, revoked_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(openauth_user_id, agent_user_id, agent_credential_id)
           DO UPDATE SET
             link_kind = excluded.link_kind,
             status = excluded.status,
             updated_at = excluded.updated_at,
             revoked_at = excluded.revoked_at`,
          )
          .bind(
            record.id,
            record.openauthUserId,
            record.agentUserId,
            record.agentCredentialId,
            record.linkKind,
            record.status,
            record.createdAt,
            record.updatedAt,
            record.revokedAt,
          ),
        ...(record.agentCredentialId === null
          ? []
          : [
              db
                .prepare(
                  `UPDATE agent_credentials
                    SET openauth_user_id = ?
                  WHERE id = ?
                    AND user_id = ?
                    AND status = 'active'
                    AND revoked_at IS NULL`,
                )
                .bind(
                  record.openauthUserId,
                  record.agentCredentialId,
                  record.agentUserId,
                ),
            ]),
      ])
    },

    listLinkedAgentsForOpenAuthUser: async (openauthUserId, limit) => {
      const boundedLimit = Math.max(1, Math.min(Math.trunc(limit), 100))
      // RESIDUAL D1 (out of scope for the #8515 agent-credentials cutover):
      // this query is anchored on `openauth_agent_links` (a still-D1-authoritative
      // domain) JOINed with `agent_credentials`, so it cannot serve purely from
      // the Postgres agent-credentials twin without also routing the
      // openauth_agent_links domain. It stays on D1; the owner "linked agents"
      // dashboard is blocked by the openauth_agent_links domain independently.
      // CFG-4 Domain 2 (#8519): links/credentials from D1, the active-agent
      // gate + display fields + `users.updated_at` ordering from Postgres.
      const result = await db
        .prepare(
          `WITH explicit_links AS (
           SELECT
             openauth_agent_links.openauth_user_id,
             openauth_agent_links.agent_user_id,
             openauth_agent_links.agent_credential_id AS credential_id,
             openauth_agent_links.link_kind,
             agent_credentials.token_prefix
           FROM openauth_agent_links
           LEFT JOIN agent_credentials
             ON agent_credentials.id = openauth_agent_links.agent_credential_id
            AND agent_credentials.status = 'active'
            AND agent_credentials.revoked_at IS NULL
           WHERE openauth_agent_links.openauth_user_id = ?
             AND openauth_agent_links.status = 'active'
             AND openauth_agent_links.revoked_at IS NULL
         ),
         credential_links AS (
           SELECT
             agent_credentials.openauth_user_id,
             agent_credentials.user_id AS agent_user_id,
             agent_credentials.id AS credential_id,
             'credential_anchor' AS link_kind,
             agent_credentials.token_prefix
           FROM agent_credentials
           WHERE agent_credentials.openauth_user_id = ?
             AND agent_credentials.status = 'active'
             AND agent_credentials.revoked_at IS NULL
         )
         SELECT * FROM explicit_links
         UNION
         SELECT * FROM credential_links`,
        )
        .bind(openauthUserId, openauthUserId)
        .all<LinkedAgentOwnerRow>()

      const links = result.results ?? []
      const users = await readActiveAgentUsers(
        identityDb,
        links.map(row => row.agent_user_id),
      )

      return links
        .flatMap(row => {
          const user = users.get(row.agent_user_id)
          return user === undefined
            ? []
            : [
                {
                  agentUserId: row.agent_user_id,
                  credentialId: row.credential_id,
                  displayName: user.displayName,
                  linkKind: row.link_kind,
                  openauthUserId: row.openauth_user_id,
                  tokenPrefix: row.token_prefix,
                  updatedAt: user.updatedAt,
                },
              ]
        })
        .sort(
          (a, b) =>
            b.updatedAt.localeCompare(a.updatedAt) ||
            a.agentUserId.localeCompare(b.agentUserId),
        )
        .slice(0, boundedLimit)
        .map(({ updatedAt: _updatedAt, ...record }) => record)
    },

    findAgentForReissue: async selector => {
      // CFG-4 Domain 2 (#8519) + CFG D1 evacuation (#8515): `agent_profiles`
      // reads serve from the Postgres twin by default (D1 is 401-dead). slug
      // resolves via `agent_profiles` then the Postgres active-agent gate;
      // externalId resolves via the Postgres `auth_identities` × `users` join
      // then `agent_profiles` for the slug.
      if ('slug' in selector) {
        const profile = credentialsOnPostgres
          ? (((
              await identityDb.query(
                `SELECT user_id, slug
             FROM agent_profiles
            WHERE slug = ?
            LIMIT 1`,
                [selector.slug],
              )
            )[0] as
              | Readonly<{ user_id: string; slug: string | null }>
              | undefined) ?? null)
          : await db
              .prepare(
                `SELECT user_id, slug
             FROM agent_profiles
            WHERE slug = ?
            LIMIT 1`,
              )
              .bind(selector.slug)
              .first<Readonly<{ user_id: string; slug: string | null }>>()
        if (profile === null) {
          return undefined
        }
        const user = await readActiveAgentUser(identityDb, profile.user_id)
        if (user === undefined) {
          return undefined
        }
        return {
          userId: user.id,
          slug: profile.slug ?? null,
          displayName: user.displayName,
        }
      }

      const rows = await identityDb.query(
        `SELECT users.id AS user_id,
              users.display_name
         FROM auth_identities
         INNER JOIN users ON users.id = auth_identities.user_id
        WHERE auth_identities.provider = 'agent_programmatic'
          AND auth_identities.provider_subject = ?
          AND users.kind = 'agent'
          AND users.status = 'active'
          AND users.deleted_at IS NULL
        LIMIT 1`,
        [selector.externalId],
      )
      const identityRow = rows[0]
      if (identityRow === undefined) {
        return undefined
      }
      const userId = String(identityRow.user_id)
      const profile = credentialsOnPostgres
        ? (((
            await identityDb.query(
              `SELECT slug FROM agent_profiles WHERE user_id = ? LIMIT 1`,
              [userId],
            )
          )[0] as Readonly<{ slug: string | null }> | undefined) ?? null)
        : await db
            .prepare(
              `SELECT slug FROM agent_profiles WHERE user_id = ? LIMIT 1`,
            )
            .bind(userId)
            .first<Readonly<{ slug: string | null }>>()

      return {
        userId,
        slug: profile?.slug ?? null,
        displayName: String(identityRow.display_name),
      }
    },

    addAgentCredential: async record => {
      // #6370 reissue: Postgres-authoritative additive credential insert by
      // default (D1 is 401-dead); the legacy 'd1' path stays as the escape hatch.
      if (credentialsOnPostgres) {
        await identityDb.query(
          `INSERT INTO agent_credentials
          (id, user_id, openauth_user_id, token_hash, token_prefix, name, status, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          [
            record.id,
            record.userId,
            record.openauthUserId,
            record.tokenHash,
            record.tokenPrefix,
            record.name,
            record.status,
            record.createdAt,
            record.expiresAt,
          ],
        )
        return
      }
      await db
        .prepare(
          `INSERT INTO agent_credentials
          (id, user_id, openauth_user_id, token_hash, token_prefix, name, status, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.userId,
          record.openauthUserId,
          record.tokenHash,
          record.tokenPrefix,
          record.name,
          record.status,
          record.createdAt,
          record.expiresAt,
        )
        .run()
    },

    findAgentForumIdentity: async (selector, now) => {
      // CFG-4 Domain 2 (#8519): profile/credential resolution stays D1;
      // the active-agent user row comes from Postgres. Slug variant gates
      // AFTER the D1 lookup; externalId variant resolves the user id from
      // the Postgres `auth_identities` join FIRST.
      type ForumIdentityD1Row = Readonly<{
        user_id: string
        slug: string | null
        metadata_json: string | null
        credential_id: string
        openauth_user_id: string | null
        token_prefix: string
      }>
      const activeCredentialClause = `agent_credentials.status = 'active'
       AND agent_credentials.revoked_at IS NULL
       AND (
         agent_credentials.expires_at IS NULL
         OR agent_credentials.expires_at > ?
       )`

      // CFG D1 evacuation (#8515): `agent_profiles`/`agent_credentials` reads
      // serve from the Postgres twin by default (D1 is 401-dead).
      const readForumIdentityRow = async (
        sql: string,
        params: ReadonlyArray<LedgerParam>,
      ): Promise<ForumIdentityD1Row | null> =>
        credentialsOnPostgres
          ? (((await identityDb.query(sql, params))[0] as
              | ForumIdentityD1Row
              | undefined) ?? null)
          : await db
              .prepare(sql)
              .bind(...params)
              .first<ForumIdentityD1Row>()

      let row: ForumIdentityD1Row | null
      let user: AgentUserRecord | undefined
      if ('slug' in selector) {
        row = await readForumIdentityRow(
          `SELECT
              agent_profiles.user_id,
              agent_profiles.slug,
              agent_profiles.metadata_json,
              agent_credentials.id AS credential_id,
              agent_credentials.openauth_user_id,
              agent_credentials.token_prefix
           FROM agent_profiles
           INNER JOIN agent_credentials ON agent_credentials.user_id = agent_profiles.user_id
           WHERE agent_profiles.slug = ?
             AND ${activeCredentialClause}
           ORDER BY agent_credentials.created_at DESC, agent_credentials.id DESC
           LIMIT 1`,
          [selector.slug, now],
        )
        if (row === null) {
          return undefined
        }
        user = await readActiveAgentUser(identityDb, row.user_id)
      } else {
        const identityRows = await identityDb.query(
          `SELECT users.id,
                users.display_name,
                users.primary_email,
                users.avatar_url,
                users.created_at,
                users.updated_at
           FROM auth_identities
           INNER JOIN users ON users.id = auth_identities.user_id
          WHERE auth_identities.provider = 'agent_programmatic'
            AND auth_identities.provider_subject = ?
            AND users.kind = 'agent'
            AND users.status = 'active'
            AND users.deleted_at IS NULL
          LIMIT 1`,
          [selector.externalId],
        )
        const identityRow = identityRows[0]
        if (identityRow === undefined) {
          return undefined
        }
        user = toActiveAgentUser(identityRow)
        row = await readForumIdentityRow(
          `SELECT
              agent_credentials.user_id,
              agent_profiles.slug,
              agent_profiles.metadata_json,
              agent_credentials.id AS credential_id,
              agent_credentials.openauth_user_id,
              agent_credentials.token_prefix
           FROM agent_credentials
           LEFT JOIN agent_profiles ON agent_profiles.user_id = agent_credentials.user_id
           WHERE agent_credentials.user_id = ?
             AND ${activeCredentialClause}
           ORDER BY agent_credentials.created_at DESC, agent_credentials.id DESC
           LIMIT 1`,
          [user.id, now],
        )
      }

      if (row === null || user === undefined) {
        return undefined
      }

      return {
        session: {
          user,
          credential: {
            id: row.credential_id,
            openauthUserId: row.openauth_user_id,
            profileMetadataJson: row.metadata_json ?? '{}',
            tokenPrefix: row.token_prefix,
            lastUsedAt: now,
          },
        },
        slug: row.slug ?? null,
      }
    },
  }
}

type MirrorableAgentRegistrationStore = AgentRegistrationStore &
  Partial<AgentReissueStore> &
  Partial<AgentForumIdentityStore>

const mirrorProfileAndCredential = (
  mirror: AgentRuntimeRemainderMirror,
  record: AgentRegistrationRecord,
): Promise<ReadonlyArray<void>> =>
  Promise.all([
    mirror.mirrorRowsByPk('agent_profiles', [record.profile.userId]),
    mirror.mirrorRowsByPk('agent_credentials', [record.credential.id]),
  ])

const mirrorCredential = (
  mirror: AgentRuntimeRemainderMirror,
  credentialId: string | null,
): Promise<void> =>
  credentialId === null
    ? Promise.resolve()
    : mirror.mirrorRowsByPk('agent_credentials', [credentialId])

export const makeMirroredAgentRegistrationStore = <
  Store extends MirrorableAgentRegistrationStore,
>(
  d1: Store,
  mirror: AgentRuntimeRemainderMirror | undefined,
): Store => {
  if (mirror === undefined) {
    return d1
  }

  const linkOpenAuthAgent = d1.linkOpenAuthAgent
  const listLinkedAgentsForOpenAuthUser = d1.listLinkedAgentsForOpenAuthUser
  const findAgentForReissue = d1.findAgentForReissue
  const addAgentCredential = d1.addAgentCredential
  const findAgentForumIdentity = d1.findAgentForumIdentity

  return {
    createAgentRegistration: async record => {
      await d1.createAgentRegistration(record)
      await mirrorProfileAndCredential(mirror, record)
    },
    findAgentByTokenHash: (tokenHash, now) =>
      d1.findAgentByTokenHash(tokenHash, now),
    touchAgentCredential: async (credentialId, lastUsedAt) => {
      await d1.touchAgentCredential(credentialId, lastUsedAt)
      await mirrorCredential(mirror, credentialId)
    },
    updateAgentDisplayName: (userId, displayName, updatedAt) =>
      d1.updateAgentDisplayName(userId, displayName, updatedAt),
    ...(linkOpenAuthAgent === undefined
      ? {}
      : {
          linkOpenAuthAgent: async (record: OpenAuthAgentLinkRecord) => {
            await linkOpenAuthAgent.call(d1, record)
            await mirrorCredential(mirror, record.agentCredentialId)
          },
        }),
    ...(listLinkedAgentsForOpenAuthUser === undefined
      ? {}
      : {
          listLinkedAgentsForOpenAuthUser: (
            openauthUserId: string,
            limit: number,
          ) => listLinkedAgentsForOpenAuthUser.call(d1, openauthUserId, limit),
        }),
    ...(findAgentForReissue === undefined
      ? {}
      : {
          findAgentForReissue: (selector: AgentReissueSelector) =>
            findAgentForReissue.call(d1, selector),
        }),
    ...(addAgentCredential === undefined
      ? {}
      : {
          addAgentCredential: async (record: AgentCredentialRecord) => {
            await addAgentCredential.call(d1, record)
            await mirrorCredential(mirror, record.id)
          },
        }),
    ...(findAgentForumIdentity === undefined
      ? {}
      : {
          findAgentForumIdentity: (
            selector: AgentReissueSelector,
            now: string,
          ) => findAgentForumIdentity.call(d1, selector, now),
        }),
  } as Store
}

// KS-8.18 follow-up (#8362) identity/auth domain mirror, narrowed by the
// CFG-4 Domain 2 hard cut (#8519): `users`/`auth_identities` are Postgres-
// AUTHORITATIVE now (written directly by the D1 store's identityDb seam
// above), so their mirror arms are DELETED — only the still-D1-owned
// `openauth_agent_links` row keeps its read-back mirror here. Composed ON
// TOP of (not instead of) the pre-existing `AgentRuntimeRemainderMirror`,
// which only ever covered `agent_profiles`/`agent_credentials` — a
// DIFFERENT domain. Fail-soft: `mirror` methods never throw.
const makeIdentityAuthMirroredAgentRegistrationStore = <
  Store extends MirrorableAgentRegistrationStore,
>(
  d1: Store,
  mirror: IdentityAuthMirror | undefined,
): Store => {
  if (mirror === undefined) {
    return d1
  }

  const linkOpenAuthAgent = d1.linkOpenAuthAgent

  return {
    ...d1,
    ...(linkOpenAuthAgent === undefined
      ? {}
      : {
          linkOpenAuthAgent: async (record: OpenAuthAgentLinkRecord) => {
            await linkOpenAuthAgent.call(d1, record)
            await mirror.mirrorRowsByKey('openauth_agent_links', [[record.id]])
          },
        }),
  } as Store
}

export const makeAgentRegistrationStoreForEnv = (
  env: AgentRuntimeRemainderStoreEnv & AgentCredentialsFlagEnv,
): AgentRegistrationStore & AgentReissueStore & AgentForumIdentityStore => {
  const credentialsMode = agentCredentialsWritesModeFromEnv(env)
  const base = makeD1AgentRegistrationStore(
    openAgentsDatabase(env),
    identityDbForEnv(env),
    credentialsMode,
  )
  // CFG D1 evacuation (#8515): in the default 'postgres' mode the
  // credential/profile WRITES are already Postgres-authoritative, so the
  // `AgentRuntimeRemainderMirror` D1->Postgres read-back is redundant AND its
  // source read hits the 401-dead D1 bridge. Skip it; the still-D1-owned
  // `openauth_agent_links` read-back (identityAuthMirror) is a different domain
  // and stays wired regardless.
  const withRemainderMirror =
    credentialsMode === 'd1'
      ? makeMirroredAgentRegistrationStore(
          base,
          makeAgentRuntimeRemainderMirrorForEnv(env),
        )
      : base
  return makeIdentityAuthMirroredAgentRegistrationStore(
    withRemainderMirror,
    identityAuthMirrorFromEnv(env),
  )
}

export const buildProgrammaticAgentRegistrationRecord = (
  input: ProgrammaticAgentRegistrationRequest,
  credentialInput: Readonly<{
    expiresAt?: string | null | undefined
    tokenHash: string
    tokenPrefix: string
  }>,
  options: Readonly<{
    now?: () => string
    makeUuid?: () => string
  }> = {},
): AgentRegistrationRecord => {
  const now = options.now ?? currentIsoTimestamp
  const makeUuid = options.makeUuid ?? randomUuid
  const createdAt = now()
  const userId = `user_${makeUuid()}`
  const credentialId = `agent_credential_${makeUuid()}`
  const identity: AgentIdentityRecord = {
    id: `auth_identity_${makeUuid()}`,
    userId,
    provider: 'agent_programmatic',
    providerSubject: input.externalId ?? userId,
    email: input.primaryEmail ?? null,
    createdAt,
    updatedAt: createdAt,
  }
  const user: AgentUserRecord = {
    id: userId,
    kind: 'agent',
    displayName: input.displayName,
    primaryEmail: input.primaryEmail ?? null,
    avatarUrl: null,
    status: 'active',
    createdAt,
    updatedAt: createdAt,
  }
  const profile: AgentProfileRecord = {
    userId,
    slug: input.slug ?? null,
    metadataJson: JSON.stringify(input.metadata ?? {}),
    createdAt,
    updatedAt: createdAt,
  }
  const credential: AgentCredentialRecord = {
    id: credentialId,
    userId,
    openauthUserId: null,
    tokenHash: credentialInput.tokenHash,
    tokenPrefix: credentialInput.tokenPrefix,
    name: `${input.displayName} programmatic token`,
    status: 'active',
    createdAt,
    expiresAt: credentialInput.expiresAt ?? null,
  }

  return {
    user,
    identity,
    profile,
    credential,
  }
}

export const createProgrammaticAgentRegistration = async (
  store: AgentRegistrationStore,
  input: ProgrammaticAgentRegistrationRequest,
  options: Readonly<{
    expiresAt?: string | null | undefined
    now?: () => string
    makeUuid?: () => string
    makeToken?: () => string
  }> = {},
): Promise<ProgrammaticAgentRegistration> => {
  const makeToken = options.makeToken ?? createAgentToken
  const token = makeToken()
  const record = buildProgrammaticAgentRegistrationRecord(
    input,
    {
      expiresAt: options.expiresAt ?? null,
      tokenHash: await sha256Hex(token),
      tokenPrefix: token.slice(0, 20),
    },
    options,
  )

  await store.createAgentRegistration(record)

  return {
    user: record.user,
    identity: {
      id: record.identity.id,
      provider: record.identity.provider,
      providerSubject: record.identity.providerSubject,
    },
    credential: {
      id: record.credential.id,
      token,
      tokenPrefix: record.credential.tokenPrefix,
      createdAt: record.credential.createdAt,
      expiresAt: record.credential.expiresAt,
    },
  }
}

// #6370: admin-only recovery. Mint a fresh `oa_agent_` credential for an
// EXISTING agent entity selected by slug or externalId, without creating a new
// agent or mutating the slug/displayName. Returns undefined when no matching
// active agent exists so the route can answer 404. Never logs the raw token.
export const reissueProgrammaticAgentToken = async (
  store: AgentReissueStore,
  selector: AgentReissueSelector,
  options: Readonly<{
    now?: () => string
    makeUuid?: () => string
    makeToken?: () => string
  }> = {},
): Promise<AgentTokenReissue | undefined> => {
  const target = await store.findAgentForReissue(selector)

  if (target === undefined) {
    return undefined
  }

  const now = options.now ?? currentIsoTimestamp
  const makeUuid = options.makeUuid ?? randomUuid
  const makeToken = options.makeToken ?? createAgentToken
  const token = makeToken()
  const createdAt = now()
  const credential: AgentCredentialRecord = {
    id: `agent_credential_${makeUuid()}`,
    userId: target.userId,
    openauthUserId: null,
    tokenHash: await sha256Hex(token),
    tokenPrefix: token.slice(0, 20),
    name: `${target.displayName} programmatic token (reissued ${createdAt})`,
    status: 'active',
    createdAt,
    expiresAt: null,
  }

  await store.addAgentCredential(credential)

  return {
    token,
    tokenPrefix: credential.tokenPrefix,
    slug: target.slug,
    actorRef: `agent:${target.userId}`,
    userId: target.userId,
    credentialId: credential.id,
  }
}

export const authenticateProgrammaticAgent = async (
  store: AgentRegistrationStore,
  token: string,
  now: () => string = currentIsoTimestamp,
): Promise<ProgrammaticAgentSession | undefined> => {
  if (!token.startsWith(AGENT_TOKEN_PREFIX)) {
    return undefined
  }

  const tokenHash = await sha256Hex(token)
  const lastUsedAt = now()
  const lookup = await store.findAgentByTokenHash(tokenHash, lastUsedAt)

  if (lookup === undefined) {
    return undefined
  }

  await store.touchAgentCredential(lookup.credentialId, lastUsedAt)

  return {
    user: lookup.user,
    credential: {
      id: lookup.credentialId,
      ...(lookup.openauthUserId === undefined
        ? {}
        : { openauthUserId: lookup.openauthUserId }),
      profileMetadataJson: lookup.profileMetadataJson,
      tokenPrefix: lookup.tokenPrefix,
      lastUsedAt,
    },
  }
}
