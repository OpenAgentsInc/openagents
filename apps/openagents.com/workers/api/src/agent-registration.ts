import { Schema as S } from 'effect'

import {
  makeAgentRuntimeRemainderMirrorForEnv,
  type AgentRuntimeRemainderMirror,
  type AgentRuntimeRemainderStoreEnv,
} from './agent-runtime-remainder-store'
import {
  identityAuthMirrorFromEnv,
  type IdentityAuthMirror,
} from './identity-auth-domain-store'
import { identityDbForEnv, type IdentityDb } from './identity-db'
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

export const makeD1AgentRegistrationStore = (
  db: D1Database,
  identityDb: IdentityDb,
): AgentRegistrationStore & AgentReissueStore & AgentForumIdentityStore => ({
  createAgentRegistration: async record => {
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
    // CFG-4 Domain 2 (#8519): THE agent auth-gate read. The old single D1
    // JOIN splits — credential/profile from D1, then the Postgres-
    // authoritative `users` gate check. Two reads per authenticated agent
    // request is the accepted hard-cut cost.
    const row = await db
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
    await db
      .prepare(
        `UPDATE agent_credentials
         SET last_used_at = ?
         WHERE id = ?`,
      )
      .bind(lastUsedAt, credentialId)
      .run()
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
    // CFG-4 Domain 2 (#8519): slug resolves via D1 `agent_profiles` then
    // the Postgres active-agent gate; externalId resolves via the
    // Postgres `auth_identities` × `users` join then D1 for the slug.
    if ('slug' in selector) {
      const profile = await db
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
    const profile = await db
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

    let row: ForumIdentityD1Row | null
    let user: AgentUserRecord | undefined
    if ('slug' in selector) {
      row = await db
        .prepare(
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
        )
        .bind(selector.slug, now)
        .first<ForumIdentityD1Row>()
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
      row = await db
        .prepare(
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
        )
        .bind(user.id, now)
        .first<ForumIdentityD1Row>()
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
})

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
          ) =>
            findAgentForumIdentity.call(d1, selector, now),
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
            await mirror.mirrorRowsByKey('openauth_agent_links', [
              [record.id],
            ])
          },
        }),
  } as Store
}

export const makeAgentRegistrationStoreForEnv = (
  env: AgentRuntimeRemainderStoreEnv,
): AgentRegistrationStore & AgentReissueStore & AgentForumIdentityStore =>
  makeIdentityAuthMirroredAgentRegistrationStore(
    makeMirroredAgentRegistrationStore(
      makeD1AgentRegistrationStore(openAgentsDatabase(env), identityDbForEnv(env)),
      makeAgentRuntimeRemainderMirrorForEnv(env),
    ),
    identityAuthMirrorFromEnv(env),
  )

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
