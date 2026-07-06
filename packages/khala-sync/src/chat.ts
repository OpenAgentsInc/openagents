import { Schema as S } from "effect"

/**
 * Owner-private chat entities for the mobile-chat milestone (MC-1, #8352).
 *
 * Thread metadata is replicated into the owner's personal scope and the
 * thread-local scope. Message bodies replicate only into the thread-local
 * `scope.thread.<threadId>` scope; no chat mutator writes a public scope.
 */

export const CHAT_THREAD_ENTITY_TYPE = "chat_thread"
export const CHAT_MESSAGE_ENTITY_TYPE = "chat_message"

export const ChatPublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
export type ChatPublicRef = typeof ChatPublicRef.Type

export const ChatIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)
export type ChatIsoTimestamp = typeof ChatIsoTimestamp.Type

export const ChatTitle = S.String.check(S.isMaxLength(160))
export type ChatTitle = typeof ChatTitle.Type

export const ChatMessageBody = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(20_000),
)
export type ChatMessageBody = typeof ChatMessageBody.Type

/**
 * Thread<->repo binding (MM-B2, #8472, mobile-only MVP pivot): the repo a
 * mobile thread is pinned to, so the org-owned cloud executor (MM-C1/C3,
 * Lane 0) can materialize a checkout without the mobile app carrying its own
 * clone. Owner/name follow GitHub's own charset (alphanumerics, hyphens,
 * underscores, periods); `defaultBranch` is recorded at bind time so the
 * executor has a concrete ref even before it walks the live repo. This is
 * additive and optional on `ChatThreadEntity` — legacy/repo-less threads
 * decode fine with no `repoBinding` key at all (plain chat continues to
 * work), per the launch audit §4 ("Repo-less threads stay allowed").
 */
export const ChatThreadRepoOwner = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(64),
  S.isPattern(/^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$/),
)
export type ChatThreadRepoOwner = typeof ChatThreadRepoOwner.Type

export const ChatThreadRepoName = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(200),
  S.isPattern(/^[A-Za-z0-9._-]+$/),
)
export type ChatThreadRepoName = typeof ChatThreadRepoName.Type

export const ChatThreadRepoBranch = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(255),
)
export type ChatThreadRepoBranch = typeof ChatThreadRepoBranch.Type

export class ChatThreadRepoBinding extends S.Class<ChatThreadRepoBinding>(
  "ChatThreadRepoBinding",
)({
  owner: ChatThreadRepoOwner,
  name: ChatThreadRepoName,
  defaultBranch: ChatThreadRepoBranch,
}) {}

export const decodeChatThreadRepoBinding = S.decodeUnknownSync(
  ChatThreadRepoBinding,
)
export const encodeChatThreadRepoBinding = S.encodeSync(ChatThreadRepoBinding)

/** `owner/name` — the stable string form used for display and as a
 * dedupe/comparison key; never itself the wire representation (the struct
 * fields are). */
export const chatThreadRepoBindingRef = (
  binding: Pick<ChatThreadRepoBinding, "name" | "owner">,
): string => `${binding.owner}/${binding.name}`

export class ChatThreadEntity extends S.Class<ChatThreadEntity>(
  "ChatThreadEntity",
)({
  threadId: ChatPublicRef,
  ownerUserId: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  title: ChatTitle,
  status: S.Literal("active"),
  messageCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  lastMessageAt: S.NullOr(ChatIsoTimestamp),
  createdAt: ChatIsoTimestamp,
  updatedAt: ChatIsoTimestamp,
  /** `undefined`/absent on legacy rows and repo-less threads; `null` once a
   * caller has explicitly recorded "no repo"; a real binding once a repo has
   * been picked. `S.optional` (not `S.optionalWith(..., {default})`, not
   * available in this repo's pinned Effect version) so decoding a
   * pre-#8472 stored row that never had this key never fails. */
  repoBinding: S.optional(S.NullOr(ChatThreadRepoBinding)),
}) {}

export class ChatMessageEntity extends S.Class<ChatMessageEntity>(
  "ChatMessageEntity",
)({
  messageId: ChatPublicRef,
  threadId: ChatPublicRef,
  authorUserId: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  body: ChatMessageBody,
  createdAt: ChatIsoTimestamp,
  updatedAt: ChatIsoTimestamp,
  deletedAt: S.NullOr(ChatIsoTimestamp),
}) {}

export const decodeChatThreadEntity = S.decodeUnknownSync(ChatThreadEntity)
export const decodeChatMessageEntity = S.decodeUnknownSync(ChatMessageEntity)
export const encodeChatThreadEntity = S.encodeSync(ChatThreadEntity)
export const encodeChatMessageEntity = S.encodeSync(ChatMessageEntity)
