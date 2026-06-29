import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import type { ReadFile, WriteFile } from "./agent-onboarding.js"

// AF-5 (#5902): forum-loop safety / bounds.
//
// The automated forum loop (intro post + tip-recipient claim + work-search) must
// be bounded so a flapping/offline node can never hammer the Forum with retries.
// This module centralizes:
//   - the daily / per-tick WRITE caps, modeled on the server-side Artanis
//     responder (ARTANIS_RESPONDER_MAX_PER_DAY = 20, ..._MAX_PER_TICK = 3),
//   - a persisted per-UTC-day attempt ledger that gates forum write attempts,
//   - a typed classifier for write responses (402 / 409 / 429 / other), so every
//     forum write path handles payment-required / conflict / rate-limit the same
//     way instead of duplicating ad hoc status branches.
//
// Scope guardrail (audit A5): the user's node only posts its OWN intro and
// discovers work. It NEVER auto-replies to arbitrary threads — replying to
// other agents is the server-side Artanis responder's job. These caps bound the
// node's own bounded writes + their retries; they are not a general posting loop.

// Modeled on the Artanis responder caps (apps/openagents.com/workers/api/src/
// artanis-forum-responder.ts). Per-day bounds total forum write attempts; the
// per-tick bound caps writes attempted in a single onboarding poll.
export const FORUM_LOOP_MAX_WRITES_PER_DAY = 20
export const FORUM_LOOP_MAX_WRITES_PER_TICK = 3

const LEDGER_FILENAME = "forum-loop-ledger.json"

const defaultReadFile: ReadFile = (path: string) => {
  try {
    if (!existsSync(path)) return null
    return readFileSync(path, "utf8")
  } catch {
    return null
  }
}

const defaultWriteFile: WriteFile = (path: string, content: string) => {
  writeFileSync(path, content, { mode: 0o600 })
}

// UTC calendar day key (YYYY-MM-DD) for the daily-cap window.
export const utcDayKey = (date: Date = new Date()): string =>
  date.toISOString().slice(0, 10)

export type ForumLoopLedger = {
  // UTC day this counter applies to. A new day resets the count.
  readonly day: string
  // Forum write attempts recorded so far today.
  readonly writes: number
}

/**
 * Load the forum-loop attempt ledger for the current UTC day. A ledger from a
 * previous day reads as a fresh `{ day: today, writes: 0 }` (the window rolls
 * over). Never throws.
 */
export const loadForumLoopLedger = (
  home: string,
  readFile: ReadFile = defaultReadFile,
  now: Date = new Date(),
): ForumLoopLedger => {
  const today = utcDayKey(now)
  const raw = readFile(join(home, LEDGER_FILENAME))
  if (raw === null) return { day: today, writes: 0 }
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return { day: today, writes: 0 }
  }
  if (typeof parsed !== "object" || parsed === null) {
    return { day: today, writes: 0 }
  }
  const r = parsed as Record<string, unknown>
  if (r.day !== today || typeof r.writes !== "number" || r.writes < 0) {
    return { day: today, writes: 0 }
  }
  return { day: today, writes: Math.trunc(r.writes) }
}

/**
 * Remaining forum write budget for the current UTC day.
 */
export const forumWriteBudgetRemaining = (
  home: string,
  readFile: ReadFile = defaultReadFile,
  now: Date = new Date(),
): number => {
  const ledger = loadForumLoopLedger(home, readFile, now)
  return Math.max(0, FORUM_LOOP_MAX_WRITES_PER_DAY - ledger.writes)
}

/**
 * Whether a forum write may be attempted now (daily cap not yet exhausted).
 */
export const canAttemptForumWrite = (
  home: string,
  readFile: ReadFile = defaultReadFile,
  now: Date = new Date(),
): boolean => forumWriteBudgetRemaining(home, readFile, now) > 0

/**
 * Record one forum write attempt against the current UTC day's counter.
 * Idempotent w.r.t. the day window (rolls over at UTC midnight). Never throws.
 */
export const recordForumWriteAttempt = (
  home: string,
  readFile: ReadFile = defaultReadFile,
  writeFile: WriteFile = defaultWriteFile,
  now: Date = new Date(),
): ForumLoopLedger => {
  const ledger = loadForumLoopLedger(home, readFile, now)
  const next: ForumLoopLedger = { day: ledger.day, writes: ledger.writes + 1 }
  try {
    writeFile(join(home, LEDGER_FILENAME), `${JSON.stringify(next, null, 2)}\n`)
  } catch {
    // Best-effort: a failed ledger write must never crash the loop.
  }
  return next
}

// --- typed forum write response classification ------------------------------

export type ForumWriteDisposition =
  // 2xx — the write succeeded (or returned an idempotent success).
  | "ok"
  // 402 — payment required. The automated loop NEVER spends; defer.
  | "payment_required"
  // 409 — idempotency / uniqueness conflict (already exists).
  | "conflict"
  // 429 — rate limited; back off.
  | "rate_limited"
  // any other non-2xx.
  | "error"

/**
 * Classify a forum write HTTP status into a typed disposition so every write
 * path handles 402/409/429 consistently. Pure.
 */
export const classifyForumWriteStatus = (
  status: number,
): ForumWriteDisposition => {
  if (status >= 200 && status < 300) return "ok"
  if (status === 402) return "payment_required"
  if (status === 409) return "conflict"
  if (status === 429) return "rate_limited"
  return "error"
}
