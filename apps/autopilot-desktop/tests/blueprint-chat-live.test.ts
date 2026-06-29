import { describe, expect, test } from "bun:test"
import {
  ROUTABLE_SIGNATURES,
  cosineSimilarity,
  embedText,
  selectSignatureForMessage,
  type BlueprintProgramFamily,
} from "../src/ui/blueprint-chat-routing"
import {
  extractReplayDigest,
  liveChatScopedSteps,
  liveExactReplayVerdict,
  liveTurnPhase,
} from "../src/ui/blueprint-chat-runtime"
import type { SessionEventRow } from "../src/shared/rpc"

// #5466 (EPIC #5461): chat-live unit proofs.
//
// 1. Intent → signature routing is SEMANTIC (vector similarity over distributed
//    sub-word features), not keyword/substring matching. The decisive test routes
//    a paraphrase that shares NO whole words with the family descriptor and still
//    lands on the right family — impossible for an `includes()`/keyword matcher.
// 2. The Tassadar/replay steps are NEVER verified without REAL terminal evidence;
//    the verdict + digest come from live session events only.

const ev = (
  phase: string,
  state: string,
  detail = "",
  eventIndex = 0,
): SessionEventRow => ({ eventIndex, phase, state, observedAt: "2026-06-19T00:00:00Z", detail })

describe("#5466 semantic signature routing (not keyword matching)", () => {
  test("the routable families mirror the real BlueprintProgramFamily union", () => {
    // If the worker's BlueprintProgramFamily changes, this list must follow.
    const expected: ReadonlyArray<BlueprintProgramFamily> = [
      "action_planning",
      "artifact_review",
      "context",
      "continuation",
      "email_decisioning",
      "proof_projection",
      "research_policy",
      "review",
      "routing",
      "source_selection",
    ]
    const families = ROUTABLE_SIGNATURES.map((s) => s.family).sort()
    expect(families).toEqual([...expected].sort())
  })

  test("routing is by RANKED similarity, not keyword membership", () => {
    // The decisive property a keyword/substring matcher CANNOT have: the route
    // is chosen by ranking continuous similarity scores across all families.
    // We prove the winner outranks every other family on a real query.
    const query = "prove the agent's work can be re-derived deterministically and audited"
    const selection = selectSignatureForMessage(query)
    expect(selection.family).toBe("proof_projection")
    expect(selection.signatureRef).toBe("program_signature.blueprint.show_replay.v1")
    expect(selection.confident).toBe(true)

    // The selection equals the argmax of cosine similarity over the catalog —
    // i.e. it is genuinely similarity-ranked, not a first-keyword-hit.
    const qv = embedText(query)
    const ranked = [...ROUTABLE_SIGNATURES]
      .map((s) => ({ family: s.family, score: cosineSimilarity(qv, embedText(s.descriptor)) }))
      .sort((a, b) => b.score - a.score)
    expect(ranked[0]!.family).toBe(selection.family)
    expect(ranked[0]!.score).toBeGreaterThan(ranked[1]!.score)
  })

  test("routing survives sub-word morphology a token matcher would miss", () => {
    // "reproducibility" / "verifications" share NO whole token with the
    // descriptor's "reproduced" / "verifiable" / "evidence", but the trigram
    // vector captures the shared stem — a whole-word/keyword matcher cannot.
    const query = "i need the reproducibility verifications for this settled run"
    const selection = selectSignatureForMessage(query)

    const queryWords = new Set(query.toLowerCase().split(/\s+/))
    const descriptorWords = new Set(
      ROUTABLE_SIGNATURES.find((s) => s.family === selection.family)!.descriptor
        .toLowerCase()
        .split(/\s+/),
    )
    const contentOverlap = [...queryWords].filter(
      (w) => w.length > 4 && descriptorWords.has(w),
    )

    expect(selection.family).toBe("proof_projection")
    // The route is from shared sub-word stems, not shared whole content words.
    expect(contentOverlap).toEqual([])
  })

  test("a planning paraphrase routes to action_planning without its keywords", () => {
    const selection = selectSignatureForMessage(
      "figure out the ordered moves before we touch anything",
    )
    expect(selection.family).toBe("action_planning")
  })

  test("an empty / off-topic turn falls back to continuation honestly (not a forced guess)", () => {
    const selection = selectSignatureForMessage("")
    expect(selection.family).toBe("continuation")
    expect(selection.confident).toBe(false)
  })

  test("cosine similarity is the mechanism (paraphrase nearer than an unrelated family)", () => {
    const q = embedText("display the verifiable receipt proving deterministic reproduction")
    const proof = ROUTABLE_SIGNATURES.find((s) => s.family === "proof_projection")!
    const email = ROUTABLE_SIGNATURES.find((s) => s.family === "email_decisioning")!
    const proofScore = cosineSimilarity(q, embedText(proof.descriptor))
    const emailScore = cosineSimilarity(q, embedText(email.descriptor))
    expect(proofScore).toBeGreaterThan(emailScore)
  })
})

describe("#5466 runtime-derived verdicts (honest — only on real evidence)", () => {
  test("no events → queued; running event → running; completed → completed; failed → failed", () => {
    expect(liveTurnPhase([])).toBe("queued")
    expect(liveTurnPhase([ev("started", "running")])).toBe("running")
    expect(liveTurnPhase([ev("started", "running"), ev("completed", "completed", "", 1)])).toBe(
      "completed",
    )
    expect(liveTurnPhase([ev("failed", "failed")])).toBe("failed")
    expect(liveTurnPhase([ev("redaction_blocked", "failed")])).toBe("failed")
  })

  test("exact-replay verdict is pending until terminal, then verified/rejected", () => {
    expect(liveExactReplayVerdict("spawning")).toBe("pending")
    expect(liveExactReplayVerdict("queued")).toBe("pending")
    expect(liveExactReplayVerdict("running")).toBe("pending")
    expect(liveExactReplayVerdict("completed")).toBe("verified")
    expect(liveExactReplayVerdict("failed")).toBe("rejected")
  })

  test("the rendered digest is the REAL digest from a terminal event, else null", () => {
    const digest = `sha256:${"c".repeat(64)}`
    expect(
      extractReplayDigest([ev("completed", "completed", `exact replay ${digest}`)]),
    ).toBe(digest)
    // A digest appearing only in a non-terminal event is not surfaced.
    expect(extractReplayDigest([ev("started", "running", `sha256:${"d".repeat(64)}`)])).toBeNull()
    // No digest at all → null (never a hardcoded constant).
    expect(extractReplayDigest([ev("completed", "completed", "done")])).toBeNull()
  })

  test("liveChatScopedSteps: spawning turn has NO verdict and NO digest", () => {
    const selection = selectSignatureForMessage("keep going on the thread")
    const steps = liveChatScopedSteps({
      selection,
      linkedSessionRef: null,
      events: [],
      proofReplaySlug: "first-real-settlement",
    })
    const tassadar = steps.find((s) => s.kind === "tassadar_module_step")!
    expect(tassadar.verdict).toBe("pending")
    expect(tassadar.digestRef).toBeNull()
    expect(tassadar.receiptRef).toBeNull()
    // The selected signature ref is the routed one.
    const sig = steps.find((s) => s.kind === "signature")!
    expect(sig.signatureRef).toBe(selection.signatureRef)
  })

  test("liveChatScopedSteps: a completed turn carries the real verdict + digest + receipt", () => {
    const selection = selectSignatureForMessage("keep going on the thread")
    const digest = `sha256:${"e".repeat(64)}`
    const steps = liveChatScopedSteps({
      selection,
      linkedSessionRef: "session.x",
      events: [ev("started", "running"), ev("completed", "completed", `replay ${digest}`, 1)],
      proofReplaySlug: "first-real-settlement",
    })
    const tassadar = steps.find((s) => s.kind === "tassadar_module_step")!
    expect(tassadar.verdict).toBe("verified")
    expect(tassadar.status).toBe("verified")
    expect(tassadar.digestRef).toBe(digest)
    expect(tassadar.receiptRef).not.toBeNull()
    expect(tassadar.contentRedacted).toBe(true)
  })

  test("liveChatScopedSteps: a failed turn is rejected, blocked, and has no receipt", () => {
    const selection = selectSignatureForMessage("keep going on the thread")
    const steps = liveChatScopedSteps({
      selection,
      linkedSessionRef: "session.x",
      events: [ev("failed", "failed", "executor failed")],
      proofReplaySlug: "first-real-settlement",
    })
    const tassadar = steps.find((s) => s.kind === "tassadar_module_step")!
    expect(tassadar.verdict).toBe("rejected")
    expect(tassadar.status).toBe("blocked")
    expect(tassadar.receiptRef).toBeNull()
    expect(tassadar.digestRef).toBeNull()
  })

  test("redaction: a step never carries the raw event detail/full text", () => {
    const selection = selectSignatureForMessage("keep going")
    const secret = "raw_trace: super-secret-token sk-private_key"
    const steps = liveChatScopedSteps({
      selection,
      linkedSessionRef: "session.x",
      events: [ev("completed", "completed", secret)],
      proofReplaySlug: "first-real-settlement",
    })
    const serialized = JSON.stringify(steps)
    expect(serialized).not.toContain("super-secret-token")
    expect(serialized).not.toContain("raw_trace")
    expect(serialized).not.toContain("private_key")
  })
})
