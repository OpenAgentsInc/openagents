import { describe, expect, it } from "bun:test"
import {
  decodeKhalaFleetIntent,
  decodeKhalaFleetIntentJson,
  defaultFleetAutoPolicy,
  type FleetAutoTargetCandidate,
  fleetHarnessKinds,
  fleetWorkerKinds,
  khalaFleetIntentKinds,
  marginalCostClasses,
  rankFleetHarnessesByCostClass,
  resolveFleetAutoTarget,
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

describe("rankFleetHarnessesByCostClass", () => {
  it("re-sorts the base order cheapest-cost-class-first from measured data", () => {
    const order = rankFleetHarnessesByCostClass({
      baseOrder: ["codex", "claude", "grok"],
      costClassByHarness: {
        claude: "subscription",
        codex: "subscription",
        grok: "free",
      },
    })
    expect(order).toEqual(["grok", "codex", "claude"])
  })

  it("treats a harness missing from the cost map as not_measured (never assumed free)", () => {
    const order = rankFleetHarnessesByCostClass({
      baseOrder: ["codex", "claude", "grok"],
      costClassByHarness: { codex: "subscription" },
    })
    // codex (subscription, rank 1) sorts ahead of claude/grok (not_measured,
    // rank 3); claude and grok keep their relative base-order position.
    expect(order).toEqual(["codex", "claude", "grok"])
  })

  it("re-ranks when the underlying data changes (free window ending), no code change", () => {
    const whileFree = rankFleetHarnessesByCostClass({
      baseOrder: ["codex", "claude", "grok"],
      costClassByHarness: { claude: "subscription", codex: "subscription", grok: "free" },
    })
    const afterFreeEnds = rankFleetHarnessesByCostClass({
      baseOrder: ["codex", "claude", "grok"],
      costClassByHarness: { claude: "subscription", codex: "subscription", grok: "api_metered" },
    })
    expect(whileFree).toEqual(["grok", "codex", "claude"])
    expect(afterFreeEnds).toEqual(["codex", "claude", "grok"])
  })
})

describe("resolveFleetAutoTarget", () => {
  const ready = (
    harnessKind: FleetAutoTargetCandidate["harnessKind"],
    accountRef: string,
    marginalCostClass: FleetAutoTargetCandidate["marginalCostClass"] = "not_measured",
  ): FleetAutoTargetCandidate => ({ accountRef, harnessKind, marginalCostClass, ready: true })

  const notReady = (
    harnessKind: FleetAutoTargetCandidate["harnessKind"],
    accountRef: string,
    reason: NonNullable<FleetAutoTargetCandidate["reason"]>,
    marginalCostClass: FleetAutoTargetCandidate["marginalCostClass"] = "not_measured",
  ): FleetAutoTargetCandidate => ({ accountRef, harnessKind, marginalCostClass, reason, ready: false })

  it("picks the first ready candidate in the fixed preference order with zero skips", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [ready("codex", "codex-a"), ready("claude", "claude-a")],
      policy: defaultFleetAutoPolicy,
    })
    expect(resolution).toEqual({
      events: [],
      selection: { accountRef: "codex-a", harnessKind: "codex", marginalCostClass: "not_measured" },
      usedFallback: false,
    })
  })

  it("falls back through an exhaustion chain to the next candidate in order", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [
        notReady("codex", "codex-a", "account_exhausted"),
        notReady("claude", "claude-a", "account_rate_limited"),
        ready("grok", "grok-a"),
      ],
      policy: defaultFleetAutoPolicy,
    })
    expect(resolution.usedFallback).toBe(true)
    expect(resolution.selection).toEqual({
      accountRef: "grok-a",
      harnessKind: "grok",
      marginalCostClass: "not_measured",
    })
    expect(resolution.events).toEqual([
      {
        accountRef: "codex-a",
        harnessKind: "codex",
        nextAccountRef: "claude-a",
        nextHarnessKind: "claude",
        type: "account_exhausted",
      },
      {
        accountRef: "claude-a",
        harnessKind: "claude",
        nextAccountRef: "grok-a",
        nextHarnessKind: "grok",
        type: "account_rate_limited",
      },
    ])
  })

  it("falls through multiple accounts of the SAME harness before moving to the next harness", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [
        notReady("codex", "codex-a", "account_exhausted"),
        notReady("codex", "codex-b", "account_requires_reauth"),
        ready("claude", "claude-a"),
      ],
      policy: defaultFleetAutoPolicy,
    })
    expect(resolution.selection?.accountRef).toBe("claude-a")
    expect(resolution.events.map(e => e.type)).toEqual([
      "account_exhausted",
      "account_requires_reauth",
    ])
    expect(resolution.events.map(e => e.accountRef)).toEqual(["codex-a", "codex-b"])
  })

  it("returns null selection with a full skip-event chain when every candidate is skipped", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [
        notReady("codex", "codex-a", "account_exhausted"),
        notReady("claude", "claude-a", "account_rate_limited"),
        notReady("grok", "grok-a", "account_unavailable"),
      ],
      policy: defaultFleetAutoPolicy,
    })
    expect(resolution.selection).toBeNull()
    expect(resolution.usedFallback).toBe(true)
    expect(resolution.events).toHaveLength(3)
    expect(resolution.events.at(-1)).toEqual({
      accountRef: "grok-a",
      harnessKind: "grok",
      nextAccountRef: null,
      nextHarnessKind: null,
      type: "account_unavailable",
    })
  })

  it("defaults an unreadied candidate with no reason to account_unavailable", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [
        { accountRef: "codex-a", harnessKind: "codex", marginalCostClass: "not_measured", ready: false },
        ready("claude", "claude-a"),
      ],
      policy: defaultFleetAutoPolicy,
    })
    expect(resolution.events[0]?.type).toBe("account_unavailable")
    expect(resolution.selection?.accountRef).toBe("claude-a")
  })

  it("only evaluates harnesses present in the policy's preferenceOrder", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [notReady("grok", "grok-a", "account_exhausted"), ready("claude", "claude-a")],
      policy: { maxMarginalCostClass: undefined, preferenceOrder: ["claude"], schema: "khala.fleet_auto_policy.v1" },
    })
    // grok is out of policy scope entirely: no event for it, and it can never
    // be selected even though a grok candidate exists in the pool.
    expect(resolution.events).toEqual([])
    expect(resolution.selection).toEqual({
      accountRef: "claude-a",
      harnessKind: "claude",
      marginalCostClass: "not_measured",
    })
  })

  it("prefers the cheaper-cost-class account first within one harness (data-driven bias)", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [
        ready("grok", "grok-metered", "api_metered"),
        ready("grok", "grok-free", "free"),
      ],
      policy: { maxMarginalCostClass: undefined, preferenceOrder: ["grok"], schema: "khala.fleet_auto_policy.v1" },
    })
    expect(resolution.selection?.accountRef).toBe("grok-free")
    expect(resolution.events).toEqual([])
  })

  it("skips a candidate over the policy's maxMarginalCostClass ceiling with a typed event", () => {
    const resolution = resolveFleetAutoTarget({
      candidates: [ready("grok", "grok-metered", "api_metered"), ready("codex", "codex-a", "subscription")],
      policy: {
        maxMarginalCostClass: "subscription",
        preferenceOrder: ["grok", "codex"],
        schema: "khala.fleet_auto_policy.v1",
      },
    })
    expect(resolution.selection).toEqual({
      accountRef: "codex-a",
      harnessKind: "codex",
      marginalCostClass: "subscription",
    })
    expect(resolution.events).toEqual([
      {
        accountRef: "grok-metered",
        harnessKind: "grok",
        nextAccountRef: "codex-a",
        nextHarnessKind: "codex",
        type: "cost_ceiling_exceeded",
      },
    ])
  })

  it("re-ranks selection across the free -> not-free transition using the same resolver (no redesign)", () => {
    const policy = { maxMarginalCostClass: undefined, preferenceOrder: ["grok", "codex"] as const, schema: "khala.fleet_auto_policy.v1" as const }
    const whileFree = resolveFleetAutoTarget({
      candidates: [ready("grok", "grok-a", "free"), ready("codex", "codex-a", "subscription")],
      policy,
    })
    expect(whileFree.selection?.harnessKind).toBe("grok")

    const afterFreeEndsAndAccountExhausted = resolveFleetAutoTarget({
      candidates: [
        notReady("grok", "grok-a", "account_exhausted", "api_metered"),
        ready("codex", "codex-a", "subscription"),
      ],
      policy,
    })
    expect(afterFreeEndsAndAccountExhausted.selection?.harnessKind).toBe("codex")
    expect(afterFreeEndsAndAccountExhausted.events[0]?.type).toBe("account_exhausted")
  })
})
