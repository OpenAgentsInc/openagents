import type {
  ChangeMessageOrDeleteKeyMessage,
  CollectionConfig,
  LoadSubsetOptions,
  PendingMutation,
  SyncMetadataApi,
  TransactionWithMutations,
  UtilsRecord,
} from "@tanstack/db"
import type { KhalaFleetIntent } from "@openagentsinc/khala-fleet-intents"
import {
  canonicalJson,
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  decodeFleetApprovalEntity,
  decodeFleetRunEntity,
  decodeFleetSteerEntity,
  decodeFleetWorkerEntity,
  decodeRuntimeEventEntity,
  decodeRuntimeTurnEntity,
  encodeChatMessageEntity,
  encodeChatThreadEntity,
  encodeFleetApprovalEntity,
  encodeFleetRunEntity,
  encodeFleetSteerEntity,
  FLEET_APPROVAL_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FLEET_STEER_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  fleetRunScope,
  personalScope,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
  type FleetApprovalEntity,
  type FleetRunEntity,
  type FleetSteerEntity,
  type FleetWorkerEntity,
  MutationId,
  type MutationEnvelope,
  MutationResult,
  MutatorName,
  type RuntimeEventEntity,
  type RuntimeTurnEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import type {
  ClientMutator,
  KhalaSyncOverlay,
  KhalaSyncSession,
  KhalaSyncSessionOptions,
  OverlayEntity,
  OverlayView,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

export type KhalaSyncDbCollectionReason =
  | "khala_sync_db_collection.decode_failed"
  | "khala_sync_db_collection.scope_denied"
  | "khala_sync_db_collection.missing_mutator"
  | "khala_sync_db_collection.mutation_id_unavailable"
  | "khala_sync_db_collection.mutation_rejected"
  | "khala_sync_db_collection.mutation_timeout"
  | "khala_sync_db_collection.mutation_not_synced"
  | "khala_sync_db_collection.session_failure"

export class KhalaSyncDbCollectionError extends Error {
  readonly _tag = "KhalaSyncDbCollectionError"
  override readonly name = "KhalaSyncDbCollectionError"

  constructor(
    readonly reasonRef: KhalaSyncDbCollectionReason,
    readonly details: {
      readonly scope: string
      readonly collection: string
      readonly messageSafe: string
      readonly mutationId?: number
      readonly cause?: unknown
    },
  ) {
    super(details.messageSafe, { cause: details.cause })
  }
}

export type KhalaSyncMutationTracker = Readonly<{
  onRejection: NonNullable<KhalaSyncSessionOptions["onRejection"]>
  getRejection: (
    mutationId: MutationId,
  ) =>
    | {
        readonly result: MutationResult
        readonly mutation?: MutationEnvelope
      }
    | undefined
  subscribe: (listener: () => void) => () => void
}>

export const createKhalaSyncMutationTracker = (): KhalaSyncMutationTracker => {
  const rejections = new Map<
    number,
    {
      readonly result: MutationResult
      readonly mutation?: MutationEnvelope
    }
  >()
  const listeners = new Set<() => void>()

  return {
    onRejection: (result, mutation) => {
      rejections.set(Number(result.mutationId), {
        ...(mutation !== undefined ? { mutation } : {}),
        result,
      })
      for (const listener of [...listeners]) listener()
    },
    getRejection: mutationId => rejections.get(Number(mutationId)),
    subscribe: listener => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}

export type AwaitMutationOptions = Readonly<{
  tracker?: KhalaSyncMutationTracker
  timeoutMs?: number
  pollIntervalMs?: number
  sleep?: (ms: number) => Promise<void>
  now?: () => number
  scope?: string
  collection?: string
}>

const defaultSleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

const mutationStillPending = (
  session: KhalaSyncSession,
  mutationId: MutationId,
): boolean =>
  session.pending().some(pending => pending.mutationId === mutationId)

export const awaitMutation = async (
  session: KhalaSyncSession,
  mutationId: MutationId,
  options: AwaitMutationOptions = {},
): Promise<MutationResult> => {
  const timeoutMs = options.timeoutMs ?? 30_000
  const pollIntervalMs = options.pollIntervalMs ?? 25
  const now = options.now ?? Date.now
  const sleep = options.sleep ?? defaultSleep
  const startedAt = now()

  for (;;) {
    const rejection = options.tracker?.getRejection(mutationId)
    if (rejection !== undefined) {
      throw new KhalaSyncDbCollectionError(
        "khala_sync_db_collection.mutation_rejected",
        {
          collection: options.collection ?? "unknown",
          messageSafe:
            rejection.result.errorMessageSafe ??
            rejection.result.errorCode ??
            "Khala Sync mutation was rejected",
          mutationId: Number(mutationId),
          scope: options.scope ?? "unknown",
        },
      )
    }

    if (!mutationStillPending(session, mutationId)) {
      await sleep(0)
      const lateRejection = options.tracker?.getRejection(mutationId)
      if (lateRejection !== undefined) {
        throw new KhalaSyncDbCollectionError(
          "khala_sync_db_collection.mutation_rejected",
          {
            collection: options.collection ?? "unknown",
            messageSafe:
              lateRejection.result.errorMessageSafe ??
              lateRejection.result.errorCode ??
              "Khala Sync mutation was rejected",
            mutationId: Number(mutationId),
            scope: options.scope ?? "unknown",
          },
        )
      }
      return new MutationResult({
        mutationId,
        status: "applied",
      })
    }

    if (now() - startedAt >= timeoutMs) {
      throw new KhalaSyncDbCollectionError(
        "khala_sync_db_collection.mutation_timeout",
        {
          collection: options.collection ?? "unknown",
          messageSafe: "Timed out waiting for Khala Sync mutation ack",
          mutationId: Number(mutationId),
          scope: options.scope ?? "unknown",
        },
      )
    }

    await sleep(pollIntervalMs)
  }
}

// Heterogeneous mutator registries are intentionally typed at the mapper edge:
// each mapper owns the argument shape for the named mutator it returns.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type KhalaSyncMutatorCommand<Args = any> = Readonly<{
  mutator: ClientMutator<Args>
  args: Args
}>

export type KhalaSyncMutationMapper<
  T extends object,
  TOperation extends "insert" | "update" | "delete",
> = (
  mutation: PendingMutation<T, TOperation>,
) => KhalaSyncMutatorCommand | Promise<KhalaSyncMutatorCommand>

export type KhalaSyncCollectionMutators<T extends object> = Readonly<{
  insert?: KhalaSyncMutationMapper<T, "insert">
  update?: KhalaSyncMutationMapper<T, "update">
  delete?: KhalaSyncMutationMapper<T, "delete">
}>

export type KhalaSyncCollectionUtils = UtilsRecord &
  Readonly<{
    awaitMutation: (
      mutationId: MutationId,
      options?: AwaitMutationOptions,
    ) => Promise<MutationResult>
    loadSubset: (options?: LoadSubsetOptions) => Promise<void>
    getLastError: () => KhalaSyncDbCollectionError | null
    getStatus: () => ReturnType<KhalaSyncSession["state"]>
  }>

export type KhalaSyncCollectionOptions<
  T extends object,
  TKey extends string | number,
> = Readonly<{
  id?: string
  scope: SyncScope
  /**
   * The Khala Sync entity type backing this TanStack collection.
   * Kept as `collection` to match the TS-3 issue API.
   */
  collection: string
  session: KhalaSyncSession
  overlay: KhalaSyncOverlay
  getKey: (item: T) => TKey
  decode?: (entity: OverlayEntity) => T
  entityIdFromKey?: (key: TKey) => string
  mutators?: KhalaSyncCollectionMutators<T>
  mutationTracker?: KhalaSyncMutationTracker
  onError?: (error: KhalaSyncDbCollectionError) => void
  startSync?: boolean
  syncMode?: "eager" | "on-demand"
  awaitMutationTimeoutMs?: number
  awaitMutationPollIntervalMs?: number
  sleep?: (ms: number) => Promise<void>
  now?: () => number
  /**
   * Keep TanStack optimistic state alive until the server-confirmed overlay
   * row reflects the mutation. Enabled by default.
   */
  awaitServerSync?: boolean
}>

type SyncWriters<T extends object, TKey extends string | number> = Readonly<{
  begin: (options?: { immediate?: boolean }) => void
  write: (message: ChangeMessageOrDeleteKeyMessage<T, TKey>) => void
  commit: () => void
  markReady: () => void
  truncate: () => void
  metadata?: SyncMetadataApi<TKey>
}>

const runEffect = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

const defaultDecode = <T extends object>(entity: OverlayEntity): T =>
  JSON.parse(entity.postImageJson) as T

const valuesMatch = (left: unknown, right: unknown): boolean =>
  canonicalJson(left) === canonicalJson(right)

const hasExpectedPatch = <T extends object>(
  row: T,
  expected: Partial<T>,
): boolean =>
  Object.entries(expected).every(([key, value]) =>
    valuesMatch((row as Record<string, unknown>)[key], value),
  )

export const khalaSyncCollectionOptions = <
  T extends object,
  TKey extends string | number = string,
>(
  options: KhalaSyncCollectionOptions<T, TKey>,
): CollectionConfig<T, TKey, never, KhalaSyncCollectionUtils> => {
  const decode = options.decode ?? defaultDecode<T>
  const entityIdFromKey = options.entityIdFromKey ?? ((key: TKey) => String(key))
  const awaitServerSync = options.awaitServerSync ?? true
  let writers: SyncWriters<T, TKey> | null = null
  let lastError: KhalaSyncDbCollectionError | null = null
  let didMarkReady = false
  let disposed = false
  let publishQueue: Promise<void> = Promise.resolve()
  let mutationQueue: Promise<unknown> = Promise.resolve()

  const recordError = (error: KhalaSyncDbCollectionError): void => {
    lastError = error
    options.onError?.(error)
  }

  const makeError = (
    reasonRef: KhalaSyncDbCollectionReason,
    messageSafe: string,
    extra: {
      readonly mutationId?: number
      readonly cause?: unknown
    } = {},
  ): KhalaSyncDbCollectionError =>
    new KhalaSyncDbCollectionError(reasonRef, {
      collection: options.collection,
      ...(extra.cause !== undefined ? { cause: extra.cause } : {}),
      messageSafe,
      ...(extra.mutationId !== undefined ? { mutationId: extra.mutationId } : {}),
      scope: options.scope,
    })

  const publishSnapshotNow = async (
    reason: "initial" | "overlay" | "state" | "load_subset" | "mutation",
  ): Promise<void> => {
    if (disposed || writers === null) return

    const state = options.session.state(options.scope)
    if (state.phase === "denied") {
      const error = makeError(
        "khala_sync_db_collection.scope_denied",
        "Khala Sync scope access was denied",
      )
      recordError(error)
      return
    }

    if (state.phase === "must_refetch") {
      writers.begin({ immediate: true })
      writers.truncate()
      writers.metadata?.collection.set("khala-sync.phase", state.phase)
      writers.metadata?.collection.set("khala-sync.refetchReason", state.reason)
      writers.commit()
      return
    }

    const view = await runEffect(options.overlay.read(options.scope)).catch(
      cause => {
        throw makeError(
          "khala_sync_db_collection.session_failure",
          "Khala Sync overlay read failed",
          { cause },
        )
      },
    )

    let rows: ReadonlyArray<T>
    try {
      rows = view.list(options.collection).map(entity => decode(entity))
    } catch (cause) {
      throw makeError(
        "khala_sync_db_collection.decode_failed",
        "Khala Sync entity post-image could not be decoded",
        { cause },
      )
    }

    writers.begin({ immediate: reason === "mutation" })
    writers.truncate()
    writers.metadata?.collection.set("khala-sync.scope", options.scope)
    writers.metadata?.collection.set("khala-sync.collection", options.collection)
    writers.metadata?.collection.set("khala-sync.phase", state.phase)
    if ("cursor" in state) {
      writers.metadata?.collection.set("khala-sync.cursor", state.cursor)
    }
    for (const row of rows) {
      writers.write({
        type: "insert",
        value: row,
      })
    }
    writers.commit()

    if (!didMarkReady && state.phase === "live") {
      didMarkReady = true
      writers.markReady()
    }
  }

  const schedulePublish = (
    reason: "initial" | "overlay" | "state" | "load_subset" | "mutation",
  ): Promise<void> => {
    publishQueue = publishQueue.then(
      () => publishSnapshotNow(reason),
      () => publishSnapshotNow(reason),
    )
    publishQueue = publishQueue.catch(error => {
      if (error instanceof KhalaSyncDbCollectionError) {
        recordError(error)
      } else {
        recordError(
          makeError(
            "khala_sync_db_collection.session_failure",
            "Khala Sync collection publish failed",
            { cause: error },
          ),
        )
      }
    })
    return publishQueue
  }

  const waitForLive = async (): Promise<void> => {
    const timeoutMs = options.awaitMutationTimeoutMs ?? 30_000
    const pollIntervalMs = options.awaitMutationPollIntervalMs ?? 25
    const sleep = options.sleep ?? defaultSleep
    const now = options.now ?? Date.now
    const startedAt = now()

    for (;;) {
      const state = options.session.state(options.scope)
      if (state.phase === "live") return
      if (state.phase === "denied") {
        throw makeError(
          "khala_sync_db_collection.scope_denied",
          "Khala Sync scope access was denied",
        )
      }
      if (now() - startedAt >= timeoutMs) {
        throw makeError(
          "khala_sync_db_collection.mutation_timeout",
          "Timed out waiting for Khala Sync scope to become live",
        )
      }
      await sleep(pollIntervalMs)
    }
  }

  const loadSubset = async (_subsetOptions: LoadSubsetOptions = {}) => {
    await runEffect(options.session.subscribe(options.scope)).catch(cause => {
      throw makeError(
        "khala_sync_db_collection.session_failure",
        "Khala Sync scope subscription failed",
        { cause },
      )
    })
    await waitForLive()
    await schedulePublish("load_subset")
  }

  const awaitMutationForCollection = (
    mutationId: MutationId,
    awaitOptions: AwaitMutationOptions = {},
  ): Promise<MutationResult> => {
    const baseOptions: AwaitMutationOptions = {
      collection: options.collection,
      scope: options.scope,
      ...(options.now !== undefined ? { now: options.now } : {}),
      ...(options.awaitMutationPollIntervalMs !== undefined
        ? { pollIntervalMs: options.awaitMutationPollIntervalMs }
        : {}),
      ...(options.sleep !== undefined ? { sleep: options.sleep } : {}),
      ...(options.awaitMutationTimeoutMs !== undefined
        ? { timeoutMs: options.awaitMutationTimeoutMs }
        : {}),
      ...(options.mutationTracker !== undefined
        ? { tracker: options.mutationTracker }
        : {}),
    }

    return awaitMutation(options.session, mutationId, {
      ...baseOptions,
      ...awaitOptions,
    })
  }

  const waitForMutationRow = async (
    mutation: PendingMutation<T>,
  ): Promise<void> => {
    if (!awaitServerSync) return

    const timeoutMs = options.awaitMutationTimeoutMs ?? 30_000
    const pollIntervalMs = options.awaitMutationPollIntervalMs ?? 25
    const sleep = options.sleep ?? defaultSleep
    const now = options.now ?? Date.now
    const startedAt = now()
    const entityId = entityIdFromKey(mutation.key as TKey)

    for (;;) {
      const view: OverlayView = await runEffect(options.overlay.read(options.scope))
      const postImageJson = view.get(options.collection, entityId)
      const synced =
        mutation.type === "delete"
          ? postImageJson === undefined
          : postImageJson !== undefined &&
            hasExpectedPatch(
              decode({
                entityId,
                entityType: options.collection,
                postImageJson,
              }),
              mutation.type === "insert"
                ? (mutation.modified as Partial<T>)
                : (mutation.changes as Partial<T>),
            )

      if (synced) return

      if (now() - startedAt >= timeoutMs) {
        throw makeError(
          "khala_sync_db_collection.mutation_not_synced",
          "Timed out waiting for Khala Sync mutation to appear in the synced row",
          { mutationId: Number(mutation.mutationId) },
        )
      }

      await sleep(pollIntervalMs)
    }
  }

  const dispatchMutation = async <
    TOperation extends "insert" | "update" | "delete",
  >(
    mutation: PendingMutation<T, TOperation>,
    mapper: KhalaSyncMutationMapper<T, TOperation> | undefined,
  ): Promise<void> => {
    if (mapper === undefined) {
      throw makeError(
        "khala_sync_db_collection.missing_mutator",
        `No Khala Sync mutator mapper is configured for ${mutation.type}`,
      )
    }

    const command = await mapper(mutation)
    const mutationId = await runEffect(options.session.mutate(command.mutator, command.args)).catch(
      cause => {
        throw makeError(
          "khala_sync_db_collection.session_failure",
          "Khala Sync mutation enqueue failed",
          { cause },
        )
      },
    )
    await awaitMutationForCollection(mutationId)
    await waitForMutationRow(mutation)
    await schedulePublish("mutation")
  }

  const dispatchTransaction = async <
    TOperation extends "insert" | "update" | "delete",
  >(
    transaction: TransactionWithMutations<T, TOperation>,
    mapper: KhalaSyncMutationMapper<T, TOperation> | undefined,
  ): Promise<void> => {
    for (const mutation of transaction.mutations) {
      mutationQueue = mutationQueue.then(
        () => dispatchMutation(mutation, mapper),
        () => dispatchMutation(mutation, mapper),
      )
      await mutationQueue
    }
  }

  const syncConfig: CollectionConfig<
    T,
    TKey,
    never,
    KhalaSyncCollectionUtils
  >["sync"] = {
    rowUpdateMode: "full",
    sync: params => {
      writers = params

      const unsubscribeState = options.session.subscribeState((scope, state) => {
        if (scope !== options.scope) return
        if (state.phase === "denied") {
          recordError(
            makeError(
              "khala_sync_db_collection.scope_denied",
              "Khala Sync scope access was denied",
            ),
          )
        }
        void schedulePublish("state")
      })
      const unsubscribeOverlay = options.overlay.subscribe(scope => {
        if (scope !== options.scope) return
        void schedulePublish("overlay")
      })

      void runEffect(options.session.subscribe(options.scope))
        .then(() => schedulePublish("initial"))
        .catch(cause =>
          recordError(
            makeError(
              "khala_sync_db_collection.session_failure",
              "Khala Sync scope subscription failed",
              { cause },
            ),
          ),
        )

      return {
        cleanup: () => {
          disposed = true
          unsubscribeState()
          unsubscribeOverlay()
          void runEffect(options.session.unsubscribe(options.scope))
        },
        loadSubset,
        unloadSubset: () => undefined,
      }
    },
  }

  return {
    ...(options.id !== undefined ? { id: options.id } : {}),
    getKey: options.getKey,
    onDelete: async ({ transaction }) =>
      dispatchTransaction(transaction, options.mutators?.delete),
    onInsert: async ({ transaction }) =>
      dispatchTransaction(transaction, options.mutators?.insert),
    onUpdate: async ({ transaction }) =>
      dispatchTransaction(transaction, options.mutators?.update),
    ...(options.startSync !== undefined ? { startSync: options.startSync } : {}),
    sync: syncConfig,
    syncMode: options.syncMode ?? "eager",
    utils: {
      awaitMutation: awaitMutationForCollection,
      getLastError: () => lastError,
      getStatus: () => options.session.state(options.scope),
      loadSubset,
    },
  }
}

export const FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME = "fleet.setDesiredSlots"
export const CHAT_CREATE_THREAD_MUTATOR_NAME = "chat.createThread"
export const CHAT_APPEND_MESSAGE_MUTATOR_NAME = "chat.appendMessage"
export const CHAT_RENAME_THREAD_MUTATOR_NAME = "chat.renameThread"

export type FleetSetDesiredSlotsArgs = Readonly<{
  runId: string
  desiredSlots: number
}>

export type ChatCreateThreadArgs = Readonly<{
  threadId: string
  title: string
}>

export type ChatAppendMessageArgs = Readonly<{
  threadId: string
  messageId: string
  body: string
}>

export type ChatRenameThreadArgs = Readonly<{
  threadId: string
  title: string
}>

export type ChatClientMutatorOptions = Readonly<{
  ownerUserId: string
  now?: () => string
}>

const defaultNowIso = (): string => new Date().toISOString()

const normalizeChatTitle = (title: string): string => title.trim()

const chatTimestampMs = (value: string | null): number => {
  if (value === null) return 0
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

export const compareChatThreadsForSidebar = (
  left: ChatThreadEntity,
  right: ChatThreadEntity,
): number => {
  const recency =
    chatTimestampMs(right.updatedAt) - chatTimestampMs(left.updatedAt)
  if (recency !== 0) return recency
  return right.threadId.localeCompare(left.threadId)
}

/**
 * Defensive hygiene against duplicate sidebar rows: collapse any entities
 * that share a `threadId` down to one, keeping the most recently updated
 * copy. The overlay/store below this function keys rows by `entityId`, not
 * by `threadId`, so a caller that ever inserts two distinct entity ids
 * carrying the same `threadId` (e.g. a retried mutation that didn't reuse
 * the original entity id) would otherwise surface as two indistinguishable
 * rows with the same title in the desktop/mobile sidebar.
 */
const dedupeChatThreadsByThreadId = (
  threads: Iterable<ChatThreadEntity>,
): Array<ChatThreadEntity> => {
  const byThreadId = new Map<string, ChatThreadEntity>()
  for (const thread of threads) {
    const existing = byThreadId.get(thread.threadId)
    if (existing === undefined || chatTimestampMs(thread.updatedAt) >= chatTimestampMs(existing.updatedAt)) {
      byThreadId.set(thread.threadId, thread)
    }
  }
  return [...byThreadId.values()]
}

export const chatThreadsForSidebar = (
  threads: Iterable<ChatThreadEntity>,
  options: { readonly searchTerm?: string | null } = {},
): Array<ChatThreadEntity> => {
  const searchTerm = options.searchTerm?.trim().toLowerCase() ?? ""
  return dedupeChatThreadsByThreadId(threads)
    .filter(thread => {
      if (searchTerm.length === 0) return true
      return (
        thread.title.toLowerCase().includes(searchTerm) ||
        thread.threadId.toLowerCase().includes(searchTerm)
      )
    })
    .sort(compareChatThreadsForSidebar)
}

const chatMessageTimestampMs = (value: string): number => {
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

export const compareChatMessagesForTranscript = (
  left: ChatMessageEntity,
  right: ChatMessageEntity,
): number => {
  const created = chatMessageTimestampMs(left.createdAt) - chatMessageTimestampMs(right.createdAt)
  if (created !== 0) return created
  return left.messageId.localeCompare(right.messageId)
}

export const chatMessagesForTranscript = (
  messages: Iterable<ChatMessageEntity>,
): Array<ChatMessageEntity> =>
  [...messages]
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

const chatThreadOverlayEffects = (
  entity: ChatThreadEntity,
): ReturnType<ClientMutator<ChatCreateThreadArgs>["apply"]> => [
  {
    entityId: entity.threadId,
    entityType: CHAT_THREAD_ENTITY_TYPE,
    kind: "upsert",
    postImageJson: canonicalJson(encodeChatThreadEntity(entity)),
    scope: personalScope(entity.ownerUserId),
  },
  {
    entityId: entity.threadId,
    entityType: CHAT_THREAD_ENTITY_TYPE,
    kind: "upsert",
    postImageJson: canonicalJson(encodeChatThreadEntity(entity)),
    scope: threadScope(entity.threadId),
  },
]

const chatMessageOverlayEffect = (
  entity: ChatMessageEntity,
): ReturnType<ClientMutator<ChatAppendMessageArgs>["apply"]>[number] => ({
  entityId: entity.messageId,
  entityType: CHAT_MESSAGE_ENTITY_TYPE,
  kind: "upsert",
  postImageJson: canonicalJson(encodeChatMessageEntity(entity)),
  scope: threadScope(entity.threadId),
})

export const chatCreateThreadClientMutator = (
  options: ChatClientMutatorOptions,
): ClientMutator<ChatCreateThreadArgs> => ({
  apply: args => chatThreadOverlayEffects(baselineChatThread(args, options)),
  name: MutatorName.make(CHAT_CREATE_THREAD_MUTATOR_NAME),
})

export const chatRenameThreadClientMutator = (
  options: ChatClientMutatorOptions,
): ClientMutator<ChatRenameThreadArgs> => ({
  apply: (args, view) => {
    const scope = personalScope(options.ownerUserId)
    const currentJson = view.get(scope, CHAT_THREAD_ENTITY_TYPE, args.threadId)
    const current =
      currentJson === undefined
        ? baselineChatThread(
            { threadId: args.threadId, title: args.title },
            options,
          )
        : decodeChatThreadEntity(JSON.parse(currentJson) as unknown)
    const now = (options.now ?? defaultNowIso)()
    return chatThreadOverlayEffects(
      decodeChatThreadEntity({
        ...current,
        title: normalizeChatTitle(args.title),
        updatedAt: now,
      }),
    )
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
      authorUserId: options.ownerUserId,
      body: args.body,
      createdAt: now,
      deletedAt: null,
      messageId: args.messageId,
      threadId: args.threadId,
      updatedAt: now,
    })
    return [
      ...chatThreadOverlayEffects(
        decodeChatThreadEntity({
          ...current,
          lastMessageAt: now,
          messageCount: current.messageCount + 1,
          updatedAt: now,
        }),
      ),
      chatMessageOverlayEffect(message),
    ]
  },
  name: MutatorName.make(CHAT_APPEND_MESSAGE_MUTATOR_NAME),
})

const baselineFleetRun = (
  runId: string,
  desiredSlots: number,
): FleetRunEntity =>
  decodeFleetRunEntity({
    counters: {
      activeAssignments: 0,
      blockedAssignments: 0,
      completedAssignments: 0,
      failedAssignments: 0,
      workUnitsTotal: 0,
    },
    desiredSlots,
    runId,
    startedAt: null,
    status: "draft",
    updatedAt: "1970-01-01T00:00:00.000Z",
    workerKind: "auto",
  })

export const fleetSetDesiredSlotsClientMutator: ClientMutator<FleetSetDesiredSlotsArgs> =
  {
    apply: (args, view) => {
      const scope = fleetRunScope(args.runId)
      const currentJson = view.get(scope, FLEET_RUN_ENTITY_TYPE, args.runId)
      const current =
        currentJson === undefined
          ? baselineFleetRun(args.runId, args.desiredSlots)
          : decodeFleetRunEntity(JSON.parse(currentJson) as unknown)
      const next = decodeFleetRunEntity({
        ...current,
        counters: { ...current.counters },
        desiredSlots: args.desiredSlots,
      })

      return [
        {
          entityId: args.runId,
          entityType: FLEET_RUN_ENTITY_TYPE,
          kind: "upsert",
          postImageJson: canonicalJson(encodeFleetRunEntity(next)),
          scope,
        },
      ]
    },
    name: MutatorName.make(FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME),
  }

export type FleetRunCollectionOptions = Omit<
  KhalaSyncCollectionOptions<FleetRunEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
> &
  Readonly<{
    setDesiredSlotsMutator?: ClientMutator<FleetSetDesiredSlotsArgs>
  }>

export const fleetRunKhalaSyncCollectionOptions = (
  options: FleetRunCollectionOptions,
): CollectionConfig<FleetRunEntity, string, never, KhalaSyncCollectionUtils> => {
  const setDesiredSlotsMutator =
    options.setDesiredSlotsMutator ?? fleetSetDesiredSlotsClientMutator

  return khalaSyncCollectionOptions<FleetRunEntity, string>({
    ...options,
    collection: FLEET_RUN_ENTITY_TYPE,
    decode: entity =>
      decodeFleetRunEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.runId,
    mutators: {
      update: mutation => {
        const desiredSlots = (mutation.changes as Partial<FleetRunEntity>)
          .desiredSlots
        if (typeof desiredSlots !== "number") {
          throw new KhalaSyncDbCollectionError(
            "khala_sync_db_collection.missing_mutator",
            {
              collection: FLEET_RUN_ENTITY_TYPE,
              messageSafe:
                "fleet_run updates currently require a desiredSlots change",
              scope: options.scope,
            },
          )
        }
        return {
          args: {
            desiredSlots,
            runId: mutation.key,
          },
          mutator: setDesiredSlotsMutator,
        }
      },
    },
  })
}

// ---------------------------------------------------------------------------
// MH-6 (#8585): fleet steering — the three MH-0 typed intents as client
// mutators, plus read-only worker/approval/steer collections for the mobile
// peek. The mutator NAMES match the server mutators exactly; the args ARE the
// `KhalaFleetIntent` value (one vocabulary, no bridge). `apply` produces the
// optimistic projected post-image so the phone reflects the steer instantly;
// the authoritative behavior change happens server/desktop-side.
// ---------------------------------------------------------------------------

export const FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME = "fleet.dispatchRunControl"
export const FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME =
  "fleet.dispatchApprovalDecision"
export const FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME =
  "fleet.dispatchSteerMessage"

const runStatusForAction = (
  action: "pause" | "resume" | "drain" | "stop",
): FleetRunEntity["status"] => {
  switch (action) {
    case "pause":
      return "paused"
    case "resume":
      return "running"
    case "drain":
      return "draining"
    case "stop":
      return "stopped"
  }
}

export const fleetDispatchRunControlClientMutator: ClientMutator<KhalaFleetIntent> =
  {
    apply: (intent, view) => {
      if (intent.kind !== "fleet_run_control" || intent.runRef === undefined) {
        return []
      }
      const scope = fleetRunScope(intent.runRef)
      const currentJson = view.get(scope, FLEET_RUN_ENTITY_TYPE, intent.runRef)
      if (currentJson === undefined) return []
      const current = decodeFleetRunEntity(JSON.parse(currentJson) as unknown)
      const next = decodeFleetRunEntity({
        ...current,
        counters: { ...current.counters },
        ...(intent.action === "stop" ? { desiredSlots: 0 } : {}),
        status: runStatusForAction(intent.action),
        updatedAt: intent.createdAt,
      })
      return [
        {
          entityId: intent.runRef,
          entityType: FLEET_RUN_ENTITY_TYPE,
          kind: "upsert",
          postImageJson: canonicalJson(encodeFleetRunEntity(next)),
          scope,
        },
      ]
    },
    name: MutatorName.make(FLEET_DISPATCH_RUN_CONTROL_MUTATOR_NAME),
  }

export const fleetDispatchApprovalDecisionClientMutator: ClientMutator<KhalaFleetIntent> =
  {
    apply: (intent, view) => {
      if (intent.kind !== "approval_decision" || intent.runRef === undefined) {
        return []
      }
      const scope = fleetRunScope(intent.runRef)
      const currentJson = view.get(
        scope,
        FLEET_APPROVAL_ENTITY_TYPE,
        intent.approvalRef,
      )
      const base =
        currentJson === undefined
          ? { approvalRef: intent.approvalRef }
          : (JSON.parse(currentJson) as Record<string, unknown>)
      const next = decodeFleetApprovalEntity({
        ...base,
        approvalRef: intent.approvalRef,
        decidedAt: intent.createdAt,
        status: intent.decision === "allow" ? "allowed" : "denied",
        updatedAt: intent.createdAt,
      })
      return [
        {
          entityId: intent.approvalRef,
          entityType: FLEET_APPROVAL_ENTITY_TYPE,
          kind: "upsert",
          postImageJson: canonicalJson(encodeFleetApprovalEntity(next)),
          scope,
        },
      ]
    },
    name: MutatorName.make(FLEET_DISPATCH_APPROVAL_DECISION_MUTATOR_NAME),
  }

export const fleetDispatchSteerMessageClientMutator: ClientMutator<KhalaFleetIntent> =
  {
    apply: (intent) => {
      if (intent.kind !== "steer_message" || intent.runRef === undefined) {
        return []
      }
      const scope = fleetRunScope(intent.runRef)
      const bodyCarrier =
        intent.body !== undefined
          ? "inline"
          : intent.bodyRef !== undefined
            ? "ref"
            : "none"
      const next = decodeFleetSteerEntity({
        bodyCarrier,
        createdAt: intent.createdAt,
        steerRef: intent.intentId,
        ...(intent.targetRef === undefined
          ? {}
          : { targetRef: intent.targetRef }),
        updatedAt: intent.createdAt,
      })
      return [
        {
          entityId: intent.intentId,
          entityType: FLEET_STEER_ENTITY_TYPE,
          kind: "upsert",
          postImageJson: canonicalJson(encodeFleetSteerEntity(next)),
          scope,
        },
      ]
    },
    name: MutatorName.make(FLEET_DISPATCH_STEER_MESSAGE_MUTATOR_NAME),
  }

/**
 * Read-only `fleet_worker` collection — the per-harness worker cards the
 * mobile peek renders. Workers are projected by the desktop authority; the
 * phone never writes them locally.
 */
export type FleetWorkerCollectionOptions = Omit<
  KhalaSyncCollectionOptions<FleetWorkerEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
>

export const fleetWorkerKhalaSyncCollectionOptions = (
  options: FleetWorkerCollectionOptions,
): CollectionConfig<FleetWorkerEntity, string, never, KhalaSyncCollectionUtils> =>
  khalaSyncCollectionOptions<FleetWorkerEntity, string>({
    ...options,
    awaitServerSync: options.awaitServerSync ?? false,
    collection: FLEET_WORKER_ENTITY_TYPE,
    decode: entity =>
      decodeFleetWorkerEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.workerId,
  })

/**
 * `fleet_approval` collection — the pending-approval cards. Reads are the
 * projected state; the `approval_decision` intent flips a card via
 * `session.mutate(fleetDispatchApprovalDecisionClientMutator, intent)`.
 */
export type FleetApprovalCollectionOptions = Omit<
  KhalaSyncCollectionOptions<FleetApprovalEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
>

export const fleetApprovalKhalaSyncCollectionOptions = (
  options: FleetApprovalCollectionOptions,
): CollectionConfig<FleetApprovalEntity, string, never, KhalaSyncCollectionUtils> =>
  khalaSyncCollectionOptions<FleetApprovalEntity, string>({
    ...options,
    awaitServerSync: options.awaitServerSync ?? false,
    collection: FLEET_APPROVAL_ENTITY_TYPE,
    decode: entity =>
      decodeFleetApprovalEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.approvalRef,
  })

/** Read-only `fleet_steer` collection — the body-free steer receipts. */
export type FleetSteerCollectionOptions = Omit<
  KhalaSyncCollectionOptions<FleetSteerEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
>

export const fleetSteerKhalaSyncCollectionOptions = (
  options: FleetSteerCollectionOptions,
): CollectionConfig<FleetSteerEntity, string, never, KhalaSyncCollectionUtils> =>
  khalaSyncCollectionOptions<FleetSteerEntity, string>({
    ...options,
    awaitServerSync: options.awaitServerSync ?? false,
    collection: FLEET_STEER_ENTITY_TYPE,
    decode: entity =>
      decodeFleetSteerEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.steerRef,
  })

export type ChatThreadCollectionOptions = Omit<
  KhalaSyncCollectionOptions<ChatThreadEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
> &
  Readonly<{
    ownerUserId: string
    createThreadMutator?: ClientMutator<ChatCreateThreadArgs>
    optimisticNow?: () => string
    renameThreadMutator?: ClientMutator<ChatRenameThreadArgs>
  }>

export const chatThreadKhalaSyncCollectionOptions = (
  options: ChatThreadCollectionOptions,
): CollectionConfig<ChatThreadEntity, string, never, KhalaSyncCollectionUtils> => {
  const mutatorOptions: ChatClientMutatorOptions = {
    ownerUserId: options.ownerUserId,
    ...(options.optimisticNow === undefined ? {} : { now: options.optimisticNow }),
  }
  const createThreadMutator =
    options.createThreadMutator ?? chatCreateThreadClientMutator(mutatorOptions)
  const renameThreadMutator =
    options.renameThreadMutator ?? chatRenameThreadClientMutator(mutatorOptions)

  return khalaSyncCollectionOptions<ChatThreadEntity, string>({
    ...options,
    awaitServerSync: options.awaitServerSync ?? false,
    collection: CHAT_THREAD_ENTITY_TYPE,
    decode: entity =>
      decodeChatThreadEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.threadId,
    mutators: {
      insert: mutation => ({
        args: {
          threadId: mutation.modified.threadId,
          title: mutation.modified.title,
        },
        mutator: createThreadMutator,
      }),
      update: mutation => {
        const title = (mutation.changes as Partial<ChatThreadEntity>).title
        if (typeof title !== "string") {
          throw new KhalaSyncDbCollectionError(
            "khala_sync_db_collection.missing_mutator",
            {
              collection: CHAT_THREAD_ENTITY_TYPE,
              messageSafe:
                "chat_thread updates currently require a title change",
              scope: options.scope,
            },
          )
        }
        return {
          args: {
            threadId: mutation.key,
            title,
          },
          mutator: renameThreadMutator,
        }
      },
    },
  })
}

export type ChatMessageCollectionOptions = Omit<
  KhalaSyncCollectionOptions<ChatMessageEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
> &
  Readonly<{
    appendMessageMutator?: ClientMutator<ChatAppendMessageArgs>
  }>

export const chatMessageKhalaSyncCollectionOptions = (
  options: ChatMessageCollectionOptions,
): CollectionConfig<ChatMessageEntity, string, never, KhalaSyncCollectionUtils> => {
  const appendMessageMutator = options.appendMessageMutator

  return khalaSyncCollectionOptions<ChatMessageEntity, string>({
    ...options,
    awaitServerSync: options.awaitServerSync ?? false,
    collection: CHAT_MESSAGE_ENTITY_TYPE,
    decode: entity =>
      decodeChatMessageEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.messageId,
    mutators: {
      insert: mutation => {
        if (appendMessageMutator === undefined) {
          throw new KhalaSyncDbCollectionError(
            "khala_sync_db_collection.missing_mutator",
            {
              collection: CHAT_MESSAGE_ENTITY_TYPE,
              messageSafe:
                "chat_message inserts require a chat.appendMessage mutator",
              scope: options.scope,
            },
          )
        }
        return {
          args: {
            body: mutation.modified.body,
            messageId: mutation.modified.messageId,
            threadId: mutation.modified.threadId,
          },
          mutator: appendMessageMutator,
        }
      },
    },
  })
}

/**
 * Read-only `runtime_event` collection (#8425 desktop render gap): Khala
 * Code desktop never writes `runtime_event` rows itself — a turn dispatched
 * from mobile or Codex/Claude App Server streams these into
 * `scope.thread.<threadId>` via the Pylon runtime-intent-supervisor, and
 * desktop only needs to fold them into a transcript. No `mutators` are
 * configured on purpose: this collection is never the target of a local
 * `.insert()`/`.update()`, so there is nothing to map to a named mutator.
 */
export type RuntimeEventCollectionOptions = Omit<
  KhalaSyncCollectionOptions<RuntimeEventEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
>

export const runtimeEventKhalaSyncCollectionOptions = (
  options: RuntimeEventCollectionOptions,
): CollectionConfig<RuntimeEventEntity, string, never, KhalaSyncCollectionUtils> =>
  khalaSyncCollectionOptions<RuntimeEventEntity, string>({
    ...options,
    awaitServerSync: options.awaitServerSync ?? false,
    collection: RUNTIME_EVENT_ENTITY_TYPE,
    decode: entity =>
      decodeRuntimeEventEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.eventId,
  })

/**
 * Read-only `runtime_turn` collection — same rationale as
 * `runtimeEventKhalaSyncCollectionOptions` above: desktop only reads turn
 * status/lane rows to fold a transcript, never writes them locally.
 */
export type RuntimeTurnCollectionOptions = Omit<
  KhalaSyncCollectionOptions<RuntimeTurnEntity, string>,
  "collection" | "decode" | "entityIdFromKey" | "getKey" | "mutators"
>

export const runtimeTurnKhalaSyncCollectionOptions = (
  options: RuntimeTurnCollectionOptions,
): CollectionConfig<RuntimeTurnEntity, string, never, KhalaSyncCollectionUtils> =>
  khalaSyncCollectionOptions<RuntimeTurnEntity, string>({
    ...options,
    awaitServerSync: options.awaitServerSync ?? false,
    collection: RUNTIME_TURN_ENTITY_TYPE,
    decode: entity =>
      decodeRuntimeTurnEntity(JSON.parse(entity.postImageJson) as unknown),
    entityIdFromKey: key => key,
    getKey: row => row.turnId,
  })
