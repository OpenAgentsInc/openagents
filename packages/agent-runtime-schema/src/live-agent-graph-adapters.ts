import { Schema as S } from "effect"

import {
  LiveAgentGraphEdge,
  LiveAgentGraphNode,
  type LiveAgentGraphAttention,
  type LiveAgentGraphCurrentTool,
  type LiveAgentGraphNode as LiveAgentGraphNodeType,
  type LiveAgentGraphParent,
  type LiveAgentGraphStatus,
  type LiveAgentGraphTerminal,
  type LiveAgentGraphWorktree,
} from "./live-agent-graph.js"

const ProviderId = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(100),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const Timestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/),
)
const PositiveInt = S.Number.check(S.isInt(), S.isGreaterThan(0))
const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))
const OmissionReason = S.Literals([
  "provider_unsupported",
  "provider_omitted",
  "not_observed",
  "not_authorized",
  "not_applicable",
])

const ParentObservation = S.Union([
  S.Struct({ state: S.Literal("root") }),
  S.Struct({ state: S.Literal("known"), threadId: ProviderId }),
  S.Struct({ state: S.Literal("omitted"), reason: OmissionReason }),
])

const WorktreeObservation = S.Union([
  S.Struct({ state: S.Literal("known"), worktreeRef: Ref }),
  S.Struct({ state: S.Literal("omitted"), reason: OmissionReason }),
])

const AttentionObservation = S.Union([
  S.Struct({ state: S.Literal("none") }),
  S.Struct({ state: S.Literals(["question", "approval", "blocked", "stale"]), attentionRef: Ref, since: Timestamp }),
  S.Struct({ state: S.Literal("omitted"), reason: OmissionReason }),
])

const CommonObservation = {
  graphRef: Ref,
  sessionRef: Ref,
  providerRef: Ref,
  attachmentGeneration: PositiveInt,
  agent: S.Struct({
    threadId: ProviderId,
    runId: ProviderId,
    parent: ParentObservation,
    worktree: WorktreeObservation,
    attention: AttentionObservation,
    activityCursor: NonNegativeInt,
    createdAt: Timestamp,
    updatedAt: Timestamp,
    startedAt: S.NullOr(Timestamp),
    endedAt: S.NullOr(Timestamp),
    version: PositiveInt,
  }),
} as const

const CodexToolObservation = S.Union([
  S.Struct({ state: S.Literal("none") }),
  S.Struct({
    state: S.Literal("known"),
    toolCallId: ProviderId,
    toolName: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
    status: S.Literals(["pending", "inProgress", "completed", "failed"]),
    version: PositiveInt,
  }),
  S.Struct({ state: S.Literal("omitted"), reason: OmissionReason }),
])

export const CodexLiveAgentObservation = S.Struct({
  schema: S.Literal("openagents.codex_live_agent_observation.v1"),
  ...CommonObservation,
  runtimeRef: Ref,
  agent: S.Struct({
    ...CommonObservation.agent.fields,
    status: S.Literals([
      "notStarted",
      "inProgress",
      "waitingForInput",
      "completed",
      "failed",
      "cancelled",
      "interrupted",
      "unknown",
    ]),
    currentTool: CodexToolObservation,
  }),
})
export type CodexLiveAgentObservation = typeof CodexLiveAgentObservation.Type

const ClaudeToolObservation = S.Union([
  S.Struct({ state: S.Literal("none") }),
  S.Struct({
    state: S.Literal("known"),
    toolUseId: ProviderId,
    toolName: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
    status: S.Literals(["queued", "running", "succeeded", "errored"]),
    version: PositiveInt,
  }),
  S.Struct({ state: S.Literal("omitted"), reason: OmissionReason }),
])

export const ClaudeLiveAgentObservation = S.Struct({
  schema: S.Literal("openagents.claude_live_agent_observation.v1"),
  ...CommonObservation,
  runtimeRef: Ref,
  agent: S.Struct({
    ...CommonObservation.agent.fields,
    status: S.Literals([
      "queued",
      "running",
      "waiting_for_permission",
      "succeeded",
      "errored",
      "cancelled",
      "interrupted",
      "unknown",
    ]),
    currentTool: ClaudeToolObservation,
  }),
})
export type ClaudeLiveAgentObservation = typeof ClaudeLiveAgentObservation.Type

export type AdaptedLiveAgentObservation = Readonly<{
  graphRef: string
  node: LiveAgentGraphNodeType
  edges: ReadonlyArray<typeof LiveAgentGraphEdge.Type>
}>

const unknown = (reason: typeof OmissionReason.Type) => ({ state: "unknown" as const, reason })
const agentRef = (provider: "codex" | "claude", threadId: string): string =>
  `agent.${provider}.${threadId}`

const parentFor = (
  provider: "codex" | "claude",
  parent: typeof ParentObservation.Type,
): LiveAgentGraphParent => parent.state === "root"
  ? { kind: "root" }
  : parent.state === "known"
    ? { kind: "agent", agentRef: agentRef(provider, parent.threadId) }
    : { kind: "unknown", reason: parent.reason }

const worktreeFor = (worktree: typeof WorktreeObservation.Type): LiveAgentGraphWorktree =>
  worktree.state === "known" ? worktree : unknown(worktree.reason)

const attentionFor = (attention: typeof AttentionObservation.Type): LiveAgentGraphAttention =>
  attention.state === "omitted" ? unknown(attention.reason) : attention

const terminalFor = (
  status: LiveAgentGraphStatus,
  endedAt: string | null,
): LiveAgentGraphTerminal => {
  if (status === "completed" || status === "failed" || status === "canceled" || status === "interrupted") {
    if (endedAt === null) throw new Error(`terminal provider status ${status} omitted endedAt`)
    return { state: "terminal", reason: status, at: endedAt }
  }
  if (endedAt !== null) throw new Error(`non-terminal provider status ${status} supplied endedAt`)
  if (status === "unknown") return unknown("not_observed")
  return { state: "active" }
}

const parentEdgeFor = (
  provider: "codex" | "claude",
  threadId: string,
  parent: typeof ParentObservation.Type,
): typeof LiveAgentGraphEdge.Type | null => parent.state !== "known" ? null : {
  edgeRef: `edge.parent.${provider}.${parent.threadId}.${threadId}`,
  kind: "parent",
  fromAgentRef: agentRef(provider, parent.threadId),
  toAgentRef: agentRef(provider, threadId),
  version: 1,
}

const canonicalNode = (input: Readonly<{
  provider: "codex" | "claude"
  providerRef: string
  runtimeKind: "codex_app_server" | "claude_agent_sdk"
  runtimeRef: string
  sessionRef: string
  attachmentGeneration: number
  agent: typeof CommonObservation.agent.Type
  status: LiveAgentGraphStatus
  currentTool: LiveAgentGraphCurrentTool
}>): LiveAgentGraphNodeType => {
  const ref = agentRef(input.provider, input.agent.threadId)
  return S.decodeUnknownSync(LiveAgentGraphNode)({
    agentRef: ref,
    sessionRef: input.sessionRef,
    threadRef: `thread.${input.provider}.${input.agent.threadId}`,
    transcriptRef: `transcript.${input.provider}.${input.agent.threadId}`,
    runRef: `run.${input.provider}.${input.agent.runId}`,
    parent: parentFor(input.provider, input.agent.parent),
    provider: { state: "known", kind: input.provider, providerRef: input.providerRef },
    runtime: { state: "known", kind: input.runtimeKind, runtimeRef: input.runtimeRef },
    worktree: worktreeFor(input.agent.worktree),
    status: input.status,
    attention: attentionFor(input.agent.attention),
    terminal: terminalFor(input.status, input.agent.endedAt),
    currentTool: input.currentTool,
    attachmentGeneration: input.attachmentGeneration,
    activityCursor: input.agent.activityCursor,
    createdAt: input.agent.createdAt,
    updatedAt: input.agent.updatedAt,
    startedAt: input.agent.startedAt,
    endedAt: input.agent.endedAt,
    version: input.agent.version,
  })
}

const CODEX_STATUS: Record<CodexLiveAgentObservation["agent"]["status"], LiveAgentGraphStatus> = {
  notStarted: "queued",
  inProgress: "running",
  waitingForInput: "waiting_for_input",
  completed: "completed",
  failed: "failed",
  cancelled: "canceled",
  interrupted: "interrupted",
  unknown: "unknown",
}
const codexStatus = (status: CodexLiveAgentObservation["agent"]["status"]): LiveAgentGraphStatus =>
  CODEX_STATUS[status]

const CLAUDE_STATUS: Record<ClaudeLiveAgentObservation["agent"]["status"], LiveAgentGraphStatus> = {
  queued: "queued",
  running: "running",
  waiting_for_permission: "waiting_for_input",
  succeeded: "completed",
  errored: "failed",
  cancelled: "canceled",
  interrupted: "interrupted",
  unknown: "unknown",
}
const claudeStatus = (status: ClaudeLiveAgentObservation["agent"]["status"]): LiveAgentGraphStatus =>
  CLAUDE_STATUS[status]

const codexTool = (
  threadId: string,
  tool: CodexLiveAgentObservation["agent"]["currentTool"],
): Readonly<{ current: LiveAgentGraphCurrentTool; edge: typeof LiveAgentGraphEdge.Type | null }> => {
  if (tool.state === "none") return { current: tool, edge: null }
  if (tool.state === "omitted") return { current: unknown(tool.reason), edge: null }
  const toolCallRef = `tool.codex.${tool.toolCallId}`
  const status = tool.status === "pending" ? "called" : tool.status === "inProgress" ? "running" : tool.status
  return {
    current: { state: "known", toolCallRef, toolName: tool.toolName, status },
    edge: {
      edgeRef: `edge.tool.codex.${threadId}.${tool.toolCallId}`,
      kind: "tool",
      agentRef: agentRef("codex", threadId),
      toolCallRef,
      status,
      version: tool.version,
    },
  }
}

const claudeTool = (
  threadId: string,
  tool: ClaudeLiveAgentObservation["agent"]["currentTool"],
): Readonly<{ current: LiveAgentGraphCurrentTool; edge: typeof LiveAgentGraphEdge.Type | null }> => {
  if (tool.state === "none") return { current: tool, edge: null }
  if (tool.state === "omitted") return { current: unknown(tool.reason), edge: null }
  const toolCallRef = `tool.claude.${tool.toolUseId}`
  const status = tool.status === "queued" ? "called" : tool.status === "succeeded" ? "completed" : tool.status === "errored" ? "failed" : "running"
  return {
    current: { state: "known", toolCallRef, toolName: tool.toolName, status },
    edge: {
      edgeRef: `edge.tool.claude.${threadId}.${tool.toolUseId}`,
      kind: "tool",
      agentRef: agentRef("claude", threadId),
      toolCallRef,
      status,
      version: tool.version,
    },
  }
}

export const adaptCodexLiveAgentObservation = (value: unknown): AdaptedLiveAgentObservation => {
  const input = S.decodeUnknownSync(CodexLiveAgentObservation)(value)
  const tool = codexTool(input.agent.threadId, input.agent.currentTool)
  const parentEdge = parentEdgeFor("codex", input.agent.threadId, input.agent.parent)
  return {
    graphRef: input.graphRef,
    node: canonicalNode({
      provider: "codex",
      providerRef: input.providerRef,
      runtimeKind: "codex_app_server",
      runtimeRef: input.runtimeRef,
      sessionRef: input.sessionRef,
      attachmentGeneration: input.attachmentGeneration,
      agent: input.agent,
      status: codexStatus(input.agent.status),
      currentTool: tool.current,
    }),
    edges: [parentEdge, tool.edge].filter((edge): edge is typeof LiveAgentGraphEdge.Type => edge !== null),
  }
}

export const adaptClaudeLiveAgentObservation = (value: unknown): AdaptedLiveAgentObservation => {
  const input = S.decodeUnknownSync(ClaudeLiveAgentObservation)(value)
  const tool = claudeTool(input.agent.threadId, input.agent.currentTool)
  const parentEdge = parentEdgeFor("claude", input.agent.threadId, input.agent.parent)
  return {
    graphRef: input.graphRef,
    node: canonicalNode({
      provider: "claude",
      providerRef: input.providerRef,
      runtimeKind: "claude_agent_sdk",
      runtimeRef: input.runtimeRef,
      sessionRef: input.sessionRef,
      attachmentGeneration: input.attachmentGeneration,
      agent: input.agent,
      status: claudeStatus(input.agent.status),
      currentTool: tool.current,
    }),
    edges: [parentEdge, tool.edge].filter((edge): edge is typeof LiveAgentGraphEdge.Type => edge !== null),
  }
}
