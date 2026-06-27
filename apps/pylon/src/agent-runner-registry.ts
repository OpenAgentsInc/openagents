import type { AgentRuntimeAdapterKind } from "@openagentsinc/agent-runtime-schema"

import type { ResolvedPylonAccountSelection } from "./account-registry.js"
import {
  CLAUDE_AGENT_CAPABILITY_REF,
  type ClaudeAgentProbeOptions,
} from "./claude-agent.js"
import {
  claudeAgentTaskFrom,
  executeClaudeAgentAssignment,
  type ClaudeAgentCheckoutRunner,
  type ClaudeAgentExecutionOptions,
  type ClaudeAgentRunner,
} from "./claude-agent-executor.js"
import {
  CODEX_AGENT_CAPABILITY_REF,
  type CodexAgentProbeOptions,
} from "./codex-agent.js"
import {
  codexAgentTaskFrom,
  executeCodexAgentAssignment,
  type CodexAgentExecutionOptions,
  type CodexAgentRunner,
} from "./codex-agent-executor.js"
import type { PylonAssignmentLease } from "./assignment.js"
import type { PylonLocalState } from "./state.js"

export type AgentRunnerKind = "claude_agent" | "codex"
export type AgentRunnerServiceRef = "claude" | "codex"
export type AgentRunnerAccountProvider = "claude_agent" | "codex"

export type AgentRunnerCloseoutRecord = {
  artifactRefs: string[]
  blockerRefs: string[]
  buildRefs: string[]
  message: string
  previewRefs: string[]
  proofRefs: string[]
  resultRefs: string[]
  runRefs: string[]
  status: "accepted" | "rejected"
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
  fetch?: typeof fetch
}

export type AgentRunnerDescriptor = {
  kind: AgentRunnerKind
  adapterKind: AgentRuntimeAdapterKind
  accountProvider: AgentRunnerAccountProvider
  serviceRef: AgentRunnerServiceRef
  capabilityRef: string
  canRunAssignment: (codingAssignment: unknown) => boolean
  execute: (
    state: PylonLocalState,
    lease: PylonAssignmentLease,
    now: Date,
    options: AgentRunnerExecutionOptions,
  ) => Promise<AgentRunnerCloseoutRecord | null>
}

function hasObjectField(value: unknown, key: string): boolean {
  const field = (value as Record<string, unknown> | undefined)?.[key]
  return field !== null && typeof field === "object"
}

function commonExecutionOptions(options: AgentRunnerExecutionOptions) {
  return {
    ...(options.account === undefined ? {} : { account: options.account }),
    ...(options.agentToken === undefined ? {} : { agentToken: options.agentToken }),
    ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
    ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
  }
}

export const AGENT_RUNNER_REGISTRY: ReadonlyArray<AgentRunnerDescriptor> = [
  {
    kind: "claude_agent",
    adapterKind: "claude_code",
    accountProvider: "claude_agent",
    serviceRef: "claude",
    capabilityRef: CLAUDE_AGENT_CAPABILITY_REF,
    canRunAssignment: (codingAssignment) => claudeAgentTaskFrom(codingAssignment) !== null,
    execute: (state, lease, now, options) =>
      executeClaudeAgentAssignment(state, lease, now, {
        ...commonExecutionOptions(options),
        ...(options.claudeAgentCheckoutRunner === undefined
          ? {}
          : { checkoutRunner: options.claudeAgentCheckoutRunner }),
        ...(options.claudeAgentProbe === undefined ? {} : { claudeAgentProbe: options.claudeAgentProbe }),
        ...(options.claudeAgentRunner === undefined ? {} : { claudeAgentRunner: options.claudeAgentRunner }),
      } satisfies ClaudeAgentExecutionOptions),
  },
  {
    kind: "codex",
    adapterKind: "codex",
    accountProvider: "codex",
    serviceRef: "codex",
    capabilityRef: CODEX_AGENT_CAPABILITY_REF,
    canRunAssignment: (codingAssignment) => codexAgentTaskFrom(codingAssignment) !== null,
    execute: (state, lease, now, options) =>
      executeCodexAgentAssignment(state, lease, now, {
        ...commonExecutionOptions(options),
        ...(options.codexAgentProbe === undefined ? {} : { codexAgentProbe: options.codexAgentProbe }),
        ...(options.codexAgentRunner === undefined ? {} : { codexAgentRunner: options.codexAgentRunner }),
      } satisfies CodexAgentExecutionOptions),
  },
]

export function agentRunnerForAdapterKind(
  adapterKind: AgentRuntimeAdapterKind,
): AgentRunnerDescriptor | null {
  return AGENT_RUNNER_REGISTRY.find((runner) => runner.adapterKind === adapterKind) ?? null
}

export function agentRunnerForLease(lease: PylonAssignmentLease): AgentRunnerDescriptor | null {
  return AGENT_RUNNER_REGISTRY.find((runner) => runner.canRunAssignment(lease.codingAssignment)) ?? null
}

export function agentRunnerServiceForLease(lease: PylonAssignmentLease): AgentRunnerServiceRef | null {
  const runner = agentRunnerForLease(lease)
  if (runner !== null) return runner.serviceRef

  const codingAssignment = lease.codingAssignment
  const legacyRunner = AGENT_RUNNER_REGISTRY.find((candidate) => {
    if (lease.capabilityRefs.includes(candidate.capabilityRef)) return true
    if (candidate.kind === "codex") return hasObjectField(codingAssignment, "codex")
    if (candidate.kind === "claude_agent") return hasObjectField(codingAssignment, "claudeAgent")
    return false
  })
  return legacyRunner?.serviceRef ?? null
}

export async function executeRegisteredAgentRunner(
  state: PylonLocalState,
  lease: PylonAssignmentLease,
  now: Date,
  options: AgentRunnerExecutionOptions,
): Promise<AgentRunnerCloseoutRecord | null> {
  const runner = agentRunnerForLease(lease)
  if (runner === null) return null
  return runner.execute(state, lease, now, options)
}
