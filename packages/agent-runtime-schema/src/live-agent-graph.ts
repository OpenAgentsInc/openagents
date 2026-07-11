import { Schema as S } from "effect"

export const LiveAgentGraphSchemaLiteral = "openagents.live_agent_graph.v1" as const

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const Timestamp = S.String.check(S.isMinLength(1), S.isMaxLength(64))
const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))
const PositiveInt = S.Number.check(S.isInt(), S.isGreaterThan(0))
const UnknownReason = S.Literals([
  "provider_unsupported",
  "provider_omitted",
  "not_observed",
  "not_authorized",
  "not_applicable",
  "replay_gap",
])

export const LiveAgentGraphParent = S.Union([
  S.Struct({ kind: S.Literal("root") }),
  S.Struct({ kind: S.Literal("agent"), agentRef: Ref }),
  S.Struct({ kind: S.Literal("unknown"), reason: UnknownReason }),
])
export type LiveAgentGraphParent = typeof LiveAgentGraphParent.Type

export const LiveAgentGraphProvider = S.Union([
  S.Struct({
    state: S.Literal("known"),
    kind: S.Literals(["codex", "claude", "openagents_native"]),
    providerRef: Ref,
  }),
  S.Struct({ state: S.Literal("unknown"), reason: UnknownReason }),
])
export type LiveAgentGraphProvider = typeof LiveAgentGraphProvider.Type

export const LiveAgentGraphRuntime = S.Union([
  S.Struct({
    state: S.Literal("known"),
    kind: S.Literals([
      "codex_app_server",
      "claude_agent_sdk",
      "claude_pylon",
      "openagents_native",
    ]),
    runtimeRef: Ref,
  }),
  S.Struct({ state: S.Literal("unknown"), reason: UnknownReason }),
])
export type LiveAgentGraphRuntime = typeof LiveAgentGraphRuntime.Type

export const LiveAgentGraphWorktree = S.Union([
  S.Struct({ state: S.Literal("known"), worktreeRef: Ref }),
  S.Struct({ state: S.Literal("unknown"), reason: UnknownReason }),
])
export type LiveAgentGraphWorktree = typeof LiveAgentGraphWorktree.Type

export const LiveAgentGraphStatus = S.Literals([
  "queued",
  "running",
  "waiting_for_input",
  "completed",
  "failed",
  "canceled",
  "interrupted",
  "unknown",
])
export type LiveAgentGraphStatus = typeof LiveAgentGraphStatus.Type

export const LiveAgentGraphAttention = S.Union([
  S.Struct({ state: S.Literal("none") }),
  S.Struct({
    state: S.Literals(["question", "approval", "blocked", "stale"]),
    attentionRef: Ref,
    since: Timestamp,
  }),
  S.Struct({ state: S.Literal("unknown"), reason: UnknownReason }),
])
export type LiveAgentGraphAttention = typeof LiveAgentGraphAttention.Type

export const LiveAgentGraphTerminal = S.Union([
  S.Struct({ state: S.Literal("active") }),
  S.Struct({
    state: S.Literal("terminal"),
    reason: S.Literals([
      "completed",
      "failed",
      "canceled",
      "interrupted",
      "provider_lost",
      "revoked",
      "unknown",
    ]),
    at: Timestamp,
    reasonRef: S.optionalKey(Ref),
  }),
  S.Struct({ state: S.Literal("unknown"), reason: UnknownReason }),
])
export type LiveAgentGraphTerminal = typeof LiveAgentGraphTerminal.Type

export const LiveAgentGraphCurrentTool = S.Union([
  S.Struct({ state: S.Literal("none") }),
  S.Struct({
    state: S.Literal("known"),
    toolCallRef: Ref,
    toolName: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
    status: S.Literals(["called", "running", "completed", "failed"]),
  }),
  S.Struct({ state: S.Literal("unknown"), reason: UnknownReason }),
])
export type LiveAgentGraphCurrentTool = typeof LiveAgentGraphCurrentTool.Type

export const LiveAgentGraphNode = S.Struct({
  agentRef: Ref,
  sessionRef: Ref,
  threadRef: Ref,
  transcriptRef: Ref,
  runRef: Ref,
  parent: LiveAgentGraphParent,
  provider: LiveAgentGraphProvider,
  runtime: LiveAgentGraphRuntime,
  worktree: LiveAgentGraphWorktree,
  status: LiveAgentGraphStatus,
  statusReasonRef: S.optionalKey(Ref),
  attention: LiveAgentGraphAttention,
  terminal: LiveAgentGraphTerminal,
  currentTool: LiveAgentGraphCurrentTool,
  attachmentGeneration: PositiveInt,
  activityCursor: NonNegativeInt,
  createdAt: Timestamp,
  updatedAt: Timestamp,
  startedAt: S.NullOr(Timestamp),
  endedAt: S.NullOr(Timestamp),
  version: PositiveInt,
})
export type LiveAgentGraphNode = typeof LiveAgentGraphNode.Type

export const LiveAgentGraphEdge = S.Union([
  S.Struct({
    edgeRef: Ref,
    kind: S.Literal("parent"),
    fromAgentRef: Ref,
    toAgentRef: Ref,
    version: PositiveInt,
  }),
  S.Struct({
    edgeRef: Ref,
    kind: S.Literal("tool"),
    agentRef: Ref,
    toolCallRef: Ref,
    status: S.Literals(["called", "running", "completed", "failed", "unknown"]),
    version: PositiveInt,
  }),
])
export type LiveAgentGraphEdge = typeof LiveAgentGraphEdge.Type

export const LiveAgentGraphSnapshot = S.Struct({
  schema: S.Literal(LiveAgentGraphSchemaLiteral),
  graphRef: Ref,
  sessionRef: Ref,
  attachmentGeneration: PositiveInt,
  cursor: NonNegativeInt,
  lastDeltaRef: S.NullOr(Ref),
  nodes: S.Array(LiveAgentGraphNode).check(S.isMaxLength(2_000)),
  edges: S.Array(LiveAgentGraphEdge).check(S.isMaxLength(4_000)),
  updatedAt: Timestamp,
})
export type LiveAgentGraphSnapshot = typeof LiveAgentGraphSnapshot.Type

export const LiveAgentGraphDelta = S.Struct({
  schema: S.Literal(LiveAgentGraphSchemaLiteral),
  deltaRef: Ref,
  graphRef: Ref,
  sessionRef: Ref,
  attachmentGeneration: PositiveInt,
  previousCursor: NonNegativeInt,
  cursor: PositiveInt,
  upsertNodes: S.Array(LiveAgentGraphNode).check(S.isMaxLength(2_000)),
  removeAgentRefs: S.Array(Ref).check(S.isMaxLength(2_000)),
  upsertEdges: S.Array(LiveAgentGraphEdge).check(S.isMaxLength(4_000)),
  removeEdgeRefs: S.Array(Ref).check(S.isMaxLength(4_000)),
  committedAt: Timestamp,
})
export type LiveAgentGraphDelta = typeof LiveAgentGraphDelta.Type

export type LiveAgentGraphErrorReason =
  | "duplicate_ref"
  | "graph_mismatch"
  | "generation_mismatch"
  | "cursor_gap"
  | "stale_delta"
  | "identity_conflict"
  | "stale_node"
  | "stale_edge"
  | "terminal_reopened"
  | "cursor_regression"
  | "timestamp_regression"
  | "missing_parent"
  | "parent_edge_mismatch"
  | "orphan_edge"
  | "cycle"
  | "terminal_mismatch"

export class LiveAgentGraphError extends Error {
  readonly _tag = "LiveAgentGraphError"
  constructor(
    readonly reason: LiveAgentGraphErrorReason,
    message: string,
  ) {
    super(message)
  }
}

export const decodeLiveAgentGraphSnapshot = S.decodeUnknownSync(LiveAgentGraphSnapshot)
export const decodeLiveAgentGraphDelta = S.decodeUnknownSync(LiveAgentGraphDelta)

const terminalStatuses = new Set<LiveAgentGraphStatus>([
  "completed",
  "failed",
  "canceled",
  "interrupted",
])

const duplicate = (values: ReadonlyArray<string>): string | null => {
  const seen = new Set<string>()
  for (const value of values) {
    if (seen.has(value)) return value
    seen.add(value)
  }
  return null
}

const fail = (reason: LiveAgentGraphErrorReason, message: string): never => {
  throw new LiveAgentGraphError(reason, message)
}

const validateNodeTerminal = (node: LiveAgentGraphNode): void => {
  const terminal = terminalStatuses.has(node.status)
  if (terminal && node.terminal.state !== "terminal") {
    fail("terminal_mismatch", `terminal agent ${node.agentRef} lacks terminal reason`)
  }
  if (!terminal && node.status !== "unknown" && node.terminal.state === "terminal") {
    fail("terminal_mismatch", `active agent ${node.agentRef} carries terminal reason`)
  }
  if (terminal !== (node.endedAt !== null)) {
    fail("terminal_mismatch", `agent ${node.agentRef} endedAt disagrees with status`)
  }
  if (terminal && node.terminal.state === "terminal") {
    const allowed = node.status === "completed"
      ? ["completed"]
      : node.status === "failed"
        ? ["failed", "provider_lost", "revoked", "unknown"]
        : node.status === "canceled"
          ? ["canceled", "revoked"]
          : ["interrupted", "provider_lost", "revoked"]
    if (!allowed.includes(node.terminal.reason)) {
      fail("terminal_mismatch", `agent ${node.agentRef} terminal reason disagrees with status`)
    }
  }
}

export const validateLiveAgentGraphSnapshot = (
  input: LiveAgentGraphSnapshot,
): LiveAgentGraphSnapshot => {
  const snapshot = decodeLiveAgentGraphSnapshot(input)
  const duplicateAgent = duplicate(snapshot.nodes.map(node => node.agentRef))
  if (duplicateAgent !== null) fail("duplicate_ref", `duplicate agentRef ${duplicateAgent}`)
  const duplicateEdge = duplicate(snapshot.edges.map(edge => edge.edgeRef))
  if (duplicateEdge !== null) fail("duplicate_ref", `duplicate edgeRef ${duplicateEdge}`)
  const nodes = new Map(snapshot.nodes.map(node => [node.agentRef, node]))
  const parentEdges = new Map<string, Extract<LiveAgentGraphEdge, { kind: "parent" }>>()

  for (const node of snapshot.nodes) {
    if (
      node.sessionRef !== snapshot.sessionRef ||
      node.attachmentGeneration !== snapshot.attachmentGeneration
    ) fail("graph_mismatch", `agent ${node.agentRef} escaped graph authority`)
    validateNodeTerminal(node)
    if (node.parent.kind === "agent" && !nodes.has(node.parent.agentRef)) {
      fail("missing_parent", `agent ${node.agentRef} references missing parent`)
    }
  }
  for (const edge of snapshot.edges) {
    if (edge.kind === "parent") {
      if (!nodes.has(edge.fromAgentRef) || !nodes.has(edge.toAgentRef)) {
        fail("orphan_edge", `parent edge ${edge.edgeRef} references missing agent`)
      }
      if (parentEdges.has(edge.toAgentRef)) {
        fail("parent_edge_mismatch", `agent ${edge.toAgentRef} has multiple parent edges`)
      }
      parentEdges.set(edge.toAgentRef, edge)
    } else if (!nodes.has(edge.agentRef)) {
      fail("orphan_edge", `tool edge ${edge.edgeRef} references missing agent`)
    }
  }
  for (const node of snapshot.nodes) {
    const edge = parentEdges.get(node.agentRef)
    if (node.parent.kind === "agent") {
      if (edge?.fromAgentRef !== node.parent.agentRef) {
        fail("parent_edge_mismatch", `agent ${node.agentRef} parent fact and edge disagree`)
      }
    } else if (edge !== undefined) {
      fail("parent_edge_mismatch", `root/unknown agent ${node.agentRef} has parent edge`)
    }
  }

  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (agentRef: string): void => {
    if (visited.has(agentRef)) return
    if (visiting.has(agentRef)) fail("cycle", `cycle at agent ${agentRef}`)
    visiting.add(agentRef)
    const parent = nodes.get(agentRef)?.parent
    if (parent?.kind === "agent") visit(parent.agentRef)
    visiting.delete(agentRef)
    visited.add(agentRef)
  }
  for (const agentRef of nodes.keys()) visit(agentRef)
  return snapshot
}

const sameKnownIdentity = (
  before: LiveAgentGraphNode,
  after: LiveAgentGraphNode,
): boolean => {
  if (
    before.sessionRef !== after.sessionRef ||
    before.threadRef !== after.threadRef ||
    before.transcriptRef !== after.transcriptRef ||
    before.runRef !== after.runRef
  ) return false
  if (
    before.parent.kind !== "unknown" &&
    JSON.stringify(before.parent) !== JSON.stringify(after.parent)
  ) return false
  if (
    before.provider.state === "known" &&
    JSON.stringify(before.provider) !== JSON.stringify(after.provider)
  ) return false
  if (
    before.runtime.state === "known" &&
    JSON.stringify(before.runtime) !== JSON.stringify(after.runtime)
  ) return false
  return !(
    before.worktree.state === "known" &&
    JSON.stringify(before.worktree) !== JSON.stringify(after.worktree)
  )
}

const validateNodeTransition = (
  before: LiveAgentGraphNode,
  after: LiveAgentGraphNode,
): void => {
  if (!sameKnownIdentity(before, after)) {
    fail("identity_conflict", `agent ${after.agentRef} changed stable identity`)
  }
  if (after.version <= before.version) fail("stale_node", `agent ${after.agentRef} version did not advance`)
  if (after.activityCursor < before.activityCursor) {
    fail("cursor_regression", `agent ${after.agentRef} activity cursor regressed`)
  }
  if (after.updatedAt < before.updatedAt) {
    fail("timestamp_regression", `agent ${after.agentRef} timestamp regressed`)
  }
  if (terminalStatuses.has(before.status) && after.status !== before.status) {
    fail("terminal_reopened", `agent ${after.agentRef} reopened terminal state`)
  }
}

export const applyLiveAgentGraphDelta = (
  current: LiveAgentGraphSnapshot,
  input: LiveAgentGraphDelta,
): LiveAgentGraphSnapshot => {
  const snapshot = validateLiveAgentGraphSnapshot(current)
  const delta = decodeLiveAgentGraphDelta(input)
  if (delta.graphRef !== snapshot.graphRef || delta.sessionRef !== snapshot.sessionRef) {
    fail("graph_mismatch", "delta targets another graph/session")
  }
  if (delta.attachmentGeneration !== snapshot.attachmentGeneration) {
    fail("generation_mismatch", "delta targets a stale/future attachment generation")
  }
  if (delta.cursor === snapshot.cursor && delta.deltaRef === snapshot.lastDeltaRef) return snapshot
  if (delta.cursor <= snapshot.cursor || delta.previousCursor < snapshot.cursor) {
    fail("stale_delta", "delta is stale or conflicts with the current cursor")
  }
  if (delta.previousCursor !== snapshot.cursor || delta.cursor !== snapshot.cursor + 1) {
    fail("cursor_gap", "delta does not continue the exact durable cursor")
  }
  if (delta.committedAt < snapshot.updatedAt) {
    fail("timestamp_regression", "delta committedAt regressed")
  }
  if (duplicate(delta.upsertNodes.map(node => node.agentRef)) !== null ||
      duplicate(delta.removeAgentRefs) !== null ||
      duplicate(delta.upsertEdges.map(edge => edge.edgeRef)) !== null ||
      duplicate(delta.removeEdgeRefs) !== null) {
    fail("duplicate_ref", "delta repeats an upsert/removal ref")
  }

  const nodes = new Map(snapshot.nodes.map(node => [node.agentRef, node]))
  const edges = new Map(snapshot.edges.map(edge => [edge.edgeRef, edge]))
  for (const agentRef of delta.removeAgentRefs) nodes.delete(agentRef)
  for (const edgeRef of delta.removeEdgeRefs) edges.delete(edgeRef)
  for (const node of delta.upsertNodes) {
    const before = nodes.get(node.agentRef)
    if (before !== undefined) validateNodeTransition(before, node)
    nodes.set(node.agentRef, node)
  }
  for (const edge of delta.upsertEdges) {
    const before = edges.get(edge.edgeRef)
    if (before !== undefined && edge.version <= before.version) {
      fail("stale_edge", `edge ${edge.edgeRef} version did not advance`)
    }
    if (before !== undefined) {
      const beforeIdentity = before.kind === "parent"
        ? [before.kind, before.fromAgentRef, before.toAgentRef]
        : [before.kind, before.agentRef, before.toolCallRef]
      const afterIdentity = edge.kind === "parent"
        ? [edge.kind, edge.fromAgentRef, edge.toAgentRef]
        : [edge.kind, edge.agentRef, edge.toolCallRef]
      if (JSON.stringify(beforeIdentity) !== JSON.stringify(afterIdentity)) {
        fail("identity_conflict", `edge ${edge.edgeRef} changed stable identity`)
      }
    }
    edges.set(edge.edgeRef, edge)
  }

  return validateLiveAgentGraphSnapshot({
    schema: LiveAgentGraphSchemaLiteral,
    graphRef: snapshot.graphRef,
    sessionRef: snapshot.sessionRef,
    attachmentGeneration: snapshot.attachmentGeneration,
    cursor: delta.cursor,
    lastDeltaRef: delta.deltaRef,
    nodes: [...nodes.values()].sort((left, right) => left.agentRef.localeCompare(right.agentRef)),
    edges: [...edges.values()].sort((left, right) => left.edgeRef.localeCompare(right.edgeRef)),
    updatedAt: delta.committedAt,
  })
}
