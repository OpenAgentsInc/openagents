import {
  ClientGroupId,
  ClientId,
  canonicalJson,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  decodeFleetAccountEntity,
  decodeFleetAssignmentEntity,
  decodeFleetInboxFlagEntity,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  FLEET_ACCOUNT_ENTITY_TYPE,
  FLEET_ASSIGNMENT_ENTITY_TYPE,
  FLEET_INBOX_FLAG_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  fleetRunScope,
  MutatorName,
  personalScope,
  SyncSchemaVersion,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { createCollection, type Collection } from "@tanstack/db"
import {
  createHttpKhalaSyncTransport,
  createOverlay,
  openKhalaSyncStore,
  createKhalaSyncSession,
  type ClientMutator,
  type KhalaSyncOverlay,
  type KhalaSyncSession,
  type KhalaSyncSqliteStore,
  type KhalaSyncTransport,
  type OverlayReadView,
  type ScopeSyncState,
  type WebSocketLike,
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
  type KhalaSyncMutationTracker,
} from "@openagentsinc/khala-sync-db-collection"
import { Cause, Effect, Exit } from "effect"
import { homedir } from "node:os"
import { dirname, join } from "node:path"
import { mkdirSync } from "node:fs"
import type {
  KhalaCodeDesktopKhalaSyncFleetMutateRequest,
  KhalaCodeDesktopKhalaSyncFleetMutateResult,
  KhalaCodeDesktopKhalaSyncChatAppendMessageRequest,
  KhalaCodeDesktopKhalaSyncChatCreateThreadRequest,
  KhalaCodeDesktopKhalaSyncChatMessagesRequest,
  KhalaCodeDesktopKhalaSyncChatMessagesResult,
  KhalaCodeDesktopKhalaSyncChatMutationResult,
  KhalaCodeDesktopKhalaSyncChatRejection,
  KhalaCodeDesktopKhalaSyncChatRenameThreadRequest,
  KhalaCodeDesktopKhalaSyncChatThreadsRequest,
  KhalaCodeDesktopKhalaSyncChatThreadsResult,
  KhalaCodeDesktopKhalaSyncFleetRejection,
  KhalaCodeDesktopKhalaSyncFleetStateRequest,
  KhalaCodeDesktopKhalaSyncFleetStateResult,
} from "../shared/rpc.js"
import { resolveKhalaCodeDesktopOpenAgentsAgentToken } from "./harness-setting.js"

/**
 * Khala Code desktop Khala Sync service (KS-6.2, #8303; SPEC §6).
 *
 * First desktop consumer of `@openagentsinc/khala-sync-client`: opens the
 * durable local SQLite store under the app data dir (`~/.khala-code/`),
 * builds the optimistic overlay with the fleet operator mutators, speaks
 * HTTP/WS against the configured OpenAgents base URL with the user's
 * OpenAgents agent token, and exposes fleet-scope reads + mutates over the
 * desktop RPC seam (`khalaSyncFleetState` / `khalaSyncFleetMutate`).
 *
 * FLAG-GATED: this path is active only when `KHALA_SYNC_FLEET=1` (or
 * `true`). The Fleet screen's polling source stays default-on until the
 * server routes are deployed and verified end-to-end; the flag flips
 * default in a follow-up on epic #8282.
 *
 * Honesty rules carried into the RPC surface:
 * - `phase` is the session's real scope state (`live` requires an open
 *   live socket) — the UI must never render "live" from anything else.
 * - `must_refetch` self-heals: the session re-bootstraps automatically;
 *   the phase is surfaced so the UI can show a visible re-sync state.
 * - In-band mutation rejections are surfaced as state (`rejections`),
 *   never thrown; the push queue keeps draining per SPEC §2.4.
 */

// ---------------------------------------------------------------------------
// Flag + config resolution
// ---------------------------------------------------------------------------

type ServiceEnv = Readonly<Record<string, string | undefined>>

export const KHALA_SYNC_FLEET_FLAG_ENV = "KHALA_SYNC_FLEET"
export const KHALA_SYNC_CHAT_FLAG_ENV = "KHALA_SYNC_CHAT"
export const KHALA_SYNC_CHAT_OWNER_USER_ID_ENV = "KHALA_SYNC_CHAT_OWNER_USER_ID"

export const khalaCodeDesktopKhalaSyncFleetEnabled = (env: ServiceEnv): boolean => {
  const value = env[KHALA_SYNC_FLEET_FLAG_ENV]?.trim().toLowerCase()
  return value === "1" || value === "true"
}

export const khalaCodeDesktopKhalaSyncChatEnabled = (env: ServiceEnv): boolean => {
  const value = env[KHALA_SYNC_CHAT_FLAG_ENV]?.trim().toLowerCase()
  return value === "1" || value === "true"
}

export const khalaCodeDesktopKhalaSyncChatOwnerUserId = (
  env: ServiceEnv,
): string | null => {
  const value = env[KHALA_SYNC_CHAT_OWNER_USER_ID_ENV]?.trim()
  return value === undefined || value.length === 0 ? null : value
}

const DEFAULT_OPENAGENTS_BASE_URL = "https://openagents.com"

const khalaSyncBaseUrl = (env: ServiceEnv): string => {
  const configured =
    env.PYLON_OPENAGENTS_BASE_URL?.trim() || env.OPENAGENTS_BASE_URL?.trim()
  return configured !== undefined && configured.length > 0
    ? configured
    : DEFAULT_OPENAGENTS_BASE_URL
}

const defaultStorePath = (env: ServiceEnv): string =>
  join(env.HOME?.trim() || homedir(), ".khala-code", "khala-sync.sqlite3")

/** Data-schema version this desktop build understands (fleet.v1 shapes). */
const KHALA_SYNC_DESKTOP_SCHEMA_VERSION = SyncSchemaVersion.make(1)

// ---------------------------------------------------------------------------
// Fleet client mutators (SPEC §2.4)
//
// Names MUST match the server registry in
// packages/khala-sync-server/src/fleet-mutators.ts — the wire contract is
// the mutator name + canonical-JSON args. The pure `apply` functions patch
// the current `fleet_run` post-image optimistically; when the scope has no
// decodable run yet there is nothing truthful to patch, so they produce no
// optimistic effect and let the server's confirmed post-image land instead.
// Replay-safe by construction: no clocks, no randomness, reads only the
// provided overlay view.
// ---------------------------------------------------------------------------

export const FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME = "fleet.setDesiredSlots"
export const FLEET_PAUSE_RUN_MUTATOR_NAME = "fleet.pauseRun"
export const FLEET_RESUME_RUN_MUTATOR_NAME = "fleet.resumeRun"
export const FLEET_PAUSE_WORKER_MUTATOR_NAME = "fleet.pauseWorker"
export const FLEET_RESUME_WORKER_MUTATOR_NAME = "fleet.resumeWorker"
export const FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME =
  "fleet.acknowledgeInboxFlag"
export const FLEET_STOP_RUN_MUTATOR_NAME = "fleet.stopRun"

type FleetEntityPatch = Readonly<Record<string, unknown>>

const patchedEntityEffects = (
  view: OverlayReadView,
  runId: string,
  entityType: string,
  entityId: string,
  decode: (value: unknown) => unknown,
  patch: FleetEntityPatch,
) => {
  const scope = fleetRunScope(runId)
  const currentJson = view.get(scope, entityType, entityId)
  if (currentJson === undefined) return []
  let current: unknown
  try {
    current = decode(JSON.parse(currentJson))
  } catch {
    return []
  }
  return [
    {
      kind: "upsert" as const,
      scope,
      entityType,
      entityId,
      postImageJson: canonicalJson({ ...(current as object), ...patch }),
    },
  ]
}

const patchedFleetRunEffects = (
  view: OverlayReadView,
  runId: string,
  patch: FleetEntityPatch,
) =>
  patchedEntityEffects(
    view,
    runId,
    FLEET_RUN_ENTITY_TYPE,
    runId,
    decodeFleetRunEntity,
    patch,
  )

export const fleetSetDesiredSlotsClientMutator: ClientMutator<{
  readonly runId: string
  readonly desiredSlots: number
}> = {
  name: MutatorName.make(FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME),
  apply: (args, view) =>
    patchedFleetRunEffects(view, args.runId, { desiredSlots: args.desiredSlots }),
}

export const fleetPauseRunClientMutator: ClientMutator<{ readonly runId: string }> = {
  name: MutatorName.make(FLEET_PAUSE_RUN_MUTATOR_NAME),
  apply: (args, view) => patchedFleetRunEffects(view, args.runId, { status: "paused" }),
}

export const fleetResumeRunClientMutator: ClientMutator<{ readonly runId: string }> = {
  name: MutatorName.make(FLEET_RESUME_RUN_MUTATOR_NAME),
  apply: (args, view) => patchedFleetRunEffects(view, args.runId, { status: "running" }),
}

export const fleetPauseWorkerClientMutator: ClientMutator<{
  readonly runId: string
  readonly workerId: string
}> = {
  name: MutatorName.make(FLEET_PAUSE_WORKER_MUTATOR_NAME),
  apply: (args, view) =>
    patchedEntityEffects(
      view,
      args.runId,
      FLEET_WORKER_ENTITY_TYPE,
      args.workerId,
      decodeFleetWorkerEntity,
      { phase: "paused" },
    ),
}

export const fleetResumeWorkerClientMutator: ClientMutator<{
  readonly runId: string
  readonly workerId: string
}> = {
  name: MutatorName.make(FLEET_RESUME_WORKER_MUTATOR_NAME),
  apply: (args, view) =>
    patchedEntityEffects(
      view,
      args.runId,
      FLEET_WORKER_ENTITY_TYPE,
      args.workerId,
      decodeFleetWorkerEntity,
      { phase: "idle" },
    ),
}

/**
 * Optimistically flips a SYNCED flag to acknowledged. The server also
 * records acks for flags it has never projected (kind `unclassified`), but
 * with no local post-image there is nothing truthful to patch — the
 * confirmed entity lands on rebase. Timestamps are server truth only
 * (replay-safe: no clocks in optimistic appliers).
 */
export const fleetAcknowledgeInboxFlagClientMutator: ClientMutator<{
  readonly runId: string
  readonly flagRef: string
}> = {
  name: MutatorName.make(FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME),
  apply: (args, view) =>
    patchedEntityEffects(
      view,
      args.runId,
      FLEET_INBOX_FLAG_ENTITY_TYPE,
      args.flagRef,
      decodeFleetInboxFlagEntity,
      { status: "acknowledged" },
    ),
}

/**
 * Terminal stop. Optimistic only when `confirm` is true — an unconfirmed
 * envelope is a guaranteed server rejection, so patching ahead of it would
 * show a lie.
 */
export const fleetStopRunClientMutator: ClientMutator<{
  readonly runId: string
  readonly confirm: boolean
}> = {
  name: MutatorName.make(FLEET_STOP_RUN_MUTATOR_NAME),
  apply: (args, view) =>
    args.confirm
      ? patchedFleetRunEffects(view, args.runId, {
          desiredSlots: 0,
          status: "stopped",
        })
      : [],
}

export const fleetClientMutators = [
  fleetSetDesiredSlotsClientMutator,
  fleetPauseRunClientMutator,
  fleetResumeRunClientMutator,
  fleetPauseWorkerClientMutator,
  fleetResumeWorkerClientMutator,
  fleetAcknowledgeInboxFlagClientMutator,
  fleetStopRunClientMutator,
] as const

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

export type KhalaCodeDesktopKhalaSyncRpc = {
  readonly chatAppendMessage: (
    request: KhalaCodeDesktopKhalaSyncChatAppendMessageRequest,
  ) => Promise<KhalaCodeDesktopKhalaSyncChatMutationResult>
  readonly chatCreateThread: (
    request: KhalaCodeDesktopKhalaSyncChatCreateThreadRequest,
  ) => Promise<KhalaCodeDesktopKhalaSyncChatMutationResult>
  readonly chatMessages: (
    request: KhalaCodeDesktopKhalaSyncChatMessagesRequest,
  ) => Promise<KhalaCodeDesktopKhalaSyncChatMessagesResult>
  readonly chatRenameThread: (
    request: KhalaCodeDesktopKhalaSyncChatRenameThreadRequest,
  ) => Promise<KhalaCodeDesktopKhalaSyncChatMutationResult>
  readonly chatThreads: (
    request?: KhalaCodeDesktopKhalaSyncChatThreadsRequest,
  ) => Promise<KhalaCodeDesktopKhalaSyncChatThreadsResult>
  readonly fleetState: (
    request: KhalaCodeDesktopKhalaSyncFleetStateRequest,
  ) => Promise<KhalaCodeDesktopKhalaSyncFleetStateResult>
  readonly fleetMutate: (
    request: KhalaCodeDesktopKhalaSyncFleetMutateRequest,
  ) => Promise<KhalaCodeDesktopKhalaSyncFleetMutateResult>
}

export type KhalaCodeDesktopKhalaSyncService = KhalaCodeDesktopKhalaSyncRpc & {
  readonly close: () => Promise<void>
}

export type KhalaCodeDesktopKhalaSyncServiceOptions = {
  readonly env: ServiceEnv
  /** Test seam: sqlite path (default `~/.khala-code/khala-sync.sqlite3`; use `":memory:"` in tests). */
  readonly storePath?: string
  /** Test seam: replaces the HTTP/WS transport with a deterministic fake. */
  readonly transport?: (config: {
    readonly baseUrl: string
    readonly authToken: () => string
  }) => KhalaSyncTransport
  /** Test seams for the real HTTP transport. */
  readonly fetch?: typeof globalThis.fetch
  readonly webSocket?: new (url: string) => WebSocketLike
  /** Injected timing (tests run instantly). */
  readonly sleep?: (ms: number) => Promise<void>
  readonly random?: () => number
  readonly now?: () => Date
}

/** The empty, honest "this path is off" result. */
export const khalaSyncFleetDisabledState = (): KhalaCodeDesktopKhalaSyncFleetStateResult => ({
  accounts: [],
  assignments: [],
  authState: "missing",
  cursor: null,
  enabled: false,
  ok: true,
  pendingMutations: 0,
  phase: "disabled",
  reason: null,
  rejections: [],
  run: null,
  workers: [],
})

export const khalaSyncChatDisabledState = (): KhalaCodeDesktopKhalaSyncChatThreadsResult => ({
  authState: "missing",
  cursor: null,
  enabled: false,
  ok: true,
  ownerUserId: null,
  pendingMutations: 0,
  phase: "disabled",
  reason: null,
  rejections: [],
  threads: [],
})

export const khalaSyncChatMessagesDisabledState = (
  threadId: string,
): KhalaCodeDesktopKhalaSyncChatMessagesResult => ({
  authState: "missing",
  cursor: null,
  enabled: false,
  ok: true,
  ownerUserId: null,
  pendingMutations: 0,
  phase: "disabled",
  reason: null,
  rejections: [],
  messages: [],
  threadId,
})

const MAX_TRACKED_REJECTIONS = 20

interface SessionRuntime {
  readonly store: KhalaSyncSqliteStore
  readonly overlay: KhalaSyncOverlay
  readonly session: KhalaSyncSession
  readonly mutationTracker: KhalaSyncMutationTracker
  readonly chatMutators: {
    readonly appendMessage: ClientMutator<ChatAppendMessageArgs>
    readonly createThread: ClientMutator<ChatCreateThreadArgs>
    readonly renameThread: ClientMutator<ChatRenameThreadArgs>
  } | null
  chatMessageCollections: Map<string, Collection<ChatMessageEntity, string, KhalaSyncCollectionUtils>>
  chatThreadsCollection: Collection<ChatThreadEntity, string, KhalaSyncCollectionUtils> | null
}

/** Run a typed Effect from promise-land, rethrowing the typed error. */
const runEffect = async <A, E>(effect: Effect.Effect<A, E>): Promise<A> => {
  const exit = await Effect.runPromiseExit(effect)
  if (Exit.isSuccess(exit)) return exit.value
  throw Cause.squash(exit.cause)
}

const errorText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const decodeListed = <A>(
  entries: ReadonlyArray<{ readonly postImageJson: string }>,
  decode: (input: unknown) => A,
): Array<A> => {
  const decoded: Array<A> = []
  for (const entry of entries) {
    try {
      decoded.push(decode(JSON.parse(entry.postImageJson)))
    } catch {
      // Pre-contract or foreign post-image: skip rather than fabricate. The
      // next confirmed projection self-heals (post-image log, SPEC §2.3).
    }
  }
  return decoded
}

export const createKhalaCodeDesktopKhalaSyncService = (
  options: KhalaCodeDesktopKhalaSyncServiceOptions,
): KhalaCodeDesktopKhalaSyncService => {
  const env = options.env
  const enabled = khalaCodeDesktopKhalaSyncFleetEnabled(env)
  const chatEnabled = khalaCodeDesktopKhalaSyncChatEnabled(env)
  const chatOwnerUserId = khalaCodeDesktopKhalaSyncChatOwnerUserId(env)
  const baseUrl = khalaSyncBaseUrl(env)
  const now = options.now ?? (() => new Date())

  let runtime: SessionRuntime | null = null
  let runtimeFailure: string | null = null
  let currentToken: string | null = null
  let closed = false
  const subscribed = new Set<SyncScope>()
  const rejections: Array<KhalaCodeDesktopKhalaSyncFleetRejection> = []
  const chatRejections: Array<KhalaCodeDesktopKhalaSyncChatRejection> = []
  let lastTransportError: string | null = null

  const recordRejection = (rejection: KhalaCodeDesktopKhalaSyncFleetRejection): void => {
    rejections.push(rejection)
    if (rejections.length > MAX_TRACKED_REJECTIONS) {
      rejections.splice(0, rejections.length - MAX_TRACKED_REJECTIONS)
    }
  }

  const recordChatRejection = (
    rejection: KhalaCodeDesktopKhalaSyncChatRejection,
  ): void => {
    chatRejections.push(rejection)
    if (chatRejections.length > MAX_TRACKED_REJECTIONS) {
      chatRejections.splice(0, chatRejections.length - MAX_TRACKED_REJECTIONS)
    }
  }

  const refreshToken = async (): Promise<string | null> => {
    // Re-resolved on every RPC entry so env/persisted token rotation is
    // picked up; the transport reads `authToken()` per request.
    currentToken = await resolveKhalaCodeDesktopOpenAgentsAgentToken(env)
    return currentToken
  }

  const ensureRuntime = async (): Promise<SessionRuntime | null> => {
    if (closed) return null
    if (runtime !== null) return runtime
    try {
      const storePath = options.storePath ?? defaultStorePath(env)
      if (storePath !== ":memory:") {
        mkdirSync(dirname(storePath), { recursive: true })
      }
      const store = openKhalaSyncStore(storePath)
      const persisted = await runEffect(store.identity())
      const clientGroupId =
        persisted?.clientGroupId ?? ClientGroupId.make(`khala-code-desktop.${crypto.randomUUID()}`)
      const clientId = persisted?.clientId ?? ClientId.make(crypto.randomUUID())
      const chatMutators = chatOwnerUserId === null
        ? null
        : {
            appendMessage: chatAppendMessageClientMutator({
              ownerUserId: chatOwnerUserId,
            }),
            createThread: chatCreateThreadClientMutator({
              ownerUserId: chatOwnerUserId,
            }),
            renameThread: chatRenameThreadClientMutator({
              ownerUserId: chatOwnerUserId,
            }),
          }
      const mutationTracker = createKhalaSyncMutationTracker()
      const overlay = await runEffect(createOverlay(store, [
        ...fleetClientMutators,
        ...(chatMutators === null
          ? []
          : [
              chatMutators.appendMessage,
              chatMutators.createThread,
              chatMutators.renameThread,
            ]),
      ]))
      const transportConfig = {
        baseUrl,
        authToken: () => currentToken ?? "",
      }
      const transport =
        options.transport?.(transportConfig) ??
        createHttpKhalaSyncTransport(transportConfig, {
          ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
          ...(options.webSocket === undefined ? {} : { webSocket: options.webSocket }),
        })
      const session = createKhalaSyncSession(
        {
          baseUrl,
          clientGroupId,
          clientId,
          schemaVersion: KHALA_SYNC_DESKTOP_SCHEMA_VERSION,
          authToken: transportConfig.authToken,
        },
        store,
        overlay,
        transport,
        {
          ...(options.sleep === undefined ? {} : { sleep: options.sleep }),
          ...(options.random === undefined ? {} : { random: options.random }),
          onRejection: (result, mutation) => {
            mutationTracker.onRejection(result, mutation)
            const mutatorName = mutation?.name ?? "unknown"
            let runId: string | null = null
            let threadId: string | null = null
            if (mutation !== undefined) {
              try {
                const args: unknown = JSON.parse(mutation.argsJson)
                if (
                  typeof args === "object" &&
                  args !== null &&
                  "runId" in args &&
                  typeof (args as { runId: unknown }).runId === "string"
                ) {
                  runId = (args as { runId: string }).runId
                }
                if (
                  typeof args === "object" &&
                  args !== null &&
                  "threadId" in args &&
                  typeof (args as { threadId: unknown }).threadId === "string"
                ) {
                  threadId = (args as { threadId: string }).threadId
                }
              } catch {
                runId = null
                threadId = null
              }
            }
            if (mutatorName.startsWith("chat.")) {
              recordChatRejection({
                errorCode: result.errorCode ?? "rejected",
                messageSafe: result.errorMessageSafe ?? "mutation rejected by the server",
                mutationId: result.mutationId,
                mutatorName,
                observedAt: now().toISOString(),
                threadId,
              })
            } else {
              recordRejection({
                errorCode: result.errorCode ?? "rejected",
                messageSafe: result.errorMessageSafe ?? "mutation rejected by the server",
                mutationId: result.mutationId,
                mutatorName,
                observedAt: now().toISOString(),
                runId,
              })
            }
          },
          onTransportError: (_context, error) => {
            lastTransportError = errorText(error)
          },
        },
      )
      runtime = {
        chatMessageCollections: new Map(),
        chatMutators,
        chatThreadsCollection: null,
        mutationTracker,
        overlay,
        session,
        store,
      }
      runtimeFailure = null
      return runtime
    } catch (error) {
      runtimeFailure = errorText(error)
      return null
    }
  }

  const ensureSubscribed = async (
    active: SessionRuntime,
    scope: SyncScope,
  ): Promise<void> => {
    // Session.subscribe is idempotent while the scope loop runs; tracking the
    // set locally just avoids re-running the identity write on every read.
    if (subscribed.has(scope)) return
    await runEffect(active.session.subscribe(scope))
    subscribed.add(scope)
  }

  const phaseOf = (
    state: ScopeSyncState,
  ): {
    readonly phase: KhalaCodeDesktopKhalaSyncFleetStateResult["phase"]
    readonly cursor: number | null
    readonly reason: string | null
  } => {
    switch (state.phase) {
      case "idle":
        return { phase: "idle", cursor: null, reason: null }
      case "bootstrapping":
        return { phase: "bootstrapping", cursor: null, reason: null }
      case "catching_up":
        return { phase: "catching_up", cursor: state.cursor, reason: null }
      case "live":
        return { phase: "live", cursor: state.cursor, reason: null }
      case "must_refetch":
        return { phase: "must_refetch", cursor: null, reason: state.reason }
      case "denied":
        // KS-7.1 (#8305): the server denied scope access (fail-closed scope
        // auth) and the scope's synced state was CLEARED
        // (khala_sync.access.revocation_clears_synced_state.v1). Surfaced
        // honestly — never rendered as any syncing state.
        return { phase: "denied", cursor: null, reason: state.reason }
    }
  }

  const chatUnavailable = (
    error: string,
  ): KhalaCodeDesktopKhalaSyncChatThreadsResult => ({
    ...khalaSyncChatDisabledState(),
    authState: "connected",
    enabled: chatEnabled,
    error,
    ok: false,
    ownerUserId: chatOwnerUserId,
    phase: chatEnabled ? "idle" : "disabled",
  })

  const chatMessagesUnavailable = (
    threadId: string,
    error: string,
  ): KhalaCodeDesktopKhalaSyncChatMessagesResult => ({
    ...khalaSyncChatMessagesDisabledState(threadId),
    authState: "connected",
    enabled: chatEnabled,
    error,
    ok: false,
    ownerUserId: chatOwnerUserId,
    phase: chatEnabled ? "idle" : "disabled",
  })

  const ensureChatThreadsCollection = (
    active: SessionRuntime,
  ): Collection<ChatThreadEntity, string, KhalaSyncCollectionUtils> | null => {
    if (!chatEnabled || chatOwnerUserId === null || active.chatMutators === null) {
      return null
    }
    if (active.chatThreadsCollection !== null) return active.chatThreadsCollection
    active.chatThreadsCollection = createCollection(
      chatThreadKhalaSyncCollectionOptions({
        awaitMutationPollIntervalMs: 0,
        awaitMutationTimeoutMs: 5_000,
        createThreadMutator: active.chatMutators.createThread,
        mutationTracker: active.mutationTracker,
        overlay: active.overlay,
        ownerUserId: chatOwnerUserId,
        renameThreadMutator: active.chatMutators.renameThread,
        scope: personalScope(chatOwnerUserId),
        session: active.session,
        ...(options.sleep === undefined ? {} : { sleep: options.sleep }),
        startSync: true,
      }),
    )
    return active.chatThreadsCollection
  }

  const ensureChatMessagesCollection = (
    active: SessionRuntime,
    threadId: string,
  ): Collection<ChatMessageEntity, string, KhalaSyncCollectionUtils> | null => {
    if (!chatEnabled || chatOwnerUserId === null || active.chatMutators === null) {
      return null
    }
    const existing = active.chatMessageCollections.get(threadId)
    if (existing !== undefined) return existing
    const collection = createCollection(
      chatMessageKhalaSyncCollectionOptions({
        appendMessageMutator: active.chatMutators.appendMessage,
        awaitMutationPollIntervalMs: 0,
        awaitMutationTimeoutMs: 5_000,
        mutationTracker: active.mutationTracker,
        overlay: active.overlay,
        scope: threadScope(threadId),
        session: active.session,
        ...(options.sleep === undefined ? {} : { sleep: options.sleep }),
        startSync: true,
      }),
    )
    active.chatMessageCollections.set(threadId, collection)
    return collection
  }

  const chatThreadToRpc = (
    thread: ChatThreadEntity,
  ): KhalaCodeDesktopKhalaSyncChatThreadsResult["threads"][number] => ({
    createdAt: thread.createdAt,
    lastMessageAt: thread.lastMessageAt,
    messageCount: thread.messageCount,
    ownerUserId: thread.ownerUserId,
    status: thread.status,
    threadId: thread.threadId,
    title: thread.title,
    updatedAt: thread.updatedAt,
  })

  const chatMessageToRpc = (
    message: ChatMessageEntity,
  ): KhalaCodeDesktopKhalaSyncChatMessagesResult["messages"][number] => ({
    authorUserId: message.authorUserId,
    body: message.body,
    createdAt: message.createdAt,
    deletedAt: message.deletedAt,
    messageId: message.messageId,
    threadId: message.threadId,
    updatedAt: message.updatedAt,
  })

  const chatThreads = async (
    request: KhalaCodeDesktopKhalaSyncChatThreadsRequest = {},
  ): Promise<KhalaCodeDesktopKhalaSyncChatThreadsResult> => {
    if (!chatEnabled || closed) return khalaSyncChatDisabledState()
    if (chatOwnerUserId === null) return chatUnavailable("missing_chat_owner_user_id")
    const token = await refreshToken()
    if (token === null) {
      return {
        ...khalaSyncChatDisabledState(),
        authState: "missing",
        enabled: true,
        error: "missing_openagents_auth",
        ownerUserId: chatOwnerUserId,
        phase: "idle",
      }
    }
    const active = await ensureRuntime()
    if (active === null) {
      return chatUnavailable(runtimeFailure ?? "khala_sync_store_unavailable")
    }
    const collection = ensureChatThreadsCollection(active)
    if (collection === null) return chatUnavailable("khala_sync_chat_unavailable")
    const scope = personalScope(chatOwnerUserId)
    try {
      await collection.preload()
      const { phase, cursor, reason } = phaseOf(active.session.state(scope))
      const limit = request.limit === undefined
        ? 50
        : Math.max(0, Math.min(200, Math.trunc(request.limit)))
      const view = await runEffect(active.overlay.read(scope))
      const threads = chatThreadsForSidebar(
        decodeListed(view.list(CHAT_THREAD_ENTITY_TYPE), decodeChatThreadEntity),
        {
          ...(request.searchTerm === undefined ? {} : { searchTerm: request.searchTerm }),
        },
      ).slice(0, limit)
      return {
        authState: "connected",
        cursor,
        enabled: true,
        ...(lastTransportError === null || phase === "live"
          ? {}
          : { error: lastTransportError }),
        ok: true,
        ownerUserId: chatOwnerUserId,
        pendingMutations: active.overlay.pending().length,
        phase,
        reason,
        rejections: [...chatRejections],
        threads: threads.map(chatThreadToRpc),
      }
    } catch (error) {
      return {
        ...chatUnavailable(errorText(error)),
        ok: false,
      }
    }
  }

  const chatMessages = async (
    request: KhalaCodeDesktopKhalaSyncChatMessagesRequest,
  ): Promise<KhalaCodeDesktopKhalaSyncChatMessagesResult> => {
    if (!chatEnabled || closed) return khalaSyncChatMessagesDisabledState(request.threadId)
    if (chatOwnerUserId === null) {
      return chatMessagesUnavailable(request.threadId, "missing_chat_owner_user_id")
    }
    const token = await refreshToken()
    if (token === null) {
      return {
        ...khalaSyncChatMessagesDisabledState(request.threadId),
        authState: "missing",
        enabled: true,
        error: "missing_openagents_auth",
        ownerUserId: chatOwnerUserId,
        phase: "idle",
      }
    }
    const active = await ensureRuntime()
    if (active === null) {
      return chatMessagesUnavailable(
        request.threadId,
        runtimeFailure ?? "khala_sync_store_unavailable",
      )
    }
    const collection = ensureChatMessagesCollection(active, request.threadId)
    if (collection === null) {
      return chatMessagesUnavailable(request.threadId, "khala_sync_chat_unavailable")
    }
    const scope = threadScope(request.threadId)
    try {
      await collection.preload()
      const { phase, cursor, reason } = phaseOf(active.session.state(scope))
      const limit = request.limit === undefined
        ? 500
        : Math.max(0, Math.min(2_000, Math.trunc(request.limit)))
      const messages = chatMessagesForTranscript(collection.values()).slice(-limit)
      return {
        authState: "connected",
        cursor,
        enabled: true,
        ...(lastTransportError === null || phase === "live"
          ? {}
          : { error: lastTransportError }),
        messages: messages.map(chatMessageToRpc),
        ok: true,
        ownerUserId: chatOwnerUserId,
        pendingMutations: active.overlay.pending().length,
        phase,
        reason,
        rejections: [...chatRejections],
        threadId: request.threadId,
      }
    } catch (error) {
      return {
        ...chatMessagesUnavailable(request.threadId, errorText(error)),
        ok: false,
      }
    }
  }

  const chatCreateThread = async (
    request: KhalaCodeDesktopKhalaSyncChatCreateThreadRequest,
  ): Promise<KhalaCodeDesktopKhalaSyncChatMutationResult> => {
    if (!chatEnabled || closed) {
      return { ok: false, error: "khala_sync_chat_disabled", threadId: request.threadId }
    }
    if (chatOwnerUserId === null) {
      return { ok: false, error: "missing_chat_owner_user_id", threadId: request.threadId }
    }
    const token = await refreshToken()
    if (token === null) {
      return { ok: false, error: "missing_openagents_auth", threadId: request.threadId }
    }
    const active = await ensureRuntime()
    if (active === null) {
      return {
        ok: false,
        error: runtimeFailure ?? "khala_sync_store_unavailable",
        threadId: request.threadId,
      }
    }
    const collection = ensureChatThreadsCollection(active)
    if (collection === null) {
      return { ok: false, error: "khala_sync_chat_unavailable", threadId: request.threadId }
    }
    try {
      await collection.preload()
      const nowIso = now().toISOString()
      const tx = collection.insert(
        decodeChatThreadEntity({
          createdAt: nowIso,
          lastMessageAt: null,
          messageCount: 0,
          ownerUserId: chatOwnerUserId,
          status: "active",
          threadId: request.threadId,
          title: request.title.trim(),
          updatedAt: nowIso,
        }),
      )
      await tx.isPersisted.promise
      return { ok: true, threadId: request.threadId }
    } catch (error) {
      return { ok: false, error: errorText(error), threadId: request.threadId }
    }
  }

  const chatAppendMessage = async (
    request: KhalaCodeDesktopKhalaSyncChatAppendMessageRequest,
  ): Promise<KhalaCodeDesktopKhalaSyncChatMutationResult> => {
    if (!chatEnabled || closed) {
      return {
        ok: false,
        error: "khala_sync_chat_disabled",
        messageId: request.messageId,
        threadId: request.threadId,
      }
    }
    if (chatOwnerUserId === null) {
      return {
        ok: false,
        error: "missing_chat_owner_user_id",
        messageId: request.messageId,
        threadId: request.threadId,
      }
    }
    const token = await refreshToken()
    if (token === null) {
      return {
        ok: false,
        error: "missing_openagents_auth",
        messageId: request.messageId,
        threadId: request.threadId,
      }
    }
    const active = await ensureRuntime()
    if (active === null) {
      return {
        ok: false,
        error: runtimeFailure ?? "khala_sync_store_unavailable",
        messageId: request.messageId,
        threadId: request.threadId,
      }
    }
    const collection = ensureChatMessagesCollection(active, request.threadId)
    if (collection === null) {
      return {
        ok: false,
        error: "khala_sync_chat_unavailable",
        messageId: request.messageId,
        threadId: request.threadId,
      }
    }
    try {
      await collection.preload()
      const nowIso = now().toISOString()
      const tx = collection.insert(
        decodeChatMessageEntity({
          authorUserId: chatOwnerUserId,
          body: request.body,
          createdAt: nowIso,
          deletedAt: null,
          messageId: request.messageId,
          threadId: request.threadId,
          updatedAt: nowIso,
        }),
      )
      await tx.isPersisted.promise
      return { ok: true, messageId: request.messageId, threadId: request.threadId }
    } catch (error) {
      return {
        ok: false,
        error: errorText(error),
        messageId: request.messageId,
        threadId: request.threadId,
      }
    }
  }

  const chatRenameThread = async (
    request: KhalaCodeDesktopKhalaSyncChatRenameThreadRequest,
  ): Promise<KhalaCodeDesktopKhalaSyncChatMutationResult> => {
    if (!chatEnabled || closed) {
      return { ok: false, error: "khala_sync_chat_disabled", threadId: request.threadId }
    }
    if (chatOwnerUserId === null) {
      return { ok: false, error: "missing_chat_owner_user_id", threadId: request.threadId }
    }
    const token = await refreshToken()
    if (token === null) {
      return { ok: false, error: "missing_openagents_auth", threadId: request.threadId }
    }
    const active = await ensureRuntime()
    if (active === null) {
      return {
        ok: false,
        error: runtimeFailure ?? "khala_sync_store_unavailable",
        threadId: request.threadId,
      }
    }
    const collection = ensureChatThreadsCollection(active)
    if (collection === null) {
      return { ok: false, error: "khala_sync_chat_unavailable", threadId: request.threadId }
    }
    try {
      await collection.preload()
      const tx = collection.update(request.threadId, draft => {
        draft.title = request.title.trim()
      })
      await tx.isPersisted.promise
      return { ok: true, threadId: request.threadId }
    } catch (error) {
      return { ok: false, error: errorText(error), threadId: request.threadId }
    }
  }

  const fleetState = async (
    request: KhalaCodeDesktopKhalaSyncFleetStateRequest,
  ): Promise<KhalaCodeDesktopKhalaSyncFleetStateResult> => {
    if (!enabled || closed) return khalaSyncFleetDisabledState()
    const token = await refreshToken()
    if (token === null) {
      return {
        ...khalaSyncFleetDisabledState(),
        authState: "missing",
        enabled: true,
        error: "missing_openagents_auth",
        phase: "idle",
      }
    }
    const active = await ensureRuntime()
    if (active === null) {
      return {
        ...khalaSyncFleetDisabledState(),
        authState: "connected",
        enabled: true,
        error: runtimeFailure ?? "khala_sync_store_unavailable",
        phase: "idle",
      }
    }
    const scope = fleetRunScope(request.runId)
    try {
      await ensureSubscribed(active, scope)
      const view = await runEffect(active.overlay.read(scope))
      const runs = decodeListed(view.list(FLEET_RUN_ENTITY_TYPE), decodeFleetRunEntity)
      const run = runs.find((entity) => entity.runId === request.runId) ?? runs[0] ?? null
      const { phase, cursor, reason } = phaseOf(active.session.state(scope))
      return {
        accounts: decodeListed(view.list(FLEET_ACCOUNT_ENTITY_TYPE), decodeFleetAccountEntity).map(
          (account) => ({
            accountRefHash: account.accountRefHash,
            rateLimitClass: account.rateLimitClass ?? null,
            readiness: account.readiness,
            updatedAt: account.updatedAt,
          }),
        ),
        assignments: decodeListed(
          view.list(FLEET_ASSIGNMENT_ENTITY_TYPE),
          decodeFleetAssignmentEntity,
        ).map((assignment) => ({
          assignmentRef: assignment.assignmentRef,
          closeoutClass: assignment.closeoutClass ?? null,
          issueRef: assignment.issueRef ?? null,
          status: assignment.status,
          updatedAt: assignment.updatedAt,
        })),
        authState: "connected",
        cursor,
        enabled: true,
        ...(lastTransportError === null || phase === "live"
          ? {}
          : { error: lastTransportError }),
        ok: true,
        pendingMutations: active.overlay.pending().length,
        phase,
        reason,
        rejections: [...rejections],
        run:
          run === null
            ? null
            : {
                counters: { ...run.counters },
                desiredSlots: run.desiredSlots,
                runId: run.runId,
                startedAt: run.startedAt,
                status: run.status,
                updatedAt: run.updatedAt,
                workerKind: run.workerKind,
              },
        workers: decodeListed(view.list(FLEET_WORKER_ENTITY_TYPE), decodeFleetWorkerEntity).map(
          (worker) => ({
            accountRefHash: worker.accountRefHash ?? null,
            assignmentRef: worker.assignmentRef ?? null,
            lastProgressAt: worker.lastProgressAt ?? null,
            phase: worker.phase,
            updatedAt: worker.updatedAt,
            workerId: worker.workerId,
          }),
        ),
      }
    } catch (error) {
      return {
        ...khalaSyncFleetDisabledState(),
        authState: "connected",
        enabled: true,
        error: errorText(error),
        ok: false,
        phase: "idle",
      }
    }
  }

  const fleetMutate = async (
    request: KhalaCodeDesktopKhalaSyncFleetMutateRequest,
  ): Promise<KhalaCodeDesktopKhalaSyncFleetMutateResult> => {
    if (!enabled || closed) return { ok: false, error: "khala_sync_fleet_disabled" }
    const token = await refreshToken()
    if (token === null) return { ok: false, error: "missing_openagents_auth" }
    const active = await ensureRuntime()
    if (active === null) {
      return { ok: false, error: runtimeFailure ?? "khala_sync_store_unavailable" }
    }
    try {
      await ensureSubscribed(active, fleetRunScope(request.runId))
      switch (request.action) {
        case "set_desired_slots": {
          const desiredSlots = request.desiredSlots
          if (
            desiredSlots === undefined ||
            !Number.isInteger(desiredSlots) ||
            desiredSlots < 0 ||
            desiredSlots > 1024
          ) {
            return { ok: false, error: "desired_slots_out_of_range" }
          }
          await runEffect(
            active.session.mutate(fleetSetDesiredSlotsClientMutator, {
              desiredSlots,
              runId: request.runId,
            }),
          )
          return { ok: true }
        }
        case "pause_worker":
        case "resume_worker": {
          const workerId = request.workerId?.trim()
          if (workerId === undefined || workerId.length === 0) {
            return { ok: false, error: "worker_id_required" }
          }
          await runEffect(
            active.session.mutate(
              request.action === "pause_worker"
                ? fleetPauseWorkerClientMutator
                : fleetResumeWorkerClientMutator,
              { runId: request.runId, workerId },
            ),
          )
          return { ok: true }
        }
        case "acknowledge_inbox_flag": {
          const flagRef = request.flagRef?.trim()
          if (flagRef === undefined || flagRef.length === 0) {
            return { ok: false, error: "flag_ref_required" }
          }
          await runEffect(
            active.session.mutate(fleetAcknowledgeInboxFlagClientMutator, {
              flagRef,
              runId: request.runId,
            }),
          )
          return { ok: true }
        }
        case "stop": {
          // Terminal guard mirrors the server: an unconfirmed stop is a
          // guaranteed `confirmation_required` rejection, so refuse it
          // locally instead of queueing known poison.
          if (request.confirm !== true) {
            return { ok: false, error: "confirm_required" }
          }
          await runEffect(
            active.session.mutate(fleetStopRunClientMutator, {
              confirm: true,
              runId: request.runId,
            }),
          )
          return { ok: true }
        }
        case "pause":
        case "resume": {
          await runEffect(
            active.session.mutate(
              request.action === "pause"
                ? fleetPauseRunClientMutator
                : fleetResumeRunClientMutator,
              { runId: request.runId },
            ),
          )
          return { ok: true }
        }
      }
    } catch (error) {
      return { ok: false, error: errorText(error) }
    }
  }

  const close = async (): Promise<void> => {
    if (closed) return
    closed = true
    const active = runtime
    runtime = null
    subscribed.clear()
    if (active === null) return
    try {
      await runEffect(active.session.close())
    } catch {
      // best-effort shutdown
    }
    try {
      await runEffect(active.store.close())
    } catch {
      // best-effort shutdown
    }
  }

  return {
    chatAppendMessage,
    chatCreateThread,
    chatMessages,
    chatRenameThread,
    chatThreads,
    close,
    fleetMutate,
    fleetState,
  }
}
