import { randomUUID } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createCodexAppServerHost } from "../../../clients/khala-code-desktop/src/bun/codex-app-server-client.js"
import { createKhalaCodeDesktopRpcRequestHandlers } from "../../../clients/khala-code-desktop/src/bun/rpc-handlers.js"
import { buildKhalaAppleFmReadiness } from "../../../clients/khala-code-desktop/src/shared/apple-fm-readiness.js"
import type {
  KhalaCodexFleetCommandInput,
  KhalaCodexFleetCommandResult,
} from "../../../clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.js"
import type {
  KhalaCodeDesktopCodexHarnessStatus,
  KhalaCodeDesktopFleetRunProjection,
  KhalaCodeDesktopFleetRunState,
  KhalaCodeDesktopRPCSchema,
} from "../../../clients/khala-code-desktop/src/shared/rpc.js"

import type { KhalaCodeRpcFetch } from "./rpc-client.js"

export type KhalaCodeRealAppFetch = {
  readonly dispose: () => void
  readonly fetch: KhalaCodeRpcFetch
}

const response = (payload: unknown, status = 200): Response =>
  new Response(JSON.stringify(payload), {
    headers: { "content-type": "application/json" },
    status,
  })

const parseMethod = (input: RequestInfo | URL): keyof KhalaCodeDesktopRPCSchema["requests"] | null => {
  const candidate = decodeURIComponent(new URL(String(input)).pathname.split("/").pop() ?? "")
  return candidate.length === 0 ? null : candidate as keyof KhalaCodeDesktopRPCSchema["requests"]
}

const parseArgs = async (init?: RequestInit): Promise<readonly unknown[]> => {
  const text = typeof init?.body === "string" ? init.body : "{}"
  const parsed = JSON.parse(text) as { readonly args?: readonly unknown[] }
  return parsed.args ?? []
}

const ok = (stdout: unknown): KhalaCodexFleetCommandResult => ({
  exitCode: 0,
  signal: null,
  stderr: "",
  stdout: typeof stdout === "string" ? stdout : JSON.stringify(stdout),
  timedOut: false,
})

const failed = (stderr: string): KhalaCodexFleetCommandResult => ({
  exitCode: 1,
  signal: null,
  stderr,
  stdout: "",
  timedOut: false,
})

const pylonArgs = (input: KhalaCodexFleetCommandInput): readonly string[] => {
  const index = input.cmd.indexOf("src/index.ts")
  return index === -1 ? input.cmd : input.cmd.slice(index + 1)
}

const readyHarness = (): KhalaCodeDesktopCodexHarnessStatus => ({
  ok: true as const,
  app: "Khala Code Desktop" as const,
  available: true,
  capability: "codex_harness" as const,
  observedAt: "2026-07-01T00:00:00.000Z",
  reason: "ready",
  status: "ready" as const,
  binary: {
    command: "codex",
    source: "PATH" as const,
    available: true,
    version: "codex-cli fixture",
    error: null,
  },
  home: {
    path: "/tmp/khala-code-real-app-fetch/codex-home",
    source: "env:CODEX_HOME" as const,
    role: "main_user_codex_home" as const,
    authPath: "/tmp/khala-code-real-app-fetch/codex-home/auth.json",
    fleetIsolation: "fleet_accounts_use_pylon_isolated_homes" as const,
  },
  auth: {
    state: "ready" as const,
    blockerRefs: [],
    accessTokenPresent: true,
    accountIdPresent: false,
    refreshTokenPresent: false,
  },
  signIn: {
    required: false,
    command: "codex login" as const,
    warning: "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
  },
})

const fleetRunner = (): ((input: KhalaCodexFleetCommandInput) => Promise<KhalaCodexFleetCommandResult>) => {
  let advertised = false
  return async (input) => {
    const args = pylonArgs(input)
    const joined = args.join(" ")
    if (joined === "provider go-online --json") {
      return ok({
        ok: true,
        ownCapacityDispatch: {
          availableCodexAssignments: advertised ? 4 : 0,
          codexAccounts: advertised
            ? [{
                accountKey: "4db4cc18ebc55f39fb4da894",
                available: 4,
                busy: 1,
                queued: 0,
                ready: 5,
              }]
            : [],
          maxCodexAssignments: advertised ? 5 : 1,
        },
        pylonRef: "pylon.local.model",
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
      return ok({ accounts: [], schema: "openagents.pylon.accounts_status.v0.1" })
    }
    if (joined === "presence heartbeat --base-url https://openagents.com --json") {
      advertised = true
      return ok({ heartbeatRef: "heartbeat.pylon.local.model.1", pylonRef: "pylon.local.model" })
    }
    if (args[0] === "khala" && args[1] === "spawn") {
      return ok({
        aggregate: {
          acceptedCount: 1,
          assignmentRefs: ["assignment.public.codex_agent_task.model"],
          durableRequestIds: ["durable.public.model"],
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
          targetPylonRef: "pylon.local.model",
        },
        results: [{
          assignmentRef: "assignment.public.codex_agent_task.model",
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
}

const fleetRun = (state: KhalaCodeDesktopFleetRunState): KhalaCodeDesktopFleetRunProjection => ({
  counters: {
    activeAssignments: state === "running" ? 1 : 0,
    blockedAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    workUnitsTotal: 1,
  },
  createdAt: "2026-07-01T00:00:00.000Z",
  dispatchKind: "supervised_dispatch" as const,
  objectiveProjected: false as const,
  pylonRef: "pylon.local.model",
  refillPolicy: {
    cooldownAware: true,
    maxPerAccount: 1,
    stopCondition: "target_reached" as const,
  },
  runRef: "fleet-run-fixture",
  startedAt: "2026-07-01T00:00:00.000Z",
  state,
  targetConcurrency: 1,
  updatedAt: "2026-07-01T00:00:00.000Z",
  workerKind: "codex" as const,
  workSource: { kind: "fixture" as const, count: 1 },
})

export const makeKhalaCodeRealAppRpcFetch = async (): Promise<KhalaCodeRealAppFetch> => {
  const root = join(tmpdir(), `khala-code-real-app-fetch-${randomUUID()}`)
  const pylonAppPath = join(root, "apps", "pylon")
  const pylonHome = join(root, "pylon-home")
  const codexHome = join(root, "codex-home")
  await mkdir(pylonAppPath, { recursive: true })
  await mkdir(pylonHome, { recursive: true })
  await mkdir(codexHome, { recursive: true })
  await writeFile(join(pylonAppPath, "package.json"), JSON.stringify({ name: "@openagentsinc/pylon" }))
  const scriptPath = join(root, "no-approval-fixture-script.json")
  await writeFile(scriptPath, JSON.stringify({
    schema: "khala-code-desktop.fixture-codex-app-server-script.v1",
    name: "model-based-no-approval-turn",
    model: "gpt-5.1-codex-fixture",
    modelProvider: "openai",
    steps: [{
      kind: "notification",
      method: "turn/started",
      params: {
        threadId: "{{threadId}}",
        turn: { id: "{{turnId}}", status: "inProgress" },
      },
    }, {
      kind: "notification",
      method: "item/completed",
      params: {
        threadId: "{{threadId}}",
        turnId: "{{turnId}}",
        item: {
          type: "agentMessage",
          id: "item-agent-{{turnId}}",
          text: "Fixture app-server completed deterministically.",
        },
      },
    }, {
      kind: "notification",
      method: "thread/tokenUsage/updated",
      params: {
        threadId: "{{threadId}}",
        turnId: "{{turnId}}",
        info: {
          last_token_usage: {
            input_tokens: 11,
            cached_input_tokens: 2,
            output_tokens: 7,
            reasoning_output_tokens: 3,
            total_tokens: 21,
          },
          total_token_usage: {
            input_tokens: 11,
            cached_input_tokens: 2,
            output_tokens: 7,
            reasoning_output_tokens: 3,
            total_tokens: 21,
          },
        },
      },
    }, {
      kind: "notification",
      method: "turn/completed",
      params: {
        threadId: "{{threadId}}",
        turn: { id: "{{turnId}}", status: "completed" },
      },
    }],
  }))

  const env = {
    CODEX_HOME: codexHome,
    KHALA_CODE_CODEX_APP_SERVER_FIXTURE: "1",
    KHALA_CODE_CODEX_APP_SERVER_FIXTURE_SCRIPT: scriptPath,
    KHALA_CODE_DESKTOP_CODEX_STATE_PATH: join(root, "codex-sessions.json"),
    KHALA_CODE_DESKTOP_RUNTIME: "codex_harness",
    OPENAGENTS_BUN_PATH: process.execPath,
    OPENAGENTS_PYLON_APP_PATH: pylonAppPath,
    PYLON_HOME: pylonHome,
  }
  const host = createCodexAppServerHost({
    codexHomePath: codexHome,
    env,
    initializeTimeoutMs: 5_000,
    requestTimeoutMs: 5_000,
  })
  let fleetRunState: "running" | "paused" | "draining" | "stopped" = "stopped"
  const handlers = createKhalaCodeDesktopRpcRequestHandlers({
    appleFmReadiness: () => buildKhalaAppleFmReadiness({
      helperFound: false,
      observedAt: "2026-07-01T00:00:00.000Z",
      platform: { arch: process.arch, platform: process.platform },
    }),
    codexAppServerHost: host,
    codexFleetToolOptions: { env, runner: fleetRunner() },
    codexHarnessStatus: readyHarness,
    codexRateLimitStatus: () => ({
      provider: "codex" as const,
      session: null,
      weekly: null,
      rateLimitResetCredits: null,
      updatedAtIso: "2026-07-01T00:00:00.000Z",
      error: null,
      status: "ok" as const,
    }),
    env,
    fleetRunSupervisor: {
      control: async (request) => {
        const previousState = fleetRunState
        fleetRunState = request.verb === "pause"
          ? "paused"
          : request.verb === "resume"
            ? "running"
            : request.verb === "drain"
              ? "draining"
              : "stopped"
        return {
          previousState,
          run: fleetRun(fleetRunState),
          supervisorActive: fleetRunState === "running",
        }
      },
      list: async () => [fleetRun(fleetRunState)],
      start: async () => {
        fleetRunState = "running"
        return { run: fleetRun(fleetRunState), supervisorStarted: true }
      },
      status: async () => ({
        run: fleetRun(fleetRunState),
        supervisorActive: fleetRunState === "running",
      }),
    },
    onDeviceDeciderStatus: () => ({
      selected: null,
      preferred: "gpt_oss",
      reason: "Fixture model tier uses the Codex app-server harness.",
      readiness: [],
    }),
    workingDirectory: process.cwd(),
  })

  const fetch: KhalaCodeRpcFetch = async (input, init) => {
    const method = parseMethod(input)
    if (method === null || !(method in handlers)) {
      return response({ error: `Unknown Khala Code RPC method: ${String(method)}` }, 404)
    }
    try {
      const args = await parseArgs(init)
      const handler = handlers[method] as (...args: readonly unknown[]) => Promise<unknown>
      return response(await handler(...args))
    } catch (error) {
      return response({
        error: error instanceof Error ? error.message : String(error),
        method,
      }, 500)
    }
  }

  return {
    dispose: () => host.dispose(),
    fetch,
  }
}
