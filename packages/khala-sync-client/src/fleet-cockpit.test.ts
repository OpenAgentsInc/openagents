import { describe, expect, test } from "vite-plus/test"
import type { ConfirmedAgentRun } from "./agent-timeline.js"
import type { ConfirmedRuntimeInteraction } from "./runtime-interactions.js"
import { admitFleetAttentionCommand, admitFleetRunCommand, projectFleetCockpitCard } from "./fleet-cockpit.js"

const run = (status: ConfirmedAgentRun["status"], version = 7): ConfirmedAgentRun => ({
  runRef: "run.fleet.1", routeRef: "thread.fleet.1", workContextRef: "work.fleet.1",
  runtime: "claude_code", backend: "pylon", status,
  createdAt: "2026-07-12T00:00:00.000Z", updatedAt: "2026-07-12T00:01:00.000Z",
  startedAt: "2026-07-12T00:00:01.000Z", completedAt: null, failedAt: null, canceledAt: null, version,
})
const approval = {
  schema: "openagents.runtime_interaction_projection.v1",
  interactionRef: "interaction.fleet.1", threadId: "thread.fleet.1", turnId: "run.fleet.1",
  kind: "tool_approval", status: "pending", displayTitle: "Approve command", displayText: "private",
  questions: [], expiresAt: "2026-07-12T01:00:00.000Z",
  requestedSequence: 2, requestedAt: "2026-07-12T00:00:02.000Z", version: 3,
} as ConfirmedRuntimeInteraction

describe("authoritative Fleet cockpit", () => {
  test("projects canonical links, provider, attention, receipts, and generation-checked actions", () => {
    const card = projectFleetCockpitCard({
      threadRef: "thread.fleet.1", title: "Fix checkout", authority: "live", run: run("running"),
      interactions: [approval], agentRefs: ["agent.root", "agent.root"], repositoryRef: "repo.1",
      receiptRefs: ["receipt.start", "receipt.start", "receipt.latest"],
    })
    expect(card).toMatchObject({ provider: "claude", actions: ["pause", "cancel"], agentRefs: ["agent.root"], receiptRefs: ["receipt.start", "receipt.latest"] })
    expect(card.attention).toEqual([{ interactionRef: "interaction.fleet.1", turnRef: "run.fleet.1", version: 3, kind: "tool_approval", title: "Approve command", actions: ["approve", "deny"] }])
    expect(admitFleetRunCommand(card, "pause")).toEqual({ action: "pause", threadRef: "thread.fleet.1", runRef: "run.fleet.1", expectedVersion: 7 })
    expect(admitFleetAttentionCommand(card, "interaction.fleet.1", "approve")).toEqual({ action: "approve", threadRef: "thread.fleet.1", runRef: "run.fleet.1", interactionRef: "interaction.fleet.1", expectedRunVersion: 7, expectedInteractionVersion: 3 })
  })

  test("offline/stale/revoked/unknown authority exposes truth but admits no optimistic control", () => {
    for (const authority of ["offline", "stale", "revoked", "unknown"] as const) {
      const card = projectFleetCockpitCard({ threadRef: "thread.fleet.1", title: "x", authority, run: run("running"), interactions: [approval], agentRefs: [], receiptRefs: [] })
      expect(card.actions).toEqual([])
      expect(admitFleetRunCommand(card, "cancel")).toBeNull()
      expect(admitFleetAttentionCommand(card, "interaction.fleet.1", "deny")).toBeNull()
    }
  })

  test("terminal and canceled status action sets are closed and old generations stay explicit", () => {
    const canceled = projectFleetCockpitCard({ threadRef: "thread.fleet.1", title: "x", authority: "live", run: run("canceled", 9), interactions: [], agentRefs: [], receiptRefs: [] })
    expect(canceled.actions).toEqual(["resume", "retry", "close"])
    expect(admitFleetRunCommand(canceled, "pause")).toBeNull()
    expect(admitFleetRunCommand(canceled, "resume")?.expectedVersion).toBe(9)
  })
})
