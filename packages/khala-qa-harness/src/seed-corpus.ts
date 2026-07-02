import {
  khalaCodeDesktopSlashCommandsWithAvailability,
  type KhalaCodeDesktopSlashCommandWithAvailability,
} from "../../../clients/khala-code-desktop/src/shared/codex-slash-commands.js"

import type { KhalaCodeRpcFetch } from "./rpc-client.js"
import type { KhalaCodeQaScenario } from "./scenario.js"

type SeedCorpusGroup =
  | "rpc.threads"
  | "rpc.turns"
  | "rpc.fleet"
  | "rpc.approvals"
  | "rpc.settings"
  | "rpc.ecosystem"
  | "rpc.slash_commands"
  | "hotbar"
  | "thread_items"

type ScenarioGroupEntry = Readonly<{
  group: SeedCorpusGroup
  scenarioIds: readonly string[]
}>

export type KhalaCodeQaSeedCorpusManifest = Readonly<{
  schema: "khala_code_qa_seed_corpus_manifest.v1"
  backend: "fixture"
  scenarioIdsByGroup: readonly ScenarioGroupEntry[]
  scenarioCount: number
}>

const observedAt = "2026-07-01T00:00:00.000Z"
const desktopSessionId = "desktop-session-fixture"
const threadId = "thread-fixture"
const forkThreadId = "thread-fork-fixture"
const turnId = "turn-fixture"
const runRef = "fleet-run-fixture"

const groupScenarioIds = new Map<SeedCorpusGroup, string[]>()

const track = <A extends KhalaCodeQaScenario>(
  group: SeedCorpusGroup,
  scenario: A,
): A => {
  const current = groupScenarioIds.get(group) ?? []
  current.push(scenario.id)
  groupScenarioIds.set(group, current)
  return scenario
}

const schema = (query: string) => ({ oracle: "schema" as const, query })
const crash = () => ({ oracle: "crash" as const })
const consistency = (left: string, right: string) => ({
  left,
  oracle: "consistency" as const,
  right,
})

const commitment = (id: string, claim: string, match: string) => ({
  claim,
  evidence: "phase-oracle" as const,
  id,
  match,
})

const runPass = (id: string, claim: string) => ({
  claim,
  evidence: "run-pass" as const,
  id,
})

const fixtureScenario = (
  group: SeedCorpusGroup,
  id: string,
  phases: KhalaCodeQaScenario["phases"],
  commitments: KhalaCodeQaScenario["commitments"],
): KhalaCodeQaScenario =>
  track(group, {
    backend: "fixture",
    commitments,
    id,
    modes: ["rpc"],
    phases,
  })

const rpcGroupScenarios: readonly KhalaCodeQaScenario[] = [
  fixtureScenario(
    "rpc.threads",
    "scenario.khala_code.seed.rpc_threads_lifecycle.v1",
    [
      {
        name: "start-thread",
        act: [{ kind: "rpc_call", method: "codexThreadStart", args: [{ sessionId: desktopSessionId }] }],
        expect: [schema("codexThreadStart"), crash()],
      },
      {
        name: "list-and-read-thread",
        act: [
          { kind: "rpc_call", method: "codexThreadList", args: [{ sessionId: desktopSessionId }] },
          { kind: "rpc_call", method: "codexThreadList", args: [{ sessionId: desktopSessionId }] },
          { kind: "rpc_call", method: "codexThreadRead", args: [{ threadId, includeTurns: true }] },
        ],
        expect: [
          schema("codexThreadList"),
          schema("codexThreadRead"),
          consistency("rpc:codexThreadList", "codexThreadList"),
          crash(),
        ],
      },
      {
        name: "mutate-thread",
        act: [
          { kind: "rpc_call", method: "codexThreadRename", args: [{ threadId, name: "Renamed fixture" }] },
          { kind: "rpc_call", method: "codexThreadArchive", args: [{ threadId }] },
          { kind: "rpc_call", method: "codexThreadUnarchive", args: [{ threadId }] },
          { kind: "rpc_call", method: "codexThreadFork", args: [{ threadId, sessionId: desktopSessionId }] },
        ],
        expect: [schema("codexThreadFork"), crash()],
      },
    ],
    [
      commitment("seed.rpc_threads.schema", "thread lifecycle RPC responses decode", "schema"),
      commitment("seed.rpc_threads.consistency", "thread list can be read consistently", "consistency"),
      runPass("seed.rpc_threads.pass", "thread lifecycle scenario passes"),
    ],
  ),
  fixtureScenario(
    "rpc.turns",
    "scenario.khala_code.seed.rpc_turns_lifecycle.v1",
    [
      {
        name: "start-turn",
        act: [
          {
            kind: "rpc_call",
            method: "codexTurnStart",
            args: [{
              messages: [{ id: "msg-user", role: "user", body: "fixture turn" }],
              sessionId: desktopSessionId,
              threadId,
              turnId,
            }],
          },
        ],
        expect: [schema("codexTurnStart"), crash()],
      },
      {
        name: "steer-interrupt-compact",
        act: [
          { kind: "rpc_call", method: "codexTurnSteer", args: [{ sessionId: desktopSessionId, turnId, text: "fixture steer" }] },
          { kind: "rpc_call", method: "codexTurnInterrupt", args: [{ sessionId: desktopSessionId, turnId }] },
          { kind: "rpc_call", method: "codexThreadCompact", args: [{ sessionId: desktopSessionId, threadId }] },
        ],
        expect: [schema("codexThreadCompact"), crash()],
      },
    ],
    [
      commitment("seed.rpc_turns.schema", "turn lifecycle RPC responses decode", "schema"),
      runPass("seed.rpc_turns.pass", "turn lifecycle scenario passes"),
    ],
  ),
  fixtureScenario(
    "rpc.fleet",
    "scenario.khala_code.seed.rpc_fleet_lifecycle.v1",
    [
      {
        name: "fleet-status-and-delegate",
        act: [
          { kind: "rpc_call", method: "codexFleetStatus" },
          {
            kind: "rpc_call",
            method: "codexFleetDelegateRun",
            args: [{ objective: "fixture delegation", mode: "fixture", count: 1, noRun: true }],
          },
        ],
        expect: [schema("codexFleetStatus"), schema("codexFleetDelegateRun"), crash()],
      },
      {
        name: "fleet-run-verbs",
        act: [
          {
            kind: "rpc_call",
            method: "fleetRunStart",
            args: [{
              objective: "fixture sustained run",
              runRef,
              targetConcurrency: 1,
              workerKind: "codex",
              workSource: { kind: "fixture", count: 1 },
            }],
          },
          { kind: "rpc_call", method: "fleetRunStatus", args: [{ runRef }] },
          { kind: "rpc_call", method: "fleetRunList", args: [{}] },
          { kind: "rpc_call", method: "fleetRunControl", args: [{ runRef, verb: "pause" }] },
          { kind: "rpc_call", method: "fleetRunControl", args: [{ runRef, verb: "resume" }] },
          { kind: "rpc_call", method: "fleetRunControl", args: [{ runRef, verb: "drain" }] },
          { kind: "rpc_call", method: "fleetRunControl", args: [{ runRef, verb: "stop" }] },
        ],
        expect: [schema("fleetRunControl"), crash()],
      },
    ],
    [
      commitment("seed.rpc_fleet.schema", "fleet and FleetRun RPC responses decode", "schema"),
      runPass("seed.rpc_fleet.pass", "fleet lifecycle scenario passes"),
    ],
  ),
  fixtureScenario(
    "rpc.approvals",
    "scenario.khala_code.seed.rpc_approvals_lifecycle.v1",
    [
      {
        name: "approval-respond",
        act: [{
          kind: "rpc_call",
          method: "codexApprovalRespond",
          args: [{
            action: "accept",
            method: "item/commandExecution/requestApproval",
            requestId: "approval-fixture",
          }],
        }],
        expect: [schema("codexApprovalRespond"), crash()],
      },
    ],
    [
      commitment("seed.rpc_approvals.schema", "approval RPC responses decode", "schema"),
      runPass("seed.rpc_approvals.pass", "approval lifecycle scenario passes"),
    ],
  ),
  fixtureScenario(
    "rpc.settings",
    "scenario.khala_code.seed.rpc_settings_lifecycle.v1",
    [
      {
        name: "read-settings",
        act: [
          { kind: "rpc_call", method: "codexSettingsRead", args: [{ cwd: "/workspace" }] },
          { kind: "rpc_call", method: "codexSettingsRead", args: [{ cwd: "/workspace" }] },
        ],
        expect: [
          schema("codexSettingsRead"),
          consistency("rpc:codexSettingsRead", "codexSettingsRead"),
          crash(),
        ],
      },
      {
        name: "write-config",
        act: [{
          kind: "rpc_call",
          method: "codexConfigValueWrite",
          args: [{ keyPath: "model", value: "gpt-5.1-codex" }],
        }],
        expect: [schema("codexConfigValueWrite"), crash()],
      },
    ],
    [
      commitment("seed.rpc_settings.schema", "settings RPC responses decode", "schema"),
      commitment("seed.rpc_settings.consistency", "settings reads are consistent", "consistency"),
      runPass("seed.rpc_settings.pass", "settings lifecycle scenario passes"),
    ],
  ),
  fixtureScenario(
    "rpc.ecosystem",
    "scenario.khala_code.seed.rpc_ecosystem_lifecycle.v1",
    [
      {
        name: "read-ecosystem",
        act: [
          { kind: "rpc_call", method: "codexEcosystemRead", args: [{ cwd: "/workspace" }] },
          { kind: "rpc_call", method: "codexMentionCandidates", args: [{ cwd: "/workspace", query: "README" }] },
        ],
        expect: [schema("codexEcosystemRead"), schema("codexMentionCandidates"), crash()],
      },
      {
        name: "background-terminal-ecosystem",
        act: [
          { kind: "rpc_call", method: "codexBackgroundTerminalsList", args: [{ threadId, limit: 10 }] },
          { kind: "rpc_call", method: "codexMcpServerReload" },
        ],
        expect: [schema("codexMcpServerReload"), crash()],
      },
    ],
    [
      commitment("seed.rpc_ecosystem.schema", "ecosystem RPC responses decode", "schema"),
      runPass("seed.rpc_ecosystem.pass", "ecosystem lifecycle scenario passes"),
    ],
  ),
  fixtureScenario(
    "rpc.slash_commands",
    "scenario.khala_code.seed.rpc_slash_commands_registry.v1",
    [
      {
        name: "list-slash-commands",
        act: [
          { kind: "rpc_call", method: "slashCommandList", args: [{ debug: true, platform: "darwin" }] },
          { kind: "rpc_call", method: "slashCommandList", args: [{ debug: true, platform: "darwin" }] },
        ],
        expect: [
          schema("slashCommandList"),
          consistency("rpc:slashCommandList", "slashCommandList"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_slash_commands.schema", "slash command list response decodes", "schema"),
      commitment("seed.rpc_slash_commands.consistency", "slash command registry reads are consistent", "consistency"),
      runPass("seed.rpc_slash_commands.pass", "slash command registry scenario passes"),
    ],
  ),
]

const hotbarScenarios: readonly KhalaCodeQaScenario[] = [
  fixtureScenario(
    "hotbar",
    "scenario.khala_code.seed.hotbar_chat_panel.v1",
    [{
      name: "chat-panel-rpc",
      act: [{ kind: "rpc_call", method: "codingStatus" }, { kind: "rpc_call", method: "appInfo" }],
      expect: [schema("codingStatus"), schema("appInfo"), crash()],
    }],
    [
      commitment("seed.hotbar.chat.schema", "chat hotbar panel backing RPC decodes", "schema"),
      runPass("seed.hotbar.chat.pass", "chat hotbar scenario passes"),
    ],
  ),
  fixtureScenario(
    "hotbar",
    "scenario.khala_code.seed.hotbar_fleet_panel.v1",
    [{
      name: "fleet-panel-rpc",
      act: [{ kind: "rpc_call", method: "codexFleetStatus" }, { kind: "rpc_call", method: "fleetRunList", args: [{}] }],
      expect: [schema("codexFleetStatus"), schema("fleetRunList"), crash()],
    }],
    [
      commitment("seed.hotbar.fleet.schema", "fleet hotbar panel backing RPC decodes", "schema"),
      runPass("seed.hotbar.fleet.pass", "fleet hotbar scenario passes"),
    ],
  ),
  fixtureScenario(
    "hotbar",
    "scenario.khala_code.seed.hotbar_settings_panel.v1",
    [{
      name: "settings-panel-rpc",
      act: [{ kind: "rpc_call", method: "codexSettingsRead", args: [{}] }, { kind: "rpc_call", method: "codexEcosystemRead", args: [{}] }],
      expect: [schema("codexSettingsRead"), schema("codexEcosystemRead"), crash()],
    }],
    [
      commitment("seed.hotbar.settings.schema", "settings hotbar panel backing RPC decodes", "schema"),
      runPass("seed.hotbar.settings.pass", "settings hotbar scenario passes"),
    ],
  ),
]

export const KHALA_CODE_QA_THREAD_ITEM_VARIANTS = [
  "hookPrompt",
  "plan",
  "commandExecution",
  "fileChange",
  "mcpToolCall",
  "dynamicToolCall",
  "collabAgentToolCall",
  "subAgentActivity",
  "webSearch",
  "imageView",
  "sleep",
  "imageGeneration",
  "enteredReviewMode",
  "exitedReviewMode",
  "contextCompaction",
  "approval",
  "approvalReview",
] as const

const threadItemScenarios: readonly KhalaCodeQaScenario[] =
  KHALA_CODE_QA_THREAD_ITEM_VARIANTS.map((variant) =>
    fixtureScenario(
      "thread_items",
      `scenario.khala_code.seed.thread_item_${variant.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)}.v1`,
      [{
        name: "read-thread-item",
        act: [{ kind: "rpc_call", method: "codexThreadRead", args: [{ threadId: `thread-item-${variant}`, includeTurns: true }] }],
        expect: [schema("codexThreadRead"), crash()],
      }],
      [
        commitment(`seed.thread_item.${variant}.schema`, `${variant} ThreadItem replay decodes`, "schema"),
        runPass(`seed.thread_item.${variant}.pass`, `${variant} ThreadItem replay scenario passes`),
      ],
    )
  )

const slashCommandScenarios: readonly KhalaCodeQaScenario[] =
  khalaCodeDesktopSlashCommandsWithAvailability({ debug: true, platform: "darwin" }).map((command) =>
    fixtureScenario(
      "rpc.slash_commands",
      `scenario.khala_code.seed.slash_command_${command.command.replace(/[^a-z0-9]+/g, "_")}.v1`,
      [{
        name: "dispatch-slash-command",
        act: [{
          kind: "rpc_call",
          method: "slashCommandDispatch",
          args: [slashCommandDispatchRequest(command)],
        }],
        expect: [schema("slashCommandDispatch"), crash()],
      }],
      [
        commitment(`seed.slash_command.${command.command}.schema`, `/${command.command} dispatch response decodes`, "schema"),
        runPass(`seed.slash_command.${command.command}.pass`, `/${command.command} scenario passes`),
      ],
    )
  )

export const KHALA_CODE_QA_SEED_SCENARIOS: readonly KhalaCodeQaScenario[] = [
  ...rpcGroupScenarios,
  ...hotbarScenarios,
  ...threadItemScenarios,
  ...slashCommandScenarios,
]

export const KHALA_CODE_QA_SEED_CORPUS_MANIFEST: KhalaCodeQaSeedCorpusManifest = {
  schema: "khala_code_qa_seed_corpus_manifest.v1",
  backend: "fixture",
  scenarioIdsByGroup: [...groupScenarioIds.entries()].map(([group, scenarioIds]) => ({
    group,
    scenarioIds,
  })),
  scenarioCount: KHALA_CODE_QA_SEED_SCENARIOS.length,
}

function slashCommandDispatchRequest(
  command: KhalaCodeDesktopSlashCommandWithAvailability,
): {
  readonly activeTurn: boolean
  readonly debug: boolean
  readonly platform: string
  readonly raw: string
  readonly sessionId: string
  readonly sideConversation: boolean
  readonly threadId?: string
} {
  return {
    activeTurn: command.availableDuringTask,
    debug: command.debug,
    platform: "darwin",
    raw: slashCommandRaw(command),
    sessionId: desktopSessionId,
    sideConversation: false,
    ...(command.dispatch.kind === "app_server" && command.dispatch.requiresThread === true ? { threadId } : {}),
  }
}

function slashCommandRaw(
  command: KhalaCodeDesktopSlashCommandWithAvailability,
): string {
  return command.supportsInlineArgs || command.dispatch.kind === "app_server" && command.dispatch.requiresArgs === true
    ? `/${command.command} fixture`
    : `/${command.command}`
}

const response = (payload: unknown): Response =>
  new Response(JSON.stringify(payload), {
    headers: { "content-type": "application/json" },
    status: 200,
  })

const parseMethod = (input: RequestInfo | URL): string =>
  new URL(String(input)).pathname.split("/").pop() ?? ""

const parseArgs = async (init?: RequestInit): Promise<readonly unknown[]> => {
  const text = typeof init?.body === "string" ? init.body : "{}"
  const parsed = JSON.parse(text) as { readonly args?: readonly unknown[] }
  return parsed.args ?? []
}

export const makeKhalaCodeQaSeedCorpusFixtureFetch = (): KhalaCodeRpcFetch => {
  let fleetRunState: "draft" | "running" | "paused" | "draining" | "completed" | "stopped" = "draft"
  return async (input, init) => {
    const method = parseMethod(input)
    const args = await parseArgs(init)
    return response(fixtureRpcPayload(method, args, {
      fleetRunState,
      setFleetRunState: (state) => {
        fleetRunState = state
      },
    }))
  }
}

const fixtureRpcPayload = (
  method: string,
  args: readonly unknown[],
  state: {
    readonly fleetRunState: "draft" | "running" | "paused" | "draining" | "completed" | "stopped"
    readonly setFleetRunState: (state: "draft" | "running" | "paused" | "draining" | "completed" | "stopped") => void
  },
): unknown => {
  switch (method) {
    case "appInfo":
      return { app: "Khala Code Desktop", ok: true, observedAt }
    case "codingStatus":
      return runtimeStatus("coding", "ready")
    case "codexThreadStart":
      return threadResult(threadId)
    case "codexThreadList":
      return threadListResult()
    case "codexThreadRead": {
      const request = args[0] as { readonly threadId?: string } | undefined
      return threadResult(request?.threadId ?? threadId)
    }
    case "codexThreadRename":
      return threadMutation("rename", threadId)
    case "codexThreadArchive":
      return threadMutation("archive", threadId)
    case "codexThreadUnarchive":
      return threadMutation("unarchive", threadId)
    case "codexThreadFork":
      return { ...threadMutation("fork", threadId), newThreadId: forkThreadId }
    case "codexTurnStart":
      return chatTurnResponse()
    case "codexTurnSteer":
    case "codexTurnInterrupt":
    case "codexThreadCompact":
      return turnActionResult()
    case "codexFleetStatus":
      return fleetStatus()
    case "codexFleetDelegateRun":
      return fleetDelegateRunResult()
    case "fleetRunStart":
      state.setFleetRunState("running")
      return { ok: true, run: fleetRun("running"), supervisorStarted: true }
    case "fleetRunStatus":
      return { ok: true, run: fleetRun(state.fleetRunState), supervisorActive: state.fleetRunState === "running" }
    case "fleetRunList":
      return { ok: true, runs: [fleetRun(state.fleetRunState)] }
    case "fleetRunControl": {
      const request = args[0] as { readonly verb?: "pause" | "resume" | "drain" | "stop" } | undefined
      const previousState = state.fleetRunState
      const next = fleetRunStateForVerb(request?.verb)
      state.setFleetRunState(next)
      return {
        ok: true,
        previousState,
        run: fleetRun(next),
        supervisorActive: next === "running",
        verb: request?.verb ?? "stop",
      }
    }
    case "codexApprovalRespond": {
      const request = args[0] as { readonly method?: string; readonly requestId?: string | number } | undefined
      return {
        method: request?.method ?? "item/commandExecution/requestApproval",
        ok: true,
        requestId: request?.requestId ?? "approval-fixture",
      }
    }
    case "codexSettingsRead":
      return settingsProjection()
    case "codexConfigValueWrite":
      return { ok: true, keyPath: "model", response: { applied: true }, settings: settingsProjection() }
    case "codexEcosystemRead":
      return ecosystemProjection()
    case "codexMentionCandidates":
      return { ok: true, candidates: [{ fileName: "README.md", kind: "file", path: "/workspace/README.md" }], source: "fuzzyFileSearch", truncated: false }
    case "codexBackgroundTerminalsList":
      return actionResult("thread/backgroundTerminals/list", { processes: [] })
    case "codexMcpServerReload":
      return actionResult("config/mcpServer/reload", { reloaded: true })
    case "fleetWorkerControl":
      return { accepted: true, assignmentRef: "assignment-fixture", inboxItemRef: "inbox-fixture", ok: true, runRef, verb: "flag", workerRefHash: "worker-fixture" }
    case "slashCommandList":
      return { ok: true, commands: khalaCodeDesktopSlashCommandsWithAvailability({ debug: true, platform: "darwin" }) }
    case "slashCommandDispatch": {
      const request = args[0] as { readonly raw?: string } | undefined
      const raw = request?.raw ?? "/status"
      const command = khalaCodeDesktopSlashCommandsWithAvailability({ debug: true, platform: "darwin" })
        .find((item) => raw.replace(/^\/+/, "").split(/\s+/)[0] === item.command)
      return slashCommandDispatchResult(command, raw)
    }
    default:
      return actionResult(method, { fixture: true })
  }
}

const fleetRunStateForVerb = (
  verb: "pause" | "resume" | "drain" | "stop" | undefined,
): "running" | "paused" | "draining" | "stopped" => {
  switch (verb) {
    case "pause":
      return "paused"
    case "resume":
      return "running"
    case "drain":
      return "draining"
    case "stop":
    case undefined:
      return "stopped"
  }
}

const runtimeStatus = (
  capability: "codex_accounts" | "codex_harness" | "coding" | "pylon" | "token_accounting",
  status: "error" | "not_configured" | "ready" | "unavailable",
) => ({
  app: "Khala Code Desktop",
  available: status === "ready",
  capability,
  observedAt,
  ok: true,
  reason: "fixture backend",
  status,
})

const threadMessage = (id: string, body: string, itemType?: string) => ({
  body,
  id,
  role: "assistant",
  ...(itemType === undefined
    ? {}
    : {
      codexItem: {
        itemId: id,
        itemType,
        status: "completed",
        title: `${itemType} fixture`,
        threadId,
        turnId,
      },
      harnessItem: {
        itemId: id,
        itemType,
        status: "completed",
        title: `${itemType} fixture`,
        threadId,
        turnId,
      },
    }),
})

const threadResult = (id: string) => {
  const itemType = id.startsWith("thread-item-") ? id.slice("thread-item-".length) : undefined
  return {
    cwd: "/workspace",
    desktopSessionId,
    messages: [threadMessage(`message-${id}`, "fixture message", itemType)],
    model: "gpt-5.1-codex",
    modelProvider: "openai",
    ok: true,
    thread: { id, title: "Fixture thread" },
    threadId: id,
  }
}

const threadSummary = (id: string) => ({
  badges: [],
  createdAt: 1,
  cwd: "/workspace",
  forkedFromId: null,
  id,
  modelProvider: "openai",
  parentThreadId: null,
  preview: "fixture message",
  projectLabel: "workspace",
  recencyAt: 1,
  sessionId: desktopSessionId,
  source: "fixture",
  status: "ready",
  statusLabel: "Ready",
  title: "Fixture thread",
  updatedAt: 1,
})

const threadListResult = () => ({
  backwardsCursor: null,
  data: [threadSummary(threadId)],
  groups: [{ key: "recent", label: "Recent", threadIds: [threadId] }],
  nextCursor: null,
  ok: true,
  threads: [threadSummary(threadId)],
})

const threadMutation = (action: "archive" | "delete" | "fork" | "rename" | "unarchive", id: string) => ({
  action,
  ok: true,
  thread: { id },
  threadId: id,
})

const turnActionResult = () => ({
  codexTurnId: turnId,
  desktopSessionId,
  desktopTurnId: turnId,
  ok: true,
  threadId,
})

const chatTurnResponse = () => ({
  backend: {
    kind: "mock",
    model: "fixture-model",
    threadId,
    turnId,
  },
  messages: [
    { body: "fixture turn response", id: "msg-assistant", role: "assistant" },
  ],
  ok: true,
  toolNames: [],
  usedTools: [],
  usage: {
    cachedInput: 0,
    input: 1,
    output: 1,
    reasoningOutput: 0,
  },
})

const fleetStatus = () => ({
  accounts: [],
  activeAssignments: [],
  availableCodexAssignments: 1,
  maxCodexAssignments: 1,
  observedAt,
  ok: true,
  processes: [],
  pylon: {
    message: "fixture pylon",
    pylonRef: "pylon-fixture",
    status: "online",
  },
  sessionLayers: {
    main: {
      homeRole: "main_user_codex_home_display_only",
      label: "Main local Codex",
      mutationPolicy: "codex_app_server_owned",
      role: "main_local_codex_session",
      runtime: "codex_harness",
      transcriptSurface: "chat",
    },
    workers: {
      homeRole: "pylon_isolated_worker_codex_home",
      label: "Fleet workers",
      mutationPolicy: "pylon_isolated_home_only",
      role: "swarm_worker_codex_session",
      runtime: "codex_harness",
      transcriptSurface: "fleet",
    },
  },
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: "fixture",
  },
})

const workerRuntime = {
  assignmentTool: "codex_spawn",
  homeRole: "pylon_isolated_worker_codex_home",
  role: "swarm_worker_codex_session",
  runtime: "codex_harness",
}

const fleetDelegateRunResult = () => ({
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
  pylonRef: "pylon-fixture",
  requestedCount: 1,
  results: [{
    accountRef: "codex",
    assignmentRef: "assignment-fixture",
    blockerRefs: [],
    closeoutStatus: "not_applicable",
    slot: 1,
    status: "accepted",
    tokensVerified: 0,
    transcriptRef: null,
  }],
  trace: [{
    blockerCode: null,
    fallbackModule: null,
    module: "fixture",
    precondition: "fixture backend",
    refs: [],
    status: "ok",
    summary: "fixture delegate path",
  }],
  validation: {
    fixture: true,
    repoPinsComplete: false,
  },
  workerRuntime,
})

const fleetRun = (state: "draft" | "running" | "paused" | "draining" | "completed" | "stopped") => ({
  counters: {
    activeAssignments: state === "running" ? 1 : 0,
    blockedAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    workUnitsTotal: 1,
  },
  createdAt: observedAt,
  dispatchKind: "supervised_dispatch",
  objectiveProjected: false,
  pylonRef: "pylon-fixture",
  refillPolicy: {
    cooldownAware: true,
    maxPerAccount: 1,
    stopCondition: "target_reached",
  },
  runRef,
  startedAt: observedAt,
  state,
  targetConcurrency: 1,
  updatedAt: observedAt,
  workerKind: "codex",
  workSource: { kind: "fixture", count: 1 },
})

const settingsProjection = () => {
  const model = {
    defaultReasoningEffort: "medium",
    defaultServiceTier: "auto",
    description: "Fixture model",
    displayName: "GPT-5.1 Codex",
    hidden: false,
    id: "gpt-5.1-codex",
    isDefault: true,
    model: "gpt-5.1-codex",
    serviceTiers: [{ id: "auto", name: "Auto", description: null }],
    supportedReasoningEfforts: [{ value: "medium", description: null }],
    supportsPersonality: true,
  }
  return {
    appearance: {
      keymap: {},
      keyPaths: {
        keymap: "tui.keymap",
        pet: "tui.pet",
        petAnchor: "tui.pet_anchor",
        personality: "personality",
        statusLine: "tui.status_line",
        statusLineUseColors: "tui.status_line_use_colors",
        theme: "tui.theme",
        vimModeDefault: "tui.vim_mode_default",
      },
      personality: null,
      pet: null,
      petAnchor: null,
      statusLine: null,
      statusLineUseColors: null,
      theme: "system",
      vimModeDefault: false,
    },
    collaboration: {
      currentMode: "solo",
      modes: [{ mode: "solo", model: "gpt-5.1-codex", name: "Solo", reasoningEffort: "medium" }],
      personality: null,
    },
    config: {
      approvalPolicy: "on-request",
      approvalsReviewer: "human",
      defaultPermissions: null,
      layersAvailable: false,
      model: "gpt-5.1-codex",
      modelProvider: "openai",
      originKeys: [],
      personality: null,
      reasoningEffort: "medium",
      reasoningSummary: null,
      sandboxMode: "workspace-write",
      serviceTier: "auto",
      verbosity: null,
      webSearch: null,
    },
    cwd: "/workspace",
    errors: [],
    models: {
      options: [model],
      selected: model,
      serviceTierCommands: [],
    },
    observedAt,
    ok: true,
    permissions: {
      blockedProfileIds: [],
      profiles: [{ allowed: true, description: "Fixture", id: "workspace-write", selected: true }],
      selectedProfile: "workspace-write",
    },
    providerCapabilities: {
      imageGeneration: false,
      namespaceTools: true,
      webSearch: false,
    },
    requirements: {
      allowedApprovalPolicies: null,
      allowedPermissionProfiles: null,
      allowedSandboxModes: null,
      blockers: [],
      defaultPermissions: null,
      managed: false,
    },
    usage: {
      available: false,
      dailyUsageBuckets: null,
      summary: null,
    },
  }
}

const ecosystemSection = (source: "apps" | "hooks" | "imports" | "khala" | "marketplace" | "mcp" | "plugins" | "skills", label: string) => ({
  authRequiredCount: 0,
  count: 1,
  disabledCount: 0,
  errorCount: 0,
  installRequiredCount: 0,
  items: [{
    authRequired: false,
    detail: "fixture ready",
    enabled: true,
    id: `${source}-fixture`,
    installed: true,
    managed: false,
    name: label,
    source,
    state: "ready",
  }],
  label,
  managedCount: 0,
  readyCount: 1,
  source,
  unknownCount: 0,
})

const ecosystemProjection = () => ({
  cwd: "/workspace",
  diagnostics: [],
  errors: [],
  notifications: [],
  observedAt,
  ok: true,
  sections: {
    apps: ecosystemSection("apps", "Apps"),
    hooks: ecosystemSection("hooks", "Hooks"),
    imports: ecosystemSection("imports", "Imports"),
    khala: ecosystemSection("khala", "Khala"),
    marketplace: ecosystemSection("marketplace", "Marketplace"),
    mcp: ecosystemSection("mcp", "MCP"),
    plugins: ecosystemSection("plugins", "Plugins"),
    skills: ecosystemSection("skills", "Skills"),
  },
})

const actionResult = (method: string, response?: unknown) => ({
  method,
  ok: true,
  ...(response === undefined ? {} : { response }),
})

const slashCommandDispatchResult = (
  command: KhalaCodeDesktopSlashCommandWithAvailability | undefined,
  raw: string,
) => {
  if (command === undefined) {
    return { message: `Unknown slash command: ${raw}`, ok: false, status: "not_found" }
  }
  if (!command.availability.available) {
    return {
      command: command.command,
      message: command.availability.reason ?? `/${command.command} unavailable`,
      ok: false,
      status: "unavailable",
    }
  }
  switch (command.dispatch.kind) {
    case "app_server":
      return {
        command: command.command,
        message: `Dispatched /${command.command}`,
        method: command.dispatch.method,
        ok: true,
        response: { fixture: true },
        status: "dispatched",
        threadId,
      }
    case "client":
      return {
        action: command.dispatch.action,
        command: command.command,
        message: `Client action /${command.command}`,
        ok: true,
        status: "client_action",
      }
    case "gap":
      return {
        command: command.command,
        gap: command.dispatch.unavailable ?? {
          gapId: `fixture.${command.command}`,
          kind: "upstream_app_server_gap",
        },
        message: command.dispatch.dependency,
        ok: true,
        status: "gap",
      }
  }
}
