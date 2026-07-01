import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
  runKhalaCodeCodexParityLiveSmoke,
} from "../src/bun/codex-parity-live-smoke"
import type {
  CodexAppServerHost,
  CodexAppServerNotification,
  CodexAppServerNotificationHandler,
} from "../src/bun/codex-app-server-client"
import type { KhalaCodeDesktopCodexHarnessStatus } from "../src/shared/rpc"

type RequestRecord = Readonly<{
  method: string
  params: unknown
}>

const readyHarnessStatus = (): KhalaCodeDesktopCodexHarnessStatus => ({
  ok: true,
  app: "Khala Code Desktop",
  auth: {
    accessTokenPresent: true,
    accountIdPresent: true,
    blockerRefs: [],
    refreshTokenPresent: true,
    state: "ready",
  },
  available: true,
  binary: {
    available: true,
    command: "codex",
    error: null,
    source: "PATH",
    version: "codex-cli 1.2.3",
  },
  capability: "codex_harness",
  home: {
    authPath: "/tmp/codex-home/auth.json",
    fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
    path: "/tmp/codex-home",
    role: "main_user_codex_home",
    source: "input",
  },
  observedAt: "2026-07-01T20:00:00.000Z",
  reason: "Codex CLI is installed and the primary user Codex home has auth state.",
  signIn: {
    command: "codex login",
    required: false,
    warning:
      "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
  },
  status: "ready",
})

const unavailableHarnessStatus = (): KhalaCodeDesktopCodexHarnessStatus => ({
  ...readyHarnessStatus(),
  auth: {
    accessTokenPresent: false,
    accountIdPresent: false,
    blockerRefs: ["blocker.codex.credentials_missing"],
    error: "Codex auth.json is missing.",
    refreshTokenPresent: false,
    state: "credentials_missing",
  },
  available: false,
  reason: "Codex auth.json is missing. Run codex login intentionally for the primary user Codex home before using Khala Code chat.",
  signIn: {
    command: "codex login",
    required: true,
    warning:
      "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
  },
  status: "unavailable",
})

function emit(
  subscribers: Set<CodexAppServerNotificationHandler>,
  notification: Omit<CodexAppServerNotification, "receivedAt">,
): void {
  const fullNotification = {
    ...notification,
    receivedAt: "2026-07-01T20:01:00.000Z",
  }
  for (const subscriber of subscribers) subscriber(fullNotification)
}

function createFakeHost(records: RequestRecord[]): CodexAppServerHost {
  const subscribers = new Set<CodexAppServerNotificationHandler>()
  return {
    dispose: () => undefined,
    request: async <Result,>(method: string, params?: unknown): Promise<Result> => {
      records.push({ method, params })
      if (method === "thread/start" || method === "thread/resume") {
        return {
          cwd: "/tmp/khala-code-parity",
          model: "gpt-5.1-codex",
          modelProvider: "openai",
          thread: {
            id: "thread-parity-live",
            status: "running",
          },
        } as Result
      }
      if (method === "turn/start") {
        queueMicrotask(() => {
          emit(subscribers, {
            method: "turn/started",
            params: {
              threadId: "thread-parity-live",
              turn: { id: "turn-parity-live", status: "inProgress" },
            },
          })
        })
        return { turn: { id: "turn-parity-live", status: "inProgress" } } as Result
      }
      if (method === "turn/interrupt") {
        queueMicrotask(() => {
          emit(subscribers, {
            method: "turn/completed",
            params: {
              threadId: "thread-parity-live",
              turn: { id: "turn-parity-live", status: "interrupted" },
            },
          })
        })
        return {} as Result
      }
      throw new Error(`unexpected request ${method}`)
    },
    respondToServerRequest: () => undefined,
    restart: async () => ({
      ok: true,
      action: "restart",
      changed: false,
      status: {
        ok: true,
        app: "Khala Code Desktop",
        adapterVersion: "test",
        codexCommand: "codex",
        codexHome: "/tmp/codex-home",
        diagnostics: [],
        initialized: true,
        initializeResult: {},
        lastError: null,
        pendingRequestCount: 0,
        pid: 123,
        state: "running",
        transport: "stdio",
      },
    }),
    start: async () => ({
      ok: true,
      action: "start",
      changed: false,
      status: {
        ok: true,
        app: "Khala Code Desktop",
        adapterVersion: "test",
        codexCommand: "codex",
        codexHome: "/tmp/codex-home",
        diagnostics: [],
        initialized: true,
        initializeResult: {},
        lastError: null,
        pendingRequestCount: 0,
        pid: 123,
        state: "running",
        transport: "stdio",
      },
    }),
    status: () => ({
      ok: true,
      app: "Khala Code Desktop",
      adapterVersion: "test",
      codexCommand: "codex",
      codexHome: "/tmp/codex-home",
      diagnostics: [],
      initialized: true,
      initializeResult: {},
      lastError: null,
      pendingRequestCount: 0,
      pid: 123,
      state: "running",
      transport: "stdio",
    }),
    stop: async () => ({
      ok: true,
      action: "stop",
      changed: true,
      status: {
        ok: true,
        app: "Khala Code Desktop",
        adapterVersion: "test",
        codexCommand: "codex",
        codexHome: "/tmp/codex-home",
        diagnostics: [],
        initialized: false,
        initializeResult: {},
        lastError: null,
        pendingRequestCount: 0,
        pid: null,
        state: "stopped",
        transport: "stdio",
      },
    }),
    subscribe: handler => {
      subscribers.add(handler)
      return () => {
        subscribers.delete(handler)
      }
    },
  }
}

describe("Khala Code Codex parity live smoke", () => {
  test("skips clearly unless live Codex parity smoke is explicitly requested", async () => {
    let inspected = false
    const result = await runKhalaCodeCodexParityLiveSmoke({
      inspectHarness: async () => {
        inspected = true
        return readyHarnessStatus()
      },
      requireLive: false,
    })

    expect(inspected).toBe(false)
    expect(result).toMatchObject({
      harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
      ok: true,
      required: false,
      skipped: true,
      status: "skipped",
    })
    expect(result.reason).toContain("KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE=1")
  })

  test("fails loudly when live smoke is required but Codex is unavailable", async () => {
    const result = await runKhalaCodeCodexParityLiveSmoke({
      inspectHarness: async () => unavailableHarnessStatus(),
      requireLive: true,
    })

    expect(result).toMatchObject({
      ok: false,
      required: true,
      skipped: false,
      status: "failed",
    })
    expect(result.reason).toContain("Explicit live Codex parity smoke requested")
    expect(result.reason).toContain("Codex auth.json is missing")
  })

  test("runs a Codex app-server thread, cached resume, turn, and interrupt path with a fake host", async () => {
    const records: RequestRecord[] = []
    const result = await runKhalaCodeCodexParityLiveSmoke({
      createHost: () => createFakeHost(records),
      inspectHarness: async () => readyHarnessStatus(),
      interruptAfterMs: 10,
      requireLive: true,
      workingDirectory: "/tmp/khala-code-parity",
    })

    expect(result).toMatchObject({
      codexTurnId: "turn-parity-live",
      harness: KHALA_CODE_CODEX_PARITY_LIVE_SMOKE_HARNESS,
      ok: true,
      required: true,
      resumedThreadId: "thread-parity-live",
      skipped: false,
      status: "ok",
      threadId: "thread-parity-live",
      turnStatus: "interrupted",
    })
    expect(records.map(record => record.method)).toEqual([
      "thread/start",
      "turn/start",
      "turn/interrupt",
    ])
  })
})
