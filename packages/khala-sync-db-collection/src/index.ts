import type {
  ChangeMessageOrDeleteKeyMessage,
  CollectionConfig,
  LoadSubsetOptions,
  PendingMutation,
  SyncMetadataApi,
  TransactionWithMutations,
  UtilsRecord,
} from "@tanstack/db"
import {
  canonicalJson,
  decodeFleetRunEntity,
  encodeFleetRunEntity,
  FLEET_RUN_ENTITY_TYPE,
  fleetRunScope,
  type FleetRunEntity,
  MutationId,
  type MutationEnvelope,
  MutationResult,
  MutatorName,
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

const captureNewMutationId = (
  beforeIds: ReadonlySet<number>,
  session: KhalaSyncSession,
  fallbackMutationId: number,
): MutationId => {
  const created = session
    .pending()
    .filter(pending => !beforeIds.has(Number(pending.mutationId)))
    .sort((a, b) => Number(a.mutationId) - Number(b.mutationId))

  const first = created[0]
  if (first !== undefined) return first.mutationId

  return MutationId.make(fallbackMutationId)
}

const pendingIds = (session: KhalaSyncSession): ReadonlySet<number> =>
  new Set(session.pending().map(pending => Number(pending.mutationId)))

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
  let lastObservedMutationId = Math.max(0, ...pendingIds(options.session))

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
    const before = pendingIds(options.session)
    const fallbackMutationId = Math.max(lastObservedMutationId, ...before) + 1
    await runEffect(options.session.mutate(command.mutator, command.args)).catch(
      cause => {
        throw makeError(
          "khala_sync_db_collection.session_failure",
          "Khala Sync mutation enqueue failed",
          { cause },
        )
      },
    )
    const mutationId = captureNewMutationId(before, options.session, fallbackMutationId)
    lastObservedMutationId = Math.max(lastObservedMutationId, Number(mutationId))
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

export type FleetSetDesiredSlotsArgs = Readonly<{
  runId: string
  desiredSlots: number
}>

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
