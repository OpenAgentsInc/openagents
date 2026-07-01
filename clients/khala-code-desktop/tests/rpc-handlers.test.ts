import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createKhalaCodeDesktopRpcRequestHandlers } from "../src/bun/rpc-handlers"
import type { CodexAppServerChatRuntime } from "../src/bun/codex-app-server-chat-runtime"
import type {
  KhalaCodexFleetCommandInput,
  KhalaCodexFleetCommandResult,
} from "../src/bun/khala-codex-fleet-tools"
import type {
  KhalaCodeDesktopChatTurnResponse,
  KhalaCodeDesktopCodexAppServerControlResult,
  KhalaCodeDesktopCodexAppServerStatus,
  KhalaCodeDesktopCodexHarnessStatus,
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
      warning: "Khala Code never starts Codex login against the default home automatically; fleet accounts stay in isolated Pylon homes.",
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

function throwingCodexChatRuntime(
  overrides: Partial<CodexAppServerChatRuntime> = {},
): CodexAppServerChatRuntime {
  return {
    compactThread: async () => {
      throw new Error("codex compact should not be called")
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
    startThread: async () => {
      throw new Error("codex start should not be called")
    },
    startTurn: async () => {
      throw new Error("codex turn should not be called")
    },
    steerTurn: async () => {
      throw new Error("codex steer should not be called")
    },
    ...overrides,
  }
}

describe("Khala Code desktop RPC handlers", () => {
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
      available: false,
      capability: "token_accounting",
      ok: true,
      status: "not_configured",
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
        expect(input.idempotencyKey).toBeTruthy()
        return "noCredit"
      },
      env: { CODEX_HOME: "/tmp/codex-home" },
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handler.consumeCodexRateLimitResetCredit()).resolves.toMatchObject({
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
      reason: "Codex auth.json is missing. Run codex login intentionally in the main user home.",
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
        warning: "Khala Code never starts Codex login against the default home automatically; fleet accounts stay in isolated Pylon homes.",
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
      turnId: "desktop-turn-1",
    })).resolves.toMatchObject({
      backend: {
        kind: "codex_app_server",
        threadId: "thread-codex-default",
      },
      messages: [{ body: "Codex default path" }],
      ok: true,
    })
    expect(codexTurnStarted).toBe(true)
    expect(legacyTurnStarted).toBe(false)
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
      },
      messages: [{ body: "Legacy runtime" }],
      ok: true,
    })
    expect(legacyTurnStarted).toBe(true)
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
      refreshedAt: "2026-06-30T18:00:00.000Z",
      schema: "openagents.pylon.active_assignment_run.v0.1",
      service: "codex",
      startedAt: "2026-06-30T17:58:00.000Z",
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
        quotaState: "available",
        readiness: "ready",
      }],
      activeAssignments: [{
        assignmentRef: "assignment.public.rpc",
        tokenRate: {
          status: "exact",
          tokenCountKind: "exact",
          tokens: 630,
          tokensPerMinute: 315,
        },
      }],
      availableCodexAssignments: 2,
      maxCodexAssignments: 3,
      pylon: {
        pylonRef: "pylon.local.rpc",
        status: "online",
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
