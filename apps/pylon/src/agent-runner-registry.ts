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
  type CodexAgentRuntimeProgress,
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
  onCodexProgress?: (progress: CodexAgentRuntimeProgress) => void | Promise<void>
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

export type AgentRunnerResolution =
  | { status: "matched"; runner: AgentRunnerDescriptor }
  | { status: "none" }
  | {
      status: "ambiguous"
      runnerKinds: AgentRunnerKind[]
      blockerRef: "blocker.assignment.agent_runner_ambiguous"
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
        ...(options.onCodexProgress === undefined ? {} : { onProgress: options.onCodexProgress }),
      } satisfies CodexAgentExecutionOptions),
  },
]

export function agentRunnerForAdapterKind(
  adapterKind: AgentRuntimeAdapterKind,
): AgentRunnerDescriptor | null {
  return AGENT_RUNNER_REGISTRY.find((runner) => runner.adapterKind === adapterKind) ?? null
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
  const resolution = agentRunnerResolutionForLease(lease)
  if (resolution.status !== "matched") return null
  return resolution.runner.execute(state, lease, now, options)
}
