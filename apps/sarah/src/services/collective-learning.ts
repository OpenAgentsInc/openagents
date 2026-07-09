/**
 * KHS-4 owner-approved collective learning (#8603, epic #8599).
 *
 * The ONLY way anything Sarah hears from one prospect can ever inform another
 * prospect's conversation is through this pipeline:
 *
 *   distill (deterministic, PII-redacted, cross-prospect recurrence only)
 *     → `sarah_learning_candidates` (status=pending)
 *     → owner decision on the admin-guarded operator endpoints
 *     → approval receipt row in `sarah_learning_receipts`
 *     → (question/answer kinds) `sarah_answer_bank` entry whose `approved_by`
 *       is the receipt ref, so a live answer dereferences back to its
 *       approval receipt and redacted source turns.
 *
 * Law (contract `sarah.collective_learning_owner_gated.v1`, enforced by
 * collective-learning.test.ts):
 * - NOTHING generalizes without an approval receipt. The shared read paths
 *   (`listApprovedLearnings` here; the KHS-6 answer bank) surface only
 *   approved entries; pending/rejected candidates are unreachable from any
 *   serve path.
 * - PII never enters candidates: examples are conservatively redacted
 *   (emails/phones/long digit runs/URLs) and DROPPED entirely when a name
 *   introduction or leftover marker survives redaction. When in doubt, drop.
 * - The approve/reject/distill operator endpoints are admin-bearer-guarded
 *   in server.ts (fail closed: unarmed → 503, wrong bearer → 401).
 * - Registry honesty: this is an internal owner-approved store. It makes no
 *   public "learning from conversations" claim while the
 *   `data.khala_free_tier_trace_capture.v1` promise family stays yellow.
 *
 * Semantic-routing invariant note: candidate GROUPING reuses the KHS-6
 * embedding client (cosine similarity over `sarahEmbedText`); the cue
 * patterns below only nominate turns for offline owner review — they never
 * route a user-facing response, select an answer, or gate serve behavior.
 *
 * Cross-prospect READ boundary: `distillLearningCandidates` deliberately
 * scans `sarah_transcript_turns` across prospects. That is an owner/operator
 * pipeline expressly permitted by `sarah.memory_query_scoped.v1`'s authority
 * boundary — its output is redacted, pending, and unreachable from any
 * prospect-facing path until the owner approves it here.
 *
 * Storage posture matches turn-store: with a configured database the
 * candidates/receipts live in `sarah_*` Postgres tables (schema-ensure,
 * fail-soft); without one (local dev, tests) an in-process ephemeral store
 * keeps the workflow testable and is reported as `memory_ephemeral` on the
 * ops surface so a silently-degraded prod is visible.
 */

import { createHash } from "node:crypto"

import { PRICING_GUARD_PATTERN } from "../llm-openai-compat.ts"
import { readSarahStore, sarahTurnStoreStatus } from "./turn-store.ts"
import {
  addApprovedSarahAnswer,
  cosineSimilarity,
  listSarahAnswerBank,
  sarahEmbedText,
} from "./semantic-answer-cache.ts"

export type LearningCandidateKind =
  | "question_gap"
  | "objection"
  | "winning_answer"
  // SQ-6 taxonomy expansions (distill may not yet emit all of these):
  | "pain_phrase"
  | "product_mapping"
  | "bad_fit_signal"
  | "follow_up_phrasing"

/** Owner-facing taxonomy labels for the learning queue (SQ-6 / #8623). */
export type LearningTaxonomy =
  | "objection"
  | "winning_answer"
  | "pain_phrase"
  | "product_mapping"
  | "bad_fit_signal"
  | "follow_up_phrasing"

export type LearningCandidateStatus = "pending" | "approved" | "rejected"

export type LearningCandidate = {
  id: string
  kind: LearningCandidateKind
  /** SQ-6 review taxonomy (maps from kind; stable for operator UX). */
  taxonomy: LearningTaxonomy
  /** Deterministic "why this should generalize" rationale for owner review. */
  whyGeneralize: string
  /** Count of redacted examples after PII scrub. */
  exampleCount: number
  /** ISO timestamp of the newest source turn (source recency). */
  sourceRecency: string | null
  summary: string
  /** PII-scrubbed verbatim examples; every entry passed redaction. */
  redactedExamples: string[]
  /** sarah_transcript_turns ids (provenance back to the redacted source). */
  sourceTurnIds: string[]
  /** For bank-publishable kinds: the canonical question (redacted). */
  questionCanonical: string | null
  /** winning_answer kind: Sarah's own answer text (redacted). */
  proposedAnswer: string | null
  status: LearningCandidateStatus
  decidedBy: string | null
  decidedAt: string | null
  /** Approval/rejection receipt id once decided. */
  receiptId: string | null
  createdAt: string
}

/** Map storage/distill kind → operator taxonomy (SQ-6). */
export function taxonomyForKind(kind: LearningCandidateKind): LearningTaxonomy {
  switch (kind) {
    case "question_gap":
    case "pain_phrase":
      return "pain_phrase"
    case "objection":
      return "objection"
    case "winning_answer":
      return "winning_answer"
    case "product_mapping":
      return "product_mapping"
    case "bad_fit_signal":
      return "bad_fit_signal"
    case "follow_up_phrasing":
      return "follow_up_phrasing"
    default:
      return "pain_phrase"
  }
}

/** Deterministic generalization rationale shown to the owner at review time. */
export function buildWhyGeneralize(input: {
  kind: LearningCandidateKind
  prospectCount: number
  summary: string
}): string {
  const tax = taxonomyForKind(input.kind)
  return (
    `Taxonomy=${tax}; observed across ${input.prospectCount} prospect(s). ` +
    `Generalize only if the redacted examples share the same intent without ` +
    `prospect-specific facts. Candidate: ${clip(input.summary, 160)}`
  )
}

/** Newest source-turn timestamp (ISO), or null when unknown. */
export function computeSourceRecency(
  turns: ReadonlyArray<{ recordedAt: string }>,
): string | null {
  if (turns.length === 0) return null
  return turns
    .map((t) => t.recordedAt)
    .filter(Boolean)
    .sort()
    .at(-1) ?? null
}

/**
 * True when a newly approved answer would materially change style vs an
 * existing bank answer for the same normalized question (SQ-6 regression gate).
 */
export function isMaterialStyleChange(
  existingAnswer: string | null | undefined,
  newAnswer: string,
): boolean {
  if (!existingAnswer) return true
  const a = normalizeLearningText(existingAnswer)
  const b = normalizeLearningText(newAnswer)
  if (a === b) return false
  // >30% token edit distance proxy: different length band or low overlap
  const aTok = new Set(a.split(" ").filter(Boolean))
  const bTok = new Set(b.split(" ").filter(Boolean))
  if (aTok.size === 0 || bTok.size === 0) return true
  let inter = 0
  for (const t of aTok) if (bTok.has(t)) inter += 1
  const overlap = inter / Math.max(aTok.size, bTok.size)
  return overlap < 0.7
}

export type LearningRegressionFixture = {
  schema: "sarah.learning_style_regression.v1"
  createdAt: string
  candidateId: string
  receiptId: string
  questionCanonical: string
  previousAnswer: string | null
  newAnswer: string
  reason: string
}

export type LearningReceipt = {
  id: string
  candidateId: string
  decision: "approved" | "rejected"
  decidedBy: string
  reason: string | null
  /** sarah_answer_bank id when the approval published a bank entry. */
  bankEntryId: string | null
  createdAt: string
}

export type LearningTurnRow = {
  id: string
  prospectRef: string
  role: string
  text: string
  recordedAt: string
}

export const learningReceiptRef = (receiptId: string): string =>
  `learning_receipt:${receiptId}`

// ---------------------------------------------------------------------------
// PII redaction (pure; conservative — when in doubt, drop)
// ---------------------------------------------------------------------------

const EMAIL_PATTERN = /[\w.+-]+@[\w-]+(?:\.[\w-]+)+/g
const URL_PATTERN = /(?:https?:\/\/|www\.)\S+/gi
const PHONE_PATTERN = /(?:\+?\d[\s().\/-]{0,2}){7,}\d/g
const LONG_DIGIT_PATTERN = /\d{6,}/g
/**
 * Name introductions drop the whole example: replacing just the name still
 * leaks that a specific person said the rest verbatim, and v1 has no safe
 * way to be sure it caught the full name. Intro cues cover normal
 * capitalizations; the name itself must be capitalized ("I'm Chris" drops,
 * "I'm looking for a tool" survives).
 */
const NAME_INTRO_PATTERN =
  /(?:[Mm]y name(?:'s|’s| is)|I'm|I’m|I am|[Tt]his is|[Cc]all me|[Yy]ou can call me|[Ss]peaking with)\s+[A-Z][a-z]+/

const EXAMPLE_MAX_CHARS = 200

function clip(text: string, max: number): string {
  const normalized = text.replace(/\s+/g, " ").trim()
  if (normalized.length <= max) return normalized
  return `${normalized.slice(0, max - 1).trimEnd()}…`
}

/**
 * Redact one candidate example. Returns the scrubbed text, or null when the
 * example must be dropped entirely (name introductions, or any residue that
 * still looks like contact data after scrubbing).
 */
export function redactLearningExample(text: string): string | null {
  const original = text.trim()
  if (!original) return null
  if (NAME_INTRO_PATTERN.test(original)) return null
  let scrubbed = original
    .replace(EMAIL_PATTERN, "[redacted-email]")
    .replace(URL_PATTERN, "[redacted-url]")
    .replace(PHONE_PATTERN, "[redacted-phone]")
    .replace(LONG_DIGIT_PATTERN, "[redacted-number]")
  // Anything @-shaped that survived the email pattern is dropped, not kept.
  if (scrubbed.replace(/\[redacted-[a-z]+\]/g, "").includes("@")) return null
  scrubbed = clip(scrubbed, EXAMPLE_MAX_CHARS)
  return scrubbed.length > 0 ? scrubbed : null
}

/** Normalization for dedupe/exact-grouping (never for user-facing routing). */
export function normalizeLearningText(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim()
}

// ---------------------------------------------------------------------------
// Deterministic candidate nomination (pure helpers)
// ---------------------------------------------------------------------------

const QUESTION_START =
  /^(?:who|what|when|where|why|how|which|can|could|do|does|did|is|are|will|would|should)\b/i

export function looksLikeQuestion(text: string): boolean {
  const trimmed = text.trim()
  if (trimmed.length < 8) return false
  return trimmed.includes("?") || QUESTION_START.test(trimmed)
}

const OBJECTION_CUES =
  /\b(?:too expensive|too pricey|can(?:'|’)?t afford|no budget|not sure|not convinced|worried|concern(?:ed|s)?|hesitant|skeptical|don(?:'|’)?t trust|already (?:use|have)|competitor|why should we|why would we|not interested|doesn(?:'|’)?t work for us|won(?:'|’)?t work|risky|too risky|compliance|security concerns)\b/i

export function looksLikeObjection(text: string): boolean {
  return OBJECTION_CUES.test(text)
}

const POSITIVE_ACK_CUES =
  /\b(?:thanks|thank you|that makes sense|makes sense|perfect|great|awesome|sounds good|very helpful|that helps|exactly what|love that)\b/i

function candidateId(kind: LearningCandidateKind, normalizedSummary: string) {
  const digest = createHash("sha256")
    .update(`${kind}|${normalizedSummary}`)
    .digest("hex")
    .slice(0, 16)
  return `lc_${digest}`
}

type NominatedTurn = {
  turn: LearningTurnRow
  redacted: string
  normalized: string
  embedding: number[] | null
}

type Group = {
  representative: NominatedTurn
  members: NominatedTurn[]
}

const GROUP_SIMILARITY_DEFAULT = 0.9

function groupSimilarityThreshold(): number {
  const raw = Number(process.env.SARAH_LEARNING_GROUP_SIMILARITY ?? NaN)
  return Number.isFinite(raw) && raw > 0 && raw <= 1
    ? raw
    : GROUP_SIMILARITY_DEFAULT
}

/**
 * Group nominated turns by meaning: cosine similarity over the KHS-6
 * embedding client when embeddings are available, exact normalized-text
 * equality as the deterministic fallback when the embedder is unarmed.
 */
export function groupNominatedTurns(
  items: NominatedTurn[],
  threshold: number,
): Group[] {
  const groups: Group[] = []
  for (const item of items) {
    let placed = false
    for (const group of groups) {
      const rep = group.representative
      const semanticMatch =
        item.embedding && rep.embedding
          ? cosineSimilarity(item.embedding, rep.embedding) >= threshold
          : item.normalized === rep.normalized
      if (semanticMatch) {
        group.members.push(item)
        placed = true
        break
      }
    }
    if (!placed) groups.push({ representative: item, members: [item] })
  }
  return groups
}

// ---------------------------------------------------------------------------
// Candidate/receipt store (Postgres when configured, ephemeral otherwise)
// ---------------------------------------------------------------------------

const memoryCandidates = new Map<string, LearningCandidate>()
const memoryReceipts = new Map<string, LearningReceipt>()
let learningSchemaReady: Promise<boolean> | null = null
let lastError: string | null = null

export function sarahLearningStoreMode(): "postgres" | "memory_ephemeral" {
  return sarahTurnStoreStatus().configured ? "postgres" : "memory_ephemeral"
}

async function ensureLearningSchema(): Promise<boolean> {
  learningSchemaReady ??= (async () => {
    const ok = await readSarahStore(async (sql) => {
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_learning_candidates (
          id TEXT PRIMARY KEY,
          kind TEXT NOT NULL,
          summary TEXT NOT NULL,
          redacted_examples JSONB NOT NULL DEFAULT '[]'::jsonb,
          source_turn_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
          question_canonical TEXT,
          proposed_answer TEXT,
          status TEXT NOT NULL DEFAULT 'pending',
          decided_by TEXT,
          decided_at TIMESTAMPTZ,
          receipt_id TEXT,
          created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )`
      // SQ-6 review ergonomics columns (additive; fail-soft if already present)
      await sql`ALTER TABLE sarah_learning_candidates ADD COLUMN IF NOT EXISTS taxonomy TEXT`
      await sql`ALTER TABLE sarah_learning_candidates ADD COLUMN IF NOT EXISTS why_generalize TEXT`
      await sql`ALTER TABLE sarah_learning_candidates ADD COLUMN IF NOT EXISTS example_count INTEGER`
      await sql`ALTER TABLE sarah_learning_candidates ADD COLUMN IF NOT EXISTS source_recency TIMESTAMPTZ`
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_learning_receipts (
          id TEXT PRIMARY KEY,
          candidate_id TEXT NOT NULL,
          decision TEXT NOT NULL,
          decided_by TEXT NOT NULL,
          reason TEXT,
          bank_entry_id TEXT,
          created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )`
      return true
    })
    return ok === true
  })()
  return learningSchemaReady
}

function rowToCandidate(row: Record<string, unknown>): LearningCandidate {
  const jsonArray = (value: unknown): string[] => {
    const parsed = typeof value === "string" ? JSON.parse(value) : value
    return Array.isArray(parsed) ? parsed.map(String) : []
  }
  const iso = (value: unknown): string | null =>
    value == null
      ? null
      : value instanceof Date
        ? value.toISOString()
        : String(value)
  const kind = String(row.kind) as LearningCandidateKind
  const redactedExamples = jsonArray(row.redacted_examples)
  const summary = String(row.summary)
  const taxonomy =
    row.taxonomy == null
      ? taxonomyForKind(kind)
      : (String(row.taxonomy) as LearningTaxonomy)
  const exampleCount =
    row.example_count == null
      ? redactedExamples.length
      : Number(row.example_count)
  return {
    id: String(row.id),
    kind,
    taxonomy,
    whyGeneralize:
      row.why_generalize == null
        ? buildWhyGeneralize({
            kind,
            prospectCount: Math.max(2, redactedExamples.length),
            summary,
          })
        : String(row.why_generalize),
    exampleCount: Number.isFinite(exampleCount)
      ? exampleCount
      : redactedExamples.length,
    sourceRecency:
      row.source_recency == null ? null : iso(row.source_recency),
    summary,
    redactedExamples,
    sourceTurnIds: jsonArray(row.source_turn_ids),
    questionCanonical:
      row.question_canonical == null ? null : String(row.question_canonical),
    proposedAnswer:
      row.proposed_answer == null ? null : String(row.proposed_answer),
    status: String(row.status) as LearningCandidateStatus,
    decidedBy: row.decided_by == null ? null : String(row.decided_by),
    decidedAt: iso(row.decided_at),
    receiptId: row.receipt_id == null ? null : String(row.receipt_id),
    createdAt: iso(row.created_at) ?? new Date().toISOString(),
  }
}

/** Insert if the id is unseen (any status — a rejection is remembered). */
async function insertCandidateIfNew(
  candidate: LearningCandidate,
): Promise<boolean> {
  if (sarahLearningStoreMode() === "memory_ephemeral") {
    if (memoryCandidates.has(candidate.id)) return false
    memoryCandidates.set(candidate.id, candidate)
    return true
  }
  if (!(await ensureLearningSchema())) return false
  const inserted = await readSarahStore(async (sql) => {
    const rows = (await sql`
      INSERT INTO sarah_learning_candidates
        (id, kind, taxonomy, why_generalize, example_count, source_recency,
         summary, redacted_examples, source_turn_ids,
         question_canonical, proposed_answer, status)
      VALUES
        (${candidate.id}, ${candidate.kind}, ${candidate.taxonomy},
         ${candidate.whyGeneralize}, ${candidate.exampleCount},
         ${candidate.sourceRecency},
         ${candidate.summary},
         ${candidate.redactedExamples}, ${candidate.sourceTurnIds},
         ${candidate.questionCanonical}, ${candidate.proposedAnswer},
         ${candidate.status})
      ON CONFLICT (id) DO NOTHING
      RETURNING id
    `) as Array<Record<string, unknown>>
    return rows.length > 0
  })
  return inserted === true
}

async function getCandidate(id: string): Promise<LearningCandidate | null> {
  if (sarahLearningStoreMode() === "memory_ephemeral") {
    return memoryCandidates.get(id) ?? null
  }
  if (!(await ensureLearningSchema())) return null
  const row = await readSarahStore(async (sql) => {
    const rows = (await sql`
      SELECT * FROM sarah_learning_candidates WHERE id = ${id} LIMIT 1
    `) as Array<Record<string, unknown>>
    return rows[0] ?? null
  })
  return row ? rowToCandidate(row) : null
}

export async function listLearningCandidates(
  status?: LearningCandidateStatus,
): Promise<LearningCandidate[]> {
  if (sarahLearningStoreMode() === "memory_ephemeral") {
    const all = [...memoryCandidates.values()]
    return (status ? all.filter((c) => c.status === status) : all).sort((a, b) =>
      a.createdAt.localeCompare(b.createdAt),
    )
  }
  if (!(await ensureLearningSchema())) return []
  const rows = await readSarahStore(async (sql) => {
    if (status) {
      return (await sql`
        SELECT * FROM sarah_learning_candidates
        WHERE status = ${status}
        ORDER BY created_at ASC
      `) as Array<Record<string, unknown>>
    }
    return (await sql`
      SELECT * FROM sarah_learning_candidates ORDER BY created_at ASC
    `) as Array<Record<string, unknown>>
  })
  return (rows ?? []).map(rowToCandidate)
}

async function writeDecision(
  candidate: LearningCandidate,
  receipt: LearningReceipt,
): Promise<boolean> {
  if (sarahLearningStoreMode() === "memory_ephemeral") {
    memoryCandidates.set(candidate.id, candidate)
    memoryReceipts.set(receipt.id, receipt)
    return true
  }
  if (!(await ensureLearningSchema())) return false
  const ok = await readSarahStore(async (sql) => {
    await sql`
      INSERT INTO sarah_learning_receipts
        (id, candidate_id, decision, decided_by, reason, bank_entry_id)
      VALUES
        (${receipt.id}, ${receipt.candidateId}, ${receipt.decision},
         ${receipt.decidedBy}, ${receipt.reason}, ${receipt.bankEntryId})`
    await sql`
      UPDATE sarah_learning_candidates
      SET status = ${candidate.status},
          decided_by = ${candidate.decidedBy},
          decided_at = now(),
          receipt_id = ${candidate.receiptId}
      WHERE id = ${candidate.id}`
    return true
  })
  return ok === true
}

export async function listLearningReceipts(): Promise<LearningReceipt[]> {
  if (sarahLearningStoreMode() === "memory_ephemeral") {
    return [...memoryReceipts.values()].sort((a, b) =>
      a.createdAt.localeCompare(b.createdAt),
    )
  }
  if (!(await ensureLearningSchema())) return []
  const rows = await readSarahStore(async (sql) => {
    return (await sql`
      SELECT * FROM sarah_learning_receipts ORDER BY created_at ASC
    `) as Array<Record<string, unknown>>
  })
  return (rows ?? []).map((row) => ({
    id: String(row.id),
    candidateId: String(row.candidate_id),
    decision: String(row.decision) as "approved" | "rejected",
    decidedBy: String(row.decided_by),
    reason: row.reason == null ? null : String(row.reason),
    bankEntryId: row.bank_entry_id == null ? null : String(row.bank_entry_id),
    createdAt:
      row.created_at instanceof Date
        ? row.created_at.toISOString()
        : String(row.created_at ?? new Date().toISOString()),
  }))
}

// ---------------------------------------------------------------------------
// THE shared read path (contract sarah.collective_learning_owner_gated.v1)
// ---------------------------------------------------------------------------

/**
 * The only collective-knowledge read Sarah's brains may use (besides the
 * KHS-6 answer bank, whose non-seed entries also arrive only through this
 * module's approval path). Returns exclusively status=approved candidates —
 * each carrying its approval receipt — never pending or rejected ones.
 */
export async function listApprovedLearnings(): Promise<LearningCandidate[]> {
  const approved = await listLearningCandidates("approved")
  // Defense in depth: even a corrupted store row cannot leak an undecided
  // candidate through the shared path without its receipt.
  return approved.filter(
    (candidate) => candidate.status === "approved" && candidate.receiptId,
  )
}

// ---------------------------------------------------------------------------
// Distillation
// ---------------------------------------------------------------------------

const DISTILL_SCAN_LIMIT = 500
const MAX_EXAMPLES_PER_CANDIDATE = 3

type TurnsSource = () => Promise<LearningTurnRow[] | null>

async function loadRecentTurnsAcrossProspects(): Promise<
  LearningTurnRow[] | null
> {
  const rows = await readSarahStore(async (sql) => {
    return (await sql`
      SELECT id, prospect_ref, role, text, recorded_at
      FROM sarah_transcript_turns
      ORDER BY recorded_at DESC, id DESC
      LIMIT ${DISTILL_SCAN_LIMIT}
    `) as Array<Record<string, unknown>>
  })
  if (!rows) return null
  return rows
    .map((row) => ({
      id: String(row.id ?? ""),
      prospectRef: String(row.prospect_ref ?? ""),
      role: String(row.role ?? ""),
      text: String(row.text ?? ""),
      recordedAt:
        row.recorded_at instanceof Date
          ? row.recorded_at.toISOString()
          : String(row.recorded_at ?? ""),
    }))
    .reverse() // chronological
}

let turnsSource: TurnsSource = loadRecentTurnsAcrossProspects

async function nominate(
  turns: Array<{ turn: LearningTurnRow }>,
): Promise<NominatedTurn[]> {
  const nominated: NominatedTurn[] = []
  const seenPerProspect = new Set<string>()
  for (const { turn } of turns) {
    const redacted = redactLearningExample(turn.text)
    if (!redacted) continue // PII law: when in doubt, drop
    const normalized = normalizeLearningText(redacted)
    if (!normalized) continue
    const dedupeKey = `${turn.prospectRef}|${normalized}`
    if (seenPerProspect.has(dedupeKey)) continue
    seenPerProspect.add(dedupeKey)
    // Embedding via the shared KHS-6 client; null (unarmed/failed) falls back
    // to exact normalized grouping.
    const embedding = await sarahEmbedText(redacted, "RETRIEVAL_DOCUMENT")
    nominated.push({ turn, redacted, normalized, embedding })
  }
  return nominated
}

function buildGroupCandidate(
  kind: LearningCandidateKind,
  group: Group,
): LearningCandidate | null {
  const prospects = new Set(group.members.map((m) => m.turn.prospectRef))
  if (prospects.size < 2) return null // collective = recurs across prospects
  const representative = group.representative
  const summary =
    kind === "question_gap"
      ? `Recurring prospect question (${prospects.size} prospects): ${representative.redacted}`
      : `Recurring objection (${prospects.size} prospects): ${representative.redacted}`
  const redactedExamples = group.members
    .slice(0, MAX_EXAMPLES_PER_CANDIDATE)
    .map((m) => m.redacted)
  return {
    id: candidateId(kind, representative.normalized),
    kind,
    taxonomy: taxonomyForKind(kind),
    whyGeneralize: buildWhyGeneralize({
      kind,
      prospectCount: prospects.size,
      summary,
    }),
    exampleCount: redactedExamples.length,
    sourceRecency: computeSourceRecency(
      group.members.map((m) => m.turn),
    ),
    summary: clip(summary, 300),
    redactedExamples,
    sourceTurnIds: group.members.map((m) => m.turn.id),
    questionCanonical:
      kind === "question_gap" ? representative.redacted : null,
    proposedAnswer: null,
    status: "pending",
    decidedBy: null,
    decidedAt: null,
    receiptId: null,
    createdAt: new Date().toISOString(),
  }
}

export type DistillResult = {
  ok: boolean
  reason?: string
  scannedTurns: number
  created: number
  createdIds: string[]
}

/**
 * Deterministic v1 distillation over recent turns across prospects:
 * - question_gap: user questions recurring across ≥2 prospects that the
 *   approved answer bank does not already cover (pricing-guard questions are
 *   skipped — the serve-time guard means a bank entry could never serve them)
 * - objection: objection-cue turns recurring across ≥2 prospects
 * - winning_answer: a user question → Sarah answer → positive-ack sequence
 *   inside one conversation (her own words, still redacted + owner-gated)
 * Every example passes redact-or-drop before it can enter a candidate.
 */
export async function distillLearningCandidates(): Promise<DistillResult> {
  const turns = await turnsSource()
  if (!turns) {
    return {
      ok: false,
      reason: "turn_store_unavailable",
      scannedTurns: 0,
      created: 0,
      createdIds: [],
    }
  }

  const bank = await listSarahAnswerBank()
  const bankNormalized = new Set(
    bank.map((entry) => normalizeLearningText(entry.questionCanonical)),
  )
  const threshold = groupSimilarityThreshold()
  const createdIds: string[] = []

  // --- question_gap ---
  const questionTurns = turns
    .filter(
      (turn) =>
        turn.role === "user" &&
        looksLikeQuestion(turn.text) &&
        !PRICING_GUARD_PATTERN.test(turn.text),
    )
    .map((turn) => ({ turn }))
  const questionGroups = groupNominatedTurns(
    await nominate(questionTurns),
    threshold,
  )
  for (const group of questionGroups) {
    if (bankNormalized.has(group.representative.normalized)) continue
    const candidate = buildGroupCandidate("question_gap", group)
    if (candidate && (await insertCandidateIfNew(candidate))) {
      createdIds.push(candidate.id)
    }
  }

  // --- objection ---
  const objectionTurns = turns
    .filter((turn) => turn.role === "user" && looksLikeObjection(turn.text))
    .map((turn) => ({ turn }))
  const objectionGroups = groupNominatedTurns(
    await nominate(objectionTurns),
    threshold,
  )
  for (const group of objectionGroups) {
    const candidate = buildGroupCandidate("objection", group)
    if (candidate && (await insertCandidateIfNew(candidate))) {
      createdIds.push(candidate.id)
    }
  }

  // --- winning_answer (per-prospect chronological triples) ---
  const byProspect = new Map<string, LearningTurnRow[]>()
  for (const turn of turns) {
    const list = byProspect.get(turn.prospectRef) ?? []
    list.push(turn)
    byProspect.set(turn.prospectRef, list)
  }
  for (const sequence of byProspect.values()) {
    for (let i = 0; i + 2 < sequence.length; i++) {
      const q = sequence[i]!
      const a = sequence[i + 1]!
      const ack = sequence[i + 2]!
      if (q.role !== "user" || !looksLikeQuestion(q.text)) continue
      if (PRICING_GUARD_PATTERN.test(q.text)) continue
      if (a.role !== "assistant" || a.text.trim().length < 40) continue
      if (ack.role !== "user" || !POSITIVE_ACK_CUES.test(ack.text)) continue
      const redactedQ = redactLearningExample(q.text)
      const redactedA = redactLearningExample(a.text)
      if (!redactedQ || !redactedA) continue
      const normalized = normalizeLearningText(redactedQ)
      if (bankNormalized.has(normalized)) continue
      const summary = clip(`Winning answer to: ${redactedQ}`, 300)
      const candidate: LearningCandidate = {
        id: candidateId("winning_answer", normalized),
        kind: "winning_answer",
        taxonomy: taxonomyForKind("winning_answer"),
        whyGeneralize: buildWhyGeneralize({
          kind: "winning_answer",
          prospectCount: 1,
          summary,
        }),
        exampleCount: 2,
        sourceRecency: computeSourceRecency([q, a, ack]),
        summary,
        redactedExamples: [redactedQ, redactedA],
        sourceTurnIds: [q.id, a.id, ack.id],
        questionCanonical: redactedQ,
        proposedAnswer: redactedA,
        status: "pending",
        decidedBy: null,
        decidedAt: null,
        receiptId: null,
        createdAt: new Date().toISOString(),
      }
      if (await insertCandidateIfNew(candidate)) createdIds.push(candidate.id)
    }
  }

  return {
    ok: true,
    scannedTurns: turns.length,
    created: createdIds.length,
    createdIds,
  }
}

// ---------------------------------------------------------------------------
// Owner decisions (always receipt-writing)
// ---------------------------------------------------------------------------

export type ApproveResult =
  | {
      ok: true
      candidate: LearningCandidate
      receipt: LearningReceipt
      bankEntryId: string | null
      /** SQ-6: set when the approval materially changes answer style. */
      regressionFixture: LearningRegressionFixture | null
    }
  | { ok: false; error: string }

/** In-memory regression fixtures for tests / ops (not a serve path). */
const memoryRegressionFixtures: LearningRegressionFixture[] = []

export function listLearningRegressionFixtures(): LearningRegressionFixture[] {
  return [...memoryRegressionFixtures]
}

export function __resetLearningRegressionFixturesForTest(): void {
  memoryRegressionFixtures.length = 0
}

function newReceiptId(): string {
  return `lr_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
}

/**
 * Owner approval: writes the approval receipt row, flips the candidate to
 * approved, and — when the candidate carries a question + answer
 * (winning_answer, or question_gap with the owner-supplied `answerText`) —
 * publishes a `sarah_answer_bank` entry whose `approved_by` is the receipt
 * ref, so a live cache answer dereferences back to this receipt and the
 * candidate's redacted source turns.
 */
export async function approveLearningCandidate(input: {
  id: string
  by: string
  answerText?: string
}): Promise<ApproveResult> {
  const by = input.by.trim()
  if (!by) return { ok: false, error: "decided_by_required" }
  const candidate = await getCandidate(input.id)
  if (!candidate) return { ok: false, error: "candidate_not_found" }
  if (candidate.status !== "pending") {
    return { ok: false, error: "already_decided" }
  }

  const receiptId = newReceiptId()
  const answer = input.answerText?.trim() || candidate.proposedAnswer
  const publishToBank = Boolean(candidate.questionCanonical && answer)
  const bankEntryId = publishToBank ? `learned_${candidate.id}` : null

  const decided: LearningCandidate = {
    ...candidate,
    status: "approved",
    decidedBy: by,
    decidedAt: new Date().toISOString(),
    receiptId,
  }
  const receipt: LearningReceipt = {
    id: receiptId,
    candidateId: candidate.id,
    decision: "approved",
    decidedBy: by,
    reason: null,
    bankEntryId,
    createdAt: decided.decidedAt!,
  }
  if (!(await writeDecision(decided, receipt))) {
    return { ok: false, error: "store_write_failed" }
  }

  let regressionFixture: LearningRegressionFixture | null = null
  if (publishToBank && bankEntryId) {
    const bank = await listSarahAnswerBank()
    const prior = bank.find(
      (e) =>
        normalizeLearningText(e.questionCanonical) ===
        normalizeLearningText(candidate.questionCanonical!),
    )
    const previousAnswer = prior?.answer ?? null
    if (isMaterialStyleChange(previousAnswer, answer!)) {
      regressionFixture = {
        schema: "sarah.learning_style_regression.v1",
        createdAt: decided.decidedAt!,
        candidateId: candidate.id,
        receiptId,
        questionCanonical: candidate.questionCanonical!,
        previousAnswer,
        newAnswer: answer!,
        reason:
          "Approved learning materially changes answer style for this question; keep as regression fixture.",
      }
      memoryRegressionFixtures.push(regressionFixture)
    }
    await addApprovedSarahAnswer({
      id: bankEntryId,
      questionCanonical: candidate.questionCanonical!,
      answer: answer!,
      approvedBy: learningReceiptRef(receiptId),
    })
  }

  return {
    ok: true,
    candidate: decided,
    receipt,
    bankEntryId,
    regressionFixture,
  }
}

export async function rejectLearningCandidate(input: {
  id: string
  by: string
  reason?: string
}): Promise<ApproveResult> {
  const by = input.by.trim()
  if (!by) return { ok: false, error: "decided_by_required" }
  const candidate = await getCandidate(input.id)
  if (!candidate) return { ok: false, error: "candidate_not_found" }
  if (candidate.status !== "pending") {
    return { ok: false, error: "already_decided" }
  }
  const receiptId = newReceiptId()
  const decided: LearningCandidate = {
    ...candidate,
    status: "rejected",
    decidedBy: by,
    decidedAt: new Date().toISOString(),
    receiptId,
  }
  const receipt: LearningReceipt = {
    id: receiptId,
    candidateId: candidate.id,
    decision: "rejected",
    decidedBy: by,
    reason: input.reason?.trim() || null,
    bankEntryId: null,
    createdAt: decided.decidedAt!,
  }
  if (!(await writeDecision(decided, receipt))) {
    return { ok: false, error: "store_write_failed" }
  }
  return {
    ok: true,
    candidate: decided,
    receipt,
    bankEntryId: null,
    regressionFixture: null,
  }
}

// ---------------------------------------------------------------------------
// Ops + test hooks
// ---------------------------------------------------------------------------

export function sarahCollectiveLearningStatus() {
  return {
    storeMode: sarahLearningStoreMode(),
    gate: "owner_approval_receipt_required",
    publicClaim: "none_internal_owner_approved_store_only",
    lastError,
  }
}

export function __setSarahLearningTurnsForTest(
  rows: LearningTurnRow[] | null,
): void {
  turnsSource = rows
    ? () => Promise.resolve(rows)
    : loadRecentTurnsAcrossProspects
}

export function __resetSarahCollectiveLearningForTest(): void {
  memoryCandidates.clear()
  memoryReceipts.clear()
  learningSchemaReady = null
  lastError = null
  turnsSource = loadRecentTurnsAcrossProspects
}
