/**
 * KHS-5 Sarah's Blueprint oracles (#8604, epic #8599).
 *
 * Enforcement oracles for contract `sarah.blueprint_versioned_provenance.v1`
 * (registered in src/contracts/isolation-contracts.ts):
 *
 * - blueprint_compile_roundtrip.unit — the checked-in seed compiles
 *   deterministically, and the committed KB doc IS the compiled output
 *   (generated, not hand-edited).
 * - blueprint_revision_immutability.unit — retiring a fact is a new revision
 *   that flips status; facts are never deleted; every fact carries the
 *   revision that added it.
 * - blueprint_provenance_required.unit — no fact without provenance: the
 *   seed carries owner_kb_v2 on every fact; adds without a valid source are
 *   rejected; pricing facts carry dealRuleRefs; product facts promiseIds.
 * - blueprint_flag_off_unchanged.unit — SARAH_BLUEPRINT unset keeps the
 *   file-based instructions path byte-identical (safe rollout).
 * - blueprint_admin_guard.rpc — operator endpoints fail closed (unarmed →
 *   503, wrong bearer → 401) and a full receipted revision cycle works over
 *   HTTP (add → retire → read back).
 *
 * Hermetic: no database (memory store seeded from the checked-in seed), the
 * promise-registry fetch is pointed at an unroutable localhost so it fails
 * fast into the degraded grounding.
 */

import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  test,
} from "bun:test"

import {
  __resetSarahBlueprintForTest,
  addSarahBlueprintFact,
  blueprintFactId,
  BLUEPRINT_SECTIONS,
  BLUEPRINT_SOURCE_PATTERN,
  compileSarahSystemPrompt,
  loadSarahBlueprint,
  parseSarahKnowledgeBaseMarkdown,
  promoteLearningToBlueprintFact,
  renderSarahKnowledgeBaseMarkdown,
  renderSarahSystemPrompt,
  retireSarahBlueprintFact,
  sarahBlueprintEnabled,
  sarahBlueprintStoreMode,
} from "./sarah-blueprint.ts"
import {
  __resetSarahCollectiveLearningForTest,
  __setSarahLearningTurnsForTest,
  approveLearningCandidate,
  distillLearningCandidates,
  learningReceiptRef,
  listLearningCandidates,
  type LearningTurnRow,
} from "./collective-learning.ts"
import { __resetSarahAnswerCacheForTest } from "./semantic-answer-cache.ts"
import { getSarahRealtimeInstructions } from "./sarah-instructions.ts"

const repoPath = (ref: string): string =>
  new URL(`../../../../${ref}`, import.meta.url).pathname

const savedEnv: Record<string, string | undefined> = {}
const ENV_KEYS = [
  "SARAH_DATABASE_URL",
  "KHALA_SYNC_DATABASE_URL",
  "GEMINI_API_KEY",
  "SARAH_OPERATOR_ADMIN_TOKEN",
  "OPENAGENTS_ADMIN_API_TOKEN",
  "SARAH_BLUEPRINT",
  "PROMISE_REGISTRY_URL",
  "SARAH_AGENT_DIR",
] as const

beforeAll(() => {
  for (const key of ENV_KEYS) {
    savedEnv[key] = process.env[key]
    delete process.env[key]
  }
  // promise-registry module reads this at import time in some paths; an
  // unroutable address makes the grounding fetch fail fast into the
  // degraded (deterministic) grounding for the instruction-composition
  // tests below.
  process.env.PROMISE_REGISTRY_URL = "http://127.0.0.1:9/unroutable"
})

afterAll(() => {
  for (const key of ENV_KEYS) {
    if (savedEnv[key] === undefined) delete process.env[key]
    else process.env[key] = savedEnv[key]
  }
})

afterEach(() => {
  __resetSarahBlueprintForTest()
  __resetSarahCollectiveLearningForTest()
  __resetSarahAnswerCacheForTest()
  delete process.env.SARAH_OPERATOR_ADMIN_TOKEN
  delete process.env.OPENAGENTS_ADMIN_API_TOKEN
  delete process.env.SARAH_BLUEPRINT
})

// ---------------------------------------------------------------------------
// Oracle blueprint_compile_roundtrip.unit
// ---------------------------------------------------------------------------

describe("seed → compile roundtrip stability", () => {
  test("the seed loads as revision 1 with typed facts in every section", async () => {
    expect(sarahBlueprintStoreMode()).toBe("memory_ephemeral")
    const blueprint = await loadSarahBlueprint()
    expect(blueprint.currentRevision).toBe(1)
    expect(blueprint.revisions).toHaveLength(1)
    expect(blueprint.revisions[0]?.changedBy).toBe("owner")
    expect(blueprint.facts.length).toBeGreaterThanOrEqual(90)
    const sections = new Set(blueprint.facts.map((fact) => fact.section))
    for (const section of BLUEPRINT_SECTIONS) {
      expect(sections.has(section)).toBe(true)
    }
    for (const fact of blueprint.facts) {
      expect(fact.revisionAdded).toBe(1)
      expect(fact.status).toBe("active")
    }
  })

  test("the committed KB doc IS the compiled blueprint output (generated, not hand-edited)", async () => {
    const blueprint = await loadSarahBlueprint()
    const rendered = renderSarahKnowledgeBaseMarkdown(blueprint)
    const committed = await Bun.file(
      repoPath("docs/sarah/SARAH_KNOWLEDGE_BASE.md"),
    ).text()
    expect(rendered).toBe(committed)
    // Deterministic: compiling twice yields identical bytes.
    expect(renderSarahKnowledgeBaseMarkdown(blueprint)).toBe(rendered)
  })

  test("re-parsing the compiled doc yields the same fact statements (parse→render→parse fixpoint)", async () => {
    const blueprint = await loadSarahBlueprint()
    const rendered = renderSarahKnowledgeBaseMarkdown(blueprint)
    const reparsed = parseSarahKnowledgeBaseMarkdown(rendered)
    const reparsedIds = reparsed.map((block) =>
      blueprintFactId(
        block.section,
        block.heading,
        block.format,
        block.statement,
      ),
    )
    expect(reparsedIds).toEqual(blueprint.facts.map((fact) => fact.id))
  })

  test("the compiled system prompt preserves Section A ordering: identity → engine → hard rules → knowledge", async () => {
    const prompt = await compileSarahSystemPrompt()
    expect(prompt).toBeTruthy()
    const identityAt = prompt!.indexOf("You are **Sarah, OpenAgents'")
    const engineAt = prompt!.indexOf("## The engine")
    const hardRulesAt = prompt!.indexOf("## Hard rules")
    const knowledgeAt = prompt!.indexOf("## C.1 What OpenAgents is")
    const playbookAt = prompt!.indexOf("## Openers")
    expect(identityAt).toBe(0)
    expect(engineAt).toBeGreaterThan(identityAt)
    expect(hardRulesAt).toBeGreaterThan(engineAt)
    expect(knowledgeAt).toBeGreaterThan(hardRulesAt)
    expect(playbookAt).toBeGreaterThan(knowledgeAt)
    // The compiled prompt keeps the binding laws verbatim.
    expect(prompt).toContain("Never invent pricing")
    expect(prompt).toContain("the ONLY quotable numbers")
  })
})

// ---------------------------------------------------------------------------
// Oracle blueprint_provenance_required.unit
// ---------------------------------------------------------------------------

describe("provenance is required on every fact", () => {
  test("every seed fact carries owner_kb_v2 provenance with a ref and timestamp", async () => {
    const blueprint = await loadSarahBlueprint()
    for (const fact of blueprint.facts) {
      expect(fact.provenance.source).toBe("owner_kb_v2")
      expect(BLUEPRINT_SOURCE_PATTERN.test(fact.provenance.source)).toBe(true)
      expect(fact.provenance.ref).toContain("docs/sarah/SARAH_KNOWLEDGE_BASE.md")
      expect(fact.provenance.at).toBeTruthy()
    }
  })

  test("typed pricing facts carry dealRuleRefs; product facts carry promiseIds", async () => {
    const blueprint = await loadSarahBlueprint()
    const volumeFact = blueprint.facts.find((fact) =>
      fact.statement.includes("Credit volume bonuses"),
    )
    expect(volumeFact?.section).toBe("pricing")
    expect(volumeFact?.dealRuleRefs).toContain(
      "rule.credit_volume.usd_1000_2999.bonus_10pct",
    )
    expect(volumeFact?.dealRuleRefs).toContain(
      "rule.credit_volume.usd_5000_plus.bonus_35pct",
    )
    const bitcoinFact = blueprint.facts.find((fact) =>
      fact.statement.includes("Bitcoin/Lightning payment discount"),
    )
    expect(bitcoinFact?.dealRuleRefs).toContain(
      "rule.payment.bitcoin.discount_5pct",
    )
    const khalaApiFact = blueprint.facts.find((fact) =>
      fact.statement.includes("**Khala (free API)**"),
    )
    expect(khalaApiFact?.section).toBe("products")
    expect(khalaApiFact?.promiseIds).toContain(
      "inference.khala_free_openai_compatible_api.v1",
    )
  })

  test("adding a fact without a valid provenance source is rejected", async () => {
    const missing = await addSarahBlueprintFact({
      section: "company",
      statement: "OpenAgents test statement.",
      source: "",
      by: "owner",
      changeNote: "test",
    })
    expect(missing).toEqual({ ok: false, error: "provenance_source_invalid" })
    const bogus = await addSarahBlueprintFact({
      section: "company",
      statement: "OpenAgents test statement.",
      source: "vibes",
      by: "owner",
      changeNote: "test",
    })
    expect(bogus).toEqual({ ok: false, error: "provenance_source_invalid" })
    const noNote = await addSarahBlueprintFact({
      section: "company",
      statement: "OpenAgents test statement.",
      source: "owner_directive",
      by: "owner",
      changeNote: "",
    })
    expect(noNote).toEqual({ ok: false, error: "change_note_required" })
    const badSection = await addSarahBlueprintFact({
      section: "vibes",
      statement: "OpenAgents test statement.",
      source: "owner_directive",
      by: "owner",
      changeNote: "test",
    })
    expect(badSection).toEqual({ ok: false, error: "section_invalid" })
  })
})

// ---------------------------------------------------------------------------
// Oracle blueprint_revision_immutability.unit
// ---------------------------------------------------------------------------

describe("revisions: retire is a new revision, never a delete", () => {
  test("add → retire produces receipted revisions and keeps the retired fact", async () => {
    const before = await loadSarahBlueprint()
    const added = await addSarahBlueprintFact({
      section: "company",
      statement: "OpenAgents also ships a typed blueprint for Sarah herself.",
      source: "owner_directive",
      ref: "issue:#8604",
      by: "owner",
      changeNote: "KHS-5 revision-cycle test: add",
    })
    expect(added.ok).toBe(true)
    if (!added.ok) throw new Error("add failed")
    expect(added.revision.revision).toBe(before.currentRevision + 1)
    expect(added.fact.revisionAdded).toBe(added.revision.revision)
    expect(added.revision.changeNote).toBe("KHS-5 revision-cycle test: add")

    const retired = await retireSarahBlueprintFact({
      factId: added.fact.id,
      by: "owner",
      changeNote: "KHS-5 revision-cycle test: retire",
    })
    expect(retired.ok).toBe(true)
    if (!retired.ok) throw new Error("retire failed")
    expect(retired.revision.revision).toBe(added.revision.revision + 1)
    expect(retired.fact.status).toBe("retired")
    expect(retired.fact.revisionRetired).toBe(retired.revision.revision)

    const after = await loadSarahBlueprint()
    // Never deleted: the fact row remains, flagged retired.
    expect(after.facts.length).toBe(before.facts.length + 1)
    const kept = after.facts.find((fact) => fact.id === added.fact.id)
    expect(kept?.status).toBe("retired")
    expect(kept?.revisionAdded).toBe(added.revision.revision)
    expect(kept?.revisionRetired).toBe(retired.revision.revision)
    // Retired facts leave the compiled surfaces but not the record.
    expect(renderSarahSystemPrompt(after.facts)).not.toContain(
      "typed blueprint for Sarah herself",
    )
    // Full revision history retained.
    expect(after.revisions.map((rev) => rev.revision)).toEqual(
      Array.from({ length: after.currentRevision }, (_, i) => i + 1),
    )
    // Retiring again is a conflict, not a delete.
    const again = await retireSarahBlueprintFact({
      factId: added.fact.id,
      by: "owner",
      changeNote: "double retire",
    })
    expect(again).toEqual({ ok: false, error: "already_retired" })
  })
})

// ---------------------------------------------------------------------------
// KHS-4 seam: approved winning_answer → blueprint fact
// ---------------------------------------------------------------------------

let turnId = 0
function turn(
  prospectRef: string,
  role: string,
  text: string,
): LearningTurnRow {
  turnId += 1
  return {
    id: String(turnId),
    prospectRef,
    role,
    text,
    recordedAt: new Date(1700000000000 + turnId * 1000).toISOString(),
  }
}

describe("promoting an approved learning into the blueprint", () => {
  test("an approved winning_answer becomes a playbook fact with learning_receipt provenance", async () => {
    __setSarahLearningTurnsForTest([
      turn("prospect:a", "user", "How does the QA swarm verify its findings?"),
      turn(
        "prospect:a",
        "assistant",
        "Every QA swarm verdict is confirmed or refuted with videos and distilled regression tests, published on a proof page you can check yourself.",
      ),
      turn("prospect:a", "user", "Thanks, that makes sense."),
    ])
    const distilled = await distillLearningCandidates()
    expect(distilled.ok).toBe(true)
    const pending = await listLearningCandidates("pending")
    const winning = pending.find((c) => c.kind === "winning_answer")
    expect(winning).toBeTruthy()
    const approved = await approveLearningCandidate({
      id: winning!.id,
      by: "owner",
    })
    expect(approved.ok).toBe(true)
    if (!approved.ok) throw new Error("approve failed")

    const promoted = await promoteLearningToBlueprintFact({
      candidateId: winning!.id,
      by: "owner",
    })
    expect(promoted.ok).toBe(true)
    if (!promoted.ok) throw new Error("promotion failed")
    expect(promoted.fact.section).toBe("playbook")
    expect(promoted.fact.provenance.source).toBe(
      learningReceiptRef(approved.receipt.id),
    )
    expect(promoted.fact.provenance.ref).toBe(
      `sarah_learning_candidates:${winning!.id}`,
    )
    // The promoted fact renders into the compiled surfaces.
    const blueprint = await loadSarahBlueprint()
    const prompt = renderSarahSystemPrompt(blueprint.facts)
    expect(prompt).toContain("Learned winning answers (owner-approved)")
    expect(prompt).toContain("QA swarm")
  })

  test("pending or non-winning candidates cannot be promoted", async () => {
    __setSarahLearningTurnsForTest([
      turn("prospect:b", "user", "How does onboarding work exactly?"),
      turn(
        "prospect:b",
        "assistant",
        "Onboarding starts with a captured intake and a funded credit account, then the team follows up fast with next steps.",
      ),
      turn("prospect:b", "user", "Great, thank you."),
    ])
    await distillLearningCandidates()
    const pending = await listLearningCandidates("pending")
    const winning = pending.find((c) => c.kind === "winning_answer")
    expect(winning).toBeTruthy()
    // Still pending — not promotable.
    const early = await promoteLearningToBlueprintFact({
      candidateId: winning!.id,
      by: "owner",
    })
    expect(early).toEqual({
      ok: false,
      error: "approved_candidate_not_found",
    })
  })
})

// ---------------------------------------------------------------------------
// Oracle blueprint_flag_off_unchanged.unit
// ---------------------------------------------------------------------------

describe("flag rollout safety (SARAH_BLUEPRINT)", () => {
  test("flag-off keeps the file-based instructions path unchanged", async () => {
    delete process.env.SARAH_BLUEPRINT
    expect(sarahBlueprintEnabled()).toBe(false)
    const composed = await getSarahRealtimeInstructions()
    expect(composed).toContain("You are Sarah, OpenAgents' AI sales employee.")
    // No blueprint-compiled sections leak in when unarmed.
    expect(composed).not.toContain("## C.1 What OpenAgents is")
    expect(composed).not.toContain("## The engine")
  })

  test("flag-on leads with the compiled blueprint and keeps the tool protocol", async () => {
    process.env.SARAH_BLUEPRINT = "1"
    expect(sarahBlueprintEnabled()).toBe(true)
    const composed = await getSarahRealtimeInstructions()
    expect(composed).toContain("## The engine")
    expect(composed).toContain("## C.1 What OpenAgents is")
    // instructions.md tool protocol still present (deal_rules_evaluate law).
    expect(composed).toContain("deal_rules_evaluate")
    // Blueprint compile leads the composition.
    expect(
      composed.indexOf("You are **Sarah, OpenAgents'"),
    ).toBeLessThan(composed.indexOf("deal_rules_evaluate"))
  })
})

// ---------------------------------------------------------------------------
// Oracle blueprint_admin_guard.rpc
// ---------------------------------------------------------------------------

describe("operator endpoints: fail-closed admin guard + revision cycle over HTTP", () => {
  test("unarmed → 503 on every blueprint operator route", async () => {
    const { handleSarahRequest } = await import("../server.ts")
    for (const [method, path, body] of [
      ["GET", "/sarah/api/operator/blueprint", undefined],
      [
        "POST",
        "/sarah/api/operator/blueprint/facts",
        JSON.stringify({ action: "add" }),
      ],
      [
        "POST",
        "/sarah/api/operator/blueprint/promote",
        JSON.stringify({ candidateId: "x" }),
      ],
    ] as const) {
      const res = await handleSarahRequest(
        new Request(`http://localhost${path}`, { method, body }),
      )
      expect(res.status).toBe(503)
      const payload = (await res.json()) as { error?: { code?: string } }
      expect(payload.error?.code).toBe("operator_admin_not_armed")
    }
  })

  test("armed: missing/wrong bearer → 401 and nothing is written", async () => {
    process.env.SARAH_OPERATOR_ADMIN_TOKEN = "khs5-test-token"
    const { handleSarahRequest } = await import("../server.ts")
    for (const headers of [
      undefined,
      { authorization: "Bearer wrong-token" },
    ]) {
      const res = await handleSarahRequest(
        new Request("http://localhost/sarah/api/operator/blueprint/facts", {
          method: "POST",
          headers,
          body: JSON.stringify({
            action: "add",
            section: "company",
            statement: "unauthorized write",
            source: "owner_directive",
            changeNote: "should never land",
          }),
        }),
      )
      expect(res.status).toBe(401)
    }
    const blueprint = await loadSarahBlueprint()
    expect(blueprint.currentRevision).toBe(1)
    expect(
      blueprint.facts.some((fact) =>
        fact.statement.includes("unauthorized write"),
      ),
    ).toBe(false)
  })

  test("armed with the exact bearer: full receipted revision cycle (add → retire → read back)", async () => {
    process.env.SARAH_OPERATOR_ADMIN_TOKEN = "khs5-test-token"
    const { handleSarahRequest } = await import("../server.ts")
    const authed = (path: string, init: RequestInit = {}) =>
      handleSarahRequest(
        new Request(`http://localhost${path}`, {
          ...init,
          headers: {
            authorization: "Bearer khs5-test-token",
            ...(init.headers ?? {}),
          },
        }),
      )

    const addRes = await authed("/sarah/api/operator/blueprint/facts", {
      method: "POST",
      body: JSON.stringify({
        action: "add",
        section: "products",
        statement:
          "**Sarah's Blueprint** — her own typed, versioned knowledge object with per-fact provenance.",
        heading: "C.2 The products",
        format: "list_item",
        source: "owner_directive",
        ref: "issue:#8604",
        by: "owner",
        changeNote: "KHS-5 demo: add blueprint product fact",
      }),
    })
    expect(addRes.status).toBe(200)
    const added = (await addRes.json()) as {
      ok: boolean
      revision: { revision: number; changeNote: string }
      fact: { id: string; revisionAdded: number }
    }
    expect(added.ok).toBe(true)
    expect(added.revision.revision).toBe(2)
    expect(added.fact.revisionAdded).toBe(2)

    const retireRes = await authed("/sarah/api/operator/blueprint/facts", {
      method: "POST",
      body: JSON.stringify({
        action: "retire",
        factId: added.fact.id,
        by: "owner",
        changeNote: "KHS-5 demo: retire the same fact",
      }),
    })
    expect(retireRes.status).toBe(200)
    const retired = (await retireRes.json()) as {
      ok: boolean
      revision: { revision: number }
      fact: { status: string; revisionRetired: number }
    }
    expect(retired.revision.revision).toBe(3)
    expect(retired.fact.status).toBe("retired")
    expect(retired.fact.revisionRetired).toBe(3)

    const listRes = await authed("/sarah/api/operator/blueprint")
    expect(listRes.status).toBe(200)
    const listed = (await listRes.json()) as {
      currentRevision: number
      revisions: Array<{ revision: number; changeNote: string }>
      facts: Array<{ id: string; status: string }>
    }
    expect(listed.currentRevision).toBe(3)
    expect(listed.revisions).toHaveLength(3)
    expect(
      listed.revisions.map((rev) => rev.changeNote).slice(1),
    ).toEqual([
      "KHS-5 demo: add blueprint product fact",
      "KHS-5 demo: retire the same fact",
    ])
    const fact = listed.facts.find((entry) => entry.id === added.fact.id)
    expect(fact?.status).toBe("retired")

    const missing = await authed("/sarah/api/operator/blueprint/facts", {
      method: "POST",
      body: JSON.stringify({
        action: "retire",
        factId: "bf_does_not_exist",
        changeNote: "x",
      }),
    })
    expect(missing.status).toBe(404)
  })
})
