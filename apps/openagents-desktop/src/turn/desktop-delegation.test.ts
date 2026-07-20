import { describe, expect, test } from "vite-plus/test"

import { decideDelegation } from "./desktop-delegation.ts"
import type { CodexLaneReadiness } from "./desktop-codex-provider.ts"

const CODEX_ROUTE = JSON.stringify({
  candidate: "codex",
  taskClass: "delegate",
  reasonCode: "needs_delegation",
  confidence: 0.9,
})

const ready: CodexLaneReadiness = { ready: true, accountRef: "acct.a" }

describe("decideDelegation — the host router decision", () => {
  test("an admitted codex recommendation with a ready lane delegates", () => {
    const decision = decideDelegation({ answerText: CODEX_ROUTE, objective: "do the task", codexReadiness: ready })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") {
      expect(decision.provider).toBe("codex")
      expect(decision.objective).toBe("do the task")
      expect(decision.recommendation.candidate).toBe("codex")
    }
  })

  test("a recommendation for an unavailable lane refuses without delegating (no start)", () => {
    const decision = decideDelegation({
      answerText: CODEX_ROUTE,
      objective: "do the task",
      codexReadiness: { ready: false, unavailableReason: "no_verified_account" },
    })
    expect(decision.kind).toBe("refuse_delegation")
    if (decision.kind === "refuse_delegation") expect(decision.reason).toBe("provider_unauthorized")
  })

  test("policy denial maps to an unadmitted refusal", () => {
    const decision = decideDelegation({
      answerText: CODEX_ROUTE,
      objective: "x",
      codexReadiness: { ready: false, unavailableReason: "policy_denied" },
    })
    expect(decision.kind).toBe("refuse_delegation")
    if (decision.kind === "refuse_delegation") expect(decision.reason).toBe("provider_unadmitted")
  })

  test("a plain-prose answer never delegates", () => {
    const decision = decideDelegation({
      answerText: "The capital of France is Paris.",
      objective: "capital of france",
      codexReadiness: ready,
    })
    expect(decision.kind).toBe("answer")
    if (decision.kind === "answer") expect(decision.text).toBe("The capital of France is Paris.")
  })

  test("a malformed structured route never delegates (fail-closed)", () => {
    const decision = decideDelegation({
      answerText: '{"candidate": "codex", "confidence": "high"}',
      objective: "x",
      codexReadiness: ready,
    })
    // A broken structured route is not a valid recommendation → it does not dispatch.
    expect(decision.kind).not.toBe("delegate")
  })

  test("an action-claim output never delegates", () => {
    const decision = decideDelegation({
      answerText: JSON.stringify({ candidate: "codex", command: "rm -rf /", confidence: 0.9 }),
      objective: "x",
      codexReadiness: ready,
    })
    expect(decision.kind).not.toBe("delegate")
  })

  test("a recommendation for a non-codex lane answers in Phase 1", () => {
    const decision = decideDelegation({
      answerText: JSON.stringify({
        candidate: "claude",
        taskClass: "delegate",
        reasonCode: "needs_delegation",
        confidence: 0.8,
      }),
      objective: "x",
      admittedDelegates: ["codex", "claude"],
      codexReadiness: ready,
    })
    expect(decision.kind).toBe("answer")
  })

  test("a null answer text answers empty (never delegates)", () => {
    const decision = decideDelegation({ answerText: null, objective: "x", codexReadiness: ready })
    expect(decision.kind).toBe("answer")
  })
})
