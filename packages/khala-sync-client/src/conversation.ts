import {
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  personalScope,
  threadScope,
  type MutationId,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import type {
  ChatAppendMessageArgs,
  ChatClientMutators,
  ChatCreateThreadArgs,
} from "./chat.js"
import type { OverlayError } from "./overlay.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type {
  KhalaSyncClientStoreError,
  KhalaSyncLocalStore,
} from "./store.js"

export type ConfirmedChatThread = Readonly<{
  threadRef: string
  title: string
  messageCount: number
  lastMessageAt: string | null
  updatedAt: string
  version: number
}>

export type ConfirmedChatMessage = Readonly<{
  messageRef: string
  threadRef: string
  body: string
  createdAt: string
  updatedAt: string
  version: number
}>

export type KhalaSyncConversationStatus = Readonly<{
  phase: ScopeSyncState["phase"]
  cursor: number | null
  pendingMutationCount: number
}>

export type KhalaSyncConversation = Readonly<{
  personalStatus: () => KhalaSyncConversationStatus
  threadStatus: (threadRef: string) => KhalaSyncConversationStatus
  listConfirmedThreads: () => Effect.Effect<
    ReadonlyArray<ConfirmedChatThread>,
    KhalaSyncClientStoreError
  >
  openThread: (threadRef: string) => Effect.Effect<void, OverlayError>
  listConfirmedMessages: (threadRef: string) => Effect.Effect<
    ReadonlyArray<ConfirmedChatMessage>,
    KhalaSyncClientStoreError
  >
  createThread: (args: ChatCreateThreadArgs) => Effect.Effect<MutationId, OverlayError>
  appendMessage: (args: ChatAppendMessageArgs) => Effect.Effect<MutationId, OverlayError>
}>

const cursorFromState = (state: ScopeSyncState): number | null =>
  state.phase === "live" || state.phase === "catching_up"
    ? Number(state.cursor)
    : null

const decodeThreads = (
  ownerUserId: string,
  rows: ReadonlyArray<{
    readonly postImageJson: string
    readonly version: number
  }>,
): ReadonlyArray<ConfirmedChatThread> => {
  const threads: Array<ConfirmedChatThread> = []
  for (const row of rows) {
    try {
      const thread = decodeChatThreadEntity(JSON.parse(row.postImageJson) as unknown)
      if (thread.ownerUserId !== ownerUserId) continue
      threads.push({
        threadRef: thread.threadId,
        title: thread.title,
        messageCount: thread.messageCount,
        lastMessageAt: thread.lastMessageAt,
        updatedAt: thread.updatedAt,
        version: Number(row.version),
      })
    } catch {
      // Ignore a foreign/pre-contract row. A confirmed replacement self-heals.
    }
  }
  return threads.sort((left, right) =>
    right.updatedAt.localeCompare(left.updatedAt) ||
    right.threadRef.localeCompare(left.threadRef))
}

const decodeMessages = (
  threadRef: string,
  rows: ReadonlyArray<{
    readonly postImageJson: string
    readonly version: number
  }>,
): ReadonlyArray<ConfirmedChatMessage> => {
  const messages: Array<ConfirmedChatMessage> = []
  for (const row of rows) {
    try {
      const message = decodeChatMessageEntity(JSON.parse(row.postImageJson) as unknown)
      if (message.threadId !== threadRef || message.deletedAt !== null) continue
      messages.push({
        messageRef: message.messageId,
        threadRef: message.threadId,
        body: message.body,
        createdAt: message.createdAt,
        updatedAt: message.updatedAt,
        version: Number(row.version),
      })
    } catch {
      // Ignore a foreign/pre-contract row. A confirmed replacement self-heals.
    }
  }
  return messages.sort((left, right) =>
    left.createdAt.localeCompare(right.createdAt) ||
    left.messageRef.localeCompare(right.messageRef))
}

export const createKhalaSyncConversation = (input: Readonly<{
  ownerUserId: string
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
  mutators: ChatClientMutators
}>): KhalaSyncConversation => {
  const personal = personalScope(input.ownerUserId)
  const status = (scope: ReturnType<typeof personalScope>): KhalaSyncConversationStatus => {
    const state = input.session.state(scope)
    return {
      phase: state.phase,
      cursor: cursorFromState(state),
      pendingMutationCount: input.session.pending().length,
    }
  }

  return {
    personalStatus: () => status(personal),
    threadStatus: threadRef => status(threadScope(threadRef)),
    listConfirmedThreads: () => Effect.map(
      input.store.readEntities(personal, CHAT_THREAD_ENTITY_TYPE),
      rows => decodeThreads(input.ownerUserId, rows),
    ),
    openThread: threadRef => input.session.subscribe(threadScope(threadRef)),
    listConfirmedMessages: threadRef => Effect.map(
      input.store.readEntities(threadScope(threadRef), CHAT_MESSAGE_ENTITY_TYPE),
      rows => decodeMessages(threadRef, rows),
    ),
    createThread: args => input.session.mutate(input.mutators.createThread, args),
    appendMessage: args => input.session.mutate(input.mutators.appendMessage, args),
  }
}
