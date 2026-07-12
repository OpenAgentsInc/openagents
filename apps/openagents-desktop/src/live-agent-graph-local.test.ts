/**
 * CUT-11 (#8691): deterministic fixture-stream tests for the desktop-local
 * canonical live agent graph assembler.
 *
 * Every test drives the assembler with a fixed typed event stream (the same
 * envelope shape the live fable-local/codex-local runtimes emit) and asserts
 * canonical `openagents.live_agent_graph.v1` assembly through the shared
 * reducer: node/edge shape, ordering, orphan refusal, interrupt settlement,
 * after-terminal refusal, permutation convergence, and exact usage
 * attribution.
 */
import { describe, expect, test } from "bun:test"

import type { FableChildUsage, FableLocalEvent } from "./fable-local-contract.ts"
import {
  createLocalAgentGraphAssembler,
  isLocalAgentGraphApplied,
  sanitizeLocalRefSegment,
  type LocalAgentGraphAssembler,
  type LocalAgentGraphResult,
} from "./live-agent-graph-local.ts"

const at = (minute: number): string =>
  `2026-07-12T05:${String(minute).padStart(2, "0")}:00.000Z`

const usageFixture: FableChildUsage = {
  inputTokens: 1_200,
  cachedInputTokens: 900,
  outputTokens: 180,
  reasoningTokens: 60,
  totalTokens: 1_440,
}

const rootUsageFixture: FableChildUsage = {
  inputTokens: 10,
  cachedInputTokens: 2,
  outputTokens: 5,
  reasoningTokens: 0,
  totalTokens: 15,
}

const makeAssembler = (): LocalAgentGraphAssembler =>
  createLocalAgentGraphAssembler({
    sessionRef: "session.local.test",
    threadRef: "thread.local.test",
    createdAt: at(0),
  })

const apply = (
  assembler: LocalAgentGraphAssembler,
  turnRef: string,
  event: FableLocalEvent,
  minute: number,
): LocalAgentGraphResult => assembler.applyEvent({ turnRef, event }, at(minute))

const expectApplied = (result: LocalAgentGraphResult): void => {
  if (!isLocalAgentGraphApplied(result)) {
    throw new Error(`expected applied, got refusal: ${result.refusal.reason} ${result.refusal.detail}`)
  }
}

const nodeOf = (assembler: LocalAgentGraphAssembler, agentRef: string) => {
  const node = assembler.snapshot().nodes.find(candidate => candidate.agentRef === agentRef)
  if (node === undefined) throw new Error(`node ${agentRef} missing`)
  return node
}

describe("createLocalAgentGraphAssembler", () => {
  test("assembles a fable root with codex delegate children into one canonical graph", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-1", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-1", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-1", {
      kind: "tool_use",
      toolName: "mcp__codex__delegate",
      summary: "delegating",
    }, 3))
    expectApplied(apply(assembler, "turn-1", {
      kind: "child_started",
      childRef: "child.codex.turn-1.1",
      summary: "task one",
    }, 4))
    expectApplied(apply(assembler, "turn-1", {
      kind: "child_activity",
      childRef: "child.codex.turn-1.1",
      activity: "item",
      summary: "agent_message: working",
    }, 5))
    expectApplied(apply(assembler, "turn-1", {
      kind: "child_completed",
      childRef: "child.codex.turn-1.1",
      accountRef: "codex-2",
      summary: "done",
      usage: usageFixture,
      durationMs: 42_000,
    }, 6))
    expectApplied(apply(assembler, "turn-1", {
      kind: "child_started",
      childRef: "child.codex.turn-1.2",
      summary: "task two",
    }, 7))
    expectApplied(apply(assembler, "turn-1", {
      kind: "child_failed",
      childRef: "child.codex.turn-1.2",
      accountRef: null,
      reason: "account_reconnect_required",
      detail: "every account needs reconnect",
    }, 8))
    expectApplied(apply(assembler, "turn-1", {
      kind: "tool_result",
      toolName: "mcp__codex__delegate",
      ok: true,
      summary: "delegate returned",
    }, 9))
    expectApplied(apply(assembler, "turn-1", {
      kind: "turn_completed",
      totalTokens: rootUsageFixture.totalTokens,
      accountRef: "claude-pylon-b",
      usage: rootUsageFixture,
    }, 10))

    const snapshot = assembler.snapshot()
    expect(snapshot.schema).toBe("openagents.live_agent_graph.v1")
    expect(snapshot.nodes.length).toBe(3)
    // 10 applied observations -> exactly 10 exact-cursor deltas.
    expect(snapshot.cursor).toBe(10)

    const rootRef = "agent.local.turn-1"
    const childOneRef = `${rootRef}.child.child.codex.turn-1.1`
    const childTwoRef = `${rootRef}.child.child.codex.turn-1.2`

    const root = nodeOf(assembler, rootRef)
    expect(root.parent).toEqual({ kind: "root" })
    expect(root.status).toBe("completed")
    expect(root.terminal).toEqual({ state: "terminal", reason: "completed", at: at(10) })
    expect(root.provider).toEqual({
      state: "known",
      kind: "claude",
      providerRef: "account.claude.claude-pylon-b",
    })
    expect(root.runtime).toEqual({
      state: "known",
      kind: "claude_agent_sdk",
      runtimeRef: "runtime.claude_agent_sdk.desktop_local",
    })
    // The local stream carries no worktree fact — explicit unknown, never fabricated.
    expect(root.worktree).toEqual({ state: "unknown", reason: "not_observed" })
    expect(root.startedAt).toBe(at(2))
    expect(root.endedAt).toBe(at(10))

    const childOne = nodeOf(assembler, childOneRef)
    expect(childOne.parent).toEqual({ kind: "agent", agentRef: rootRef })
    expect(childOne.status).toBe("completed")
    expect(childOne.provider).toEqual({
      state: "known",
      kind: "codex",
      providerRef: "account.codex.codex-2",
    })
    expect(childOne.runtime).toEqual({
      state: "known",
      kind: "codex_app_server",
      runtimeRef: "runtime.codex_exec.desktop_local",
    })
    expect(childOne.startedAt).toBe(at(4))
    expect(childOne.endedAt).toBe(at(6))

    const childTwo = nodeOf(assembler, childTwoRef)
    expect(childTwo.status).toBe("failed")
    expect(childTwo.terminal).toEqual({ state: "terminal", reason: "revoked", at: at(8) })
    expect(childTwo.statusReasonRef).toBe("reason.account_reconnect_required")
    // No final account was observed — provider stays an explicit unknown.
    expect(childTwo.provider).toEqual({ state: "unknown", reason: "not_observed" })

    const parentEdges = snapshot.edges.filter(edge => edge.kind === "parent")
    expect(parentEdges.map(edge => edge.kind === "parent" ? edge.toAgentRef : "")).toEqual(
      [childOneRef, childTwoRef].sort((a, b) => a.localeCompare(b)),
    )
    const toolEdges = snapshot.edges.filter(edge => edge.kind === "tool")
    expect(toolEdges.length).toBe(1)
    expect(toolEdges[0]?.kind === "tool" && toolEdges[0].status).toBe("completed")

    // Canonical ordering: nodes by agentRef, edges by edgeRef.
    expect(snapshot.nodes.map(node => node.agentRef)).toEqual(
      [...snapshot.nodes.map(node => node.agentRef)].sort((a, b) => a.localeCompare(b)),
    )
    expect(snapshot.edges.map(edge => edge.edgeRef)).toEqual(
      [...snapshot.edges.map(edge => edge.edgeRef)].sort((a, b) => a.localeCompare(b)),
    )

    const attributions = assembler.usageAttributions()
    expect(attributions.length).toBe(3)
    expect(attributions[0]).toMatchObject({
      agentRef: childOneRef,
      childRef: "child.codex.turn-1.1",
      provider: "codex",
      accountRef: "codex-2",
      usageTruth: "exact",
      usage: usageFixture,
    })
    expect(attributions[1]).toMatchObject({
      agentRef: childTwoRef,
      provider: "codex",
      accountRef: null,
      usageTruth: "unreported",
      usage: null,
    })
    expect(attributions[2]).toMatchObject({
      agentRef: rootRef,
      childRef: null,
      provider: "claude_agent",
      accountRef: "claude-pylon-b",
      usageTruth: "exact",
      usage: rootUsageFixture,
    })
    expect(assembler.refusals()).toEqual([])
  })

  test("codex-local root turns unify onto the same canonical semantics", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-cx", threadRef: "thread.local.test", lane: "codex_local" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-cx", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-cx", { kind: "reasoning", text: "thinking" }, 3))
    expectApplied(apply(assembler, "turn-cx", {
      kind: "turn_completed",
      totalTokens: usageFixture.totalTokens,
      accountRef: "codex-2",
      usage: usageFixture,
    }, 4))

    const root = nodeOf(assembler, "agent.local.turn-cx")
    expect(root.provider).toEqual({
      state: "known",
      kind: "codex",
      providerRef: "account.codex.codex-2",
    })
    expect(root.runtime).toEqual({
      state: "known",
      kind: "codex_app_server",
      runtimeRef: "runtime.codex_exec.desktop_local",
    })
    expect(root.status).toBe("completed")
    expect(root.activityCursor).toBe(3)
    expect(assembler.usageAttributions()).toEqual([
      {
        attributionRef: "usage.agent.local.turn-cx.1",
        agentRef: "agent.local.turn-cx",
        turnRef: "turn-cx",
        childRef: null,
        provider: "codex",
        accountRef: "codex-2",
        usageTruth: "exact",
        usage: usageFixture,
        recordedAt: at(4),
      },
    ])
  })

  test("orphan events are refused as typed records without mutating the graph", () => {
    const assembler = makeAssembler()
    const before = assembler.postImage().postImageJson

    const unknownTurn = apply(assembler, "never-started", {
      kind: "text_delta",
      text: "orphan",
    }, 1)
    expect(unknownTurn.applied).toBe(false)
    if (!unknownTurn.applied) expect(unknownTurn.refusal.reason).toBe("unknown_turn")

    expectApplied(assembler.startTurn(
      { turnRef: "turn-o", threadRef: "thread.local.test", lane: "fable_claude" },
      at(2),
    ))
    const unknownChild = apply(assembler, "turn-o", {
      kind: "child_steered",
      childRef: "ghost",
      action: "interrupt",
      outcome: "not_found",
      detail: "no child",
    }, 3)
    expect(unknownChild.applied).toBe(false)
    if (!unknownChild.applied) expect(unknownChild.refusal.reason).toBe("unknown_child")

    const malformed = assembler.applyEnvelopeValue({ nonsense: true }, at(4))
    expect(malformed.applied).toBe(false)
    if (!malformed.applied) expect(malformed.refusal.reason).toBe("invalid_event")

    // Only the startTurn advanced the graph; every refusal left it untouched.
    expect(assembler.snapshot().cursor).toBe(1)
    expect(assembler.refusals().map(refusal => refusal.reason)).toEqual([
      "unknown_turn",
      "unknown_child",
      "invalid_event",
    ])
    expect(before).not.toBe(assembler.postImage().postImageJson)
  })

  test("child activity without an observed start creates a loss-accounted child", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-l", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-l", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-l", {
      kind: "child_activity",
      childRef: "c-late",
      activity: "item",
      summary: "already running",
    }, 3))

    const child = nodeOf(assembler, "agent.local.turn-l.child.c-late")
    expect(child.status).toBe("running")
    // Start was never observed — honestly null, not fabricated from arrival.
    expect(child.startedAt).toBeNull()
    expect(child.parent).toEqual({ kind: "agent", agentRef: "agent.local.turn-l" })
    const parentEdge = assembler.snapshot().edges.find(edge =>
      edge.kind === "parent" && edge.toAgentRef === child.agentRef,
    )
    expect(parentEdge).toBeDefined()
  })

  test("events after a settled node are refused, never reopened", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-t", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-t", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-t", {
      kind: "child_started",
      childRef: "c1",
      summary: "task",
    }, 3))
    expectApplied(apply(assembler, "turn-t", {
      kind: "child_completed",
      childRef: "c1",
      accountRef: "codex-2",
      summary: "done",
      usage: null,
      durationMs: 1_000,
    }, 4))

    const duplicateSettle = apply(assembler, "turn-t", {
      kind: "child_completed",
      childRef: "c1",
      accountRef: "codex-2",
      summary: "done again",
      usage: usageFixture,
      durationMs: 1_000,
    }, 5)
    expect(duplicateSettle.applied).toBe(false)
    if (!duplicateSettle.applied) expect(duplicateSettle.refusal.reason).toBe("after_terminal")

    expectApplied(apply(assembler, "turn-t", {
      kind: "turn_completed",
      totalTokens: null,
    }, 6))
    const afterRootTerminal = apply(assembler, "turn-t", { kind: "text_delta", text: "late" }, 7)
    expect(afterRootTerminal.applied).toBe(false)
    if (!afterRootTerminal.applied) expect(afterRootTerminal.refusal.reason).toBe("after_terminal")

    const cursorBefore = assembler.snapshot().cursor
    const child = nodeOf(assembler, "agent.local.turn-t.child.c1")
    expect(child.status).toBe("completed")
    expect(assembler.snapshot().cursor).toBe(cursorBefore)
    // The unreported-usage completion stayed loss-accounted; the refused
    // duplicate recorded no second attribution.
    const childAttributions = assembler.usageAttributions().filter(entry => entry.childRef === "c1")
    expect(childAttributions.length).toBe(1)
    expect(childAttributions[0]?.usageTruth).toBe("unreported")
  })

  test("interrupt settles the root and every still-running child honestly", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-i", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-i", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-i", {
      kind: "tool_use",
      toolName: "mcp__codex__delegate",
      summary: "delegating",
    }, 3))
    expectApplied(apply(assembler, "turn-i", {
      kind: "child_started",
      childRef: "c1",
      summary: "task",
    }, 4))
    expectApplied(apply(assembler, "turn-i", {
      kind: "turn_failed",
      reason: "interrupted",
      detail: "user interrupt",
    }, 5))

    const root = nodeOf(assembler, "agent.local.turn-i")
    expect(root.status).toBe("interrupted")
    expect(root.terminal).toEqual({ state: "terminal", reason: "interrupted", at: at(5) })
    expect(root.statusReasonRef).toBe("reason.interrupted")
    expect(root.currentTool).toEqual({ state: "none" })

    const child = nodeOf(assembler, "agent.local.turn-i.child.c1")
    expect(child.status).toBe("interrupted")
    expect(child.terminal).toEqual({ state: "terminal", reason: "interrupted", at: at(5) })
    expect(child.statusReasonRef).toBe("reason.parent_settled")

    // The dangling delegate tool edge settled as loss-accounted unknown.
    const toolEdge = assembler.snapshot().edges.find(edge => edge.kind === "tool")
    expect(toolEdge?.kind === "tool" && toolEdge.status).toBe("unknown")
  })

  test("steer-interrupt settles exactly the targeted child", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-s", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-s", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-s", { kind: "child_started", childRef: "c1", summary: "a" }, 3))
    expectApplied(apply(assembler, "turn-s", { kind: "child_started", childRef: "c2", summary: "b" }, 4))
    expectApplied(apply(assembler, "turn-s", {
      kind: "child_steered",
      childRef: "c1",
      action: "interrupt",
      outcome: "interrupted",
      detail: "steered",
    }, 5))

    expect(nodeOf(assembler, "agent.local.turn-s.child.c1").status).toBe("interrupted")
    expect(nodeOf(assembler, "agent.local.turn-s.child.c1").statusReasonRef).toBe("reason.steer_interrupt")
    expect(nodeOf(assembler, "agent.local.turn-s.child.c2").status).toBe("running")
  })

  test("question attention follows pending/resolved and clears at terminal", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-q", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-q", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-q", {
      kind: "question_pending",
      questionRef: "q-1",
      questions: [{
        question: "Which lane?",
        header: "Lane",
        options: [{ label: "a" }, { label: "b" }],
        multiSelect: false,
      }],
    }, 3))

    let root = nodeOf(assembler, "agent.local.turn-q")
    expect(root.status).toBe("waiting_for_input")
    expect(root.attention).toEqual({
      state: "question",
      attentionRef: "question.q-1",
      since: at(3),
    })

    expectApplied(apply(assembler, "turn-q", {
      kind: "question_resolved",
      questionRef: "q-1",
      outcome: "answered",
    }, 4))
    root = nodeOf(assembler, "agent.local.turn-q")
    expect(root.status).toBe("running")
    expect(root.attention).toEqual({ state: "none" })
  })

  test("duplicate turns and cross-thread turns are refused", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-d", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    const duplicate = assembler.startTurn(
      { turnRef: "turn-d", threadRef: "thread.local.test", lane: "fable_claude" },
      at(2),
    )
    expect(duplicate.applied).toBe(false)
    if (!duplicate.applied) expect(duplicate.refusal.reason).toBe("duplicate_turn")

    const crossThread = assembler.startTurn(
      { turnRef: "turn-x", threadRef: "thread.local.other", lane: "fable_claude" },
      at(3),
    )
    expect(crossThread.applied).toBe(false)
    if (!crossThread.applied) expect(crossThread.refusal.reason).toBe("thread_mismatch")
    expect(assembler.snapshot().nodes.length).toBe(1)
  })

  test("independent child settlements converge byte-identically across delivery orders", () => {
    const run = (order: ReadonlyArray<"c1" | "c2">): string => {
      const assembler = makeAssembler()
      expectApplied(assembler.startTurn(
        { turnRef: "turn-p", threadRef: "thread.local.test", lane: "fable_claude" },
        at(1),
      ))
      expectApplied(apply(assembler, "turn-p", { kind: "turn_started" }, 2))
      // Both children start at the same observed instant.
      for (const childRef of order) {
        expectApplied(apply(assembler, "turn-p", {
          kind: "child_started",
          childRef,
          summary: `task ${childRef}`,
        }, 3))
      }
      // Both settle at the same observed instant, in either delivery order.
      for (const childRef of order) {
        expectApplied(apply(assembler, "turn-p", {
          kind: "child_completed",
          childRef,
          accountRef: "codex-2",
          summary: "done",
          usage: usageFixture,
          durationMs: 5_000,
        }, 4))
      }
      expectApplied(apply(assembler, "turn-p", {
        kind: "turn_completed",
        totalTokens: rootUsageFixture.totalTokens,
        accountRef: "claude-pylon-b",
        usage: rootUsageFixture,
      }, 5))
      return assembler.postImage().postImageJson
    }
    expect(run(["c1", "c2"])).toBe(run(["c2", "c1"]))
  })

  test("hostile refs sanitize deterministically without collisions", () => {
    expect(sanitizeLocalRefSegment("a b")).not.toBe(sanitizeLocalRefSegment("a-b"))
    expect(sanitizeLocalRefSegment("a-b")).toBe("a-b")
    const pattern = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/
    expect(pattern.test(sanitizeLocalRefSegment("turn/../weird ref"))).toBe(true)
    expect(pattern.test(sanitizeLocalRefSegment("---"))).toBe(true)

    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn one/with spaces", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(assembler.applyEvent(
      { turnRef: "turn one/with spaces", event: { kind: "turn_started" } },
      at(2),
    ))
    const snapshot = assembler.snapshot()
    expect(snapshot.nodes.length).toBe(1)
    expect(pattern.test(snapshot.nodes[0]?.agentRef ?? "")).toBe(true)
  })

  test("tool edges track called/completed lifecycle with a bounded current tool", () => {
    const assembler = makeAssembler()
    expectApplied(assembler.startTurn(
      { turnRef: "turn-w", threadRef: "thread.local.test", lane: "fable_claude" },
      at(1),
    ))
    expectApplied(apply(assembler, "turn-w", { kind: "turn_started" }, 2))
    expectApplied(apply(assembler, "turn-w", {
      kind: "tool_use",
      toolName: "Read",
      summary: "reading",
    }, 3))

    let root = nodeOf(assembler, "agent.local.turn-w")
    expect(root.currentTool).toEqual({
      state: "known",
      toolCallRef: "tool.turn-w.1",
      toolName: "Read",
      status: "running",
    })

    expectApplied(apply(assembler, "turn-w", {
      kind: "tool_result",
      toolName: "Read",
      ok: false,
      summary: "missing file",
    }, 4))
    root = nodeOf(assembler, "agent.local.turn-w")
    expect(root.currentTool).toEqual({ state: "none" })
    const toolEdge = assembler.snapshot().edges.find(edge =>
      edge.kind === "tool" && edge.toolCallRef === "tool.turn-w.1",
    )
    expect(toolEdge?.kind === "tool" && toolEdge.status).toBe("failed")
  })
})
