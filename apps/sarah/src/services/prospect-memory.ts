/**
 * Prospect memory v1 — Sarah remembers the user she's talking to (KHS-2,
 * #8601, epic #8599).
 *
 * Reads the durable record (`sarah_transcript_turns` +
 * `sarah_prospect_contacts`, see turn-store.ts) for ONE prospect and builds a
 * compact memory block that the two brains prepend to their system
 * instructions: distilled facts with per-fact source turn ids (provenance)
 * plus a short verbatim recap of the most recent turns. Honest v1 is
 * deterministic distillation — bounded pattern-matched quotes from what the
 * prospect actually said, no extra LLM calls and no fake inference.
 *
 * Derivation is read-time (zero write hooks at the turn-record sites, so the
 * memory can never drift from the transcript). The distilled facts are also
 * upserted into `sarah_prospect_profile` (sarah_-prefixed, auto-ensured,
 * fail-soft) as a typed `memory_updated` receipt/projection — the transcript
 * stays the source of truth.
 *
 * SCOPING LAW (KHS-3 oracles this seam): every read in this module is
 * filtered to the exact prospect ref passed in — plus its deterministic
 * re-encodings of the SAME identity (`<ref>` on the text lane vs
 * `prospect:<ref>` on the avatar lane; see prospectRefAliases). There is no
 * code path here that queries across prospects, lists refs, or matches by
 * pattern. `visitor:` refs never alias to a cookie ref.
 *
 * Memory NEVER weakens the pricing guard: both call sites fetch memory only
 * on the model path, after the deterministic guard has already run.
 */

import { readSarahStore } from "./turn-store.ts"

export type SarahMemoryTurnRow = {
  /** sarah_transcript_turns.id — provenance for distilled facts. */
  id: string
  role: string
  modality: string
  text: string
  /** ISO timestamp (recorded_at). */
  recordedAt: string
}

export type SarahMemoryContactRow = {
  contactEmail: string | null
  contactId: string | null
}

export type SarahProspectFact = {
  fact: string
  sourceTurnId: string
  at: string
}

/** Max characters for the whole memory block — prompts stay lean. */
export const PROSPECT_MEMORY_MAX_CHARS = 1200
export const CROSS_PROSPECT_MEMORY_REFUSAL_REPLY =
  "I can't share another prospect or customer's private conversation, memory, or profile. I can only use your own context and approved public OpenAgents information."

const RECENT_TURNS_QUERY_LIMIT = 40
const RECAP_TURNS = 6
const MAX_FACTS = 6
const FACT_QUOTE_MAX = 140
const RECAP_QUOTE_MAX = 110
const CROSS_SCOPE_EMAIL_PATTERN =
  /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi
const CROSS_SCOPE_PHONE_PATTERN =
  /(?<![A-Z0-9])(?:\+?\d[\d\s().-]{7,}\d)(?![A-Z0-9])/gi
const CROSS_PROSPECT_MEMORY_PROBE_PATTERN =
  /\b(?:what|tell|show|share|summarize|quote|repeat|reveal)\b[\s\S]{0,90}\b(?:last|previous|other|another|different)\s+(?:customer|prospect|user|client|visitor|person|company)\b[\s\S]{0,90}\b(?:said|say|asked|told|shared|conversation|memory|profile|data|objection|need|pain)\b/i

/**
 * KHS-3 deterministic injection probe guard. This catches requests to reveal
 * another prospect/customer's private memory before either brain can call a
 * model. Public aggregate questions should go through normal public-claim
 * grounding; private cross-prospect recall never does.
 */
export function isCrossProspectMemoryProbe(text: string): boolean {
  return CROSS_PROSPECT_MEMORY_PROBE_PATTERN.test(text.replace(/\s+/g, " "))
}

/**
 * KHS-3 redaction gate for any future fact promotion beyond one prospect's
 * private scope (for example KHS-4 owner-approved collective learning). The
 * prospect-memory path itself remains prospect-scoped; this pure function is
 * the oracleable boundary for data that wants to leave that scope.
 */
export function redactProspectFactForCrossScope(text: string): string {
  return text
    .replace(CROSS_SCOPE_EMAIL_PATTERN, "[redacted-email]")
    .replace(CROSS_SCOPE_PHONE_PATTERN, "[redacted-phone]")
}

/**
 * Deterministic encodings of ONE prospect identity. The text lane stores the
 * raw cookie ref; the avatar lane stores the `prospect:<ref>` conversation
 * ref (llm-openai-compat.ts). Anonymous `visitor:` refs are their own
 * identity and never alias to a cookie ref. Pure — KHS-3 can oracle it.
 */
export function prospectRefAliases(prospectRef: string): string[] {
  const ref = prospectRef.trim()
  if (!ref) return []
  if (ref.startsWith("visitor:")) return [ref]
  const canonical = ref.startsWith("prospect:")
    ? ref.slice("prospect:".length)
    : ref
  if (!canonical) return [ref]
  return [canonical, `prospect:${canonical}`]
}

const FACT_CUES: Array<{ label: string; pattern: RegExp }> = [
  {
    label: "company",
    pattern:
      /\b(?:i work (?:at|for)|my company|our company|our startup|our team|we(?:'|’)re a|we are a|company is called|i(?:'|’)m (?:at|with|from))\b/i,
  },
  {
    label: "role",
    pattern:
      /\b(?:i(?:'|’)m (?:the|a|an)|i am (?:the|a|an)|my role|my job|as (?:the|a|an) (?:cto|ceo|coo|cfo|founder|co-founder|engineer|developer|designer|pm|product manager|manager|director|vp|head))\b|\b(?:cto|ceo|coo|cfo|founder|co-founder)\b/i,
  },
  {
    label: "need",
    pattern:
      /\b(?:we need|i need|we want|i want|we(?:'|’)re looking for|looking for|trying to|we(?:'|’)re trying|interested in|our (?:problem|pain|goal)|use case|evaluating)\b/i,
  },
  {
    label: "stack",
    pattern:
      /\b(?:we use|we(?:'|’)re using|i use|our stack|built (?:with|on)|running on|we(?:'|’)re on|migrating from|typescript|python|rust|react|postgres|kubernetes|aws|gcp|azure)\b/i,
  },
  {
    label: "contact",
    pattern: /\b(?:my email|reach me|email me|my name is|call me)\b/i,
  },
]

function clip(text: string, max: number): string {
  const normalized = text.replace(/\s+/g, " ").trim()
  if (normalized.length <= max) return normalized
  return `${normalized.slice(0, max - 1).trimEnd()}…`
}

/**
 * Deterministic v1 distillation: short verbatim quotes from the prospect's
 * own (user-role) turns that match stated company/role/need/stack/contact
 * cues, each carrying its source turn id. No inference beyond the match.
 * Pure — unit-testable without a database.
 */
export function distillProspectFacts(
  rows: SarahMemoryTurnRow[],
): SarahProspectFact[] {
  const facts: SarahProspectFact[] = []
  const seen = new Set<string>()
  for (const row of rows) {
    if (row.role !== "user") continue
    const text = row.text.trim()
    if (!text) continue
    for (const cue of FACT_CUES) {
      if (!cue.pattern.test(text)) continue
      const fact = `${cue.label}: "${clip(text, FACT_QUOTE_MAX)}"`
      const dedupeKey = fact.toLowerCase()
      if (seen.has(dedupeKey)) break
      seen.add(dedupeKey)
      facts.push({ fact, sourceTurnId: row.id, at: row.recordedAt })
      break // one fact per turn — the quote already carries full context
    }
  }
  return facts.slice(-MAX_FACTS)
}

/**
 * Pure formatter for the memory block. Returns null when there is nothing
 * worth remembering. Output is capped at `maxChars` (default 1200).
 */
export function formatMemoryContext(input: {
  prospectRef: string
  contact?: SarahMemoryContactRow | null
  facts: SarahProspectFact[]
  recentTurns: SarahMemoryTurnRow[]
  maxChars?: number
}): string | null {
  const { contact, facts } = input
  const recap = input.recentTurns.slice(-RECAP_TURNS)
  if (!contact?.contactEmail && facts.length === 0 && recap.length === 0) {
    return null
  }

  const lines: string[] = [
    "[prospect memory — returning visitor; recall naturally, verify when unsure]",
  ]
  if (contact?.contactEmail) {
    lines.push(`Known contact email: ${contact.contactEmail}`)
  }
  if (facts.length > 0) {
    lines.push("Facts they stated previously (verbatim, with source turn id):")
    for (const fact of facts) {
      lines.push(`- ${fact.fact} (turn ${fact.sourceTurnId})`)
    }
  }
  if (recap.length > 0) {
    lines.push("Most recent conversation turns:")
    for (const turn of recap) {
      const speaker = turn.role === "assistant" ? "sarah" : "user"
      lines.push(`- ${speaker}: "${clip(turn.text, RECAP_QUOTE_MAX)}"`)
    }
  }
  lines.push(
    "Do not read turn ids aloud. If memory conflicts with what they say now, trust them and update.",
  )

  const maxChars = input.maxChars ?? PROSPECT_MEMORY_MAX_CHARS
  let block = lines.join("\n")
  while (block.length > maxChars && lines.length > 2) {
    // Drop recap lines first (oldest first), then facts — the header and the
    // closing instruction stay.
    const recapStart = lines.findIndex((line) =>
      line.startsWith("Most recent conversation turns:"),
    )
    if (recapStart >= 0 && recapStart + 1 < lines.length - 1) {
      lines.splice(recapStart + 1, 1)
      if (lines[recapStart + 1]?.startsWith("Do not read")) {
        lines.splice(recapStart, 1) // recap section emptied — drop its header
      }
    } else {
      let lastFact = -1
      for (let i = lines.length - 1; i >= 0; i--) {
        if (lines[i]!.startsWith("- ")) {
          lastFact = i
          break
        }
      }
      if (lastFact <= 0) break
      lines.splice(lastFact, 1)
    }
    block = lines.join("\n")
  }
  return block.length <= maxChars ? block : clip(block, maxChars)
}

type ProfileFactJson = { fact: string; source_turn_id: string; at: string }

let profileSchemaReady: Promise<boolean> | null = null

async function ensureProfileSchema(): Promise<boolean> {
  profileSchemaReady ??= (async () => {
    const ok = await readSarahStore(async (sql) => {
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_prospect_profile (
          prospect_ref TEXT PRIMARY KEY,
          facts JSONB NOT NULL DEFAULT '[]'::jsonb,
          updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )`
      return true
    })
    return ok === true
  })()
  return profileSchemaReady
}

/**
 * Typed `memory_updated` receipt: persist the distilled facts for this
 * prospect ref. Projection only (source of truth stays the transcript);
 * fail-soft — a failed upsert never blocks the conversation.
 */
async function upsertProspectProfile(
  prospectRef: string,
  facts: SarahProspectFact[],
): Promise<void> {
  if (facts.length === 0) return
  if (!(await ensureProfileSchema())) return
  const payload: ProfileFactJson[] = facts.map((fact) => ({
    fact: fact.fact,
    source_turn_id: fact.sourceTurnId,
    at: fact.at,
  }))
  await readSarahStore(
    // Bun SQL: bind the JS array directly — a stringified param lands as a
    // jsonb *string*, not an array (verified against Postgres 16).
    (sql) => sql`
      INSERT INTO sarah_prospect_profile (prospect_ref, facts, updated_at)
      VALUES (${prospectRef}, ${payload}, now())
      ON CONFLICT (prospect_ref) DO UPDATE SET
        facts = EXCLUDED.facts,
        updated_at = now()
    `,
  )
}

function asMemoryTurnRow(row: Record<string, unknown>): SarahMemoryTurnRow {
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
 * Build the memory block for ONE prospect ref, or null when there is no
 * durable store, no prior record, or nothing worth recalling. This is the
 * only entry point the brains call; every SQL read inside is bound to
 * `prospectRefAliases(prospectRef)` — the exact identity, nothing else.
 */
export async function getProspectMemoryContext(
  prospectRef: string,
): Promise<string | null> {
  const aliases = prospectRefAliases(prospectRef)
  if (aliases.length === 0) return null

  const rows = await readSarahStore(async (sql) => {
    // Bun SQL list binding: `IN ${sql(list)}` (a JS array does not serialize
    // into `= ANY(...)`). The list is always exactly prospectRefAliases(ref).
    const turns = (await sql`
      SELECT id, role, modality, text, recorded_at
      FROM sarah_transcript_turns
      WHERE prospect_ref IN ${sql(aliases)}
      ORDER BY recorded_at DESC, id DESC
      LIMIT ${RECENT_TURNS_QUERY_LIMIT}
    `) as Array<Record<string, unknown>>
    const contacts = (await sql`
      SELECT contact_email, contact_id
      FROM sarah_prospect_contacts
      WHERE prospect_ref IN ${sql(aliases)}
      LIMIT ${aliases.length}
    `) as Array<Record<string, unknown>>
    return { turns, contacts }
  })
  if (!rows || rows.turns.length === 0) return null

  const turns = rows.turns.map(asMemoryTurnRow).reverse() // chronological
  const contactRow = rows.contacts.find((row) => row.contact_email)
  const contact: SarahMemoryContactRow | null = contactRow
    ? {
        contactEmail: String(contactRow.contact_email ?? "") || null,
        contactId:
          contactRow.contact_id == null ? null : String(contactRow.contact_id),
      }
    : null

  const facts = distillProspectFacts(turns)
  await upsertProspectProfile(aliases[0]!, facts)

  return formatMemoryContext({ prospectRef, contact, facts, recentTurns: turns })
}
