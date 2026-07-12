import { describe, expect, test } from "bun:test"
import { type View } from "@effect-native/core"
import { decodeLiveAgentGraphEntity } from "@openagentsinc/khala-sync"
import { projectLiveAgentGraphPresentation } from "../agent-graph-presentation.ts"

import { runtimeAgentGraphDetailFields, runtimeAgentGraphView } from "./runtime-agent-graph.ts"

const now = "2026-07-11T20:00:00.000Z"
const graph = decodeLiveAgentGraphEntity({
  schema: "openagents.live_agent_graph.v1",
  graphRef: "graph.desktop.presentation",
  sessionRef: "session.desktop.presentation",
  threadRef: "thread.desktop.presentation",
  attachmentGeneration: 1,
  cursor: 2,
  lastDeltaRef: "delta.desktop.presentation.2",
  nodes: [
    {
      agentRef: "agent.desktop.root",
      sessionRef: "session.desktop.presentation",
      threadRef: "thread.desktop.root",
      transcriptRef: "transcript.desktop.root",
      runRef: "run.desktop.root",
      parent: { kind: "root" },
      provider: { state: "known", kind: "codex", providerRef: "provider.codex.named" },
      runtime: { state: "known", kind: "codex_app_server", runtimeRef: "runtime.codex.named" },
      worktree: { state: "known", worktreeRef: "worktree.desktop.main" },
      status: "running",
      attention: { state: "none" },
      terminal: { state: "active" },
      currentTool: { state: "known", toolCallRef: "tool.desktop.root", toolName: "Search", status: "running" },
      attachmentGeneration: 1,
      activityCursor: 2,
      createdAt: now,
      updatedAt: now,
      startedAt: now,
      endedAt: null,
      version: 2,
    },
    {
      agentRef: "agent.desktop.child",
      sessionRef: "session.desktop.presentation",
      threadRef: "thread.desktop.child",
      transcriptRef: "transcript.desktop.child",
      runRef: "run.desktop.child",
      parent: { kind: "agent", agentRef: "agent.desktop.root" },
      provider: { state: "known", kind: "claude", providerRef: "provider.claude.named" },
      runtime: { state: "known", kind: "claude_agent_sdk", runtimeRef: "runtime.claude.named" },
      worktree: { state: "unknown", reason: "provider_unsupported" },
      status: "waiting_for_input",
      attention: { state: "approval", attentionRef: "approval.desktop.child", since: now },
      terminal: { state: "active" },
      currentTool: { state: "none" },
      attachmentGeneration: 1,
      activityCursor: 1,
      createdAt: now,
      updatedAt: now,
      startedAt: now,
      endedAt: null,
      version: 1,
    },
  ],
  edges: [{
    edgeRef: "edge.desktop.root.child",
    kind: "parent",
    fromAgentRef: "agent.desktop.root",
    toAgentRef: "agent.desktop.child",
    version: 1,
  }],
  updatedAt: now,
})

type Node = Readonly<Record<string, unknown>>
const collectNodes = (root: unknown): Node[] => {
  const nodes: Node[] = []
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) return value.forEach(visit)
    if (value === null || typeof value !== "object") return
    const node = value as Node
    if (typeof node._tag === "string") nodes.push(node)
    for (const [key, child] of Object.entries(node)) {
      if (key !== "style" && key !== "a11y") visit(child)
    }
  }
  visit(root)
  return nodes
}

const nodeByKey = (view: View, key: string): Node | undefined =>
  collectNodes(view).find(node => node.key === key)

describe("contract openagents_desktop.agent_graph.pointer_keyboard_focus_equivalence.v1", () => {
  test("expanded hierarchy exposes typed pointer/keyboard actions and full accessible details", () => {
    const presentation = projectLiveAgentGraphPresentation(graph, { nowMs: Date.parse(now) })
    const view = runtimeAgentGraphView({
      graph: presentation,
      expanded: true,
      selectedAgentRef: "agent.desktop.child",
      selectedTranscript: [
        { role: "user", text: "Inspect the failing test." },
        { role: "assistant", text: "The assertion uses the stale fixture." },
      ],
    })

    expect(nodeByKey(view, "runtime-agent-toggle")?.onPress).toMatchObject({ name: "DesktopAgentGraphToggled" })
    expect(nodeByKey(view, "runtime-agent-select-agent.desktop.child")?.onPress).toMatchObject({
      name: "DesktopAgentAction",
    })
    expect(JSON.stringify(nodeByKey(view, "runtime-agent-select-agent.desktop.child")?.onPress)).toContain("inspect_agent")
    expect(JSON.stringify(nodeByKey(view, "runtime-agent-select-agent.desktop.child")?.onPress)).toContain("agent.desktop.child")
    expect(nodeByKey(view, "runtime-agent-focus-agent.desktop.child")?.onPress).toMatchObject({
      name: "DesktopAgentAction",
    })
    expect(JSON.stringify(nodeByKey(view, "runtime-agent-focus-agent.desktop.child")?.onPress)).toContain("focus_agent")
    expect((nodeByKey(view, "runtime-agent-select-agent.desktop.child")?.a11y as { label?: string }).label).toContain("Approval needs attention")
    expect(runtimeAgentGraphDetailFields(presentation.rows[1]!).map(field => field.label)).toEqual([
      "Status", "Provider", "Runtime", "Session", "Worktree", "Elapsed", "Tokens", "Attention",
    ])
    expect(nodeByKey(view, "runtime-agent-summary-row")?.direction).toBe("column")
    expect(nodeByKey(view, "runtime-agent-row-agent.desktop.child")?.direction).toBe("column")
    expect(nodeByKey(view, "runtime-agent-fields-agent.desktop.child")?._tag).toBe("Stack")
    expect(nodeByKey(view, "runtime-agent-transcript-agent.desktop.child")?._tag).toBe("Stack")
    expect(nodeByKey(view, "runtime-agent-transcript-text-agent.desktop.child-0")?.content)
      .toBe("Inspect the failing test.")
    expect(nodeByKey(view, "runtime-agent-transcript-text-agent.desktop.child-1")?.content)
      .toBe("The assertion uses the stale fixture.")
    expect(JSON.stringify((view as unknown as Node).interactions)).toContain("DesktopAgentAction")
  })

  test("token attribution stays exact when reported and loss-accounted otherwise", () => {
    const attributed = projectLiveAgentGraphPresentation(graph, {
      nowMs: Date.parse(now),
      tokenAttributions: [{
        agentRef: "agent.desktop.child",
        usageTruth: "exact",
        usage: { inputTokens: 2_100, cachedInputTokens: 300, outputTokens: 450, reasoningTokens: 50, totalTokens: 2_600 },
      }],
    })
    const fieldsFor = (agentRef: string) =>
      runtimeAgentGraphDetailFields(attributed.rows.find(row => row.agentRef === agentRef)!)
    expect(fieldsFor("agent.desktop.child")).toContainEqual({
      label: "Tokens",
      value: "2,100 in · 450 out · 2,600 total · exact",
    })
    expect(fieldsFor("agent.desktop.root")).toContainEqual({
      label: "Tokens",
      value: "Unreported",
    })
    const view = runtimeAgentGraphView({
      graph: attributed,
      expanded: true,
      selectedAgentRef: "agent.desktop.child",
    })
    expect((nodeByKey(view, "runtime-agent-select-agent.desktop.child")?.a11y as { label?: string }).label)
      .toContain("2,600 total · exact")
  })

  test("collapsed hierarchy omits rows and historical authority omits live focus", () => {
    const live = projectLiveAgentGraphPresentation(graph, { nowMs: Date.parse(now) })
    const collapsed = runtimeAgentGraphView({ graph: live, expanded: false, selectedAgentRef: null })
    expect(nodeByKey(collapsed, "runtime-agent-select-agent.desktop.root")).toBeUndefined()

    const historical = projectLiveAgentGraphPresentation(graph, {
      authority: "historical",
      nowMs: Date.parse(now),
    })
    const historicalView = runtimeAgentGraphView({
      graph: historical,
      expanded: true,
      selectedAgentRef: "agent.desktop.root",
    })
    expect(nodeByKey(historicalView, "runtime-agent-focus-agent.desktop.root")).toBeUndefined()
    expect((nodeByKey(historicalView, "runtime-agent-graph")?.a11y as { label?: string }).label).toBe("Historical import agent graph")
  })
})
