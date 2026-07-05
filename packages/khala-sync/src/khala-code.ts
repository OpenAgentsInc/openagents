import { Schema as S } from "effect"

/**
 * Khala Code product-state entity contracts (KS-8.13, #8324; SPEC §2.1
 * `scope.team.<teamId>` / `scope.thread.<threadId>`).
 *
 * These are the entity post-image shapes that ride inside
 * `ChangelogEntry.postImageJson` for the thread/team scopes produced by the
 * Khala Code product-state domain (threads, teams, workspaces) — the
 * server-side D1-shadow projection in `@openagentsinc/khala-sync-server`
 * (`khala-code-product-state-projection.ts`) allowlist-maps accepted D1 rows
 * into these schemas (never spreads raw rows) before anything is serialized.
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): every field is either a
 * closed literal set, a bounded ref, a bounded token, an ISO timestamp, or a
 * bounded content field that the authorized scope is allowed to read (chat
 * bodies and message payloads are the product for thread/team members). The
 * following NEVER appear in these shapes, by omission from the contracts:
 *
 *   - invite `token_hash` and `invitee_email` / `invitee_email_normalized`
 *     (`team_workspace_invites` — secret- and email-bearing)
 *   - `invited_by_actor_ref` (may embed an email-shaped actor ref)
 *   - R2 `object_key` / `storage_provider` internals (`thread_files`)
 *   - `metadata_json` blobs (unbounded, may carry local paths)
 *   - money-bearing balances (`teams.credits`)
 *   - `projection_json` / `audience_json` / `canonical_url` payloads
 *     (`share_projections` — served by their own public read path)
 *
 * Ref-typed fields structurally exclude `@` (emails), `/` (filesystem
 * paths), and whitespace, so a raw secret cannot even decode into them.
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle. Entity-type names
 * are exported as plain strings; brand them with `EntityType.make(...)` at
 * append call sites.
 */

// ---------------------------------------------------------------------------
// Entity type names (changelog `entityType` values)
// ---------------------------------------------------------------------------

export const KHALA_CODE_TEAM_ENTITY_TYPE = "team"
export const KHALA_CODE_TEAM_MEMBERSHIP_ENTITY_TYPE = "team_membership"
export const KHALA_CODE_TEAM_PROJECT_ENTITY_TYPE = "team_project"
export const KHALA_CODE_TEAM_INVITE_ENTITY_TYPE = "team_workspace_invite"
export const KHALA_CODE_TEAM_CHAT_MESSAGE_ENTITY_TYPE = "team_chat_message"
export const KHALA_CODE_THREAD_MESSAGE_ENTITY_TYPE = "thread_message"
export const KHALA_CODE_THREAD_FILE_ENTITY_TYPE = "thread_file"
export const KHALA_CODE_THREAD_FILE_MESSAGE_REF_ENTITY_TYPE =
  "thread_file_message_ref"
export const KHALA_CODE_PREFILLED_WORKSPACE_ENTITY_TYPE = "prefilled_workspace"
export const KHALA_CODE_SHARE_PROJECTION_ENTITY_TYPE = "share_projection"

export const KHALA_CODE_PRODUCT_STATE_ENTITY_TYPES = [
  KHALA_CODE_TEAM_ENTITY_TYPE,
  KHALA_CODE_TEAM_MEMBERSHIP_ENTITY_TYPE,
  KHALA_CODE_TEAM_PROJECT_ENTITY_TYPE,
  KHALA_CODE_TEAM_INVITE_ENTITY_TYPE,
  KHALA_CODE_TEAM_CHAT_MESSAGE_ENTITY_TYPE,
  KHALA_CODE_THREAD_MESSAGE_ENTITY_TYPE,
  KHALA_CODE_THREAD_FILE_ENTITY_TYPE,
  KHALA_CODE_THREAD_FILE_MESSAGE_REF_ENTITY_TYPE,
  KHALA_CODE_PREFILLED_WORKSPACE_ENTITY_TYPE,
  KHALA_CODE_SHARE_PROJECTION_ENTITY_TYPE,
] as const

// ---------------------------------------------------------------------------
// Bounded field primitives
// ---------------------------------------------------------------------------

/**
 * A bounded structured ref/id: dot/colon/dash/underscore-separated
 * identifier segments. Excludes `@`, `/`, and whitespace by construction,
 * so emails, filesystem paths, and URLs cannot decode into ref-typed
 * fields.
 */
export const KhalaCodeRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
export type KhalaCodeRef = typeof KhalaCodeRef.Type

/** ISO-8601 UTC timestamp string (same shape the wire contracts use). */
export const KhalaCodeIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)
export type KhalaCodeIsoTimestamp = typeof KhalaCodeIsoTimestamp.Type

/**
 * A bounded lower_snake_case classification token for status/kind sets that
 * evolve with the source system.
 */
export const KhalaCodeToken = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(64),
  S.isPattern(/^[a-z][a-z0-9_-]*$/),
)
export type KhalaCodeToken = typeof KhalaCodeToken.Type

/** Bounded human-readable name/title (team names, project names, titles). */
export const KhalaCodeName = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(500),
)
export type KhalaCodeName = typeof KhalaCodeName.Type

/** Bounded free text (descriptions, summaries). */
export const KhalaCodeText = S.String.check(S.isMaxLength(4000))
export type KhalaCodeText = typeof KhalaCodeText.Type

/**
 * Bounded message content. Team chat bodies are CHECK-bounded at 4000 in
 * D1; thread message `body_json` payloads are larger structured blobs.
 */
export const KhalaCodeMessageBody = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(262_144),
)
export type KhalaCodeMessageBody = typeof KhalaCodeMessageBody.Type

const boundedCount = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(1_000_000_000),
)

// ---------------------------------------------------------------------------
// team (`scope.team.<teamId>`)
// ---------------------------------------------------------------------------

export const KhalaCodeTeamKind = S.Literals(["organization", "personal"])
export type KhalaCodeTeamKind = typeof KhalaCodeTeamKind.Type

export const KhalaCodeTeamStatus = S.Literals(["active", "archived"])
export type KhalaCodeTeamStatus = typeof KhalaCodeTeamStatus.Type

/**
 * One team — the root entity of `scope.team.<teamId>`. Deliberately
 * excludes `credits` (money-bearing) and `logo_url`.
 */
export class KhalaCodeTeamEntity extends S.Class<KhalaCodeTeamEntity>(
  "KhalaCodeTeamEntity",
)({
  teamId: KhalaCodeRef,
  name: KhalaCodeName,
  slug: S.NullOr(KhalaCodeRef),
  kind: KhalaCodeTeamKind,
  plan: S.NullOr(KhalaCodeToken),
  status: KhalaCodeTeamStatus,
  ownerUserId: S.NullOr(KhalaCodeRef),
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  archivedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// team_membership
// ---------------------------------------------------------------------------

export const KhalaCodeTeamRole = S.Literals([
  "owner",
  "admin",
  "member",
  "viewer",
])
export type KhalaCodeTeamRole = typeof KhalaCodeTeamRole.Type

export const KhalaCodeMembershipStatus = S.Literals([
  "active",
  "invited",
  "removed",
])
export type KhalaCodeMembershipStatus = typeof KhalaCodeMembershipStatus.Type

/**
 * One team membership. `entityId` is `<teamId>:<userId>` (the D1 natural
 * key), which is what the acceptance criterion's membership-set equality
 * fingerprints.
 */
export class KhalaCodeTeamMembershipEntity extends S.Class<
  KhalaCodeTeamMembershipEntity
>("KhalaCodeTeamMembershipEntity")({
  membershipId: KhalaCodeRef,
  teamId: KhalaCodeRef,
  userId: KhalaCodeRef,
  role: KhalaCodeTeamRole,
  status: KhalaCodeMembershipStatus,
  invitedByUserId: S.NullOr(KhalaCodeRef),
  joinedAt: S.NullOr(KhalaCodeIsoTimestamp),
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  removedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// team_project
// ---------------------------------------------------------------------------

export const KhalaCodeProjectStatus = S.Literals(["active", "archived"])
export type KhalaCodeProjectStatus = typeof KhalaCodeProjectStatus.Type

/** One team project. Excludes `metadata_json`. */
export class KhalaCodeTeamProjectEntity extends S.Class<
  KhalaCodeTeamProjectEntity
>("KhalaCodeTeamProjectEntity")({
  projectId: KhalaCodeRef,
  teamId: KhalaCodeRef,
  slug: KhalaCodeRef,
  name: KhalaCodeName,
  description: KhalaCodeText,
  status: KhalaCodeProjectStatus,
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  archivedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// team_workspace_invite (REDACTED: no email, no token, no actor ref)
// ---------------------------------------------------------------------------

export const KhalaCodeInviteStatus = S.Literals([
  "pending",
  "accepted",
  "expired",
  "revoked",
])
export type KhalaCodeInviteStatus = typeof KhalaCodeInviteStatus.Type

/**
 * One team workspace invite, redacted for the team scope: the invitee's
 * email, the invite `token_hash`, the inviter actor ref, and the email
 * message id are all structurally absent. Team members see that an invite
 * exists, its role, and its lifecycle — the sensitive material stays on the
 * D1-authorized invite management read path.
 */
export class KhalaCodeTeamInviteEntity extends S.Class<
  KhalaCodeTeamInviteEntity
>("KhalaCodeTeamInviteEntity")({
  inviteId: KhalaCodeRef,
  teamId: KhalaCodeRef,
  projectId: S.NullOr(KhalaCodeRef),
  role: KhalaCodeTeamRole,
  status: KhalaCodeInviteStatus,
  acceptedByUserId: S.NullOr(KhalaCodeRef),
  sendCount: boundedCount,
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  expiresAt: KhalaCodeIsoTimestamp,
  acceptedAt: S.NullOr(KhalaCodeIsoTimestamp),
  revokedAt: S.NullOr(KhalaCodeIsoTimestamp),
  lastSentAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// team_chat_message
// ---------------------------------------------------------------------------

/**
 * One team chat message (also projected into the linked autopilot thread's
 * `scope.thread.<autopilotThreadId>` when present). Excludes
 * `metadata_json`.
 */
export class KhalaCodeTeamChatMessageEntity extends S.Class<
  KhalaCodeTeamChatMessageEntity
>("KhalaCodeTeamChatMessageEntity")({
  messageId: KhalaCodeRef,
  teamId: KhalaCodeRef,
  projectId: S.NullOr(KhalaCodeRef),
  authorUserId: KhalaCodeRef,
  kind: KhalaCodeToken,
  body: KhalaCodeMessageBody,
  autopilotThreadId: S.NullOr(KhalaCodeRef),
  agentRunId: S.NullOr(KhalaCodeRef),
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  deletedAt: S.NullOr(KhalaCodeIsoTimestamp),
  archivedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// thread_message (`scope.thread.<threadId>`)
// ---------------------------------------------------------------------------

/**
 * One thread message. `version` is the message's own optimistic version
 * column; the acceptance criterion's message-chain contiguity fingerprints
 * ride (threadId, createdAt, messageId) ordering.
 */
export class KhalaCodeThreadMessageEntity extends S.Class<
  KhalaCodeThreadMessageEntity
>("KhalaCodeThreadMessageEntity")({
  messageId: KhalaCodeRef,
  threadId: KhalaCodeRef,
  orgId: KhalaCodeRef,
  authorId: S.NullOr(KhalaCodeRef),
  bodyJson: KhalaCodeMessageBody,
  version: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  deletedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// thread_file (REDACTED: no object_key / storage internals)
// ---------------------------------------------------------------------------

export const KhalaCodeFileScope = S.Literals(["personal", "team"])
export type KhalaCodeFileScope = typeof KhalaCodeFileScope.Type

export const KhalaCodeUploadStatus = S.Literals(["uploaded", "failed"])
export type KhalaCodeUploadStatus = typeof KhalaCodeUploadStatus.Type

export const KhalaCodeScanStatus = S.Literals([
  "pending",
  "passed",
  "failed",
  "skipped",
])
export type KhalaCodeScanStatus = typeof KhalaCodeScanStatus.Type

/**
 * One thread file's metadata. The R2 `object_key` and storage provider are
 * structurally absent — clients fetch bytes through the authorized download
 * route, never from a synced storage pointer.
 */
export class KhalaCodeThreadFileEntity extends S.Class<
  KhalaCodeThreadFileEntity
>("KhalaCodeThreadFileEntity")({
  fileId: KhalaCodeRef,
  fileScope: KhalaCodeFileScope,
  threadId: KhalaCodeRef,
  teamId: S.NullOr(KhalaCodeRef),
  ownerUserId: KhalaCodeRef,
  filename: S.String.check(S.isMinLength(1), S.isMaxLength(512)),
  contentType: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  sizeBytes: boundedCount,
  checksumSha256: S.NullOr(S.String.check(S.isPattern(/^[0-9a-f]{64}$/))),
  uploadStatus: KhalaCodeUploadStatus,
  scanStatus: KhalaCodeScanStatus,
  downloadEnabled: S.Boolean,
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  deletedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// thread_file_message_ref
// ---------------------------------------------------------------------------

/** One file↔message reference edge. */
export class KhalaCodeThreadFileMessageRefEntity extends S.Class<
  KhalaCodeThreadFileMessageRefEntity
>("KhalaCodeThreadFileMessageRefEntity")({
  refId: KhalaCodeRef,
  fileId: KhalaCodeRef,
  threadId: S.NullOr(KhalaCodeRef),
  teamId: S.NullOr(KhalaCodeRef),
  messageId: KhalaCodeRef,
  referenceKind: KhalaCodeToken,
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  deletedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// prefilled_workspace (private-team scoped)
// ---------------------------------------------------------------------------

export const KhalaCodeWorkspaceStatus = S.Literals([
  "draft",
  "invited",
  "active",
  "archived",
])
export type KhalaCodeWorkspaceStatus = typeof KhalaCodeWorkspaceStatus.Type

/**
 * One prefilled workspace, projected into its private team's scope once
 * claimed. Excludes `holder_ref` (opaque prospect ref, potentially
 * email-shaped) and `intro_receipt_json`.
 */
export class KhalaCodePrefilledWorkspaceEntity extends S.Class<
  KhalaCodePrefilledWorkspaceEntity
>("KhalaCodePrefilledWorkspaceEntity")({
  workspaceId: KhalaCodeRef,
  holderUserId: S.NullOr(KhalaCodeRef),
  projectName: KhalaCodeName,
  status: KhalaCodeWorkspaceStatus,
  accessMode: S.NullOr(KhalaCodeToken),
  privateTeamId: S.NullOr(KhalaCodeRef),
  privateProjectId: S.NullOr(KhalaCodeRef),
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  archivedAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// share_projection (REDACTED: metadata only, no projection payload)
// ---------------------------------------------------------------------------

export const KhalaCodeShareStatus = S.Literals(["active", "revoked"])
export type KhalaCodeShareStatus = typeof KhalaCodeShareStatus.Type

/**
 * One share projection's lifecycle metadata for the owning team's scope.
 * The projected payload (`projection_json`), audience list, canonical URL,
 * and storage key are structurally absent — the public share surface serves
 * those through its own continuously-servable read path.
 */
export class KhalaCodeShareProjectionEntity extends S.Class<
  KhalaCodeShareProjectionEntity
>("KhalaCodeShareProjectionEntity")({
  shareId: KhalaCodeRef,
  sourceKind: KhalaCodeToken,
  sourceId: KhalaCodeRef,
  ownerUserId: KhalaCodeRef,
  teamId: S.NullOr(KhalaCodeRef),
  projectId: S.NullOr(KhalaCodeRef),
  title: KhalaCodeName,
  summary: S.NullOr(KhalaCodeText),
  status: KhalaCodeShareStatus,
  projectionVersion: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  redactionPolicyId: KhalaCodeRef,
  createdAt: KhalaCodeIsoTimestamp,
  updatedAt: KhalaCodeIsoTimestamp,
  revokedAt: S.NullOr(KhalaCodeIsoTimestamp),
  expiresAt: S.NullOr(KhalaCodeIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodeKhalaCodeTeamEntity = S.decodeUnknownSync(
  KhalaCodeTeamEntity,
)
export const encodeKhalaCodeTeamEntity = S.encodeSync(KhalaCodeTeamEntity)
export const decodeKhalaCodeTeamMembershipEntity = S.decodeUnknownSync(
  KhalaCodeTeamMembershipEntity,
)
export const encodeKhalaCodeTeamMembershipEntity = S.encodeSync(
  KhalaCodeTeamMembershipEntity,
)
export const decodeKhalaCodeTeamProjectEntity = S.decodeUnknownSync(
  KhalaCodeTeamProjectEntity,
)
export const encodeKhalaCodeTeamProjectEntity = S.encodeSync(
  KhalaCodeTeamProjectEntity,
)
export const decodeKhalaCodeTeamInviteEntity = S.decodeUnknownSync(
  KhalaCodeTeamInviteEntity,
)
export const encodeKhalaCodeTeamInviteEntity = S.encodeSync(
  KhalaCodeTeamInviteEntity,
)
export const decodeKhalaCodeTeamChatMessageEntity = S.decodeUnknownSync(
  KhalaCodeTeamChatMessageEntity,
)
export const encodeKhalaCodeTeamChatMessageEntity = S.encodeSync(
  KhalaCodeTeamChatMessageEntity,
)
export const decodeKhalaCodeThreadMessageEntity = S.decodeUnknownSync(
  KhalaCodeThreadMessageEntity,
)
export const encodeKhalaCodeThreadMessageEntity = S.encodeSync(
  KhalaCodeThreadMessageEntity,
)
export const decodeKhalaCodeThreadFileEntity = S.decodeUnknownSync(
  KhalaCodeThreadFileEntity,
)
export const encodeKhalaCodeThreadFileEntity = S.encodeSync(
  KhalaCodeThreadFileEntity,
)
export const decodeKhalaCodeThreadFileMessageRefEntity = S.decodeUnknownSync(
  KhalaCodeThreadFileMessageRefEntity,
)
export const encodeKhalaCodeThreadFileMessageRefEntity = S.encodeSync(
  KhalaCodeThreadFileMessageRefEntity,
)
export const decodeKhalaCodePrefilledWorkspaceEntity = S.decodeUnknownSync(
  KhalaCodePrefilledWorkspaceEntity,
)
export const encodeKhalaCodePrefilledWorkspaceEntity = S.encodeSync(
  KhalaCodePrefilledWorkspaceEntity,
)
export const decodeKhalaCodeShareProjectionEntity = S.decodeUnknownSync(
  KhalaCodeShareProjectionEntity,
)
export const encodeKhalaCodeShareProjectionEntity = S.encodeSync(
  KhalaCodeShareProjectionEntity,
)
