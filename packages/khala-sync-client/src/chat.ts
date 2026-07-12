import {
  canonicalJson,
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  encodeChatMessageEntity,
  encodeChatThreadEntity,
  MutatorName,
  personalScope,
  threadScope,
  type ChatMessageEntity,
  type ChatMessageImageAttachment,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import type { ClientMutator } from "./overlay.js"

export const CHAT_CREATE_THREAD_MUTATOR_NAME = "chat.createThread"
export const CHAT_APPEND_MESSAGE_MUTATOR_NAME = "chat.appendMessage"
export const CHAT_RENAME_THREAD_MUTATOR_NAME = "chat.renameThread"

export type ChatCreateThreadArgs = Readonly<{
  threadId: string
  title: string
}>

export type ChatAppendMessageArgs = Readonly<{
  threadId: string
  messageId: string
  body: string
  attachments?: ReadonlyArray<ChatMessageImageAttachment>
}>

export type ChatRenameThreadArgs = Readonly<{
  threadId: string
  title: string
}>

export type ChatClientMutatorOptions = Readonly<{
  ownerUserId: string
  now?: () => string
}>

export type ChatClientMutators = Readonly<{
  appendMessage: ClientMutator<ChatAppendMessageArgs>
  createThread: ClientMutator<ChatCreateThreadArgs>
  renameThread: ClientMutator<ChatRenameThreadArgs>
}>

const defaultNowIso = (): string => new Date().toISOString()
const normalizeChatTitle = (title: string): string => title.trim()
const timestampMs = (value: string | null): number => {
  if (value === null) return 0
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

export const compareChatThreadsForSidebar = (
  left: ChatThreadEntity,
  right: ChatThreadEntity,
): number => {
  const recency = timestampMs(right.updatedAt) - timestampMs(left.updatedAt)
  return recency !== 0 ? recency : right.threadId.localeCompare(left.threadId)
}

export const chatThreadsForSidebar = (
  threads: Iterable<ChatThreadEntity>,
  options: { readonly searchTerm?: string | null } = {},
): Array<ChatThreadEntity> => {
  const byThreadId = new Map<string, ChatThreadEntity>()
  for (const thread of threads) {
    const existing = byThreadId.get(thread.threadId)
    if (existing === undefined || timestampMs(thread.updatedAt) >= timestampMs(existing.updatedAt)) {
      byThreadId.set(thread.threadId, thread)
    }
  }
  const searchTerm = options.searchTerm?.trim().toLowerCase() ?? ""
  return [...byThreadId.values()]
    .filter(thread => searchTerm === "" ||
      thread.title.toLowerCase().includes(searchTerm) ||
      thread.threadId.toLowerCase().includes(searchTerm))
    .sort(compareChatThreadsForSidebar)
}

export const compareChatMessagesForTranscript = (
  left: ChatMessageEntity,
  right: ChatMessageEntity,
): number => {
  const created = timestampMs(left.createdAt) - timestampMs(right.createdAt)
  return created !== 0 ? created : left.messageId.localeCompare(right.messageId)
}

export const chatMessagesForTranscript = (
  messages: Iterable<ChatMessageEntity>,
): Array<ChatMessageEntity> => [...messages]
  .filter(message => message.deletedAt === null)
  .sort(compareChatMessagesForTranscript)

const baselineChatThread = (
  args: ChatCreateThreadArgs,
  options: ChatClientMutatorOptions,
): ChatThreadEntity => {
  const now = (options.now ?? defaultNowIso)()
  return decodeChatThreadEntity({
    createdAt: now,
    lastMessageAt: null,
    messageCount: 0,
    ownerUserId: options.ownerUserId,
    status: "active",
    threadId: args.threadId,
    title: normalizeChatTitle(args.title),
    updatedAt: now,
  })
}

const threadEffects = (
  entity: ChatThreadEntity,
): ReturnType<ClientMutator<ChatCreateThreadArgs>["apply"]> => {
  const postImageJson = canonicalJson(encodeChatThreadEntity(entity))
  return [
    {
      entityId: entity.threadId,
      entityType: CHAT_THREAD_ENTITY_TYPE,
      kind: "upsert",
      postImageJson,
      scope: personalScope(entity.ownerUserId),
    },
    {
      entityId: entity.threadId,
      entityType: CHAT_THREAD_ENTITY_TYPE,
      kind: "upsert",
      postImageJson,
      scope: threadScope(entity.threadId),
    },
  ]
}

export const chatCreateThreadClientMutator = (
  options: ChatClientMutatorOptions,
): ClientMutator<ChatCreateThreadArgs> => ({
  apply: args => threadEffects(baselineChatThread(args, options)),
  name: MutatorName.make(CHAT_CREATE_THREAD_MUTATOR_NAME),
})

export const chatRenameThreadClientMutator = (
  options: ChatClientMutatorOptions,
): ClientMutator<ChatRenameThreadArgs> => ({
  apply: (args, view) => {
    const currentJson = view.get(
      personalScope(options.ownerUserId),
      CHAT_THREAD_ENTITY_TYPE,
      args.threadId,
    )
    const current = currentJson === undefined
      ? baselineChatThread(args, options)
      : decodeChatThreadEntity(JSON.parse(currentJson) as unknown)
    return threadEffects(decodeChatThreadEntity({
      ...current,
      title: normalizeChatTitle(args.title),
      updatedAt: (options.now ?? defaultNowIso)(),
    }))
  },
  name: MutatorName.make(CHAT_RENAME_THREAD_MUTATOR_NAME),
})

export const chatAppendMessageClientMutator = (
  options: ChatClientMutatorOptions,
): ClientMutator<ChatAppendMessageArgs> => ({
  apply: (args, view) => {
    const currentJson =
      view.get(personalScope(options.ownerUserId), CHAT_THREAD_ENTITY_TYPE, args.threadId) ??
      view.get(threadScope(args.threadId), CHAT_THREAD_ENTITY_TYPE, args.threadId)
    if (currentJson === undefined) return []
    const current = decodeChatThreadEntity(JSON.parse(currentJson) as unknown)
    const now = (options.now ?? defaultNowIso)()
    const message = decodeChatMessageEntity({
      ...(args.attachments === undefined ? {} : { attachments: args.attachments }),
      authorUserId: options.ownerUserId,
      body: args.body,
      createdAt: now,
      deletedAt: null,
      messageId: args.messageId,
      threadId: args.threadId,
      updatedAt: now,
    })
    return [
      ...threadEffects(decodeChatThreadEntity({
        ...current,
        lastMessageAt: now,
        messageCount: current.messageCount + 1,
        updatedAt: now,
      })),
      {
        entityId: message.messageId,
        entityType: CHAT_MESSAGE_ENTITY_TYPE,
        kind: "upsert",
        postImageJson: canonicalJson(encodeChatMessageEntity(message)),
        scope: threadScope(message.threadId),
      },
    ]
  },
  name: MutatorName.make(CHAT_APPEND_MESSAGE_MUTATOR_NAME),
})

export const createChatClientMutators = (
  options: ChatClientMutatorOptions,
): ChatClientMutators => ({
  appendMessage: chatAppendMessageClientMutator(options),
  createThread: chatCreateThreadClientMutator(options),
  renameThread: chatRenameThreadClientMutator(options),
})
