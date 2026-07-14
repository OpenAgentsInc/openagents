import { describe, expect, test } from "vite-plus/test"
import {
  decodeLiveAgentGraphEntity,
  type LiveAgentGraphEntity,
} from "@openagentsinc/khala-sync"

import {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
  resolveLiveAgentGraphSelection,
} from "./live-agent-graph-presentation.js"

const now = "2026-07-11T20:00:00.000Z"
type LiveAgentGraphNode = LiveAgentGraphEntity["nodes"][number]

const node = (
  agentRef: string,
  parent: LiveAgentGraphNode["parent"],
  overrides: Partial<LiveAgentGraphNode> = {},
): LiveAgentGraphNode => ({
  agentRef,
  sessionRef: "session.presentation.1",
  threadRef: `thread.${agentRef}`,
  transcriptRef: `transcript.${agentRef}`,
  runRef: `run.${agentRef}`,
  parent,
  provider: { state: "known", kind: "claude", providerRef: "provider.claude.named" },
  runtime: { state: "known", kind: "claude_agent_sdk", runtimeRef: "runtime.claude.named" },
  worktree: { state: "known", worktreeRef: "worktree.presentation.main" },
  status: "running",
  attention: { state: "none" },
  terminal: { state: "active" },
  currentTool: { state: "known", toolCallRef: `tool.${agentRef}`, toolName: "Search", status: "running" },
  attachmentGeneration: 1,
  activityCursor: 1,
  createdAt: "2026-07-11T19:58:30.000Z",
  updatedAt: now,
  startedAt: "2026-07-11T19:58:30.000Z",
  endedAt: null,
  version: 1,
  ...overrides,
})

const graph = (nodes: ReadonlyArray<LiveAgentGraphNode>, cursor = 1): LiveAgentGraphEntity =>
  decodeLiveAgentGraphEntity({
    schema: "openagents.live_agent_graph.v1",
    graphRef: "graph.presentation.1",
    sessionRef: "session.presentation.1",
    threadRef: "thread.presentation.1",
    attachmentGeneration: 1,
    cursor,
    lastDeltaRef: cursor === 0 ? null : `delta.presentation.${cursor}`,
    nodes,
    edges: nodes.flatMap(candidate => candidate.parent.kind === "agent"
      ? [{
          edgeRef: `edge.parent.${candidate.agentRef}`,
          kind: "parent" as const,
          fromAgentRef: candidate.parent.agentRef,
          toAgentRef: candidate.agentRef,
          version: 1,
        }]
      : []),
    updatedAt: now,
  })

describe("contract khala_mobile.agent_graph.confirmed_hierarchy_and_safe_focus.v1 — live-agent graph presentation", () => {
  test("projects hierarchy, current action, elapsed time, attention, terminal reason, and loss-accounted facts", () => {
    const root = node("agent.root", { kind: "root" }, {
      provider: { state: "known", kind: "codex", providerRef: "provider.codex.named" },
      attention: { state: "approval", attentionRef: "approval.1", since: now },
    })
    const child = node("agent.child", { kind: "agent", agentRef: "agent.root" }, {
      provider: { state: "unknown", reason: "provider_omitted" },
      runtime: { state: "unknown", reason: "not_observed" },
      worktree: { state: "unknown", reason: "provider_unsupported" },
      currentTool: { state: "unknown", reason: "not_observed" },
      status: "completed",
      terminal: { state: "terminal", reason: "completed", at: now },
      endedAt: now,
    })
    const result = projectLiveAgentGraphPresentation(graph([child, root]), {
      nowMs: Date.parse(now),
    })

    expect(result.rows.map(row => [row.agentRef, row.depth])).toEqual([
      ["agent.root", 0],
      ["agent.child", 1],
    ])
    expect(result.rows[0]).toMatchObject({
      attentionLabel: "Approval needs attention",
      statusLabel: "Running",
      toolLabel: "Search · Running",
      elapsedLabel: "1m 30s elapsed",
      tone: "attention",
    })
    expect(result.rows[1]).toMatchObject({
      providerLabel: "Provider unavailable · Provider Omitted",
      runtimeLabel: "Runtime unavailable · Not Observed",
      worktreeLabel: "Worktree unavailable · Provider Unsupported",
      toolLabel: "Action unavailable · Not Observed",
      terminalLabel: "Finished · Completed",
      canControl: false,
    })
  })

  test("historical imports remain inspectable but never controllable", () => {
    const result = projectLiveAgentGraphPresentation(graph([
      node("agent.root", { kind: "root" }),
    ]), { authority: "historical", nowMs: Date.parse(now) })
    expect(result.authorityLabel).toBe("Historical import")
    expect(result.rows[0]?.canControl).toBe(false)
  })

  test("rapid focus changes resolve deterministically across graph replacement", () => {
    const first = projectLiveAgentGraphPresentation(graph([
      node("agent.root", { kind: "root" }),
      node("agent.child", { kind: "agent", agentRef: "agent.root" }),
    ]), { nowMs: Date.parse(now) })
    expect(resolveLiveAgentGraphSelection(first, "agent.child")).toBe("agent.child")
    const replacement = projectLiveAgentGraphPresentation(graph([
      node("agent.root", { kind: "root" }),
    ], 2), { nowMs: Date.parse(now) })
    expect(resolveLiveAgentGraphSelection(replacement, "agent.child")).toBe("agent.root")
  })

  test("large graphs stay hierarchy-ordered behind an explicit bound", () => {
    const nodes = [node("agent.root", { kind: "root" })]
    for (let index = 0; index < 100; index += 1) {
      nodes.push(node(`agent.child.${String(index).padStart(3, "0")}`, {
        kind: "agent",
        agentRef: "agent.root",
      }))
    }
    const result = projectLiveAgentGraphPresentation(graph(nodes), {
      maxRows: 40,
      nowMs: Date.parse(now),
    })
    expect(result.rows).toHaveLength(40)
    expect(result.totalCount).toBe(101)
    expect(result.hiddenCount).toBe(61)
    expect(new Set(result.rows.map(row => row.agentRef)).size).toBe(40)
  })

  test("selects the newest attachment/cursor deterministically", () => {
    const old = graph([node("agent.root", { kind: "root" })], 1)
    const current = decodeLiveAgentGraphEntity({ ...old, graphRef: "graph.presentation.2", cursor: 2 })
    expect(newestLiveAgentGraph([current, old])?.graphRef).toBe("graph.presentation.2")
  })

  test("attributes exact per-node token usage and loss-accounts everything else", () => {
    const result = projectLiveAgentGraphPresentation(graph([
      node("agent.root", { kind: "root" }),
      node("agent.child", { kind: "agent", agentRef: "agent.root" }),
      node("agent.silent", { kind: "agent", agentRef: "agent.root" }),
    ]), {
      nowMs: Date.parse(now),
      tokenAttributions: [
        {
          agentRef: "agent.root",
          usageTruth: "exact",
          usage: { inputTokens: 1_200, cachedInputTokens: 100, outputTokens: 340, reasoningTokens: 60, totalTokens: 1_600 },
        },
        {
          agentRef: "agent.root",
          usageTruth: "exact",
          usage: { inputTokens: 800, cachedInputTokens: 0, outputTokens: 160, reasoningTokens: 40, totalTokens: 1_000 },
        },
        {
          agentRef: "agent.child",
          usageTruth: "exact",
          usage: { inputTokens: 500, cachedInputTokens: 0, outputTokens: 100, reasoningTokens: 0, totalTokens: 600 },
        },
        { agentRef: "agent.child", usageTruth: "unreported", usage: null },
      ],
    })
    const byRef = new Map(result.rows.map(row => [row.agentRef, row]))
    expect(byRef.get("agent.root")).toMatchObject({
      tokenTruth: "exact",
      tokensLabel: "2,000 in · 500 out · 2,600 total · exact",
    })
    expect(byRef.get("agent.child")).toMatchObject({
      tokenTruth: "partial",
      tokensLabel: "600 total from 1 exact turn · 1 unreported",
    })
    expect(byRef.get("agent.silent")).toMatchObject({
      tokenTruth: "unreported",
      tokensLabel: "Unreported",
    })
  })

  test("never synthesizes token truth from malformed exact claims", () => {
    const result = projectLiveAgentGraphPresentation(graph([
      node("agent.root", { kind: "root" }),
    ]), {
      nowMs: Date.parse(now),
      tokenAttributions: [
        // An "exact" claim without the reported split is a contradiction:
        // it must demote to loss-accounted, never render invented numbers.
        { agentRef: "agent.root", usageTruth: "exact", usage: null },
        {
          agentRef: "agent.root",
          usageTruth: "exact",
          usage: { inputTokens: Number.NaN, cachedInputTokens: 0, outputTokens: -5, reasoningTokens: 0, totalTokens: 10 },
        },
      ],
    })
    expect(result.rows[0]).toMatchObject({
      tokenTruth: "unreported",
      tokensLabel: "Unreported",
    })
  })
})
