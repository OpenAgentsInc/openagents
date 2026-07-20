import { describe, expect, test } from "vite-plus/test"

import { decideDelegation, isDelegateProvider, type DelegateProvider } from "./desktop-delegation.ts"
import type { CodexLaneReadiness } from "./desktop-codex-provider.ts"

const routeFor = (candidate: string) =>
  JSON.stringify({
    candidate,
    taskClass: "delegate",
    reasonCode: "needs_delegation",
    confidence: 0.9,
  })

const CODEX_ROUTE = routeFor("codex")
const CLAUDE_ROUTE = routeFor("claude")
const GROK_ROUTE = routeFor("grok_acp")

const ready: CodexLaneReadiness = { ready: true, accountRef: "acct.a" }
const readyMap = (provider: DelegateProvider): Readonly<Partial<Record<DelegateProvider, CodexLaneReadiness>>> => ({
  [provider]: ready,
})

describe("decideDelegation — the host router decision", () => {
  test("an admitted codex recommendation with a ready lane delegates", () => {
    const decision = decideDelegation({ answerText: CODEX_ROUTE, objective: "do the task", readiness: readyMap("codex") })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") {
      expect(decision.provider).toBe("codex")
      expect(decision.objective).toBe("do the task")
      expect(decision.recommendation.candidate).toBe("codex")
    }
  })

  test("an admitted claude recommendation with a ready lane delegates (#9091)", () => {
    const decision = decideDelegation({ answerText: CLAUDE_ROUTE, objective: "task X to claude", readiness: readyMap("claude") })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") {
      expect(decision.provider).toBe("claude")
      expect(decision.objective).toBe("task X to claude")
      expect(decision.recommendation.candidate).toBe("claude")
    }
  })

  test("an admitted grok recommendation with a ready lane delegates (#9091)", () => {
    const decision = decideDelegation({ answerText: GROK_ROUTE, objective: "task X to grok", readiness: readyMap("grok_acp") })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") {
      expect(decision.provider).toBe("grok_acp")
      expect(decision.recommendation.candidate).toBe("grok_acp")
    }
  })

  test("a claude recommendation with an unavailable lane refuses (no start)", () => {
    const decision = decideDelegation({
      answerText: CLAUDE_ROUTE,
      objective: "task X to claude",
      readiness: { claude: { ready: false, unavailableReason: "no_verified_account" } },
    })
    expect(decision.kind).toBe("refuse_delegation")
    if (decision.kind === "refuse_delegation") {
      expect(decision.provider).toBe("claude")
      expect(decision.reason).toBe("provider_unauthorized")
    }
  })

  test("a grok recommendation with no wired lane refuses honestly (no start)", () => {
    // No readiness entry for grok at all → unavailable, never faked.
    const decision = decideDelegation({ answerText: GROK_ROUTE, objective: "x", readiness: {} })
    expect(decision.kind).toBe("refuse_delegation")
    if (decision.kind === "refuse_delegation") {
      expect(decision.provider).toBe("grok_acp")
      expect(decision.reason).toBe("provider_unavailable")
    }
  })

  test("a recommendation for an unavailable lane refuses without delegating (no start)", () => {
    const decision = decideDelegation({
      answerText: CODEX_ROUTE,
      objective: "do the task",
      readiness: { codex: { ready: false, unavailableReason: "no_verified_account" } },
    })
    expect(decision.kind).toBe("refuse_delegation")
    if (decision.kind === "refuse_delegation") expect(decision.reason).toBe("provider_unauthorized")
  })

  test("policy denial maps to an unadmitted refusal", () => {
    const decision = decideDelegation({
      answerText: CODEX_ROUTE,
      objective: "x",
      readiness: { codex: { ready: false, unavailableReason: "policy_denied" } },
    })
    expect(decision.kind).toBe("refuse_delegation")
    if (decision.kind === "refuse_delegation") expect(decision.reason).toBe("provider_unadmitted")
  })

  test("a plain-prose answer never delegates", () => {
    const decision = decideDelegation({
      answerText: "The capital of France is Paris.",
      objective: "capital of france",
      readiness: readyMap("codex"),
    })
    expect(decision.kind).toBe("answer")
    if (decision.kind === "answer") expect(decision.text).toBe("The capital of France is Paris.")
  })

  test("a malformed structured route never delegates (fail-closed)", () => {
    const decision = decideDelegation({
      answerText: '{"candidate": "codex", "confidence": "high"}',
      objective: "x",
      readiness: readyMap("codex"),
    })
    // A broken structured route is not a valid recommendation → it does not dispatch.
    expect(decision.kind).not.toBe("delegate")
  })

  test("an action-claim output never delegates", () => {
    const decision = decideDelegation({
      answerText: JSON.stringify({ candidate: "codex", command: "rm -rf /", confidence: 0.9 }),
      objective: "x",
      readiness: readyMap("codex"),
    })
    expect(decision.kind).not.toBe("delegate")
  })

  test("a recommendation for a non-delegate lane answers (never dispatches)", () => {
    // `cursor_acp` is a candidate but not a host-dispatchable delegate lane.
    const decision = decideDelegation({
      answerText: JSON.stringify({
        candidate: "cursor_acp",
        taskClass: "delegate",
        reasonCode: "needs_delegation",
        confidence: 0.8,
      }),
      objective: "x",
      admittedDelegates: ["codex", "claude", "grok_acp"],
      readiness: readyMap("codex"),
    })
    expect(decision.kind).toBe("answer")
  })

  test("a null answer text answers empty (never delegates)", () => {
    const decision = decideDelegation({ answerText: null, objective: "x", readiness: readyMap("codex") })
    expect(decision.kind).toBe("answer")
  })

  test("isDelegateProvider recognizes exactly the dispatchable lanes", () => {
    expect(isDelegateProvider("codex")).toBe(true)
    expect(isDelegateProvider("claude")).toBe(true)
    expect(isDelegateProvider("grok_acp")).toBe(true)
    expect(isDelegateProvider("apple_fm")).toBe(false)
    expect(isDelegateProvider("cursor_acp")).toBe(false)
  })
})
