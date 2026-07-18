import { describe, expect, test } from "vite-plus/test"

import { CodexAppServerError, type CodexAppServerMessage } from "./codex-app-server-client.ts"
import {
  codexAppServerPoolKey,
  createCodexAppServerSupervisor,
  type CodexAppServerClientFactory,
  type CodexAppServerClientFactoryInput,
  type CodexAppServerPoolTarget,
} from "./codex-app-server-supervisor.ts"

const target = (overrides: Partial<CodexAppServerPoolTarget> = {}): CodexAppServerPoolTarget => ({
  binary: "/Applications/OpenAgents.app/codex",
  binarySha256: "fixture-codex-sha256",
  env: { CODEX_HOME: "/accounts/codex-primary" },
  cwd: "/workspace",
  accountRef: "codex-primary",
  hostTarget: "local-mac",
  ...overrides,
})

const waitFor = async (condition: () => boolean): Promise<void> => {
  for (let attempt = 0; attempt < 100 && !condition(); attempt += 1) await Promise.resolve()
  expect(condition()).toBe(true)
}

type FakeRequest = Readonly<{ method: string; params: unknown }>

class FakeClient {
  readonly requests: FakeRequest[] = []
  readonly notifications: FakeRequest[] = []
  readonly capturedNotificationListeners: Array<(message: CodexAppServerMessage) => void> = []
  initializeCount = 0
  closeCount = 0
  closed = false
  failInitialize: Error | null = null
  failClosedMethod: string | null = null
  deferUntilCloseMethod: string | null = null
  private rejectDeferred: ((error: Error) => void) | null = null
  reverseBeforeInitialize: unknown = null
  private readonly notificationListeners = new Set<(message: CodexAppServerMessage) => void>()

  constructor(readonly input: CodexAppServerClientFactoryInput) {}

  async initialize(): Promise<void> {
    this.initializeCount += 1
    this.reverseBeforeInitialize = await this.input.onServerRequest({
      id: `approval-${this.input.generation}`,
      method: "item/commandExecution/requestApproval",
      params: {},
    })
    if (this.failInitialize !== null) throw this.failInitialize
  }

  async request(method: string, params: unknown, options: Readonly<{ signal?: AbortSignal }> = {}): Promise<unknown> {
    if (options.signal?.aborted === true) throw new CodexAppServerError("cancelled", `${method} cancelled`)
    this.requests.push({ method, params })
    if (method === this.deferUntilCloseMethod) {
      return await new Promise<never>((_resolve, reject) => { this.rejectDeferred = reject })
    }
    if (method === this.failClosedMethod) {
      const error = new CodexAppServerError("closed", "fake app-server crashed")
      this.closed = true
      this.input.onClose(error)
      throw error
    }
    if (method === "thread/resume") return { thread: { id: (params as { threadId: string }).threadId } }
    return { ok: true, generation: this.input.generation }
  }

  async notify(method: string, params: unknown): Promise<void> {
    if (this.closed) throw new CodexAppServerError("closed", "fake app-server is closed")
    this.notifications.push({ method, params })
    if (method === this.deferUntilCloseMethod) {
      await new Promise<never>((_resolve, reject) => { this.rejectDeferred = reject })
    }
  }

  onNotification(listener: (message: CodexAppServerMessage) => void): () => void {
    this.notificationListeners.add(listener)
    this.capturedNotificationListeners.push(listener)
    return () => this.notificationListeners.delete(listener)
  }

  emit(message: CodexAppServerMessage): void {
    for (const listener of this.notificationListeners) listener(message)
  }

  emitCaptured(message: CodexAppServerMessage): void {
    for (const listener of this.capturedNotificationListeners) listener(message)
  }

  crash(): void {
    if (this.closed) return
    this.closed = true
    this.input.onClose(new CodexAppServerError("closed", "fake app-server crashed"))
  }

  isClosed(): boolean { return this.closed }

  deferredRequestReady(): boolean { return this.rejectDeferred !== null }

  close(): void {
    if (this.closed) return
    this.closed = true
    this.closeCount += 1
    this.input.onClose(new CodexAppServerError("closed", "fake app-server closed"))
    this.rejectDeferred?.(new CodexAppServerError("closed", "fake app-server closed"))
    this.rejectDeferred = null
  }
}

const fakeFactory = (configure?: (client: FakeClient) => void): Readonly<{
  clients: FakeClient[]
  factory: CodexAppServerClientFactory
}> => {
  const clients: FakeClient[] = []
  const factory: CodexAppServerClientFactory = input => {
    const client = new FakeClient(input)
    configure?.(client)
    clients.push(client)
    return client
  }
  return { clients, factory }
}

describe("CAP-01 Codex app-server supervisor", () => {
  test("pools two simultaneous thread leases and initializes their shared connection once", async () => {
    const fake = fakeFactory()
    const supervisor = createCodexAppServerSupervisor({ clientFactory: fake.factory })
    const [first, second] = await Promise.all([supervisor.acquire(target()), supervisor.acquire(target())])

    expect(fake.clients).toHaveLength(1)
    expect(fake.clients[0]?.initializeCount).toBe(1)
    expect(first.key).toBe(second.key)
    expect(first.state()).toEqual({ status: "ready", generation: 1 })

    const firstSeen: unknown[] = []
    const secondSeen: unknown[] = []
    first.subscribe(notification => firstSeen.push(notification))
    second.subscribe(notification => secondSeen.push(notification))
    fake.clients[0]?.emit({ method: "turn/completed", params: { threadId: "thread-1" } })
    expect(firstSeen).toEqual(secondSeen)
    expect(firstSeen).toHaveLength(1)

    expect(codexAppServerPoolKey(target({ accountRef: "other" }))).not.toBe(first.key)
    expect(codexAppServerPoolKey(target({ env: { CODEX_HOME: "/other" } }))).not.toBe(first.key)
    expect(codexAppServerPoolKey(target({ binary: "/other/codex" }))).not.toBe(first.key)
    expect(codexAppServerPoolKey(target({ binarySha256: "other-hash" }))).not.toBe(first.key)
    expect(codexAppServerPoolKey(target({ hostTarget: "remote-mac" }))).not.toBe(first.key)
    first.release()
    second.release()
    const laterTurn = await supervisor.acquire(target())
    expect(fake.clients).toHaveLength(1)
    expect(fake.clients[0]?.initializeCount).toBe(1)
    expect(fake.clients[0]?.closeCount).toBe(0)
    laterTurn.release()
    await supervisor.acquire(target({ env: { CODEX_HOME: "/other" } }))
    expect(fake.clients).toHaveLength(2)
    supervisor.close()
  })

  test("installs the complete deny registry before initialize", async () => {
    const fake = fakeFactory()
    const supervisor = createCodexAppServerSupervisor({ clientFactory: fake.factory })
    const lease = await supervisor.acquire(target())

    expect(fake.clients[0]?.reverseBeforeInitialize).toEqual({ decision: "decline" })
    expect(Object.keys(fake.clients[0]!.input.reverseMethodRegistry).sort()).toEqual([
      "account/chatgptAuthTokens/refresh",
      "applyPatchApproval",
      "attestation/generate",
      "currentTime/read",
      "execCommandApproval",
      "item/commandExecution/requestApproval",
      "item/fileChange/requestApproval",
      "item/permissions/requestApproval",
      "item/tool/call",
      "item/tool/requestUserInput",
      "mcpServer/elicitation/request",
    ])
    await expect(fake.clients[0]?.input.onServerRequest({
      id: "question",
      method: "item/tool/requestUserInput",
      params: {},
    })).resolves.toEqual({ answers: {} })
    await expect(fake.clients[0]?.input.onServerRequest({
      id: "tokens",
      method: "account/chatgptAuthTokens/refresh",
      params: {},
    })).rejects.toMatchObject({ reason: "authority_unavailable" })
    await expect(fake.clients[0]?.input.onServerRequest({
      id: "permissions",
      method: "item/permissions/requestApproval",
      params: { threadId: "thread-1", turnId: "turn-1", reason: "test", permissions: {} },
    })).resolves.toEqual({ permissions: {} })
    await expect(fake.clients[0]?.input.onServerRequest({
      id: "time",
      method: "currentTime/read",
      params: { threadId: "thread-1" },
    })).resolves.toEqual({ currentTimeAt: Math.floor(Date.now() / 1_000) })
    await expect(fake.clients[0]?.input.onServerRequest({
      id: "unknown",
      method: "future/reverseMethod",
      params: {},
    })).rejects.toMatchObject({ reason: "unsupported_reverse_request" })
    lease.release()
  })

  test("rejects notifications captured from a stale generation", async () => {
    const fake = fakeFactory()
    const supervisor = createCodexAppServerSupervisor({
      clientFactory: fake.factory,
      sleep: async () => undefined,
    })
    const lease = await supervisor.acquire(target())
    const seen: unknown[] = []
    lease.subscribe(notification => seen.push(notification))

    fake.clients[0]?.crash()
    await waitFor(() => fake.clients.length === 2 && lease.state().status === "ready")
    fake.clients[0]?.emitCaptured({ method: "item/started", params: { stale: true } })
    fake.clients[1]?.emit({ method: "item/started", params: { stale: false } })

    expect(seen).toEqual([{
      generation: 2,
      message: { method: "item/started", params: { stale: false } },
    }])
    supervisor.close()
  })

  test("routes reverse requests to their owning thread lease and denies an unmatched route", async () => {
    const fake = fakeFactory()
    const supervisor = createCodexAppServerSupervisor({ clientFactory: fake.factory })
    const first = await supervisor.acquire(target())
    const second = await supervisor.acquire(target())
    first.registerVisibleThread("thread-1")
    second.registerVisibleThread("thread-2")
    let firstCalls = 0
    let secondCalls = 0
    first.registerReverseHandler(() => { firstCalls += 1; return { decision: "accept" } })
    second.registerReverseHandler(() => { secondCalls += 1; return { decision: "acceptForSession" } })

    await expect(fake.clients[0]?.input.onServerRequest({
      id: "approval-1",
      method: "item/commandExecution/requestApproval",
      params: { threadId: "thread-1", turnId: "turn-1" },
    })).resolves.toEqual({ decision: "accept" })
    await expect(fake.clients[0]?.input.onServerRequest({
      id: "approval-2",
      method: "item/commandExecution/requestApproval",
      params: { threadId: "thread-2", turnId: "turn-2" },
    })).resolves.toEqual({ decision: "acceptForSession" })
    await expect(fake.clients[0]?.input.onServerRequest({
      id: "approval-unmatched",
      method: "item/commandExecution/requestApproval",
      params: { threadId: "thread-other" },
    })).resolves.toEqual({ decision: "decline" })
    expect({ firstCalls, secondCalls }).toEqual({ firstCalls: 1, secondCalls: 1 })
    supervisor.close()
  })

  test("reconnects after a crash, resumes two visible threads, and never replays a mutating request", async () => {
    const fake = fakeFactory()
    const supervisor = createCodexAppServerSupervisor({
      clientFactory: fake.factory,
      sleep: async () => undefined,
    })
    const first = await supervisor.acquire(target())
    const second = await supervisor.acquire(target())
    const reconciled: unknown[] = []
    first.registerVisibleThread("thread-1", receipt => reconciled.push(receipt))
    second.registerVisibleThread("thread-2", receipt => reconciled.push(receipt))
    fake.clients[0]!.failClosedMethod = "turn/start"

    await expect(first.request("turn/start", { threadId: "thread-1", input: "mutating" }))
      .rejects.toMatchObject({ reason: "closed" })
    await waitFor(() => fake.clients.length === 2 && first.state().status === "ready")

    expect(fake.clients[0]?.requests.filter(request => request.method === "turn/start")).toHaveLength(1)
    expect(fake.clients[1]?.requests).toEqual([
      { method: "thread/resume", params: { threadId: "thread-1" } },
      { method: "thread/resume", params: { threadId: "thread-2" } },
    ])
    expect(reconciled).toMatchObject([
      { generation: 2, threadId: "thread-1" },
      { generation: 2, threadId: "thread-2" },
    ])
    supervisor.close()
  })

  test("bounds reconnect attempts and leaves a typed degraded state", async () => {
    const backoffs: number[] = []
    const fake = fakeFactory(client => {
      if (client.input.generation > 1) client.failInitialize = new Error("initialize unavailable")
    })
    const supervisor = createCodexAppServerSupervisor({
      clientFactory: fake.factory,
      maxReconnectAttempts: 2,
      reconnectBackoffMs: attempt => attempt * 10,
      sleep: async milliseconds => { backoffs.push(milliseconds) },
    })
    const lease = await supervisor.acquire(target())
    fake.clients[0]?.crash()
    await waitFor(() => fake.clients.length === 3 && lease.state().status === "degraded")

    expect(backoffs).toEqual([10, 20])
    expect(lease.state()).toMatchObject({ status: "degraded", generation: 3, attempt: 2 })
    await expect(lease.request("thread/read", { threadId: "thread-1" }))
      .rejects.toMatchObject({ reason: "reconnect_exhausted" })
    supervisor.close()
  })

  test("clean shutdown closes one pooled client and fences every lease", async () => {
    const fake = fakeFactory()
    const supervisor = createCodexAppServerSupervisor({ clientFactory: fake.factory })
    const states: string[] = []
    supervisor.subscribeState((_identity, state) => states.push(state.status))
    const first = await supervisor.acquire(target())
    const second = await supervisor.acquire(target())

    supervisor.close()
    supervisor.close()

    expect(fake.clients[0]?.closeCount).toBe(1)
    expect(first.state()).toEqual({ status: "closed", generation: 1 })
    expect(second.state()).toEqual({ status: "closed", generation: 1 })
    expect(states.at(-1)).toBe("closed")
    await expect(first.request("thread/read", {})).rejects.toMatchObject({ reason: "closed" })
    await expect(second.notify("thread/compact", {})).rejects.toMatchObject({ reason: "closed" })
  })

  test("closing with an in-flight request or notification never leaks a background repair rejection", async () => {
    const unhandled: unknown[] = []
    const onUnhandled = (error: unknown): void => { unhandled.push(error) }
    process.on("unhandledRejection", onUnhandled)
    try {
      for (const operation of ["request", "notify"] as const) {
        const fake = fakeFactory()
        const supervisor = createCodexAppServerSupervisor({ clientFactory: fake.factory })
        const lease = await supervisor.acquire(target())
        fake.clients[0]!.deferUntilCloseMethod = "skills/list"
        const pending = (operation === "request"
          ? lease.request("skills/list", {})
          : lease.notify("skills/list", {}))
          .then(() => null, error => error)
        await waitFor(() => fake.clients[0]!.deferredRequestReady())

        supervisor.close()
        await expect(pending).resolves.toMatchObject({ reason: "closed" })
        await new Promise<void>(resolve => setImmediate(resolve))
        await new Promise<void>(resolve => setImmediate(resolve))
      }
      expect(unhandled).toEqual([])
    } finally {
      process.off("unhandledRejection", onUnhandled)
    }
  })
})
