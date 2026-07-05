import {
  canonicalJson,
  encodeKhalaCodePrefilledWorkspaceEntity,
  encodeKhalaCodeShareProjectionEntity,
  encodeKhalaCodeShareProjectionRecipientEntity,
  encodeKhalaCodeTeamChatMessageEntity,
  encodeKhalaCodeTeamEntity,
  encodeKhalaCodeTeamInviteEntity,
  encodeKhalaCodeTeamMembershipEntity,
  encodeKhalaCodeTeamProjectEntity,
  encodeKhalaCodeThreadFileEntity,
  encodeKhalaCodeThreadFileMessageRefEntity,
  encodeKhalaCodeThreadMessageEntity,
  EntityId,
  EntityType,
  KhalaCodePrefilledWorkspaceEntity,
  KhalaCodeShareProjectionEntity,
  KhalaCodeShareProjectionRecipientEntity,
  KhalaCodeTeamChatMessageEntity,
  KhalaCodeTeamEntity,
  KhalaCodeTeamInviteEntity,
  KhalaCodeTeamMembershipEntity,
  KhalaCodeTeamProjectEntity,
  KhalaCodeThreadFileEntity,
  KhalaCodeThreadFileMessageRefEntity,
  KhalaCodeThreadMessageEntity,
  personalScope,
  teamScope,
  threadScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"

import {
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
  type KhalaCodeProductStateRow,
  type KhalaCodeProductStateTable,
} from "./khala-code-product-state-tables.js"

/**
 * KS-8.13 (#8324): Khala Code product-state → sync-scope projection.
 *
 * Maps accepted D1 rows into the typed PUBLIC-SAFE entity contracts from
 * `@openagentsinc/khala-sync` (`khala-code.ts`) and routes them to the
 * `scope.team.<teamId>` / `scope.thread.<threadId>` scopes their subscribers
 * ride. Raw rows NEVER reach a post-image: every projected entity is built
 * through an explicit allowlist mapping (never spreads), validated by its
 * schema, and then screened against
 * {@link KHALA_CODE_POST_IMAGE_FORBIDDEN_PATTERN} as a belt-and-braces guard
 * (SPEC §7 invariant 9).
 *
 * Projection is FAIL-SOFT relative to the Postgres row mirror: a row that
 * cannot be mapped (schema drift, unparseable timestamp, redaction match)
 * skips its scope change — the Cloud SQL twin still converges and the skip
 * is surfaced through the `onSkip` callback so the Worker can log a
 * diagnostic. D1 stays the read authority throughout the shadow window.
 */

// ---------------------------------------------------------------------------
// Redaction guard
// ---------------------------------------------------------------------------

/**
 * Forbidden material that must never ride a product-state post-image's
 * STRUCTURAL fields: secret-ish key names, invite token material, email
 * addresses, and local filesystem paths.
 */
export const KHALA_CODE_POST_IMAGE_FORBIDDEN_PATTERN =
  /token_hash|apiKey|authorization|\/Users\/|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/i

/**
 * Bounded free-content fields that the authorized scope is allowed to read
 * (chat bodies, message payloads, filenames, human names/titles). A user
 * typing an email address or a path into a chat message is product content,
 * not a leak, so these fields are exempt from the forbidden-material scan —
 * their safety comes from the allowlist mapping itself (the sensitive D1
 * columns are never mapped at all).
 */
export const KHALA_CODE_POST_IMAGE_CONTENT_FIELDS: ReadonlySet<string> =
  new Set([
    "authorName",
    "body",
    "bodyJson",
    "description",
    "filename",
    "name",
    "projectName",
    "summary",
    "title",
  ])

export class KhalaCodePostImageRedactionError extends Error {
  readonly _tag = "KhalaCodePostImageRedactionError"
  override readonly name = "KhalaCodePostImageRedactionError"
  constructor(readonly entityType: string) {
    // Deliberately does NOT echo the offending value.
    super(
      `refusing to project ${entityType}: serialized post-image matches the ` +
        "forbidden-material pattern (SPEC §7 invariant 9)",
    )
  }
}

const structuralView = (postImage: unknown): unknown => {
  if (typeof postImage !== "object" || postImage === null) return postImage
  const structural: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(postImage)) {
    if (!KHALA_CODE_POST_IMAGE_CONTENT_FIELDS.has(key)) {
      structural[key] = value
    }
  }
  return structural
}

const assertRedacted = (entityType: string, postImage: unknown): unknown => {
  if (
    KHALA_CODE_POST_IMAGE_FORBIDDEN_PATTERN.test(
      canonicalJson(structuralView(postImage)),
    )
  ) {
    throw new KhalaCodePostImageRedactionError(entityType)
  }
  return postImage
}

// ---------------------------------------------------------------------------
// Row-value coercion helpers (D1 read-back and backfill rows both pass here)
// ---------------------------------------------------------------------------

const str = (
  row: KhalaCodeProductStateRow,
  column: string,
): string | undefined => {
  const value = row[column]
  return value === undefined || value === null || String(value).length === 0
    ? undefined
    : String(value)
}

const requireStr = (row: KhalaCodeProductStateRow, column: string): string => {
  const value = str(row, column)
  if (value === undefined) {
    throw new Error(`missing required column ${column}`)
  }
  return value
}

const nullableStr = (
  row: KhalaCodeProductStateRow,
  column: string,
): string | null => str(row, column) ?? null

/** Normalize any parseable timestamp representation to ISO-8601 UTC. */
const toIso = (value: unknown): string => {
  if (value instanceof Date) return value.toISOString()
  if (typeof value === "number") return new Date(value).toISOString()
  return new Date(String(value)).toISOString()
}

const requireIso = (row: KhalaCodeProductStateRow, column: string): string =>
  toIso(requireStr(row, column))

const nullableIso = (
  row: KhalaCodeProductStateRow,
  column: string,
): string | null => {
  const value = str(row, column)
  return value === undefined ? null : toIso(value)
}

const toInt = (value: unknown, fallback?: number): number => {
  const parsed =
    typeof value === "number"
      ? value
      : typeof value === "bigint"
        ? Number(value)
        : Number(String(value))
  if (!Number.isFinite(parsed)) {
    if (fallback !== undefined) return fallback
    throw new Error("expected an integer column value")
  }
  return Math.trunc(parsed)
}

const toBool = (value: unknown, fallback: boolean): boolean => {
  if (value === undefined || value === null) return fallback
  if (typeof value === "boolean") return value
  if (typeof value === "number") return value !== 0
  const normalized = String(value).trim().toLowerCase()
  if (normalized === "true" || normalized === "1") return true
  if (normalized === "false" || normalized === "0") return false
  return fallback
}

// ---------------------------------------------------------------------------
// Allowlist mappings (raw row shapes → contract entities, encoded for wire)
// ---------------------------------------------------------------------------

export const khalaCodeTeamPostImage = (row: KhalaCodeProductStateRow) =>
  encodeKhalaCodeTeamEntity(
    new KhalaCodeTeamEntity({
      archivedAt: nullableIso(row, "archived_at"),
      createdAt: requireIso(row, "created_at"),
      kind: requireStr(row, "kind") as "organization" | "personal",
      name: requireStr(row, "name"),
      ownerUserId: nullableStr(row, "owner_user_id"),
      plan: nullableStr(row, "plan"),
      slug: nullableStr(row, "slug"),
      status: requireStr(row, "status") as "active" | "archived",
      teamId: requireStr(row, "id"),
      updatedAt: requireIso(row, "updated_at"),
    }),
  )

export const khalaCodeTeamMembershipPostImage = (
  row: KhalaCodeProductStateRow,
) =>
  encodeKhalaCodeTeamMembershipEntity(
    new KhalaCodeTeamMembershipEntity({
      createdAt: requireIso(row, "created_at"),
      invitedByUserId: nullableStr(row, "invited_by_user_id"),
      joinedAt: nullableIso(row, "joined_at"),
      membershipId: requireStr(row, "id"),
      removedAt: nullableIso(row, "removed_at"),
      role: requireStr(row, "role") as "owner" | "admin" | "member" | "viewer",
      status: requireStr(row, "status") as "active" | "invited" | "removed",
      teamId: requireStr(row, "team_id"),
      updatedAt: requireIso(row, "updated_at"),
      userId: requireStr(row, "user_id"),
    }),
  )

export const khalaCodeTeamProjectPostImage = (row: KhalaCodeProductStateRow) =>
  encodeKhalaCodeTeamProjectEntity(
    new KhalaCodeTeamProjectEntity({
      archivedAt: nullableIso(row, "archived_at"),
      createdAt: requireIso(row, "created_at"),
      description: str(row, "description") ?? "",
      name: requireStr(row, "name"),
      projectId: requireStr(row, "id"),
      slug: requireStr(row, "slug"),
      status: requireStr(row, "status") as "active" | "archived",
      teamId: requireStr(row, "team_id"),
      updatedAt: requireIso(row, "updated_at"),
    }),
  )

export const khalaCodeTeamInvitePostImage = (row: KhalaCodeProductStateRow) =>
  encodeKhalaCodeTeamInviteEntity(
    new KhalaCodeTeamInviteEntity({
      acceptedAt: nullableIso(row, "accepted_at"),
      acceptedByUserId: nullableStr(row, "accepted_by_user_id"),
      createdAt: requireIso(row, "created_at"),
      expiresAt: requireIso(row, "expires_at"),
      inviteId: requireStr(row, "id"),
      lastSentAt: nullableIso(row, "last_sent_at"),
      projectId: nullableStr(row, "project_id"),
      revokedAt: nullableIso(row, "revoked_at"),
      role: requireStr(row, "role") as "admin" | "member" | "viewer",
      sendCount: toInt(row["send_count"], 0),
      status: requireStr(row, "status") as
        | "pending"
        | "accepted"
        | "expired"
        | "revoked",
      teamId: requireStr(row, "team_id"),
      updatedAt: requireIso(row, "updated_at"),
    }),
  )

export const khalaCodeTeamChatMessagePostImage = (
  row: KhalaCodeProductStateRow,
) =>
  encodeKhalaCodeTeamChatMessageEntity(
    new KhalaCodeTeamChatMessageEntity({
      agentRunId: nullableStr(row, "agent_run_id"),
      archivedAt: nullableIso(row, "archived_at"),
      // KS-6.11 (#8422): denormalized author display-identity snapshot.
      // Only present when the row was read back through the Worker mirror's
      // `team_chat_messages`-specific JOIN (khala-code-product-state-store.ts);
      // the generic backfill/verify sweep reads raw rows without it, so these
      // fall back to `null` for historical rows rather than throwing.
      authorAvatarUrl: nullableStr(row, "author_avatar_url"),
      authorGithubUsername: nullableStr(row, "author_github_username"),
      authorName: nullableStr(row, "author_name"),
      authorUserId: requireStr(row, "author_user_id"),
      autopilotThreadId: nullableStr(row, "autopilot_thread_id"),
      body: requireStr(row, "body"),
      createdAt: requireIso(row, "created_at"),
      deletedAt: nullableIso(row, "deleted_at"),
      kind: requireStr(row, "kind"),
      messageId: requireStr(row, "id"),
      projectId: nullableStr(row, "project_id"),
      teamId: requireStr(row, "team_id"),
      updatedAt: requireIso(row, "updated_at"),
    }),
  )

export const khalaCodeThreadMessagePostImage = (
  row: KhalaCodeProductStateRow,
) =>
  encodeKhalaCodeThreadMessageEntity(
    new KhalaCodeThreadMessageEntity({
      authorId: nullableStr(row, "author_id"),
      bodyJson: requireStr(row, "body_json"),
      createdAt: requireIso(row, "created_at"),
      deletedAt: nullableIso(row, "deleted_at"),
      messageId: requireStr(row, "id"),
      orgId: requireStr(row, "org_id"),
      threadId: requireStr(row, "thread_id"),
      updatedAt: requireIso(row, "updated_at"),
      version: toInt(row["version"], 1),
    }),
  )

export const khalaCodeThreadFilePostImage = (row: KhalaCodeProductStateRow) =>
  encodeKhalaCodeThreadFileEntity(
    new KhalaCodeThreadFileEntity({
      checksumSha256: nullableStr(row, "checksum_sha256"),
      contentType: requireStr(row, "content_type"),
      createdAt: requireIso(row, "created_at"),
      deletedAt: nullableIso(row, "deleted_at"),
      downloadEnabled: toBool(row["download_enabled"], true),
      fileId: requireStr(row, "id"),
      filename: requireStr(row, "filename"),
      fileScope: requireStr(row, "scope") as "personal" | "team",
      ownerUserId: requireStr(row, "owner_user_id"),
      scanStatus: requireStr(row, "scan_status") as
        | "pending"
        | "passed"
        | "failed"
        | "skipped",
      sizeBytes: toInt(row["size_bytes"]),
      teamId: nullableStr(row, "team_id"),
      threadId: requireStr(row, "thread_id"),
      updatedAt: requireIso(row, "updated_at"),
      uploadStatus: requireStr(row, "upload_status") as "uploaded" | "failed",
    }),
  )

export const khalaCodeThreadFileMessageRefPostImage = (
  row: KhalaCodeProductStateRow,
) =>
  encodeKhalaCodeThreadFileMessageRefEntity(
    new KhalaCodeThreadFileMessageRefEntity({
      createdAt: requireIso(row, "created_at"),
      deletedAt: nullableIso(row, "deleted_at"),
      fileId: requireStr(row, "file_id"),
      messageId: requireStr(row, "message_id"),
      refId: requireStr(row, "id"),
      referenceKind: requireStr(row, "reference_kind"),
      teamId: nullableStr(row, "team_id"),
      threadId: nullableStr(row, "thread_id"),
      updatedAt: requireIso(row, "updated_at"),
    }),
  )

export const khalaCodePrefilledWorkspacePostImage = (
  row: KhalaCodeProductStateRow,
) =>
  encodeKhalaCodePrefilledWorkspaceEntity(
    new KhalaCodePrefilledWorkspaceEntity({
      accessMode: nullableStr(row, "access_mode"),
      archivedAt: nullableIso(row, "archived_at"),
      createdAt: requireIso(row, "created_at"),
      holderUserId: nullableStr(row, "holder_user_id"),
      privateProjectId: nullableStr(row, "private_project_id"),
      privateTeamId: nullableStr(row, "private_team_id"),
      projectName: requireStr(row, "project_name"),
      status: requireStr(row, "status") as
        | "draft"
        | "invited"
        | "active"
        | "archived",
      updatedAt: requireIso(row, "updated_at"),
      workspaceId: requireStr(row, "id"),
    }),
  )

export const khalaCodeShareProjectionPostImage = (
  row: KhalaCodeProductStateRow,
) =>
  encodeKhalaCodeShareProjectionEntity(
    new KhalaCodeShareProjectionEntity({
      createdAt: requireIso(row, "created_at"),
      expiresAt: nullableIso(row, "expires_at"),
      ownerUserId: requireStr(row, "owner_user_id"),
      projectId: nullableStr(row, "project_id"),
      projectionVersion: toInt(row["projection_version"], 1),
      redactionPolicyId: str(row, "redaction_policy_id") ?? "default",
      revokedAt: nullableIso(row, "revoked_at"),
      shareId: requireStr(row, "id"),
      sourceId: requireStr(row, "source_id"),
      sourceKind: requireStr(row, "source_kind"),
      status: requireStr(row, "status") as "active" | "revoked",
      summary: nullableStr(row, "summary"),
      teamId: nullableStr(row, "team_id"),
      title: requireStr(row, "title"),
      updatedAt: requireIso(row, "updated_at"),
    }),
  )

export const khalaCodeShareProjectionRecipientPostImage = (
  row: KhalaCodeProductStateRow,
) =>
  encodeKhalaCodeShareProjectionRecipientEntity(
    new KhalaCodeShareProjectionRecipientEntity({
      createdAt: requireIso(row, "created_at"),
      shareId: requireStr(row, "share_id"),
      subjectId: requireStr(row, "subject_id"),
      subjectKind: requireStr(row, "subject_kind") as "user" | "team",
    }),
  )

const POST_IMAGE_MAPPERS: Partial<
  Record<
    KhalaCodeProductStateTable,
    (row: KhalaCodeProductStateRow) => unknown
  >
> = {
  prefilled_workspaces: khalaCodePrefilledWorkspacePostImage,
  share_projection_recipients: khalaCodeShareProjectionRecipientPostImage,
  share_projections: khalaCodeShareProjectionPostImage,
  team_chat_messages: khalaCodeTeamChatMessagePostImage,
  team_memberships: khalaCodeTeamMembershipPostImage,
  team_projects: khalaCodeTeamProjectPostImage,
  team_workspace_invites: khalaCodeTeamInvitePostImage,
  teams: khalaCodeTeamPostImage,
  thread_file_message_refs: khalaCodeThreadFileMessageRefPostImage,
  thread_files: khalaCodeThreadFilePostImage,
  thread_messages: khalaCodeThreadMessagePostImage,
}

// ---------------------------------------------------------------------------
// Scope routing
// ---------------------------------------------------------------------------

export type KhalaCodeProductStateScopeChange = Readonly<{
  scope: SyncScope
  entityType: EntityType
  entityId: EntityId
  /** Encoded PUBLIC-SAFE contract entity — never the raw D1 row. */
  postImage: unknown
}>

export type KhalaCodeProductStateProjectionSkip = (
  table: KhalaCodeProductStateTable,
  reasonSafe: string,
) => void

const stringValue = (
  row: KhalaCodeProductStateRow,
  column: string,
): string | undefined => str(row, column)

const entityIdForRow = (
  table: KhalaCodeProductStateTable,
  row: KhalaCodeProductStateRow,
): EntityId => {
  const spec = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table]
  const raw = spec.keyColumns
    .map((column) => stringValue(row, column) ?? "")
    .join(":")
  return EntityId.make(raw.length === 0 ? table : raw)
}

const entityTypeForTable = (table: KhalaCodeProductStateTable): EntityType =>
  EntityType.make(table.endsWith("s") ? table.slice(0, -1) : table)

const scopesForRow = (
  table: KhalaCodeProductStateTable,
  row: KhalaCodeProductStateRow,
): ReadonlyArray<SyncScope> => {
  const scopes: Array<SyncScope> = []
  const teamId = stringValue(row, "team_id")
  const threadId = stringValue(row, "thread_id")
  const autopilotThreadId = stringValue(row, "autopilot_thread_id")

  switch (table) {
    case "teams": {
      const id = stringValue(row, "id")
      if (id !== undefined) scopes.push(teamScope(id))
      break
    }
    case "team_memberships":
    case "team_projects":
    case "team_workspace_invites":
      if (teamId !== undefined) scopes.push(teamScope(teamId))
      break
    case "team_chat_messages":
      if (teamId !== undefined) scopes.push(teamScope(teamId))
      if (autopilotThreadId !== undefined) {
        scopes.push(threadScope(autopilotThreadId))
      }
      break
    case "thread_messages":
      if (threadId !== undefined) scopes.push(threadScope(threadId))
      break
    case "thread_files":
    case "thread_file_message_refs":
      if (teamId !== undefined) scopes.push(teamScope(teamId))
      if (threadId !== undefined) scopes.push(threadScope(threadId))
      break
    case "prefilled_workspaces": {
      const privateTeamId = stringValue(row, "private_team_id")
      if (privateTeamId !== undefined) scopes.push(teamScope(privateTeamId))
      break
    }
    case "share_projections":
      if (teamId !== undefined) scopes.push(teamScope(teamId))
      break
    case "share_projection_recipients": {
      // A recipient row is projected into the SUBJECT's own scope. `email`
      // subjects have no sync scope (and the id is PII) so they never fan
      // out — they stay Postgres-mirror-only.
      const subjectKind = stringValue(row, "subject_kind")
      const subjectId = stringValue(row, "subject_id")
      if (subjectId !== undefined) {
        if (subjectKind === "user") scopes.push(personalScope(subjectId))
        else if (subjectKind === "team") scopes.push(teamScope(subjectId))
      }
      break
    }
    default:
      break
  }

  return scopes
}

/**
 * Bounded, value-free skip reason. Schema decode errors echo the offending
 * value in their message, so anything that is not one of OUR deliberate
 * errors is reduced to its constructor name — a skip diagnostic must never
 * become the leak it prevented.
 */
const safeReason = (error: unknown): string => {
  if (error instanceof KhalaCodePostImageRedactionError) return error.message
  if (error instanceof Error && error.message.startsWith("missing required column")) {
    return error.message.slice(0, 200)
  }
  const name =
    error instanceof Error ? error.name : typeof error
  return `post-image mapping failed (${name})`.slice(0, 200)
}

/**
 * Route a mirrored row into the Khala Sync scopes that clients subscribe
 * to, with the post-image allowlist-mapped into the typed public-safe
 * contract entity for the table. Rows whose scope routing yields nothing
 * (receipt/feedback/cloud tables — Postgres-mirror-only during KS-8.13)
 * return no changes; rows that cannot be mapped or that trip the redaction
 * guard are skipped fail-soft via `onSkip`.
 */
export const scopeChangesForKhalaCodeProductStateRow = (
  table: KhalaCodeProductStateTable,
  row: KhalaCodeProductStateRow,
  onSkip?: KhalaCodeProductStateProjectionSkip,
): ReadonlyArray<KhalaCodeProductStateScopeChange> => {
  const scopes = scopesForRow(table, row)
  if (scopes.length === 0) return []

  const mapper = POST_IMAGE_MAPPERS[table]
  if (mapper === undefined) {
    onSkip?.(table, "no post-image contract registered for table")
    return []
  }

  const entityType = entityTypeForTable(table)
  let postImage: unknown
  try {
    postImage = assertRedacted(String(entityType), mapper(row))
  } catch (error) {
    onSkip?.(table, safeReason(error))
    return []
  }

  const entityId = entityIdForRow(table, row)
  return scopes.map((scope) => ({ entityId, entityType, postImage, scope }))
}

// ---------------------------------------------------------------------------
// Delete tombstones (KS-8.13 follow-up #8356)
// ---------------------------------------------------------------------------

/**
 * One tombstone target: the (scope, entityType, entityId) triple a
 * `op:"delete"` changelog entry is appended to. Tombstones carry NO
 * post-image, so — unlike {@link scopeChangesForKhalaCodeProductStateRow} —
 * this resolves scope/type/id purely from the row's key + scope columns and
 * never runs the allowlist post-image mapper or the redaction guard: there is
 * no serialized entity body to leak, and a row that fails post-image mapping
 * (e.g. schema drift on a NON-key column) must still be able to replicate its
 * removal to subscribers.
 */
export type KhalaCodeProductStateScopeTombstone = Readonly<{
  scope: SyncScope
  entityType: EntityType
  entityId: EntityId
}>

/**
 * Resolve the delete-tombstone targets for a hard-deleted product-state row.
 *
 * The Worker mirror READS the row(s) a hard-delete will remove BEFORE issuing
 * the delete (their scope/key columns are gone afterward), converges the
 * Postgres twin, and appends one `op:"delete"` changelog entry per resolved
 * scope so that scope subscribers converge on the removal (the interactive
 * chat/thread/file surfaces instead soft-delete via `deleted_at`, which rides
 * as a normal upsert). Rows whose scope routing yields nothing
 * (receipt/feedback/cloud/workroom tables — Postgres-mirror-only) return no
 * tombstones, exactly as they produce no upsert fan-out.
 */
export const scopeTombstonesForKhalaCodeProductStateRow = (
  table: KhalaCodeProductStateTable,
  row: KhalaCodeProductStateRow,
): ReadonlyArray<KhalaCodeProductStateScopeTombstone> => {
  const scopes = scopesForRow(table, row)
  if (scopes.length === 0) return []
  const entityType = entityTypeForTable(table)
  const entityId = entityIdForRow(table, row)
  return scopes.map((scope) => ({ entityId, entityType, scope }))
}
