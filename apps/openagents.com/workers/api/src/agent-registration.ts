import { Schema as S } from 'effect'

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

type AgentCredentialLookupRow = Readonly<{
  user_id: string
  display_name: string
  primary_email: string | null
  avatar_url: string | null
  status: 'active'
  created_at: string
  updated_at: string
  credential_id: string
  openauth_user_id: string | null
  metadata_json: string | null
  token_prefix: string
}>

type AgentReissueTargetRow = Readonly<{
  user_id: string
  display_name: string
  slug: string | null
}>

type AgentForumIdentityRow = Readonly<{
  user_id: string
  display_name: string
  primary_email: string | null
  avatar_url: string | null
  status: 'active'
  user_created_at: string
  user_updated_at: string
  slug: string | null
  metadata_json: string | null
  credential_id: string
  openauth_user_id: string | null
  token_prefix: string
}>

type LinkedAgentOwnerRow = Readonly<{
  agent_user_id: string
  credential_id: string | null
  display_name: string
  link_kind: OpenAuthAgentLinkKind
  openauth_user_id: string
  token_prefix: string | null
}>

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
): AgentRegistrationStore & AgentReissueStore & AgentForumIdentityStore => ({
  createAgentRegistration: async record => {
    await db.batch([
      db
        .prepare(
          `INSERT INTO users
            (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.user.id,
          record.user.kind,
          record.user.displayName,
          record.user.primaryEmail,
          record.user.avatarUrl,
          record.user.status,
          record.user.createdAt,
          record.user.updatedAt,
        ),
      db
        .prepare(
          `INSERT INTO auth_identities
            (id, user_id, provider, provider_subject, email, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.identity.id,
          record.identity.userId,
          record.identity.provider,
          record.identity.providerSubject,
          record.identity.email,
          record.identity.createdAt,
          record.identity.updatedAt,
        ),
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
    const row = await db
      .prepare(
        `SELECT
            users.id AS user_id,
            users.display_name,
            users.primary_email,
            users.avatar_url,
            users.status,
            users.created_at,
            users.updated_at,
            agent_credentials.id AS credential_id,
            agent_credentials.openauth_user_id,
            agent_profiles.metadata_json,
            agent_credentials.token_prefix
         FROM agent_credentials
         INNER JOIN users ON users.id = agent_credentials.user_id
         LEFT JOIN agent_profiles ON agent_profiles.user_id = users.id
         WHERE agent_credentials.token_hash = ?
           AND agent_credentials.status = 'active'
           AND users.kind = 'agent'
           AND users.status = 'active'
           AND users.deleted_at IS NULL
           AND agent_credentials.revoked_at IS NULL
           AND (
             agent_credentials.expires_at IS NULL
             OR agent_credentials.expires_at > ?
           )`,
      )
      .bind(tokenHash, now)
      .first<AgentCredentialLookupRow>()

    if (row === null) {
      return undefined
    }

    return {
      user: {
        id: row.user_id,
        kind: 'agent',
        displayName: row.display_name,
        primaryEmail: row.primary_email,
        avatarUrl: row.avatar_url,
        status: row.status,
        createdAt: row.created_at,
        updatedAt: row.updated_at,
      },
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
    const result = await db
      .prepare(
        `UPDATE users
         SET display_name = ?, updated_at = ?
         WHERE id = ?
           AND kind = 'agent'
           AND status = 'active'
           AND deleted_at IS NULL`,
      )
      .bind(displayName, updatedAt, userId)
      .run()

    const changes = result.meta?.changes

    return typeof changes === 'number' ? changes : 0
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
         SELECT
           linked.openauth_user_id,
           linked.agent_user_id,
           linked.credential_id,
           linked.link_kind,
           users.display_name,
           linked.token_prefix
         FROM (
           SELECT * FROM explicit_links
           UNION
           SELECT * FROM credential_links
         ) AS linked
         INNER JOIN users
           ON users.id = linked.agent_user_id
          AND users.kind = 'agent'
          AND users.status = 'active'
          AND users.deleted_at IS NULL
         ORDER BY users.updated_at DESC, linked.agent_user_id ASC
         LIMIT ?`,
      )
      .bind(openauthUserId, openauthUserId, boundedLimit)
      .all<LinkedAgentOwnerRow>()

    return (result.results ?? []).map(row => ({
      agentUserId: row.agent_user_id,
      credentialId: row.credential_id,
      displayName: row.display_name,
      linkKind: row.link_kind,
      openauthUserId: row.openauth_user_id,
      tokenPrefix: row.token_prefix,
    }))
  },

  findAgentForReissue: async selector => {
    const row =
      'slug' in selector
        ? await db
            .prepare(
              `SELECT
                  users.id AS user_id,
                  users.display_name,
                  agent_profiles.slug
               FROM agent_profiles
               INNER JOIN users ON users.id = agent_profiles.user_id
               WHERE agent_profiles.slug = ?
                 AND users.kind = 'agent'
                 AND users.status = 'active'
                 AND users.deleted_at IS NULL
               LIMIT 1`,
            )
            .bind(selector.slug)
            .first<AgentReissueTargetRow>()
        : await db
            .prepare(
              `SELECT
                  users.id AS user_id,
                  users.display_name,
                  agent_profiles.slug
               FROM auth_identities
               INNER JOIN users ON users.id = auth_identities.user_id
               LEFT JOIN agent_profiles ON agent_profiles.user_id = users.id
               WHERE auth_identities.provider = 'agent_programmatic'
                 AND auth_identities.provider_subject = ?
                 AND users.kind = 'agent'
                 AND users.status = 'active'
                 AND users.deleted_at IS NULL
               LIMIT 1`,
            )
            .bind(selector.externalId)
            .first<AgentReissueTargetRow>()

    if (row === null) {
      return undefined
    }

    return {
      userId: row.user_id,
      slug: row.slug ?? null,
      displayName: row.display_name,
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
    const activeCredentialClause = `agent_credentials.status = 'active'
       AND agent_credentials.revoked_at IS NULL
       AND (
         agent_credentials.expires_at IS NULL
         OR agent_credentials.expires_at > ?
       )`
    const row =
      'slug' in selector
        ? await db
            .prepare(
              `SELECT
                  users.id AS user_id,
                  users.display_name,
                  users.primary_email,
                  users.avatar_url,
                  users.status,
                  users.created_at AS user_created_at,
                  users.updated_at AS user_updated_at,
                  agent_profiles.slug,
                  agent_profiles.metadata_json,
                  agent_credentials.id AS credential_id,
                  agent_credentials.openauth_user_id,
                  agent_credentials.token_prefix
               FROM agent_profiles
               INNER JOIN users ON users.id = agent_profiles.user_id
               INNER JOIN agent_credentials ON agent_credentials.user_id = users.id
               WHERE agent_profiles.slug = ?
                 AND users.kind = 'agent'
                 AND users.status = 'active'
                 AND users.deleted_at IS NULL
                 AND ${activeCredentialClause}
               ORDER BY agent_credentials.created_at DESC, agent_credentials.id DESC
               LIMIT 1`,
            )
            .bind(selector.slug, now)
            .first<AgentForumIdentityRow>()
        : await db
            .prepare(
              `SELECT
                  users.id AS user_id,
                  users.display_name,
                  users.primary_email,
                  users.avatar_url,
                  users.status,
                  users.created_at AS user_created_at,
                  users.updated_at AS user_updated_at,
                  agent_profiles.slug,
                  agent_profiles.metadata_json,
                  agent_credentials.id AS credential_id,
                  agent_credentials.openauth_user_id,
                  agent_credentials.token_prefix
               FROM auth_identities
               INNER JOIN users ON users.id = auth_identities.user_id
               LEFT JOIN agent_profiles ON agent_profiles.user_id = users.id
               INNER JOIN agent_credentials ON agent_credentials.user_id = users.id
               WHERE auth_identities.provider = 'agent_programmatic'
                 AND auth_identities.provider_subject = ?
                 AND users.kind = 'agent'
                 AND users.status = 'active'
                 AND users.deleted_at IS NULL
                 AND ${activeCredentialClause}
               ORDER BY agent_credentials.created_at DESC, agent_credentials.id DESC
               LIMIT 1`,
            )
            .bind(selector.externalId, now)
            .first<AgentForumIdentityRow>()

    if (row === null) {
      return undefined
    }

    return {
      session: {
        user: {
          id: row.user_id,
          kind: 'agent',
          displayName: row.display_name,
          primaryEmail: row.primary_email,
          avatarUrl: row.avatar_url,
          status: row.status,
          createdAt: row.user_created_at,
          updatedAt: row.user_updated_at,
        },
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
