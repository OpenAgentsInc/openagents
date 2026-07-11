import { describe, expect, mock, test } from "bun:test"
import { decodeLiveAgentGraphEntity, type LiveAgentGraphEntity } from "@openagentsinc/khala-sync"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

mock.module("../src/theme/typography", () => ({
  khalaMobileFontsToLoad: {},
  khalaMobileTextSizes: {
    lg: { fontSize: 20, lineHeight: 32 }, md: { fontSize: 18, lineHeight: 26 },
    sm: { fontSize: 16, lineHeight: 24 }, xl: { fontSize: 24, lineHeight: 34 },
    xs: { fontSize: 14, lineHeight: 21 }, xxl: { fontSize: 36, lineHeight: 44 },
    xxs: { fontSize: 12, lineHeight: 18 },
  },
  khalaMobileTypography: {
    code: { bold: "test-mono-bold", normal: "test-mono" },
    display: "test-display",
    primary: {
      bold: "test-sans-bold", light: "test-sans-light", medium: "test-sans-medium",
      normal: "test-sans", semiBold: "test-sans-semibold",
    },
  },
}))

const { LiveAgentGraphPanel } = await import("../src/components/live-agent-graph-panel")

type Node = LiveAgentGraphEntity["nodes"][number]
const now = "2026-07-11T20:00:00.000Z"

const node = (agentRef: string, parent: Node["parent"], overrides: Partial<Node> = {}): Node => ({
  agentRef,
  sessionRef: "session.mobile.graph",
  threadRef: `thread.${agentRef}`,
  transcriptRef: `transcript.${agentRef}`,
  runRef: `run.${agentRef}`,
  parent,
  provider: { state: "known", kind: "claude", providerRef: "provider.claude.named" },
  runtime: { state: "known", kind: "claude_agent_sdk", runtimeRef: "runtime.claude.named" },
  worktree: { state: "known", worktreeRef: "worktree.mobile.main" },
  status: "running",
  attention: { state: "none" },
  terminal: { state: "active" },
  currentTool: { state: "known", toolCallRef: `tool.${agentRef}`, toolName: "Search", status: "running" },
  attachmentGeneration: 1,
  activityCursor: 1,
  createdAt: "2026-07-11T19:59:00.000Z",
  updatedAt: now,
  startedAt: "2026-07-11T19:59:00.000Z",
  endedAt: null,
  version: 1,
  ...overrides,
})

const graph = (nodes: ReadonlyArray<Node>): LiveAgentGraphEntity => decodeLiveAgentGraphEntity({
  schema: "openagents.live_agent_graph.v1",
  graphRef: "graph.mobile.live",
  sessionRef: "session.mobile.graph",
  threadRef: "thread.mobile.graph",
  attachmentGeneration: 1,
  cursor: 1,
  lastDeltaRef: "delta.mobile.graph.1",
  nodes,
  edges: nodes.flatMap(candidate => candidate.parent.kind === "agent" ? [{
    edgeRef: `edge.${candidate.agentRef}`,
    kind: "parent" as const,
    fromAgentRef: candidate.parent.agentRef,
    toAgentRef: candidate.agentRef,
    version: 1,
  }] : []),
  updatedAt: now,
})

type AnyNode = { props: Record<string, unknown>; type: unknown }
const textContent = (value: unknown): string => {
  if (typeof value === "string" || typeof value === "number") return String(value)
  if (Array.isArray(value)) return value.map(textContent).join("")
  if (React.isValidElement(value)) return textContent((value.props as { children?: unknown }).children)
  return ""
}

const mount = async (element: React.ReactElement) => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  await act(async () => {
    renderer = createTestRenderer(element)
    await Promise.resolve()
  })
  return renderer!
}

describe("contract khala_mobile.agent_graph.confirmed_hierarchy_and_safe_focus.v1 — mobile live-agent graph panel", () => {
  test("renders the hierarchy, accessible inspect action, attention, and typed focus action", async () => {
    const actions = mock(() => undefined)
    const value = graph([
      node("agent.root", { kind: "root" }),
      node("agent.child", { kind: "agent", agentRef: "agent.root" }, {
        attention: { state: "question", attentionRef: "question.mobile.1", since: now },
      }),
    ])
    const renderer = await mount(
      <LiveAgentGraphPanel graphs={[value]} nowMs={Date.parse(now)} onAction={actions} phase="ready" />,
    )
    const agentButtons = renderer.root.findAll((candidate: AnyNode) =>
      typeof candidate.type === "string" &&
      candidate.props.accessibilityRole === "button" &&
      typeof candidate.props.accessibilityLabel === "string" &&
      candidate.props.accessibilityLabel.includes("Tap to inspect"))
    expect(agentButtons).toHaveLength(2)
    expect(String(agentButtons[1]!.props.accessibilityLabel)).toContain("Question needs attention")

    await act(async () => {
      ;(agentButtons[1]!.props.onPress as () => void)()
      await Promise.resolve()
    })
    expect(actions).toHaveBeenCalledWith({
      kind: "inspect_agent",
      graphRef: "graph.mobile.live",
      agentRef: "agent.child",
    })
    const focus = renderer.root.findAll((candidate: AnyNode) =>
      typeof candidate.type === "string" &&
      candidate.props.accessibilityLabel === "Focus Claude subagent 1 · child")
    expect(focus).toHaveLength(1)
    act(() => (focus[0]!.props.onPress as () => void)())
    expect(actions).toHaveBeenLastCalledWith({
      kind: "focus_agent",
      graphRef: "graph.mobile.live",
      agentRef: "agent.child",
    })
  })

  test("labels historical authority and withholds controls", async () => {
    const renderer = await mount(
      <LiveAgentGraphPanel
        authority="historical"
        graphs={[graph([node("agent.root", { kind: "root" })])]}
        nowMs={Date.parse(now)}
        onAction={() => undefined}
        phase="ready"
      />,
    )
    expect(renderer.root.findAll((candidate: AnyNode) =>
      typeof candidate.type === "string" &&
      typeof candidate.props.accessibilityLabel === "string" &&
      candidate.props.accessibilityLabel.includes("Historical import · controls unavailable"))).not.toHaveLength(0)
    expect(renderer.root.findAll((candidate: AnyNode) =>
      typeof candidate.type === "string" &&
      typeof candidate.props.accessibilityLabel === "string" &&
      candidate.props.accessibilityLabel.startsWith("Focus "))).toHaveLength(0)
  })

  test("large graphs expand behind a 40-row safety bound with an explicit remainder", async () => {
    const nodes: Node[] = [node("agent.root", { kind: "root" })]
    for (let index = 0; index < 100; index += 1) {
      nodes.push(node(`agent.child.${String(index).padStart(3, "0")}`, {
        kind: "agent",
        agentRef: "agent.root",
      }))
    }
    const renderer = await mount(
      <LiveAgentGraphPanel graphs={[graph(nodes)]} nowMs={Date.parse(now)} phase="ready" />,
    )
    const summary = renderer.root.findAll((candidate: AnyNode) =>
      candidate.props.accessibilityRole === "button" &&
      typeof candidate.props.accessibilityLabel === "string" &&
      candidate.props.accessibilityLabel.includes("101 agents"))[0]!
    expect(summary.props.accessibilityState).toEqual({ expanded: false })
    await act(async () => {
      ;(summary.props.onPress as () => void)()
      await Promise.resolve()
    })
    const agentButtons = renderer.root.findAll((candidate: AnyNode) =>
      typeof candidate.type === "string" &&
      typeof candidate.props.accessibilityLabel === "string" &&
      candidate.props.accessibilityLabel.includes("Tap to inspect"))
    expect(agentButtons).toHaveLength(40)
    const renderedText = renderer.root.findAll((candidate: AnyNode) =>
      typeof candidate.type === "string" && candidate.type === "Text").map(candidate => textContent(candidate.props.children)).join(" ")
    expect(renderedText).toContain("61 more agents hidden by the mobile safety bound")
  })
})
