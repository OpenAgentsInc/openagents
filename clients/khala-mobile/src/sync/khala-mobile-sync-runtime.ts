import { createCollection, type Collection } from "@tanstack/db"
import {
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  ClientGroupId,
  ClientId,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  personalScope,
  SyncSchemaVersion,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
  type MutationEnvelope,
  type MutationResult
} from "@openagentsinc/khala-sync"
import {
  createHttpKhalaSyncTransport,
  createKhalaSyncSession,
  createOverlay,
  type ClientMutator,
  type KhalaSyncOverlay,
  type KhalaSyncSession,
  type KhalaSyncSessionOptions,
  type KhalaSyncTransport,
  type WebSocketLike
} from "@openagentsinc/khala-sync-client"
import {
  chatAppendMessageClientMutator,
  chatCreateThreadClientMutator,
  chatMessageKhalaSyncCollectionOptions,
  chatMessagesForTranscript,
  chatRenameThreadClientMutator,
  chatThreadKhalaSyncCollectionOptions,
  chatThreadsForSidebar,
  createKhalaSyncMutationTracker,
  type ChatAppendMessageArgs,
  type ChatCreateThreadArgs,
  type ChatRenameThreadArgs,
  type KhalaSyncCollectionUtils,
  type KhalaSyncMutationTracker
} from "@openagentsinc/khala-sync-db-collection"
import { Effect } from "effect"

import { loadKhalaApiKey } from "../security/keychain"
import {
  openKhalaMobileSyncStore,
  type ExpoSqliteModule,
  type KhalaMobileSyncStore
} from "./expo-db-sqlite-persistence"
import {
  chatBindThreadRepoClientMutator,
  type ChatBindThreadRepoArgs
} from "./khala-thread-repo-binding-core"

const KHALA_MOBILE_SYNC_SCHEMA_VERSION = SyncSchemaVersion.make(1)
const DEFAULT_CHAT_LIMIT = 500

export type KhalaMobileSyncPhase =
  | "idle"
  | "bootstrapping"
  | "catching_up"
  | "live"
  | "must_refetch"
  | "denied"

export type KhalaMobileChatRejection = Readonly<{
  errorCode: string
  messageSafe: string
  mutationId: number
  mutatorName: string
  observedAt: string
  threadId: string | null
}>

export type KhalaMobileChatThreadsState = Readonly<{
  authState: "connected"
  cursor: number | null
  error?: string
  ok: boolean
  ownerUserId: string
  pendingMutations: number
  phase: KhalaMobileSyncPhase
  reason: string | null
  rejections: ReadonlyArray<KhalaMobileChatRejection>
  threads: ReadonlyArray<ChatThreadEntity>
}>

export type KhalaMobileChatMessagesState = Readonly<{
  authState: "connected"
  cursor: number | null
  error?: string
  messages: ReadonlyArray<ChatMessageEntity>
  ok: boolean
  ownerUserId: string
  pendingMutations: number
  phase: KhalaMobileSyncPhase
  reason: string | null
  rejections: ReadonlyArray<KhalaMobileChatRejection>
  threadId: string
}>

export type KhalaMobileChatMutationResult = Readonly<{
  ok: boolean
  error?: string
  messageId?: string
  threadId: string
}>

export type KhalaMobileChatRuntime = Readonly<{
  appendMessage: (
    input: Readonly<{ body: string; messageId?: string; threadId: string }>,
  ) => Promise<KhalaMobileChatMutationResult>
  /** MM-B2 (#8472): binds (or clears, when `repo: null`) the repo pinned to
   * a thread. Pushed through the overlay directly (`overlay.mutate`, not a
   * TanStack DB collection) since `chatThreadKhalaSyncCollectionOptions`'s
   * `update` handler currently only supports title changes. KNOWN GAP: the
   * server does not yet recognize `chat.bindThreadRepo` as a mutator, so
   * this mutation applies optimistically to the local durable store/overlay
   * (immediately visible on-device) but is expected to be rejected by the
   * sync session's push loop until a server-side counterpart lands — see
   * `khala-thread-repo-binding-core.ts`'s header comment and the #8472
   * issue comment documenting the exact server contract needed. */
  bindThreadRepo: (
    input: ChatBindThreadRepoArgs,
  ) => Promise<KhalaMobileChatMutationResult>
  chatMessages: (
    input: Readonly<{ limit?: number; threadId: string }>,
  ) => Promise<KhalaMobileChatMessagesState>
  chatThreads: (
    input?: Readonly<{ limit?: number; searchTerm?: string | null }>,
  ) => Promise<KhalaMobileChatThreadsState>
  createThread: (
    input: Readonly<{ threadId: string; title: string }>,
  ) => Promise<KhalaMobileChatMutationResult>
  close: () => Promise<void>
}>

export type KhalaMobileSyncRuntime = KhalaMobileChatRuntime & Readonly<{
  overlay: KhalaSyncOverlay
  session: KhalaSyncSession
  store: KhalaMobileSyncStore
}>

type ChatMutators = Readonly<{
  appendMessage: ClientMutator<ChatAppendMessageArgs>
  bindThreadRepo: ClientMutator<ChatBindThreadRepoArgs>
  createThread: ClientMutator<ChatCreateThreadArgs>
  renameThread: ClientMutator<ChatRenameThreadArgs>
}>

type RuntimeCollections = {
  chatMessageCollections: Map<string, Collection<ChatMessageEntity, string, KhalaSyncCollectionUtils>>
  chatThreadsCollection: Collection<ChatThreadEntity, string, KhalaSyncCollectionUtils> | null
}

export const makeKhalaMobileMessageId = (
  input: {
    readonly now?: () => Date
    readonly randomId?: () => string
  } = {},
): string => {
  const now = input.now?.() ?? new Date()
  const random = input.randomId?.() ?? fallbackRandomId()
  return `chat-message.mobile.${now.getTime()}.${random}`
}

const fallbackRandomId = (): string => {
  const maybeCrypto = globalThis.crypto as
    | { randomUUID?: () => string }
    | undefined
  return maybeCrypto?.randomUUID?.() ?? Math.random().toString(36).slice(2)
}

const runEffect = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

const errorText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const phaseOf = (
  state: ReturnType<KhalaSyncSession["state"]>,
): {
  readonly cursor: number | null
  readonly phase: KhalaMobileSyncPhase
  readonly reason: string | null
} => {
  switch (state.phase) {
    case "idle":
      return { cursor: null, phase: "idle", reason: null }
    case "bootstrapping":
      return { cursor: null, phase: "bootstrapping", reason: null }
    case "catching_up":
      return { cursor: state.cursor, phase: "catching_up", reason: null }
    case "live":
      return { cursor: state.cursor, phase: "live", reason: null }
    case "must_refetch":
      return { cursor: null, phase: "must_refetch", reason: state.reason }
    case "denied":
      return { cursor: null, phase: "denied", reason: state.reason }
  }
}

const threadIdFromMutation = (mutation: MutationEnvelope | undefined): string | null => {
  if (mutation === undefined) return null
  try {
    const args: unknown = JSON.parse(mutation.argsJson)
    return typeof args === "object" &&
      args !== null &&
      "threadId" in args &&
      typeof (args as { threadId?: unknown }).threadId === "string"
      ? (args as { threadId: string }).threadId
      : null
  } catch {
    return null
  }
}

const chatRejectionFromResult = (
  result: MutationResult,
  mutation: MutationEnvelope | undefined,
  observedAt: string,
): KhalaMobileChatRejection => ({
  errorCode: result.errorCode ?? "rejected",
  messageSafe: result.errorMessageSafe ?? "mutation rejected by the server",
  mutationId: Number(result.mutationId),
  mutatorName: mutation === undefined ? "unknown" : String(mutation.name),
  observedAt,
  threadId: threadIdFromMutation(mutation)
})

const chatUnavailable = (
  ownerUserId: string,
  error: string,
  rejections: ReadonlyArray<KhalaMobileChatRejection>,
): KhalaMobileChatThreadsState => ({
  authState: "connected",
  cursor: null,
  error,
  ok: false,
  ownerUserId,
  pendingMutations: 0,
  phase: "idle",
  reason: null,
  rejections,
  threads: []
})

const chatMessagesUnavailable = (
  ownerUserId: string,
  threadId: string,
  error: string,
  rejections: ReadonlyArray<KhalaMobileChatRejection>,
): KhalaMobileChatMessagesState => ({
  authState: "connected",
  cursor: null,
  error,
  messages: [],
  ok: false,
  ownerUserId,
  pendingMutations: 0,
  phase: "idle",
  reason: null,
  rejections,
  threadId
})

export const createKhalaMobileChatRuntime = (
  input: Readonly<{
    mutationTracker: KhalaSyncMutationTracker
    mutators: ChatMutators
    now?: () => Date
    overlay: KhalaSyncOverlay
    ownerUserId: string
    rejections?: () => ReadonlyArray<KhalaMobileChatRejection>
    session: KhalaSyncSession
    sleep?: (ms: number) => Promise<void>
    store: KhalaMobileSyncStore
  }>,
): KhalaMobileChatRuntime => {
  const collections: RuntimeCollections = {
    chatMessageCollections: new Map(),
    chatThreadsCollection: null
  }
  const rejections = () => input.rejections?.() ?? []

  const confirmedChatThreads = async (
    request: Readonly<{ limit?: number; searchTerm?: string | null }> | undefined,
  ): Promise<ReadonlyArray<ChatThreadEntity>> => {
    const limit = request?.limit === undefined
      ? 50
      : Math.max(0, Math.min(200, Math.trunc(request.limit)))
    const entities = await runEffect(
      input.store.readEntities(personalScope(input.ownerUserId), CHAT_THREAD_ENTITY_TYPE)
    )
    return chatThreadsForSidebar(
      entities.map(entity =>
        decodeChatThreadEntity(JSON.parse(entity.postImageJson) as unknown)
      ),
      {
        ...(request?.searchTerm === undefined
          ? {}
          : { searchTerm: request.searchTerm })
      }
    ).slice(0, limit)
  }

  const confirmedChatMessages = async (
    request: Readonly<{ limit?: number; threadId: string }>,
  ): Promise<ReadonlyArray<ChatMessageEntity>> => {
    const limit = request.limit === undefined
      ? DEFAULT_CHAT_LIMIT
      : Math.max(0, Math.min(2_000, Math.trunc(request.limit)))
    const entities = await runEffect(
      input.store.readEntities(threadScope(request.threadId), CHAT_MESSAGE_ENTITY_TYPE)
    )
    return chatMessagesForTranscript(
      entities.map(entity =>
        decodeChatMessageEntity(JSON.parse(entity.postImageJson) as unknown)
      )
    ).slice(-limit)
  }

  const ensureChatThreadsCollection = () => {
    if (collections.chatThreadsCollection !== null) {
      return collections.chatThreadsCollection
    }
    collections.chatThreadsCollection = createCollection(
      chatThreadKhalaSyncCollectionOptions({
        awaitMutationPollIntervalMs: 0,
        awaitMutationTimeoutMs: 5_000,
        createThreadMutator: input.mutators.createThread,
        id: "khala-mobile-chat-threads",
        mutationTracker: input.mutationTracker,
        onError: () => undefined,
        overlay: input.overlay,
        ownerUserId: input.ownerUserId,
        renameThreadMutator: input.mutators.renameThread,
        scope: personalScope(input.ownerUserId),
        session: input.session,
        ...(input.sleep === undefined ? {} : { sleep: input.sleep }),
        startSync: true
      })
    )
    return collections.chatThreadsCollection
  }

  const ensureChatMessageCollection = (
    threadId: string,
  ): Collection<ChatMessageEntity, string, KhalaSyncCollectionUtils> => {
    const existing = collections.chatMessageCollections.get(threadId)
    if (existing !== undefined) return existing
    const collection = createCollection(
      chatMessageKhalaSyncCollectionOptions({
        appendMessageMutator: input.mutators.appendMessage,
        awaitMutationPollIntervalMs: 0,
        awaitMutationTimeoutMs: 5_000,
        id: `khala-mobile-chat-messages-${threadId}`,
        mutationTracker: input.mutationTracker,
        onError: () => undefined,
        overlay: input.overlay,
        scope: threadScope(threadId),
        session: input.session,
        ...(input.sleep === undefined ? {} : { sleep: input.sleep }),
        startSync: true
      })
    )
    collections.chatMessageCollections.set(threadId, collection)
    return collection
  }

  return {
    bindThreadRepo: async request => {
      try {
        await runEffect(input.overlay.mutate(input.mutators.bindThreadRepo, request))
        return { ok: true, threadId: request.threadId }
      } catch (error) {
        return { error: errorText(error), ok: false, threadId: request.threadId }
      }
    },
    appendMessage: async request => {
      const messageId = request.messageId ?? makeKhalaMobileMessageId({ now: input.now })
      const collection = ensureChatMessageCollection(request.threadId)
      try {
        await collection.preload()
        const nowIso = (input.now?.() ?? new Date()).toISOString()
        const tx = collection.insert(
          decodeChatMessageEntity({
            authorUserId: input.ownerUserId,
            body: request.body,
            createdAt: nowIso,
            deletedAt: null,
            messageId,
            threadId: request.threadId,
            updatedAt: nowIso
          })
        )
        await tx.isPersisted.promise
        return { ok: true, messageId, threadId: request.threadId }
      } catch (error) {
        return {
          error: errorText(error),
          messageId,
          ok: false,
          threadId: request.threadId
        }
      }
    },
    chatMessages: async request => {
      const collection = ensureChatMessageCollection(request.threadId)
      try {
        await collection.preload()
        const scope = threadScope(request.threadId)
        const { cursor, phase, reason } = phaseOf(input.session.state(scope))
        return {
          authState: "connected",
          cursor,
          messages: await confirmedChatMessages(request),
          ok: true,
          ownerUserId: input.ownerUserId,
          pendingMutations: input.overlay.pending().length,
          phase,
          reason,
          rejections: rejections(),
          threadId: request.threadId
        }
      } catch (error) {
        return chatMessagesUnavailable(
          input.ownerUserId,
          request.threadId,
          errorText(error),
          rejections()
        )
      }
    },
    chatThreads: async request => {
      const collection = ensureChatThreadsCollection()
      try {
        await collection.preload()
        const scope = personalScope(input.ownerUserId)
        const { cursor, phase, reason } = phaseOf(input.session.state(scope))
        return {
          authState: "connected",
          cursor,
          ok: true,
          ownerUserId: input.ownerUserId,
          pendingMutations: input.overlay.pending().length,
          phase,
          reason,
          rejections: rejections(),
          threads: await confirmedChatThreads(request)
        }
      } catch (error) {
        return chatUnavailable(input.ownerUserId, errorText(error), rejections())
      }
    },
    close: async () => undefined,
    createThread: async request => {
      const collection = ensureChatThreadsCollection()
      try {
        await collection.preload()
        const nowIso = (input.now?.() ?? new Date()).toISOString()
        const tx = collection.insert(
          decodeChatThreadEntity({
            createdAt: nowIso,
            lastMessageAt: null,
            messageCount: 0,
            ownerUserId: input.ownerUserId,
            status: "active",
            threadId: request.threadId,
            title: request.title.trim(),
            updatedAt: nowIso
          })
        )
        await tx.isPersisted.promise
        return { ok: true, threadId: request.threadId }
      } catch (error) {
        return { error: errorText(error), ok: false, threadId: request.threadId }
      }
    }
  }
}

export type OpenKhalaMobileSyncRuntimeResult =
  | Readonly<{
      ok: true
      runtime: KhalaMobileSyncRuntime
    }>
  | Readonly<{
      ok: false
      authState: "missing" | "connected"
      error: string
    }>

export const openKhalaMobileSyncRuntime = async (
  input: Readonly<{
    clientGroupId?: string
    databaseName?: string
    fetch?: typeof globalThis.fetch
    now?: () => Date
    ownerUserId: string
    randomId?: () => string
    secureTokenLoader?: () => Promise<string | null>
    sleep?: KhalaSyncSessionOptions["sleep"]
    sqliteLoader?: () => Promise<ExpoSqliteModule>
    syncBaseUrl: string
    transport?: (config: {
      readonly authToken: () => string
      readonly baseUrl: string
    }) => KhalaSyncTransport
    webSocket?: new (url: string) => WebSocketLike
  }>,
): Promise<OpenKhalaMobileSyncRuntimeResult> => {
  const token = await (input.secureTokenLoader ?? loadKhalaApiKey)()
  if (token === null) {
    return { authState: "missing", error: "missing_openagents_auth", ok: false }
  }

  const store = await openKhalaMobileSyncStore({
    ...(input.databaseName === undefined ? {} : { databaseName: input.databaseName }),
    ...(input.sqliteLoader === undefined ? {} : { sqliteLoader: input.sqliteLoader })
  })
  const persisted = await runEffect(store.identity())
  const randomId = input.randomId ?? fallbackRandomId
  const clientGroupId =
    persisted?.clientGroupId ??
    ClientGroupId.make(input.clientGroupId ?? `khala-mobile.${randomId()}`)
  const clientId = persisted?.clientId ?? ClientId.make(randomId())
  const mutators = {
    appendMessage: chatAppendMessageClientMutator({
      ownerUserId: input.ownerUserId
    }),
    bindThreadRepo: chatBindThreadRepoClientMutator({
      ownerUserId: input.ownerUserId
    }),
    createThread: chatCreateThreadClientMutator({
      ownerUserId: input.ownerUserId
    }),
    renameThread: chatRenameThreadClientMutator({
      ownerUserId: input.ownerUserId
    })
  }
  const mutationTracker = createKhalaSyncMutationTracker()
  const chatRejections: Array<KhalaMobileChatRejection> = []
  const overlay: KhalaSyncOverlay = await runEffect(createOverlay(store, [
    mutators.appendMessage,
    mutators.bindThreadRepo,
    mutators.createThread,
    mutators.renameThread
  ]))
  const transportConfig = {
    authToken: () => token,
    baseUrl: input.syncBaseUrl
  }
  const transport =
    input.transport?.(transportConfig) ??
    createHttpKhalaSyncTransport(transportConfig, {
      ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
      ...(input.webSocket === undefined ? {} : { webSocket: input.webSocket })
    })
  const session = createKhalaSyncSession(
    {
      authToken: transportConfig.authToken,
      baseUrl: input.syncBaseUrl,
      clientGroupId,
      clientId,
      schemaVersion: KHALA_MOBILE_SYNC_SCHEMA_VERSION
    },
    store,
    overlay,
    transport,
    {
      ...(input.sleep === undefined ? {} : { sleep: input.sleep }),
      onRejection: (result, mutation) => {
        mutationTracker.onRejection(result, mutation)
        chatRejections.unshift(
          chatRejectionFromResult(
            result,
            mutation,
            (input.now?.() ?? new Date()).toISOString()
          )
        )
        chatRejections.splice(20)
      }
    }
  )
  const chatRuntime = createKhalaMobileChatRuntime({
    mutationTracker,
    mutators,
    ...(input.now === undefined ? {} : { now: input.now }),
    overlay,
    ownerUserId: input.ownerUserId,
    rejections: () => chatRejections,
    session,
    ...(input.sleep === undefined ? {} : { sleep: input.sleep }),
    store
  })

  return {
    ok: true,
    runtime: {
      ...chatRuntime,
      close: async () => {
        await runEffect(session.close())
        await runEffect(store.close())
      },
      overlay,
      session,
      store
    }
  }
}
