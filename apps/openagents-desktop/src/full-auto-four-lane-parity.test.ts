import { describe, expect, test } from "vite-plus/test"

import { Schema } from "effect"

import type { DesktopThread } from "./chat-contract.ts"
import { fullAutoLanePolicy, FULL_AUTO_LANE_POLICIES } from "./full-auto-lane.ts"
import {
  buildProviderHandoffEnvelope,
  ProviderHandoffEnvelopeSchema,
  providerHandoffDispositionForEnvelope,
} from "./full-auto-provider-handoff.ts"
import { classifyFullAutoDispatchFailure } from "./full-auto-reconcile.ts"
import { validateFullAutoRoutingPolicy, type FullAutoRoutingLaneGate } from "./full-auto-routing.ts"
import type { FullAutoRun } from "./full-auto-run-registry.ts"

/**
 * FAV-02 (#9112): four-lane rotation parity. The owner-real matrix proved
 * Codex<->Claude. These parity tests prove the SAME handoff-envelope and
 * routing invariants hold for Grok and Cursor — the two lanes that were
 * eligible but unproven — across every ordered lane pair. The live
 * owner-real receipt rows (a real Grok/Cursor account writing a fact another
 * provider continues) are the activation rung and need admitted ACP peers.
 */

const LANES = ["codex-local", "claude-local", "acp:grok-cli", "acp:cursor-agent"] as const

const allReadyGate: FullAutoRoutingLaneGate = (lane) =>
  (LANES as ReadonlyArray<string>).includes(lane) ? { admitted: true, fullAuto: true } : null

const makeThread = (
  notes: ReadonlyArray<Readonly<{ role: "user" | "assistant" | "system"; text: string }>>,
): DesktopThread => ({
  id: "thread.parity",
  title: "Parity thread",
  updatedAt: "2026-07-20T00:00:00.000Z",
  notes: notes.map((note, index) => ({
    key: `note-${index}`,
    role: note.role,
    text: note.text,
    timestamp: `2026-07-20T00:00:${String(index).padStart(2, "0")}.000Z`,
  })),
})

const makeRun = (): FullAutoRun => ({
  runRef: "run.full-auto.parity-1",
  threadRef: "thread.parity",
  title: "Parity run",
  objective: "Do the one next useful thing in this repo.",
  objectiveSource: "user",
  doneCondition: "The named verification passes.",
  objectiveHistory: [],
  turnCap: 20,
  successfulAttempts: 0,
  failedAttempts: 0,
  state: "paused",
  stateRevision: 2,
  createdAt: "2026-07-20T00:00:00.000Z",
  transitions: [],
})

const orderedPairs = (): ReadonlyArray<readonly [string, string]> => {
  const pairs: Array<readonly [string, string]> = []
  for (const source of LANES) for (const target of LANES) if (source !== target) pairs.push([source, target])
  return pairs
}

describe("four-lane eligibility parity", () => {
  test("all four lanes are Full-Auto-eligible with safe background-question settlement", () => {
    for (const lane of LANES) {
      const policy = fullAutoLanePolicy(lane)
      expect(policy).not.toBeNull()
      expect(policy?.autoResolveQuestions).toBe(true)
    }
    // The policy set is exactly the seven fleet lanes — the four proven above
    // plus the three host-run SDK-harness lanes (#9187) — no silent extra or
    // missing lane. The harness-lane eligibility is asserted directly below.
    expect(Object.keys(FULL_AUTO_LANE_POLICIES).sort()).toEqual(
      ["acp:cursor-agent", "acp:grok-cli", "claude-local", "codex-local", "harness:goose", "harness:opencode", "harness:pi"],
    )
  })

  test("the three host-run SDK-harness lanes are Full-Auto-eligible with safe background-question settlement (#9187)", () => {
    for (const lane of ["harness:opencode", "harness:pi", "harness:goose"]) {
      const policy = fullAutoLanePolicy(lane)
      expect(policy).not.toBeNull()
      // The background-question invariant: each harness settles without a
      // renderer, so a background turn never parks forever waiting for input.
      expect(policy?.autoResolveQuestions).toBe(true)
    }
  })

  test("an ordered routing policy over all four lanes is admitted, order preserved", () => {
    const forward = validateFullAutoRoutingPolicy(
      LANES.map((lane) => ({ lane })),
      allReadyGate,
    )
    expect(forward.ok).toBe(true)
    if (forward.ok) expect(forward.policy.map((c) => c.lane)).toEqual([...LANES])

    // A Grok/Cursor-first order is equally admitted — no lane is second-class.
    const reordered = ["acp:cursor-agent", "acp:grok-cli", "claude-local", "codex-local"].map((lane) => ({ lane }))
    expect(validateFullAutoRoutingPolicy(reordered, allReadyGate).ok).toBe(true)
  })
})

describe("handoff-envelope parity across every ordered lane pair", () => {
  test("every pair preserves lane refs and always carries the provider-private omission", () => {
    for (const [sourceLaneRef, targetLaneRef] of orderedPairs()) {
      const envelope = buildProviderHandoffEnvelope({
        run: makeRun(),
        sourceLaneRef,
        targetLaneRef,
        thread: makeThread([
          { role: "user", text: "Do the thing." },
          { role: "assistant", text: "Doing it." },
        ]),
        reason: "rotation on typed failure",
        actor: "control_api",
        at: "2026-07-20T01:00:00.000Z",
      })
      const decoded = Schema.decodeUnknownSync(ProviderHandoffEnvelopeSchema)(envelope)
      expect(decoded.sourceLaneRef).toBe(sourceLaneRef)
      expect(decoded.targetLaneRef).toBe(targetLaneRef)
      // The host-owned invariant holds for every pair, ACP lanes included.
      expect(
        decoded.omissions.some((o) => o.reason === "provider_private_never_transferred"),
      ).toBe(true)
      // Only host-owned bounded projection transfers — the objective is the
      // run's, and recent context came from the thread, not the provider.
      expect(decoded.objective).toBe("Do the one next useful thing in this repo.")
      expect(decoded.recentContext.length).toBeGreaterThan(0)
    }
  })

  test("an ACP-lane handoff discloses truncation identically to the Codex/Claude path", () => {
    const longThread = makeThread(
      Array.from({ length: 60 }, (_, i) => ({
        role: (i % 2 === 0 ? "user" : "assistant") as "user" | "assistant",
        text: `message ${i} ${"x".repeat(2000)}`,
      })),
    )
    // Grok -> Cursor, the pair with no prior proof.
    const acp = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "acp:grok-cli",
      targetLaneRef: "acp:cursor-agent",
      thread: longThread,
      reason: "rotation",
      actor: "control_api",
      at: "2026-07-20T01:00:00.000Z",
    })
    expect(acp.contextTruncated).toBe(true)
    expect(acp.omissions.some((o) => o.reason === "bounded_truncation")).toBe(true)
    expect(providerHandoffDispositionForEnvelope(acp)).toBe("truncated_with_confirmation")

    // Same input shape on the proven Codex->Claude pair yields the same disposition.
    const codexClaude = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread: longThread,
      reason: "rotation",
      actor: "control_api",
      at: "2026-07-20T01:00:00.000Z",
    })
    expect(providerHandoffDispositionForEnvelope(acp)).toBe(
      providerHandoffDispositionForEnvelope(codexClaude),
    )
  })

  test("no envelope carries provider-private material for any lane pair", () => {
    for (const [sourceLaneRef, targetLaneRef] of orderedPairs()) {
      const envelope = buildProviderHandoffEnvelope({
        run: makeRun(),
        sourceLaneRef,
        targetLaneRef,
        thread: makeThread([{ role: "assistant", text: "state" }]),
        reason: "rotation",
        actor: "owner_ui",
        at: "2026-07-20T01:00:00.000Z",
      })
      const serialized = JSON.stringify(envelope)
      // The envelope is a bounded projection — never a credential/session dump.
      expect(serialized).not.toMatch(/authToken|api[_-]?key|credential[^s]|nsec|mnemonic/i)
    }
  })
})

describe("rotation-reason parity is lane-agnostic", () => {
  test("the typed rotation vocabulary is the same three reasons regardless of lane", () => {
    // Canonical typed reasons pass through directly for any lane's adapter.
    for (const reason of ["account_exhausted", "rate_limited", "provider_error"] as const) {
      expect(classifyFullAutoDispatchFailure(reason)).toBe(reason)
    }
    // Per-provider account exhaustion maps to the same typed reason.
    expect(classifyFullAutoDispatchFailure("no_codex_account")).toBe("account_exhausted")
    expect(classifyFullAutoDispatchFailure("no_claude_account")).toBe("account_exhausted")
    // A rate-limit detail on a session failure classifies as rate_limited for
    // any lane; a generic session failure stays provider_error.
    expect(classifyFullAutoDispatchFailure("session_failed", "rate limit exceeded")).toBe(
      "rate_limited",
    )
    expect(classifyFullAutoDispatchFailure("session_failed", "some other failure")).toBe(
      "provider_error",
    )
  })
})
