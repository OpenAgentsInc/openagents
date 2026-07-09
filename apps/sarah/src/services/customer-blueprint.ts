/**
 * KHS-9 customer Blueprint drafts (#8608, epic #8599): Sarah helps a prospect
 * build THEIR Blueprint in conversation.
 *
 * A draft is composed from what Sarah already durably knows about ONE
 * prospect — the KHS-2 `sarah_prospect_profile` facts (per-fact provenance
 * turn ids preserved), the `sarah_prospect_contacts` row, and stated needs
 * distilled from `sarah_transcript_turns` — mapped onto the configured
 * AI-employee modules (deal-rules config) and the operator-assisted business
 * workspace packs from the knowledge base.
 *
 * HONEST HANDOFF SEAM: the draft is a DRAFT for the operator-assisted
 * business-workspace pipeline. Nothing here provisions a workspace — that is
 * the CB-1.4 prefill pipeline's lane (MASTER_ROADMAP: intake → public-data
 * research run → seeded brain + starter employee in `observe`, intro receipt
 * naming every source). This module records that convergence explicitly and
 * claims none of it as automated today.
 *
 * LAW:
 * - Per-prospect scoping (KHS-3, contract sarah.cross_prospect_isolation.v1):
 *   every read is bound to `prospectRefAliases(prospectRef)` — the exact
 *   identity's deterministic re-encodings, never a pattern or cross-prospect
 *   list. The operator listing endpoint is owner-facing only and never feeds
 *   a prospect-facing serve path.
 * - No improvised pricing: suggested modules carry the deal-rules
 *   `pricingStatus` verbatim (`owner_pricing_required` passes through
 *   untouched) and never a price. Workspace packs are operator-assisted and
 *   owner-priced by definition.
 * - Semantic routing law: need→module matching is embedding + cosine via the
 *   shared `sarahEmbedText` lane. With no embedder available the offerings
 *   are listed as default candidates (matchBasis `candidate_default`) — an
 *   honest degradation, never a keyword fallback.
 * - Fail-soft persistence: `sarah_customer_blueprints` (sarah_-prefixed,
 *   schema-ensured) stores revisions; a failed write never breaks the
 *   conversation — the draft is still returned.
 */

import { SQL } from "bun"

import { DEFAULT_DEAL_RULES_CONFIG } from "./deal-rules.ts"
import {
  distillProspectFacts,
  prospectRefAliases,
  type SarahMemoryTurnRow,
  type SarahProspectFact,
} from "./prospect-memory.ts"
import {
  publishSarahAvatarEvent,
  publishSarahBlueprintDelta,
} from "./avatar-event-bus.ts"
import { cosineSimilarity, sarahEmbedText } from "./semantic-answer-cache.ts"
import { readSarahStore } from "./turn-store.ts"

export const CUSTOMER_BLUEPRINT_SCHEMA = "sarah.customer_blueprint_draft.v1"

const TURNS_QUERY_LIMIT = 60
const MODULE_MATCH_MIN_SIMILARITY_DEFAULT = 0.45

export type CustomerBlueprintNeed = {
  need: string
  sourceTurnId: string
  at: string
}

export type SuggestedModule = {
  ref: string
  name: string
  kind: "ai_employee_module" | "workspace_pack"
  availability: "operator_assisted"
  /**
   * Deal-rules pricingStatus passed through VERBATIM. Never a price:
   * pricing stays deal-rule land (sarah.no_improvised_pricing.v1).
   */
  pricingStatus: string
  /** Provenance: which stated needs suggested this offering. */
  matchedNeedTurnIds: string[]
  matchBasis: "semantic" | "candidate_default"
}

export type CustomerBlueprintDraft = {
  schema: typeof CUSTOMER_BLUEPRINT_SCHEMA
  prospectRef: string
  revision: number
  createdAt: string
  business: {
    /** Non-need profile facts (company/role/stack/contact cues), provenance kept. */
    facts: SarahProspectFact[]
  }
  contacts: {
    email: string | null
    contactId: string | null
  }
  needs: CustomerBlueprintNeed[]
  suggestedModules: SuggestedModule[]
  sources: {
    turnIds: string[]
    factCount: number
    provenance: "sarah_prospect_profile + sarah_transcript_turns (per-fact source turn ids)"
  }
  handoff: {
    pipeline: "operator_assisted_business_workspace"
    automatedProvisioning: false
    convergesWith: "CB-1.4 prefill pipeline (intake -> public-data research -> seeded workspace)"
    note: string
  }
}

/**
 * The offerings a draft may suggest: the three configured AI-employee modules
 * from the deal-rules config (pricingStatus verbatim) plus the KB's
 * operator-assisted business workspace packs. No prices anywhere.
 */
export type BlueprintOffering = {
  ref: string
  name: string
  kind: SuggestedModule["kind"]
  pricingStatus: string
  /** Embedded for the semantic need match. */
  descriptor: string
}

export function blueprintOfferings(
  config = DEFAULT_DEAL_RULES_CONFIG,
): BlueprintOffering[] {
  const modules: BlueprintOffering[] = config.modules.map((module) => ({
    ref: module.id,
    name: module.name,
    kind: "ai_employee_module",
    pricingStatus: module.pricingStatus,
    descriptor:
      module.id === "module.internal_operations_ai"
        ? "Internal operations AI employee: ops busywork, internal process automation, back-office triage, scheduling, reporting."
        : module.id === "module.customer_support_ai"
          ? "Customer support AI employee: support tickets, help desk, customer questions, response drafting, triage."
          : "Sales employee AI: lead follow-up, outreach, qualification, pipeline, closing conversations.",
  }))
  const packs: BlueprintOffering[] = [
    {
      ref: "workspace_pack.ecommerce",
      name: "E-commerce business workspace pack",
      kind: "workspace_pack",
      pricingStatus: "owner_pricing_required",
      descriptor:
        "E-commerce workspace: online store, inventory-aware ads, product catalog, shop marketing.",
    },
    {
      ref: "workspace_pack.legal",
      name: "Legal business workspace pack",
      kind: "workspace_pack",
      pricingStatus: "owner_pricing_required",
      descriptor:
        "Legal workspace: law firm intake, review-gated intake copilots, matter handling, document review.",
    },
    {
      ref: "workspace_pack.agency",
      name: "Agency business workspace pack",
      kind: "workspace_pack",
      pricingStatus: "owner_pricing_required",
      descriptor:
        "Agency workspace: white-label landing pages, email sequences, client campaigns, marketing agency delivery.",
    },
  ]
  return [...modules, ...packs]
}

export function moduleMatchMinSimilarity(): number {
  const raw = Number(process.env.SARAH_BLUEPRINT_MODULE_MIN_SIMILARITY ?? NaN)
  return Number.isFinite(raw) && raw > 0 && raw <= 1
    ? raw
    : MODULE_MATCH_MIN_SIMILARITY_DEFAULT
}

const offeringEmbeddings = new Map<string, number[]>()

/**
 * Semantic (embedding/cosine) mapping of stated needs onto offerings — the
 * workspace semantic-routing law; no keyword matching. Fail-soft: when the
 * embedder is unavailable, every offering is returned as an unmatched
 * default candidate so the draft stays honest about what was NOT inferred.
 */
export async function matchNeedsToOfferings(
  needs: CustomerBlueprintNeed[],
  offerings: BlueprintOffering[] = blueprintOfferings(),
): Promise<SuggestedModule[]> {
  const threshold = moduleMatchMinSimilarity()
  const matchedTurnIds = new Map<string, Set<string>>()
  let embedderAvailable = needs.length > 0

  for (const need of needs) {
    const needEmbedding = await sarahEmbedText(need.need, "RETRIEVAL_QUERY")
    if (!needEmbedding) {
      embedderAvailable = false
      break
    }
    for (const offering of offerings) {
      let embedding = offeringEmbeddings.get(offering.descriptor) ?? null
      if (!embedding) {
        embedding = await sarahEmbedText(offering.descriptor, "RETRIEVAL_DOCUMENT")
        if (!embedding) {
          embedderAvailable = false
          break
        }
        offeringEmbeddings.set(offering.descriptor, embedding)
      }
      const similarity = cosineSimilarity(needEmbedding, embedding)
      if (similarity < threshold) continue
      const bucket = matchedTurnIds.get(offering.ref) ?? new Set<string>()
      bucket.add(need.sourceTurnId)
      matchedTurnIds.set(offering.ref, bucket)
    }
    if (!embedderAvailable) break
  }

  if (embedderAvailable && matchedTurnIds.size > 0) {
    return offerings
      .filter((offering) => matchedTurnIds.has(offering.ref))
      .map((offering) => ({
        ref: offering.ref,
        name: offering.name,
        kind: offering.kind,
        availability: "operator_assisted",
        pricingStatus: offering.pricingStatus,
        matchedNeedTurnIds: [...(matchedTurnIds.get(offering.ref) ?? [])],
        matchBasis: "semantic",
      }))
  }

  // Honest degradation: list the catalog as candidates, claiming no match.
  return offerings.map((offering) => ({
    ref: offering.ref,
    name: offering.name,
    kind: offering.kind,
    availability: "operator_assisted",
    pricingStatus: offering.pricingStatus,
    matchedNeedTurnIds: [],
    matchBasis: "candidate_default",
  }))
}

/**
 * Deterministic parse of OUR OWN typed fact encoding ("<label>: \"quote\"",
 * written by distillProspectFacts). This is a bounded field of a value this
 * codebase produced — not user-intent routing.
 */
export function factLabel(fact: SarahProspectFact): string {
  const separator = fact.fact.indexOf(":")
  return separator > 0 ? fact.fact.slice(0, separator).trim() : "other"
}

/** Pure composer — unit-testable without a store or embedder. */
export function composeCustomerBlueprintDraft(input: {
  prospectRef: string
  facts: SarahProspectFact[]
  contact: { email: string | null; contactId: string | null } | null
  suggestedModules: SuggestedModule[]
  revision: number
  now?: string
}): CustomerBlueprintDraft {
  const needs: CustomerBlueprintNeed[] = input.facts
    .filter((fact) => factLabel(fact) === "need")
    .map((fact) => ({
      need: fact.fact,
      sourceTurnId: fact.sourceTurnId,
      at: fact.at,
    }))
  const businessFacts = input.facts.filter((fact) => factLabel(fact) !== "need")
  const turnIds = [...new Set(input.facts.map((fact) => fact.sourceTurnId))]
  return {
    schema: CUSTOMER_BLUEPRINT_SCHEMA,
    prospectRef: input.prospectRef,
    revision: input.revision,
    createdAt: input.now ?? new Date().toISOString(),
    business: { facts: businessFacts },
    contacts: {
      email: input.contact?.email ?? null,
      contactId: input.contact?.contactId ?? null,
    },
    needs,
    suggestedModules: input.suggestedModules,
    sources: {
      turnIds,
      factCount: input.facts.length,
      provenance:
        "sarah_prospect_profile + sarah_transcript_turns (per-fact source turn ids)",
    },
    handoff: {
      pipeline: "operator_assisted_business_workspace",
      automatedProvisioning: false,
      convergesWith:
        "CB-1.4 prefill pipeline (intake -> public-data research -> seeded workspace)",
      note:
        "Draft only. A human operator reviews this draft and seeds the actual business workspace; nothing is provisioned automatically, and setup pricing is owner-quoted.",
    },
  }
}

// ---------------------------------------------------------------------------
// Store reads (single-ref seams) + revisioned persistence, all fail-soft
// ---------------------------------------------------------------------------

type BlueprintStoreInputs = {
  profileFacts: SarahProspectFact[]
  contact: { email: string | null; contactId: string | null } | null
  turns: SarahMemoryTurnRow[]
  /** Highest stored revision for this prospect (0 when none). */
  latestRevision: number
}

type BlueprintStoreReader = (
  aliases: string[],
) => Promise<BlueprintStoreInputs | null>

let blueprintSchemaReady: Promise<boolean> | null = null

async function ensureBlueprintSchema(): Promise<boolean> {
  blueprintSchemaReady ??= (async () => {
    const ok = await readSarahStore(async (sql) => {
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_customer_blueprints (
          id BIGSERIAL PRIMARY KEY,
          prospect_ref TEXT NOT NULL,
          revision INTEGER NOT NULL,
          draft JSONB NOT NULL,
          created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
          UNIQUE (prospect_ref, revision)
        )`
      return true
    })
    return ok === true
  })()
  return blueprintSchemaReady
}

function asTurnRow(row: Record<string, unknown>): SarahMemoryTurnRow {
  const recordedAt = row.recorded_at
  return {
    id: String(row.id ?? ""),
    role: String(row.role ?? ""),
    modality: String(row.modality ?? ""),
    text: String(row.text ?? ""),
    recordedAt:
      recordedAt instanceof Date
        ? recordedAt.toISOString()
        : String(recordedAt ?? ""),
  }
}

/**
 * Every SQL read here is bound to `IN ${sql(aliases)}` where aliases is
 * exactly prospectRefAliases(prospectRef) — the KHS-3 single-ref query seam.
 */
const defaultStoreReader: BlueprintStoreReader = async (aliases) => {
  if (!(await ensureBlueprintSchema())) {
    // Blueprint table unavailable is not fatal for reads of the other tables.
  }
  return readSarahStore(async (sql: SQL) => {
    const profileRows = (await sql`
      SELECT facts
      FROM sarah_prospect_profile
      WHERE prospect_ref IN ${sql(aliases)}
      LIMIT ${aliases.length}
    `) as Array<{ facts: unknown }>
    const contactRows = (await sql`
      SELECT contact_email, contact_id
      FROM sarah_prospect_contacts
      WHERE prospect_ref IN ${sql(aliases)}
      LIMIT ${aliases.length}
    `) as Array<Record<string, unknown>>
    const turnRows = (await sql`
      SELECT id, role, modality, text, recorded_at
      FROM sarah_transcript_turns
      WHERE prospect_ref IN ${sql(aliases)}
      ORDER BY recorded_at DESC, id DESC
      LIMIT ${TURNS_QUERY_LIMIT}
    `) as Array<Record<string, unknown>>
    const revisionRows = (await sql`
      SELECT COALESCE(MAX(revision), 0) AS latest
      FROM sarah_customer_blueprints
      WHERE prospect_ref IN ${sql(aliases)}
    `) as Array<{ latest: number | string | null }>

    const profileFacts: SarahProspectFact[] = []
    for (const row of profileRows) {
      const parsed =
        typeof row.facts === "string" ? JSON.parse(row.facts) : row.facts
      if (!Array.isArray(parsed)) continue
      for (const entry of parsed as Array<Record<string, unknown>>) {
        if (typeof entry?.fact !== "string") continue
        profileFacts.push({
          fact: entry.fact,
          sourceTurnId: String(entry.source_turn_id ?? ""),
          at: String(entry.at ?? ""),
        })
      }
    }
    const contactRow = contactRows.find((row) => row.contact_email)
    return {
      profileFacts,
      contact: contactRow
        ? {
            email: String(contactRow.contact_email ?? "") || null,
            contactId:
              contactRow.contact_id == null
                ? null
                : String(contactRow.contact_id),
          }
        : null,
      turns: turnRows.map(asTurnRow).reverse(),
      latestRevision: Number(revisionRows[0]?.latest ?? 0) || 0,
    }
  })
}

let storeReader: BlueprintStoreReader = defaultStoreReader

type BlueprintWriter = (
  prospectRef: string,
  revision: number,
  draft: CustomerBlueprintDraft,
) => Promise<boolean>

const defaultWriter: BlueprintWriter = async (prospectRef, revision, draft) => {
  if (!(await ensureBlueprintSchema())) return false
  const ok = await readSarahStore(
    // Explicit ::jsonb cast on a stringified param — the proven Bun SQL
    // pattern for non-array JSON values (see semantic-answer-cache).
    (sql) => sql`
      INSERT INTO sarah_customer_blueprints (prospect_ref, revision, draft)
      VALUES (${prospectRef}, ${revision}, ${JSON.stringify(draft)}::jsonb)
      ON CONFLICT (prospect_ref, revision) DO NOTHING
    `,
  )
  return ok !== null
}

let writer: BlueprintWriter = defaultWriter

export type BuildCustomerBlueprintResult =
  | {
      ok: true
      draft: CustomerBlueprintDraft
      stored: boolean
      revision: number
    }
  | { ok: false; error: string }

/**
 * Build (and fail-soft persist) the customer Blueprint draft for ONE
 * prospect ref. This is the only entry point the tool path calls; every read
 * inside is bound to prospectRefAliases(prospectRef).
 */
export async function buildCustomerBlueprintDraft(
  prospectRef: string,
): Promise<BuildCustomerBlueprintResult> {
  const aliases = prospectRefAliases(prospectRef)
  if (aliases.length === 0) return { ok: false, error: "missing_prospect_ref" }

  const inputs = await storeReader(aliases)
  // Facts: the KHS-2 profile projection when present; otherwise a fresh
  // deterministic distillation from the prospect's own turns (same function
  // that fills the profile, so provenance is identical either way).
  const distilled = inputs ? distillProspectFacts(inputs.turns) : []
  const facts =
    inputs && inputs.profileFacts.length > 0 ? inputs.profileFacts : distilled
  const needs: CustomerBlueprintNeed[] = facts
    .filter((fact) => factLabel(fact) === "need")
    .map((fact) => ({
      need: fact.fact,
      sourceTurnId: fact.sourceTurnId,
      at: fact.at,
    }))

  const suggestedModules = await matchNeedsToOfferings(needs)
  const revision = (inputs?.latestRevision ?? 0) + 1
  const draft = composeCustomerBlueprintDraft({
    prospectRef: aliases[0]!,
    facts,
    contact: inputs?.contact ?? null,
    suggestedModules,
    revision,
  })

  const stored = await writer(aliases[0]!, revision, draft)

  // Surface card: the browser pops "Your Blueprint draft" as Sarah builds it.
  // Published to each deterministic encoding of this ONE identity (text lane
  // subscribes on the raw ref, avatar lane on prospect:<ref>).
  const semanticCount = draft.suggestedModules.filter(
    (module) => module.matchBasis === "semantic",
  ).length
  const summary = `${draft.needs.length} stated need${draft.needs.length === 1 ? "" : "s"} · ${
    semanticCount > 0
      ? `${semanticCount} matched module${semanticCount === 1 ? "" : "s"}`
      : `${draft.suggestedModules.length} candidate modules`
  } · sources: ${draft.sources.turnIds.length} turn${draft.sources.turnIds.length === 1 ? "" : "s"} · operator-assisted handoff (draft only)`
  for (const alias of aliases) {
    publishSarahAvatarEvent(alias, {
      type: "card",
      title: "Your Blueprint draft",
      body: summary,
    })
  }
  publishSarahBlueprintDelta(aliases, {
    kind: "draft_revision",
    revision,
    needsCount: draft.needs.length,
    matchedModules: draft.suggestedModules.map((module) => ({
      ref: module.ref,
      name: module.name,
      matchBasis: module.matchBasis,
      matchedNeedTurnIds: module.matchedNeedTurnIds,
    })),
  })

  return { ok: true, draft, stored, revision }
}

/**
 * Owner/operator listing (admin-bearer-guarded at the route). Intentionally
 * cross-prospect: owner-facing only, never feeds a prospect-facing serve path
 * (sarah.memory_query_scoped.v1 authority boundary).
 */
export async function listCustomerBlueprintsForOperator(): Promise<{
  blueprints: Array<{
    prospectRef: string
    revision: number
    createdAt: string
    draft: unknown
  }>
  storeConfigured: boolean
}> {
  if (!(await ensureBlueprintSchema())) {
    return { blueprints: [], storeConfigured: false }
  }
  const rows = await readSarahStore(async (sql) => {
    return (await sql`
      SELECT prospect_ref, revision, draft, created_at
      FROM sarah_customer_blueprints
      ORDER BY created_at DESC, id DESC
      LIMIT 50
    `) as Array<Record<string, unknown>>
  })
  return {
    blueprints: (rows ?? []).map((row) => ({
      prospectRef: String(row.prospect_ref ?? ""),
      revision: Number(row.revision ?? 0),
      createdAt:
        row.created_at instanceof Date
          ? row.created_at.toISOString()
          : String(row.created_at ?? ""),
      draft:
        typeof row.draft === "string" ? JSON.parse(row.draft) : row.draft,
    })),
    storeConfigured: rows !== null,
  }
}

// ---------------------------------------------------------------------------
// Test hooks
// ---------------------------------------------------------------------------

export function __setCustomerBlueprintStoreReaderForTest(
  fn: BlueprintStoreReader | null,
): void {
  storeReader = fn ?? defaultStoreReader
}

export function __setCustomerBlueprintWriterForTest(
  fn: BlueprintWriter | null,
): void {
  writer = fn ?? defaultWriter
}

export function __resetCustomerBlueprintForTest(): void {
  storeReader = defaultStoreReader
  writer = defaultWriter
  offeringEmbeddings.clear()
  blueprintSchemaReady = null
}
