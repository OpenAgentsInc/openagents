import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createKhalaCodeDesktopRpcRequestHandlers } from "../src/bun/rpc-handlers"
import type { CodexAppServerChatRuntime } from "../src/bun/codex-app-server-chat-runtime"
import type {
  CodexAppServerHost,
  CodexAppServerNotificationHandler,
} from "../src/bun/codex-app-server-client"
import type {
  KhalaCodexFleetCommandInput,
  KhalaCodexFleetCommandResult,
} from "../src/bun/khala-codex-fleet-tools"
import type {
  KhalaCodeDesktopChatTurnResponse,
  KhalaCodeDesktopCodexAppServerControlResult,
  KhalaCodeDesktopCodexAppServerStatus,
  KhalaCodeDesktopCodexHarnessStatus,
  KhalaCodeDesktopFleetRunProjection,
} from "../src/shared/rpc"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempPylonFixture(): Promise<{
  readonly appPath: string
  readonly env: Record<string, string>
  readonly home: string
}> {
  const root = await mkdtemp(join(tmpdir(), "khala-code-rpc-fleet-"))
  tempDirs.push(root)
  const appPath = join(root, "apps", "pylon")
  const home = join(root, "pylon-home")
  await mkdir(appPath, { recursive: true })
  await mkdir(home, { recursive: true })
  await writeFile(join(appPath, "package.json"), JSON.stringify({ name: "@openagentsinc/pylon" }))
  return {
    appPath,
    env: {
      OPENAGENTS_BUN_PATH: process.execPath,
      OPENAGENTS_PYLON_APP_PATH: appPath,
      PYLON_HOME: home,
    },
    home,
  }
}

function ok(stdout: unknown): KhalaCodexFleetCommandResult {
  return {
    exitCode: 0,
    signal: null,
    stderr: "",
    stdout: typeof stdout === "string" ? stdout : JSON.stringify(stdout),
    timedOut: false,
  }
}

function failed(stderr: string): KhalaCodexFleetCommandResult {
  return {
    exitCode: 1,
    signal: null,
    stderr,
    stdout: "",
    timedOut: false,
  }
}

function pylonArgs(input: KhalaCodexFleetCommandInput): readonly string[] {
  const index = input.cmd.indexOf("src/index.ts")
  return index === -1 ? input.cmd : input.cmd.slice(index + 1)
}

function readyHarness(
  input: Partial<KhalaCodeDesktopCodexHarnessStatus> = {},
): KhalaCodeDesktopCodexHarnessStatus {
  return {
    ok: true,
    app: "Khala Code Desktop",
    available: true,
    capability: "codex_harness",
    observedAt: "2026-07-01T15:00:00.000Z",
    reason: "ready",
    status: "ready",
    binary: {
      command: "codex",
      source: "PATH",
      available: true,
      version: "codex-cli 1.2.3",
      error: null,
    },
    home: {
      path: "/home/user/.codex",
      source: "default:~/.codex",
      role: "main_user_codex_home",
      authPath: "/home/user/.codex/auth.json",
      fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
    },
    auth: {
      state: "ready",
      blockerRefs: [],
      accessTokenPresent: true,
      accountIdPresent: false,
      refreshTokenPresent: false,
    },
    signIn: {
      required: false,
      command: "codex login",
      warning: "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
    },
    ...input,
  }
}

const stoppedAppServerStatus = (): KhalaCodeDesktopCodexAppServerStatus => ({
  ok: true,
  app: "Khala Code Desktop",
  adapterVersion: "test-adapter",
  codexCommand: "codex",
  codexHome: "/home/user/.codex",
  diagnostics: [],
  initialized: false,
  initializeResult: null,
  lastError: null,
  pendingRequestCount: 0,
  pid: null,
  state: "stopped",
  transport: "stdio",
})

const fleetRunProjection = (
  input: Partial<KhalaCodeDesktopFleetRunProjection> = {},
): KhalaCodeDesktopFleetRunProjection => ({
  counters: {
    activeAssignments: 0,
    blockedAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    workUnitsTotal: 25,
  },
  createdAt: "2026-07-01T12:00:00.000Z",
  dispatchKind: "supervised_dispatch",
  objectiveProjected: false,
  pylonRef: "pylon.owner",
  refillPolicy: {
    cooldownAware: true,
    maxPerAccount: 1,
    stopCondition: "backlog_empty",
  },
  runRef: "fleet_run.test",
  startedAt: "2026-07-01T12:00:00.000Z",
  state: "running",
  targetConcurrency: 25,
  updatedAt: "2026-07-01T12:00:00.000Z",
  workerKind: "codex",
  workSource: {
    kind: "issue_list",
    repo: "OpenAgentsInc/openagents",
    issues: [7832],
  },
  ...input,
})

function codexAppServerHost(
  request: CodexAppServerHost["request"],
): CodexAppServerHost {
  return {
    dispose: () => undefined,
    request,
    respondToServerRequest: () => undefined,
    restart: async () => ({
      ok: true,
      action: "restart",
      changed: false,
      status: stoppedAppServerStatus(),
    }),
    start: async () => ({
      ok: true,
      action: "start",
      changed: false,
      status: stoppedAppServerStatus(),
    }),
    status: stoppedAppServerStatus,
    stop: async () => ({
      ok: true,
      action: "stop",
      changed: false,
      status: stoppedAppServerStatus(),
    }),
    subscribe: () => () => undefined,
  }
}

function throwingCodexChatRuntime(
  overrides: Partial<CodexAppServerChatRuntime> = {},
): CodexAppServerChatRuntime {
  return {
    archiveThread: async () => {
      throw new Error("codex archive should not be called")
    },
    compactThread: async () => {
      throw new Error("codex compact should not be called")
    },
    deleteThread: async () => {
      throw new Error("codex delete should not be called")
    },
    forkThread: async () => {
      throw new Error("codex fork should not be called")
    },
    interruptTurn: async () => {
      throw new Error("codex interrupt should not be called")
    },
    listThreads: async () => {
      throw new Error("codex list should not be called")
    },
    resumeThread: async () => {
      throw new Error("codex resume should not be called")
    },
    readThread: async () => {
      throw new Error("codex read should not be called")
    },
    renameThread: async () => {
      throw new Error("codex rename should not be called")
    },
    startThread: async () => {
      throw new Error("codex start should not be called")
    },
    startTurn: async () => {
      throw new Error("codex turn should not be called")
    },
    steerTurn: async () => {
      throw new Error("codex steer should not be called")
    },
    threadIdForSession: async () => null,
    unarchiveThread: async () => {
      throw new Error("codex unarchive should not be called")
    },
    ...overrides,
  }
}

describe("Khala Code desktop RPC handlers", () => {
  test("starts fleet runs through the supervisor RPC port", async () => {
    const calls: unknown[] = []
    const run = fleetRunProjection()
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      fleetRunSupervisor: {
        control: async () => {
          throw new Error("not used")
        },
        list: async () => {
          throw new Error("not used")
        },
        start: async request => {
          calls.push(request)
          return { run, supervisorStarted: true }
        },
        status: async () => {
          throw new Error("not used")
        },
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.fleetRunStart({
      objective: "Burn down public issue backlog.",
      targetConcurrency: 25,
      workSource: {
        kind: "issue_list",
        repo: "OpenAgentsInc/openagents",
        issues: [7832],
      },
    })).resolves.toEqual({
      ok: true,
      run,
      supervisorStarted: true,
    })
    expect(calls).toHaveLength(1)
  })

  test("reads fleet run status through a public-safe projection", async () => {
    const run = fleetRunProjection({
      counters: {
        activeAssignments: 3,
        blockedAssignments: 1,
        completedAssignments: 20,
        failedAssignments: 1,
        workUnitsTotal: 25,
      },
      state: "draining",
    })
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      fleetRunSupervisor: {
        control: async () => {
          throw new Error("not used")
        },
        list: async () => {
          throw new Error("not used")
        },
        start: async () => {
          throw new Error("not used")
        },
        status: async request => ({
          run: request.runRef === run.runRef ? run : null,
          supervisorActive: true,
        }),
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    const result = await handlers.fleetRunStatus({ runRef: run.runRef })

    expect(result).toEqual({ ok: true, run, supervisorActive: true })
    expect(result.run).not.toHaveProperty("objective")
  })

  test("controls fleet run pause resume drain and stop transitions through the supervisor", async () => {
    const calls: string[] = []
    let current = fleetRunProjection({ state: "running" })
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      fleetRunSupervisor: {
        control: async request => {
          calls.push(request.verb)
          const previousState = current.state
          const nextState = request.verb === "pause"
            ? "paused"
            : request.verb === "resume"
              ? "running"
              : request.verb === "drain"
                ? "draining"
                : "stopped"
          current = fleetRunProjection({ state: nextState })
          return { previousState, run: current, supervisorActive: nextState === "running" }
        },
        list: async () => {
          throw new Error("not used")
        },
        start: async () => {
          throw new Error("not used")
        },
        status: async () => ({ run: current, supervisorActive: current.state === "running" }),
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.fleetRunControl({ runRef: current.runRef, verb: "pause" }))
      .resolves.toMatchObject({ previousState: "running", run: { state: "paused" }, verb: "pause" })
    await expect(handlers.fleetRunControl({ runRef: current.runRef, verb: "resume" }))
      .resolves.toMatchObject({ previousState: "paused", run: { state: "running" }, verb: "resume" })
    await expect(handlers.fleetRunControl({ runRef: current.runRef, verb: "drain" }))
      .resolves.toMatchObject({ previousState: "running", run: { state: "draining" }, verb: "drain" })
    await expect(handlers.fleetRunControl({ runRef: current.runRef, verb: "stop" }))
      .resolves.toMatchObject({ previousState: "draining", run: { state: "stopped" }, verb: "stop" })

    expect(calls).toEqual(["pause", "resume", "drain", "stop"])
  })

  test("delegates fleet run control transition authority to the supervisor", async () => {
    const run = fleetRunProjection({ state: "completed" })
    let mutated = false
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      fleetRunSupervisor: {
        control: async () => {
          mutated = true
          return { previousState: "completed", run, supervisorActive: false }
        },
        list: async () => [run],
        start: async () => ({ run, supervisorStarted: false }),
        status: async () => ({ run, supervisorActive: false }),
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.fleetRunControl({ runRef: run.runRef, verb: "pause" }))
      .resolves.toMatchObject({ previousState: "completed", run, verb: "pause" })
    expect(mutated).toBe(true)
  })

  test("rejects fleet run RPCs when the supervisor is unconfigured", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.fleetRunStatus({ runRef: "fleet_run.missing" }))
      .rejects.toThrow("Fleet run supervisor is not configured.")
  })

  test("rejects empty fleet run objectives and non-integer target concurrency", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      fleetRunSupervisor: {
        control: async () => {
          throw new Error("not used")
        },
        list: async () => [],
        start: async () => {
          throw new Error("not used")
        },
        status: async () => ({ run: null, supervisorActive: false }),
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.fleetRunStart({
      objective: " ",
      targetConcurrency: 1,
      workSource: { kind: "fixture", count: 1 },
    })).rejects.toThrow("fleetRunStart requires objective")
    await expect(handlers.fleetRunStart({
      objective: "Run fixture work.",
      targetConcurrency: 1.5,
      workSource: { kind: "fixture", count: 1 },
    })).rejects.toThrow("fleetRunStart requires positive integer targetConcurrency")
  })

  test("lists fleet runs through the supervisor RPC port", async () => {
    const running = fleetRunProjection({ runRef: "fleet_run.running", state: "running" })
    const paused = fleetRunProjection({ runRef: "fleet_run.paused", state: "paused" })
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      fleetRunSupervisor: {
        control: async () => {
          throw new Error("not used")
        },
        list: async request => [running, paused].filter(run => request?.state === undefined || run.state === request.state),
        start: async () => {
          throw new Error("not used")
        },
        status: async () => {
          throw new Error("not used")
        },
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.fleetRunList({ state: "paused" })).resolves.toEqual({
      ok: true,
      runs: [paused],
    })
  })

  test("answers native desktop status probes instead of falling through", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexRateLimitStatus: () => ({
        provider: "codex",
        session: {
          usedPercent: 20,
          remainingPercent: 80,
          windowMinutes: 300,
          resetsAtIso: "2026-06-30T03:00:00.000Z",
          resetDescription: "10:00 PM",
        },
        weekly: {
          usedPercent: 40,
          remainingPercent: 60,
          windowMinutes: 10080,
          resetsAtIso: null,
          resetDescription: null,
        },
        rateLimitResetCredits: {
          availableCount: 1,
          nextExpiresAtIso: "2026-07-01T03:00:00.000Z",
        },
        updatedAtIso: "2026-06-29T19:00:00.000Z",
        error: null,
        status: "ok",
      }),
      codexHarnessStatus: () => readyHarness(),
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>() => ({} as Result),
        respondToServerRequest: () => undefined,
        restart: async (): Promise<KhalaCodeDesktopCodexAppServerControlResult> => ({
          ok: true,
          action: "restart",
          changed: true,
          status: { ...stoppedAppServerStatus(), state: "running", initialized: true, pid: 400 },
        }),
        start: async (): Promise<KhalaCodeDesktopCodexAppServerControlResult> => ({
          ok: true,
          action: "start",
          changed: true,
          status: { ...stoppedAppServerStatus(), state: "running", initialized: true, pid: 400 },
        }),
        status: () => stoppedAppServerStatus(),
        stop: async (): Promise<KhalaCodeDesktopCodexAppServerControlResult> => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.codingStatus()).resolves.toMatchObject({
      available: true,
      capability: "coding",
      ok: true,
      status: "ready",
    })
    await expect(handlers.codexAppServerStatus()).resolves.toMatchObject({
      adapterVersion: "test-adapter",
      state: "stopped",
      transport: "stdio",
    })
    await expect(handlers.codexAppServerStart()).resolves.toMatchObject({
      action: "start",
      ok: true,
      status: {
        state: "running",
        initialized: true,
      },
    })
    await expect(handlers.codexHarnessStatus()).resolves.toMatchObject({
      available: true,
      capability: "codex_harness",
      home: {
        role: "main_user_codex_home",
        fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
      },
      ok: true,
      status: "ready",
    })
    const pylonStatus = await handlers.pylonStatus()
    expect(pylonStatus).toMatchObject({
      capability: "pylon",
      ok: true,
    })
    expect(["ready", "unavailable"]).toContain(pylonStatus.status)
    expect(typeof pylonStatus.available).toBe("boolean")
    await expect(handlers.codexAccountsStatus()).resolves.toMatchObject({
      available: true,
      accounts: [
        {
          accountRef: "default",
          credentialSource: "default_home",
          homeRole: "main_user_codex_home",
          provider: "codex",
          readiness: {
            state: "ready",
            blockerRefs: [],
          },
        },
      ],
      capability: "codex_accounts",
      harness: {
        capability: "codex_harness",
        available: true,
      },
      ok: true,
      rateLimits: {
        provider: "codex",
        session: {
          usedPercent: 20,
          windowMinutes: 300,
        },
        rateLimitResetCredits: {
          availableCount: 1,
        },
      },
      status: "ready",
    })
    await expect(handlers.tokenAccountingStatus()).resolves.toMatchObject({
      available: true,
      capability: "token_accounting",
      ok: true,
      reason: expect.stringContaining("stored locally"),
      status: "ready",
    })
  })

  test("surfaces provider reset-credit outcomes through RPC", async () => {
    const handler = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexRateLimitStatus: () => ({
        provider: "codex",
        session: null,
        weekly: null,
        rateLimitResetCredits: {
          availableCount: 0,
          nextExpiresAtIso: null,
        },
        updatedAtIso: "2026-06-29T19:00:00.000Z",
        error: null,
        status: "ok",
      }),
      codexHarnessStatus: () => readyHarness({
        home: {
          path: "/tmp/codex-home",
          source: "env:CODEX_HOME",
          role: "main_user_codex_home",
          authPath: "/tmp/codex-home/auth.json",
          fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
        },
      }),
      consumeCodexRateLimitResetCredit: input => {
        expect(input.accountRef).toBe("default")
        expect(input.codexHomePath).toBeNull()
        expect(input.idempotencyKey).toBeTruthy()
        return "noCredit"
      },
      env: { CODEX_HOME: "/tmp/codex-home" },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handler.consumeCodexRateLimitResetCredit({ accountRef: "default" })).resolves.toMatchObject({
      ok: true,
      outcome: "noCredit",
      status: {
        available: true,
        capability: "codex_accounts",
        accounts: [
          {
            credentialSource: "CODEX_HOME",
            homeRef: "env:CODEX_HOME",
          },
        ],
      },
    })
  })

  test("does not fetch rate limits when the Codex harness is not ready", async () => {
    const blockedHarness = readyHarness({
      available: false,
      reason: "Codex auth.json is missing. Run codex login intentionally for the primary user Codex home before using Khala Code chat.",
      status: "unavailable",
      auth: {
        state: "credentials_missing",
        blockerRefs: ["blocker.codex.credentials_missing"],
        accessTokenPresent: false,
        accountIdPresent: false,
        refreshTokenPresent: false,
        error: "Codex auth.json is missing.",
      },
      signIn: {
        required: true,
        command: "codex login",
        warning: "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
      },
    })
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexHarnessStatus: () => blockedHarness,
      codexRateLimitStatus: () => {
        throw new Error("rate limits should not be fetched")
      },
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.codingStatus()).resolves.toMatchObject({
      available: false,
      capability: "coding",
      status: "unavailable",
    })
    await expect(handlers.codexAccountsStatus()).resolves.toMatchObject({
      available: false,
      status: "unavailable",
      accounts: [{
        readiness: {
          state: "credentials_missing",
          blockerRefs: ["blocker.codex.credentials_missing"],
        },
      }],
      rateLimits: {
        status: "unavailable",
      },
    })
  })

  test("routes chat submits to the Codex app-server runtime by default", async () => {
    let codexTurnStarted = false
    let legacyTurnStarted = false
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime({
        startTurn: async request => {
          codexTurnStarted = true
          expect(request.cwd).toBe(process.cwd())
          expect(request.threadId).toBe("thread-active")
          return {
            backend: {
              kind: "codex_app_server",
              model: "gpt-5.1-codex",
              threadId: "thread-codex-default",
              turnId: "turn-codex-default",
            },
            messages: [{ id: "agent-1", role: "assistant", body: "Codex default path" }],
            ok: true,
            toolNames: [],
            usedTools: [],
          }
        },
      }),
      env: {},
      legacyChatTurn: async (): Promise<KhalaCodeDesktopChatTurnResponse> => {
        legacyTurnStarted = true
        throw new Error("legacy runtime should not be the default")
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.submitChatMessage({
      messages: [{ id: "user-1", role: "user", body: "Run tests" }],
      sessionId: "desktop-session-1",
      threadId: "thread-active",
      turnId: "desktop-turn-1",
    })).resolves.toMatchObject({
      backend: {
        kind: "codex_app_server",
        runtimeMode: "codex_harness",
        threadId: "thread-codex-default",
        toolCatalogKind: "codex_app_server",
      },
      messages: [{ body: "Codex default path" }],
      ok: true,
    })
    expect(codexTurnStarted).toBe(true)
    expect(legacyTurnStarted).toBe(false)
  })

  test("routes explicit Codex turn starts through the selected Codex chat runtime", async () => {
    let codexTurnStarted = false
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime({
        startTurn: async request => {
          codexTurnStarted = true
          expect(request.cwd).toBe("/workspace/project")
          return {
            backend: {
              kind: "codex_app_server",
              model: "gpt-5.1-codex",
              threadId: request.threadId,
              turnId: "turn-selected-codex",
            },
            messages: [{ id: "agent-selected-codex", role: "assistant", body: "selected codex" }],
            ok: true,
            toolNames: [],
            usedTools: [],
          }
        },
      }),
      env: { KHALA_CODE_DESKTOP_RUNTIME: "codex_harness" },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/workspace/project",
    })

    await expect(handlers.codexTurnStart({
      messages: [{ id: "user-selected-codex", role: "user", body: "Run selected Codex" }],
      sessionId: "desktop-session-selected-codex",
      threadId: "thread-selected-codex",
      turnId: "desktop-turn-selected-codex",
    })).resolves.toMatchObject({
      backend: {
        kind: "codex_app_server",
        threadId: "thread-selected-codex",
      },
      messages: [{ body: "selected codex" }],
      ok: true,
    })
    expect(codexTurnStarted).toBe(true)
  })

  test("returns a typed unsupported response for the Claude runtime seam", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: { KHALA_CODE_DESKTOP_RUNTIME: "claude_runtime" },
      legacyChatTurn: async (): Promise<KhalaCodeDesktopChatTurnResponse> => {
        throw new Error("legacy runtime should not handle Claude-selected turns")
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.submitChatMessage({
      messages: [{ id: "user-claude", role: "user", body: "Use Claude" }],
      sessionId: "desktop-session-claude",
      turnId: "desktop-turn-claude",
    })).resolves.toMatchObject({
      backend: {
        blockerRefs: ["blocker.claude_app_sdk.unsupported_until_t8_2"],
        kind: "claude_app_sdk",
        runtimeMode: "claude_runtime",
        turnStatus: "unsupported",
      },
      messages: [{ body: "Claude app SDK chat runtime is not supported until T8.2." }],
      ok: false,
    })

    await expect(handlers.codexTurnStart({
      messages: [{ id: "user-claude-turn", role: "user", body: "Start Claude" }],
      sessionId: "desktop-session-claude",
      turnId: "desktop-turn-claude-2",
    })).resolves.toMatchObject({
      backend: {
        kind: "claude_app_sdk",
        runtimeMode: "claude_runtime",
        turnStatus: "unsupported",
      },
      ok: false,
    })
  })

  test("routes thread lifecycle RPCs through the selected Claude seam", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: { KHALA_CODE_DESKTOP_RUNTIME: "claude_runtime" },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.codexThreadStart({ sessionId: "desktop-session-claude" }))
      .rejects.toThrow("Claude app SDK chat runtime is not supported until T8.2.")
    await expect(handlers.codexThreadList({ sessionId: "desktop-session-claude" }))
      .rejects.toThrow("Claude app SDK chat runtime is not supported until T8.2.")
  })

  test("registers the Khala Fleet MCP bridge before default Codex chat turns", async () => {
    const requests: Array<{ method: string; params: unknown }> = []
    const host = {
      request: async (method: string, params?: unknown) => {
        requests.push({ method, params })
        return {}
      },
      subscribe: () => () => {},
    }
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: host as never,
      codexChatRuntime: throwingCodexChatRuntime({
        startTurn: async () => ({
          backend: {
            kind: "codex_app_server",
            model: "gpt-5.1-codex",
            threadId: "thread-codex-default",
            turnId: "turn-codex-default",
          },
          messages: [{ id: "agent-1", role: "assistant", body: "Codex default path" }],
          ok: true,
          toolNames: [],
          usedTools: [],
        }),
      }),
      enableFleetMcpBridge: true,
      env: { KHALA_CODE_DESKTOP_BUN_COMMAND: "bun-test" },
      fleetMcpBridgeRepoRoot: "/repo/openagents",
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.submitChatMessage({
      messages: [{ id: "user-1", role: "user", body: "Fan out one worker" }],
      sessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })).resolves.toMatchObject({
      backend: { kind: "codex_app_server" },
      messages: [{ body: "Codex default path" }],
      ok: true,
    })

    expect(requests.map(request => request.method)).toEqual([
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/mcpServer/reload",
    ])
    expect(requests[0]?.params).toEqual({
      keyPath: "mcp_servers.khala_fleet.command",
      mergeStrategy: "replace",
      value: "bun-test",
    })
  })

  test("materializes image attachment payloads before routing chat submits", async () => {
    let capturedAttachmentPath: string | undefined
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime({
        startTurn: async request => {
          const attachment = request.attachments?.[0]
          expect(attachment).toMatchObject({
            id: "attachment-image-1",
            kind: "image",
            mime: "image/png",
            name: "composer.png",
            sizeBytes: 11,
          })
          expect(attachment?.dataBase64).toBeUndefined()
          expect(attachment?.path).toContain("khala-code-chat-attachments")
          capturedAttachmentPath = attachment?.path
          if (capturedAttachmentPath === undefined) throw new Error("missing materialized attachment path")
          await expect(readFile(capturedAttachmentPath, "utf8")).resolves.toBe("image-bytes")
          return {
            backend: {
              kind: "codex_app_server",
              model: "gpt-5.1-codex",
              threadId: "thread-image",
              turnId: "turn-image",
            },
            messages: [{ id: "agent-image", role: "assistant", body: "I can see it" }],
            ok: true,
            toolNames: [],
            usedTools: [],
          }
        },
      }),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.submitChatMessage({
      attachments: [{
        dataBase64: "aW1hZ2UtYnl0ZXM=",
        id: "attachment-image-1",
        kind: "image",
        mime: "image/png",
        name: "composer.png",
        sizeBytes: 11,
      }],
      messages: [{ id: "user-image", role: "user", body: "Summarize this image" }],
      sessionId: "desktop-session-image",
      turnId: "desktop-turn-image",
    })).resolves.toMatchObject({
      messages: [{ body: "I can see it" }],
      ok: true,
    })
    expect(capturedAttachmentPath).toContain("attachment-image-1-composer.png")
    await expect(readFile(capturedAttachmentPath ?? "", "utf8")).rejects.toThrow()
  })

  test("adds typed Codex auth blocker refs to failed Codex app-server chat turns", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime({
        startTurn: async () => ({
          backend: {
            kind: "codex_app_server",
            model: "gpt-5.1-codex",
            threadId: "thread-codex-auth-missing",
            turnId: "turn-codex-auth-missing",
            turnStatus: "failed",
          },
          messages: [{
            id: "turn-codex-auth-missing-status",
            role: "system",
            body: "Codex completed the turn with status: failed.",
          }],
          ok: false,
          toolNames: [],
          usedTools: [],
        }),
      }),
      codexHarnessStatus: () => readyHarness({
        available: false,
        reason: "Codex auth.json is missing.",
        status: "unavailable",
        auth: {
          state: "credentials_missing",
          blockerRefs: ["blocker.codex.credentials_missing"],
          accessTokenPresent: false,
          accountIdPresent: false,
          refreshTokenPresent: false,
          error: "Codex auth.json is missing.",
        },
        signIn: {
          required: true,
          command: "codex login",
          warning: "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
        },
      }),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.submitChatMessage({
      messages: [{ id: "user-1", role: "user", body: "Say hello" }],
      sessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })).resolves.toMatchObject({
      backend: {
        kind: "codex_app_server",
        blockerRefs: ["blocker.codex.credentials_missing"],
        runtimeMode: "codex_harness",
        toolCatalogKind: "codex_app_server",
        turnStatus: "failed",
      },
      ok: false,
    })
  })

  test("keeps the Khala-native chat runtime behind the explicit legacy flag", async () => {
    let legacyTurnStarted = false
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: { KHALA_CODE_DESKTOP_RUNTIME: "khala_native_runtime" },
      legacyChatTurn: async input => {
        legacyTurnStarted = true
        expect(input.request.sessionId).toBe("desktop-session-legacy")
        return {
          backend: {
            kind: "mock",
            model: "legacy-khala-native",
          },
          messages: [{ id: "legacy-1", role: "assistant", body: "Legacy runtime" }],
          ok: true,
          toolNames: [],
          usedTools: [],
        }
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.submitChatMessage({
      messages: [{ id: "user-1", role: "user", body: "Use legacy" }],
      sessionId: "desktop-session-legacy",
      turnId: "desktop-turn-legacy",
    })).resolves.toMatchObject({
      backend: {
        kind: "mock",
        model: "legacy-khala-native",
        runtimeMode: "khala_native_runtime",
        toolCatalogKind: "khala_native_legacy",
      },
      messages: [
        { body: "Legacy Khala native runtime handled this turn. The default Khala Code path wraps the local Codex harness." },
        { body: "Legacy runtime" },
      ],
      ok: true,
    })
    expect(legacyTurnStarted).toBe(true)
  })

  test("does not fall back to the legacy Khala runtime when Codex is unavailable", async () => {
    let legacyTurnStarted = false
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      legacyChatTurn: async (): Promise<KhalaCodeDesktopChatTurnResponse> => {
        legacyTurnStarted = true
        throw new Error("legacy runtime should not be called")
      },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.submitChatMessage({
      messages: [{ id: "user-1", role: "user", body: "Run tests" }],
      sessionId: "desktop-session-no-codex",
    })).rejects.toThrow("Codex app-server chat runtime is not configured.")
    expect(legacyTurnStarted).toBe(false)
  })

  test("returns supplemental tool catalog by default and full catalog only in legacy mode", async () => {
    const defaultHandlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })
    const legacyHandlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: { KHALA_CODE_DESKTOP_RUNTIME: "khala_native_runtime" },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(defaultHandlers.toolCatalog()).resolves.toMatchObject({
      catalogKind: "codex_harness_supplemental",
      runtimeMode: "codex_harness",
      toolCount: 6,
      tools: [
        { name: "pylon_ensure", role: "supplemental_swarm" },
        { name: "codex_fleet_status", role: "supplemental_swarm" },
        { name: "codex_spawn", role: "supplemental_swarm" },
        { name: "fleet_run_start", role: "supplemental_swarm" },
        { name: "fleet_run_status", role: "supplemental_swarm" },
        { name: "fleet_run_control", role: "supplemental_swarm" },
      ],
    })
    const legacyCatalog = await legacyHandlers.toolCatalog()
    expect(legacyCatalog).toMatchObject({
      catalogKind: "khala_native_legacy",
      runtimeMode: "khala_native_runtime",
      toolCount: 14,
    })
    expect(legacyCatalog.tools.find(tool => tool.name === "exec_command")?.role)
      .toBe("legacy_codex_equivalent")
  })

  test("promotes a main Codex thread into a bounded swarm delegation request", async () => {
    const fixture = await tempPylonFixture()
    const accountKey = "4db4cc18ebc55f39fb4da894"
    const accountRefHash = `account.pylon.codex.${accountKey}`
    const requestPrompts: string[] = []
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 1,
            codexAccounts: [{
              accountKey,
              available: 1,
              busy: 0,
              queued: 0,
              ready: 1,
            }],
            maxCodexAssignments: 1,
          },
          pylonRef: "pylon.local.promote",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [{
            accountRef: "codex-worker",
            accountRefHash,
            homeState: "present",
            provider: "codex",
          }],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [{
            accountRef: "codex-worker",
            accountRefHash,
            provider: "codex",
            readiness: { state: "ready" },
          }],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.promote.1",
          pylonRef: "pylon.local.promote",
        })
      }
      if (joined === "khala apm --base-url https://openagents.com --json") {
        return ok({
          active: { serverAssignmentCount: 0, serverAssignments: [] },
          counted: { completedTokenRows: 0, completedTokensPerMinute: 0 },
          schema: "openagents.pylon.khala_apm.v0.1",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        const prompt = args[args.indexOf("--prompt") + 1] ?? ""
        requestPrompts.push(prompt)
        expect(args).toContain("--workflow")
        expect(args).toContain("codex_agent_task")
        expect(args).toContain("--account-ref")
        expect(args).toContain("codex-worker")
        expect(args).toContain("--fixture")
        expect(args).toContain("--no-run")
        expect(prompt).toContain("Khala swarm delegation from a main local Codex thread.")
        expect(prompt).toContain("Origin thread: thread-main-1")
        expect(prompt).toContain("transcript included: false")
        expect(prompt).toContain("allowed refs: repo.OpenAgentsInc.openagents, issue.7791")
        expect(prompt).toContain("Objective: Split the failing test matrix across workers.")
        expect(prompt).not.toContain("full private transcript")
        return ok({
          assignmentRef: "assignment.public.promoted",
          autoRun: {
            attempted: false,
            reason: "disabled_by_no_run",
          },
          durableRequestId: "durable.public.promoted",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexFleetToolOptions: { runner },
      codexHarnessStatus: () => readyHarness(),
      env: fixture.env,
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.codexFleetPromoteThread({
      contextBoundary: {
        allowedRefs: ["repo.OpenAgentsInc.openagents", "issue.7791"],
        includeTranscript: false,
        mode: "summary_only",
        summary: "Use only the current failing test names.",
      },
      count: 1,
      fixture: true,
      noRun: true,
      objective: "Split the failing test matrix across workers.",
      sessionId: "desktop-session-main",
      threadId: "thread-main-1",
    })).resolves.toMatchObject({
      acceptedCount: 1,
      contextBoundary: {
        includeTranscript: false,
        mode: "summary_only",
      },
      ok: true,
      origin: {
        role: "main_local_codex_session",
        sessionId: "desktop-session-main",
        threadId: "thread-main-1",
      },
      pylonRef: "pylon.local.promote",
      requestedCount: 1,
      results: [{
        accountRef: "codex-worker",
        assignmentRef: "assignment.public.promoted",
        status: "accepted",
        transcriptRef: "durable.public.promoted",
      }],
      workerRuntime: {
        assignmentTool: "codex_spawn",
        homeRole: "pylon_isolated_worker_codex_home",
        role: "swarm_worker_codex_session",
        runtime: "codex_harness",
      },
    })
    expect(requestPrompts).toHaveLength(1)
  })

  test("runs the deterministic fleet delegate through a public-safe RPC projection", async () => {
    const fixture = await tempPylonFixture()
    const calls: KhalaCodexFleetCommandInput[] = []
    let advertised = false
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      calls.push(input)
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return advertised
          ? ok({
              ok: true,
              ownCapacityDispatch: {
                availableCodexAssignments: 4,
                codexAccounts: [{
                  accountKey: "4db4cc18ebc55f39fb4da894",
                  available: 4,
                  busy: 1,
                  queued: 0,
                  ready: 5,
                }],
                maxCodexAssignments: 5,
              },
              pylonRef: "pylon.local.delegate",
            })
          : ok({
              ok: true,
              ownCapacityDispatch: {
                availableCodexAssignments: 0,
                maxCodexAssignments: 1,
              },
              pylonRef: "pylon.local.delegate",
            })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [{
            accountRef: "codex-worker",
            accountRefHash: "account.pylon.codex.4db4cc18ebc55f39fb4da894",
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          }],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        advertised = true
        return ok({
          heartbeatRef: "heartbeat.pylon.local.delegate.1",
          pylonRef: "pylon.local.delegate",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        return ok({
          aggregate: {
            acceptedCount: 1,
            assignmentRefs: ["assignment.public.codex_agent_task.delegate"],
            durableRequestIds: ["durable.public.delegate"],
            ownerOnlyRawEventCount: 1,
            ownerOnlyTraceCount: 1,
            totalTokenRows: 1,
            totalVerifiedTokens: 100,
          },
          counter: { expectedMinimumDelta: 0, state: "not_checked" },
          ok: true,
          plan: {
            requestedCount: 1,
            slots: [{ account: { accountRef: "codex-worker" }, slotIndex: 0 }],
            targetPylonRef: "pylon.local.delegate",
          },
          results: [{
            assignmentRef: "assignment.public.codex_agent_task.delegate",
            blockerRefs: [],
            closeoutStatus: "accepted",
            ok: true,
            proof: { rawEventCount: 1, tokenRows: 1, totalTokens: 100, traceCount: 1 },
            runAccepted: true,
            slotIndex: 0,
            state: "completed",
          }],
          schema: "openagents.pylon.khala_spawn_run.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexFleetToolOptions: { runner },
      codexHarnessStatus: () => readyHarness(),
      env: fixture.env,
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    const result = await handlers.codexFleetDelegateRun({
      count: 1,
      mode: "fixture",
      objective: "Run the public fixture without exposing this objective in the UI projection.",
    })

    expect(result).toMatchObject({
      acceptedCount: 1,
      delegateSignature: "khala.fleet.delegate",
      delegateStatus: "completed",
      mode: "fixture",
      ok: true,
      projection: {
        localPathsProjected: false,
        objectiveProjected: false,
        providerPayloadProjected: false,
        rawTraceMessagesProjected: false,
      },
      requestedCount: 1,
      results: [{
        accountRef: "codex-worker",
        assignmentRef: "assignment.public.codex_agent_task.delegate",
        status: "accepted",
      }],
      validation: {
        fixture: true,
        repoPinsComplete: true,
      },
    })
    expect(result.trace.map(step => step.module)).toEqual([
      "ensure_pylon",
      "advertise_capacity",
      "select_account",
      "prepare_work",
      "dispatch",
      "verify_closeout",
    ])
    expect(result.trace.find(step => step.module === "advertise_capacity")?.summary)
      .toContain("Codex capacity advertisement")
    const serialized = JSON.stringify(result)
    expect(serialized).not.toContain("Run the public fixture without exposing")
    expect(result.projection.providerPayloadProjected).toBe(false)
    expect(result.projection.rawTraceMessagesProjected).toBe(false)
    expect(serialized).not.toMatch(/\/Users\/|auth\.json|bearer|credential|sk-[a-z0-9]/i)
    const commandOrder = calls.map(call => pylonArgs(call).join(" "))
    expect(commandOrder.indexOf("presence heartbeat --base-url https://openagents.com --json"))
      .toBeLessThan(commandOrder.findIndex(command => command.startsWith("khala spawn ")))

    await expect(handlers.codexFleetDelegateRun({
      mode: "real_work",
      objective: "Missing repo pins should never dispatch.",
    })).rejects.toThrow("requires repo, claimRef, commit, and verify pins")
  })

  test("runs real-work fleet delegate RPC with claim and repository pins", async () => {
    const fixture = await tempPylonFixture()
    const liveCommit = "0123456789abcdef0123456789abcdef01234567"
    const calls: KhalaCodexFleetCommandInput[] = []
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      calls.push(input)
      if (input.cmd[0] === "git" && input.cmd[1] === "ls-remote") {
        expect(input.cmd).toEqual([
          "git",
          "ls-remote",
          "https://github.com/OpenAgentsInc/openagents.git",
          "refs/heads/main",
        ])
        return ok(`${liveCommit}\trefs/heads/main\n`)
      }
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 1,
            codexAccounts: [{
              accountKey: "4db4cc18ebc55f39fb4da894",
              available: 1,
              busy: 0,
              queued: 0,
              ready: 1,
            }],
            maxCodexAssignments: 1,
          },
          pylonRef: "pylon.local.real_work",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [{
            accountRef: "codex-worker",
            accountRefHash: "account.pylon.codex.4db4cc18ebc55f39fb4da894",
            homeState: "present",
            provider: "codex",
            readiness: { state: "ready" },
          }],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.real_work.1",
          pylonRef: "pylon.local.real_work",
        })
      }
      if (joined === "khala apm --base-url https://openagents.com --json") {
        return ok({
          active: { serverAssignmentCount: 0, serverAssignments: [] },
          counted: { completedTokenRows: 0, completedTokensPerMinute: 0 },
          schema: "openagents.pylon.khala_apm.v0.1",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        expect(args).toContain("--repo")
        expect(args).toContain("OpenAgentsInc/openagents")
        expect(args).toContain("--commit")
        expect(args).toContain(liveCommit)
        expect(args).toContain("--verify")
        expect(args).toContain("command.public.pylon_khala.verify.d32c71ee8e1025e99460d008")
        expect(args).not.toContain("--fixture")
        const objective = args[args.indexOf("--objective") + 1] ?? ""
        expect(objective).toContain("Ship the pinned public fix.")
        expect(objective).toContain("Claim: claim.public.t4_2.rpc_real_work.")
        return ok({
          aggregate: {
            acceptedCount: 1,
            assignmentRefs: ["assignment.public.codex_agent_task.real_work"],
            durableRequestIds: ["durable.public.real_work"],
            ownerOnlyRawEventCount: 1,
            ownerOnlyTraceCount: 1,
            totalTokenRows: 1,
            totalVerifiedTokens: 121,
          },
          counter: { expectedMinimumDelta: 0, state: "not_checked" },
          ok: true,
          plan: {
            requestedCount: 1,
            slots: [{ account: { accountRef: "codex-worker" }, slotIndex: 0 }],
            targetPylonRef: "pylon.local.real_work",
          },
          results: [{
            assignmentRef: "assignment.public.codex_agent_task.real_work",
            blockerRefs: [],
            closeoutStatus: "accepted",
            ok: true,
            proof: { rawEventCount: 1, tokenRows: 1, totalTokens: 121, traceCount: 1 },
            runAccepted: true,
            slotIndex: 0,
            state: "completed",
          }],
          schema: "openagents.pylon.khala_spawn_run.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexFleetToolOptions: { runner },
      codexHarnessStatus: () => readyHarness(),
      env: fixture.env,
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    const result = await handlers.codexFleetDelegateRun({
      branch: "main",
      claimRef: "claim.public.t4_2.rpc_real_work",
      commit: liveCommit,
      count: 1,
      mode: "real_work",
      objective: "Ship the pinned public fix.",
      repo: "OpenAgentsInc/openagents",
      verify: "command.public.pylon_khala.verify.d32c71ee8e1025e99460d008",
    })

    expect(result).toMatchObject({
      acceptedCount: 1,
      delegateStatus: "completed",
      mode: "real_work",
      ok: true,
      results: [{
        assignmentRef: "assignment.public.codex_agent_task.real_work",
        status: "accepted",
        tokensVerified: 121,
      }],
      validation: {
        fixture: false,
        repoPinsComplete: true,
      },
    })
    expect(calls.findIndex(call => call.cmd[0] === "git" && call.cmd[1] === "ls-remote"))
      .toBeLessThan(calls.findIndex(call => pylonArgs(call)[0] === "khala" && pylonArgs(call)[1] === "spawn"))
  })

  test("lists Codex slash commands with desktop availability rules", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    const response = await handlers.slashCommandList({
      activeTurn: true,
      platform: "darwin",
    })
    const byCommand = new Map(response.commands.map(command => [command.command, command]))

    expect(response.ok).toBe(true)
    expect(byCommand.get("app")).toBeDefined()
    expect(byCommand.get("sandbox-add-read-dir")).toBeUndefined()
    expect(byCommand.get("plan")?.availability).toMatchObject({
      available: false,
      reason: "/plan is not available while Codex is working.",
    })
    expect(byCommand.get("raw")?.availability).toEqual({ available: true })
  })

  test("dispatches thread-scoped slash commands through Codex app-server", async () => {
    const requests: { method: string, params: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>(method: string, params?: unknown) => {
          requests.push({ method, params })
          return { ok: true } as Result
        },
        respondToServerRequest: () => undefined,
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      codexChatRuntime: throwingCodexChatRuntime({
        threadIdForSession: async sessionId => {
          expect(sessionId).toBe("desktop-session-slash")
          return "thread-session-slash"
        },
      }),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/rename Sharp New Name",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: true,
      command: "rename",
      method: "thread/name/set",
      status: "dispatched",
      threadId: "thread-session-slash",
    })

    expect(requests).toEqual([{
      method: "thread/name/set",
      params: {
        threadId: "thread-session-slash",
        name: "Sharp New Name",
      },
    }])
  })

  test("dispatches background terminal slash commands and RPC actions through Codex app-server", async () => {
    const requests: { method: string, params: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>(method: string, params?: unknown) => {
          requests.push({ method, params })
          return method === "thread/backgroundTerminals/list"
            ? {
              data: [{
                command: "python3 -m http.server",
                cwd: "/workspace",
                itemId: "item-1",
                osPid: null,
                processId: "42",
              }],
              nextCursor: null,
            } as Result
            : { ok: true } as Result
        },
        respondToServerRequest: () => undefined,
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      codexChatRuntime: throwingCodexChatRuntime({
        threadIdForSession: async sessionId => {
          expect(sessionId).toBe("desktop-session-bg")
          return "thread-bg"
        },
      }),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/ps",
      sessionId: "desktop-session-bg",
    })).resolves.toMatchObject({
      ok: true,
      command: "ps",
      method: "thread/backgroundTerminals/list",
      status: "dispatched",
      message: "Loaded Codex background terminals.",
      threadId: "thread-bg",
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/clean",
      sessionId: "desktop-session-bg",
    })).resolves.toMatchObject({
      ok: true,
      command: "stop",
      method: "thread/backgroundTerminals/clean",
      status: "dispatched",
      message: "Requested Codex background terminal cleanup.",
      threadId: "thread-bg",
    })

    await expect(handlers.codexBackgroundTerminalsTerminate({
      processId: "42",
      threadId: "thread-bg",
    })).resolves.toMatchObject({
      ok: true,
      method: "thread/backgroundTerminals/terminate",
    })

    expect(requests).toEqual([
      {
        method: "thread/backgroundTerminals/list",
        params: {
          threadId: "thread-bg",
          cursor: null,
          limit: 50,
        },
      },
      {
        method: "thread/backgroundTerminals/clean",
        params: {
          threadId: "thread-bg",
        },
      },
      {
        method: "thread/backgroundTerminals/terminate",
        params: {
          threadId: "thread-bg",
          processId: "42",
        },
      },
    ])
  })

  test("steers the active Codex turn for BTW slash notes", async () => {
    const steerCalls: {
      readonly clientUserMessageId?: string | undefined
      readonly sessionId: string
      readonly text: string
      readonly turnId?: string | undefined
    }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime({
        steerTurn: async request => {
          steerCalls.push(request)
          return {
            ok: true,
            codexTurnId: "turn-codex-btw",
            desktopSessionId: request.sessionId,
            desktopTurnId: "desktop-turn-btw",
            response: { accepted: true },
            threadId: "thread-session-btw",
          }
        },
      }),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.slashCommandDispatch({
      activeTurn: true,
      raw: "/btw keep the existing Codex plan authority",
      sessionId: "desktop-session-btw",
    })).resolves.toMatchObject({
      ok: true,
      command: "btw",
      method: "turn/steer",
      status: "dispatched",
      threadId: "thread-session-btw",
    })

    expect(steerCalls).toEqual([expect.objectContaining({
      sessionId: "desktop-session-btw",
      text: "keep the existing Codex plan authority",
    })])
    expect(steerCalls[0]?.clientUserMessageId).toStartWith("khala-code-slash-btw-")
  })

  test("returns blocked and gap results for unavailable slash commands", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.slashCommandDispatch({
      activeTurn: true,
      raw: "/plan tighten the implementation",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: false,
      command: "plan",
      status: "blocked",
      message: "/plan is not available while Codex is working.",
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/init",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: false,
      command: "init",
      status: "gap",
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/side investigate in a side conversation",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: false,
      command: "side",
      gap: {
        kind: "upstream_app_server_gap",
        gapId: "codex.app_server.gap.side_agent_plan_controls",
      },
      status: "unavailable",
    })
  })

  test("dispatches mention candidates through bounded Codex app-server file search", async () => {
    const requests: { method: string, params: unknown }[] = []
    const files = Array.from({ length: 25 }, (_, index) => ({
      root: "/repo",
      path: `src/file-${index}.ts`,
      match_type: index % 2 === 0 ? "file" : "directory",
      file_name: `file-${index}.ts`,
      score: 100 - index,
      indices: [0],
    }))
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: codexAppServerHost(async <Result>(method: string, params?: unknown) => {
        requests.push({ method, params })
        return { files } as Result
      }),
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    const result = await handlers.slashCommandDispatch({
      cwd: "/repo/workspace",
      raw: "/mention worker",
      sessionId: "desktop-session-slash",
    })
    const composerResult = await handlers.codexMentionCandidates({
      cwd: "/repo/workspace",
      query: "worker",
    })

    expect(result).toMatchObject({
      ok: true,
      command: "mention",
      method: "fuzzyFileSearch",
      status: "dispatched",
    })
    expect(composerResult).toMatchObject({
      ok: true,
      source: "fuzzyFileSearch",
      truncated: true,
    })
    expect(requests).toEqual([
      {
        method: "fuzzyFileSearch",
        params: {
          query: "worker",
          roots: ["/repo/workspace"],
          cancellationToken: null,
        },
      },
      {
        method: "fuzzyFileSearch",
        params: {
          query: "worker",
          roots: ["/repo/workspace"],
          cancellationToken: null,
        },
      },
    ])
    expect(result.message).toContain("Codex fuzzyFileSearch mention candidates (truncated)")
    expect(JSON.stringify(result.response)).toContain('"truncated":true')
    expect((result.response as { candidates: unknown[] }).candidates).toHaveLength(20)
    expect(composerResult.candidates).toHaveLength(20)
  })

  test("dispatches empty mention browsing through Codex fs/readDirectory", async () => {
    const requests: { method: string, params: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: codexAppServerHost(async <Result>(method: string, params?: unknown) => {
        requests.push({ method, params })
        return { entries: [] } as Result
      }),
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/mention",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: true,
      command: "mention",
      method: "fs/readDirectory",
      message: "Codex fs/readDirectory returned no mention candidates.",
      status: "dispatched",
    })
    await expect(handlers.codexMentionCandidates()).resolves.toMatchObject({
      ok: true,
      candidates: [],
      source: "fs/readDirectory",
      truncated: false,
    })
    expect(requests).toEqual([
      {
        method: "fs/readDirectory",
        params: { path: "/repo" },
      },
      {
        method: "fs/readDirectory",
        params: { path: "/repo" },
      },
    ])
  })

  test("dispatches diff and IDE status through Codex app-server", async () => {
    const requests: { method: string, params: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: codexAppServerHost(async <Result>(method: string, params?: unknown) => {
        requests.push({ method, params })
        if (method === "gitDiffToRemote") {
          return {
            sha: "abc123",
            diff: "diff --git a/a.ts b/a.ts\n+added\n",
          } as Result
        }
        return {
          config: {
            ide_integration: {
              status: "connected",
              editor: "vscode",
            },
          },
        } as Result
      }),
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    await expect(handlers.slashCommandDispatch({
      cwd: "/repo/project",
      raw: "/diff",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: true,
      command: "diff",
      method: "gitDiffToRemote",
      message: expect.stringContaining("```diff"),
      status: "dispatched",
    })
    await expect(handlers.slashCommandDispatch({
      cwd: "/repo/project",
      raw: "/ide",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: true,
      command: "ide",
      method: "config/read",
      message: expect.stringContaining("connected"),
      status: "dispatched",
    })
    expect(requests).toEqual([
      {
        method: "gitDiffToRemote",
        params: { cwd: "/repo/project" },
      },
      {
        method: "config/read",
        params: { cwd: "/repo/project", includeLayers: true },
      },
    ])
  })

  test("dispatches preference slash commands through Codex config methods", async () => {
    const requests: { method: string, params: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: codexAppServerHost(async <Result>(method: string, params?: unknown) => {
        requests.push({ method, params })
        return { ok: true } as Result
      }),
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/theme",
      sessionId: "desktop-session-slash",
      cwd: "/repo/project",
    })).resolves.toMatchObject({
      ok: true,
      command: "theme",
      method: "config/read",
      status: "dispatched",
    })
    await expect(handlers.slashCommandDispatch({
      raw: "/pet spark",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: true,
      command: "pets",
      method: "config/value/write",
      status: "dispatched",
    })

    expect(requests).toEqual([
      {
        method: "config/read",
        params: {
          cwd: "/repo/project",
          includeLayers: true,
        },
      },
      {
        method: "config/value/write",
        params: {
          keyPath: "tui.pet",
          value: "spark",
          mergeStrategy: "replace",
        },
      },
    ])
  })

  test("returns typed diff empty and error states", async () => {
    const emptyHandlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: codexAppServerHost(async <Result>() => ({
        sha: "abc123",
        diff: "",
      } as Result)),
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })
    await expect(emptyHandlers.slashCommandDispatch({
      raw: "/diff",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: true,
      command: "diff",
      message: "Codex gitDiffToRemote returned no diff.",
      status: "dispatched",
    })

    const errorHandlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: codexAppServerHost(async () => {
        throw new Error("gitDiffToRemote unavailable")
      }),
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })
    await expect(errorHandlers.slashCommandDispatch({
      raw: "/diff",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: false,
      command: "diff",
      message: "gitDiffToRemote unavailable",
      method: "gitDiffToRemote",
      status: "blocked",
    })
  })

  test("returns blocked preference writes when Codex config is unavailable", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: codexAppServerHost(async <Result>(method: string) => {
        if (method === "config/value/write") {
          throw new Error("Invalid configuration: tui.pet is managed")
        }
        return {} as Result
      }),
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    await expect(handlers.slashCommandDispatch({
      raw: "/pet spark",
      sessionId: "desktop-session-slash",
    })).resolves.toMatchObject({
      ok: false,
      command: "pets",
      method: "config/value/write",
      status: "blocked",
      message: "Invalid configuration: tui.pet is managed",
    })
  })

  test("sends Codex approval responses through the app-server host", async () => {
    const responses: { id: number | string, result: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>() => ({} as Result),
        respondToServerRequest: (id, result) => {
          responses.push({ id, result })
        },
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.codexApprovalRespond({
      action: "acceptWithExecpolicyAmendment",
      execpolicyAmendment: ["git status"],
      method: "item/commandExecution/requestApproval",
      requestId: 71,
    })).resolves.toMatchObject({
      ok: true,
      method: "item/commandExecution/requestApproval",
      requestId: 71,
      payload: {
        decision: {
          acceptWithExecpolicyAmendment: {
            execpolicy_amendment: ["git status"],
          },
        },
      },
    })

    expect(responses).toEqual([{
      id: 71,
      result: {
        decision: {
          acceptWithExecpolicyAmendment: {
            execpolicy_amendment: ["git status"],
          },
        },
      },
    }])
  })

  test("reads Codex settings from app-server config, model, profile, usage, and collaboration APIs", async () => {
    const requests: { method: string, params: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>(method: string, params?: unknown) => {
          requests.push({ method, params })
          const responses: Record<string, unknown> = {
            "config/read": {
              config: {
                model: "gpt-5.1-codex",
                model_provider: "openai",
                model_reasoning_effort: "high",
                service_tier: "priority",
                default_permissions: ":workspace",
              },
              origins: { model: { source: "user" } },
              layers: [],
            },
            "model/list": {
              data: [
                {
                  id: "gpt-5.1-codex",
                  model: "gpt-5.1-codex",
                  displayName: "GPT-5.1 Codex",
                  description: "Codex",
                  hidden: false,
                  isDefault: true,
                  supportsPersonality: true,
                  defaultReasoningEffort: "medium",
                  supportedReasoningEfforts: [{ reasoningEffort: "high", description: "deep" }],
                  serviceTiers: [{ id: "priority", name: "Priority", description: "fast" }],
                  defaultServiceTier: "priority",
                },
              ],
            },
            "modelProvider/capabilities/read": {
              namespaceTools: true,
              imageGeneration: false,
              webSearch: true,
            },
            "permissionProfile/list": {
              data: [{ id: ":workspace", description: "Workspace", allowed: true }],
            },
            "configRequirements/read": {
              requirements: { allowedPermissionProfiles: { ":workspace": true } },
            },
            "account/usage/read": {
              summary: { lifetimeTokens: 99 },
              dailyUsageBuckets: [],
            },
            "collaborationMode/list": {
              data: [{ name: "Plan", mode: "plan", model: null, reasoning_effort: "medium" }],
            },
          }
          return responses[method] as Result
        },
        respondToServerRequest: () => undefined,
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    const settings = await handlers.codexSettingsRead({
      cwd: "/repo/project",
      includeHiddenModels: true,
    })

    expect(settings).toMatchObject({
      ok: true,
      cwd: "/repo/project",
      config: {
        model: "gpt-5.1-codex",
        modelProvider: "openai",
        reasoningEffort: "high",
        serviceTier: "priority",
        defaultPermissions: ":workspace",
      },
      models: {
        selected: {
          id: "gpt-5.1-codex",
        },
        serviceTierCommands: ["priority"],
      },
      providerCapabilities: {
        namespaceTools: true,
        imageGeneration: false,
        webSearch: true,
      },
      usage: {
        available: true,
      },
    })
    expect(requests.map(request => request.method)).toEqual([
      "config/read",
      "model/list",
      "modelProvider/capabilities/read",
      "permissionProfile/list",
      "configRequirements/read",
      "account/usage/read",
      "collaborationMode/list",
    ])
    expect(requests[0]?.params).toEqual({ cwd: "/repo/project", includeLayers: true })
    expect(requests[1]?.params).toEqual({ includeHidden: true })
    expect(requests[3]?.params).toEqual({ cwd: "/repo/project" })
  })

  test("writes Codex config values through app-server and reloads settings projection", async () => {
    const requests: { method: string, params: unknown }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>(method: string, params?: unknown) => {
          requests.push({ method, params })
          if (method === "config/value/write") {
            return { status: "changed", version: "v2", filePath: "/home/user/.codex/config.toml" } as Result
          }
          if (method === "config/read") {
            return {
              config: {
                model: "gpt-5.1-codex",
                default_permissions: ":workspace",
              },
              origins: {},
              layers: [],
            } as Result
          }
          if (method === "model/list") {
            return {
              data: [
                {
                  id: "gpt-5.1-codex",
                  model: "gpt-5.1-codex",
                  displayName: "GPT-5.1 Codex",
                  description: "",
                  hidden: false,
                  isDefault: true,
                  supportsPersonality: false,
                  defaultReasoningEffort: "medium",
                  supportedReasoningEfforts: [],
                  serviceTiers: [],
                  defaultServiceTier: null,
                },
              ],
            } as Result
          }
          return {} as Result
        },
        respondToServerRequest: () => undefined,
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    const result = await handlers.codexConfigValueWrite({
      cwd: "/repo",
      keyPath: "model",
      value: "gpt-5.1-codex",
    })

    expect(result).toMatchObject({
      ok: true,
      keyPath: "model",
      settings: {
        config: {
          model: "gpt-5.1-codex",
        },
      },
    })
    expect(requests[0]).toEqual({
      method: "config/value/write",
      params: {
        keyPath: "model",
        value: "gpt-5.1-codex",
        mergeStrategy: "replace",
      },
    })
    expect(requests.slice(1).map(request => request.method)).toEqual([
      "config/read",
      "model/list",
      "modelProvider/capabilities/read",
      "permissionProfile/list",
      "configRequirements/read",
      "account/usage/read",
      "collaborationMode/list",
    ])
  })

  test("returns config write denials without mutating Khala-local state", async () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>(method: string) => {
          if (method === "config/value/write") throw new Error("Invalid configuration: model is managed")
          throw new Error(`unexpected request ${method}`)
        },
        respondToServerRequest: () => undefined,
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    await expect(handlers.codexConfigValueWrite({
      keyPath: "model",
      value: "gpt-5.1-codex",
    })).resolves.toEqual({
      ok: false,
      keyPath: "model",
      error: "Invalid configuration: model is managed",
    })
  })

  test("reads Codex ecosystem state through app-server catalog APIs", async () => {
    const requests: {
      readonly method: string
      readonly params: unknown
    }[] = []
    const subscribers: CodexAppServerNotificationHandler[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>(method: string, params: unknown = {}): Promise<Result> => {
          requests.push({ method, params })
          if (method === "skills/list") {
            return {
              data: [{
                cwd: "/repo",
                skills: [{
                  name: "github",
                  description: "GitHub skill",
                  path: "/home/user/.codex/skills/github/SKILL.md",
                  scope: "user",
                  enabled: true,
                }],
                errors: [],
              }],
            } as Result
          }
          if (method === "hooks/list") {
            return { data: [{ cwd: "/repo", hooks: [], warnings: [], errors: [] }] } as Result
          }
          if (method === "externalAgentConfig/detect") {
            return {
              items: [{
                itemType: "AGENTS_MD",
                description: "Import AGENTS.md from another agent",
                cwd: "/repo",
                details: null,
              }],
            } as Result
          }
          if (method === "externalAgentConfig/import/readHistories") {
            return {
              data: [{
                importId: "import-1",
                completedAtMs: 1782926400000,
                successes: [{
                  itemType: "AGENTS_MD",
                  cwd: "/repo",
                  source: "CLAUDE.md",
                  target: "AGENTS.md",
                }],
                failures: [],
              }],
            } as Result
          }
          if (method === "plugin/list" || method === "plugin/installed") {
            return {
              marketplaces: [{
                name: "curated",
                path: null,
                interface: null,
                plugins: [{
                  id: "github@curated",
                  name: "github",
                  installed: true,
                  enabled: true,
                  installPolicy: "AVAILABLE",
                  authPolicy: "ON_USE",
                  availability: "AVAILABLE",
                  source: { type: "remote" },
                  keywords: [],
                }],
              }],
              marketplaceLoadErrors: [],
              featuredPluginIds: [],
            } as Result
          }
          if (method === "app/list") {
            return {
              data: [{
                id: "linear",
                name: "Linear",
                isAccessible: true,
                isEnabled: true,
                pluginDisplayNames: [],
              }],
              nextCursor: null,
            } as Result
          }
          if (method === "mcpServerStatus/list") {
            return {
              data: [{
                name: "github",
                tools: {},
                resources: [],
                resourceTemplates: [],
                authStatus: "notLoggedIn",
              }],
              nextCursor: null,
            } as Result
          }
          throw new Error(`unexpected request ${method}`)
        },
        respondToServerRequest: () => undefined,
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: handler => {
          subscribers.push(handler)
          return () => undefined
        },
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    const emit = subscribers[0]
    expect(emit).toBeDefined()
    if (emit === undefined) throw new Error("ecosystem notification subscriber was not registered")
    emit({
      method: "skills/changed",
      params: {},
      receivedAt: "2026-07-01T12:00:00.000Z",
    })
    const result = await handlers.codexEcosystemRead({
      cwd: "/repo",
      forceRefetchApps: true,
      forceReloadSkills: true,
      threadId: "thread-1",
    })

    expect(requests.map(request => request.method)).toEqual([
      "skills/list",
      "hooks/list",
      "externalAgentConfig/detect",
      "externalAgentConfig/import/readHistories",
      "plugin/list",
      "plugin/installed",
      "app/list",
      "mcpServerStatus/list",
    ])
    expect(requests[0].params).toEqual({
      cwds: ["/repo"],
      forceReload: true,
    })
    expect(requests[2].params).toEqual({
      cwds: ["/repo"],
      includeHome: true,
    })
    expect(requests[6].params).toEqual({
      threadId: "thread-1",
      forceRefetch: true,
    })
    expect(requests[7].params).toEqual({
      detail: "full",
      threadId: "thread-1",
    })
    expect(result.sections.skills.count).toBe(1)
    expect(result.sections.imports.installRequiredCount).toBe(1)
    expect(result.sections.mcp.authRequiredCount).toBe(1)
    expect(result.notifications.map(notification => notification.method)).toContain("skills/changed")
  })

  test("passes Codex ecosystem mutations and MCP calls directly to app-server", async () => {
    const requests: {
      readonly method: string
      readonly params: unknown
    }[] = []
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexAppServerHost: {
        dispose: () => undefined,
        request: async <Result>(method: string, params: unknown = {}): Promise<Result> => {
          requests.push({ method, params })
          return { method, params } as Result
        },
        respondToServerRequest: () => undefined,
        restart: async () => ({
          ok: true,
          action: "restart",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        start: async () => ({
          ok: true,
          action: "start",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        status: stoppedAppServerStatus,
        stop: async () => ({
          ok: true,
          action: "stop",
          changed: false,
          status: stoppedAppServerStatus(),
        }),
        subscribe: () => () => undefined,
      },
      codexChatRuntime: throwingCodexChatRuntime(),
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: "/repo",
    })

    await handlers.codexSkillsExtraRootsSet({ extraRoots: ["/repo/skills"] })
    await handlers.codexSkillsConfigWrite({ name: "github", enabled: false })
    await handlers.codexExternalAgentConfigDetect({ cwds: ["/repo"], includeHome: true })
    await handlers.codexExternalAgentConfigImport({
      source: "claude",
      migrationItems: [{
        itemType: "AGENTS_MD",
        description: "Import AGENTS.md",
        cwd: "/repo",
        details: null,
      }],
    })
    await handlers.codexExternalAgentConfigImportHistoriesRead()
    await handlers.codexFsReadFile({ path: "/repo/AGENTS.md" })
    await handlers.codexFsWriteFile({ path: "/repo/AGENTS.md", dataBase64: "IyBBR0VOVFMK" })
    await handlers.codexFsGetMetadata({ path: "/repo/AGENTS.md" })
    await handlers.codexMarketplaceAdd({ source: "https://github.com/acme/plugins", refName: "main" })
    await handlers.codexMarketplaceRemove({ marketplaceName: "acme" })
    await handlers.codexMarketplaceUpgrade({ marketplaceName: "acme" })
    await handlers.codexPluginInstall({ remoteMarketplaceName: "curated", pluginName: "github" })
    await handlers.codexPluginUninstall({ pluginId: "github@curated" })
    await handlers.codexMcpOauthLogin({ server: "github", threadId: "thread-1", scopes: ["repo"], timeoutSecs: 15 })
    await handlers.codexMcpResourceRead({ server: "github", threadId: "thread-1", uri: "repo://OpenAgentsInc/openagents" })
    const toolResult = await handlers.codexMcpToolCall({
      threadId: "thread-1",
      server: "github",
      tool: "list_issues",
      arguments: { owner: "OpenAgentsInc", repo: "openagents" },
      meta: { source: "khala-code-ui" },
    })
    await handlers.codexMcpServerReload()

    expect(requests.map(request => request.method)).toEqual([
      "skills/extraRoots/set",
      "skills/config/write",
      "externalAgentConfig/detect",
      "externalAgentConfig/import",
      "externalAgentConfig/import/readHistories",
      "fs/readFile",
      "fs/writeFile",
      "fs/getMetadata",
      "marketplace/add",
      "marketplace/remove",
      "marketplace/upgrade",
      "plugin/install",
      "plugin/uninstall",
      "mcpServer/oauth/login",
      "mcpServer/resource/read",
      "mcpServer/tool/call",
      "config/mcpServer/reload",
    ])
    expect(requests[15].params).toEqual({
      threadId: "thread-1",
      server: "github",
      tool: "list_issues",
      arguments: { owner: "OpenAgentsInc", repo: "openagents" },
      _meta: { source: "khala-code-ui" },
    })
    expect(toolResult).toMatchObject({
      ok: true,
      method: "mcpServer/tool/call",
    })
  })

  test("projects Fleet Status capacity and token evidence through RPC", async () => {
    const fixture = await tempPylonFixture()
    const accountKey = "4db4cc18ebc55f39fb4da894"
    const accountRefHash = `account.pylon.codex.${accountKey}`
    const markerRoot = join(fixture.home, "active-assignment-runs")
    await mkdir(markerRoot, { recursive: true })
    await writeFile(join(markerRoot, "assignment.public.rpc.json"), JSON.stringify({
      accountRefHash,
      assignmentRef: "assignment.public.rpc",
      closeoutStatus: "accepted",
      refreshedAt: "2026-06-30T18:00:00.000Z",
      schema: "openagents.pylon.active_assignment_run.v0.1",
      service: "codex",
      startedAt: "2026-06-30T17:58:00.000Z",
      transcriptRef: "transcript.public.rpc",
    }))

    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            codexAccounts: [{
              accountKey,
              available: 2,
              busy: 1,
              queued: 0,
              ready: 3,
            }],
            maxCodexAssignments: 3,
          },
          pylonRef: "pylon.local.rpc",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [{
            accountRef: "codex-2",
            accountRefHash,
            homeState: "present",
            provider: "codex",
          }],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [{
            accountRef: "codex-2",
            accountRefHash,
            provider: "codex",
            quota: { state: "available" },
            readiness: { state: "ready" },
          }],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "khala apm --base-url https://openagents.com --json") {
        return ok({
          active: {
            adjustedTokensPerMinute: 315,
            inFlightTokens: 630,
            inFlightTokensPerMinute: 315,
            serverAssignmentCount: 1,
            serverAssignments: [{
              assignmentRef: "assignment.public.rpc",
              elapsedMs: 120_000,
              source: "fleet.activeAssignments.tokensSoFar",
              tokenCountKind: "exact",
              tokens: 630,
              tokensPerMinute: 315,
            }],
          },
          counted: {
            completedTokenRows: 2,
            completedTokensPerMinute: 48,
            sourceRefs: ["d1:token_usage_events"],
          },
          schema: "openagents.pylon.khala_apm.v0.1",
        })
      }
      if (input.cmd[0] === "ps") return ok("  PID  PPID     ELAPSED COMMAND\n")
      return failed(`unexpected command: ${joined}`)
    }

    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      codexFleetToolOptions: { runner },
      codexHarnessStatus: () => readyHarness(),
      env: fixture.env,
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.codexFleetStatus()).resolves.toMatchObject({
      accounts: [{
        accountRef: "codex-2",
        capacity: {
          available: 2,
          busy: 1,
          queued: 0,
          ready: 3,
        },
        homeRole: "pylon_isolated_worker_codex_home",
        quotaState: "available",
        readiness: "ready",
        sessionRole: "swarm_worker_codex_session",
      }],
      activeAssignments: [{
        assignmentRef: "assignment.public.rpc",
        closeoutStatus: "accepted",
        tokenRate: {
          status: "exact",
          tokenCountKind: "exact",
          tokens: 630,
          tokensPerMinute: 315,
        },
        workerSession: {
          closeoutStatus: "accepted",
          executionRuntime: "codex_harness",
          homeRole: "pylon_isolated_worker_codex_home",
          reviewState: "ready_for_review",
          role: "swarm_worker_codex_session",
          transcriptRef: "transcript.public.rpc",
        },
      }],
      availableCodexAssignments: 2,
      maxCodexAssignments: 3,
      pylon: {
        pylonRef: "pylon.local.rpc",
        status: "online",
      },
      sessionLayers: {
        main: {
          role: "main_local_codex_session",
          transcriptSurface: "chat",
        },
        workers: {
          homeRole: "pylon_isolated_worker_codex_home",
          role: "swarm_worker_codex_session",
          transcriptSurface: "fleet",
        },
      },
      tokenRate: {
        completedStatus: "exact",
        completedTokenRows: 2,
        completedTokensPerMinute: 48,
        inFlightTokens: 630,
        inFlightTokensPerMinute: 315,
      },
    })
  })
})
