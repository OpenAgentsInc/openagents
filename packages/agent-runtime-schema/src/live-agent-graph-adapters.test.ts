import { describe, expect, test } from "bun:test"

import {
  adaptClaudeLiveAgentObservation,
  adaptCodexLiveAgentObservation,
  type ClaudeLiveAgentObservation,
  type CodexLiveAgentObservation,
} from "./live-agent-graph-adapters.js"
import {
  LiveAgentGraphSchemaLiteral,
  validateLiveAgentGraphSnapshot,
} from "./live-agent-graph.js"

const createdAt = "2026-07-11T19:00:00.000Z"
const updatedAt = "2026-07-11T19:01:00.000Z"

const codex = (
  status: CodexLiveAgentObservation["agent"]["status"] = "inProgress",
): CodexLiveAgentObservation => ({
  schema: "openagents.codex_live_agent_observation.v1",
  graphRef: "graph.cross-provider.1",
  sessionRef: "session.cross-provider.1",
  threadRef: "thread.canonical.1",
  provider: { state: "known", providerRef: "provider.codex.owner" },
  runtimeRef: "runtime.codex.owner",
  attachmentGeneration: 1,
  agent: {
    threadId: "child-1",
    runId: "run-1",
    parent: { state: "known", threadId: "root-1" },
    worktree: { state: "known", worktreeRef: "worktree.repo.feature" },
    status,
    attention: status === "waitingForInput"
      ? { state: "approval", attentionRef: "approval.1", since: updatedAt }
      : { state: "none" },
    currentTool: {
      state: "known",
      toolCallId: "call-1",
      toolName: "Read",
      status: "inProgress",
      version: 2,
    },
    activityCursor: 7,
    createdAt,
    updatedAt,
    startedAt: createdAt,
    endedAt: ["completed", "failed", "cancelled", "interrupted"].includes(status) ? updatedAt : null,
    version: 3,
  },
})

const claude = (
  status: ClaudeLiveAgentObservation["agent"]["status"] = "running",
): ClaudeLiveAgentObservation => ({
  schema: "openagents.claude_live_agent_observation.v1",
  graphRef: "graph.cross-provider.1",
  sessionRef: "session.cross-provider.1",
  threadRef: "thread.canonical.1",
  provider: { state: "known", providerRef: "provider.claude.owner" },
  runtimeRef: "runtime.claude.owner",
  attachmentGeneration: 1,
  agent: {
    threadId: "child-1",
    runId: "run-1",
    parent: { state: "known", threadId: "root-1" },
    worktree: { state: "known", worktreeRef: "worktree.repo.feature" },
    status,
    attention: status === "waiting_for_permission"
      ? { state: "approval", attentionRef: "approval.1", since: updatedAt }
      : { state: "none" },
    currentTool: {
      state: "known",
      toolUseId: "call-1",
      toolName: "Read",
      status: "running",
      version: 2,
    },
    activityCursor: 7,
    createdAt,
    updatedAt,
    startedAt: createdAt,
    endedAt: ["succeeded", "errored", "cancelled", "interrupted"].includes(status) ? updatedAt : null,
    version: 3,
  },
})

const semantics = (value: ReturnType<typeof adaptCodexLiveAgentObservation>) => ({
  graphRef: value.graphRef,
  threadRef: value.threadRef,
  sessionRef: value.node.sessionRef,
  parentKind: value.node.parent.kind,
  worktree: value.node.worktree,
  status: value.node.status,
  attention: value.node.attention,
  terminal: value.node.terminal,
  currentTool: value.node.currentTool.state === "known"
    ? { state: "known", toolName: value.node.currentTool.toolName, status: value.node.currentTool.status }
    : value.node.currentTool,
  attachmentGeneration: value.node.attachmentGeneration,
  activityCursor: value.node.activityCursor,
  createdAt: value.node.createdAt,
  updatedAt: value.node.updatedAt,
  startedAt: value.node.startedAt,
  endedAt: value.node.endedAt,
  version: value.node.version,
  edgeKinds: value.edges.map(edge => edge.kind),
  edgeStatuses: value.edges.flatMap(edge => edge.kind === "tool" ? [edge.status] : []),
})

describe("canonical live-agent provider adapters", () => {
  test("Codex and Claude observations converge to equivalent graph semantics", () => {
    expect(semantics(adaptCodexLiveAgentObservation(codex()))).toEqual(
      semantics(adaptClaudeLiveAgentObservation(claude())),
    )
    expect(semantics(adaptCodexLiveAgentObservation(codex("waitingForInput")))).toEqual(
      semantics(adaptClaudeLiveAgentObservation(claude("waiting_for_permission"))),
    )
    expect(semantics(adaptCodexLiveAgentObservation(codex("completed")))).toEqual(
      semantics(adaptClaudeLiveAgentObservation(claude("succeeded"))),
    )
  })

  test("each provider emits a graph-valid root, child, parent edge, and tool edge", () => {
    const inputs = [
      {
        adapt: adaptCodexLiveAgentObservation,
        child: codex(),
        root: {
          ...codex(),
          agent: { ...codex().agent, threadId: "root-1", parent: { state: "root" as const } },
        },
      },
      {
        adapt: adaptClaudeLiveAgentObservation,
        child: claude(),
        root: {
          ...claude(),
          agent: { ...claude().agent, threadId: "root-1", parent: { state: "root" as const } },
        },
      },
    ] as const
    for (const input of inputs) {
      const root = input.adapt(input.root)
      const child = input.adapt(input.child)
      const value = validateLiveAgentGraphSnapshot({
        schema: LiveAgentGraphSchemaLiteral,
        graphRef: child.graphRef,
        sessionRef: child.node.sessionRef,
        threadRef: child.threadRef,
        attachmentGeneration: 1,
        cursor: 1,
        lastDeltaRef: "delta.provider.1",
        nodes: [root.node, child.node],
        edges: [...root.edges, ...child.edges],
        updatedAt,
      })
      expect(value.nodes).toHaveLength(2)
      expect(value.edges.map(edge => edge.kind).sort()).toEqual(["parent", "tool", "tool"])
    }
  })

  test("maps every provider status into the canonical closed status set", () => {
    const codexCases = [
      ["notStarted", "queued"],
      ["inProgress", "running"],
      ["waitingForInput", "waiting_for_input"],
      ["completed", "completed"],
      ["failed", "failed"],
      ["cancelled", "canceled"],
      ["interrupted", "interrupted"],
      ["unknown", "unknown"],
    ] as const
    const claudeCases = [
      ["queued", "queued"],
      ["running", "running"],
      ["waiting_for_permission", "waiting_for_input"],
      ["succeeded", "completed"],
      ["errored", "failed"],
      ["cancelled", "canceled"],
      ["interrupted", "interrupted"],
      ["unknown", "unknown"],
    ] as const
    for (const [provider, canonical] of codexCases) {
      expect(adaptCodexLiveAgentObservation(codex(provider)).node.status).toBe(canonical)
    }
    for (const [provider, canonical] of claudeCases) {
      expect(adaptClaudeLiveAgentObservation(claude(provider)).node.status).toBe(canonical)
    }
  })

  test("loss-accounts omitted facts instead of fabricating provider parity", () => {
    const codexInput = codex()
    const claudeInput = claude()
    const codexAdapted = adaptCodexLiveAgentObservation({
      ...codexInput,
      provider: { state: "omitted", reason: "provider_omitted" },
      agent: {
        ...codexInput.agent,
        parent: { state: "omitted", reason: "provider_unsupported" },
        worktree: { state: "omitted", reason: "not_authorized" },
        attention: { state: "omitted", reason: "not_observed" },
        currentTool: { state: "omitted", reason: "provider_omitted" },
      },
    })
    const claudeAdapted = adaptClaudeLiveAgentObservation({
      ...claudeInput,
      provider: { state: "omitted", reason: "provider_omitted" },
      agent: {
        ...claudeInput.agent,
        parent: { state: "omitted", reason: "provider_unsupported" },
        worktree: { state: "omitted", reason: "not_authorized" },
        attention: { state: "omitted", reason: "not_observed" },
        currentTool: { state: "omitted", reason: "provider_omitted" },
      },
    })
    for (const adapted of [codexAdapted, claudeAdapted]) {
      expect(adapted.node.parent).toEqual({ kind: "unknown", reason: "provider_unsupported" })
      expect(adapted.node.provider).toEqual({ state: "unknown", reason: "provider_omitted" })
      expect(adapted.node.worktree).toEqual({ state: "unknown", reason: "not_authorized" })
      expect(adapted.node.attention).toEqual({ state: "unknown", reason: "not_observed" })
      expect(adapted.node.currentTool).toEqual({ state: "unknown", reason: "provider_omitted" })
      expect(adapted.edges).toEqual([])
    }
  })

  test("rejects invalid provider IDs and terminal observations without an end time", () => {
    expect(() => adaptCodexLiveAgentObservation({
      ...codex(),
      agent: { ...codex().agent, threadId: "../private/thread" },
    })).toThrow()
    expect(() => adaptClaudeLiveAgentObservation({
      ...claude("succeeded"),
      agent: { ...claude("succeeded").agent, endedAt: null },
    })).toThrow("omitted endedAt")
    expect(() => adaptCodexLiveAgentObservation({
      ...codex(),
      agent: { ...codex().agent, endedAt: updatedAt },
    })).toThrow("supplied endedAt")
  })
})
