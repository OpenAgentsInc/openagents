import type { AgentRuntimeAdapterKind } from "@openagentsinc/agent-runtime-schema"

import type { ResolvedPylonAccountSelection } from "./account-registry.js"
import type { PylonCodexAuthValidityProbe } from "./account-connect.js"
import {
  CLAUDE_AGENT_SDK_PACKAGE,
  CLAUDE_AGENT_CAPABILITY_REF,
  type ClaudeAgentProbeOptions,
} from "./claude-agent.js"
import {
  CLAUDE_AGENT_TASK_SCHEMA,
  CLAUDE_AGENT_TASK_AGENT_KIND,
  claudeAgentTaskFrom,
  executeClaudeAgentAssignment,
  type ClaudeAgentCheckoutRunner,
  type ClaudeAgentExecutionOptions,
  type ClaudeAgentRunner,
} from "./claude-agent-executor.js"
import {
  CODEX_AGENT_SDK_PACKAGE,
  CODEX_AGENT_CAPABILITY_REF,
  type CodexAgentProbeOptions,
} from "./codex-agent.js"
import {
  CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY,
  CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE,
  CODEX_AGENT_TASK_SCHEMA,
  CODEX_AGENT_TASK_AGENT_KIND,
  codexAgentTaskFrom,
  executeCodexAgentAssignment,
  type CodexAgentExecutionOptions,
  type CodexAgentRuntimeProgress,
  type CodexAgentRunner,
} from "./codex-agent-executor.js"
import type { PylonAssignmentLease } from "./assignment.js"
import type { PylonLocalState } from "./state.js"
import type { GitCheckoutWorkspace } from "./workspace-materializer.js"

export type AgentRunnerKind = "claude_agent" | "codex"
export type AgentRunnerTaskAgentKind =
  | typeof CLAUDE_AGENT_TASK_AGENT_KIND
  | typeof CODEX_AGENT_TASK_AGENT_KIND
export type AgentRunnerServiceRef = "claude" | "codex"
export type AgentRunnerAccountProvider = "claude_agent" | "codex"
export type AgentRunnerTaskField = "claudeAgent" | "codex"

export type AgentRunnerReadinessProbeKind =
  | "claude_agent_sdk_import"
  | "codex_sdk_import_or_cli_login"

export type AgentRunnerWorkspaceBoundary =
  | {
      strategy: "pre_tool_use_hook"
      enforcement: "deny_before_tool_use"
    }
  | {
      strategy: "post_hoc_workspace_validation"
      enforcement: "reject_closeout_on_escape"
    }

export type AgentRunnerExecutionPolicy = {
  approvalPolicy: "pre_tool_use_deny" | typeof CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY
  networkAccess: "runner_default" | "enabled"
  sandboxMode: "bounded_tool_allowlist" | typeof CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE
}

export type AgentRunnerTurnReporterKind =
  | "pylon_claude_turn_reporter"
  | "pylon_codex_turn_reporter"

export type AgentRunnerTurnReporterContract = {
  endpointPath: "/api/pylon/claude/turns" | "/api/pylon/codex/turns"
  failSoft: true
  kind: AgentRunnerTurnReporterKind
  usageTruth: "exact"
}

export type AgentRunnerRuntimeContract = {
  sdkPackage: string
  readinessProbe: AgentRunnerReadinessProbeKind
  executionPolicy: AgentRunnerExecutionPolicy
  turnReporter: AgentRunnerTurnReporterContract
  workspaceBoundary: AgentRunnerWorkspaceBoundary
}

export type AgentTaskPayload = {
  schema: string
  agentKind: AgentRunnerTaskAgentKind
  fixtureRef?: string
  objectiveSummary?: string
  timeoutSeconds?: number
  workspace?: GitCheckoutWorkspace
}

export type AgentRunnerTaskContract = {
  field: AgentRunnerTaskField
  schema: string
  agentKind: AgentRunnerTaskAgentKind
}

export type AgentRunnerCloseoutRecord = {
  artifactRefs: string[]
  blockerRefs: string[]
  buildRefs: string[]
  message: string
  previewRefs: string[]
  proofRefs: string[]
  resultRefs: string[]
  runRefs: string[]
  status: "accepted" | "rejected" | "timed-out"
  summaryRefs: string[]
  testRefs: string[]
}

export type AgentRunnerExecutionOptions = {
  account?: ResolvedPylonAccountSelection | null
  agentToken?: string
  baseUrl?: string
  claudeAgentCheckoutRunner?: ClaudeAgentCheckoutRunner
  claudeAgentProbe?: ClaudeAgentProbeOptions
  claudeAgentRunner?: ClaudeAgentRunner
  codexAgentProbe?: CodexAgentProbeOptions
  codexAgentRunner?: CodexAgentRunner
  codexAuthValidityProbe?: PylonCodexAuthValidityProbe
  onCodexProgress?: (progress: CodexAgentRuntimeProgress) => void | Promise<void>
  fetch?: typeof fetch
}

export type AgentRunnerInput = {
  state: PylonLocalState
  lease: PylonAssignmentLease
  now: Date
  options: AgentRunnerExecutionOptions
}

export interface AgentRunner {
  run(input: AgentRunnerInput): Promise<AgentRunnerCloseoutRecord | null>
}

export type AgentRunnerDescriptor = {
  kind: AgentRunnerKind
  agentKind: AgentRunnerTaskAgentKind
  adapterKind: AgentRuntimeAdapterKind
  accountProvider: AgentRunnerAccountProvider
  serviceRef: AgentRunnerServiceRef
  capabilityRef: string
  task: AgentRunnerTaskContract
  runtime: AgentRunnerRuntimeContract
  canRunAssignment: (codingAssignment: unknown) => boolean
  run: AgentRunner["run"]
  execute: (
    state: PylonLocalState,
    lease: PylonAssignmentLease,
    now: Date,
    options: AgentRunnerExecutionOptions,
  ) => Promise<AgentRunnerCloseoutRecord | null>
}

export type AgentRunnerResolution =
  | { status: "matched"; runner: AgentRunnerDescriptor }
  | { status: "none" }
  | {
      status: "ambiguous"
      runnerKinds: AgentRunnerKind[]
      blockerRef: "blocker.assignment.agent_runner_ambiguous"
    }

function hasObjectField(value: unknown, key: AgentRunnerTaskField): boolean {
  const field = (value as Record<string, unknown> | undefined)?.[key]
  return field !== null && typeof field === "object"
}

export function agentTaskPayloadFromAssignment(
  codingAssignment: unknown,
  task: AgentRunnerTaskContract,
): AgentTaskPayload | null {
  const payload = (codingAssignment as Record<string, unknown> | undefined)?.[task.field]
  if (payload === null || typeof payload !== "object") return null
  const candidate = payload as AgentTaskPayload
  if (candidate.schema !== task.schema) return null
  if (candidate.agentKind !== task.agentKind) return null
  return candidate
}

function commonExecutionOptions(options: AgentRunnerExecutionOptions) {
  return {
    ...(options.account === undefined ? {} : { account: options.account }),
    ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
    ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
    ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
  }
}

const CLAUDE_AGENT_RUNNER_TASK = {
  field: "claudeAgent",
  schema: CLAUDE_AGENT_TASK_SCHEMA,
  agentKind: CLAUDE_AGENT_TASK_AGENT_KIND,
} satisfies AgentRunnerTaskContract

const CODEX_AGENT_RUNNER_TASK = {
  field: "codex",
  schema: CODEX_AGENT_TASK_SCHEMA,
  agentKind: CODEX_AGENT_TASK_AGENT_KIND,
} satisfies AgentRunnerTaskContract

function canRunClaudeAgentAssignment(codingAssignment: unknown): boolean {
  return (
    agentTaskPayloadFromAssignment(codingAssignment, CLAUDE_AGENT_RUNNER_TASK) !== null &&
    claudeAgentTaskFrom(codingAssignment) !== null
  )
}

function canRunCodexAgentAssignment(codingAssignment: unknown): boolean {
  return (
    agentTaskPayloadFromAssignment(codingAssignment, CODEX_AGENT_RUNNER_TASK) !== null &&
    codexAgentTaskFrom(codingAssignment) !== null
  )
}

function runClaudeAgentRunner({ state, lease, now, options }: AgentRunnerInput) {
  return executeClaudeAgentAssignment(state, lease, now, {
    ...commonExecutionOptions(options),
    ...(options.claudeAgentCheckoutRunner === undefined
      ? {}
      : { checkoutRunner: options.claudeAgentCheckoutRunner }),
    ...(options.claudeAgentProbe === undefined ? {} : { claudeAgentProbe: options.claudeAgentProbe }),
    ...(options.claudeAgentRunner === undefined ? {} : { claudeAgentRunner: options.claudeAgentRunner }),
  } satisfies ClaudeAgentExecutionOptions)
}

function runCodexAgentRunner({ state, lease, now, options }: AgentRunnerInput) {
  return executeCodexAgentAssignment(state, lease, now, {
    ...commonExecutionOptions(options),
    ...(options.codexAgentProbe === undefined ? {} : { codexAgentProbe: options.codexAgentProbe }),
    ...(options.codexAgentRunner === undefined ? {} : { codexAgentRunner: options.codexAgentRunner }),
    ...(options.codexAuthValidityProbe === undefined
      ? {}
      : { codexAuthValidityProbe: options.codexAuthValidityProbe }),
    ...(options.onCodexProgress === undefined ? {} : { onProgress: options.onCodexProgress }),
  } satisfies CodexAgentExecutionOptions)
}

export const AGENT_RUNNER_REGISTRY: ReadonlyArray<AgentRunnerDescriptor> = [
  {
    kind: "claude_agent",
    agentKind: CLAUDE_AGENT_TASK_AGENT_KIND,
    adapterKind: "claude_code",
    accountProvider: "claude_agent",
    serviceRef: "claude",
    capabilityRef: CLAUDE_AGENT_CAPABILITY_REF,
    task: CLAUDE_AGENT_RUNNER_TASK,
    runtime: {
      sdkPackage: CLAUDE_AGENT_SDK_PACKAGE,
      readinessProbe: "claude_agent_sdk_import",
      executionPolicy: {
        approvalPolicy: "pre_tool_use_deny",
        networkAccess: "runner_default",
        sandboxMode: "bounded_tool_allowlist",
      },
      turnReporter: {
        endpointPath: "/api/pylon/claude/turns",
        failSoft: true,
        kind: "pylon_claude_turn_reporter",
        usageTruth: "exact",
      },
      workspaceBoundary: {
        strategy: "pre_tool_use_hook",
        enforcement: "deny_before_tool_use",
      },
    },
    canRunAssignment: canRunClaudeAgentAssignment,
    run: runClaudeAgentRunner,
    execute: (state, lease, now, options) => runClaudeAgentRunner({ state, lease, now, options }),
  },
  {
    kind: "codex",
    agentKind: CODEX_AGENT_TASK_AGENT_KIND,
    adapterKind: "codex",
    accountProvider: "codex",
    serviceRef: "codex",
    capabilityRef: CODEX_AGENT_CAPABILITY_REF,
    task: CODEX_AGENT_RUNNER_TASK,
    runtime: {
      sdkPackage: CODEX_AGENT_SDK_PACKAGE,
      readinessProbe: "codex_sdk_import_or_cli_login",
      executionPolicy: {
        approvalPolicy: CODEX_AGENT_OWNER_LOCAL_APPROVAL_POLICY,
        networkAccess: "enabled",
        sandboxMode: CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE,
      },
      turnReporter: {
        endpointPath: "/api/pylon/codex/turns",
        failSoft: true,
        kind: "pylon_codex_turn_reporter",
        usageTruth: "exact",
      },
      workspaceBoundary: {
        strategy: "post_hoc_workspace_validation",
        enforcement: "reject_closeout_on_escape",
      },
    },
    canRunAssignment: canRunCodexAgentAssignment,
    run: runCodexAgentRunner,
    execute: (state, lease, now, options) => runCodexAgentRunner({ state, lease, now, options }),
  },
]

export const AGENT_RUNNER_REGISTRY_BY_KIND = Object.freeze(
  Object.fromEntries(AGENT_RUNNER_REGISTRY.map((runner) => [runner.kind, runner])) as Record<
    AgentRunnerKind,
    AgentRunnerDescriptor
  >,
)

export const AGENT_RUNNER_REGISTRY_BY_AGENT_KIND = Object.freeze(
  Object.fromEntries(AGENT_RUNNER_REGISTRY.map((runner) => [runner.agentKind, runner])) as Record<
    AgentRunnerTaskAgentKind,
    AgentRunnerDescriptor
  >,
)

export function agentRunnerForAdapterKind(
  adapterKind: AgentRuntimeAdapterKind,
): AgentRunnerDescriptor | null {
  return AGENT_RUNNER_REGISTRY.find((runner) => runner.adapterKind === adapterKind) ?? null
}

export function agentRunnerForKind(kind: AgentRunnerKind): AgentRunnerDescriptor {
  return AGENT_RUNNER_REGISTRY_BY_KIND[kind]
}

export function agentRunnerForAgentKind(
  agentKind: AgentRunnerTaskAgentKind,
): AgentRunnerDescriptor {
  return AGENT_RUNNER_REGISTRY_BY_AGENT_KIND[agentKind]
}

export function agentRunnerResolutionForLease(lease: PylonAssignmentLease): AgentRunnerResolution {
  const matches = AGENT_RUNNER_REGISTRY.filter((runner) => runner.canRunAssignment(lease.codingAssignment))
  if (matches.length === 0) return { status: "none" }
  if (matches.length === 1) return { status: "matched", runner: matches[0] }
  return {
    status: "ambiguous",
    runnerKinds: matches.map((runner) => runner.kind),
    blockerRef: "blocker.assignment.agent_runner_ambiguous",
  }
}

export function agentRunnerForLease(lease: PylonAssignmentLease): AgentRunnerDescriptor | null {
  const resolution = agentRunnerResolutionForLease(lease)
  return resolution.status === "matched" ? resolution.runner : null
}

export function agentRunnerServiceForLease(lease: PylonAssignmentLease): AgentRunnerServiceRef | null {
  const resolution = agentRunnerResolutionForLease(lease)
  if (resolution.status === "matched") return resolution.runner.serviceRef
  if (resolution.status === "ambiguous") return null

  const codingAssignment = lease.codingAssignment
  const legacyRunner = AGENT_RUNNER_REGISTRY.find((candidate) => {
    if (lease.capabilityRefs.includes(candidate.capabilityRef)) return true
    return hasObjectField(codingAssignment, candidate.task.field)
  })
  return legacyRunner?.serviceRef ?? null
}

export async function executeRegisteredAgentRunner(
  state: PylonLocalState,
  lease: PylonAssignmentLease,
  now: Date,
  options: AgentRunnerExecutionOptions,
): Promise<AgentRunnerCloseoutRecord | null> {
  const resolution = agentRunnerResolutionForLease(lease)
  if (resolution.status !== "matched") return null
  return resolution.runner.run({ state, lease, now, options })
}
