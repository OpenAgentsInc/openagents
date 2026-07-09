import {
  FleetPublicRef,
  SyncScope,
  fleetRunScope,
} from "@openagentsinc/khala-sync"
import {
  decodeKhalaFleetIntent,
  type ApprovalDecisionValue,
  type FleetRunControlAction,
  type KhalaFleetIntent,
} from "@openagentsinc/khala-fleet-intents"
import { Schema } from "effect"

import type { SarahFleetOwnerProjection } from "../contracts/fleet-owner-projection.ts"
import { makeSarahFleetBrowserPersistence } from "./fleet-sync-browser-persistence.ts"
import {
  makeSarahFleetSyncClient,
  type SarahFleetFetch,
  type SarahFleetSyncClient,
} from "./fleet-sync-client.ts"
import {
  makeSarahFleetLiveSession,
  type SarahFleetConnectionState,
  type SarahFleetLiveSession,
  type SarahFleetWebSocketConstructor,
} from "./fleet-sync-live-session.ts"

export const SARAH_FLEET_RUN_QUERY_PARAMETER = "fleet_run" as const

export type SarahFleetBrowserConfig = Readonly<{
  runRef: typeof FleetPublicRef.Type
  scope: typeof SyncScope.Type
}>

export class SarahFleetBrowserHostError extends Error {
  readonly _tag = "SarahFleetBrowserHostError"
  override readonly name = "SarahFleetBrowserHostError"

  constructor(
    readonly reason:
      | "invalid_run_scope"
      | "command_unavailable"
      | "invalid_command_target",
  ) {
    super(
      reason === "invalid_run_scope"
        ? "Fleet run scope is invalid."
        : reason === "command_unavailable"
          ? "Fleet command is temporarily unavailable."
          : "Fleet command target is invalid.",
    )
  }
}

const hostError = (
  reason: SarahFleetBrowserHostError["reason"],
): SarahFleetBrowserHostError => new SarahFleetBrowserHostError(reason)

/**
 * The retained Sarah URL is currently the only honest run-selection source.
 * It accepts exactly one `fleet_run` public ref, rejects alternate selectors,
 * and derives the Sync scope rather than trusting a second scope claim.
 */
export const parseSarahFleetBrowserConfig = (
  rawUrl: string,
): SarahFleetBrowserConfig | null => {
  let url: URL
  try {
    url = new URL(rawUrl)
  } catch {
    throw hostError("invalid_run_scope")
  }
  const alternateSelectors = [
    "fleet_scope",
    "fleet_run_ref",
    "fleetRun",
    "fleetRunRef",
    "run",
    "runRef",
    "run_ref",
    "scope",
  ]
  if (alternateSelectors.some((key) => url.searchParams.has(key))) {
    throw hostError("invalid_run_scope")
  }
  const values = url.searchParams.getAll(SARAH_FLEET_RUN_QUERY_PARAMETER)
  if (values.length === 0) return null
  if (values.length !== 1) throw hostError("invalid_run_scope")
  try {
    const runRef = Schema.decodeUnknownSync(FleetPublicRef)(values[0])
    const scope = Schema.decodeUnknownSync(SyncScope)(fleetRunScope(runRef))
    return { runRef, scope }
  } catch {
    throw hostError("invalid_run_scope")
  }
}

export type SarahFleetBrowserViewState = Readonly<{
  config: SarahFleetBrowserConfig
  connection: SarahFleetConnectionState
  projection: SarahFleetOwnerProjection | null
}>

export type SarahFleetBrowserCommandReceipt = Awaited<
  ReturnType<SarahFleetSyncClient["submitIntent"]>
>

export type SarahFleetBrowserCommands = Readonly<{
  runControl: (input: Readonly<{
    runRef: string
    action: FleetRunControlAction
  }>) => Promise<SarahFleetBrowserCommandReceipt>
  approvalDecision: (input: Readonly<{
    runRef: string
    approvalRef: string
    decision: ApprovalDecisionValue
  }>) => Promise<SarahFleetBrowserCommandReceipt>
  steer: (input: Readonly<{
    runRef: string
    targetRef: string
    body: string
    bodyRef?: string
  }>) => Promise<SarahFleetBrowserCommandReceipt>
}>

type CommandEntry = {
  readonly cursor: number
  readonly mutationId: number
  readonly intent: KhalaFleetIntent
  promise: Promise<SarahFleetBrowserCommandReceipt> | null
  receipt: SarahFleetBrowserCommandReceipt | null
}

const MAX_SARAH_FLEET_BROWSER_COMMAND_ENTRIES = 256

const exactRef = (raw: string): typeof FleetPublicRef.Type => {
  try {
    return Schema.decodeUnknownSync(FleetPublicRef)(raw)
  } catch {
    throw hostError("invalid_command_target")
  }
}

/**
 * One fresh client id per page/mount permits the mutation counter to begin at
 * one without colliding with a prior browser lifetime. Entries are keyed by
 * exact action target and projection cursor, so a double click shares one
 * mutation while a later command after cursor advance remains distinct.
 */
export const makeSarahFleetBrowserCommands = (input: Readonly<{
  config: SarahFleetBrowserConfig
  client: Pick<SarahFleetSyncClient, "submitIntent">
  cursor: () => number
  now?: () => string
  randomId?: () => string
}>): SarahFleetBrowserCommands => {
  let nextMutationId = 1
  const entries = new Map<string, CommandEntry>()
  let activeKey: string | null = null
  let retryKey: string | null = null
  let retryCommandKey: string | null = null
  const now = input.now ?? (() => new Date().toISOString())
  const randomId = input.randomId ?? (() => crypto.randomUUID())

  const submit = (
    commandKey: string,
    build: (identity: Readonly<{
      intentId: string
      idempotencyKey: string
      createdAt: string
    }>) => unknown,
  ): Promise<SarahFleetBrowserCommandReceipt> => {
    const cursor = input.cursor()
    if (!Number.isSafeInteger(cursor) || cursor < 0) {
      return Promise.reject(hostError("command_unavailable"))
    }
    const key =
      retryKey !== null && retryCommandKey === commandKey
        ? retryKey
        : `${cursor}:${commandKey}`
    const existing = entries.get(key)
    if (existing?.receipt !== null && existing?.receipt !== undefined) {
      return Promise.resolve(existing.receipt)
    }
    if (existing?.promise !== null && existing?.promise !== undefined) {
      return existing.promise
    }
    // Mutation ids are monotone per fresh page client. Never let a later id
    // overtake an unacknowledged or retryable earlier mutation.
    if (
      (activeKey !== null && activeKey !== key) ||
      (retryKey !== null && retryKey !== key)
    ) {
      return Promise.reject(hostError("command_unavailable"))
    }

    let entry = existing
    if (entry === undefined) {
      if (entries.size >= MAX_SARAH_FLEET_BROWSER_COMMAND_ENTRIES) {
        const settledKey = [...entries].find(
          ([, candidate]) => candidate.promise === null,
        )?.[0]
        if (settledKey === undefined) {
          return Promise.reject(hostError("command_unavailable"))
        }
        entries.delete(settledKey)
      }
      const suffix = exactRef(randomId())
      const identity = {
        intentId: exactRef(`intent.sarah.${suffix}`),
        idempotencyKey: exactRef(`idem.sarah.${suffix}`),
        createdAt: now(),
      }
      let intent: KhalaFleetIntent
      try {
        intent = decodeKhalaFleetIntent(build(identity))
      } catch {
        return Promise.reject(hostError("invalid_command_target"))
      }
      entry = {
        cursor,
        mutationId: nextMutationId,
        intent,
        promise: null,
        receipt: null,
      }
      nextMutationId += 1
      entries.set(key, entry)
    }

    const selected = entry
    activeKey = key
    const promise = input.client
      .submitIntent({
        scope: input.config.scope,
        mutationId: selected.mutationId,
        intent: selected.intent,
      })
      .then((receipt) => {
        selected.receipt = receipt
        selected.promise = null
        activeKey = null
        retryKey = null
        retryCommandKey = null
        return receipt
      })
      .catch(() => {
        // Retain the exact mutation and idempotency key for an explicit retry.
        selected.promise = null
        activeKey = null
        retryKey = key
        retryCommandKey = commandKey
        throw hostError("command_unavailable")
      })
    selected.promise = promise
    return promise
  }

  const assertRunRef = (raw: string): typeof FleetPublicRef.Type => {
    const runRef = exactRef(raw)
    if (runRef !== input.config.runRef) {
      throw hostError("invalid_command_target")
    }
    return runRef
  }
  const base = (identity: Readonly<{
    intentId: string
    idempotencyKey: string
    createdAt: string
  }>) => ({
    schema: "khala.fleet_intent.v1" as const,
    ...identity,
    origin: { surface: "web" as const },
    runRef: input.config.runRef,
  })

  return {
    runControl: ({ runRef: rawRunRef, action }) => {
      const runRef = assertRunRef(rawRunRef)
      return submit(`run:${runRef}:${action}`, (identity) => ({
        ...base(identity),
        kind: "fleet_run_control",
        action,
      }))
    },
    approvalDecision: ({ runRef: rawRunRef, approvalRef: rawApprovalRef, decision }) => {
      const runRef = assertRunRef(rawRunRef)
      const approvalRef = exactRef(rawApprovalRef)
      return submit(`approval:${runRef}:${approvalRef}:${decision}`, (identity) => ({
        ...base(identity),
        kind: "approval_decision",
        approvalRef,
        decision,
      }))
    },
    steer: ({ runRef: rawRunRef, targetRef: rawTargetRef, body, bodyRef: rawBodyRef }) => {
      const runRef = assertRunRef(rawRunRef)
      const targetRef = exactRef(rawTargetRef)
      const bodyRef = rawBodyRef === undefined ? undefined : exactRef(rawBodyRef)
      if (body.length < 1 || body.length > 16_384) {
        return Promise.reject(hostError("invalid_command_target"))
      }
      // The private body is used only for the typed push and its in-memory
      // dedupe identity. It never enters view state, persistence, receipts,
      // or errors.
      return submit(`steer:${runRef}:${targetRef}:${bodyRef ?? "inline"}:${body}`, (identity) => ({
        ...base(identity),
        kind: "steer_message",
        targetRef,
        body,
        ...(bodyRef === undefined ? {} : { bodyRef }),
      }))
    },
  }
}

export type SarahFleetBrowserRuntime = Readonly<{
  config: SarahFleetBrowserConfig
  start: () => Promise<void>
  snapshot: () => SarahFleetBrowserViewState
  subscribe: (listener: (state: SarahFleetBrowserViewState) => void) => () => void
  commands: SarahFleetBrowserCommands
  dispose: () => void
}>

export type SarahFleetBrowserCoordinator = Readonly<{
  setConfig: (config: SarahFleetBrowserConfig | null) => void
  current: () => SarahFleetBrowserRuntime | null
  dispose: () => void
}>

/** Owns at most one exact-run runtime and disposes it before any scope swap. */
export const makeSarahFleetBrowserCoordinator = (input: Readonly<{
  makeRuntime: (config: SarahFleetBrowserConfig) => SarahFleetBrowserRuntime
  onState: (state: SarahFleetBrowserViewState | null) => void
}>): SarahFleetBrowserCoordinator => {
  let current: SarahFleetBrowserRuntime | null = null
  let unsubscribe: (() => void) | null = null
  let disposed = false

  const clear = (): void => {
    unsubscribe?.()
    unsubscribe = null
    current?.dispose()
    current = null
  }

  return {
    setConfig: (config) => {
      if (disposed) return
      if (config?.scope === current?.config.scope) return
      clear()
      if (config === null) {
        input.onState(null)
        return
      }
      let runtime: SarahFleetBrowserRuntime
      try {
        runtime = input.makeRuntime(config)
        current = runtime
        unsubscribe = runtime.subscribe((state) => {
          if (current === runtime) input.onState(state)
        })
      } catch {
        clear()
        input.onState(null)
        return
      }
      void runtime.start().catch(() => {
        if (current !== runtime) return
        clear()
        input.onState(null)
      })
    },
    current: () => current,
    dispose: () => {
      if (disposed) return
      disposed = true
      clear()
    },
  }
}

export const makeSarahFleetBrowserRuntime = (input: Readonly<{
  config: SarahFleetBrowserConfig
  origin: string
  fetch: SarahFleetFetch
  storage?: Storage
  webSocket?: SarahFleetWebSocketConstructor
  randomId?: () => string
  makeSession?: (input: Readonly<{
    client: SarahFleetSyncClient
    config: SarahFleetBrowserConfig
  }>) => SarahFleetLiveSession
}>): SarahFleetBrowserRuntime => {
  const randomId = input.randomId ?? (() => crypto.randomUUID())
  const pageClientId = exactRef(randomId())
  const client = makeSarahFleetSyncClient({
    fetch: input.fetch,
    clientGroupId: "sarah.web.fleet.v1",
    clientId: `sarah.web.${pageClientId}`,
  })
  const session =
    input.makeSession?.({ client, config: input.config }) ??
    makeSarahFleetLiveSession({
      client,
      persistence: makeSarahFleetBrowserPersistence(input.storage),
      origin: input.origin,
      ...(input.webSocket === undefined ? {} : { webSocket: input.webSocket }),
    })
  const listeners = new Set<(state: SarahFleetBrowserViewState) => void>()
  let connection = session.snapshot()
  let projection = session.projection()
  let disposed = false

  const snapshot = (): SarahFleetBrowserViewState => ({
    config: input.config,
    connection,
    projection,
  })
  const emit = (): void => {
    const state = snapshot()
    for (const listener of [...listeners]) listener(state)
  }
  const unsubscribeConnection = session.subscribe((next) => {
    connection = next
    emit()
  })
  const unsubscribeProjection = session.subscribeProjection((next) => {
    if (next.run.runRef !== input.config.runRef) {
      session.dispose()
      projection = null
      connection = {
        phase: "failed",
        scope: input.config.scope,
        cursor: null,
        error: {
          reason: "foreign_state",
          messageSafe: "Saved fleet state belongs to another scope.",
          retryable: false,
        },
      }
      emit()
      return
    }
    projection = next
    emit()
  })
  const commands = makeSarahFleetBrowserCommands({
    config: input.config,
    client,
    cursor: () => {
      const current = session.snapshot()
      return "cursor" in current && current.cursor !== null ? current.cursor : 0
    },
    randomId,
  })

  return {
    config: input.config,
    start: () => session.start(input.config.scope),
    snapshot,
    subscribe: (listener) => {
      listeners.add(listener)
      listener(snapshot())
      return () => listeners.delete(listener)
    },
    commands,
    dispose: () => {
      if (disposed) return
      disposed = true
      unsubscribeConnection()
      unsubscribeProjection()
      listeners.clear()
      session.dispose()
    },
  }
}
