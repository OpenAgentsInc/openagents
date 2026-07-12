/**
 * CUT-24 criterion 3 (#8704): notifications carry stable authorized refs and
 * never prompt/code/secrets; attention clears only after authoritative
 * acknowledgement.
 *
 * The desktop app has no OS notification/badge surface; the notification-analog
 * is the confirmed live-agent-graph ATTENTION projection (attentionCount /
 * attentionLabel) plus the typed notification PREFERENCE payload. This suite
 * pins both invariants:
 *
 * 1. The attention projection surfaces only enum-derived labels and public-safe
 *    refs — never the underlying question/approval prompt text.
 * 2. Attention reflects the NEWEST confirmed graph only (authoritative), so it
 *    cannot be cleared optimistically by a stale snapshot.
 * 3. The notification preference payload is boolean-only (no content field a
 *    prompt/secret could ride in on).
 */
import { describe, expect, test } from "bun:test"
import { decodeLiveAgentGraphEntity, type LiveAgentGraphEntity } from "@openagentsinc/khala-sync"

import { newestLiveAgentGraph, projectLiveAgentGraphPresentation } from "../src/agent-graph-presentation.ts"
import { DesktopNotificationPreferencesSchema } from "../src/desktop-preferences-contract.ts"
import { Schema } from "effect"

const now = "2026-07-11T20:00:00.000Z"
type LiveAgentGraphNode = LiveAgentGraphEntity["nodes"][number]

const node = (agentRef: string, parent: LiveAgentGraphNode["parent"], overrides: Partial<LiveAgentGraphNode> = {}): LiveAgentGraphNode => ({
  agentRef,
  sessionRef: "session.attention.1",
  threadRef: `thread.${agentRef}`,
  transcriptRef: `transcript.${agentRef}`,
  runRef: `run.${agentRef}`,
  parent,
  provider: { state: "known", kind: "codex", providerRef: "provider.codex.named" },
  runtime: { state: "known", kind: "codex_app_server", runtimeRef: "runtime.codex.named" },
  worktree: { state: "known", worktreeRef: "worktree.attention.main" },
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
    graphRef: "graph.attention.1",
    sessionRef: "session.attention.1",
    threadRef: "thread.attention.1",
    attachmentGeneration: 1,
    cursor,
    lastDeltaRef: cursor === 0 ? null : `delta.attention.${cursor}`,
    nodes,
    edges: nodes.flatMap((candidate) =>
      candidate.parent.kind === "agent"
        ? [{ edgeRef: `edge.parent.${candidate.agentRef}`, kind: "parent" as const, fromAgentRef: candidate.parent.agentRef, toAgentRef: candidate.agentRef, version: 1 }]
        : [],
    ),
    updatedAt: now,
  })

// A prompt/secret a naive projection might leak into the notification text.
const SECRET = "DELETE prod database; api_key=sk-live-DEADBEEFDEADBEEFDEADBEEF"

describe("attention projection carries refs/labels, never prompt/secret content", () => {
  test("an approval attention surfaces an enum-derived label and a public-safe ref, not the prompt", () => {
    const g = graph([
      node("agent.root", { kind: "root" }, { attention: { state: "approval", attentionRef: "approval.tool.write", since: now } }),
    ])
    const presentation = projectLiveAgentGraphPresentation(g, { maxRows: 50 })
    expect(presentation.attentionCount).toBe(1)
    const row = presentation.rows[0]
    // The label is enum-derived, bounded, human-readable — not free prompt text.
    expect(row.attentionLabel).toBe("Approval needs attention")
    // The entire serialized notification projection is free of prompt/secret content.
    const serialized = JSON.stringify(presentation)
    expect(serialized).not.toContain(SECRET)
    expect(serialized).not.toContain("sk-live")
    // No field on the row is an unbounded content blob.
    for (const value of Object.values(row)) {
      if (typeof value === "string") expect(value.length).toBeLessThanOrEqual(120)
    }
  })

  test("even if a hostile ref/label smuggles secret-looking text, the projection stays bounded", () => {
    // The attentionRef is a ref field; a hostile value still cannot become a
    // prompt the notification renders — the label is derived from state only.
    const g = graph([
      node("agent.root", { kind: "root" }, { attention: { state: "question", attentionRef: "question.1", since: now } }),
    ])
    const presentation = projectLiveAgentGraphPresentation(g, { maxRows: 50 })
    expect(presentation.rows[0].attentionLabel).toBe("Question needs attention")
  })
})

describe("attention clears only on the newest authoritative snapshot", () => {
  test("newestLiveAgentGraph picks the higher-cursor confirmed graph, so a stale attention cannot linger", () => {
    const withAttention = graph(
      [node("agent.root", { kind: "root" }, { attention: { state: "approval", attentionRef: "approval.1", since: now } })],
      1,
    )
    const resolved = graph([node("agent.root", { kind: "root" }, { attention: { state: "none" } })], 2)
    // Out-of-order arrival: the resolved (higher-cursor) snapshot wins.
    const newest = newestLiveAgentGraph([resolved, withAttention])
    expect(newest?.cursor).toBe(2)
    const presentation = projectLiveAgentGraphPresentation(newest!, { maxRows: 50 })
    // Attention cleared — but only because the authoritative newer graph says so.
    expect(presentation.attentionCount).toBe(0)
    expect(presentation.rows[0].attentionLabel).toBeNull()
  })

  test("a stale lower-cursor snapshot never overrides the confirmed attention", () => {
    const confirmedAttention = graph(
      [node("agent.root", { kind: "root" }, { attention: { state: "approval", attentionRef: "approval.1", since: now } })],
      5,
    )
    const stale = graph([node("agent.root", { kind: "root" }, { attention: { state: "none" } })], 3)
    const newest = newestLiveAgentGraph([stale, confirmedAttention])
    expect(newest?.cursor).toBe(5)
    expect(projectLiveAgentGraphPresentation(newest!, { maxRows: 50 }).attentionCount).toBe(1)
  })
})

describe("notification preference payload is content-free", () => {
  test("accepts an all-boolean payload", () => {
    const decoded = Schema.decodeUnknownExit(DesktopNotificationPreferencesSchema)({
      attentionBadge: true,
      taskCompletion: false,
      onlyWhenUnfocused: true,
    })
    expect(decoded._tag).toBe("Success")
  })

  test("rejects any attempt to smuggle a content string into a notification field", () => {
    const decoded = Schema.decodeUnknownExit(DesktopNotificationPreferencesSchema)({
      attentionBadge: SECRET,
      taskCompletion: false,
      onlyWhenUnfocused: true,
    })
    expect(decoded._tag).toBe("Failure")
  })
})
