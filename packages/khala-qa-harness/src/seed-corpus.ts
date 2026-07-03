import { KHALA_CODE_HOTBAR_SLOTS } from "../../../clients/khala-code-desktop/src/ui/sidebar.js"
import { KHALA_CODE_CODEX_APPROVAL_ACTIONS } from "../../../clients/khala-code-desktop/src/shared/codex-approval-decisions.js"
import {
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES,
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE,
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_VARIANTS,
  type KhalaCodeCodexThreadItemFixture,
} from "../../../clients/khala-code-desktop/src/bun/codex-thread-item-fixtures.js"
import {
  KHALA_CODE_DESKTOP_SLASH_COMMANDS,
  khalaCodeDesktopSlashCommandsWithAvailability,
  type KhalaCodeDesktopSlashCommand,
} from "../../../clients/khala-code-desktop/src/shared/codex-slash-commands.js"
import {
  evaluateKhalaCodeQaMetricBudgets,
  khalaCodeQaMetricBudgets,
  khalaCodeQaMetricDefinitions,
  type KhalaCodeQaMetricSample,
} from "../../../clients/khala-code-desktop/src/shared/qa-metrics.js"

import {
  KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS,
  type KhalaCodeQaCoverageAvailability,
} from "./coverage-ledger.js"
import {
  KHALA_CODE_QA_CROSS_MODE_SURFACES,
  khalaCodeQaProjectionQuery,
} from "./mode-projection.js"
import {
  distillKhalaCodeQaExploreSessionToRegression,
  type KhalaCodeQaExploreSession,
} from "./explore-distiller.js"
import type { KhalaCodeRpcFetch, KhalaCodeRpcMethodName } from "./rpc-client.js"
import type { KhalaCodeQaScenario } from "./scenario.js"

export type SeedCorpusGroup =
  | "rpc.threads"
  | "rpc.turns"
  | "rpc.approvals"
  | "rpc.settings_config"
  | "rpc.models_personality"
  | "rpc.ecosystem"
  | "rpc.fs_mentions_attachments"
  | "rpc.background_terminals"
  | "rpc.slash_commands"
  | "rpc.token_summaries"
  | "rpc.fleet"
  | "rpc.fleet_run"
  | "rpc.session_catalog"
  | "rpc.forum_panel"
  | "rpc.inbox_routing"
  | "rpc.gym_pane"
  | "rpc.plans_billing"
  | "rpc.headless_events"
  | "rpc.qa_metrics"
  | "hotbar"
  | "cross_mode"
  | "error_states"
  | "distilled_regressions"
  | "thread_items"

type ScenarioGroupEntry = Readonly<{
  group: SeedCorpusGroup
  scenarioIds: readonly string[]
}>

type GroupedScenario = Readonly<{
  group: SeedCorpusGroup
  scenario: KhalaCodeQaScenario
}>

export type KhalaCodeQaSeedCorpusManifest = Readonly<{
  schema: "khala_code_qa_seed_corpus_manifest.v1"
  backend: "fixture"
  coverage: Readonly<{
    approvalDecisionKinds: readonly string[]
    crossModeSurfaces: readonly string[]
    errorStateCases: readonly string[]
    fleetRunControlVerbs: readonly string[]
    hotbarPanels: readonly string[]
    inboxRoutingFlagKinds: readonly string[]
    rpcGroups: readonly string[]
    rpcMethodsByGroup: Readonly<Record<string, readonly string[]>>
    selectors: readonly string[]
    settingsKeys: readonly string[]
    slashCommandAvailabilityStates: Readonly<Record<string, readonly KhalaCodeQaCoverageAvailability[]>>
    slashCommands: readonly string[]
    threadItemFixtureSource: typeof KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE
    threadItemFixtures: readonly {
      readonly fixtureId: string
      readonly rendersVisible: boolean
      readonly variant: string
    }[]
    threadItemVariants: readonly string[]
  }>
  scenarioIdsByGroup: readonly ScenarioGroupEntry[]
  scenarioCount: number
}>

const observedAt = "2026-07-01T00:00:00.000Z"
const desktopSessionId = "desktop-session-fixture"
const threadId = "thread-fixture"
const forkThreadId = "thread-fork-fixture"
const turnId = "turn-fixture"
const runRef = "fleet-run-fixture"

type FixtureFleetRunState = "draft" | "running" | "paused" | "draining" | "completed" | "stopped"
type FixtureAppServerState = "errored" | "running" | "starting" | "stopped"
type FixtureThreadState = {
  readonly archived: boolean
  readonly deleted: boolean
  readonly forked: boolean
  readonly title: string
}

export const KHALA_CODE_QA_ERROR_STATE_CASES = [
  {
    caseId: "codex_binary_missing",
    description: "Codex binary missing before the desktop claims harness readiness",
    targetMethod: "codexHarnessStatus",
  },
  {
    caseId: "auth_expired",
    description: "Codex auth expired or invalid in the primary user Codex home",
    targetMethod: "codexHarnessStatus",
  },
  {
    caseId: "pylon_offline",
    description: "Pylon unavailable while the fleet panel preserves account/session context",
    targetMethod: "codexFleetStatus",
  },
  {
    caseId: "single_rpc_failure_partial_degradation",
    description: "A single FleetRun RPC failure degrades that section without blanking prior data",
    targetMethod: "fleetRunList",
  },
  {
    caseId: "corrupt_session_state_recovery",
    description: "Corrupt session-state recovery keeps a usable catalog entry and diagnostics",
    targetMethod: "sessionCatalog",
  },
  {
    caseId: "mcp_server_down",
    description: "MCP server down renders typed ecosystem diagnostics and retry affordance data",
    targetMethod: "codexEcosystemRead",
  },
  {
    caseId: "network_loss_mid_turn",
    description: "Network loss mid-turn returns a recoverable turn failure without losing messages",
    targetMethod: "codexTurnStart",
  },
  {
    caseId: "interrupt_mid_tool_call",
    description: "Interrupt mid-tool-call returns a typed interrupt result and preserves the thread",
    targetMethod: "codexTurnInterrupt",
  },
  {
    caseId: "app_server_crash_restart",
    description: "App-server crash, restart, and thread resume preserve the active thread",
    targetMethod: "codexAppServerStatus",
  },
] as const satisfies readonly {
  readonly caseId: string
  readonly description: string
  readonly targetMethod: KhalaCodeRpcMethodName
}[]

export type KhalaCodeQaErrorStateCaseId =
  typeof KHALA_CODE_QA_ERROR_STATE_CASES[number]["caseId"]

const groupedFixtureScenario = (
  group: SeedCorpusGroup,
  id: string,
  phases: KhalaCodeQaScenario["phases"],
  commitments: KhalaCodeQaScenario["commitments"],
): GroupedScenario => ({
  group,
  scenario: {
    backend: "fixture",
    commitments,
    id,
    modes: ["rpc"],
    phases,
  },
})

const groupedCrossModeFixtureScenario = (
  id: string,
  phases: KhalaCodeQaScenario["phases"],
  commitments: KhalaCodeQaScenario["commitments"],
): GroupedScenario => ({
  group: "cross_mode",
  scenario: {
    backend: "fixture",
    commitments,
    id,
    modes: ["rpc", "dom"],
    phases,
  },
})

const schema = (query: string) => ({ oracle: "schema" as const, query })
const crash = () => ({ oracle: "crash" as const })
const consistency = (left: string, right: string) => ({
  left,
  oracle: "consistency" as const,
  right,
})
const perf = (metric: string, budget: number) => ({
  budget,
  metric,
  oracle: "perf" as const,
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

const scenarioIdsByGroup = (
  scenarios: readonly GroupedScenario[],
): readonly ScenarioGroupEntry[] =>
  [...scenarios.reduce((groups, entry) => {
    groups.set(entry.group, [...(groups.get(entry.group) ?? []), entry.scenario.id])
    return groups
  }, new Map<SeedCorpusGroup, string[]>()).entries()].map(([group, scenarioIds]) => ({
    group,
    scenarioIds,
  }))

const groupedRpcScenarios: readonly GroupedScenario[] = [
  groupedFixtureScenario(
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
          consistency("rpc:codexThreadList#1", "rpc:codexThreadList#2"),
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
  groupedFixtureScenario(
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
  groupedFixtureScenario(
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
  groupedFixtureScenario(
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
  groupedFixtureScenario(
    "rpc.settings_config",
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
          consistency("rpc:codexSettingsRead#1", "rpc:codexSettingsRead#2"),
          crash(),
        ],
      },
      {
        name: "write-config",
        act: [
          {
            kind: "rpc_call",
            method: "codexConfigValueWrite",
            args: [{ keyPath: "model", value: "gpt-5.1-codex" }],
          },
          { kind: "rpc_call", method: "harnessSettingRead" },
          { kind: "rpc_call", method: "harnessSettingWrite", args: [{ mode: "codex_harness" }] },
        ],
        expect: [
          schema("codexConfigValueWrite"),
          schema("harnessSettingRead"),
          schema("harnessSettingWrite"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_settings.schema", "settings RPC responses decode", "schema"),
      commitment("seed.rpc_settings.consistency", "settings reads are consistent", "consistency"),
      runPass("seed.rpc_settings.pass", "settings lifecycle scenario passes"),
    ],
  ),
  groupedFixtureScenario(
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
  groupedFixtureScenario(
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
          consistency("rpc:slashCommandList#1", "rpc:slashCommandList#2"),
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

export const KHALA_CODE_QA_SEED_FLEET_RUN_CONTROL_VERBS = ["pause", "resume", "drain", "stop"] as const
export const KHALA_CODE_QA_SEED_INBOX_ROUTING_FLAG_KINDS = ["interrupt", "retry", "flag"] as const

const approvalRespondRequest = (
  action: string,
  index: number,
) => ({
  action,
  method: action.startsWith("grantPermissions")
    ? "item/permissions/requestApproval"
    : action === "applyNetworkPolicyAmendment"
      ? "item/commandExecution/requestApproval"
      : "item/commandExecution/requestApproval",
  requestId: `approval-fixture-${index}-${action}`,
  ...(action === "acceptWithExecpolicyAmendment"
    ? { execpolicyAmendment: ["allow fixture command"] }
    : {}),
  ...(action === "applyNetworkPolicyAmendment"
    ? { networkPolicyAmendment: { action: "allow", host: "fixture.local" } }
    : {}),
  ...(action.startsWith("grantPermissions")
    ? {
      permissions: {
        fileSystem: { read: ["/workspace"], write: ["/workspace"] },
        network: { enabled: true },
      },
    }
    : {}),
})

const fixtureImageAttachment = () => ({
  dataBase64: "iVBORw0KGgo=",
  id: "attachment-image-fixture",
  kind: "image",
  mime: "image/png",
  name: "fixture.png",
  sizeBytes: 68,
})

const fixtureChatTurnRequest = (
  options: {
    readonly attachments?: boolean
    readonly startNewThread?: boolean
    readonly turnId?: string
  } = {},
) => ({
  ...(options.attachments === true ? { attachments: [fixtureImageAttachment()] } : {}),
  messages: [{ body: "fixture chat turn", id: "msg-user", role: "user" }],
  sessionId: desktopSessionId,
  ...(options.startNewThread === undefined ? { threadId } : { startNewThread: options.startNewThread }),
  turnId: options.turnId ?? turnId,
})

const qaMetricSample = (
  metric = "startup.interactive_ms",
  value = 1,
): KhalaCodeQaMetricSample => ({
  context: { surface: "seed-corpus" },
  metric: metric as KhalaCodeQaMetricSample["metric"],
  observedAt,
  unit: "ms",
  value,
})

const groupedQ41RpcScenarios: readonly GroupedScenario[] = [
  groupedFixtureScenario(
    "rpc.threads",
    "scenario.khala_code.seed.rpc_threads_q41_completion.v1",
    [
      {
        name: "compact-resume-delete-thread",
        act: [
          { kind: "rpc_call", method: "codexThreadCompact", args: [{ sessionId: desktopSessionId, threadId }] },
          { kind: "rpc_call", method: "codexThreadResume", args: [{ cwd: "/workspace", sessionId: desktopSessionId, threadId }] },
          { kind: "rpc_call", method: "codexThreadDelete", args: [{ threadId }] },
        ],
        expect: [
          schema("codexThreadCompact"),
          schema("codexThreadResume"),
          schema("codexThreadDelete"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_threads.q41_schema", "thread compact, resume, and delete RPCs decode", "schema"),
      runPass("seed.rpc_threads.q41_pass", "Q4.1 thread completion scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.approvals",
    "scenario.khala_code.seed.rpc_approvals_every_decision.v1",
    [
      {
        name: "approval-request-projection",
        act: [
          { kind: "rpc_call", method: "claudeApprovalPending" },
          {
            kind: "rpc_call",
            method: "claudeApprovalRespond",
            args: [{ decision: { behavior: "allow", decisionClassification: "fixture_allow" }, requestId: "claude-approval-fixture" }],
          },
          {
            kind: "rpc_call",
            method: "claudeApprovalRespond",
            args: [{ decision: { behavior: "deny", decisionClassification: "fixture_deny", message: "fixture deny" }, requestId: "claude-approval-fixture" }],
          },
        ],
        expect: [
          schema("claudeApprovalPending"),
          schema("claudeApprovalRespond"),
          crash(),
        ],
      },
      ...KHALA_CODE_CODEX_APPROVAL_ACTIONS.map((action, index) => ({
        name: `codex-approval-${action}`,
        act: [{
          kind: "rpc_call" as const,
          method: "codexApprovalRespond",
          args: [approvalRespondRequest(action, index)],
        }],
        expect: [schema("codexApprovalRespond"), crash()],
      })),
    ],
    [
      commitment("seed.rpc_approvals.every_decision_schema", "every Codex approval decision kind decodes", "schema"),
      runPass("seed.rpc_approvals.every_decision_pass", "approval decision corpus scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.models_personality",
    "scenario.khala_code.seed.rpc_models_personality_lifecycle.v1",
    [
      {
        name: "read-models-and-write-personality",
        act: [
          { kind: "rpc_call", method: "codexSettingsRead", args: [{ cwd: "/workspace", includeHiddenModels: true }] },
          { kind: "rpc_call", method: "codexConfigValueWrite", args: [{ keyPath: "personality", value: "concise" }] },
          { kind: "rpc_call", method: "onDeviceDeciderStatus" },
          { kind: "rpc_call", method: "appleFmReadiness" },
        ],
        expect: [
          schema("codexSettingsRead"),
          schema("codexConfigValueWrite"),
          schema("onDeviceDeciderStatus"),
          schema("appleFmReadiness"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_models_personality.schema", "model, personality, and local decider projections decode", "schema"),
      runPass("seed.rpc_models_personality.pass", "models/personality scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.ecosystem",
    "scenario.khala_code.seed.rpc_ecosystem_full_lifecycle.v1",
    [
      {
        name: "read-ecosystem-and-tool-catalog",
        act: [
          { kind: "rpc_call", method: "codexEcosystemRead", args: [{ cwd: "/workspace", forceReloadSkills: true, forceRefetchApps: true, threadId }] },
          { kind: "rpc_call", method: "toolCatalog" },
        ],
        expect: [schema("codexEcosystemRead"), schema("toolCatalog"), crash()],
      },
      {
        name: "mcp-resource-tool-and-oauth",
        act: [
          { kind: "rpc_call", method: "codexMcpServerReload" },
          { kind: "rpc_call", method: "codexMcpResourceRead", args: [{ server: "fixture", threadId, uri: "resource://fixture/readme" }] },
          {
            kind: "rpc_call",
            method: "codexMcpToolCall",
            args: [{ arguments: { query: "ping" }, meta: { source: "seed-corpus" }, server: "fixture", threadId, tool: "fixtureTool" }],
          },
          { kind: "rpc_call", method: "codexMcpOauthLogin", args: [{ scopes: ["read"], server: "fixture", threadId, timeoutSecs: 1 }] },
        ],
        expect: [
          schema("codexMcpServerReload"),
          schema("codexMcpResourceRead"),
          schema("codexMcpToolCall"),
          schema("codexMcpOauthLogin"),
          crash(),
        ],
      },
      {
        name: "skills-plugins-marketplaces",
        act: [
          { kind: "rpc_call", method: "codexSkillsExtraRootsSet", args: [{ extraRoots: ["/workspace/.khala/skills"] }] },
          { kind: "rpc_call", method: "codexSkillsConfigWrite", args: [{ enabled: true, name: "fixture-skill", path: "/workspace/.khala/skills/fixture" }] },
          { kind: "rpc_call", method: "codexMarketplaceAdd", args: [{ refName: "fixture", source: "https://github.com/OpenAgentsInc/fixture-marketplace", sparsePaths: ["plugins/fixture"] }] },
          { kind: "rpc_call", method: "codexMarketplaceUpgrade", args: [{ marketplaceName: "fixture-marketplace" }] },
          { kind: "rpc_call", method: "codexMarketplaceRemove", args: [{ marketplaceName: "fixture-marketplace" }] },
          { kind: "rpc_call", method: "codexPluginInstall", args: [{ marketplacePath: "plugins/fixture", pluginName: "fixture-plugin", remoteMarketplaceName: "fixture-marketplace" }] },
          { kind: "rpc_call", method: "codexPluginUninstall", args: [{ pluginId: "plugin-fixture" }] },
        ],
        expect: [
          schema("codexSkillsExtraRootsSet"),
          schema("codexSkillsConfigWrite"),
          schema("codexMarketplaceAdd"),
          schema("codexMarketplaceUpgrade"),
          schema("codexMarketplaceRemove"),
          schema("codexPluginInstall"),
          schema("codexPluginUninstall"),
          crash(),
        ],
      },
      {
        name: "external-agent-hooks-imports",
        act: [
          { kind: "rpc_call", method: "codexExternalAgentConfigDetect", args: [{ cwds: ["/workspace"], includeHome: false }] },
          {
            kind: "rpc_call",
            method: "codexExternalAgentConfigImport",
            args: [{ migrationItems: [{ cwd: "/workspace", description: "fixture hook import", details: { hook: true }, itemType: "hook" }], source: "fixture" }],
          },
          { kind: "rpc_call", method: "codexExternalAgentConfigImportHistoriesRead" },
        ],
        expect: [
          schema("codexExternalAgentConfigDetect"),
          schema("codexExternalAgentConfigImport"),
          schema("codexExternalAgentConfigImportHistoriesRead"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_ecosystem.full_schema", "ecosystem MCP, skill, plugin, app, hook, and marketplace RPCs decode", "schema"),
      runPass("seed.rpc_ecosystem.full_pass", "full ecosystem scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.fs_mentions_attachments",
    "scenario.khala_code.seed.rpc_fs_mentions_attachments_lifecycle.v1",
    [
      {
        name: "fs-and-mentions",
        act: [
          { kind: "rpc_call", method: "codexMentionCandidates", args: [{ cwd: "/workspace", query: "README" }] },
          { kind: "rpc_call", method: "codexFsGetMetadata", args: [{ path: "/workspace/README.md" }] },
          { kind: "rpc_call", method: "codexFsReadFile", args: [{ path: "/workspace/README.md" }] },
          { kind: "rpc_call", method: "codexFsWriteFile", args: [{ dataBase64: "Zml4dHVyZQ==", path: "/workspace/tmp/fixture.txt" }] },
        ],
        expect: [
          schema("codexMentionCandidates"),
          schema("codexFsGetMetadata"),
          schema("codexFsReadFile"),
          schema("codexFsWriteFile"),
          crash(),
        ],
      },
      {
        name: "chat-attachment",
        act: [{ kind: "rpc_call", method: "submitChatMessage", args: [fixtureChatTurnRequest({ attachments: true, turnId: "turn-attachment-fixture" })] }],
        expect: [schema("submitChatMessage"), crash()],
      },
    ],
    [
      commitment("seed.rpc_fs_mentions_attachments.schema", "fs, mentions, and attachment RPCs decode", "schema"),
      runPass("seed.rpc_fs_mentions_attachments.pass", "fs/mentions/attachments scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.background_terminals",
    "scenario.khala_code.seed.rpc_background_terminals_lifecycle.v1",
    [
      {
        name: "list-clean-terminate-background-terminals",
        act: [
          { kind: "rpc_call", method: "codexBackgroundTerminalsList", args: [{ cursor: null, limit: 10, threadId }] },
          { kind: "rpc_call", method: "codexBackgroundTerminalsClean", args: [{ threadId }] },
          { kind: "rpc_call", method: "codexBackgroundTerminalsTerminate", args: [{ processId: "process-fixture", threadId }] },
        ],
        expect: [
          schema("codexBackgroundTerminalsList"),
          schema("codexBackgroundTerminalsClean"),
          schema("codexBackgroundTerminalsTerminate"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_background_terminals.schema", "background terminal RPCs decode", "schema"),
      runPass("seed.rpc_background_terminals.pass", "background terminal scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.token_summaries",
    "scenario.khala_code.seed.rpc_token_summaries_lifecycle.v1",
    [
      {
        name: "read-token-status-and-thread-summary",
        act: [
          { kind: "rpc_call", method: "tokenAccountingStatus" },
          { kind: "rpc_call", method: "threadTokenSummary", args: [{ threadId }] },
        ],
        expect: [schema("tokenAccountingStatus"), schema("threadTokenSummary"), crash()],
      },
    ],
    [
      commitment("seed.rpc_token_summaries.schema", "token status and thread summary RPCs decode", "schema"),
      runPass("seed.rpc_token_summaries.pass", "token summaries scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.fleet",
    "scenario.khala_code.seed.rpc_fleet_status_delegate_promote.v1",
    [
      {
        name: "fleet-status-delegate-promote",
        act: [
          { kind: "rpc_call", method: "codexFleetStatus" },
          { kind: "rpc_call", method: "codexFleetDelegateRun", args: [{ count: 1, mode: "fixture", noRun: true, objective: "fixture delegation" }] },
          {
            kind: "rpc_call",
            method: "codexFleetPromoteThread",
            args: [{
              contextBoundary: { allowedRefs: [threadId], includeTranscript: false, mode: "summary_only", summary: "fixture thread summary" },
              count: 1,
              fixture: true,
              noRun: true,
              objective: "fixture promotion",
              sessionId: desktopSessionId,
              threadId,
            }],
          },
        ],
        expect: [schema("codexFleetStatus"), schema("codexFleetDelegateRun"), schema("codexFleetPromoteThread"), crash()],
      },
    ],
    [
      commitment("seed.rpc_fleet.promote_schema", "fleet status, delegate, and promote RPCs decode", "schema"),
      runPass("seed.rpc_fleet.promote_pass", "fleet status/delegate/promote scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.fleet_run",
    "scenario.khala_code.seed.rpc_fleet_run_lifecycle.v1",
    [
      {
        name: "start-status-list-control-fleet-run",
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
          ...KHALA_CODE_QA_SEED_FLEET_RUN_CONTROL_VERBS.map((verb) => ({
            kind: "rpc_call" as const,
            method: "fleetRunControl",
            args: [{ runRef, verb }],
          })),
        ],
        expect: [
          schema("fleetRunStart"),
          schema("fleetRunStatus"),
          schema("fleetRunList"),
          schema("fleetRunControl"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_fleet_run.schema", "FleetRun start/status/list/control RPCs decode", "schema"),
      runPass("seed.rpc_fleet_run.pass", "FleetRun lifecycle scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.session_catalog",
    "scenario.khala_code.seed.rpc_session_catalog_lifecycle.v1",
    [
      {
        name: "search-session-catalog",
        act: [
          { kind: "rpc_call", method: "sessionCatalog", args: [{ limit: 10, searchTerm: "fixture" }] },
          { kind: "rpc_call", method: "sessionCatalog", args: [{ limit: 10, searchTerm: "fixture" }] },
        ],
        expect: [
          schema("sessionCatalog"),
          consistency("rpc:sessionCatalog#1", "rpc:sessionCatalog#2"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_session_catalog.schema", "session catalog RPC responses decode", "schema"),
      commitment("seed.rpc_session_catalog.consistency", "session catalog search is stable", "consistency"),
      runPass("seed.rpc_session_catalog.pass", "session catalog scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.forum_panel",
    "scenario.khala_code.seed.rpc_forum_panel_lifecycle.v1",
    [
      {
        name: "browse-post-tip-forum",
        act: [
          { kind: "rpc_call", method: "forumRequest", args: [{ method: "GET", path: "/forum" }] },
          { kind: "rpc_call", method: "forumRequest", args: [{ body: { body: "fixture post", title: "Fixture" }, method: "POST", path: "/forum/posts" }] },
          { kind: "rpc_call", method: "forumRequest", args: [{ body: { amountSats: 1 }, method: "POST", path: "/forum/posts/post-fixture/tips" }] },
        ],
        expect: [schema("forumRequest"), crash()],
      },
    ],
    [
      commitment("seed.rpc_forum_panel.schema", "forum browse, post, and tip proxy RPCs decode", "schema"),
      runPass("seed.rpc_forum_panel.pass", "forum panel scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.inbox_routing",
    "scenario.khala_code.seed.rpc_inbox_routing_lifecycle.v1",
    [
      {
        name: "read-inbox-source-rpcs",
        act: [
          { kind: "rpc_call", method: "codexFleetStatus" },
          { kind: "rpc_call", method: "pylonStatus" },
          { kind: "rpc_call", method: "codingStatus" },
          { kind: "rpc_call", method: "tokenAccountingStatus" },
          { kind: "rpc_call", method: "codexHarnessStatus" },
          { kind: "rpc_call", method: "codexEcosystemRead", args: [{ cwd: "/workspace", threadId }] },
        ],
        expect: [
          schema("codexFleetStatus"),
          schema("pylonStatus"),
          schema("codingStatus"),
          schema("tokenAccountingStatus"),
          schema("codexHarnessStatus"),
          schema("codexEcosystemRead"),
          crash(),
        ],
      },
      {
        name: "route-worker-control-flags",
        act: KHALA_CODE_QA_SEED_INBOX_ROUTING_FLAG_KINDS.map((verb) => ({
          kind: "rpc_call" as const,
          method: "fleetWorkerControl",
          args: [{
            assignmentRef: "assignment-fixture",
            issueRef: "#8027",
            note: `fixture ${verb}`,
            runRef,
            verb,
            workerRefHash: "worker-fixture",
          }],
        })),
        expect: [schema("fleetWorkerControl"), crash()],
      },
    ],
    [
      commitment("seed.rpc_inbox_routing.schema", "inbox source and worker-control routing RPCs decode", "schema"),
      runPass("seed.rpc_inbox_routing.pass", "inbox routing scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.gym_pane",
    "scenario.khala_code.seed.rpc_gym_pane_lifecycle.v1",
    [
      {
        name: "read-gym-source-projections",
        act: [
          { kind: "rpc_call", method: "fleetRunStatus", args: [{ runRef }] },
          { kind: "rpc_call", method: "fleetRunList", args: [{}] },
          { kind: "rpc_call", method: "codexFleetStatus" },
        ],
        expect: [schema("fleetRunStatus"), schema("fleetRunList"), schema("codexFleetStatus"), crash()],
      },
    ],
    [
      commitment("seed.rpc_gym_pane.schema", "Gym source projections decode at the RPC boundary", "schema"),
      runPass("seed.rpc_gym_pane.pass", "Gym pane source scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.plans_billing",
    "scenario.khala_code.seed.rpc_plans_billing_lifecycle.v1",
    [
      {
        name: "read-status-and-purchase-plan",
        act: [
          { kind: "rpc_call", method: "khalaCodePlanCatalog" },
          { kind: "rpc_call", method: "khalaCodePlanStatus" },
          { kind: "rpc_call", method: "khalaCodePlanPurchase", args: [{ idempotencyKey: "fixture-purchase" }] },
        ],
        expect: [schema("khalaCodePlanCatalog"), schema("khalaCodePlanStatus"), schema("khalaCodePlanPurchase"), crash()],
      },
    ],
    [
      commitment("seed.rpc_plans_billing.schema", "plan catalog, status, and purchase RPCs decode", "schema"),
      runPass("seed.rpc_plans_billing.pass", "plans/billing scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.headless_events",
    "scenario.khala_code.seed.rpc_headless_events_lifecycle.v1",
    [
      {
        name: "headless-chat-event-source",
        act: [{ kind: "rpc_call", method: "submitChatMessage", args: [fixtureChatTurnRequest({ startNewThread: false, turnId: "turn-headless-fixture" })] }],
        expect: [schema("submitChatMessage"), crash()],
      },
      {
        name: "headless-fleet-event-source",
        act: [{ kind: "rpc_call", method: "fleetRunStatus", args: [{ runRef }] }],
        expect: [schema("fleetRunStatus"), crash()],
      },
    ],
    [
      commitment("seed.rpc_headless_events.schema", "headless chat and fleet event source RPCs decode", "schema"),
      runPass("seed.rpc_headless_events.pass", "headless events source scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "rpc.qa_metrics",
    "scenario.khala_code.seed.rpc_qa_metrics_lifecycle.v1",
    [
      {
        name: "sample-and-read-qa-metrics",
        act: [
          { kind: "rpc_call", method: "qaMetricSample", args: [qaMetricSample()] },
          { kind: "rpc_call", method: "qaMetrics" },
        ],
        expect: [
          schema("qaMetricSample"),
          schema("qaMetrics"),
          perf("startup.interactive_ms", 2000),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.rpc_qa_metrics.schema", "QA metric sample and snapshot RPCs decode", "schema"),
      commitment("seed.rpc_qa_metrics.perf", "fixture QA metrics satisfy a budget oracle", "perf"),
      runPass("seed.rpc_qa_metrics.pass", "QA metrics scenario passes"),
    ],
  ),
]

const groupedHotbarScenarios: readonly GroupedScenario[] = [
  groupedFixtureScenario(
    "hotbar",
    "scenario.khala_code.seed.hotbar_chat_panel.v1",
    [{
      name: "chat-panel-rpc",
      act: [{ kind: "hotbar", target: "chat" }, { kind: "rpc_call", method: "codingStatus" }, { kind: "rpc_call", method: "appInfo" }],
      expect: [schema("codingStatus"), schema("appInfo"), crash()],
    }],
    [
      commitment("seed.hotbar.chat.schema", "chat hotbar panel backing RPC decodes", "schema"),
      runPass("seed.hotbar.chat.pass", "chat hotbar scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "hotbar",
    "scenario.khala_code.seed.hotbar_fleet_panel.v1",
    [{
      name: "fleet-panel-rpc",
      act: [{ kind: "hotbar", target: "fleet" }, { kind: "rpc_call", method: "codexFleetStatus" }, { kind: "rpc_call", method: "fleetRunList", args: [{}] }],
      expect: [schema("codexFleetStatus"), schema("fleetRunList"), crash()],
    }],
    [
      commitment("seed.hotbar.fleet.schema", "fleet hotbar panel backing RPC decodes", "schema"),
      runPass("seed.hotbar.fleet.pass", "fleet hotbar scenario passes"),
    ],
  ),
  groupedFixtureScenario(
    "hotbar",
    "scenario.khala_code.seed.hotbar_settings_panel.v1",
    [{
      name: "settings-panel-rpc",
      act: [{ kind: "hotbar", target: "settings" }, { kind: "rpc_call", method: "codexSettingsRead", args: [{}] }, { kind: "rpc_call", method: "codexEcosystemRead", args: [{}] }],
      expect: [schema("codexSettingsRead"), schema("codexEcosystemRead"), crash()],
    }],
    [
      commitment("seed.hotbar.settings.schema", "settings hotbar panel backing RPC decodes", "schema"),
      runPass("seed.hotbar.settings.pass", "settings hotbar scenario passes"),
    ],
  ),
]

export const KHALA_CODE_QA_THREAD_ITEM_VARIANTS = KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_VARIANTS
export const KHALA_CODE_QA_THREAD_ITEM_FIXTURE_SOURCE = KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE
export const KHALA_CODE_QA_THREAD_ITEM_FIXTURES =
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES.map((fixture) => ({
    fixtureId: String(fixture.item.id ?? fixture.variant),
    rendersVisible: fixture.rendersVisible,
    variant: fixture.variant,
  }))
export const KHALA_CODE_QA_ERROR_STATE_CASE_IDS =
  KHALA_CODE_QA_ERROR_STATE_CASES.map((entry) => entry.caseId)
export const KHALA_CODE_QA_SEED_HOTBAR_PANELS = KHALA_CODE_HOTBAR_SLOTS.map((slot) => slot.value)
export const KHALA_CODE_QA_SEED_SETTINGS_KEYS = ["model", "personality"] as const
export const KHALA_CODE_QA_SEED_APPROVAL_DECISION_KINDS = KHALA_CODE_CODEX_APPROVAL_ACTIONS
export const KHALA_CODE_QA_SEED_SELECTORS = [] as const
export const KHALA_CODE_QA_SEED_SLASH_COMMANDS = KHALA_CODE_DESKTOP_SLASH_COMMANDS.map((command) => command.command)
export const KHALA_CODE_QA_SEED_CROSS_MODE_SURFACES = KHALA_CODE_QA_CROSS_MODE_SURFACES

export const KHALA_CODE_QA_SEED_SLASH_COMMAND_AVAILABILITY_STATES =
  Object.fromEntries(KHALA_CODE_DESKTOP_SLASH_COMMANDS.map((command) => [
    command.command,
    slashCommandExpectedAvailabilityStates(command),
  ])) as Readonly<Record<string, readonly KhalaCodeQaCoverageAvailability[]>>

const groupedThreadItemScenarios: readonly GroupedScenario[] =
  KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES.map((fixture) =>
    groupedFixtureScenario(
      "thread_items",
      `scenario.khala_code.seed.thread_item_${fixture.variant.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)}.v1`,
      [{
        name: "read-thread-item",
        act: [{ kind: "rpc_call", method: "codexThreadRead", args: [{ threadId: threadItemThreadId(fixture.variant), includeTurns: true }] }],
        expect: [schema("codexThreadRead"), crash()],
      }],
      [
        commitment(`seed.thread_item.${fixture.variant}.schema`, `${fixture.variant} ThreadItem pinned fixture replay decodes`, "schema"),
        runPass(`seed.thread_item.${fixture.variant}.pass`, `${fixture.variant} ThreadItem pinned fixture scenario passes`),
      ],
    )
  )

const errorStateArmAction = (caseId: KhalaCodeQaErrorStateCaseId) => ({
  kind: "rpc_call" as const,
  method: "qaMetricSample",
  args: [{
    ...qaMetricSample("startup.interactive_ms", 0),
    context: {
      errorStateCase: caseId,
      surface: "seed-corpus",
    },
  }],
})

const degradedState = (caseId: KhalaCodeQaErrorStateCaseId) => ({
  id: "typed-degraded-state",
  match: caseId,
  oracle: "invariant" as const,
})

const noConsoleErrors = () => ({
  id: "no-console-errors",
  oracle: "invariant" as const,
})

const noDataLoss = (caseId: KhalaCodeQaErrorStateCaseId) => ({
  id: "no-data-loss",
  match: caseId,
  oracle: "invariant" as const,
})

const errorStateExpectations = (
  caseId: KhalaCodeQaErrorStateCaseId,
  method: KhalaCodeRpcMethodName,
): KhalaCodeQaScenario["phases"][number]["expect"] => [
  schema(method),
  degradedState(caseId),
  noConsoleErrors(),
  noDataLoss(caseId),
  crash(),
]

const errorStateAction = (
  caseId: KhalaCodeQaErrorStateCaseId,
  method: KhalaCodeRpcMethodName,
): KhalaCodeQaScenario["phases"][number]["act"][number] => {
  switch (method) {
    case "codexFleetStatus":
    case "codexHarnessStatus":
    case "codexAppServerStatus":
      return { kind: "rpc_call", method }
    case "fleetRunList":
      return { kind: "rpc_call", method, args: [{}] }
    case "sessionCatalog":
      return { kind: "rpc_call", method, args: [{ limit: 10, searchTerm: "fixture" }] }
    case "codexEcosystemRead":
      return { kind: "rpc_call", method, args: [{ cwd: "/workspace", threadId }] }
    case "codexTurnStart":
      return {
        kind: "rpc_call",
        method,
        args: [fixtureChatTurnRequest({ turnId: `turn-error-${caseId}` })],
      }
    case "codexTurnInterrupt":
      return {
        kind: "rpc_call",
        method,
        args: [{ sessionId: desktopSessionId, turnId: `turn-error-${caseId}` }],
      }
    default:
      return { kind: "rpc_call", method }
  }
}

const errorStateScenarioPhases = (
  entry: typeof KHALA_CODE_QA_ERROR_STATE_CASES[number],
): KhalaCodeQaScenario["phases"] => {
  if (entry.caseId === "app_server_crash_restart") {
    return [
      {
        name: "observe-app-server-crash",
        act: [
          errorStateArmAction(entry.caseId),
          { kind: "rpc_call", method: "codexAppServerStatus" },
        ],
        expect: errorStateExpectations(entry.caseId, "codexAppServerStatus"),
      },
      {
        name: "restart-and-resume-thread",
        act: [
          errorStateArmAction(entry.caseId),
          { kind: "rpc_call", method: "codexAppServerRestart" },
          { kind: "rpc_call", method: "codexThreadResume", args: [{ cwd: "/workspace", sessionId: desktopSessionId, threadId }] },
        ],
        expect: [
          schema("codexAppServerRestart"),
          schema("codexThreadResume"),
          degradedState(entry.caseId),
          noConsoleErrors(),
          noDataLoss(entry.caseId),
          crash(),
        ],
      },
    ]
  }
  return [{
    name: `exercise-${entry.caseId.replace(/_/g, "-")}`,
    act: [
      errorStateArmAction(entry.caseId),
      errorStateAction(entry.caseId, entry.targetMethod),
    ],
    expect: errorStateExpectations(entry.caseId, entry.targetMethod),
  }]
}

const groupedErrorStateScenarios: readonly GroupedScenario[] =
  KHALA_CODE_QA_ERROR_STATE_CASES.map((entry) =>
    groupedFixtureScenario(
      "error_states",
      `scenario.khala_code.seed.error_state_${entry.caseId}.v1`,
      errorStateScenarioPhases(entry),
      [
        commitment(`seed.error_state.${entry.caseId}.schema`, `${entry.description} decodes at the RPC boundary`, "schema"),
        commitment(`seed.error_state.${entry.caseId}.degraded`, `${entry.description} has a typed degraded state`, "invariant"),
        commitment(`seed.error_state.${entry.caseId}.console`, `${entry.description} emits no console errors`, "invariant"),
        commitment(`seed.error_state.${entry.caseId}.data`, `${entry.description} preserves data`, "invariant"),
        runPass(`seed.error_state.${entry.caseId}.pass`, `${entry.description} scenario passes`),
      ],
    )
  )

const groupedSlashCommandScenarios: readonly GroupedScenario[] =
  KHALA_CODE_DESKTOP_SLASH_COMMANDS.map((command) =>
    groupedFixtureScenario(
      "rpc.slash_commands",
      `scenario.khala_code.seed.slash_command_${command.command.replace(/[^a-z0-9]+/g, "_")}.v1`,
      slashCommandScenarioPhases(command),
      [
        commitment(`seed.slash_command.${command.command}.schema`, `/${command.command} dispatch response decodes`, "schema"),
        commitment(`seed.slash_command.${command.command}.availability`, `/${command.command} availability states are counted`, "schema"),
        runPass(`seed.slash_command.${command.command}.pass`, `/${command.command} scenario passes`),
      ],
    )
  )

const projectionRead = (
  surface: typeof KHALA_CODE_QA_CROSS_MODE_SURFACES[number],
) => ({
  kind: "read" as const,
  query: khalaCodeQaProjectionQuery(surface),
})

const crossModeConsistency = (
  surface: typeof KHALA_CODE_QA_CROSS_MODE_SURFACES[number],
) =>
  consistency(
    `rpc:${khalaCodeQaProjectionQuery(surface)}`,
    `dom:${khalaCodeQaProjectionQuery(surface)}`,
  )

const groupedCrossModeScenarios: readonly GroupedScenario[] = [
  groupedCrossModeFixtureScenario(
    "scenario.khala_code.seed.cross_mode_consistency.v1",
    [
      {
        name: "thread-list-cross-mode",
        act: [
          { kind: "rpc_call", method: "codexThreadList", args: [{ sessionId: desktopSessionId }] },
          projectionRead("thread_list"),
        ],
        expect: [
          schema("codexThreadList"),
          crossModeConsistency("thread_list"),
          crash(),
        ],
      },
      {
        name: "fleet-counts-cross-mode",
        act: [
          { kind: "rpc_call", method: "codexFleetStatus" },
          projectionRead("fleet_counts"),
        ],
        expect: [
          schema("codexFleetStatus"),
          crossModeConsistency("fleet_counts"),
          crash(),
        ],
      },
      {
        name: "gym-state-cross-mode",
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
          { kind: "rpc_call", method: "codexFleetStatus" },
          projectionRead("gym_state"),
        ],
        expect: [
          schema("fleetRunStatus"),
          schema("fleetRunList"),
          schema("codexFleetStatus"),
          crossModeConsistency("gym_state"),
          crash(),
        ],
      },
      {
        name: "runtime-badges-cross-mode",
        act: [
          { kind: "rpc_call", method: "codingStatus" },
          { kind: "rpc_call", method: "pylonStatus" },
          { kind: "rpc_call", method: "codexHarnessStatus" },
          { kind: "rpc_call", method: "tokenAccountingStatus" },
          projectionRead("runtime_badges"),
        ],
        expect: [
          schema("codingStatus"),
          schema("pylonStatus"),
          schema("codexHarnessStatus"),
          schema("tokenAccountingStatus"),
          crossModeConsistency("runtime_badges"),
          crash(),
        ],
      },
    ],
    [
      commitment("seed.cross_mode.thread_list", "Mode P and Mode D thread-list projections agree", "consistency"),
      commitment("seed.cross_mode.fleet_counts", "Mode P and Mode D fleet-count projections agree", "consistency"),
      commitment("seed.cross_mode.gym_state", "Mode P and Mode D gym-state projections agree", "consistency"),
      commitment("seed.cross_mode.runtime_badges", "Mode P and Mode D runtime badges agree", "consistency"),
      runPass("seed.cross_mode.pass", "cross-mode consistency scenario passes"),
    ],
  ),
]

const firstDistilledExploreSession: KhalaCodeQaExploreSession = {
  actionLog: [
    {
      action: { kind: "hotbar", target: "fleet" },
      index: 0,
      rationale: "explorer opened the fleet panel from the coverage frontier",
    },
    {
      action: { kind: "rpc_call", method: "codexFleetStatus" },
      index: 1,
      rationale: "explorer confirmed fleet status can be fetched",
    },
    {
      action: { kind: "read", query: "projection:fleet_counts" },
      index: 2,
      rationale: "explorer captured the rendered fleet-count projection",
    },
  ],
  backend: "fixture",
  explorer: "llm",
  mode: "rpc",
  oracleExpectations: [
    schema("codexFleetStatus"),
    crash(),
  ],
  runId: "q6_2_first_fleet_panel_distilled_regression",
  schema: "khala_code_qa_explore_session.v1",
  status: "pass",
}

export const KHALA_CODE_QA_FIRST_DISTILLED_REGRESSION =
  distillKhalaCodeQaExploreSessionToRegression(firstDistilledExploreSession)

const requireDistilledScenario = (
  result: typeof KHALA_CODE_QA_FIRST_DISTILLED_REGRESSION,
): KhalaCodeQaScenario => {
  if (result.verdict !== "CONFIRMED") {
    throw new Error(`First Q6.2 distilled regression is not committable: ${result.reason}`)
  }
  return result.distilled.scenario
}

const groupedDistilledRegressionScenarios: readonly GroupedScenario[] = [
  {
    group: "distilled_regressions",
    scenario: requireDistilledScenario(KHALA_CODE_QA_FIRST_DISTILLED_REGRESSION),
  },
]

const groupedSeedScenarios: readonly GroupedScenario[] = [
  ...groupedRpcScenarios,
  ...groupedQ41RpcScenarios,
  ...groupedHotbarScenarios,
  ...groupedCrossModeScenarios,
  ...groupedThreadItemScenarios,
  ...groupedErrorStateScenarios,
  ...groupedSlashCommandScenarios,
  ...groupedDistilledRegressionScenarios,
]

export const KHALA_CODE_QA_SEED_SCENARIOS: readonly KhalaCodeQaScenario[] =
  groupedSeedScenarios.map((entry) => entry.scenario)

export const KHALA_CODE_QA_SEED_CORPUS_MANIFEST: KhalaCodeQaSeedCorpusManifest = {
  schema: "khala_code_qa_seed_corpus_manifest.v1",
  backend: "fixture",
  coverage: {
    approvalDecisionKinds: KHALA_CODE_QA_SEED_APPROVAL_DECISION_KINDS,
    crossModeSurfaces: KHALA_CODE_QA_SEED_CROSS_MODE_SURFACES,
    errorStateCases: KHALA_CODE_QA_ERROR_STATE_CASE_IDS,
    fleetRunControlVerbs: KHALA_CODE_QA_SEED_FLEET_RUN_CONTROL_VERBS,
    hotbarPanels: KHALA_CODE_QA_SEED_HOTBAR_PANELS,
    inboxRoutingFlagKinds: KHALA_CODE_QA_SEED_INBOX_ROUTING_FLAG_KINDS,
    rpcGroups: Object.keys(KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS),
    rpcMethodsByGroup: KHALA_CODE_QA_ROADMAP_RPC_METHOD_GROUPS,
    selectors: KHALA_CODE_QA_SEED_SELECTORS,
    settingsKeys: KHALA_CODE_QA_SEED_SETTINGS_KEYS,
    slashCommandAvailabilityStates: KHALA_CODE_QA_SEED_SLASH_COMMAND_AVAILABILITY_STATES,
    slashCommands: KHALA_CODE_QA_SEED_SLASH_COMMANDS,
    threadItemFixtureSource: KHALA_CODE_QA_THREAD_ITEM_FIXTURE_SOURCE,
    threadItemFixtures: KHALA_CODE_QA_THREAD_ITEM_FIXTURES,
    threadItemVariants: KHALA_CODE_QA_THREAD_ITEM_VARIANTS,
  },
  scenarioIdsByGroup: scenarioIdsByGroup(groupedSeedScenarios),
  scenarioCount: KHALA_CODE_QA_SEED_SCENARIOS.length,
}

function slashCommandDispatchRequest(
  command: KhalaCodeDesktopSlashCommand,
  options: {
    readonly activeTurn?: boolean
    readonly includeArgs?: boolean
    readonly includeThread?: boolean
    readonly sideConversation?: boolean
  } = {},
): {
  readonly activeTurn: boolean
  readonly debug: boolean
  readonly platform: string
  readonly raw: string
  readonly sessionId: string
  readonly sideConversation: boolean
  readonly threadId?: string
} {
  const includeThread = options.includeThread ?? true
  return {
    activeTurn: options.activeTurn ?? false,
    debug: true,
    platform: slashCommandVisiblePlatform(command),
    raw: slashCommandRaw(command, { includeArgs: options.includeArgs ?? true }),
    sessionId: desktopSessionId,
    sideConversation: options.sideConversation ?? false,
    ...(includeThread && command.dispatch.kind === "app_server" && command.dispatch.requiresThread === true ? { threadId } : {}),
  }
}

function slashCommandRaw(
  command: KhalaCodeDesktopSlashCommand,
  options: { readonly includeArgs?: boolean } = {},
): string {
  const includeArgs = options.includeArgs ?? true
  return includeArgs && (command.supportsInlineArgs || command.dispatch.kind === "app_server" && command.dispatch.requiresArgs === true)
    ? `/${command.command} fixture`
    : `/${command.command}`
}

function slashCommandListRequest(
  command: KhalaCodeDesktopSlashCommand,
  options: {
    readonly activeTurn?: boolean
    readonly sideConversation?: boolean
  } = {},
): {
  readonly activeTurn?: boolean
  readonly debug: boolean
  readonly platform: string
  readonly sideConversation?: boolean
} {
  return {
    ...(options.activeTurn === undefined ? {} : { activeTurn: options.activeTurn }),
    debug: true,
    platform: slashCommandVisiblePlatform(command),
    ...(options.sideConversation === undefined ? {} : { sideConversation: options.sideConversation }),
  }
}

function slashCommandVisiblePlatform(
  command: KhalaCodeDesktopSlashCommand,
): string {
  return command.visibility.kind === "platform"
    ? command.visibility.platforms[0] ?? "darwin"
    : "darwin"
}

function sortedAvailabilityStates(
  states: Iterable<KhalaCodeQaCoverageAvailability>,
): readonly KhalaCodeQaCoverageAvailability[] {
  return [...new Set(states)].sort() as readonly KhalaCodeQaCoverageAvailability[]
}

function threadItemThreadId(variant: string): string {
  return `thread-item-${variant}`
}

function threadItemFixtureForThreadId(
  id: string,
): KhalaCodeCodexThreadItemFixture | undefined {
  if (!id.startsWith("thread-item-")) return undefined
  const variant = id.slice("thread-item-".length)
  return KHALA_CODE_CODEX_THREAD_ITEM_FIXTURES.find((fixture) => fixture.variant === variant)
}

function slashCommandExpectedAvailabilityStates(
  command: KhalaCodeDesktopSlashCommand,
): readonly KhalaCodeQaCoverageAvailability[] {
  const states = new Set<KhalaCodeQaCoverageAvailability>()
  if (command.debug === true || command.visibility.kind === "debug") states.add("debug")
  if (command.availableDuringTask === true) states.add("task")
  if (command.availableInSideConversation === true) states.add("side_conversation")
  if (command.availableDuringTask === false || command.availableInSideConversation === false) states.add("unavailable")
  if (command.dispatch.kind === "app_server") {
    if (command.dispatch.requiresArgs === true) {
      states.add("args")
      states.add("unavailable")
    }
    if (command.dispatch.requiresThread === true) {
      states.add("thread")
      states.add("unavailable")
    }
  }
  if (command.dispatch.kind === "gap") {
    states.add("gap")
    if (command.dispatch.unavailable !== undefined) states.add("unavailable")
  }
  return states.size === 0 ? ["always"] : sortedAvailabilityStates(states)
}

function slashCommandScenarioPhases(
  command: KhalaCodeDesktopSlashCommand,
): KhalaCodeQaScenario["phases"] {
  const disabledDispatches: KhalaCodeQaScenario["phases"][number][] = []
  if (!command.availableDuringTask) {
    disabledDispatches.push({
      name: "dispatch-while-task-active",
      act: [{
        kind: "rpc_call",
        method: "slashCommandDispatch",
        args: [slashCommandDispatchRequest(command, { activeTurn: true })],
      }],
      expect: [schema("slashCommandDispatch"), crash()],
    })
  }
  if (!command.availableInSideConversation) {
    disabledDispatches.push({
      name: "dispatch-from-side-conversation",
      act: [{
        kind: "rpc_call",
        method: "slashCommandDispatch",
        args: [slashCommandDispatchRequest(command, { sideConversation: true })],
      }],
      expect: [schema("slashCommandDispatch"), crash()],
    })
  }
  if (command.dispatch.kind === "app_server" && command.dispatch.requiresArgs === true) {
    disabledDispatches.push({
      name: "dispatch-without-required-args",
      act: [{
        kind: "rpc_call",
        method: "slashCommandDispatch",
        args: [slashCommandDispatchRequest(command, { includeArgs: false })],
      }],
      expect: [schema("slashCommandDispatch"), crash()],
    })
  }
  if (command.dispatch.kind === "app_server" && command.dispatch.requiresThread === true) {
    disabledDispatches.push({
      name: "dispatch-without-required-thread",
      act: [{
        kind: "rpc_call",
        method: "slashCommandDispatch",
        args: [slashCommandDispatchRequest(command, { includeThread: false })],
      }],
      expect: [schema("slashCommandDispatch"), crash()],
    })
  }
  return [
    {
      name: "list-slash-command-availability",
      act: [
        { kind: "rpc_call", method: "slashCommandList", args: [slashCommandListRequest(command)] },
        { kind: "rpc_call", method: "slashCommandList", args: [slashCommandListRequest(command, { activeTurn: true })] },
        { kind: "rpc_call", method: "slashCommandList", args: [slashCommandListRequest(command, { sideConversation: true })] },
      ],
      expect: [
        schema("slashCommandList"),
        crash(),
      ],
    },
    {
      name: "dispatch-slash-command",
      act: [{
        kind: "rpc_call",
        method: "slashCommandDispatch",
        args: [slashCommandDispatchRequest(command)],
      }],
      expect: [schema("slashCommandDispatch"), crash()],
    },
    ...disabledDispatches,
  ]
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

const errorStateDataPreservedRef = (caseId: KhalaCodeQaErrorStateCaseId): string =>
  `qa.error_state.${caseId}.data_preserved`

const errorStateMarker = (
  caseId: KhalaCodeQaErrorStateCaseId,
  state: "degraded" | "recovering" | "recovered" = "degraded",
) => ({
  caseId,
  dataLoss: false,
  kind: "khala_code_qa_error_state",
  preservesData: true,
  recoverable: true,
  ref: errorStateDataPreservedRef(caseId),
  state,
})

const errorStateCaseFromMetricSample = (
  sample: KhalaCodeQaMetricSample,
): KhalaCodeQaErrorStateCaseId | null => {
  const caseId = sample.context?.errorStateCase
  return typeof caseId === "string" && (KHALA_CODE_QA_ERROR_STATE_CASE_IDS as readonly string[]).includes(caseId)
    ? caseId as KhalaCodeQaErrorStateCaseId
    : null
}

export const makeKhalaCodeQaSeedCorpusFixtureFetch = (): KhalaCodeRpcFetch => {
  let activeErrorStateCase: KhalaCodeQaErrorStateCaseId | null = null
  let fleetRunState: FixtureFleetRunState = "draft"
  let appServerState: FixtureAppServerState = "stopped"
  const qaMetricSamples: KhalaCodeQaMetricSample[] = []
  let threadState: FixtureThreadState = {
    archived: false,
    deleted: false,
    forked: false,
    title: "Fixture thread",
  }
  return async (input, init) => {
    const method = parseMethod(input)
    const args = await parseArgs(init)
    return response(fixtureRpcPayload(method, args, {
      activeErrorStateCase,
      appServerState,
      fleetRunState,
      qaMetricSamples,
      appendQaMetricSample: (sample) => {
        qaMetricSamples.push(sample)
      },
      setActiveErrorStateCase: (caseId) => {
        activeErrorStateCase = caseId
      },
      setAppServerState: (state) => {
        appServerState = state
      },
      setFleetRunState: (state) => {
        fleetRunState = state
      },
      setThreadState: (patch) => {
        threadState = { ...threadState, ...patch }
      },
      threadState,
    }))
  }
}

const fixtureRpcPayload = (
  method: string,
  args: readonly unknown[],
  state: {
    readonly activeErrorStateCase: KhalaCodeQaErrorStateCaseId | null
    readonly appServerState: FixtureAppServerState
    readonly fleetRunState: FixtureFleetRunState
    readonly qaMetricSamples: readonly KhalaCodeQaMetricSample[]
    readonly appendQaMetricSample: (sample: KhalaCodeQaMetricSample) => void
    readonly setActiveErrorStateCase: (caseId: KhalaCodeQaErrorStateCaseId | null) => void
    readonly setAppServerState: (state: FixtureAppServerState) => void
    readonly setFleetRunState: (state: FixtureFleetRunState) => void
    readonly setThreadState: (patch: Partial<FixtureThreadState>) => void
    readonly threadState: FixtureThreadState
  },
): unknown => {
  switch (method) {
    case "appInfo":
      return { app: "Khala Code Desktop", ok: true, observedAt }
    case "appleFmReadiness":
      return appleFmReadiness()
    case "codexHarnessStatus":
      return codexHarnessStatus(state.activeErrorStateCase)
    case "codexAppServerStatus":
      return appServerStatus(
        state.activeErrorStateCase === "app_server_crash_restart" ? "errored" : state.appServerState,
        state.activeErrorStateCase === "app_server_crash_restart" ? state.activeErrorStateCase : null,
      )
    case "codexAppServerStart":
      state.setAppServerState("running")
      return appServerControl("start", state.appServerState, "running")
    case "codexAppServerRestart":
      state.setAppServerState("running")
      return appServerControl(
        "restart",
        state.activeErrorStateCase === "app_server_crash_restart" ? "errored" : state.appServerState,
        "running",
        state.activeErrorStateCase === "app_server_crash_restart" ? state.activeErrorStateCase : null,
      )
    case "codexAppServerStop":
      state.setAppServerState("stopped")
      return appServerControl("stop", state.appServerState, "stopped")
    case "codingStatus":
      return runtimeStatus("coding", "ready")
    case "pylonStatus":
      return runtimeStatus(
        "pylon",
        state.activeErrorStateCase === "pylon_offline" ? "unavailable" : "ready",
        state.activeErrorStateCase === "pylon_offline"
          ? errorStateDataPreservedRef(state.activeErrorStateCase)
          : undefined,
      )
    case "tokenAccountingStatus":
      return runtimeStatus("token_accounting", "ready")
    case "onDeviceDeciderStatus":
      return onDeviceDeciderStatus()
    case "codexThreadStart":
      state.setThreadState({
        archived: false,
        deleted: false,
        title: "Fixture thread",
      })
      return threadResult(threadId, state.threadState)
    case "codexThreadList":
      return threadListResult(args[0], state.threadState)
    case "codexThreadRead": {
      const request = args[0] as { readonly threadId?: string } | undefined
      return threadResult(request?.threadId ?? threadId, state.threadState)
    }
    case "codexThreadRename": {
      const request = args[0] as { readonly name?: string } | undefined
      state.setThreadState({ title: request?.name ?? "Renamed fixture" })
      return threadMutation("rename", threadId)
    }
    case "codexThreadArchive":
      state.setThreadState({ archived: true })
      return threadMutation("archive", threadId)
    case "codexThreadUnarchive":
      state.setThreadState({ archived: false })
      return threadMutation("unarchive", threadId)
    case "codexThreadDelete":
      state.setThreadState({ deleted: true })
      return threadMutation("delete", threadId)
    case "codexThreadFork":
      state.setThreadState({ forked: true })
      return { ...threadMutation("fork", threadId), newThreadId: forkThreadId }
    case "codexThreadResume": {
      const request = args[0] as { readonly threadId?: string } | undefined
      return threadResult(
        request?.threadId ?? threadId,
        state.threadState,
        state.activeErrorStateCase === "app_server_crash_restart" ? state.activeErrorStateCase : null,
      )
    }
    case "codexTurnStart":
    case "submitChatMessage":
      return chatTurnResponse(
        state.activeErrorStateCase === "network_loss_mid_turn" ? state.activeErrorStateCase : null,
      )
    case "codexTurnSteer":
    case "codexThreadCompact":
      return turnActionResult()
    case "codexTurnInterrupt":
      return turnActionResult(
        state.activeErrorStateCase === "interrupt_mid_tool_call" ? state.activeErrorStateCase : null,
      )
    case "codexFleetStatus":
      return fleetStatus(
        state.activeErrorStateCase === "pylon_offline" ? state.activeErrorStateCase : null,
      )
    case "codexFleetDelegateRun":
      return fleetDelegateRunResult()
    case "codexFleetPromoteThread":
      return fleetPromotionResult(args[0])
    case "fleetRunStart":
      state.setFleetRunState("running")
      return { ok: true, run: fleetRun("running"), supervisorStarted: true }
    case "fleetRunStatus":
      return { ok: true, run: fleetRun(state.fleetRunState), supervisorActive: state.fleetRunState === "running" }
    case "fleetRunList":
      return state.activeErrorStateCase === "single_rpc_failure_partial_degradation"
        ? {
          ok: false,
          runs: [fleetRun(state.fleetRunState, { dataPreservedCase: state.activeErrorStateCase })],
        }
        : { ok: true, runs: [fleetRun(state.fleetRunState)] }
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
    case "fleetWorkerControl": {
      const request = args[0] as {
        readonly assignmentRef?: string | null
        readonly issueRef?: string | null
        readonly runRef?: string | null
        readonly verb?: "interrupt" | "retry" | "flag"
        readonly workerRefHash?: string
      } | undefined
      return {
        accepted: true,
        assignmentRef: request?.assignmentRef ?? null,
        inboxItemRef: `inbox.fixture.${request?.verb ?? "flag"}`,
        ok: true,
        runRef: request?.runRef ?? null,
        verb: request?.verb ?? "flag",
        workerRefHash: request?.workerRefHash ?? "worker-fixture",
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
    case "claudeApprovalPending":
      return {
        ok: true,
        requests: [{
          id: "claude-approval-fixture",
          input: { command: "echo fixture" },
          options: { title: "Fixture approval", description: "Fixture approval request" },
          toolName: "Bash",
        }],
      }
    case "claudeApprovalRespond": {
      const request = args[0] as { readonly decision?: unknown; readonly requestId?: string } | undefined
      return {
        decision: request?.decision ?? { behavior: "allow" },
        ok: true,
        requestId: request?.requestId ?? "claude-approval-fixture",
      }
    }
    case "codexSettingsRead":
      return settingsProjection()
    case "codexConfigValueWrite": {
      const request = args[0] as { readonly keyPath?: string } | undefined
      return { ok: true, keyPath: request?.keyPath ?? "model", response: { applied: true }, settings: settingsProjection() }
    }
    case "harnessSettingRead":
      return harnessSetting("codex_harness")
    case "harnessSettingWrite": {
      const request = args[0] as { readonly mode?: "claude_runtime" | "codex_harness" | "khala_native_runtime" } | undefined
      return harnessSetting(request?.mode ?? "codex_harness", true)
    }
    case "codexEcosystemRead":
      return ecosystemProjection(
        state.activeErrorStateCase === "mcp_server_down" ? state.activeErrorStateCase : null,
      )
    case "toolCatalog":
      return toolCatalog()
    case "codexMentionCandidates":
      return { ok: true, candidates: [{ fileName: "README.md", kind: "file", path: "/workspace/README.md" }], source: "fuzzyFileSearch", truncated: false }
    case "codexBackgroundTerminalsList":
      return actionResult("thread/backgroundTerminals/list", { processes: [] })
    case "codexBackgroundTerminalsClean":
      return actionResult("thread/backgroundTerminals/clean", { cleaned: true })
    case "codexBackgroundTerminalsTerminate":
      return actionResult("thread/backgroundTerminals/terminate", { terminated: true })
    case "codexFsGetMetadata":
      return actionResult("fs/getMetadata", { kind: "file", path: "/workspace/README.md", sizeBytes: 123 })
    case "codexFsReadFile":
      return actionResult("fs/readFile", { dataBase64: "Zml4dHVyZQ==", path: "/workspace/README.md" })
    case "codexFsWriteFile":
      return actionResult("fs/writeFile", { bytesWritten: 7 })
    case "codexExternalAgentConfigDetect":
      return actionResult("config/externalAgent/detect", { items: [] })
    case "codexExternalAgentConfigImport":
      return actionResult("config/externalAgent/import", { imported: 1 })
    case "codexExternalAgentConfigImportHistoriesRead":
      return actionResult("config/externalAgent/importHistories/read", { histories: [] })
    case "codexMarketplaceAdd":
      return actionResult("marketplace/add", { marketplaceName: "fixture-marketplace" })
    case "codexMarketplaceRemove":
      return actionResult("marketplace/remove", { marketplaceName: "fixture-marketplace" })
    case "codexMarketplaceUpgrade":
      return actionResult("marketplace/upgrade", { marketplaceName: "fixture-marketplace" })
    case "codexMcpResourceRead":
      return actionResult("mcp/resource/read", { text: "fixture resource" })
    case "codexMcpToolCall":
      return actionResult("mcp/tool/call", { content: [{ text: "fixture tool result", type: "text" }] })
    case "codexMcpOauthLogin":
      return actionResult("mcp/oauth/login", { status: "completed" })
    case "codexMcpServerReload":
      return actionResult("config/mcpServer/reload", { reloaded: true })
    case "codexPluginInstall":
      return actionResult("plugin/install", { pluginId: "plugin-fixture" })
    case "codexPluginUninstall":
      return actionResult("plugin/uninstall", { pluginId: "plugin-fixture" })
    case "codexSkillsConfigWrite":
      return actionResult("skills/config/write", { written: true })
    case "codexSkillsExtraRootsSet":
      return actionResult("skills/extraRoots/set", { extraRoots: ["/workspace/.khala/skills"] })
    case "threadTokenSummary":
      return threadTokenSummary(args[0])
    case "sessionCatalog":
      return sessionCatalog(
        state.activeErrorStateCase === "corrupt_session_state_recovery" ? state.activeErrorStateCase : null,
      )
    case "forumRequest":
      return forumResponse(args[0])
    case "khalaCodePlanCatalog":
      return { ok: true, catalog: planCatalog() }
    case "khalaCodePlanStatus":
      return { state: "ok", plan: { captureExcluded: false, kind: "free", planId: "khala-code-free", reasonRef: "fixture.default_free" } }
    case "khalaCodePlanPurchase":
      return { ok: false, error: "khala_code_paid_plans_not_enabled" }
    case "qaMetricSample": {
      const sample = args[0] as KhalaCodeQaMetricSample
      state.appendQaMetricSample(sample)
      state.setActiveErrorStateCase(errorStateCaseFromMetricSample(sample))
      return { ok: true, observedAt: sample.observedAt }
    }
    case "qaMetrics":
      return qaMetricsSnapshot(state.qaMetricSamples)
    case "slashCommandList": {
      const request = args[0] as {
        readonly activeTurn?: boolean
        readonly debug?: boolean
        readonly platform?: string
        readonly sideConversation?: boolean
      } | undefined
      return {
        ok: true,
        commands: khalaCodeDesktopSlashCommandsWithAvailability({
          ...(request?.activeTurn === undefined ? {} : { activeTurn: request.activeTurn }),
          debug: request?.debug ?? true,
          platform: request?.platform ?? "darwin",
          ...(request?.sideConversation === undefined ? {} : { sideConversation: request.sideConversation }),
        }),
      }
    }
    case "slashCommandDispatch": {
      const request = args[0] as {
        readonly activeTurn?: boolean
        readonly debug?: boolean
        readonly platform?: string
        readonly raw?: string
        readonly sideConversation?: boolean
        readonly threadId?: string
      } | undefined
      const raw = request?.raw ?? "/status"
      const command = khalaCodeDesktopSlashCommandsWithAvailability({
        ...(request?.activeTurn === undefined ? {} : { activeTurn: request.activeTurn }),
        debug: request?.debug ?? true,
        platform: request?.platform ?? "darwin",
        ...(request?.sideConversation === undefined ? {} : { sideConversation: request.sideConversation }),
      })
        .find((item) => raw.replace(/^\/+/, "").split(/\s+/)[0] === item.command)
      return slashCommandDispatchResult(command, raw, request)
    }
    default:
      return actionResult(method, { fixture: true })
  }
}

const fleetRunStateForVerb = (
  verb: "pause" | "resume" | "drain" | "stop" | undefined,
): FixtureFleetRunState => {
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

const appServerStatus = (
  state: FixtureAppServerState,
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => ({
  adapterVersion: "fixture",
  app: "Khala Code Desktop",
  codexCommand: "codex",
  codexHome: "/fixture/codex-home",
  diagnostics: errorCase === null ? [] : [errorStateDataPreservedRef(errorCase)],
  initialized: state === "running",
  initializeResult: errorCase === null
    ? state === "running" ? { ok: true, fixture: true } : null
    : { degradedState: errorStateMarker(errorCase, state === "running" ? "recovered" : "degraded") },
  lastError: state === "errored"
    ? `fixture app-server error; ${errorCase === null ? "data preserved" : errorStateDataPreservedRef(errorCase)}`
    : null,
  ok: true,
  pendingRequestCount: 0,
  pid: state === "running" ? 12345 : null,
  state,
  transport: "stdio",
})

const appServerControl = (
  action: "restart" | "start" | "stop",
  previousState: FixtureAppServerState,
  nextState: FixtureAppServerState,
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => ({
  action,
  changed: previousState !== nextState || action === "restart",
  ok: true,
  status: appServerStatus(nextState, errorCase),
})

const runtimeStatus = (
  capability: "codex_accounts" | "codex_harness" | "coding" | "pylon" | "token_accounting",
  status: "error" | "not_configured" | "ready" | "unavailable",
  reason = "fixture backend",
) => ({
  app: "Khala Code Desktop",
  available: status === "ready",
  capability,
  observedAt,
  ok: true,
  reason,
  status,
})

const codexHarnessStatus = (
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => {
  const binaryMissing = errorCase === "codex_binary_missing"
  const authExpired = errorCase === "auth_expired"
  return {
    ...runtimeStatus("codex_harness", "ready"),
    ...(binaryMissing || authExpired
      ? runtimeStatus("codex_harness", "unavailable", `${errorStateDataPreservedRef(errorCase)}; existing threads and session metadata remain readable`)
      : {}),
    auth: {
      accessTokenPresent: !authExpired,
      accountIdPresent: !authExpired,
      blockerRefs: authExpired ? [errorStateDataPreservedRef(errorCase)] : [],
      refreshTokenPresent: !authExpired,
      state: authExpired ? "invalid" : "ready",
      ...(authExpired ? { error: "Codex auth expired; data preserved" } : {}),
    },
    binary: {
      available: !binaryMissing,
      command: "codex",
      error: binaryMissing ? "Codex CLI not found; data preserved" : null,
      source: "PATH",
      version: binaryMissing ? null : "fixture",
    },
    home: {
      authPath: "/fixture/codex-home/auth.json",
      fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
      path: "/fixture/codex-home",
      role: "main_user_codex_home",
      source: "input",
    },
    signIn: {
      command: "codex login",
      required: authExpired,
      warning: authExpired
        ? "Sign in again; isolated worker homes and existing transcript data are preserved."
        : "fixture sign-in not required",
    },
  }
}

const appleFmReadiness = () => ({
  available: false,
  backendKind: "apple_foundation_models",
  blockerRefs: ["fixture.apple_fm.unavailable"],
  capability: "local_decider",
  contentRedacted: true,
  demandKind: "own_capacity",
  demandSource: "khala_code_fixture",
  kind: "khala_desktop_apple_fm_readiness",
  model: "fixture-apple-fm",
  observedAt,
  profileId: "fixture-profile",
  provider: "apple",
  pylon: null,
  pylonControlConfigured: false,
  schema: "openagents.khala_code.apple_fm_readiness.v1",
  state: "unavailable",
  supported: true,
  usageTruth: "estimated",
})

const onDeviceDeciderStatus = () => ({
  preferred: "hosted_openagents",
  readiness: [{
    available: false,
    backend: "apple_foundation_models",
    detail: "fixture unavailable",
    model: "fixture-apple-fm",
  }],
  reason: "fixture backend",
  selected: null,
})

const harnessSetting = (
  mode: "claude_runtime" | "codex_harness" | "khala_native_runtime",
  saved?: boolean,
) => ({
  envOverride: null,
  mode,
  ok: true,
  path: "/fixture/khala-code-runtime-mode.json",
  persistedMode: mode,
  ...(saved === undefined ? {} : { saved }),
})

const toolCatalog = () => ({
  catalogKind: "codex_harness_supplemental",
  defaultEnabled: true,
  description: "Fixture tool catalog",
  runtimeMode: "codex_harness",
  toolCount: 1,
  tools: [{
    authority: "fixture",
    name: "fixture_tool",
    role: "supplemental_swarm",
  }],
})

const fixtureItemStatus = (fixture: KhalaCodeCodexThreadItemFixture): string =>
  typeof fixture.item.status === "string" ? fixture.item.status : fixture.rendersVisible ? "completed" : "suppressed"

const threadItemCard = (
  fixture: KhalaCodeCodexThreadItemFixture,
  fallbackId: string,
) => {
  const itemId = typeof fixture.item.id === "string" ? fixture.item.id : fallbackId
  return {
    itemId,
    itemType: fixture.variant,
    status: fixtureItemStatus(fixture),
    subtitle: KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE.referenceLabel,
    title: `${fixture.variant} fixture`,
    threadId,
    turnId,
  }
}

const threadMessage = (
  id: string,
  body: string,
  fixture?: KhalaCodeCodexThreadItemFixture,
) => ({
  body,
  id,
  role: "assistant",
  ...(fixture === undefined
    ? {}
    : {
      codexItem: threadItemCard(fixture, id),
      harnessItem: threadItemCard(fixture, id),
    }),
})

const threadResult = (id: string, state: FixtureThreadState = {
  archived: false,
  deleted: false,
  forked: false,
  title: "Fixture thread",
}, errorCase: KhalaCodeQaErrorStateCaseId | null = null) => {
  const fixture = threadItemFixtureForThreadId(id)
  return {
    cwd: "/workspace",
    desktopSessionId,
    messages: [threadMessage(
      `message-${id}`,
      fixture === undefined
        ? "fixture message"
        : `Pinned ThreadItem fixture replay: ${fixture.variant}`,
      fixture,
    )],
    model: "gpt-5.1-codex",
    modelProvider: "openai",
    ok: true,
    thread: {
      archived: state.archived,
      deleted: state.deleted,
      id,
      parityFixture: fixture === undefined
        ? null
        : {
          item: fixture.item,
          rendersVisible: fixture.rendersVisible,
          source: KHALA_CODE_CODEX_THREAD_ITEM_FIXTURE_SOURCE,
          variant: fixture.variant,
        },
      ...(errorCase === null ? {} : { degradedState: errorStateMarker(errorCase, "recovered") }),
      title: fixture === undefined ? state.title : `${fixture.variant} fixture`,
    },
    threadId: id,
  }
}

const threadSummary = (id: string, state: FixtureThreadState = {
  archived: false,
  deleted: false,
  forked: false,
  title: "Fixture thread",
}) => ({
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
  title: state.title,
  updatedAt: 1,
})

const threadListResult = (request: unknown, state: FixtureThreadState) => {
  const archived = (request as { readonly archived?: boolean } | undefined)?.archived ?? false
  const visibleThreads = state.deleted || state.archived !== archived ? [] : [threadSummary(threadId, state)]
  return {
  backwardsCursor: null,
  data: visibleThreads,
  groups: [{ key: archived ? "archived" : "recent", label: archived ? "Archived" : "Recent", threadIds: visibleThreads.map((thread) => thread.id) }],
  nextCursor: null,
  ok: true,
  threads: visibleThreads,
  }
}

const threadMutation = (action: "archive" | "delete" | "fork" | "rename" | "unarchive", id: string) => ({
  action,
  ok: true,
  thread: { id },
  threadId: id,
})

const turnActionResult = (
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => ({
  codexTurnId: turnId,
  desktopSessionId,
  desktopTurnId: turnId,
  ok: true,
  ...(errorCase === null ? {} : { response: { degradedState: errorStateMarker(errorCase, "recovering") } }),
  threadId,
})

const chatTurnResponse = (
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => ({
  backend: {
    kind: errorCase === "network_loss_mid_turn" ? "codex_app_server" : "mock",
    model: "fixture-model",
    threadId,
    turnId,
    ...(errorCase === null ? {} : { turnStatus: `qa.error_state.${errorCase}.data_preserved` }),
  },
  messages: [
    {
      body: errorCase === null
        ? "fixture turn response"
        : `${errorStateDataPreservedRef(errorCase)}: turn degraded; draft and prior messages preserved`,
      id: "msg-assistant",
      role: "assistant",
    },
  ],
  ok: errorCase === null,
  toolNames: [],
  usedTools: [],
  usage: {
    cachedInput: 0,
    input: 1,
    output: 1,
    reasoningOutput: 0,
  },
})

const fleetStatus = (
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => ({
  accounts: [],
  activeAssignments: [],
  availableCodexAssignments: errorCase === "pylon_offline" ? 0 : 1,
  maxCodexAssignments: 1,
  observedAt,
  ok: true,
  processes: [],
  pylon: {
    message: errorCase === "pylon_offline"
      ? `${errorStateDataPreservedRef(errorCase)}; fleet context preserved while Pylon is offline`
      : "fixture pylon",
    pylonRef: errorCase === "pylon_offline" ? null : "pylon-fixture",
    status: errorCase === "pylon_offline" ? "unavailable" : "online",
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
    unavailableReason: errorCase === null ? "fixture" : errorStateDataPreservedRef(errorCase),
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
    module: "intake",
    precondition: "fixture request decoded",
    refs: [],
    status: "ok",
    summary: "fixture intake accepted",
  }, {
    blockerCode: null,
    fallbackModule: null,
    module: "preflight",
    precondition: "fixture pins checked",
    refs: [],
    status: "ok",
    summary: "fixture preflight passed",
  }, {
    blockerCode: null,
    fallbackModule: null,
    module: "capacity",
    precondition: "fixture slot available",
    refs: [],
    status: "ok",
    summary: "fixture capacity reserved",
  }, {
    blockerCode: null,
    fallbackModule: null,
    module: "dispatch",
    precondition: "fixture assignment accepted",
    refs: ["assignment-fixture"],
    status: "ok",
    summary: "fixture dispatch completed",
  }, {
    blockerCode: null,
    fallbackModule: null,
    module: "closeout",
    precondition: "fixture no-spend closeout",
    refs: [],
    status: "ok",
    summary: "fixture closeout not applicable",
  }, {
    blockerCode: null,
    fallbackModule: null,
    module: "report",
    precondition: "fixture report public-safe",
    refs: [],
    status: "ok",
    summary: "fixture report emitted",
  }],
  validation: {
    fixture: true,
    repoPinsComplete: false,
  },
  workerRuntime,
})

const fleetPromotionResult = (request: unknown) => {
  const input = request as {
    readonly contextBoundary?: {
      readonly allowedRefs?: readonly string[]
      readonly includeTranscript?: false
      readonly mode?: "explicit_objective" | "summary_only"
      readonly summary?: string | null
    }
    readonly count?: number
    readonly sessionId?: string
    readonly threadId?: string
  } | undefined
  return {
    acceptedCount: input?.count ?? 1,
    contextBoundary: {
      allowedRefs: input?.contextBoundary?.allowedRefs ?? [threadId],
      includeTranscript: false,
      mode: input?.contextBoundary?.mode ?? "summary_only",
      summary: input?.contextBoundary?.summary ?? "fixture summary",
    },
    ok: true,
    origin: {
      role: "main_local_codex_session",
      sessionId: input?.sessionId ?? desktopSessionId,
      threadId: input?.threadId ?? threadId,
    },
    pylonRef: "pylon-fixture",
    requestedCount: input?.count ?? 1,
    results: [{
      accountRef: "codex",
      assignmentRef: "assignment-fixture",
      closeoutStatus: "not_applicable",
      status: "accepted",
      summary: "fixture promotion accepted",
      tokensVerified: 0,
      transcriptRef: null,
    }],
    workerRuntime,
  }
}

const fleetRun = (
  state: "draft" | "running" | "paused" | "draining" | "completed" | "stopped",
  options: { readonly dataPreservedCase?: KhalaCodeQaErrorStateCaseId } = {},
) => ({
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
  workSource: {
    kind: "fixture",
    count: 1,
    ...(options.dataPreservedCase === undefined
      ? {}
      : { planRef: errorStateDataPreservedRef(options.dataPreservedCase) }),
  },
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

const ecosystemSection = (
  source: "apps" | "hooks" | "imports" | "khala" | "marketplace" | "mcp" | "plugins" | "skills",
  label: string,
  options: { readonly errorCase?: KhalaCodeQaErrorStateCaseId } = {},
) => ({
  authRequiredCount: 0,
  count: 1,
  disabledCount: 0,
  errorCount: options.errorCase === undefined ? 0 : 1,
  installRequiredCount: 0,
  items: [{
    authRequired: false,
    detail: options.errorCase === undefined ? "fixture ready" : errorStateDataPreservedRef(options.errorCase),
    enabled: options.errorCase === undefined,
    id: `${source}-fixture`,
    installed: true,
    managed: false,
    name: label,
    source,
    state: options.errorCase === undefined ? "ready" : "error",
  }],
  label,
  managedCount: 0,
  readyCount: options.errorCase === undefined ? 1 : 0,
  source,
  unknownCount: 0,
})

const ecosystemProjection = (
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => ({
  cwd: "/workspace",
  diagnostics: errorCase === "mcp_server_down"
    ? [{
      action: "refresh",
      detail: `${errorStateDataPreservedRef(errorCase)}; existing ecosystem sections remain visible`,
      observedAt,
      ref: errorStateDataPreservedRef(errorCase),
      severity: "warning",
      source: "mcp",
      title: "Fixture MCP server down",
    }]
    : [],
  errors: errorCase === "mcp_server_down" ? [errorStateDataPreservedRef(errorCase)] : [],
  notifications: errorCase === "mcp_server_down"
    ? [{
      method: "mcpServer/reload",
      receivedAt: observedAt,
      severity: "warning",
      summary: errorStateDataPreservedRef(errorCase),
    }]
    : [],
  observedAt,
  ok: errorCase !== "mcp_server_down",
  sections: {
    apps: ecosystemSection("apps", "Apps"),
    hooks: ecosystemSection("hooks", "Hooks"),
    imports: ecosystemSection("imports", "Imports"),
    khala: ecosystemSection("khala", "Khala"),
    marketplace: ecosystemSection("marketplace", "Marketplace"),
    mcp: ecosystemSection("mcp", "MCP", errorCase === "mcp_server_down" ? { errorCase } : {}),
    plugins: ecosystemSection("plugins", "Plugins"),
    skills: ecosystemSection("skills", "Skills"),
  },
})

const threadTokenSummary = (request: unknown) => ({
  auditRows: 1,
  codexStateDbPath: "/fixture/codex-state.db",
  codexStateTokens: 1,
  leaderboardLabel: "OpenAgents Stats",
  leaderboardSyncedTokens: 1,
  localLedgerPath: "/fixture/token-usage.jsonl",
  localMessageAuditLedgerPath: "/fixture/message-audit.jsonl",
  missingUsageTurns: 0,
  ok: true,
  pendingSyncTokens: 0,
  remoteConfigured: false,
  remoteDisabled: true,
  threadId: (request as { readonly threadId?: string | null } | undefined)?.threadId ?? threadId,
  totalTokens: 2,
  updatedAt: observedAt,
  usageEventRows: 1,
})

const sessionCatalog = (
  errorCase: KhalaCodeQaErrorStateCaseId | null = null,
) => ({
  diagnostics: errorCase === "corrupt_session_state_recovery"
    ? [errorStateDataPreservedRef(errorCase)]
    : [],
  entries: [{
    catalogEntryId: "catalog-fixture",
    createdAt: 1,
    cwd: "/workspace",
    desktopSessionRef: desktopSessionId,
    exactTotals: {
      cachedInputTokens: 0,
      inputTokens: 1,
      outputTokens: 1,
      reasoningOutputTokens: 0,
      source: "fixture",
      totalTokens: 2,
    },
    harnessKind: "codex",
    lastTurnRef: turnId,
    preview: "fixture session",
    projectLabel: "workspace",
    recencyAt: 1,
    sessionRef: desktopSessionId,
    source: "fixture",
    status: errorCase === "corrupt_session_state_recovery" ? "degraded_recovered" : "ready",
    statusLabel: errorCase === "corrupt_session_state_recovery" ? "Recovered" : "Ready",
    threadRef: threadId,
    title: "Fixture session",
    updatedAt: 1,
  }],
  ok: true,
  schemaVersion: "khala-code-desktop.session-catalog.v1",
})

const forumResponse = (request: unknown) => {
  const input = request as {
    readonly body?: unknown
    readonly method?: "GET" | "POST"
    readonly path?: string
  } | undefined
  return {
    ok: true,
    payload: {
      body: input?.body ?? null,
      method: input?.method ?? "GET",
      path: input?.path ?? "/forum",
      ref: "forum-fixture",
    },
    status: input?.method === "POST" ? 201 : 200,
  }
}

const planCatalog = () => ({
  authorityBoundary: "server_resolved_fixture",
  blockerRefs: ["khala_code_paid_plans_not_enabled"],
  catalogVersion: "fixture.1",
  plans: [
    {
      captureExcluded: false,
      isDefault: true,
      kind: "free",
      label: "Free",
      planId: "khala-code-free",
      priceLabel: "$0",
      tagline: "Pay with data",
      terms: ["Fixture free plan"],
    },
    {
      captureExcluded: true,
      isDefault: false,
      kind: "paid",
      label: "Paid",
      planId: "khala-code-paid",
      priceLabel: "$20/mo",
      purchase: {
        armed: false,
        envFlag: "KHALA_CODE_PAID_PLANS_ENABLED",
        route: "/api/khala-code/plans/purchase",
      },
      tagline: "Private data",
      terms: ["Fixture paid plan"],
    },
  ],
  promiseId: "khala_code.free_paid_plans.v1",
  relatedPromiseIds: [],
  schemaVersion: "openagents.khala_code.plan_catalog.v1",
  summary: "Fixture free and paid plan catalog.",
})

const qaMetricsSnapshot = (
  samples: readonly KhalaCodeQaMetricSample[],
) => ({
  budgets: khalaCodeQaMetricBudgets,
  definitions: khalaCodeQaMetricDefinitions,
  evaluations: evaluateKhalaCodeQaMetricBudgets(samples),
  ok: true,
  observedAt,
  samples,
  schema: "openagents.khala_code.qa_metrics.v1",
})

const actionResult = (method: string, response?: unknown) => ({
  method,
  ok: true,
  ...(response === undefined ? {} : { response }),
})

const slashCommandDispatchResult = (
  command: ReturnType<typeof khalaCodeDesktopSlashCommandsWithAvailability>[number] | undefined,
  raw: string,
  request?: {
    readonly threadId?: string
  },
) => {
  if (command === undefined) {
    return { message: `Unknown slash command: ${raw}`, ok: false, status: "not_found" }
  }
  const args = raw.trim().split(/\s+/).slice(1).join(" ").trim()
  if (!command.availability.available) {
    return {
      command: command.command,
      message: command.availability.reason ?? `/${command.command} unavailable`,
      ok: false,
      status: "blocked",
    }
  }
  switch (command.dispatch.kind) {
    case "app_server": {
      if (command.dispatch.requiresArgs === true && args.length === 0) {
        return {
          command: command.command,
          message: `/${command.command} requires inline arguments.`,
          method: command.dispatch.method,
          ok: false,
          status: "blocked",
        }
      }
      if (command.dispatch.requiresThread === true && (request?.threadId ?? "").trim().length === 0) {
        return {
          command: command.command,
          message: `/${command.command} requires an active Codex thread.`,
          method: command.dispatch.method,
          ok: false,
          status: "blocked",
        }
      }
      return {
        command: command.command,
        message: `Dispatched /${command.command}`,
        method: command.dispatch.method,
        ok: true,
        response: { fixture: true },
        status: "dispatched",
        ...(request?.threadId === undefined ? {} : { threadId: request.threadId }),
      }
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
        ...(command.dispatch.unavailable === undefined ? {} : { gap: command.dispatch.unavailable }),
        message: command.dispatch.dependency,
        ok: false,
        status: command.dispatch.unavailable === undefined ? "gap" : "unavailable",
      }
  }
}
