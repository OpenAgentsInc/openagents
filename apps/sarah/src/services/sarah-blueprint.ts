/**
 * KHS-5 Sarah's own Blueprint (#8604, epic #8599).
 *
 * Replaces the flat pasted knowledge base with a typed, versioned knowledge
 * object — Blueprint-lite vocabulary per the P4 company-brain direction
 * (MASTER_ROADMAP P4 / AE-2.1; sovereignty analysis §3): typed facts with
 * per-fact provenance, versioned revisions, owner sign-off on changes. Sarah
 * is the flagship `ai_employee.v1` seed, so her promotion onto the formal
 * record becomes a data migration over objects that already have the right
 * joints.
 *
 * Model:
 * - `BlueprintFact` — one typed statement of knowledge/behavior. Every fact
 *   carries provenance ({source, ref?, at}); pricing facts carry
 *   `dealRuleRefs` into the deal-rule config; product/proof facts carry
 *   `promiseIds` into the public promise registry.
 * - `BlueprintRevision` — an immutable revision record {revision, createdAt,
 *   changedBy, changeNote}. Facts reference the revision that added them;
 *   RETIRING a fact is a new revision that flips status and stamps
 *   `revisionRetired` — facts are NEVER deleted.
 * - Compilation — `compileSarahSystemPrompt()` renders active facts into the
 *   system-prompt shape (Section A ordering preserved: identity → engine →
 *   hard rules; then knowledge; playbook last).
 *   `compileSarahKnowledgeBaseMarkdown()` regenerates the full KB doc
 *   (docs/sarah/SARAH_KNOWLEDGE_BASE.md is generated FROM the Blueprint, not
 *   hand-edited — see apps/sarah/scripts/render-kb-from-blueprint.ts).
 *
 * Storage posture matches turn-store/collective-learning: with a configured
 * database the object lives in `sarah_blueprint_facts` +
 * `sarah_blueprint_revisions` (schema-ensure, fail-soft); without one the
 * checked-in seed (apps/sarah/config/blueprint-seed.json, generated ONCE from
 * the owner's KB by scripts/seed-blueprint-from-kb.ts) serves from memory.
 * An empty store loads the seed as revision 1.
 *
 * Consumption is flag-armed (SARAH_BLUEPRINT=1) in sarah-instructions.ts;
 * flag-off leaves the current file-based path byte-identical (safe rollout).
 *
 * KHS-4 seam: an APPROVED learning candidate of kind `winning_answer` may be
 * promoted to a playbook fact whose provenance source is
 * `learning_receipt:<receiptId>` — the owner-approval receipt chain from
 * collective-learning.ts extends into the Blueprint unbroken.
 *
 * Authority unchanged: the openagents.com API stays the system of record for
 * CRM/credits/checkout; deal-rules code remains the only pricing authority —
 * blueprint facts can inform language, never prices.
 */

import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

import { readSarahStore, sarahTurnStoreStatus } from "./turn-store.ts"
import {
  listLearningCandidates,
  learningReceiptRef,
} from "./collective-learning.ts"

const appRoot = fileURLToPath(new URL("../..", import.meta.url))
const SEED_PATH = path.join(appRoot, "config/blueprint-seed.json")

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export const BLUEPRINT_SECTIONS = [
  "identity",
  "conversation_rules",
  "hard_rules",
  "company",
  "products",
  "pricing",
  "proof",
  "links",
  "playbook",
] as const

export type BlueprintSection = (typeof BLUEPRINT_SECTIONS)[number]

export type BlueprintFactFormat =
  | "paragraph"
  | "list_item"
  | "numbered_item"
  | "table_row"

/**
 * Where a fact came from. `learning_receipt:<id>` chains a fact back to a
 * KHS-4 owner-approval receipt.
 */
export const BLUEPRINT_SOURCE_PATTERN =
  /^(owner_kb_v2|owner_directive|promise_registry|deal_rules|learning_receipt:[A-Za-z0-9_.:-]+)$/

export type BlueprintProvenance = {
  source: string
  ref: string | null
  at: string
}

export type BlueprintFact = {
  id: string
  section: BlueprintSection
  /** Subsection heading the fact renders under (verbatim, no `## `). */
  heading: string | null
  format: BlueprintFactFormat
  /** table_row only: the verbatim header row of the table it belongs to. */
  tableHeader: string | null
  /** Global render order within the document (seeded facts spaced by 100). */
  position: number
  /** Verbatim markdown block, minus list/number markers on the first line. */
  statement: string
  provenance: BlueprintProvenance
  /** Pricing facts: refs into the deal-rule config (sarah.deal_rules.v1). */
  dealRuleRefs: string[]
  /** Product/proof facts: promise-registry record ids the claim rests on. */
  promiseIds: string[]
  status: "active" | "retired"
  revisionAdded: number
  revisionRetired: number | null
}

export type BlueprintRevision = {
  revision: number
  createdAt: string
  changedBy: string
  changeNote: string
}

export type SarahBlueprint = {
  currentRevision: number
  revisions: BlueprintRevision[]
  facts: BlueprintFact[]
}

export function blueprintFactId(
  section: BlueprintSection,
  heading: string | null,
  format: BlueprintFactFormat,
  statement: string,
): string {
  const digest = createHash("sha256")
    .update(`${section}|${heading ?? ""}|${format}|${statement}`)
    .digest("hex")
    .slice(0, 16)
  return `bf_${digest}`
}

// ---------------------------------------------------------------------------
// KB markdown parsing (used once by scripts/seed-blueprint-from-kb.ts, and by
// its tests; the runtime store never re-parses the doc)
// ---------------------------------------------------------------------------

export type ParsedKbBlock = {
  section: BlueprintSection
  heading: string | null
  format: BlueprintFactFormat
  tableHeader: string | null
  statement: string
}

type Banner = "A" | "B" | "C"

const BANNER_TITLES: Record<Banner, string> = {
  A: "SYSTEM PROMPT",
  B: "THE PLAYBOOK",
  C: "THE KNOWLEDGE",
}

function bannerForSection(section: BlueprintSection): Banner {
  if (
    section === "identity" ||
    section === "conversation_rules" ||
    section === "hard_rules"
  ) {
    return "A"
  }
  if (section === "playbook") return "B"
  return "C"
}

function sectionForHeading(
  banner: Banner,
  heading: string | null,
): BlueprintSection {
  if (banner === "A") {
    if (!heading) return "identity"
    if (heading.startsWith("Hard rules")) return "hard_rules"
    return "conversation_rules"
  }
  if (banner === "B") return "playbook"
  if (!heading) return "company"
  if (heading.startsWith("C.2")) return "products"
  if (heading.startsWith("C.3")) return "proof"
  if (heading.startsWith("C.4")) return "pricing"
  if (heading.startsWith("C.5")) return "proof"
  if (heading.startsWith("C.6")) return "links"
  return "company"
}

const TABLE_SEPARATOR_PATTERN = /^\|[\s:|-]+\|$/
const NUMBERED_ITEM_PATTERN = /^(\d+)\.\s+/

/**
 * Parse the single-paste KB document into typed blocks. Statements keep the
 * verbatim line content (including hard wraps and continuation indents) so
 * the compiled document stays deterministic and nearly identical to the
 * source; list/number markers are stripped from first lines and re-rendered.
 */
export function parseSarahKnowledgeBaseMarkdown(
  markdown: string,
): ParsedKbBlock[] {
  const blocks: ParsedKbBlock[] = []
  let banner: Banner | null = null
  let heading: string | null = null
  let inComment = false
  let current: ParsedKbBlock | null = null
  let tableHeader: string | null = null
  let tableSeparatorSeen = false

  const flush = () => {
    if (current && current.statement.trim().length > 0) blocks.push(current)
    current = null
  }
  const resetTable = () => {
    tableHeader = null
    tableSeparatorSeen = false
  }

  for (const rawLine of markdown.split("\n")) {
    const line = rawLine.replace(/\s+$/, "")
    if (inComment) {
      if (line.includes("-->")) inComment = false
      continue
    }
    if (line.startsWith("<!--")) {
      if (!line.includes("-->")) inComment = true
      continue
    }
    if (line.trim() === "") {
      flush()
      resetTable()
      continue
    }
    if (line.startsWith("# ")) {
      flush()
      resetTable()
      const match = line.match(/^# SECTION ([ABC])\b/)
      banner = match ? (match[1] as Banner) : banner
      heading = null
      continue
    }
    if (line === "---") {
      flush()
      resetTable()
      continue
    }
    if (line.startsWith("## ")) {
      flush()
      resetTable()
      heading = line.slice(3)
      continue
    }
    if (!banner) continue // preamble outside the SECTION structure

    const section = sectionForHeading(banner, heading)
    if (line.startsWith("|")) {
      if (!tableHeader) {
        flush()
        tableHeader = line
        continue
      }
      if (!tableSeparatorSeen && TABLE_SEPARATOR_PATTERN.test(line)) {
        tableSeparatorSeen = true
        continue
      }
      flush()
      blocks.push({
        section,
        heading,
        format: "table_row",
        tableHeader,
        statement: line,
      })
      continue
    }
    if (line.startsWith("- ")) {
      flush()
      current = {
        section,
        heading,
        format: "list_item",
        tableHeader: null,
        statement: line.slice(2),
      }
      continue
    }
    const numbered = line.match(NUMBERED_ITEM_PATTERN)
    if (numbered) {
      flush()
      current = {
        section,
        heading,
        format: "numbered_item",
        tableHeader: null,
        statement: line.slice(numbered[0].length),
      }
      continue
    }
    if (current) {
      current.statement += `\n${line}`
    } else {
      current = {
        section,
        heading,
        format: "paragraph",
        tableHeader: null,
        statement: line,
      }
    }
  }
  flush()
  return blocks
}

// ---------------------------------------------------------------------------
// Compilation (pure renderers over facts)
// ---------------------------------------------------------------------------

function tableSeparatorFor(header: string): string {
  const columns = header.split("|").length - 2
  return `|${"---|".repeat(Math.max(columns, 1))}`
}

function renderFactLines(
  facts: BlueprintFact[],
  options: { banners: boolean },
): string[] {
  const lines: string[] = []
  let currentBanner: Banner | null = null
  let currentHeading: string | null | undefined
  let previous: BlueprintFact | null = null
  let numberCounter = 0
  let openTableHeader: string | null = null

  for (const fact of facts) {
    const banner = bannerForSection(fact.section)
    if (banner !== currentBanner) {
      if (options.banners) {
        if (lines.length > 0) lines.push("")
        lines.push("---", "", `# SECTION ${banner} — ${BANNER_TITLES[banner]}`)
      }
      currentBanner = banner
      currentHeading = undefined
      previous = options.banners ? null : previous
    }
    if (fact.heading !== currentHeading) {
      if (fact.heading !== null) {
        if (lines.length > 0) lines.push("")
        lines.push(`## ${fact.heading}`)
        previous = null
      }
      currentHeading = fact.heading
      numberCounter = 0
      openTableHeader = null
    }

    const sameRun =
      previous !== null &&
      previous.format === fact.format &&
      (fact.format === "list_item" ||
        fact.format === "numbered_item" ||
        fact.format === "table_row")
    if (!sameRun) {
      if (previous !== null || lines.length > 0) lines.push("")
      numberCounter = 0
      openTableHeader = null
    }

    switch (fact.format) {
      case "paragraph":
        lines.push(...fact.statement.split("\n"))
        break
      case "list_item": {
        const [first, ...rest] = fact.statement.split("\n")
        lines.push(`- ${first}`, ...rest)
        break
      }
      case "numbered_item": {
        numberCounter += 1
        const [first, ...rest] = fact.statement.split("\n")
        lines.push(`${numberCounter}. ${first}`, ...rest)
        break
      }
      case "table_row": {
        const header = fact.tableHeader ?? "| | |"
        if (openTableHeader !== header) {
          lines.push(header, tableSeparatorFor(header))
          openTableHeader = header
        }
        lines.push(fact.statement)
        break
      }
    }
    previous = fact
  }
  return lines
}

function sortedActiveFacts(
  facts: BlueprintFact[],
  order: "document" | "system_prompt",
): BlueprintFact[] {
  const bannerPriority: Record<Banner, number> = { A: 0, C: 1, B: 2 }
  return facts
    .filter((fact) => fact.status === "active")
    .sort((a, b) => {
      if (order === "system_prompt") {
        const delta =
          bannerPriority[bannerForSection(a.section)] -
          bannerPriority[bannerForSection(b.section)]
        if (delta !== 0) return delta
      }
      if (a.position !== b.position) return a.position - b.position
      return a.id.localeCompare(b.id)
    })
}

/**
 * Deterministic render of active facts into the system-prompt shape:
 * Section A ordering preserved (identity → engine → hard rules), then the
 * knowledge sections, playbook last. Pure given a fact list; the async
 * wrapper loads the current blueprint.
 */
export function renderSarahSystemPrompt(facts: BlueprintFact[]): string {
  const ordered = sortedActiveFacts(facts, "system_prompt")
  return renderFactLines(ordered, { banners: false }).join("\n").trim()
}

export async function compileSarahSystemPrompt(): Promise<string | null> {
  const blueprint = await loadSarahBlueprint()
  if (blueprint.facts.length === 0) return null
  const prompt = renderSarahSystemPrompt(blueprint.facts)
  return prompt.length > 0 ? prompt : null
}

/** Regenerates the full KB doc (docs/sarah/SARAH_KNOWLEDGE_BASE.md). */
export function renderSarahKnowledgeBaseMarkdown(
  blueprint: SarahBlueprint,
): string {
  const active = sortedActiveFacts(blueprint.facts, "document")
  const retiredCount = blueprint.facts.length - active.length
  const header = [
    "# Sarah Knowledge Base + System Prompt — single-paste document for voice/chat agent surfaces",
    "",
    "<!--",
    "  GENERATED FILE — do not hand-edit (KHS-5 #8604).",
    "  Compiled from Sarah's Blueprint: the typed, versioned knowledge object",
    "  with per-fact provenance in apps/sarah/src/services/sarah-blueprint.ts",
    `  (seed: apps/sarah/config/blueprint-seed.json; blueprint revision ${blueprint.currentRevision};`,
    `  ${active.length} active facts, ${retiredCount} retired).`,
    "  Regenerate with: bun apps/sarah/scripts/render-kb-from-blueprint.ts",
    "  Edit path: the admin-guarded /sarah/api/operator/blueprint endpoints",
    "  (add/retire facts with a change note -> new receipted revision).",
    "  Section A is the system prompt (how Sarah behaves and DRIVES the",
    "  conversation). Section B is the playbook. Section C is the knowledge.",
    "  Sources of truth: the blueprint seed (owner_kb_v2), deal rules",
    "  (apps/sarah/src/services/deal-rules.ts), and the live promise registry",
    "  (https://openagents.com/api/public/product-promises).",
    "  Public-safe: no secrets, no customer data, no internal-only claims.",
    "-->",
    "",
  ]
  const body = renderFactLines(active, { banners: true })
  return [...header, ...body, ""].join("\n")
}

export async function compileSarahKnowledgeBaseMarkdown(): Promise<
  string | null
> {
  const blueprint = await loadSarahBlueprint()
  if (blueprint.facts.length === 0) return null
  return renderSarahKnowledgeBaseMarkdown(blueprint)
}

// ---------------------------------------------------------------------------
// Store (Postgres when configured, seed-backed memory otherwise; fail-soft)
// ---------------------------------------------------------------------------

type SeedFile = {
  schema?: string
  revision?: Partial<BlueprintRevision>
  facts?: Array<Partial<BlueprintFact>>
}

let memoryFacts = new Map<string, BlueprintFact>()
let memoryRevisions: BlueprintRevision[] = []
let seededPromise: Promise<void> | null = null
let blueprintSchemaReady: Promise<boolean> | null = null
let lastError: string | null = null

export function sarahBlueprintStoreMode(): "postgres" | "memory_ephemeral" {
  return sarahTurnStoreStatus().configured ? "postgres" : "memory_ephemeral"
}

export function sarahBlueprintEnabled(): boolean {
  return process.env.SARAH_BLUEPRINT === "1"
}

async function loadSeed(): Promise<{
  revision: BlueprintRevision
  facts: BlueprintFact[]
} | null> {
  try {
    const raw = JSON.parse(await readFile(SEED_PATH, "utf8")) as SeedFile
    const revision: BlueprintRevision = {
      revision: raw.revision?.revision ?? 1,
      createdAt: raw.revision?.createdAt ?? "1970-01-01T00:00:00.000Z",
      changedBy: raw.revision?.changedBy ?? "owner",
      changeNote: raw.revision?.changeNote ?? "seed",
    }
    const facts: BlueprintFact[] = []
    for (const fact of raw.facts ?? []) {
      if (!fact.id || !fact.section || !fact.statement) continue
      if (!fact.provenance?.source || !fact.provenance.at) continue
      facts.push({
        id: fact.id,
        section: fact.section,
        heading: fact.heading ?? null,
        format: fact.format ?? "paragraph",
        tableHeader: fact.tableHeader ?? null,
        position: fact.position ?? 0,
        statement: fact.statement,
        provenance: {
          source: fact.provenance.source,
          ref: fact.provenance.ref ?? null,
          at: fact.provenance.at,
        },
        dealRuleRefs: fact.dealRuleRefs ?? [],
        promiseIds: fact.promiseIds ?? [],
        status: fact.status ?? "active",
        revisionAdded: fact.revisionAdded ?? revision.revision,
        revisionRetired: fact.revisionRetired ?? null,
      })
    }
    if (facts.length === 0) return null
    return { revision, facts }
  } catch (error) {
    lastError = error instanceof Error ? error.message : String(error)
    return null
  }
}

async function ensureBlueprintSchema(): Promise<boolean> {
  blueprintSchemaReady ??= (async () => {
    const ok = await readSarahStore(async (sql) => {
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_blueprint_revisions (
          revision INTEGER PRIMARY KEY,
          created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
          changed_by TEXT NOT NULL,
          change_note TEXT NOT NULL
        )`
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_blueprint_facts (
          id TEXT PRIMARY KEY,
          section TEXT NOT NULL,
          heading TEXT,
          format TEXT NOT NULL DEFAULT 'paragraph',
          table_header TEXT,
          position INTEGER NOT NULL DEFAULT 0,
          statement TEXT NOT NULL,
          provenance_source TEXT NOT NULL,
          provenance_ref TEXT,
          provenance_at TEXT NOT NULL,
          deal_rule_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
          promise_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
          status TEXT NOT NULL DEFAULT 'active',
          revision_added INTEGER NOT NULL,
          revision_retired INTEGER
        )`
      return true
    })
    return ok === true
  })()
  return blueprintSchemaReady
}

function rowToFact(row: Record<string, unknown>): BlueprintFact {
  const jsonArray = (value: unknown): string[] => {
    const parsed = typeof value === "string" ? JSON.parse(value) : value
    return Array.isArray(parsed) ? parsed.map(String) : []
  }
  return {
    id: String(row.id),
    section: String(row.section) as BlueprintSection,
    heading: row.heading == null ? null : String(row.heading),
    format: String(row.format ?? "paragraph") as BlueprintFactFormat,
    tableHeader: row.table_header == null ? null : String(row.table_header),
    position: Number(row.position ?? 0),
    statement: String(row.statement),
    provenance: {
      source: String(row.provenance_source),
      ref: row.provenance_ref == null ? null : String(row.provenance_ref),
      at: String(row.provenance_at),
    },
    dealRuleRefs: jsonArray(row.deal_rule_refs),
    promiseIds: jsonArray(row.promise_ids),
    status: String(row.status ?? "active") as "active" | "retired",
    revisionAdded: Number(row.revision_added ?? 1),
    revisionRetired:
      row.revision_retired == null ? null : Number(row.revision_retired),
  }
}

async function insertFactRow(fact: BlueprintFact): Promise<boolean> {
  const ok = await readSarahStore(async (sql) => {
    await sql`
      INSERT INTO sarah_blueprint_facts
        (id, section, heading, format, table_header, position, statement,
         provenance_source, provenance_ref, provenance_at,
         deal_rule_refs, promise_ids, status, revision_added, revision_retired)
      VALUES
        (${fact.id}, ${fact.section}, ${fact.heading}, ${fact.format},
         ${fact.tableHeader}, ${fact.position}, ${fact.statement},
         ${fact.provenance.source}, ${fact.provenance.ref},
         ${fact.provenance.at}, ${fact.dealRuleRefs}, ${fact.promiseIds},
         ${fact.status}, ${fact.revisionAdded}, ${fact.revisionRetired})
      ON CONFLICT (id) DO NOTHING`
    return true
  })
  return ok === true
}

async function insertRevisionRow(
  revision: BlueprintRevision,
): Promise<boolean> {
  const ok = await readSarahStore(async (sql) => {
    await sql`
      INSERT INTO sarah_blueprint_revisions
        (revision, created_at, changed_by, change_note)
      VALUES
        (${revision.revision}, ${revision.createdAt}, ${revision.changedBy},
         ${revision.changeNote})
      ON CONFLICT (revision) DO NOTHING`
    return true
  })
  return ok === true
}

/** Empty store → the checked-in seed loads as revision 1. */
async function ensureSeeded(): Promise<void> {
  seededPromise ??= (async () => {
    if (sarahBlueprintStoreMode() === "memory_ephemeral") {
      if (memoryRevisions.length > 0) return
      const seed = await loadSeed()
      if (!seed) return
      memoryRevisions = [seed.revision]
      memoryFacts = new Map(seed.facts.map((fact) => [fact.id, fact]))
      return
    }
    if (!(await ensureBlueprintSchema())) return
    const existing = await readSarahStore(async (sql) => {
      const rows = (await sql`
        SELECT revision FROM sarah_blueprint_revisions LIMIT 1
      `) as Array<Record<string, unknown>>
      return rows.length
    })
    if (existing == null || existing > 0) return
    const seed = await loadSeed()
    if (!seed) return
    await insertRevisionRow(seed.revision)
    for (const fact of seed.facts) await insertFactRow(fact)
  })()
  return seededPromise
}

export async function loadSarahBlueprint(): Promise<SarahBlueprint> {
  await ensureSeeded()
  if (sarahBlueprintStoreMode() === "memory_ephemeral") {
    const revisions = [...memoryRevisions].sort(
      (a, b) => a.revision - b.revision,
    )
    return {
      currentRevision: revisions.at(-1)?.revision ?? 0,
      revisions,
      facts: [...memoryFacts.values()].sort(
        (a, b) => a.position - b.position || a.id.localeCompare(b.id),
      ),
    }
  }
  if (!(await ensureBlueprintSchema())) {
    return { currentRevision: 0, revisions: [], facts: [] }
  }
  const loaded = await readSarahStore(async (sql) => {
    const revisionRows = (await sql`
      SELECT revision, created_at, changed_by, change_note
      FROM sarah_blueprint_revisions ORDER BY revision ASC
    `) as Array<Record<string, unknown>>
    const factRows = (await sql`
      SELECT * FROM sarah_blueprint_facts ORDER BY position ASC, id ASC
    `) as Array<Record<string, unknown>>
    return { revisionRows, factRows }
  })
  if (!loaded) return { currentRevision: 0, revisions: [], facts: [] }
  const revisions = loaded.revisionRows.map((row) => ({
    revision: Number(row.revision),
    createdAt:
      row.created_at instanceof Date
        ? row.created_at.toISOString()
        : String(row.created_at ?? ""),
    changedBy: String(row.changed_by ?? ""),
    changeNote: String(row.change_note ?? ""),
  }))
  return {
    currentRevision: revisions.at(-1)?.revision ?? 0,
    revisions,
    facts: loaded.factRows.map(rowToFact),
  }
}

// ---------------------------------------------------------------------------
// Revision-writing mutations (retire is a new revision, never a delete)
// ---------------------------------------------------------------------------

export type BlueprintMutationResult =
  | { ok: true; revision: BlueprintRevision; fact: BlueprintFact }
  | { ok: false; error: string }

async function writeRevision(
  changedBy: string,
  changeNote: string,
  currentRevision: number,
): Promise<BlueprintRevision | null> {
  const revision: BlueprintRevision = {
    revision: currentRevision + 1,
    createdAt: new Date().toISOString(),
    changedBy,
    changeNote,
  }
  if (sarahBlueprintStoreMode() === "memory_ephemeral") {
    memoryRevisions.push(revision)
    return revision
  }
  return (await insertRevisionRow(revision)) ? revision : null
}

export async function addSarahBlueprintFact(input: {
  section: string
  statement: string
  heading?: string | null
  format?: BlueprintFactFormat
  source: string
  ref?: string | null
  dealRuleRefs?: string[]
  promiseIds?: string[]
  by: string
  changeNote: string
}): Promise<BlueprintMutationResult> {
  const by = input.by?.trim()
  const changeNote = input.changeNote?.trim()
  const statement = input.statement?.trim()
  const source = input.source?.trim()
  if (!by) return { ok: false, error: "changed_by_required" }
  if (!changeNote) return { ok: false, error: "change_note_required" }
  if (!statement) return { ok: false, error: "statement_required" }
  // Provenance is mandatory on every fact — no anonymous knowledge.
  if (!source || !BLUEPRINT_SOURCE_PATTERN.test(source)) {
    return { ok: false, error: "provenance_source_invalid" }
  }
  if (!BLUEPRINT_SECTIONS.includes(input.section as BlueprintSection)) {
    return { ok: false, error: "section_invalid" }
  }
  const section = input.section as BlueprintSection
  const format = input.format ?? "paragraph"
  const heading = input.heading?.trim() || null

  const blueprint = await loadSarahBlueprint()
  if (blueprint.currentRevision === 0) {
    return { ok: false, error: "blueprint_store_unavailable" }
  }
  const id = blueprintFactId(section, heading, format, statement)
  if (blueprint.facts.some((fact) => fact.id === id)) {
    return { ok: false, error: "fact_already_exists" }
  }
  const sectionPositions = blueprint.facts
    .filter((fact) => fact.section === section)
    .map((fact) => fact.position)
  const position =
    sectionPositions.length > 0
      ? Math.max(...sectionPositions) + 1
      : (Math.max(0, ...blueprint.facts.map((fact) => fact.position)) + 100)

  const revision = await writeRevision(
    by,
    changeNote,
    blueprint.currentRevision,
  )
  if (!revision) return { ok: false, error: "store_write_failed" }
  const fact: BlueprintFact = {
    id,
    section,
    heading,
    format,
    tableHeader: null,
    position,
    statement,
    provenance: {
      source,
      ref: input.ref?.trim() || null,
      at: revision.createdAt,
    },
    dealRuleRefs: input.dealRuleRefs ?? [],
    promiseIds: input.promiseIds ?? [],
    status: "active",
    revisionAdded: revision.revision,
    revisionRetired: null,
  }
  if (sarahBlueprintStoreMode() === "memory_ephemeral") {
    memoryFacts.set(fact.id, fact)
    return { ok: true, revision, fact }
  }
  if (!(await insertFactRow(fact))) {
    return { ok: false, error: "store_write_failed" }
  }
  return { ok: true, revision, fact }
}

export async function retireSarahBlueprintFact(input: {
  factId: string
  by: string
  changeNote: string
}): Promise<BlueprintMutationResult> {
  const by = input.by?.trim()
  const changeNote = input.changeNote?.trim()
  if (!by) return { ok: false, error: "changed_by_required" }
  if (!changeNote) return { ok: false, error: "change_note_required" }
  const blueprint = await loadSarahBlueprint()
  const fact = blueprint.facts.find((entry) => entry.id === input.factId)
  if (!fact) return { ok: false, error: "fact_not_found" }
  if (fact.status !== "active") return { ok: false, error: "already_retired" }

  const revision = await writeRevision(
    by,
    changeNote,
    blueprint.currentRevision,
  )
  if (!revision) return { ok: false, error: "store_write_failed" }
  const retired: BlueprintFact = {
    ...fact,
    status: "retired",
    revisionRetired: revision.revision,
  }
  if (sarahBlueprintStoreMode() === "memory_ephemeral") {
    memoryFacts.set(retired.id, retired)
    return { ok: true, revision, fact: retired }
  }
  const ok = await readSarahStore(async (sql) => {
    await sql`
      UPDATE sarah_blueprint_facts
      SET status = 'retired', revision_retired = ${revision.revision}
      WHERE id = ${retired.id}`
    return true
  })
  if (ok !== true) return { ok: false, error: "store_write_failed" }
  return { ok: true, revision, fact: retired }
}

/**
 * KHS-4 → KHS-5 seam: promote an owner-APPROVED learning candidate of kind
 * `winning_answer` into a playbook fact. Provenance is the approval receipt
 * (`learning_receipt:<id>`), so the fact dereferences back to the owner
 * decision and the redacted source turns. Additive: the answer bank entry
 * created at approval time is untouched.
 */
export async function promoteLearningToBlueprintFact(input: {
  candidateId: string
  by: string
  changeNote?: string
}): Promise<BlueprintMutationResult> {
  const approved = await listLearningCandidates("approved")
  const candidate = approved.find((entry) => entry.id === input.candidateId)
  if (!candidate) return { ok: false, error: "approved_candidate_not_found" }
  if (candidate.kind !== "winning_answer") {
    return { ok: false, error: "candidate_kind_not_promotable" }
  }
  if (!candidate.receiptId) return { ok: false, error: "receipt_missing" }
  if (!candidate.questionCanonical || !candidate.proposedAnswer) {
    return { ok: false, error: "candidate_incomplete" }
  }
  return addSarahBlueprintFact({
    section: "playbook",
    heading: "Learned winning answers (owner-approved)",
    format: "paragraph",
    statement: `**Prospect asks:** ${candidate.questionCanonical}\n**Approved answer:** ${candidate.proposedAnswer}`,
    source: learningReceiptRef(candidate.receiptId),
    ref: `sarah_learning_candidates:${candidate.id}`,
    by: input.by,
    changeNote:
      input.changeNote?.trim() ||
      `Promoted approved learning ${candidate.id} to playbook fact`,
  })
}

// ---------------------------------------------------------------------------
// Ops + test hooks
// ---------------------------------------------------------------------------

export function sarahBlueprintStatus() {
  return {
    enabled: sarahBlueprintEnabled(),
    storeMode: sarahBlueprintStoreMode(),
    seedPath: "apps/sarah/config/blueprint-seed.json",
    editPath: "/sarah/api/operator/blueprint (admin-guarded, receipted revisions)",
    lastError,
  }
}

export function __resetSarahBlueprintForTest(): void {
  memoryFacts = new Map()
  memoryRevisions = []
  seededPromise = null
  blueprintSchemaReady = null
  lastError = null
}
