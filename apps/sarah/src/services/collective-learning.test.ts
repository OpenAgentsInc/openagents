/**
 * KHS-4 owner-approved collective learning oracles (#8603, epic #8599).
 *
 * These are the enforcement oracles for contract
 * `sarah.collective_learning_owner_gated.v1` (registered in
 * src/contracts/isolation-contracts.ts; human doc
 * docs/sarah/SARAH_CONTRACTS.md):
 *
 * - approved_store_only.unit — the shared-knowledge read path
 *   (listApprovedLearnings + the KHS-6 answer bank) returns ONLY
 *   owner-approved entries, every one carrying an approval receipt;
 *   pending/rejected candidates are unreachable from any serve path.
 * - learning_admin_guard.rpc — approve/reject/distill/list over HTTP are
 *   admin-bearer-guarded and fail CLOSED: unarmed → 503 (an approve without
 *   the guard is impossible), missing/wrong bearer → 401.
 * - learning_pii_redaction.unit — examples with emails/phones/names are
 *   scrubbed or dropped before they can enter a candidate.
 *
 * Hermetic: no database (memory_ephemeral store), no network (embedder
 * unarmed → deterministic normalized-text grouping), injected turns.
 */

import { afterEach, beforeAll, afterAll, describe, expect, test } from "bun:test"

import {
  __resetLearningRegressionFixturesForTest,
  __resetSarahCollectiveLearningForTest,
  __setSarahLearningTurnsForTest,
  approveLearningCandidate,
  buildWhyGeneralize,
  computeSourceRecency,
  distillLearningCandidates,
  isMaterialStyleChange,
  learningReceiptRef,
  listApprovedLearnings,
  listLearningCandidates,
  listLearningReceipts,
  listLearningRegressionFixtures,
  looksLikeObjection,
  looksLikeQuestion,
  normalizeLearningText,
  redactLearningExample,
  rejectLearningCandidate,
  sarahLearningStoreMode,
  taxonomyForKind,
  type LearningTurnRow,
} from "./collective-learning.ts"
import {
  __resetSarahAnswerCacheForTest,
  __setSarahAnswerBankForTest,
  listSarahAnswerBank,
} from "./semantic-answer-cache.ts"

const savedEnv: Record<string, string | undefined> = {}
const ENV_KEYS = [
  "SARAH_DATABASE_URL",
  "KHALA_SYNC_DATABASE_URL",
  "GEMINI_API_KEY",
  "SARAH_OPERATOR_ADMIN_TOKEN",
  "OPENAGENTS_ADMIN_API_TOKEN",
  "SARAH_SEMANTIC_CACHE",
] as const

beforeAll(() => {
  for (const key of ENV_KEYS) {
    savedEnv[key] = process.env[key]
    delete process.env[key]
  }
})

afterAll(() => {
  for (const key of ENV_KEYS) {
    if (savedEnv[key] === undefined) delete process.env[key]
    else process.env[key] = savedEnv[key]
  }
})

afterEach(() => {
  __resetSarahCollectiveLearningForTest()
  __resetSarahAnswerCacheForTest()
  __resetLearningRegressionFixturesForTest()
  delete process.env.SARAH_OPERATOR_ADMIN_TOKEN
  delete process.env.OPENAGENTS_ADMIN_API_TOKEN
})

let turnId = 0
function turn(
  prospectRef: string,
  role: "user" | "assistant",
  text: string,
): LearningTurnRow {
  turnId += 1
  return {
    id: `t${turnId}`,
    prospectRef,
    role,
    text,
    recordedAt: new Date(1750000000000 + turnId * 1000).toISOString(),
  }
}

// ---------------------------------------------------------------------------
// Oracle learning_pii_redaction.unit — PII never enters candidates
// (contract sarah.collective_learning_owner_gated.v1)
// ---------------------------------------------------------------------------

describe("PII redaction — redact or drop, never keep", () => {
  test("emails are scrubbed", () => {
    const out = redactLearningExample(
      "Can you email the deck to jane.doe+work@bigco.example.com today?",
    )
    expect(out).not.toBeNull()
    expect(out!).toContain("[redacted-email]")
    expect(out!).not.toContain("jane.doe")
    expect(out!).not.toContain("bigco.example.com")
  })

  test("phone numbers and long digit runs are scrubbed", () => {
    const out = redactLearningExample(
      "Call me back at +1 (555) 010-9922, account 99887766 needs review.",
    )
    expect(out).not.toBeNull()
    expect(out!).not.toMatch(/555|010-9922|99887766/)
    expect(out!).toMatch(/\[redacted-(?:phone|number)\]/)
  })

  test("name introductions drop the whole example", () => {
    expect(
      redactLearningExample("Hi, my name is Alice and we need automation."),
    ).toBeNull()
    expect(redactLearningExample("I'm Chris from the ops team.")).toBeNull()
    expect(redactLearningExample("You can call me Bob.")).toBeNull()
  })

  test("leftover @-residue after scrubbing drops the example", () => {
    expect(redactLearningExample("ping me @ jane at bigco")).toBeNull()
  })

  test("clean product questions pass through intact", () => {
    const out = redactLearningExample("How does the Khala API handle retries?")
    expect(out).toBe("How does the Khala API handle retries?")
  })

  test("URLs are scrubbed", () => {
    const out = redactLearningExample(
      "We compared you against https://competitor.example.com/pricing-page",
    )
    expect(out).not.toBeNull()
    expect(out!).toContain("[redacted-url]")
    expect(out!).not.toContain("competitor.example.com")
  })

  test("distilled candidates carry only redacted examples", async () => {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "Can you integrate with reach-me@corp-a.example? How does onboarding work?"),
      turn("prospect:b", "user", "Can you integrate with admin@corp-b.example? How does onboarding work?"),
    ])
    const result = await distillLearningCandidates()
    expect(result.ok).toBe(true)
    const pending = await listLearningCandidates("pending")
    expect(pending.length).toBeGreaterThan(0)
    for (const candidate of pending) {
      const everything = [
        candidate.summary,
        candidate.questionCanonical ?? "",
        candidate.proposedAnswer ?? "",
        ...candidate.redactedExamples,
      ].join("\n")
      expect(everything).not.toContain("corp-a.example")
      expect(everything).not.toContain("corp-b.example")
      expect(everything).not.toContain("reach-me")
    }
  })
})

// ---------------------------------------------------------------------------
// Deterministic distillation
// ---------------------------------------------------------------------------

describe("distillLearningCandidates — deterministic v1", () => {
  test("questions recurring across two prospects become one pending question_gap", async () => {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "How does the Khala API handle rate limits?"),
      turn("prospect:b", "user", "How does the Khala API handle rate limits?"),
      turn("prospect:a", "user", "Do you support on-prem deployments?"), // single prospect
    ])
    const result = await distillLearningCandidates()
    expect(result.ok).toBe(true)
    const pending = await listLearningCandidates("pending")
    const gaps = pending.filter((c) => c.kind === "question_gap")
    expect(gaps.length).toBe(1)
    expect(gaps[0]!.status).toBe("pending")
    expect(gaps[0]!.sourceTurnIds.length).toBe(2)
    expect(gaps[0]!.questionCanonical).toContain("rate limits")
  })

  test("pricing-guard questions never become candidates", async () => {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "Can I get a discount on the annual plan?"),
      turn("prospect:b", "user", "Can I get a discount on the annual plan?"),
    ])
    await distillLearningCandidates()
    expect(await listLearningCandidates()).toEqual([])
  })

  test("objection cues recurring across prospects become an objection candidate", async () => {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "Honestly this feels too risky for our compliance team."),
      turn("prospect:b", "user", "Honestly this feels too risky for our compliance team."),
    ])
    await distillLearningCandidates()
    const pending = await listLearningCandidates("pending")
    expect(pending.some((c) => c.kind === "objection")).toBe(true)
  })

  test("question→answer→positive-ack becomes a winning_answer candidate with the pair", async () => {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "How do agents report their work back to us?"),
      turn(
        "prospect:a",
        "assistant",
        "Every agent action produces a signed receipt you can audit in the dashboard, with full traceability.",
      ),
      turn("prospect:a", "user", "That makes sense, thanks!"),
    ])
    await distillLearningCandidates()
    const pending = await listLearningCandidates("pending")
    const winners = pending.filter((c) => c.kind === "winning_answer")
    expect(winners.length).toBe(1)
    expect(winners[0]!.questionCanonical).toContain("report their work")
    expect(winners[0]!.proposedAnswer).toContain("signed receipt")
    expect(winners[0]!.sourceTurnIds.length).toBe(3)
  })

  test("re-running distillation creates no duplicates", async () => {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "How does the Khala API handle rate limits?"),
      turn("prospect:b", "user", "How does the Khala API handle rate limits?"),
    ])
    const first = await distillLearningCandidates()
    expect(first.created).toBe(1)
    const second = await distillLearningCandidates()
    expect(second.created).toBe(0)
    expect((await listLearningCandidates()).length).toBe(1)
  })

  test("cue helpers are deterministic nomination only", () => {
    expect(looksLikeQuestion("How does billing work?")).toBe(true)
    expect(looksLikeQuestion("ok")).toBe(false)
    expect(looksLikeObjection("we already use a competitor")).toBe(true)
    expect(looksLikeObjection("sounds great")).toBe(false)
    expect(normalizeLearningText("  How does BILLING work?! ")).toBe(
      "how does billing work",
    )
  })
})

// ---------------------------------------------------------------------------
// Oracle approved_store_only.unit — the shared read path serves ONLY
// owner-approved entries with receipts
// (contract sarah.collective_learning_owner_gated.v1)
// ---------------------------------------------------------------------------

describe("contract sarah.collective_learning_owner_gated.v1 — approved store only", () => {
  async function seedTwoPendingCandidates() {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      // Candidate 1 (will be approved): winning answer triple.
      turn("prospect:a", "user", "How do agents report their work back to us?"),
      turn(
        "prospect:a",
        "assistant",
        "Every agent action produces a signed receipt you can audit in the dashboard, with full traceability.",
      ),
      turn("prospect:a", "user", "Perfect, that helps."),
      // Candidate 2 (stays pending): recurring question.
      turn("prospect:b", "user", "Do you offer a self-hosted deployment option?"),
      turn("prospect:c", "user", "Do you offer a self-hosted deployment option?"),
    ])
    const result = await distillLearningCandidates()
    expect(result.ok).toBe(true)
    const pending = await listLearningCandidates("pending")
    const winner = pending.find((c) => c.kind === "winning_answer")
    const gap = pending.find((c) => c.kind === "question_gap")
    expect(winner).toBeDefined()
    expect(gap).toBeDefined()
    return { winner: winner!, gap: gap! }
  }

  test("nothing generalizes without an approval receipt; only approved entries are readable", async () => {
    const { winner, gap } = await seedTwoPendingCandidates()

    // Before any decision the shared read path is empty.
    expect(await listApprovedLearnings()).toEqual([])

    const approval = await approveLearningCandidate({
      id: winner.id,
      by: "owner:chris",
    })
    expect(approval.ok).toBe(true)
    if (!approval.ok) return

    // The shared path returns ONLY the approved candidate, receipt attached.
    const shared = await listApprovedLearnings()
    expect(shared.length).toBe(1)
    expect(shared[0]!.id).toBe(winner.id)
    expect(shared[0]!.status).toBe("approved")
    expect(shared[0]!.receiptId).toBe(approval.receipt.id)
    // The pending candidate is unreachable from the shared path.
    expect(shared.some((c) => c.id === gap.id)).toBe(false)

    // Receipt row recorded with the decider.
    const receipts = await listLearningReceipts()
    expect(receipts.length).toBe(1)
    expect(receipts[0]!.decision).toBe("approved")
    expect(receipts[0]!.decidedBy).toBe("owner:chris")
    expect(receipts[0]!.candidateId).toBe(winner.id)
  })

  test("a live answer-bank entry dereferences back to its approval receipt", async () => {
    const { winner } = await seedTwoPendingCandidates()
    const approval = await approveLearningCandidate({
      id: winner.id,
      by: "owner:chris",
    })
    expect(approval.ok).toBe(true)
    if (!approval.ok) return
    expect(approval.bankEntryId).toBe(`learned_${winner.id}`)

    const bank = await listSarahAnswerBank()
    const entry = bank.find((e) => e.id === approval.bankEntryId)
    expect(entry).toBeDefined()
    // approved_by IS the receipt ref — live answer → receipt → redacted source.
    expect(entry!.approvedBy).toBe(learningReceiptRef(approval.receipt.id))
    expect(entry!.answer).toContain("signed receipt")
  })

  test("question_gap approval publishes to the bank only with an owner-supplied answer", async () => {
    const { gap } = await seedTwoPendingCandidates()
    const approval = await approveLearningCandidate({
      id: gap.id,
      by: "owner:chris",
      answerText:
        "Yes — self-hosted deployments are available on the enterprise plan; the team walks you through setup.",
    })
    expect(approval.ok).toBe(true)
    if (!approval.ok) return
    expect(approval.bankEntryId).toBe(`learned_${gap.id}`)
    const bank = await listSarahAnswerBank()
    expect(bank.some((e) => e.id === approval.bankEntryId)).toBe(true)
  })

  test("rejections are recorded and never reach the shared path or the bank", async () => {
    const { winner, gap } = await seedTwoPendingCandidates()
    const rejection = await rejectLearningCandidate({
      id: gap.id,
      by: "owner:chris",
      reason: "not general enough",
    })
    expect(rejection.ok).toBe(true)

    expect(await listApprovedLearnings()).toEqual([])
    expect((await listSarahAnswerBank()).length).toBe(0)

    const receipts = await listLearningReceipts()
    expect(receipts.length).toBe(1)
    expect(receipts[0]!.decision).toBe("rejected")
    expect(receipts[0]!.reason).toBe("not general enough")

    // A decided candidate cannot be re-decided.
    expect((await rejectLearningCandidate({ id: gap.id, by: "x" })).ok).toBe(false)
    expect(
      (await approveLearningCandidate({ id: gap.id, by: "x" })).ok,
    ).toBe(false)
    // The other candidate is untouched.
    const pending = await listLearningCandidates("pending")
    expect(pending.some((c) => c.id === winner.id)).toBe(true)
  })

  test("hermetic runs use the ephemeral store (no database configured)", () => {
    expect(sarahLearningStoreMode()).toBe("memory_ephemeral")
  })
})

// ---------------------------------------------------------------------------
// Oracle learning_admin_guard.rpc — admin bearer mandatory on the HTTP
// surface; fail closed (contract sarah.collective_learning_owner_gated.v1)
// ---------------------------------------------------------------------------

describe("contract sarah.collective_learning_owner_gated.v1 — admin guard on the routes", () => {
  const post = (path: string, headers: Record<string, string> = {}, body?: unknown) =>
    new Request(`http://localhost/sarah${path}`, {
      method: "POST",
      headers: { "content-type": "application/json", ...headers },
      body: JSON.stringify(body ?? {}),
    })

  test("unarmed guard fails closed: approve/reject/distill/list are 503, never executed", async () => {
    delete process.env.SARAH_OPERATOR_ADMIN_TOKEN
    delete process.env.OPENAGENTS_ADMIN_API_TOKEN
    const { handleSarahRequest } = await import("../server.ts")

    for (const request of [
      new Request("http://localhost/sarah/api/operator/learning"),
      post("/api/operator/learning/distill"),
      post("/api/operator/learning/any-id/approve", {
        authorization: "Bearer anything",
      }),
      post("/api/operator/learning/any-id/reject"),
    ]) {
      const res = await handleSarahRequest(request)
      expect(res.status).toBe(503)
      const body = (await res.json()) as { error: { code: string } }
      expect(body.error.code).toBe("operator_admin_not_armed")
    }
    // Nothing was decided as a side effect.
    expect(await listLearningReceipts()).toEqual([])
  })

  test("missing or wrong bearer is 401 on every learning route", async () => {
    process.env.SARAH_OPERATOR_ADMIN_TOKEN = "khs4-test-admin-token"
    const { handleSarahRequest } = await import("../server.ts")

    for (const request of [
      new Request("http://localhost/sarah/api/operator/learning"),
      new Request("http://localhost/sarah/api/operator/learning", {
        headers: { authorization: "Bearer wrong-token" },
      }),
      post("/api/operator/learning/distill", {
        authorization: "Bearer wrong-token",
      }),
      post("/api/operator/learning/any-id/approve", {
        authorization: "Bearer wrong-token",
      }),
      post("/api/operator/learning/any-id/reject"),
    ]) {
      const res = await handleSarahRequest(request)
      expect(res.status).toBe(401)
    }
    expect(await listLearningReceipts()).toEqual([])
  })

  test("the armed bearer can list, distill, approve, and reject", async () => {
    process.env.SARAH_OPERATOR_ADMIN_TOKEN = "khs4-test-admin-token"
    const authed = { authorization: "Bearer khs4-test-admin-token" }
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "How does the Khala API handle rate limits?"),
      turn("prospect:b", "user", "How does the Khala API handle rate limits?"),
      turn("prospect:c", "user", "Is there a sandbox environment we can try?"),
      turn("prospect:d", "user", "Is there a sandbox environment we can try?"),
    ])
    const { handleSarahRequest } = await import("../server.ts")

    const distillRes = await handleSarahRequest(
      post("/api/operator/learning/distill", authed),
    )
    expect(distillRes.status).toBe(200)
    const distill = (await distillRes.json()) as { created: number }
    expect(distill.created).toBe(2)

    const listRes = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/learning", {
        headers: authed,
      }),
    )
    expect(listRes.status).toBe(200)
    const listing = (await listRes.json()) as {
      pending: Array<{ id: string }>
      approved: unknown[]
    }
    expect(listing.pending.length).toBe(2)
    expect(listing.approved.length).toBe(0)

    const [first, second] = listing.pending
    const approveRes = await handleSarahRequest(
      post(`/api/operator/learning/${first!.id}/approve`, authed, {
        by: "owner:chris",
        answer: "Rate limits ride the Khala gateway with metered receipts.",
      }),
    )
    expect(approveRes.status).toBe(200)
    const approved = (await approveRes.json()) as {
      ok: boolean
      receipt: { id: string }
    }
    expect(approved.ok).toBe(true)
    expect(approved.receipt.id).toMatch(/^lr_/)

    const rejectRes = await handleSarahRequest(
      post(`/api/operator/learning/${second!.id}/reject`, authed, {
        by: "owner:chris",
        reason: "needs rewording",
      }),
    )
    expect(rejectRes.status).toBe(200)

    const shared = await listApprovedLearnings()
    expect(shared.length).toBe(1)
    expect(shared[0]!.id).toBe(first!.id)

    // Unknown id → 404; double-decide → 409.
    expect(
      (
        await handleSarahRequest(
          post("/api/operator/learning/lc_missing/approve", authed),
        )
      ).status,
    ).toBe(404)
    expect(
      (
        await handleSarahRequest(
          post(`/api/operator/learning/${first!.id}/approve`, authed),
        )
      ).status,
    ).toBe(409)
  })
})

// ---------------------------------------------------------------------------
// SQ-6 / #8623 — learning-queue review ergonomics
// ---------------------------------------------------------------------------

describe("SQ-6 learning-queue review ergonomics (#8623)", () => {
  test("taxonomy maps kinds for owner review", () => {
    expect(taxonomyForKind("question_gap")).toBe("pain_phrase")
    expect(taxonomyForKind("objection")).toBe("objection")
    expect(taxonomyForKind("winning_answer")).toBe("winning_answer")
  })

  test("whyGeneralize is deterministic and non-empty", () => {
    const why = buildWhyGeneralize({
      kind: "objection",
      prospectCount: 3,
      summary: "too expensive for us",
    })
    expect(why).toContain("Taxonomy=objection")
    expect(why).toContain("3 prospect")
  })

  test("sourceRecency picks newest turn", () => {
    expect(
      computeSourceRecency([
        { recordedAt: "2026-07-01T00:00:00.000Z" },
        { recordedAt: "2026-07-09T12:00:00.000Z" },
        { recordedAt: "2026-07-05T00:00:00.000Z" },
      ]),
    ).toBe("2026-07-09T12:00:00.000Z")
  })

  test("material style change detects low token overlap", () => {
    expect(isMaterialStyleChange(null, "new answer")).toBe(true)
    expect(
      isMaterialStyleChange(
        "We help teams ship coding agents from their phone.",
        "We help teams ship coding agents from their phone.",
      ),
    ).toBe(false)
    expect(
      isMaterialStyleChange(
        "We help teams ship coding agents from their phone.",
        "Totally different enterprise consulting pitch with MSA language.",
      ),
    ).toBe(true)
  })

  test("distilled candidates carry taxonomy, whyGeneralize, exampleCount", async () => {
    // Identical text across prospects so exact-normalized grouping works when
    // the embedder is unarmed in hermetic tests.
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn(
        "prospect:a",
        "user",
        "Honestly this feels too risky for our compliance team.",
      ),
      turn(
        "prospect:b",
        "user",
        "Honestly this feels too risky for our compliance team.",
      ),
    ])
    await distillLearningCandidates()
    const pending = await listLearningCandidates("pending")
    const objection = pending.find((c) => c.kind === "objection")
    expect(objection).toBeDefined()
    expect(objection!.taxonomy).toBe("objection")
    expect(objection!.whyGeneralize.length).toBeGreaterThan(20)
    expect(objection!.exampleCount).toBeGreaterThanOrEqual(2)
    expect(objection!.sourceRecency).not.toBeNull()
  })

  test("approve of material style change records regression fixture", async () => {
    __setSarahAnswerBankForTest([])
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "How do agents report their work back to us?"),
      turn(
        "prospect:a",
        "assistant",
        "Every agent action produces a signed receipt you can audit in the dashboard, with full traceability.",
      ),
      turn("prospect:a", "user", "That makes sense, thanks!"),
    ])
    await distillLearningCandidates()
    const pending = await listLearningCandidates("pending")
    const win = pending.find((c) => c.kind === "winning_answer")
    expect(win).toBeDefined()
    // Inject a prior bank answer with different style for the same question
    // AFTER distill so the candidate is still created.
    __setSarahAnswerBankForTest([
      {
        id: "seed_style",
        questionCanonical: win!.questionCanonical!,
        answer:
          "Totally different enterprise consulting pitch with MSA language only.",
        approvedBy: "seed",
        minSimilarity: null,
        embedding: null,
      },
    ])
    const result = await approveLearningCandidate({
      id: win!.id,
      by: "owner@openagents.com",
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.regressionFixture).not.toBeNull()
      expect(result.regressionFixture!.schema).toBe(
        "sarah.learning_style_regression.v1",
      )
    }
    expect(listLearningRegressionFixtures().length).toBe(1)
  })
})
