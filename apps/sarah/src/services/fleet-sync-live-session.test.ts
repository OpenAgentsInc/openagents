import { describe, expect, test } from "bun:test"
import {
  FLEET_RUN_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  KHALA_SYNC_PROTOCOL_VERSION,
  MustRefetchFrame,
  canonicalJson,
  decodeBootstrapResponse,
  decodeFleetRunEntity,
  decodeFleetWorkerEntity,
  decodeLogPage,
  encodeFleetRunEntity,
  encodeFleetWorkerEntity,
  encodeLiveFrame,
  fleetRunScope,
  type BootstrapResponse,
  type LogPage,
} from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import {
  SarahFleetSyncClientError,
  SARAH_FLEET_CURSOR_STATE_SCHEMA,
  type SarahFleetSyncClient,
  type SarahFleetSyncCursorState,
  type SarahFleetSyncRequestOptions,
} from "./fleet-sync-client.ts"
import {
  SARAH_FLEET_BROWSER_STORAGE_PREFIX,
  makeSarahFleetBrowserPersistence,
  type SarahFleetBrowserStorage,
} from "./fleet-sync-browser-persistence.ts"
import {
  buildSarahFleetLiveConnectUrl,
  makeSarahFleetLiveSession,
  SarahFleetConnectionState,
  type SarahFleetSchedule,
  type SarahFleetWebSocketLike,
} from "./fleet-sync-live-session.ts"
import {
  reduceSarahFleetBootstrapPages,
  type SarahFleetProjectionState,
} from "./fleet-sync-projection-store.ts"

const ORIGIN = "https://openagents.com"
const RUN_REF = "fleet.run.fc3.live"
const SCOPE = fleetRunScope(RUN_REF)
const FOREIGN_RUN_REF = "fleet.run.fc3.foreign"
const FOREIGN_SCOPE = fleetRunScope(FOREIGN_RUN_REF)

const run = decodeFleetRunEntity({
  runId: RUN_REF,
  status: "running",
  desiredSlots: 1,
  workerKind: "codex",
  startedAt: "2026-07-09T20:00:00.000Z",
  counters: {
    workUnitsTotal: 0,
    activeAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    blockedAssignments: 0,
  },
  updatedAt: "2026-07-09T20:00:00.000Z",
})

const worker = decodeFleetWorkerEntity({
  workerId: "worker.fc3.live.codex",
  phase: "idle",
  harnessKind: "codex",
  updatedAt: "2026-07-09T20:00:00.000Z",
})

const bootstrap = (
  runRef = RUN_REF,
  cursor = 10,
): ReadonlyArray<BootstrapResponse> => {
  const selectedRun =
    runRef === RUN_REF
      ? run
      : decodeFleetRunEntity({ ...run, runId: runRef })
  const entities = [
    {
      entityType: FLEET_RUN_ENTITY_TYPE,
      entityId: selectedRun.runId,
      postImageJson: canonicalJson(encodeFleetRunEntity(selectedRun)),
    },
    ...(runRef === RUN_REF
      ? [
          {
            entityType: FLEET_WORKER_ENTITY_TYPE,
            entityId: worker.workerId,
            postImageJson: canonicalJson(encodeFleetWorkerEntity(worker)),
          },
        ]
      : []),
  ]
  return [
    decodeBootstrapResponse({
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      scope: fleetRunScope(runRef),
      entities,
      cursor,
    }),
  ]
}

const projectionState = (
  runRef = RUN_REF,
  cursor = 10,
): SarahFleetProjectionState => reduceSarahFleetBootstrapPages(bootstrap(runRef, cursor))

const cursorState = (
  cursor: number,
  scope = SCOPE,
): SarahFleetSyncCursorState => ({
  schema: SARAH_FLEET_CURSOR_STATE_SCHEMA,
  scope,
  cursor: cursor as SarahFleetSyncCursorState["cursor"],
})

const emptyLog = (cursor: number, scope = SCOPE): LogPage =>
  decodeLogPage({
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    scope,
    entries: [],
    nextCursor: cursor,
    upToDate: true,
  })

class MemoryStorage implements SarahFleetBrowserStorage {
  readonly values = new Map<string, string>()
  readonly writes: string[] = []

  getItem(key: string): string | null {
    return this.values.get(key) ?? null
  }

  setItem(key: string, value: string): void {
    this.writes.push(key)
    this.values.set(key, value)
  }

  removeItem(key: string): void {
    this.values.delete(key)
  }
}

class FakeWebSocket implements SarahFleetWebSocketLike {
  static instances: FakeWebSocket[] = []

  onopen: ((event: unknown) => void) | null = null
  onmessage: ((event: { readonly data: unknown }) => void) | null = null
  onerror: ((event: unknown) => void) | null = null
  onclose: ((event: { readonly code?: number }) => void) | null = null
  readonly sent: string[] = []
  closed = false

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  send(data: string): void {
    if (this.closed) throw new Error("socket closed")
    this.sent.push(data)
  }

  close(): void {
    this.closed = true
  }

  open(): void {
    this.onopen?.({})
  }

  message(value: unknown): void {
    this.onmessage?.({
      data: typeof value === "string" ? value : JSON.stringify(value),
    })
  }

  serverClose(code = 1006): void {
    this.closed = true
    this.onclose?.({ code })
  }
}

class TestScheduler {
  now = 1_000
  private nextId = 1
  private readonly tasks: Array<{
    id: number
    task: () => void
    delayMs: number
    cancelled: boolean
  }> = []

  readonly schedule: SarahFleetSchedule = (task, delayMs) => {
    const entry = {
      id: this.nextId++,
      task,
      delayMs,
      cancelled: false,
    }
    this.tasks.push(entry)
    return () => {
      entry.cancelled = true
    }
  }

  runNext(): boolean {
    const entry = this.tasks
      .filter((task) => !task.cancelled)
      .sort((left, right) => left.delayMs - right.delayMs || left.id - right.id)[0]
    if (entry === undefined) return false
    entry.cancelled = true
    this.now += entry.delayMs
    entry.task()
    return true
  }
}

const flush = async (): Promise<void> => {
  for (let step = 0; step < 10; step += 1) await Promise.resolve()
}

const resumeOnlyClient = (
  resume: (
    state: SarahFleetSyncCursorState,
    options?: SarahFleetSyncRequestOptions,
  ) => ReturnType<SarahFleetSyncClient["resume"]>,
): Pick<SarahFleetSyncClient, "bootstrap" | "resume"> => ({
  bootstrap: async () => {
    throw new Error("bootstrap must not run")
  },
  resume,
})

const readyResume = async (
  state: SarahFleetSyncCursorState,
): ReturnType<SarahFleetSyncClient["resume"]> => ({
  pages: [emptyLog(state.cursor, state.scope)],
  state,
})

const makeReadyHarness = async () => {
  FakeWebSocket.instances = []
  const storage = new MemoryStorage()
  const persistence = makeSarahFleetBrowserPersistence(storage)
  await persistence.save(projectionState())
  const scheduler = new TestScheduler()
  const states: unknown[] = []
  const session = makeSarahFleetLiveSession({
    client: resumeOnlyClient(readyResume),
    persistence,
    origin: ORIGIN,
    webSocket: FakeWebSocket,
    now: () => scheduler.now,
    schedule: scheduler.schedule,
    random: () => 0,
    cadenceMs: 100,
    staleAfterMs: 300,
    connectTimeoutMs: 100,
    backoffBaseMs: 10,
    backoffMaxMs: 20,
    maxReconnectAttempts: 3,
  })
  session.subscribe((state) => states.push(state))
  const started = session.start(SCOPE)
  await flush()
  const socket = FakeWebSocket.instances[0]!
  socket.open()
  await started
  return { persistence, scheduler, session, socket, states, storage }
}

describe("Sarah FC-3 browser fleet persistence", () => {
  test("stores and reopens only one exact-scope allowlisted projection", async () => {
    const storage = new MemoryStorage()
    const persistence = makeSarahFleetBrowserPersistence(storage)
    const state = projectionState()

    await persistence.save(state)
    expect(storage.writes).toEqual([
      `${SARAH_FLEET_BROWSER_STORAGE_PREFIX}${SCOPE}`,
    ])
    expect(await persistence.load(SCOPE)).toEqual(state)
    expect([...storage.values.values()].join("\n")).not.toMatch(
      /token|rawPrompt|commandOutput/,
    )

    await persistence.clear(SCOPE)
    expect(await persistence.load(SCOPE)).toBeNull()
  })

  test("fails closed on corrupt and foreign state without echoing storage contents", async () => {
    const storage = new MemoryStorage()
    const persistence = makeSarahFleetBrowserPersistence(storage)
    const key = `${SARAH_FLEET_BROWSER_STORAGE_PREFIX}${SCOPE}`
    const secret = "PRIVATE STORAGE SECRET SENTINEL"
    storage.values.set(key, `{not-json:${secret}`)
    const corrupt = await persistence.load(SCOPE).catch((error) => error)
    expect(corrupt).toMatchObject({ reason: "invalid_state" })
    expect(JSON.stringify(corrupt)).not.toContain(secret)

    storage.values.set(key, JSON.stringify(projectionState(FOREIGN_RUN_REF)))
    await expect(persistence.load(SCOPE)).rejects.toMatchObject({
      reason: "foreign_scope",
    })
  })
})

describe("Sarah FC-3 bounded live Fleet Sync session", () => {
  test("reopens the saved exact cursor, hydrates, and connects without latest or a token query", async () => {
    const { session, socket } = await makeReadyHarness()
    const url = new URL(socket.url)

    expect(session.snapshot()).toMatchObject({
      phase: "live",
      scope: SCOPE,
      cursor: 10,
    })
    expect(session.projection()?.run.runRef).toBe(RUN_REF)
    expect(url.pathname).toBe("/api/sync/connect")
    expect(url.searchParams.get("scope")).toBe(SCOPE)
    expect(url.searchParams.get("cursor")).toBe("10")
    expect(url.searchParams.has("token")).toBe(false)
    expect(socket.url).not.toContain("latest")
    expect(buildSarahFleetLiveConnectUrl(ORIGIN, SCOPE, 10)).toBe(socket.url)
    session.dispose()
  })

  test("coalesces refresh while one exact-cursor catch-up request is in flight", async () => {
    FakeWebSocket.instances = []
    const storage = new MemoryStorage()
    const persistence = makeSarahFleetBrowserPersistence(storage)
    await persistence.save(projectionState())
    let calls = 0
    let resolveResume: ((value: Awaited<ReturnType<SarahFleetSyncClient["resume"]>>) => void) | undefined
    const resume = (
      _state: SarahFleetSyncCursorState,
    ): ReturnType<SarahFleetSyncClient["resume"]> => {
      calls += 1
      return new Promise((resolve) => {
        resolveResume = resolve
      })
    }
    const scheduler = new TestScheduler()
    const session = makeSarahFleetLiveSession({
      client: resumeOnlyClient(resume),
      persistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => scheduler.now,
      schedule: scheduler.schedule,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
    })

    const first = session.start(SCOPE)
    const second = session.refresh()
    await flush()
    expect(calls).toBe(1)
    resolveResume?.({ pages: [emptyLog(10)], state: cursorState(10) })
    await flush()
    expect(FakeWebSocket.instances).toHaveLength(1)
    FakeWebSocket.instances[0]!.open()
    await Promise.all([first, second])
    expect(calls).toBe(1)
    session.dispose()
  })

  test("aborts an in-flight catch-up and disposes an open socket", async () => {
    FakeWebSocket.instances = []
    const storage = new MemoryStorage()
    const persistence = makeSarahFleetBrowserPersistence(storage)
    await persistence.save(projectionState())
    let observedSignal: AbortSignal | undefined
    const client = resumeOnlyClient((_state, options) => {
      observedSignal = options?.signal
      return new Promise((_resolve, reject) => {
        options?.signal?.addEventListener(
          "abort",
          () => reject(new SarahFleetSyncClientError("request_aborted")),
          { once: true },
        )
      })
    })
    const scheduler = new TestScheduler()
    const session = makeSarahFleetLiveSession({
      client,
      persistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => scheduler.now,
      schedule: scheduler.schedule,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
    })
    const aborter = new AbortController()
    const started = session.start(SCOPE, aborter.signal)
    await flush()
    aborter.abort()
    await started
    expect(observedSignal?.aborted).toBe(true)
    expect(session.snapshot()).toEqual({
      phase: "stopped",
      scope: SCOPE,
      reason: "aborted",
    })

    const ready = await makeReadyHarness()
    ready.session.dispose()
    expect(ready.socket.closed).toBe(true)
    expect(ready.session.snapshot()).toMatchObject({
      phase: "stopped",
      reason: "disposed",
    })
  })

  test("keeps the first start signal as the sole idempotent lifecycle owner", async () => {
    FakeWebSocket.instances = []
    const storage = new MemoryStorage()
    const persistence = makeSarahFleetBrowserPersistence(storage)
    await persistence.save(projectionState())
    const scheduler = new TestScheduler()
    const session = makeSarahFleetLiveSession({
      client: resumeOnlyClient(readyResume),
      persistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => scheduler.now,
      schedule: scheduler.schedule,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
    })
    const owner = new AbortController()
    const nonOwner = new AbortController()
    const first = session.start(SCOPE, owner.signal)
    await flush()
    FakeWebSocket.instances[0]!.open()
    await first
    await session.start(SCOPE, nonOwner.signal)

    nonOwner.abort()
    expect(session.snapshot().phase).toBe("live")
    owner.abort()
    expect(session.snapshot()).toMatchObject({
      phase: "stopped",
      reason: "aborted",
    })
  })

  test("turns a live cursor gap into bounded catch-up instead of applying it", async () => {
    const { session, socket } = await makeReadyHarness()
    const gapWorker = decodeFleetWorkerEntity({
      ...worker,
      phase: "completed",
      updatedAt: "2026-07-09T20:00:02.000Z",
    })
    socket.message({
      _tag: "DeltaFrame",
      scope: SCOPE,
      cursor: 12,
      entries: [
        {
          scope: SCOPE,
          version: 12,
          entityType: FLEET_WORKER_ENTITY_TYPE,
          entityId: worker.workerId,
          op: "upsert",
          postImageJson: canonicalJson(encodeFleetWorkerEntity(gapWorker)),
          committedAt: "2026-07-09T20:00:02.000Z",
        },
      ],
    })
    await flush()

    expect(session.snapshot()).toMatchObject({
      phase: "reconnecting",
      cursor: 10,
      error: { reason: "cursor_no_progress", retryable: true },
    })
    expect(session.projection()?.workers[0]?.phase).toBe("idle")
    session.dispose()
  })

  test("applies and persists an exact next delta once, then reopens at that cursor", async () => {
    const first = await makeReadyHarness()
    const completedWorker = decodeFleetWorkerEntity({
      ...worker,
      phase: "completed",
      updatedAt: "2026-07-09T20:00:01.000Z",
    })
    const delta = {
      _tag: "DeltaFrame",
      scope: SCOPE,
      cursor: 11,
      entries: [
        {
          scope: SCOPE,
          version: 11,
          entityType: FLEET_WORKER_ENTITY_TYPE,
          entityId: worker.workerId,
          op: "upsert",
          postImageJson: canonicalJson(encodeFleetWorkerEntity(completedWorker)),
          committedAt: "2026-07-09T20:00:01.000Z",
        },
      ],
    }
    first.socket.message(delta)
    await flush()
    expect(first.session.snapshot()).toMatchObject({ phase: "live", cursor: 11 })
    expect(first.session.projection()?.workers[0]?.phase).toBe("completed")

    first.socket.message(delta)
    await flush()
    expect(first.session.snapshot()).toMatchObject({ phase: "live", cursor: 11 })
    first.session.dispose()

    FakeWebSocket.instances = []
    const scheduler = new TestScheduler()
    const resumeCursors: number[] = []
    const reopened = makeSarahFleetLiveSession({
      client: resumeOnlyClient(async (state) => {
        resumeCursors.push(state.cursor)
        return { pages: [emptyLog(state.cursor)], state }
      }),
      persistence: first.persistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => scheduler.now,
      schedule: scheduler.schedule,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
    })
    const started = reopened.start(SCOPE)
    await flush()
    FakeWebSocket.instances[0]!.open()
    await started
    expect(resumeCursors).toEqual([11])
    expect(new URL(FakeWebSocket.instances[0]!.url).searchParams.get("cursor")).toBe(
      "11",
    )
    reopened.dispose()
  })

  test("pings on cadence and leaves live after the bounded stale deadline", async () => {
    const { scheduler, session, socket } = await makeReadyHarness()
    expect(scheduler.runNext()).toBe(true)
    expect(socket.sent).toHaveLength(1)
    expect(JSON.parse(socket.sent[0]!)).toEqual({ _tag: "PingFrame" })
    expect(scheduler.runNext()).toBe(true)
    expect(socket.sent).toHaveLength(2)
    expect(scheduler.runNext()).toBe(true)
    await flush()
    expect(session.snapshot()).toMatchObject({
      phase: "reconnecting",
      error: { reason: "socket_stale", retryable: true },
    })
    session.dispose()
  })

  test("recovers from a network error through bounded backoff", async () => {
    FakeWebSocket.instances = []
    const storage = new MemoryStorage()
    const persistence = makeSarahFleetBrowserPersistence(storage)
    await persistence.save(projectionState())
    const scheduler = new TestScheduler()
    let calls = 0
    const session = makeSarahFleetLiveSession({
      client: resumeOnlyClient(async (state) => {
        calls += 1
        if (calls === 1) throw new SarahFleetSyncClientError("network_unavailable")
        return { pages: [emptyLog(state.cursor)], state }
      }),
      persistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => scheduler.now,
      schedule: scheduler.schedule,
      random: () => 0,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
      backoffBaseMs: 10,
      backoffMaxMs: 20,
      maxReconnectAttempts: 3,
    })

    await session.start(SCOPE)
    expect(session.snapshot()).toMatchObject({
      phase: "reconnecting",
      attempt: 1,
      error: { reason: "network_unavailable" },
    })
    expect(scheduler.runNext()).toBe(true)
    await flush()
    expect(calls).toBe(2)
    FakeWebSocket.instances[0]!.open()
    await flush()
    expect(session.snapshot().phase).toBe("live")
    session.dispose()
  })

  test("rejects invalid random samples and exhausts a finite reconnect budget", async () => {
    for (const invalidRandom of [Number.NaN, Number.POSITIVE_INFINITY, -0.1, 1]) {
      const storage = new MemoryStorage()
      const persistence = makeSarahFleetBrowserPersistence(storage)
      await persistence.save(projectionState())
      const session = makeSarahFleetLiveSession({
        client: resumeOnlyClient(async () => {
          throw new SarahFleetSyncClientError("network_unavailable")
        }),
        persistence,
        origin: ORIGIN,
        webSocket: FakeWebSocket,
        random: () => invalidRandom,
        cadenceMs: 100,
        staleAfterMs: 300,
        connectTimeoutMs: 100,
      })
      await session.start(SCOPE)
      expect(session.snapshot()).toMatchObject({
        phase: "failed",
        error: { reason: "protocol_failure", retryable: false },
      })
    }

    const storage = new MemoryStorage()
    const persistence = makeSarahFleetBrowserPersistence(storage)
    await persistence.save(projectionState())
    const scheduler = new TestScheduler()
    const exhausted = makeSarahFleetLiveSession({
      client: resumeOnlyClient(async () => {
        throw new SarahFleetSyncClientError("network_unavailable")
      }),
      persistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => scheduler.now,
      schedule: scheduler.schedule,
      random: () => 0,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
      backoffBaseMs: 10,
      backoffMaxMs: 20,
      maxReconnectAttempts: 2,
    })
    await exhausted.start(SCOPE)
    for (let attempt = 0; attempt < 2; attempt += 1) {
      expect(scheduler.runNext()).toBe(true)
      await flush()
    }
    expect(exhausted.snapshot()).toMatchObject({
      phase: "failed",
      error: { reason: "retry_exhausted", retryable: false },
    })
  })

  test("contains hostile clock failures at socket-open and retry callbacks", async () => {
    FakeWebSocket.instances = []
    const openStorage = new MemoryStorage()
    const openPersistence = makeSarahFleetBrowserPersistence(openStorage)
    await openPersistence.save(projectionState())
    const scheduler = new TestScheduler()
    let openClockReads = 0
    const openSession = makeSarahFleetLiveSession({
      client: resumeOnlyClient(readyResume),
      persistence: openPersistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => {
        openClockReads += 1
        if (openClockReads >= 3) throw new Error("PRIVATE CLOCK OPEN FAILURE")
        return scheduler.now
      },
      schedule: scheduler.schedule,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
    })
    const opening = openSession.start(SCOPE)
    await flush()
    expect(() => FakeWebSocket.instances[0]!.open()).not.toThrow()
    await opening
    expect(openSession.snapshot()).toMatchObject({
      phase: "failed",
      error: { reason: "protocol_failure", retryable: false },
    })
    expect(JSON.stringify(openSession.snapshot())).not.toContain(
      "PRIVATE CLOCK OPEN FAILURE",
    )

    const retryStorage = new MemoryStorage()
    const retryPersistence = makeSarahFleetBrowserPersistence(retryStorage)
    await retryPersistence.save(projectionState())
    let retryClockReads = 0
    const retrySession = makeSarahFleetLiveSession({
      client: resumeOnlyClient(async () => {
        throw new SarahFleetSyncClientError("network_unavailable")
      }),
      persistence: retryPersistence,
      origin: ORIGIN,
      webSocket: FakeWebSocket,
      now: () => {
        retryClockReads += 1
        if (retryClockReads >= 2) throw new Error("PRIVATE CLOCK RETRY FAILURE")
        return 1_000
      },
      random: () => 0,
      cadenceMs: 100,
      staleAfterMs: 300,
      connectTimeoutMs: 100,
    })
    await retrySession.start(SCOPE)
    expect(retrySession.snapshot()).toMatchObject({
      phase: "failed",
      error: { reason: "protocol_failure", retryable: false },
    })
    expect(JSON.stringify(retrySession.snapshot())).not.toContain(
      "PRIVATE CLOCK RETRY FAILURE",
    )
  })

  test("surfaces corrupt and foreign storage as terminal typed states", async () => {
    for (const [stored, expectedReason] of [
      ["{corrupt", "storage_corrupt"],
      [JSON.stringify(projectionState(FOREIGN_RUN_REF)), "foreign_state"],
    ] as const) {
      FakeWebSocket.instances = []
      const storage = new MemoryStorage()
      storage.values.set(`${SARAH_FLEET_BROWSER_STORAGE_PREFIX}${SCOPE}`, stored)
      const persistence = makeSarahFleetBrowserPersistence(storage)
      let clientCalls = 0
      const session = makeSarahFleetLiveSession({
        client: resumeOnlyClient(async (state) => {
          clientCalls += 1
          return { pages: [emptyLog(state.cursor)], state }
        }),
        persistence,
        origin: ORIGIN,
        webSocket: FakeWebSocket,
        cadenceMs: 100,
        staleAfterMs: 300,
        connectTimeoutMs: 100,
      })

      await session.start(SCOPE)
      expect(session.snapshot()).toMatchObject({
        phase: "failed",
        error: { reason: expectedReason, retryable: false },
      })
      expect(clientCalls).toBe(0)
      expect(FakeWebSocket.instances).toHaveLength(0)
    }
  })

  test("surfaces malformed frames and MustRefetch without echoing private data", async () => {
    const { session, socket, states } = await makeReadyHarness()
    const secret = "PRIVATE LIVE FRAME SENTINEL"
    socket.message(JSON.stringify({ _tag: "PingFrame", rawPrompt: secret }))
    await flush()
    expect(session.snapshot()).toMatchObject({
      phase: "reconnecting",
      error: { reason: "protocol_failure" },
    })
    expect(JSON.stringify(session.snapshot())).not.toContain(secret)
    session.dispose()

    const refetch = await makeReadyHarness()
    refetch.socket.message(
      encodeLiveFrame(
        new MustRefetchFrame({ scope: SCOPE, reason: "scope_reset" }),
      ),
    )
    await flush()
    expect(refetch.states).toContainEqual({
      phase: "must_refetch",
      scope: SCOPE,
      cursor: 10,
      reason: "scope_reset",
    })
    expect(refetch.session.snapshot()).toMatchObject({
      phase: "reconnecting",
      mustRefetchReason: "scope_reset",
      error: { reason: "must_refetch" },
    })
    refetch.session.dispose()
    expect(states.length).toBeGreaterThan(0)
  })

  test("rejects non-finite and non-safe generic state counters", () => {
    for (const invalid of [
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.MAX_SAFE_INTEGER + 1,
    ]) {
      expect(() =>
        Schema.decodeUnknownSync(SarahFleetConnectionState)({
          phase: "connecting",
          scope: SCOPE,
          cursor: 10,
          attempt: invalid,
        }),
      ).toThrow()
    }
  })
})
