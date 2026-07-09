/**
 * KHS-6 semantic answer cache (#8605, epic #8599): high-frequency prospect
 * questions answered from an owner-approved answer bank WITHOUT an LLM call.
 *
 * Law (workspace semantic-routing invariant): matching is embedding +
 * cosine-similarity ONLY — never keyword/regex/string intent routing. The one
 * regex that runs here is the deterministic pricing guard, and it runs FIRST
 * to BLOCK the cache, never to select an answer: a pricing/discount question
 * must always reach the guard refusal path, so the cache refuses to match it.
 *
 * Flag-gated: `maybeSemanticCacheAnswer` is a no-op unless
 * `SARAH_SEMANTIC_CACHE=1` — the contested reply paths (llm-openai-compat,
 * owned-runtime) carry exactly one hook call each and prod behavior is
 * unchanged until the owner arms the flag.
 *
 * Bank: versioned rows in Postgres `sarah_answer_bank` (same fail-soft
 * schema-ensure posture as turn-store; Sarah owns only `sarah_*` tables),
 * seeded from `config/answer-bank-seed.json` — verbatim owner-approved copy
 * out of docs/sarah/SARAH_KNOWLEDGE_BASE.md (`approved_by: owner_kb_v2`).
 * Future entries arrive through the KHS-4 owner-approval queue, never ad hoc.
 * Without a database the seed bank serves from memory; persistence being down
 * never breaks a conversation.
 *
 * Embeddings: Gemini `:embedContent` (`gemini-embedding-001` by default) on
 * the same GEMINI_API_KEY lane as google-inference. Bank embeddings compute
 * lazily on first match and are cached in-process (and written back to the
 * bank row fail-soft). Any embed failure is a silent miss — the model path
 * always remains available.
 */

import { SQL } from "bun"
import { readFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

import { PRICING_GUARD_PATTERN } from "../llm-openai-compat.ts"

const appRoot = fileURLToPath(new URL("../..", import.meta.url))
const SEED_PATH = path.join(appRoot, "config/answer-bank-seed.json")

const EMBED_MODEL_DEFAULT = "gemini-embedding-001"
const EMBED_BASE_URL_DEFAULT = "https://generativelanguage.googleapis.com/v1beta"
const MIN_SIMILARITY_DEFAULT = 0.86

export type AnswerBankEntry = {
  id: string
  questionCanonical: string
  answer: string
  /** Per-entry override; falls back to the env/default threshold. */
  minSimilarity: number | null
  approvedBy: string
  /** Lazily computed + cached; null until first match attempt. */
  embedding: number[] | null
}

export type AnswerCacheHit = {
  id: string
  answer: string
  similarity: number
}

// ---------------------------------------------------------------------------
// Pure math (unit-tested without any network)
// ---------------------------------------------------------------------------

export function cosineSimilarity(
  a: ReadonlyArray<number>,
  b: ReadonlyArray<number>,
): number {
  if (a.length === 0 || a.length !== b.length) return 0
  let dot = 0
  let normA = 0
  let normB = 0
  for (let i = 0; i < a.length; i++) {
    dot += a[i]! * b[i]!
    normA += a[i]! * a[i]!
    normB += b[i]! * b[i]!
  }
  if (normA === 0 || normB === 0) return 0
  return dot / (Math.sqrt(normA) * Math.sqrt(normB))
}

export function defaultMinSimilarity(): number {
  const raw = Number(process.env.SARAH_SEMANTIC_CACHE_MIN_SIMILARITY ?? NaN)
  return Number.isFinite(raw) && raw > 0 && raw <= 1 ? raw : MIN_SIMILARITY_DEFAULT
}

/** Pure best-match over an embedded bank; entries without embeddings skip. */
export function matchAgainstBank(
  queryEmbedding: ReadonlyArray<number>,
  bank: ReadonlyArray<AnswerBankEntry>,
  fallbackThreshold: number,
): AnswerCacheHit | null {
  let best: AnswerCacheHit | null = null
  for (const entry of bank) {
    if (!entry.embedding) continue
    const similarity = cosineSimilarity(queryEmbedding, entry.embedding)
    const threshold = entry.minSimilarity ?? fallbackThreshold
    if (similarity < threshold) continue
    if (!best || similarity > best.similarity) {
      best = { id: entry.id, answer: entry.answer, similarity }
    }
  }
  return best
}

// ---------------------------------------------------------------------------
// Embedding client (GEMINI_API_KEY lane; fail-soft)
// ---------------------------------------------------------------------------

type EmbedTaskType = "RETRIEVAL_QUERY" | "RETRIEVAL_DOCUMENT"
type Embedder = (text: string, taskType: EmbedTaskType) => Promise<number[] | null>

async function geminiEmbed(
  text: string,
  taskType: EmbedTaskType,
): Promise<number[] | null> {
  const apiKey = process.env.GEMINI_API_KEY?.trim()
  if (!apiKey) {
    lastError = "embed_not_armed"
    return null
  }
  const model = process.env.SARAH_EMBED_MODEL?.trim() || EMBED_MODEL_DEFAULT
  const baseUrl = (
    process.env.SARAH_GOOGLE_INFERENCE_BASE_URL?.trim() || EMBED_BASE_URL_DEFAULT
  ).replace(/\/+$/, "")
  const timeoutMs = Number(process.env.SARAH_EMBED_TIMEOUT_MS ?? 10_000)
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetch(
      `${baseUrl}/models/${model}:embedContent?key=${apiKey}`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        signal: controller.signal,
        body: JSON.stringify({
          content: { parts: [{ text }] },
          taskType,
        }),
      },
    )
    if (!response.ok) {
      // Never surface the URL — it carries the key.
      lastError = `embed_http_${response.status}`
      return null
    }
    const data = (await response.json()) as {
      embedding?: { values?: number[] }
    }
    const values = data.embedding?.values
    if (!Array.isArray(values) || values.length === 0) {
      lastError = "embed_empty"
      return null
    }
    return values
  } catch (error) {
    lastError =
      error instanceof Error && error.name === "AbortError"
        ? "embed_timeout"
        : "embed_unreachable"
    return null
  } finally {
    clearTimeout(timeout)
  }
}

let embedder: Embedder = geminiEmbed

// ---------------------------------------------------------------------------
// Postgres bank (fail-soft, same posture as turn-store; sarah_* tables only)
// ---------------------------------------------------------------------------

let sqlClient: SQL | null | undefined
let schemaReady: Promise<boolean> | null = null
let lastError: string | null = null

function databaseUrl(): string | null {
  return (
    process.env.SARAH_DATABASE_URL?.trim() ||
    process.env.KHALA_SYNC_DATABASE_URL?.trim() ||
    null
  )
}

function client(): SQL | null {
  if (sqlClient !== undefined) return sqlClient
  const url = databaseUrl()
  if (!url) {
    sqlClient = null
    return null
  }
  try {
    // Cloud SQL Auth Connector shape (host-less DSN + unix-socket PGHOST) —
    // same handling as turn-store.ts.
    const parsed = new URL(url)
    const pgHost = process.env.PGHOST?.trim()
    if (!parsed.hostname && pgHost) {
      const database = parsed.pathname.slice(1)
      const username = parsed.username || process.env.PGUSER?.trim() || "postgres"
      const password = parsed.password || process.env.PGPASSWORD || ""
      const options = pgHost.startsWith("/")
        ? { path: pgHost + "/.s.PGSQL.5432", database, username, password, max: 2 }
        : { hostname: pgHost, database, username, password, max: 2 }
      sqlClient = new SQL(options as unknown as string)
    } else {
      sqlClient = new SQL(url)
    }
  } catch (error) {
    lastError = error instanceof Error ? error.message : String(error)
    sqlClient = null
  }
  return sqlClient
}

async function ensureSchema(sql: SQL): Promise<boolean> {
  schemaReady ??= (async () => {
    try {
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_answer_bank (
          id TEXT PRIMARY KEY,
          version INTEGER NOT NULL DEFAULT 1,
          question_canonical TEXT NOT NULL,
          answer TEXT NOT NULL,
          embedding JSONB,
          min_similarity DOUBLE PRECISION,
          approved_by TEXT NOT NULL,
          created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
          updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )`
      return true
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error)
      return false
    }
  })()
  return schemaReady
}

type SeedFile = {
  entries?: Array<{
    id?: string
    question_canonical?: string
    answer?: string
    min_similarity?: number
    approved_by?: string
  }>
}

async function loadSeedEntries(): Promise<AnswerBankEntry[]> {
  try {
    const raw = JSON.parse(await readFile(SEED_PATH, "utf8")) as SeedFile
    const entries: AnswerBankEntry[] = []
    for (const seed of raw.entries ?? []) {
      if (!seed.id || !seed.question_canonical || !seed.answer) continue
      entries.push({
        id: seed.id,
        questionCanonical: seed.question_canonical,
        answer: seed.answer,
        minSimilarity:
          typeof seed.min_similarity === "number" ? seed.min_similarity : null,
        approvedBy: seed.approved_by ?? "owner_kb_v2",
        embedding: null,
      })
    }
    return entries
  } catch (error) {
    lastError = error instanceof Error ? error.message : String(error)
    return []
  }
}

let bankPromise: Promise<AnswerBankEntry[]> | null = null

/**
 * Bank = seed file upserted into `sarah_answer_bank` (ON CONFLICT DO NOTHING —
 * owner-edited/KHS-4-approved rows in the database win over seeds), then all
 * rows read back. Without a database: the in-memory seed bank.
 */
async function loadBank(): Promise<AnswerBankEntry[]> {
  bankPromise ??= (async () => {
    const seeds = await loadSeedEntries()
    const sql = client()
    if (!sql || !(await ensureSchema(sql))) return seeds
    try {
      for (const seed of seeds) {
        await sql`
          INSERT INTO sarah_answer_bank
            (id, question_canonical, answer, min_similarity, approved_by)
          VALUES
            (${seed.id}, ${seed.questionCanonical}, ${seed.answer},
             ${seed.minSimilarity}, ${seed.approvedBy})
          ON CONFLICT (id) DO NOTHING`
      }
      const rows = (await sql`
        SELECT id, question_canonical, answer, embedding, min_similarity, approved_by
        FROM sarah_answer_bank
        ORDER BY id`) as Array<{
        id: string
        question_canonical: string
        answer: string
        embedding: number[] | string | null
        min_similarity: number | null
        approved_by: string
      }>
      return rows.map((row) => {
        const embedding =
          typeof row.embedding === "string"
            ? (JSON.parse(row.embedding) as number[])
            : row.embedding
        return {
          id: row.id,
          questionCanonical: row.question_canonical,
          answer: row.answer,
          minSimilarity: row.min_similarity,
          approvedBy: row.approved_by,
          embedding: Array.isArray(embedding) && embedding.length > 0 ? embedding : null,
        }
      })
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error)
      return seeds
    }
  })()
  return bankPromise
}

async function persistEmbedding(id: string, embedding: number[]): Promise<void> {
  const sql = client()
  if (!sql || !(await ensureSchema(sql))) return
  try {
    await sql`
      UPDATE sarah_answer_bank
      SET embedding = ${JSON.stringify(embedding)}::jsonb, updated_at = now()
      WHERE id = ${id}`
  } catch (error) {
    lastError = error instanceof Error ? error.message : String(error)
  }
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

const metrics = {
  hits: 0,
  misses: 0,
  guardBlocked: 0,
  tokensSavedEstimate: 0,
}

function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4)
}

export function sarahSemanticCacheEnabled(): boolean {
  return process.env.SARAH_SEMANTIC_CACHE?.trim() === "1"
}

export function sarahAnswerCacheStatus() {
  return {
    enabled: sarahSemanticCacheEnabled(),
    minSimilarity: defaultMinSimilarity(),
    embedModel: process.env.SARAH_EMBED_MODEL?.trim() || EMBED_MODEL_DEFAULT,
    databaseConfigured: Boolean(databaseUrl()),
    hits: metrics.hits,
    misses: metrics.misses,
    guardBlocked: metrics.guardBlocked,
    tokensSavedEstimate: metrics.tokensSavedEstimate,
    lastError,
  }
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

/**
 * Embedding/cosine match against the approved answer bank. The pricing guard
 * runs FIRST and unconditionally: a query the guard would refuse can never be
 * answered from the cache (the caller's own guard branch handles the refusal).
 */
export async function matchAnswer(query: string): Promise<AnswerCacheHit | null> {
  const trimmed = query.trim()
  if (!trimmed) return null
  if (PRICING_GUARD_PATTERN.test(trimmed)) {
    metrics.guardBlocked += 1
    return null
  }
  const bank = await loadBank()
  if (bank.length === 0) return null

  const queryEmbedding = await embedder(trimmed, "RETRIEVAL_QUERY")
  if (!queryEmbedding) {
    metrics.misses += 1
    return null
  }

  // Bank embeddings compute lazily on first use and cache in-process.
  for (const entry of bank) {
    if (entry.embedding) continue
    const embedded = await embedder(entry.questionCanonical, "RETRIEVAL_DOCUMENT")
    if (!embedded) continue
    entry.embedding = embedded
    await persistEmbedding(entry.id, embedded)
  }

  const hit = matchAgainstBank(queryEmbedding, bank, defaultMinSimilarity())
  if (hit) {
    metrics.hits += 1
    metrics.tokensSavedEstimate += estimateTokens(hit.answer)
    return hit
  }
  metrics.misses += 1
  return null
}

/**
 * The single hook the reply paths call. Flag-off (`SARAH_SEMANTIC_CACHE`
 * unset) it returns null without touching the bank, the database, or the
 * embedder — zero behavior change until the owner arms it.
 */
export async function maybeSemanticCacheAnswer(
  query: string,
): Promise<AnswerCacheHit | null> {
  if (!sarahSemanticCacheEnabled()) return null
  try {
    return await matchAnswer(query)
  } catch (error) {
    // Fail-soft everywhere: a broken cache never blocks the model path.
    lastError = error instanceof Error ? error.message : String(error)
    return null
  }
}

// ---------------------------------------------------------------------------
// Test hooks (no network, no database in unit tests)
// ---------------------------------------------------------------------------

export function __setSarahEmbedderForTest(fn: Embedder | null): void {
  embedder = fn ?? geminiEmbed
}

export function __setSarahAnswerBankForTest(
  entries: AnswerBankEntry[] | null,
): void {
  bankPromise = entries ? Promise.resolve(entries) : null
}

export function __resetSarahAnswerCacheForTest(): void {
  embedder = geminiEmbed
  bankPromise = null
  metrics.hits = 0
  metrics.misses = 0
  metrics.guardBlocked = 0
  metrics.tokensSavedEstimate = 0
  lastError = null
}
