import { Effect } from "effect"
import { createHash } from "node:crypto"
import { join } from "node:path"
import {
  bundledCodexExecutableSha256,
} from "@openagentsinc/codex-app-server-protocol/compatibility"
import { bundledCodex01441ProtocolManifest } from "@openagentsinc/codex-app-server-protocol/parity"

import {
  openCodexAppServerClient,
  type CodexAppServerClient,
  type CodexAppServerMessage,
  type CodexAppServerProtocolMessage,
  type CodexAppServerRequest,
  type CodexAppServerSpawn,
} from "./codex-app-server-client.ts"
import {
  makeCodexNativeEventPlane,
  type CodexCompatibilityReceipt,
  type CodexNativeEnvelope,
  type CodexNativeEventPlane,
  type CodexNativeJournalEntry,
} from "./codex-native-event-plane.ts"
import {
  makeCodexReverseRpcArbiter,
  denyCodexReverseRpc,
  type CodexReverseRpcArbiter,
  type CodexReverseRpcAttention,
  type CodexReverseRpcReceipt,
} from "./codex-reverse-rpc-arbiter.ts"

export type CodexAppServerPoolTarget = Readonly<{
  binary: string
  binarySha256?: string
  env: NodeJS.ProcessEnv
  cwd: string
  spawnImpl?: CodexAppServerSpawn
  requestTimeoutMs?: number
  /** Stable account identity. Credential material must never be used here. */
  accountRef: string | null
  /** Stable local/remote host identity selected by the main process. */
  hostTarget: string
}>

export type CodexAppServerPoolIdentity = Readonly<{
  binary: string
  binarySha256: string
  codexHome: string | null
  accountRef: string | null
  hostTarget: string
}>

export const codexAppServerPoolIdentity = (
  target: CodexAppServerPoolTarget,
): CodexAppServerPoolIdentity => ({
  binary: target.binary,
  binarySha256: target.binarySha256 ?? bundledCodexExecutableSha256,
  codexHome: target.env.CODEX_HOME ?? null,
  accountRef: target.accountRef,
  hostTarget: target.hostTarget,
})

export const codexAppServerPoolKey = (target: CodexAppServerPoolTarget): string => {
  const identity = codexAppServerPoolIdentity(target)
  return JSON.stringify([
    identity.binary,
    identity.binarySha256,
    identity.codexHome,
    identity.accountRef,
    identity.hostTarget,
  ])
}

export type CodexAppServerSupervisorState =
  | Readonly<{ status: "ready"; generation: number }>
  | Readonly<{ status: "degraded"; generation: number; attempt: number; reason: string }>
  | Readonly<{ status: "repairing"; generation: number; attempt: number; maxAttempts: number }>
  | Readonly<{ status: "closed"; generation: number }>

export type CodexAppServerNotification = Readonly<{
  generation: number
  message: CodexAppServerMessage
}>

export type CodexAppServerReconciliation = Readonly<{
  generation: number
  threadId: string
  response: unknown
}>

const reverseMethods = bundledCodex01441ProtocolManifest.members
  .filter(member => member.direction === "server-request")
  .map(member => member.method)

/** Exact generated bundled-0.144.1 server-request method union. */
export type CodexAppServerReverseMethod = typeof reverseMethods[number]

export type CodexAppServerReverseHandler = (
  request: CodexAppServerRequest,
) => Promise<unknown> | unknown

export type CodexAppServerReverseMethodRegistry = Readonly<
  Record<CodexAppServerReverseMethod, CodexAppServerReverseHandler>
>

/** A complete, method-shaped, deny-only registry suitable for unattended startup. */
export const denyCodexAppServerReverseRequests: CodexAppServerReverseMethodRegistry =
  Object.freeze(Object.fromEntries(reverseMethods.map(method => [
    method,
    (): unknown => denyCodexReverseRpc(method),
  ])) as unknown as CodexAppServerReverseMethodRegistry)

export class CodexAppServerSupervisorError extends Error {
  readonly _tag = "CodexAppServerSupervisorError"
  override readonly name = "CodexAppServerSupervisorError"

  constructor(
    readonly reason: "closed" | "not_ready" | "reconnect_exhausted" | "unsafe_reverse_request" | "unsupported_reverse_request",
    message: string,
  ) { super(message) }
}

export type SupervisedCodexAppServerClient = CodexAppServerClient

export type CodexAppServerClientFactoryInput = Readonly<{
  target: CodexAppServerPoolTarget
  identity: CodexAppServerPoolIdentity
  generation: number
  /** Installed by the supervisor before initialize is invoked. */
  onServerRequest: (request: CodexAppServerRequest) => Promise<unknown>
  /** Complete generated 0.144.1 server-request inventory, available before initialize. */
  reverseMethodRegistry: CodexAppServerReverseMethodRegistry
  /** Installed at client construction so process death cannot race initialization. */
  onClose: (error: Error) => void
  onProtocolMessage: (message: CodexAppServerProtocolMessage) => void
  strictGeneratedDecoding: boolean
}>

export type CodexAppServerClientFactory = (
  input: CodexAppServerClientFactoryInput,
) => SupervisedCodexAppServerClient | Promise<SupervisedCodexAppServerClient>

export type CodexAppServerLease = Readonly<{
  key: string
  identity: CodexAppServerPoolIdentity
  state: () => CodexAppServerSupervisorState
  request: (method: string, params: unknown, options?: Readonly<{ signal?: AbortSignal }>) => Promise<unknown>
  notify: (method: string, params: unknown) => Promise<void>
  subscribe: (listener: (notification: CodexAppServerNotification) => void) => () => void
  subscribeCompatibility: (listener: (receipt: CodexCompatibilityReceipt) => void) => () => void
  nativeEnvelopes: (filter?: Readonly<{ threadId?: string; turnId?: string; itemId?: string; method?: string }>) => ReadonlyArray<CodexNativeEnvelope>
  compatibilityReceipts: () => ReadonlyArray<CodexCompatibilityReceipt>
  nativeJournal: () => ReadonlyArray<CodexNativeJournalEntry>
  registerVisibleThread: (
    threadId: string,
    onReconciled?: (reconciliation: CodexAppServerReconciliation) => void,
  ) => () => void
  /** Install before thread/start; returned thread/turn ids are bound to this lease automatically. */
  registerReverseHandler: (handler: CodexAppServerReverseHandler) => () => void
  release: () => void
}>

export type CodexAppServerSupervisor = Readonly<{
  acquire: (target: CodexAppServerPoolTarget) => Promise<CodexAppServerLease>
  state: (target: CodexAppServerPoolTarget) => CodexAppServerSupervisorState | null
  subscribeState: (listener: (
    identity: CodexAppServerPoolIdentity,
    state: CodexAppServerSupervisorState,
  ) => void) => () => void
  subscribeReverseRpcAttention: (listener: (attention: CodexReverseRpcAttention) => void) => () => void
  reverseRpcReceipts: () => ReadonlyArray<CodexReverseRpcReceipt>
  close: () => void
}>

type LeaseRecord = {
  closed: boolean
  notifications: Set<(notification: CodexAppServerNotification) => void>
  compatibility: Set<(receipt: CodexCompatibilityReceipt) => void>
  visibleThreads: Map<string, Set<(reconciliation: CodexAppServerReconciliation) => void>>
  reverseHandler: CodexAppServerReverseHandler | null
  threadIds: Set<string>
  turnIds: Set<string>
}

type Connection = {
  readonly key: string
  readonly target: CodexAppServerPoolTarget
  readonly identity: CodexAppServerPoolIdentity
  readonly leases: Set<LeaseRecord>
  readonly visibleThreads: Map<string, Set<(reconciliation: CodexAppServerReconciliation) => void>>
  readonly nativePlane: CodexNativeEventPlane
  generation: number
  state: CodexAppServerSupervisorState
  client: SupervisedCodexAppServerClient | null
  removeNotificationListener: (() => void) | null
  repair: Promise<void> | null
  closed: boolean
}

const errorMessage = (error: unknown): string => error instanceof Error ? error.message : String(error)

const defaultFactory: CodexAppServerClientFactory = ({
  target,
  onServerRequest,
  onClose,
  onProtocolMessage,
  strictGeneratedDecoding,
}) =>
  openCodexAppServerClient({
    binary: target.binary,
    env: target.env,
    cwd: target.cwd,
    onServerRequest,
    onClose,
    onProtocolMessage,
    strictGeneratedDecoding,
    ...(target.spawnImpl === undefined ? {} : { spawnImpl: target.spawnImpl }),
    ...(target.requestTimeoutMs === undefined ? {} : { requestTimeoutMs: target.requestTimeoutMs }),
  })

const defaultSleep = (milliseconds: number): Promise<void> =>
  Effect.runPromise(Effect.sleep(milliseconds))

export const createCodexAppServerSupervisor = (options: Readonly<{
  clientFactory?: CodexAppServerClientFactory
  reverseHandlers?: Partial<CodexAppServerReverseMethodRegistry>
  maxReconnectAttempts?: number
  reconnectBackoffMs?: (attempt: number) => number
  sleep?: (milliseconds: number) => Promise<void>
  nativeJournalRoot?: string
  strictGeneratedDecoding?: boolean
  reverseRpcJournalPath?: string
  reverseRpcTimeoutMs?: number
}> = {}): CodexAppServerSupervisor => {
  const factory = options.clientFactory ?? defaultFactory
  const maxReconnectAttempts = Math.max(0, Math.floor(options.maxReconnectAttempts ?? 3))
  const reconnectBackoffMs = options.reconnectBackoffMs ?? (attempt => Math.min(1_000, 25 * (2 ** (attempt - 1))))
  const sleep = options.sleep ?? defaultSleep
  const connections = new Map<string, Connection>()
  const reverseRpcArbiter: CodexReverseRpcArbiter = makeCodexReverseRpcArbiter({
    ...(options.reverseRpcJournalPath === undefined ? {} : { journalPath: options.reverseRpcJournalPath }),
    ...(options.reverseRpcTimeoutMs === undefined ? {} : { timeoutMs: options.reverseRpcTimeoutMs }),
  })
  const stateListeners = new Set<(
    identity: CodexAppServerPoolIdentity,
    state: CodexAppServerSupervisorState,
  ) => void>()
  let closed = false

  const setState = (connection: Connection, state: CodexAppServerSupervisorState): void => {
    connection.state = state
    for (const listener of stateListeners) {
      try { listener(connection.identity, state) } catch { /* isolate observers */ }
    }
  }

  const closeClient = (connection: Connection): void => {
    connection.removeNotificationListener?.()
    connection.removeNotificationListener = null
    const client = connection.client
    connection.client = null
    client?.close()
  }

  const closeConnection = (connection: Connection): void => {
    if (connection.closed) return
    connection.closed = true
    closeClient(connection)
    setState(connection, { status: "closed", generation: connection.generation })
    connections.delete(connection.key)
  }

  const reverseRegistry: CodexAppServerReverseMethodRegistry = Object.freeze(Object.fromEntries(
    reverseMethods.map(method => [
      method,
      options.reverseHandlers?.[method] ?? denyCodexAppServerReverseRequests[method],
    ]),
  ))

  const onServerRequest = async (connection: Connection, request: CodexAppServerRequest): Promise<unknown> => {
    if (!reverseMethods.includes(request.method as CodexAppServerReverseMethod)) {
      throw new CodexAppServerSupervisorError(
        "unsupported_reverse_request",
        `Unsupported Codex server request: ${request.method}`,
      )
    }
    const params = request.params !== null && typeof request.params === "object"
      ? request.params as Record<string, unknown>
      : {}
    const threadId = typeof params.threadId === "string" ? params.threadId : null
    const turnId = typeof params.turnId === "string" ? params.turnId : null
    const routed = [...connection.leases].filter(lease => !lease.closed && lease.reverseHandler !== null && (
      (threadId !== null && lease.threadIds.has(threadId)) ||
      (turnId !== null && lease.turnIds.has(turnId))
    ))
    const preStart = routed.length === 0 && threadId === null && turnId === null
      ? [...connection.leases].filter(lease => !lease.closed && lease.reverseHandler !== null)
      : []
    const method = request.method as CodexAppServerReverseMethod
    const privateHandler = options.reverseHandlers?.[method]
    const privileged = method === "item/permissions/requestApproval" ||
      method === "mcpServer/elicitation/request" ||
      method === "account/chatgptAuthTokens/refresh" ||
      method === "attestation/generate" ||
      method === "currentTime/read"
    const leaseHandlers = privileged ? [] : (routed.length > 0 ? routed : preStart)
      .map(lease => lease.reverseHandler!)
    const proposers = [
      ...(privateHandler === undefined ? [] : [privateHandler]),
      ...leaseHandlers,
    ]
    return reverseRpcArbiter.arbitrate({
      connectionKey: connection.key,
      generation: connection.generation,
      request,
      proposers,
    })
  }

  const visibleThreads = (connection: Connection): ReadonlyArray<Readonly<{
    threadId: string
    listeners: ReadonlyArray<(reconciliation: CodexAppServerReconciliation) => void>
  }>> => {
    return [...connection.visibleThreads].map(([threadId, listeners]) => ({
      threadId,
      listeners: [...listeners],
    }))
  }

  const connect = async (connection: Connection, attempt: number): Promise<void> => {
    if (closed || connection.closed) throw new CodexAppServerSupervisorError("closed", "Codex app-server supervisor is closed")
    closeClient(connection)
    const generation = ++connection.generation
    setState(connection, { status: "repairing", generation, attempt, maxAttempts: maxReconnectAttempts })

    let client: SupervisedCodexAppServerClient | null = null
    client = await factory({
      target: connection.target,
      identity: connection.identity,
      generation,
      onServerRequest: request => onServerRequest(connection, request),
      reverseMethodRegistry: reverseRegistry,
      onClose: error => {
        if (generation !== connection.generation || connection.client !== client) return
        void beginRepair(connection, error)
      },
      onProtocolMessage: message => {
        if (generation !== connection.generation) return
        connection.nativePlane.accept({ generation, requestId: message.requestId, decoded: message.decoded })
      },
      strictGeneratedDecoding: options.strictGeneratedDecoding === true,
    })
    if (closed || connection.closed || generation !== connection.generation) {
      client.close()
      throw new CodexAppServerSupervisorError("closed", "Codex app-server connection was superseded")
    }
    connection.client = client
    connection.removeNotificationListener = client.onNotification(message => {
      if (closed || connection.closed || generation !== connection.generation || connection.client !== client) return
      const notification = { generation, message }
      for (const lease of connection.leases) {
        if (!lease.closed) for (const listener of lease.notifications) {
          try { listener(notification) } catch { /* isolate observers */ }
        }
      }
    })
    // The factory receives the complete reverse registry before this call.
    await client.initialize()
    for (const visible of visibleThreads(connection)) {
      const response = await client.request("thread/resume", { threadId: visible.threadId })
      if (generation !== connection.generation || connection.client !== client) return
      const reconciliation = { generation, threadId: visible.threadId, response }
      for (const listener of visible.listeners) listener(reconciliation)
    }
    if (generation === connection.generation && connection.client === client) {
      setState(connection, { status: "ready", generation })
    }
  }

  const beginRepair = (connection: Connection, cause: unknown): Promise<void> => {
    if (closed || connection.closed) return Promise.reject(
      new CodexAppServerSupervisorError("closed", "Codex app-server supervisor is closed"),
    )
    if (connection.repair !== null) return connection.repair
    setState(connection, {
      status: "degraded",
      generation: connection.generation,
      attempt: 0,
      reason: errorMessage(cause),
    })
    closeClient(connection)
    const repair = (async (): Promise<void> => {
      let lastError = cause
      for (let attempt = 1; attempt <= maxReconnectAttempts; attempt += 1) {
        await sleep(Math.max(0, reconnectBackoffMs(attempt)))
        try {
          await connect(connection, attempt)
          return
        } catch (error) {
          lastError = error
          closeClient(connection)
          if (closed || connection.closed) throw error
          setState(connection, {
            status: "degraded",
            generation: connection.generation,
            attempt,
            reason: errorMessage(error),
          })
        }
      }
      throw new CodexAppServerSupervisorError(
        "reconnect_exhausted",
        `Codex app-server reconnect attempts exhausted: ${errorMessage(lastError)}`,
      )
    })()
    connection.repair = repair
    void repair.catch(() => undefined).finally(() => {
      if (connection.repair === repair) connection.repair = null
    })
    return repair
  }

  const request = async (
    connection: Connection,
    method: string,
    params: unknown,
    options?: Readonly<{ signal?: AbortSignal }>,
  ): Promise<unknown> => {
    if (closed || connection.closed) throw new CodexAppServerSupervisorError("closed", "Codex app-server lease is closed")
    if (connection.state.status !== "ready" || connection.client === null) {
      if (connection.repair === null) {
        throw new CodexAppServerSupervisorError("not_ready", "Codex app-server is not ready")
      }
      await connection.repair
    }
    const client = connection.client
    if (connection.state.status !== "ready" || client === null) {
      throw new CodexAppServerSupervisorError("not_ready", "Codex app-server is not ready")
    }
    try {
      return await client.request(method, params, options)
    } catch (error) {
      // The failed operation is deliberately not retained or replayed.
      if (client.isClosed()) void beginRepair(connection, error)
      throw error
    }
  }

  const acquire = async (target: CodexAppServerPoolTarget): Promise<CodexAppServerLease> => {
    if (closed) throw new CodexAppServerSupervisorError("closed", "Codex app-server supervisor is closed")
    const key = codexAppServerPoolKey(target)
    let connection = connections.get(key)
    if (connection === undefined) {
      const journalPath = options.nativeJournalRoot === undefined
        ? undefined
        : join(options.nativeJournalRoot, `${createHash("sha256").update(key).digest("hex")}.json`)
      let createdConnection: Connection | null = null
      const nativePlane = makeCodexNativeEventPlane({
        ...(journalPath === undefined ? {} : { journalPath }),
        onCompatibilityReceipt: receipt => {
          if (createdConnection === null) return
          for (const lease of createdConnection.leases) {
            if (!lease.closed) for (const listener of lease.compatibility) {
              try { listener(receipt) } catch { /* isolate observers */ }
            }
          }
        },
      })
      connection = {
        key,
        target,
        identity: codexAppServerPoolIdentity(target),
        leases: new Set(),
        visibleThreads: new Map(),
        nativePlane,
        generation: 0,
        state: { status: "repairing", generation: 0, attempt: 0, maxAttempts: maxReconnectAttempts },
        client: null,
        removeNotificationListener: null,
        repair: null,
        closed: false,
      }
      createdConnection = connection
      connections.set(key, connection)
      const initial = connect(connection, 0)
      connection.repair = initial
      try {
        await initial
      } catch (error) {
        connection.repair = null
        try {
          await beginRepair(connection, error)
        } catch {
          closeConnection(connection)
          throw error
        }
      } finally {
        if (connection.repair === initial) connection.repair = null
      }
    } else if (connection.repair !== null) {
      await connection.repair
    }

    const ownedConnection = connection
    const record: LeaseRecord = {
      closed: false,
      notifications: new Set(),
      compatibility: new Set(),
      visibleThreads: new Map(),
      reverseHandler: null,
      threadIds: new Set(),
      turnIds: new Set(),
    }
    ownedConnection.leases.add(record)
    let released = false
    const assertOpen = (): void => {
      if (released || record.closed || ownedConnection.closed || closed) {
        throw new CodexAppServerSupervisorError("closed", "Codex app-server lease is closed")
      }
    }
    return {
      key,
      identity: ownedConnection.identity,
      state: () => ownedConnection.state,
      request: async (method, params, requestOptions) => {
        assertOpen()
        const response = await request(ownedConnection, method, params, requestOptions)
        if (response !== null && typeof response === "object") {
          const value = response as Record<string, unknown>
          const thread = value.thread !== null && typeof value.thread === "object"
            ? value.thread as Record<string, unknown>
            : null
          const turn = value.turn !== null && typeof value.turn === "object"
            ? value.turn as Record<string, unknown>
            : null
          if (typeof thread?.id === "string") record.threadIds.add(thread.id)
          if (typeof turn?.id === "string") record.turnIds.add(turn.id)
          if (method === "thread/resume" && params !== null && typeof params === "object" &&
            typeof (params as Record<string, unknown>).threadId === "string") {
            record.threadIds.add((params as { threadId: string }).threadId)
          }
        }
        return response
      },
      notify: async (method, params) => {
        assertOpen()
        if (ownedConnection.state.status !== "ready" || ownedConnection.client === null) {
          throw new CodexAppServerSupervisorError("not_ready", "Codex app-server is not ready")
        }
        const client = ownedConnection.client
        try {
          await client.notify(method, params)
        } catch (error) {
          if (client.isClosed()) void beginRepair(ownedConnection, error)
          throw error
        }
      },
      subscribe: listener => {
        assertOpen()
        record.notifications.add(listener)
        let subscribed = true
        return () => {
          if (!subscribed) return
          subscribed = false
          record.notifications.delete(listener)
        }
      },
      subscribeCompatibility: listener => {
        assertOpen()
        record.compatibility.add(listener)
        let subscribed = true
        return () => {
          if (!subscribed) return
          subscribed = false
          record.compatibility.delete(listener)
        }
      },
      nativeEnvelopes: filter => ownedConnection.nativePlane.envelopes(filter),
      compatibilityReceipts: () => ownedConnection.nativePlane.receipts(),
      nativeJournal: () => ownedConnection.nativePlane.journal(),
      registerVisibleThread: (threadId, onReconciled) => {
        assertOpen()
        const normalized = threadId.trim()
        if (normalized === "") throw new TypeError("threadId must not be empty")
        const listeners = ownedConnection.visibleThreads.get(normalized) ?? new Set()
        if (onReconciled !== undefined) listeners.add(onReconciled)
        ownedConnection.visibleThreads.set(normalized, listeners)
        record.threadIds.add(normalized)
        let registered = true
        return () => {
          if (!registered) return
          registered = false
          if (onReconciled !== undefined) listeners.delete(onReconciled)
          if (listeners.size === 0) ownedConnection.visibleThreads.delete(normalized)
        }
      },
      registerReverseHandler: handler => {
        assertOpen()
        if (record.reverseHandler !== null) throw new TypeError("reverse handler already registered for lease")
        record.reverseHandler = handler
        let registered = true
        return () => {
          if (!registered) return
          registered = false
          if (record.reverseHandler === handler) record.reverseHandler = null
        }
      },
      release: () => {
        if (released) return
        released = true
        record.closed = true
        record.notifications.clear()
        record.compatibility.clear()
        record.visibleThreads.clear()
        record.reverseHandler = null
        record.threadIds.clear()
        record.turnIds.clear()
        ownedConnection.leases.delete(record)
      },
    }
  }

  return {
    acquire,
    state: target => connections.get(codexAppServerPoolKey(target))?.state ?? null,
    subscribeState: listener => {
      if (closed) throw new CodexAppServerSupervisorError("closed", "Codex app-server supervisor is closed")
      stateListeners.add(listener)
      return () => stateListeners.delete(listener)
    },
    subscribeReverseRpcAttention: listener => reverseRpcArbiter.subscribe(listener),
    reverseRpcReceipts: () => reverseRpcArbiter.receipts(),
    close: () => {
      if (closed) return
      closed = true
      for (const connection of [...connections.values()]) {
        for (const lease of connection.leases) {
          lease.closed = true
          lease.notifications.clear()
          lease.compatibility.clear()
          lease.visibleThreads.clear()
          lease.reverseHandler = null
          lease.threadIds.clear()
          lease.turnIds.clear()
        }
        connection.leases.clear()
        connection.visibleThreads.clear()
        closeConnection(connection)
      }
      stateListeners.clear()
      reverseRpcArbiter.close()
    },
  }
}
