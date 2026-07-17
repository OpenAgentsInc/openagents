import { describe, expect, test } from "vite-plus/test"
import { IntentRef, StaticPayload } from "@effect-native/core"
import { Effect, Stream } from "@effect-native/core/effect"
import {
  decodeLiveAgentGraphEntity,
  type LiveAgentGraphEntity,
} from "@openagentsinc/khala-sync"
import { projectLiveAgentGraphPresentation } from "@openagentsinc/khala-sync-client"

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"
import {
  agentStackViews,
  defaultMobileAccessibilityProfile,
  initialKhalaState,
  mobileAgentRowDetailFields,
  MOBILE_AGENT_GRAPH_MAX_ROWS,
} from "../src/screens/khala-core"

const now = "2026-07-12T05:00:00.000Z"
type LiveAgentGraphNode = LiveAgentGraphEntity["nodes"][number]

const node = (
  agentRef: string,
  parent: LiveAgentGraphNode["parent"],
  overrides: Partial<LiveAgentGraphNode> = {},
): LiveAgentGraphNode => ({
  agentRef,
  sessionRef: "session.mobile.graph",
  threadRef: `thread.${agentRef}`,
  transcriptRef: `transcript.${agentRef}`,
  runRef: `run.${agentRef}`,
  parent,
  provider: { state: "known", kind: "codex", providerRef: "provider.codex.named" },
  runtime: { state: "known", kind: "codex_app_server", runtimeRef: "runtime.codex.named" },
  worktree: { state: "known", worktreeRef: "worktree.mobile.main" },
  status: "running",
  attention: { state: "none" },
  terminal: { state: "active" },
  currentTool: { state: "known", toolCallRef: `tool.${agentRef}`, toolName: "Search", status: "running" },
  attachmentGeneration: 1,
  activityCursor: 1,
  createdAt: now,
  updatedAt: now,
  startedAt: now,
  endedAt: null,
  version: 1,
  ...overrides,
})

const graphEntity = (
  nodes: ReadonlyArray<LiveAgentGraphNode>,
  cursor = 1,
): LiveAgentGraphEntity =>
  decodeLiveAgentGraphEntity({
    schema: "openagents.live_agent_graph.v1",
    graphRef: "graph.mobile.supervision",
    sessionRef: "session.mobile.graph",
    threadRef: "thread.synced.graph",
    attachmentGeneration: 1,
    cursor,
    lastDeltaRef: `delta.mobile.${cursor}`,
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

const threadWith = (
  graphs: ReadonlyArray<LiveAgentGraphEntity>,
): MobileConversationThread => ({
  threadRef: "thread.synced.graph",
  title: "Supervised",
  status: "active",
  messageCount: 1,
  lastMessageAt: now,
  updatedAt: now,
  version: 3,
  messages: [{
    messageRef: "message.synced.graph",
    threadRef: "thread.synced.graph",
    body: "Confirmed",
    createdAt: now,
    updatedAt: now,
    version: 5,
  }],
  graphs,
})

const hostFor = (
  openThread: (threadRef: string) => Promise<MobileConversationThread | null>,
): MobileConversationHost => ({
  listThreads: async () => [threadWith([])],
  newThread: async () => ({ ok: false, error: "unused" }),
  openThread,
  sendMessage: async () => ({ ok: false, error: "unused" }),
})

const selection = (
  activeThread: MobileConversationThread,
  host: MobileConversationHost,
): Extract<MobileConversationSelection, { mode: "sync" }> => ({
  mode: "sync",
  host,
  threads: [activeThread],
  archivedThreads: [],
  activeThread,
})

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.seam.agent_graph_inline_supervision.v1", () => {
  test("projects the confirmed thread-scope hierarchy above the transcript with attention auto-open", () => {
    const thread = threadWith([graphEntity([
      node("agent.root", { kind: "root" }),
      node("agent.child", { kind: "agent", agentRef: "agent.root" }, {
        provider: { state: "known", kind: "claude", providerRef: "provider.claude.named" },
        runtime: { state: "known", kind: "claude_agent_sdk", runtimeRef: "runtime.claude.named" },
        status: "waiting_for_input",
        attention: { state: "approval", attentionRef: "approval.mobile.child", since: now },
        currentTool: { state: "none" },
      }),
    ])])
    const program = buildHomeProgram({
      conversation: selection(thread, hostFor(async () => thread)),
    })
    const khala = program.initialState.khala
    expect(khala.agentGraph?.authorityLabel).toBe("Live")
    expect(khala.agentGraph?.rows.map(row => [row.agentRef, row.depth])).toEqual([
      ["agent.root", 0],
      ["agent.child", 1],
    ])
    // Attention auto-opens the stack before any tap.
    expect(khala.agentGraphExpanded).toBe(true)
    // No row is auto-inspected before the first explicit tap.
    expect(khala.selectedAgentRef).toBeNull()

    const view = JSON.stringify(renderContentView(program.initialState))
    expect(view).toContain("Live agent stack")
    expect(view).toContain("2 agents · 2 active · 1 need attention")
    expect(view).toContain("khala-agent-select-agent.root")
    expect(view).toContain("khala-agent-select-agent.child")
    expect(view).toContain("Approval needs attention")
  })

  test("tap selects and inspects the exact typed agent ref; a second tap closes the inspector", async () => {
    const thread = threadWith([graphEntity([
      node("agent.root", { kind: "root" }),
      node("agent.child", { kind: "agent", agentRef: "agent.root" }, {
        attention: { state: "question", attentionRef: "question.mobile.child", since: now },
      }),
    ])])
    const program = buildHomeProgram({
      conversation: selection(thread, hostFor(async () => thread)),
    })

    program.khala.selectAgentRow("agent.child")
    await Effect.runPromise(settle)
    const selected = await Effect.runPromise(lastState(program))
    expect(selected.khala.selectedAgentRef).toBe("agent.child")
    const inspecting = JSON.stringify(renderContentView(selected))
    expect(inspecting).toContain("khala-agent-inspector-agent.child")
    expect(inspecting).toContain("Tokens · Unreported")
    expect(inspecting).toContain("Worktree main")

    program.khala.selectAgentRow("agent.child")
    await Effect.runPromise(settle)
    const cleared = await Effect.runPromise(lastState(program))
    expect(cleared.khala.selectedAgentRef).toBeNull()
    expect(JSON.stringify(renderContentView(cleared))).not.toContain("khala-agent-inspector-agent.child")
  })

  test("selection survives graph replacement through the deterministic fallback", async () => {
    let current = threadWith([graphEntity([
      node("agent.root", { kind: "root" }),
      node("agent.child", { kind: "agent", agentRef: "agent.root" }),
    ], 1)])
    const program = buildHomeProgram({
      conversation: selection(current, hostFor(async () => current)),
    })

    program.khala.selectAgentRow("agent.child")
    await Effect.runPromise(settle)

    // The confirmed replacement drops the selected child.
    current = threadWith([graphEntity([node("agent.root", { kind: "root" })], 2)])
    await Effect.runPromise(program.report(IntentRef(
      "ConversationThreadSelected",
      StaticPayload({ threadRef: "thread.synced.graph" }),
    )) as Effect.Effect<unknown>)
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)
    const replaced = await Effect.runPromise(lastState(program))
    expect(replaced.khala.agentGraph?.rows.map(row => row.agentRef)).toEqual(["agent.root"])
    expect(replaced.khala.selectedAgentRef).toBe("agent.root")
  })

  test("collapses without attention, expands on toggle, and names the exact hidden remainder", async () => {
    const nodes: LiveAgentGraphNode[] = [node("agent.root", { kind: "root" })]
    for (let index = 0; index < 49; index += 1) {
      nodes.push(node(`agent.child.${String(index).padStart(3, "0")}`, {
        kind: "agent",
        agentRef: "agent.root",
      }))
    }
    const thread = threadWith([graphEntity(nodes)])
    const program = buildHomeProgram({
      conversation: selection(thread, hostFor(async () => thread)),
    })
    expect(program.initialState.khala.agentGraphExpanded).toBe(false)
    expect(program.initialState.khala.agentGraph?.rows).toHaveLength(MOBILE_AGENT_GRAPH_MAX_ROWS)
    expect(program.initialState.khala.agentGraph?.hiddenCount).toBe(10)
    const collapsed = JSON.stringify(renderContentView(program.initialState))
    expect(collapsed).toContain("50 agents · 50 active")
    expect(collapsed).not.toContain("khala-agent-select-agent.root")

    program.khala.toggleAgentStack()
    await Effect.runPromise(settle)
    const expanded = await Effect.runPromise(lastState(program))
    const view = JSON.stringify(renderContentView(expanded))
    expect(view).toContain("khala-agent-select-agent.root")
    expect(view).toContain("10 more agents hidden by the mobile safety bound")
  })

  test("historical authority is labeled and never claims live control", () => {
    const historical = projectLiveAgentGraphPresentation(graphEntity([
      node("agent.root", { kind: "root" }),
    ]), { authority: "historical", nowMs: Date.parse(now) })
    expect(historical.rows[0]?.canControl).toBe(false)
    const views = agentStackViews({
      ...initialKhalaState,
      agentGraph: historical,
      agentGraphExpanded: true,
      selectedAgentRef: "agent.root",
    }, defaultMobileAccessibilityProfile)
    const serialized = JSON.stringify(views)
    expect(serialized).toContain("Historical import agent stack")
    expect(serialized).toContain("Historical import · controls unavailable")
    // The mobile stack exposes exactly the local select/inspect intent set:
    // no runtime-control intent is reachable from a graph row.
    expect(serialized).not.toContain("RuntimeTurnControlRequested")
    expect(serialized).not.toContain("focus_agent")
  })

  test("exact token attribution renders per node and stays loss-accounted otherwise", () => {
    const attributed = projectLiveAgentGraphPresentation(graphEntity([
      node("agent.root", { kind: "root" }),
      node("agent.child", { kind: "agent", agentRef: "agent.root" }),
    ]), {
      nowMs: Date.parse(now),
      tokenAttributions: [{
        agentRef: "agent.child",
        usageTruth: "exact",
        usage: { inputTokens: 1_500, cachedInputTokens: 200, outputTokens: 300, reasoningTokens: 100, totalTokens: 1_900 },
      }],
    })
    const child = attributed.rows.find(row => row.agentRef === "agent.child")!
    expect(mobileAgentRowDetailFields(child)).toContainEqual({
      label: "Tokens",
      value: "1,500 in · 300 out · 1,900 total · exact",
    })
    const root = attributed.rows.find(row => row.agentRef === "agent.root")!
    expect(mobileAgentRowDetailFields(root)).toContainEqual({
      label: "Tokens",
      value: "Unreported",
    })
    const views = agentStackViews({
      ...initialKhalaState,
      agentGraph: attributed,
      agentGraphExpanded: true,
      selectedAgentRef: "agent.child",
    }, defaultMobileAccessibilityProfile)
    expect(JSON.stringify(views)).toContain("1,900 total · exact")
  })
})
