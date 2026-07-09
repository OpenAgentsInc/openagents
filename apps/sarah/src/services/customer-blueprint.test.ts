/**
 * KHS-9 (#8608) customer Blueprint draft tests.
 *
 * Oracles for contracts sarah.cross_prospect_isolation.v1 and
 * sarah.memory_query_scoped.v1 (registered in
 * src/contracts/isolation-contracts.ts; human doc
 * docs/sarah/SARAH_CONTRACTS.md): buildCustomerBlueprintDraft is a
 * single-ref seam — every store read is bound to
 * prospectRefAliases(prospectRef) and a draft can never carry another
 * prospect's data. Plus the pricing law: suggested modules pass the
 * deal-rules pricingStatus through verbatim and never carry a price.
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test"

import {
  __setSarahEmbedderForTest,
  __resetSarahAnswerCacheForTest,
} from "./semantic-answer-cache.ts"
import { prospectRefAliases, type SarahProspectFact } from "./prospect-memory.ts"
import {
  __resetCustomerBlueprintForTest,
  __setCustomerBlueprintLatestDraftReaderForTest,
  __setCustomerBlueprintStoreReaderForTest,
  __setCustomerBlueprintWriterForTest,
  blueprintOfferings,
  buildCustomerBlueprintDraft,
  composeCustomerBlueprintDraft,
  CUSTOMER_BLUEPRINT_SCHEMA,
  factLabel,
  getCurrentCustomerBlueprintMapSeed,
  matchNeedsToOfferings,
  type CustomerBlueprintDraft,
  type CustomerBlueprintNeed,
} from "./customer-blueprint.ts"

const DIM = 32

function hashVec(text: string): number[] {
  let h = 2166136261
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  let x = h || 1
  const v: number[] = []
  for (let i = 0; i < DIM; i++) {
    x ^= x << 13
    x ^= x >>> 17
    x ^= x << 5
    v.push(((x >>> 0) % 2000) / 1000 - 1)
  }
  return v
}

function axis(index: number): number[] {
  const v = new Array<number>(DIM).fill(0)
  v[index] = 1
  return v
}

const pinned = new Map<string, number[]>()

beforeEach(() => {
  __resetCustomerBlueprintForTest()
  __resetSarahAnswerCacheForTest()
  pinned.clear()
  __setSarahEmbedderForTest(async (text) => pinned.get(text) ?? hashVec(text))
})

afterEach(() => {
  __resetCustomerBlueprintForTest()
  __resetSarahAnswerCacheForTest()
})

function fact(
  label: string,
  quote: string,
  turnId: string,
): SarahProspectFact {
  return {
    fact: `${label}: "${quote}"`,
    sourceTurnId: turnId,
    at: "2026-07-09T10:00:00.000Z",
  }
}

describe("composeCustomerBlueprintDraft (pure)", () => {
  test("every stated need cites its source turn id (provenance)", () => {
    const facts = [
      fact("company", "we're a 40-person e-commerce shop", "turn-1"),
      fact("need", "we need help with support tickets", "turn-2"),
      fact("need", "we want automated ad campaigns", "turn-3"),
    ]
    const draft = composeCustomerBlueprintDraft({
      prospectRef: "prospect-compose",
      facts,
      contact: { email: "buyer@example.com", contactId: "oa_user:u1" },
      suggestedModules: [],
      revision: 1,
    })
    expect(draft.schema).toBe(CUSTOMER_BLUEPRINT_SCHEMA)
    expect(draft.needs.length).toBe(2)
    for (const need of draft.needs) {
      expect(need.sourceTurnId).toBeTruthy()
    }
    expect(draft.needs.map((need) => need.sourceTurnId).sort()).toEqual([
      "turn-2",
      "turn-3",
    ])
    // Business facts keep their provenance and exclude needs.
    expect(draft.business.facts.length).toBe(1)
    expect(draft.business.facts[0]!.sourceTurnId).toBe("turn-1")
    // Sources aggregate every cited turn.
    expect(draft.sources.turnIds.sort()).toEqual(["turn-1", "turn-2", "turn-3"])
    expect(draft.contacts.email).toBe("buyer@example.com")
  })

  test("handoff is honest: a draft for the operator pipeline, never automated provisioning", () => {
    const draft = composeCustomerBlueprintDraft({
      prospectRef: "prospect-honest",
      facts: [],
      contact: null,
      suggestedModules: [],
      revision: 1,
    })
    expect(draft.handoff.automatedProvisioning).toBe(false)
    expect(draft.handoff.pipeline).toBe("operator_assisted_business_workspace")
    expect(draft.handoff.convergesWith).toContain("CB-1.4")
    expect(draft.handoff.note).toContain("nothing is provisioned automatically")
  })

  test("factLabel parses only our own typed encoding", () => {
    expect(factLabel(fact("need", "x", "t"))).toBe("need")
    expect(factLabel(fact("company", "y", "t"))).toBe("company")
    expect(
      factLabel({ fact: "free text without a label", sourceTurnId: "t", at: "" }),
    ).toBe("other")
  })
})

describe("matchNeedsToOfferings (semantic law + pricing law)", () => {
  const offerings = blueprintOfferings()
  const supportDescriptor = offerings.find(
    (offering) => offering.ref === "module.customer_support_ai",
  )!.descriptor

  test("needs map to offerings by embedding match, with need-turn provenance", async () => {
    const needs: CustomerBlueprintNeed[] = [
      {
        need: 'need: "we are drowning in support tickets"',
        sourceTurnId: "turn-9",
        at: "2026-07-09T10:00:00.000Z",
      },
    ]
    pinned.set(needs[0]!.need, axis(0))
    pinned.set(supportDescriptor, axis(0))

    const suggested = await matchNeedsToOfferings(needs, offerings)
    const support = suggested.find(
      (module) => module.ref === "module.customer_support_ai",
    )
    expect(support).toBeDefined()
    expect(support!.matchBasis).toBe("semantic")
    expect(support!.matchedNeedTurnIds).toEqual(["turn-9"])
    expect(support!.availability).toBe("operator_assisted")
  })

  test("pricingStatus passes through verbatim and no price ever appears", async () => {
    const needs: CustomerBlueprintNeed[] = [
      {
        need: 'need: "we are drowning in support tickets"',
        sourceTurnId: "turn-9",
        at: "2026-07-09T10:00:00.000Z",
      },
    ]
    pinned.set(needs[0]!.need, axis(0))
    pinned.set(supportDescriptor, axis(0))
    const suggested = await matchNeedsToOfferings(needs, offerings)
    for (const module of suggested) {
      expect(module.pricingStatus).toBe("owner_pricing_required")
    }
    const serialized = JSON.stringify(suggested)
    expect(serialized).not.toContain("setupPriceUsdCents")
    expect(serialized).not.toContain("$")
    expect(serialized).not.toContain("price")
  })

  test("no embedder: honest candidate_default listing, never a keyword fallback", async () => {
    __setSarahEmbedderForTest(async () => null)
    const needs: CustomerBlueprintNeed[] = [
      {
        need: 'need: "we need help with support tickets"',
        sourceTurnId: "turn-9",
        at: "2026-07-09T10:00:00.000Z",
      },
    ]
    const suggested = await matchNeedsToOfferings(needs, offerings)
    expect(suggested.length).toBe(offerings.length)
    for (const module of suggested) {
      expect(module.matchBasis).toBe("candidate_default")
      expect(module.matchedNeedTurnIds).toEqual([])
    }
  })

  test("offerings are the three configured modules plus the KB workspace packs", () => {
    expect(offerings.map((offering) => offering.ref).sort()).toEqual(
      [
        "module.customer_support_ai",
        "module.internal_operations_ai",
        "module.sales_employee_ai",
        "workspace_pack.agency",
        "workspace_pack.ecommerce",
        "workspace_pack.legal",
      ].sort(),
    )
  })
})

// Oracle customer_blueprint_prospect_scoping.unit for contracts
// sarah.cross_prospect_isolation.v1 and sarah.memory_query_scoped.v1.
describe("buildCustomerBlueprintDraft scoping (KHS-3 single-ref seam)", () => {
  const seeded: Record<
    string,
    {
      profileFacts: SarahProspectFact[]
      contact: { email: string | null; contactId: string | null } | null
      turns: never[]
      latestRevision: number
    }
  > = {
    "prospect-a": {
      profileFacts: [
        fact("company", "Acme Retail, ALPHA-ONLY-DETAIL-77", "turn-a1"),
        fact("need", "we need ALPHA support automation", "turn-a2"),
      ],
      contact: { email: "alpha@example.com", contactId: "oa_user:alpha" },
      turns: [],
      latestRevision: 0,
    },
    "prospect-b": {
      profileFacts: [
        fact("company", "Bravo Legal, BRAVO-ONLY-SECRET-99", "turn-b1"),
        fact("need", "we need BRAVO intake review", "turn-b2"),
      ],
      contact: { email: "bravo@example.com", contactId: "oa_user:bravo" },
      turns: [],
      latestRevision: 2,
    },
  }

  const readerCalls: string[][] = []
  const storedDrafts: Array<{
    prospectRef: string
    revision: number
    draft: CustomerBlueprintDraft
  }> = []

  beforeEach(() => {
    readerCalls.length = 0
    storedDrafts.length = 0
    __setCustomerBlueprintStoreReaderForTest(async (aliases) => {
      readerCalls.push([...aliases])
      return seeded[aliases[0]!] ?? null
    })
    __setCustomerBlueprintWriterForTest(async (prospectRef, revision, draft) => {
      storedDrafts.push({ prospectRef, revision, draft })
      return true
    })
  })

  test("every store read is bound to exactly the requested identity's aliases", async () => {
    await buildCustomerBlueprintDraft("prospect-a")
    expect(readerCalls).toEqual([prospectRefAliases("prospect-a")])
    expect(readerCalls[0]).toEqual(["prospect-a", "prospect:prospect-a"])
  })

  test("drafts for two prospects never cross-contaminate", async () => {
    const a = await buildCustomerBlueprintDraft("prospect-a")
    const b = await buildCustomerBlueprintDraft("prospect:prospect-b")
    expect(a.ok && b.ok).toBe(true)
    if (!a.ok || !b.ok) return

    const serializedA = JSON.stringify(a.draft)
    const serializedB = JSON.stringify(b.draft)
    expect(serializedA).toContain("ALPHA-ONLY-DETAIL-77")
    expect(serializedA).not.toContain("BRAVO")
    expect(serializedA).not.toContain("bravo@example.com")
    expect(serializedA).not.toContain("turn-b1")
    expect(serializedB).toContain("BRAVO-ONLY-SECRET-99")
    expect(serializedB).not.toContain("ALPHA")
    expect(serializedB).not.toContain("alpha@example.com")
    expect(serializedB).not.toContain("turn-a2")
  })

  test("needs carry provenance and revisions increment per prospect", async () => {
    const a = await buildCustomerBlueprintDraft("prospect-a")
    const b = await buildCustomerBlueprintDraft("prospect-b")
    expect(a.ok && b.ok).toBe(true)
    if (!a.ok || !b.ok) return

    expect(a.draft.needs.length).toBe(1)
    expect(a.draft.needs[0]!.sourceTurnId).toBe("turn-a2")
    expect(a.revision).toBe(1)
    // prospect-b already has revision 2 stored — the new draft is 3.
    expect(b.revision).toBe(3)
    expect(storedDrafts.map((row) => row.revision)).toEqual([1, 3])
    expect(storedDrafts[0]!.prospectRef).toBe("prospect-a")
  })

  test("draft revisions publish typed blueprint deltas to this prospect's aliases", async () => {
    const { sarahAvatarEventStream } = await import("./avatar-event-bus.ts")
    const raw = sarahAvatarEventStream("prospect-a")
    const alias = sarahAvatarEventStream("prospect:prospect-a")
    const rawReader = raw.body!.getReader()
    const aliasReader = alias.body!.getReader()
    const decoder = new TextDecoder()
    await rawReader.read()
    await aliasReader.read()

    const result = await buildCustomerBlueprintDraft("prospect-a")
    expect(result.ok).toBe(true)

    await rawReader.read() // existing "Your Blueprint draft" card
    await aliasReader.read()
    const rawDelta = JSON.parse(
      decoder.decode((await rawReader.read()).value).replace(/^data:\s*/, ""),
    ) as {
      type: string
      delta: {
        kind: string
        revision: number
        needsCount: number
        matchedModules: Array<{
          ref: string
          name: string
          matchBasis: string
          matchedNeedTurnIds: string[]
        }>
      }
    }
    const aliasDelta = JSON.parse(
      decoder.decode((await aliasReader.read()).value).replace(/^data:\s*/, ""),
    ) as typeof rawDelta

    expect(rawDelta).toEqual(aliasDelta)
    expect(rawDelta.type).toBe("blueprint_delta")
    expect(rawDelta.delta.kind).toBe("draft_revision")
    expect(rawDelta.delta.revision).toBe(1)
    expect(rawDelta.delta.needsCount).toBe(1)
    expect(rawDelta.delta.matchedModules.length).toBeGreaterThan(0)
    expect(rawDelta.delta.matchedModules[0]).toEqual(
      expect.objectContaining({
        ref: expect.any(String),
        name: expect.any(String),
        matchBasis: expect.any(String),
        matchedNeedTurnIds: expect.any(Array),
      }),
    )

    await rawReader.cancel()
    await aliasReader.cancel()
  })

  test("BM-2 current map seed reads the latest draft and facts for one prospect", async () => {
    __setCustomerBlueprintLatestDraftReaderForTest(async (aliases) => {
      const refs = new Set(aliases)
      return storedDrafts
        .filter((row) => refs.has(row.prospectRef))
        .sort((a, b) => b.revision - a.revision)[0]?.draft ?? null
    })

    const built = await buildCustomerBlueprintDraft("prospect-a")
    expect(built.ok).toBe(true)
    const seed = await getCurrentCustomerBlueprintMapSeed("prospect-a")
    expect(seed?.storeConfigured).toBe(true)
    expect(seed?.draft?.revision).toBe(1)
    expect(seed?.facts.map((entry) => entry.fact)).toContain(
      'company: "Acme Retail, ALPHA-ONLY-DETAIL-77"',
    )
    expect(seed?.contact?.email).toBe("alpha@example.com")

    const unknown = await getCurrentCustomerBlueprintMapSeed("")
    expect(unknown).toBeNull()
  })

  test("an empty ref refuses instead of reading unscoped", async () => {
    const result = await buildCustomerBlueprintDraft("")
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.error).toBe("missing_prospect_ref")
    expect(readerCalls).toEqual([])
  })

  test("no durable store fails soft to an honest minimal draft", async () => {
    __setCustomerBlueprintStoreReaderForTest(async () => null)
    __setCustomerBlueprintWriterForTest(async () => false)
    const result = await buildCustomerBlueprintDraft("prospect-nostore")
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.draft.needs).toEqual([])
    expect(result.stored).toBe(false)
    // Honest degradation on the module map: candidates, no claimed matches.
    for (const module of result.draft.suggestedModules) {
      expect(module.matchBasis).toBe("candidate_default")
    }
  })
})
