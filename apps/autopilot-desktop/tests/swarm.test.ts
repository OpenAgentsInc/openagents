// CS-A2 (#5362): swarm / multi-session view helper + reducer tests.
//
// The swarm view is a lane/grid over the N concurrent coding sessions the
// runtime can already run (concurrent spawner #4869, control `session.list`,
// external-session `parentRef` nesting). It is a PURE read projection over the
// existing node-state — no new wire verb — plus one "open in composer"
// adoption action. These tests drive the pure helpers (helpers.ts) and the
// pure reducer (update.ts) without a DOM or a runtime, the same way the
// composer/CL-53 tests cover the other panes.

import { describe, expect, test } from "bun:test"

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import {
  isActiveSwarmSession,
  orderSwarmSessions,
  swarmAccountLabel,
  swarmSessionPendingApprovals,
  swarmStatusLabel,
  swarmSummaryLine,
  swarmWorkspaceLabel,
} from "../src/ui/helpers"
import { initialModel, Model } from "../src/ui/model"
import { ClickedOpenSessionInComposer } from "../src/ui/message"
import { update } from "../src/ui/update"

const session = (over: Partial<SessionSummary> = {}): SessionSummary => ({
  sessionRef: over.sessionRef ?? "session.pylon.codex.a",
  adapter: over.adapter ?? "codex",
  state: over.state ?? "running",
  accountRefHash: over.accountRefHash ?? null,
  updatedAt: over.updatedAt ?? "2026-06-18T12:00:00.000Z",
  ...over,
})

describe("swarm helpers (#5362)", () => {
  test("isActiveSwarmSession: only non-terminal states are active", () => {
    expect(isActiveSwarmSession("queued")).toBe(true)
    expect(isActiveSwarmSession("running")).toBe(true)
    expect(isActiveSwarmSession("started")).toBe(true)
    expect(isActiveSwarmSession("completed")).toBe(false)
    expect(isActiveSwarmSession("failed")).toBe(false)
    expect(isActiveSwarmSession("cancelled")).toBe(false)
  })

  test("swarmStatusLabel: maps each state to a short public-safe label + tone", () => {
    expect(swarmStatusLabel("queued")).toEqual({ text: "queued", toneClass: "swarm-queued" })
    expect(swarmStatusLabel("running")).toEqual({ text: "running", toneClass: "swarm-running" })
    expect(swarmStatusLabel("started")).toEqual({ text: "running", toneClass: "swarm-running" })
    expect(swarmStatusLabel("completed")).toEqual({ text: "done", toneClass: "swarm-completed" })
    expect(swarmStatusLabel("failed")).toEqual({ text: "failed", toneClass: "swarm-failed" })
    expect(swarmStatusLabel("cancelled")).toEqual({ text: "cancelled", toneClass: "swarm-cancelled" })
  })

  test("swarmAccountLabel: names the account from the readiness projection", () => {
    const accounts = [
      { accountRefHash: "hash-codex-a", provider: "codex", selector: "registry_ref" },
      { accountRefHash: "hash-codex-default", provider: "codex", selector: "default_home" },
    ]
    // A registry account shows provider + the hash suffix.
    expect(
      swarmAccountLabel({ accountRefHash: "hash-codex-a", adapter: "codex" }, accounts),
    ).toBe("codex · odex-a")
    // A default-home account shows provider + "default".
    expect(
      swarmAccountLabel({ accountRefHash: "hash-codex-default", adapter: "codex" }, accounts),
    ).toBe("codex · default")
    // No hash → adapter + default.
    expect(swarmAccountLabel({ accountRefHash: null, adapter: "claude_agent" }, accounts)).toBe(
      "claude_agent · default",
    )
    // Hash present but not in the projection → still named by suffix.
    expect(
      swarmAccountLabel({ accountRefHash: "unknown-xyz999", adapter: "codex" }, accounts),
    ).toBe("codex · xyz999")
  })

  test("swarmWorkspaceLabel: tails a workspace ref, placeholder when absent", () => {
    expect(swarmWorkspaceLabel({ workspaceRef: null })).toBe("—")
    expect(swarmWorkspaceLabel({})).toBe("—")
    expect(swarmWorkspaceLabel({ workspaceRef: "repo-abc" })).toBe("repo-abc")
    expect(swarmWorkspaceLabel({ workspaceRef: "workspace.pylon.very.long.ref.value" })).toContain(
      "…",
    )
  })

  test("swarmSessionPendingApprovals: counts unresolved decision_requested events", () => {
    expect(swarmSessionPendingApprovals(undefined)).toBe(0)
    expect(swarmSessionPendingApprovals([])).toBe(0)
    expect(
      swarmSessionPendingApprovals([
        { phase: "started" },
        { phase: "decision_requested" },
      ]),
    ).toBe(1)
    // A resolution cancels a pending request.
    expect(
      swarmSessionPendingApprovals([
        { phase: "decision_requested" },
        { phase: "decision_resolved" },
      ]),
    ).toBe(0)
    // A second request without a resolution leaves one pending.
    expect(
      swarmSessionPendingApprovals([
        { phase: "decision_requested" },
        { phase: "decision_resolved" },
        { phase: "decision_requested" },
      ]),
    ).toBe(1)
    // decision_cancelled also clears a pending request.
    expect(
      swarmSessionPendingApprovals([
        { phase: "decision_requested" },
        { phase: "decision_cancelled" },
      ]),
    ).toBe(0)
  })

  test("orderSwarmSessions: active first, then by recency", () => {
    const ordered = orderSwarmSessions([
      session({ sessionRef: "done-old", state: "completed", updatedAt: "2026-06-18T10:00:00.000Z" }),
      session({ sessionRef: "run-new", state: "running", updatedAt: "2026-06-18T12:00:00.000Z" }),
      session({ sessionRef: "queue-mid", state: "queued", updatedAt: "2026-06-18T11:00:00.000Z" }),
    ])
    expect(ordered.map((s) => s.sessionRef)).toEqual(["run-new", "queue-mid", "done-old"])
  })

  test("orderSwarmSessions: parentRef children are nested under their parent", () => {
    const ordered = orderSwarmSessions([
      session({ sessionRef: "child-1", state: "running", parentRef: "parent-1", updatedAt: "2026-06-18T12:30:00.000Z" }),
      session({ sessionRef: "parent-1", state: "running", updatedAt: "2026-06-18T12:00:00.000Z" }),
      session({ sessionRef: "solo", state: "running", updatedAt: "2026-06-18T13:00:00.000Z" }),
    ])
    const refs = ordered.map((s) => s.sessionRef)
    // The child sits immediately after its parent (a sub-lane), not scattered.
    expect(refs.indexOf("child-1")).toBe(refs.indexOf("parent-1") + 1)
    // A child whose parent is NOT in the set is treated as a root (no crash).
    const orphan = orderSwarmSessions([
      session({ sessionRef: "orphan", state: "running", parentRef: "missing-parent" }),
    ])
    expect(orphan.map((s) => s.sessionRef)).toEqual(["orphan"])
  })

  test("swarmSummaryLine: counts sessions, active, and pending approvals", () => {
    expect(swarmSummaryLine([], 0)).toBe("no sessions")
    expect(
      swarmSummaryLine([{ state: "running" }, { state: "completed" }], 0),
    ).toBe("2 sessions · 1 active")
    expect(
      swarmSummaryLine([{ state: "running" }, { state: "running" }], 3),
    ).toBe("2 sessions · 2 active · 3 pending approvals")
    expect(swarmSummaryLine([{ state: "queued" }], 1)).toBe(
      "1 session · 1 active · 1 pending approval",
    )
  })
})

describe("swarm reducer (#5362)", () => {
  test("ClickedOpenSessionInComposer adopts a session into the composer thread", () => {
    const start = Model.make({
      ...initialModel,
      pane: "swarm",
      composerSessionRef: null,
      composerRepoPath: "",
    })
    const [model, commands] = update(
      start,
      ClickedOpenSessionInComposer({
        sessionRef: "session.pylon.claude.live",
        workspaceRef: "workspace.pylon.repo.abc",
        adapter: "claude_agent",
      }),
    )
    expect(model.pane).toBe("composer")
    expect(model.composerSessionRef).toBe("session.pylon.claude.live")
    expect(model.composerRepoPath).toBe("workspace.pylon.repo.abc")
    expect(model.spawnAdapter).toBe("claude_agent")
    expect(model.composerStatus.tone).toBe("info")
    // Opening the composer loads the managed-account registry (CS-A1 reuse).
    // #5485: + the inference-gateway readiness for the route hint.
    expect(model.managedAccountsPending).toBe(true)
    expect(commands).toHaveLength(2)
    expect(commands.map((command) => command.name)).toEqual([
      "LoadManagedAccounts",
      "LoadInferenceGatewayReadiness",
    ])
  })

  test("ClickedOpenSessionInComposer tolerates a null workspaceRef", () => {
    const [model] = update(
      initialModel,
      ClickedOpenSessionInComposer({
        sessionRef: "session.pylon.codex.x",
        workspaceRef: null,
        adapter: "codex",
      }),
    )
    expect(model.composerRepoPath).toBe("")
    expect(model.composerSessionRef).toBe("session.pylon.codex.x")
  })
})
