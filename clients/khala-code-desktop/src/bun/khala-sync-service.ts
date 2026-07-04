import {
  ClientGroupId,
  ClientId,
  canonicalJson,
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
  SyncSchemaVersion,
  type SyncScope,
} from "@openagentsinc/khala-sync"
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
import { Cause, Effect, Exit } from "effect"
import { homedir } from "node:os"
import { dirname, join } from "node:path"
import { mkdirSync } from "node:fs"
import type {
  KhalaCodeDesktopKhalaSyncFleetMutateRequest,
  KhalaCodeDesktopKhalaSyncFleetMutateResult,
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

export const khalaCodeDesktopKhalaSyncFleetEnabled = (env: ServiceEnv): boolean => {
  const value = env[KHALA_SYNC_FLEET_FLAG_ENV]?.trim().toLowerCase()
  return value === "1" || value === "true"
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

const MAX_TRACKED_REJECTIONS = 20

interface SessionRuntime {
  readonly store: KhalaSyncSqliteStore
  readonly overlay: KhalaSyncOverlay
  readonly session: KhalaSyncSession
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
  const baseUrl = khalaSyncBaseUrl(env)
  const now = options.now ?? (() => new Date())

  let runtime: SessionRuntime | null = null
  let runtimeFailure: string | null = null
  let currentToken: string | null = null
  let closed = false
  const subscribed = new Set<SyncScope>()
  const rejections: Array<KhalaCodeDesktopKhalaSyncFleetRejection> = []
  let lastTransportError: string | null = null

  const recordRejection = (rejection: KhalaCodeDesktopKhalaSyncFleetRejection): void => {
    rejections.push(rejection)
    if (rejections.length > MAX_TRACKED_REJECTIONS) {
      rejections.splice(0, rejections.length - MAX_TRACKED_REJECTIONS)
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
      const overlay = await runEffect(createOverlay(store, [...fleetClientMutators]))
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
            let runId: string | null = null
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
              } catch {
                runId = null
              }
            }
            recordRejection({
              errorCode: result.errorCode ?? "rejected",
              messageSafe: result.errorMessageSafe ?? "mutation rejected by the server",
              mutationId: result.mutationId,
              mutatorName: mutation?.name ?? "unknown",
              observedAt: now().toISOString(),
              runId,
            })
          },
          onTransportError: (_context, error) => {
            lastTransportError = errorText(error)
          },
        },
      )
      runtime = { store, overlay, session }
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
        // auth). Surfaced honestly — never rendered as any syncing state.
        return { phase: "denied", cursor: null, reason: state.reason }
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

  return { fleetState, fleetMutate, close }
}
