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
