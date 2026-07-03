import { randomUUID } from "node:crypto"
import { chmod, mkdtemp, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

import { Effect } from "effect"

import { makeKhalaCodeRpcQaDriver } from "./rpc-driver.js"
import { runKhalaCodeQaScenario, type KhalaCodeQaScenarioRunReport } from "./runner.js"
import { KHALA_CODE_QA_SEED_SCENARIOS } from "./seed-corpus.js"
import type { KhalaCodeQaScenario } from "./scenario.js"

export const KHALA_CODE_REAL_BRIDGE_SMOKE_SCHEMA =
  "openagents.khala_code.real_bridge_smoke.v1"

const rpcTokenHeader = "x-khala-code-preview-token"
const currentDir = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(currentDir, "../../..")
const desktopCwd = join(repoRoot, "clients", "khala-code-desktop")
const desktopEntry = join(desktopCwd, "src", "bun", "index.ts")
const fixtureAppServerPath = join(desktopCwd, "src", "bun", "fixture-codex-app-server.ts")

export type KhalaCodeRealBridgeSmokeHost = {
  readonly baseUrl: string
  readonly child: Bun.Subprocess
  readonly command: string
  readonly dispose: () => void
  readonly token: string
}

export type KhalaCodeRealBridgeSmokeReport = {
  readonly schema: typeof KHALA_CODE_REAL_BRIDGE_SMOKE_SCHEMA
  readonly bearerAuth: {
    readonly acceptedStatus: number
    readonly rejectedStatus: number
  }
  readonly bridge: {
    readonly baseUrl: string
    readonly command: string
    readonly excludedScenarioIds: readonly string[]
    readonly fixtureAppServerPath: string
    readonly mode: "real_http_bearer_sse"
    readonly scenarioSource: "seed_corpus_transport_valid"
  }
  readonly scenarioCount: number
  readonly scenarios: readonly KhalaCodeQaScenarioRunReport[]
  readonly sse: {
    readonly connected: boolean
    readonly contentType: string | null
    readonly observedChatTurnEvent: boolean
  }
  readonly status: "pass" | "fail"
}

const randomPort = (): number =>
  52_000 + Math.floor(Math.random() * 8_000)

const fixtureScript = (name: string): string =>
  JSON.stringify({
    schema: "khala-code-desktop.fixture-codex-app-server-script.v1",
    name,
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
          id: "item-agent-{{turnId}}",
          text: "Fixture app-server completed through the real HTTP bridge.",
          type: "agentMessage",
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
            cached_input_tokens: 2,
            input_tokens: 11,
            output_tokens: 7,
            reasoning_output_tokens: 3,
            total_tokens: 21,
          },
          total_token_usage: {
            cached_input_tokens: 2,
            input_tokens: 11,
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
  }, null, 2)

const waitForHealth = async (
  baseUrl: string,
  child: Bun.Subprocess,
  timeoutMs: number,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs
  let lastError = ""
  while (Date.now() < deadline) {
    if (child.exitCode !== null) break
    try {
      const response = await fetch(`${baseUrl}/health`)
      if (response.ok) return
      lastError = `${response.status} ${response.statusText}`
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error)
    }
    await Bun.sleep(50)
  }
  throw new Error(`Khala Code real bridge did not become healthy: ${lastError}`)
}

export const startKhalaCodeRealBridgeSmokeHost = async (
  options: {
    readonly env?: Record<string, string | undefined>
    readonly port?: number
    readonly token?: string
    readonly waitTimeoutMs?: number
  } = {},
): Promise<KhalaCodeRealBridgeSmokeHost> => {
  const port = options.port ?? randomPort()
  const token = options.token ?? `real-bridge-smoke-${randomUUID()}`
  const root = await mkdtemp(join(tmpdir(), "khala-code-real-bridge-smoke-"))
  const scriptPath = join(root, "fixture-app-server-script.json")
  const fixtureCommandPath = join(root, "fixture-codex-app-server-command.sh")
  await writeFile(scriptPath, fixtureScript("real-bridge-seed-corpus"))
  await writeFile(
    fixtureCommandPath,
    [
      "#!/bin/sh",
      `exec ${JSON.stringify(process.execPath)} ${JSON.stringify(fixtureAppServerPath)} --stdio`,
      "",
    ].join("\n"),
  )
  await chmod(fixtureCommandPath, 0o755)

  const child = Bun.spawn([process.execPath, desktopEntry], {
    cwd: desktopCwd,
    env: {
      ...process.env,
      CODEX_HOME: join(root, "codex-home"),
      KHALA_CODE_CODEX_APP_SERVER_FIXTURE_SCRIPT: scriptPath,
      KHALA_CODE_CODEX_COMMAND: fixtureCommandPath,
      KHALA_CODE_DESKTOP_BUNDLED_SKILLS: "0",
      KHALA_CODE_DESKTOP_OPEN_WINDOW: "0",
      KHALA_CODE_DESKTOP_PREVIEW_PORT: String(port),
      KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN: token,
      KHALA_CODE_DESKTOP_WORKSPACE: root,
      KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED: "1",
      KHALA_CODE_TOKEN_USAGE_DISABLED: "1",
      PYLON_HOME: join(root, "pylon-home"),
      ...options.env,
    },
    stderr: "pipe",
    stdout: "pipe",
  })

  const baseUrl = `http://127.0.0.1:${port}`
  try {
    await waitForHealth(baseUrl, child, options.waitTimeoutMs ?? 8_000)
  } catch (error) {
    child.kill("SIGKILL")
    throw error
  }

  return {
    baseUrl,
    child,
    command: fixtureCommandPath,
    dispose: () => child.kill("SIGKILL"),
    token,
  }
}

const rpc = async (
  host: KhalaCodeRealBridgeSmokeHost,
  method: string,
  token: string | null,
  args: readonly unknown[] = [],
): Promise<Response> =>
  fetch(`${host.baseUrl}/rpc/${encodeURIComponent(method)}`, {
    body: JSON.stringify({ args }),
    headers: {
      "content-type": "application/json",
      ...(token === null ? {} : { [rpcTokenHeader]: token }),
    },
    method: "POST",
  })

const readUntilSseEvent = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
  expectedEvent: string,
  timeoutMs: number,
): Promise<unknown> => {
  const decoder = new TextDecoder()
  const deadline = Date.now() + timeoutMs
  let buffer = ""
  while (Date.now() < deadline) {
    const read = await Promise.race([
      reader.read(),
      Bun.sleep(250).then(() => null),
    ])
    if (read === null) continue
    if (read.done) break
    buffer += decoder.decode(read.value, { stream: true })
    const frames = buffer.split("\n\n")
    buffer = frames.pop() ?? ""
    for (const frame of frames) {
      if (!frame.includes(`event: ${expectedEvent}`)) continue
      const dataLine = frame.split("\n").find(line => line.startsWith("data: "))
      return dataLine === undefined ? null : JSON.parse(dataLine.slice("data: ".length)) as unknown
    }
  }
  throw new Error(`timed out waiting for SSE event ${expectedEvent}`)
}

const realBridgeInProcessOnlyScenarioIds = new Set([
  "scenario.khala_code.seed.rpc_threads_lifecycle.v1",
  "scenario.khala_code.seed.rpc_fleet_lifecycle.v1",
  "scenario.khala_code.seed.rpc_settings_lifecycle.v1",
  "scenario.khala_code.seed.rpc_threads_q41_completion.v1",
  "scenario.khala_code.seed.rpc_fleet_status_delegate_promote.v1",
  "scenario.khala_code.seed.rpc_fleet_run_lifecycle.v1",
  "scenario.khala_code.seed.rpc_forum_panel_lifecycle.v1",
  "scenario.khala_code.seed.rpc_inbox_routing_lifecycle.v1",
  "scenario.khala_code.seed.planner_coder_judge_judge_verdict_cards.v1",
  "scenario.khala_code.seed.planner_coder_judge_advisor_guards.v1",
])

const isRealBridgeTransportValidSeedScenario = (scenario: KhalaCodeQaScenario): boolean =>
  scenario.modes.includes("rpc") &&
  !scenario.id.includes(".error_state_") &&
  !scenario.id.includes(".cross_mode_") &&
  !scenario.id.includes(".thread_item_") &&
  !realBridgeInProcessOnlyScenarioIds.has(scenario.id)

export const KHALA_CODE_REAL_BRIDGE_EXCLUDED_SEED_SCENARIO_IDS: readonly string[] =
  KHALA_CODE_QA_SEED_SCENARIOS
    .filter(scenario => scenario.modes.includes("rpc") && !isRealBridgeTransportValidSeedScenario(scenario))
    .map(scenario => scenario.id)

const realBridgeSeedScenarios = (): readonly KhalaCodeQaScenario[] =>
  KHALA_CODE_QA_SEED_SCENARIOS.filter(isRealBridgeTransportValidSeedScenario)

export const runKhalaCodeRealBridgeSeedSmoke = async (
  options: {
    readonly host?: KhalaCodeRealBridgeSmokeHost
  } = {},
): Promise<KhalaCodeRealBridgeSmokeReport> => {
  const ownedHost = options.host === undefined
    ? await startKhalaCodeRealBridgeSmokeHost()
    : undefined
  const host = options.host ?? ownedHost!
  try {
    const rejected = await rpc(host, "appInfo", null)
    const accepted = await rpc(host, "appInfo", host.token)
    const events = await fetch(`${host.baseUrl}/rpc/events`, {
      headers: { [rpcTokenHeader]: host.token },
    })
    const reader = events.body?.getReader()
    if (reader === undefined) {
      throw new Error("real bridge SSE response did not include a readable body")
    }
    const sseEvent = readUntilSseEvent(reader, "chatTurnEvent", 12_000)

    try {
      const scenarios: KhalaCodeQaScenarioRunReport[] = []
      for (const scenario of realBridgeSeedScenarios()) {
        const driver = makeKhalaCodeRpcQaDriver({
          accessToken: host.token,
          baseUrl: host.baseUrl,
        })
        scenarios.push(await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario })))
      }

      await sseEvent
      return {
        schema: KHALA_CODE_REAL_BRIDGE_SMOKE_SCHEMA,
        bearerAuth: {
          acceptedStatus: accepted.status,
          rejectedStatus: rejected.status,
        },
        bridge: {
          baseUrl: host.baseUrl,
          command: host.command,
          excludedScenarioIds: KHALA_CODE_REAL_BRIDGE_EXCLUDED_SEED_SCENARIO_IDS,
          fixtureAppServerPath,
          mode: "real_http_bearer_sse",
          scenarioSource: "seed_corpus_transport_valid",
        },
        scenarioCount: scenarios.length,
        scenarios,
        sse: {
          connected: events.ok,
          contentType: events.headers.get("content-type"),
          observedChatTurnEvent: true,
        },
        status: rejected.status === 401 && accepted.ok && events.ok && scenarios.every(report => report.status === "pass")
          ? "pass"
          : "fail",
      }
    } finally {
      await reader.cancel().catch(() => undefined)
    }
  } finally {
    ownedHost?.dispose()
  }
}

if (import.meta.main) {
  const report = await runKhalaCodeRealBridgeSeedSmoke()
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)
  process.exit(report.status === "pass" ? 0 : 1)
}
