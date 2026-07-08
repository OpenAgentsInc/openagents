import { describe, expect, it } from "bun:test"
import {
  decodeKhalaFleetIntent,
  decodeKhalaFleetIntentJson,
  fleetHarnessKinds,
  fleetWorkerKinds,
  khalaFleetIntentKinds,
  marginalCostClasses,
} from "./index.ts"

describe("khala-fleet-intents vocabulary", () => {
  it("includes grok alongside codex/claude and auto for worker selection", () => {
    expect([...fleetWorkerKinds]).toEqual(["codex", "claude", "grok", "auto"])
    expect([...fleetHarnessKinds]).toEqual(["codex", "claude", "grok"])
  })

  it("exposes the four marginal cost classes", () => {
    expect([...marginalCostClasses]).toEqual([
      "free",
      "subscription",
      "api_metered",
      "not_measured",
    ])
  })

  it("exposes the four intent kinds", () => {
    expect([...khalaFleetIntentKinds]).toEqual([
      "fleet_run_control",
      "approval_decision",
      "steer_message",
      "worker_selection",
    ])
  })
})

describe("KhalaFleetIntent decoding", () => {
  const base = {
    schema: "khala.fleet_intent.v1" as const,
    intentId: "intent-1",
    createdAt: "2026-07-08T00:00:00.000Z",
    origin: { surface: "mobile" as const },
    idempotencyKey: "idem-1",
  }

  it("decodes a fleet_run_control intent (drain)", () => {
    const intent = decodeKhalaFleetIntent({
      ...base,
      kind: "fleet_run_control",
      action: "drain",
      runRef: "run-1",
    })
    expect(intent.kind).toBe("fleet_run_control")
    if (intent.kind === "fleet_run_control") {
      expect(intent.action).toBe("drain")
    }
  })

  it("decodes an approval_decision intent (deny)", () => {
    const intent = decodeKhalaFleetIntent({
      ...base,
      kind: "approval_decision",
      approvalRef: "approval-1",
      decision: "deny",
    })
    expect(intent.kind).toBe("approval_decision")
  })

  it("decodes a worker_selection intent with auto policy and opaque session", () => {
    const intent = decodeKhalaFleetIntent({
      ...base,
      kind: "worker_selection",
      workerKind: "auto",
      autoPolicy: {
        schema: "khala.fleet_auto_policy.v1",
        preferenceOrder: ["grok", "codex", "claude"],
        maxMarginalCostClass: "subscription",
      },
      session: {
        harnessKind: "grok",
        sessionRef: "opaque-session-handle",
        capabilities: { resume: true, fork: false },
      },
    })
    expect(intent.kind).toBe("worker_selection")
    if (intent.kind === "worker_selection") {
      expect(intent.workerKind).toBe("auto")
      expect(intent.session?.capabilities.resume).toBe(true)
    }
  })

  it("decodes a steer_message intent from a JSON string (Sync mutator form)", () => {
    const intent = decodeKhalaFleetIntentJson(
      JSON.stringify({
        ...base,
        kind: "steer_message",
        body: "focus on the failing test first",
        targetRef: "worker-2",
      }),
    )
    expect(intent.kind).toBe("steer_message")
  })

  it("rejects an unknown intent kind", () => {
    expect(() =>
      decodeKhalaFleetIntent({ ...base, kind: "not_a_real_intent" }),
    ).toThrow()
  })
})
