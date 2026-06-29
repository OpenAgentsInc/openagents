// #5467 (EPIC #5461): Autonomous loop view — pure helper + reducer + render
// tests. Mirrors the swarm/CL-53 approach: drive the pure derivations and the
// pure reducer/view without a DOM or a runtime. The loop view is a READ-ONLY
// projection of the autonomous coordinator loop over the existing control
// surface (`intent.list` + `coordinator.status`); these tests pin that it
// derives the five loop stages from REAL status/statusHistory, states the ship
// gate honestly, reuses pause/resume, and renders a mountable Document.

import { describe, expect, test } from "bun:test"

import type { NodeStateMessage } from "../src/shared/rpc"
import {
  autonomousLoopSummary,
  loopStageStates,
  LOOP_STAGES,
  shipGateLine,
} from "../src/ui/helpers"
import { GotNodeState, NavigatedTo } from "../src/ui/message"
import { initialModel, Model, modelNode } from "../src/ui/model"
import { NAV_DESTINATIONS, NAV_GROUPS } from "../src/ui/nav"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

const intent = (over: Partial<{
  intentId: string
  title: string
  status: string
  submittedByClientRef: string
  statusHistory: ReadonlyArray<{ status: string; observedAt: string }>
}> = {}) => ({
  intentId: over.intentId ?? "intent.abc12345",
  title: over.title ?? "",
  status: over.status ?? "received",
  submittedByClientRef: over.submittedByClientRef ?? "desktop",
  ...(over.statusHistory ? { statusHistory: over.statusHistory } : {}),
})

describe("#5467 loop-stage derivation (real status + statusHistory)", () => {
  test("the five stages are intent → plan → fanout → reconcile → ship", () => {
    expect(LOOP_STAGES.map((s) => s.id)).toEqual([
      "intent",
      "plan",
      "fanout",
      "reconcile",
      "ship",
    ])
  })

  test("a freshly received intent: intent active, rest pending", () => {
    const states = loopStageStates(intent({ status: "received" }))
    expect(states.map((s) => s.state)).toEqual([
      "active",
      "pending",
      "pending",
      "pending",
      "pending",
    ])
  })

  test("a fanning_out intent marks earlier stages done and fanout active", () => {
    const states = loopStageStates(
      intent({
        status: "fanning_out",
        statusHistory: [
          { status: "received", observedAt: "t0" },
          { status: "planning", observedAt: "t1" },
          { status: "fanning_out", observedAt: "t2" },
        ],
      }),
    )
    expect(states.map((s) => s.state)).toEqual([
      "done",
      "done",
      "active",
      "pending",
      "pending",
    ])
  })

  test("a shipped intent marks every stage done", () => {
    const states = loopStageStates(intent({ status: "shipped" }))
    expect(states.every((s) => s.state === "done")).toBe(true)
  })

  test("a failed intent marks its furthest stage failed, not done", () => {
    const states = loopStageStates(intent({ status: "failed" }))
    // failed ranks at the ship stage; it renders failed there.
    expect(states[4]?.state).toBe("failed")
    expect(states.some((s) => s.state === "active")).toBe(false)
  })
})

describe("#5467 ship gate is stated honestly (default-DENY / escalate)", () => {
  test("pending statuses never imply autonomous spend", () => {
    const line = shipGateLine("received")
    expect(line.tone).toBe("neutral")
    expect(line.text.toLowerCase()).toContain("default-deny")
    expect(line.text.toLowerCase()).not.toContain("spent")
  })

  test("shipping escalates to the owner", () => {
    const line = shipGateLine("shipping")
    expect(line.tone).toBe("active")
    expect(line.text.toLowerCase()).toContain("escalates")
  })

  test("shipped is owner-gated, not autonomous spend", () => {
    const line = shipGateLine("shipped")
    expect(line.tone).toBe("shipped")
    expect(line.text.toLowerCase()).toContain("owner-gated")
  })

  test("failed before ship", () => {
    expect(shipGateLine("failed").tone).toBe("failed")
  })
})

describe("#5467 loop summary roll-up", () => {
  test("counts in-flight vs terminal and reflects the paused flag", () => {
    const intents = [
      intent({ status: "received" }),
      intent({ status: "fanning_out" }),
      intent({ status: "shipped" }),
      intent({ status: "failed" }),
    ]
    expect(autonomousLoopSummary(intents, false)).toBe(
      "running · 2 in-flight · 1 shipped · 1 failed",
    )
    expect(autonomousLoopSummary(intents, true)).toContain("paused")
    expect(autonomousLoopSummary([], null)).toContain("no asks yet")
  })
})

describe("#5467 nav registration (Supervise group, no new top-level button)", () => {
  test("the loop is a destination inside the Supervise group", () => {
    const supervise = NAV_GROUPS.find((g) => g.id === "supervise")
    expect(supervise).toBeDefined()
    expect(supervise?.destinations.some((d) => d.pane === "autonomous-loop")).toBe(true)
  })

  test("the loop is NOT its own top-level group (anti-clutter)", () => {
    expect(NAV_GROUPS.some((g) => g.defaultPane === "autonomous-loop")).toBe(false)
    expect(NAV_GROUPS.length).toBeLessThanOrEqual(6)
  })

  test("the loop appears exactly once across all destinations", () => {
    const hits = NAV_DESTINATIONS.filter((d) => d.pane === "autonomous-loop")
    expect(hits.length).toBe(1)
  })
})

describe("#5467 reducer + render (read-only over node-state)", () => {
  const nodeWith = (over: Partial<NodeStateMessage> = {}): NodeStateMessage => ({
    ok: true,
    schema: "openagents.pylon.control.v0.3",
    sessions: [],
    intents: [],
    coordinatorPaused: false,
    ...over,
  })

  test("NavigatedTo autonomous-loop selects the pane", () => {
    const [model] = update(initialModel, NavigatedTo({ pane: "autonomous-loop" }))
    expect(model.pane).toBe("autonomous-loop")
  })

  test("the pane mounts a Document (not a blank window) with live data", () => {
    const [withNode] = update(
      initialModel,
      GotNodeState({
        node: nodeWith({
          intents: [intent({ status: "fanning_out", title: "ref-title" })],
          sessions: [
            {
              sessionRef: "session.pylon.codex.live",
              adapter: "codex",
              state: "running",
              accountRefHash: null,
              updatedAt: "2026-06-19T00:00:00.000Z",
            },
          ],
        }),
      }),
    )
    const model = Model.make({ ...withNode, pane: "autonomous-loop" })
    const doc = view(model) as unknown as { title: string; body: unknown }
    expect(doc.title).toBe("Autopilot")
    expect(doc.body).toBeDefined()
    expect(doc.body).not.toBeNull()
    // The node projection round-tripped through the model and is what the pane
    // reads (no fabricated data).
    expect(modelNode(model)?.intents?.[0]?.status).toBe("fanning_out")
  })

  test("renders with no node yet (connecting) without crashing", () => {
    const model = Model.make({ ...initialModel, pane: "autonomous-loop" })
    const doc = view(model) as unknown as { body: unknown }
    expect(doc.body).toBeDefined()
  })
})
