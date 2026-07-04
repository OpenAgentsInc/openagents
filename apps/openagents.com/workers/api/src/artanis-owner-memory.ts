// Artanis owner-interaction memory (issue #6363, epic #6359).
//
// Persistent, OWNER-SCOPED memory for the "Talk to Artanis" operator channel.
// This is the half of #6363 that lets the real operator agent REMEMBER THE
// OWNER: prior owner<->Artanis conversation turns plus durable notes (stated
// decisions, preferences) survive across sessions, so continuity holds instead
// of starting cold each time.
//
// Privacy invariants (load-bearing):
//   - Every row is keyed by `ownerId`. `loadArtanisMemory(ownerId)` returns ONLY
//     that owner's rows. One owner can never read another's memory.
//   - This memory is private operator state. It is never projected publicly, and
//     it must never feed the public Khala collective-intelligence identity, any
//     public counter, or any public projection.
//
// The Artanis operator core (separate lane, #6359) consumes exactly the two
// exported functions `loadArtanisMemory` / `appendArtanisMemory` via the
// `ArtanisOwnerMemoryStore` they hang off. The store is injectable so it is
// independently unit-testable against an in-memory fake or a real D1 binding.

import {
  artanisAuthorityDb,
  mirrorArtanisRows,
  type ArtanisDatabase,
} from './artanis-domain-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

// Bounded so a single owner timeline read stays cheap and a single turn/note
// can't blow up a row. Mirrors the khala_feedback bounds.
export const ARTANIS_MEMORY_MAX_BODY_CHARS = 8_000
export const ARTANIS_MEMORY_MAX_LIMIT = 200
export const ARTANIS_MEMORY_DEFAULT_LIMIT = 50

export type ArtanisMemoryRole = 'owner' | 'artanis'
export type ArtanisMemoryNoteCategory = 'decision' | 'preference' | 'fact'

// A single thing appended to an owner's memory. Either a conversation turn
// (role + text of one owner/artanis message) or a durable note (a decision,
// stated preference, or fact worth carrying forward).
export type ArtanisMemoryTurn =
  | Readonly<{ kind: 'turn'; role: ArtanisMemoryRole; body: string }>
  | Readonly<{
      kind: 'note'
      noteCategory: ArtanisMemoryNoteCategory
      body: string
    }>

// A stored, ordered memory entry as read back from the store.
export type ArtanisMemoryEntry = Readonly<{
  memoryRef: string
  ownerId: string
  kind: 'turn' | 'note'
  role: ArtanisMemoryRole | null
  noteCategory: ArtanisMemoryNoteCategory | null
  body: string
  createdAt: string
}>

export type ArtanisMemoryLoadInput = Readonly<{
  ownerId: string
  limit: number
  // When true, return only durable notes (decisions/preferences/facts).
  notesOnly?: boolean | undefined
}>

export type ArtanisMemoryAppendInput = Readonly<{
  memoryRef: string
  ownerId: string
  turn: ArtanisMemoryTurn
  createdAt: string
}>

export type ArtanisOwnerMemoryStore = Readonly<{
  append: (input: ArtanisMemoryAppendInput) => Promise<ArtanisMemoryEntry>
  // Returns the owner's entries most-recent-first, bounded by `limit`.
  load: (
    input: ArtanisMemoryLoadInput,
  ) => Promise<ReadonlyArray<ArtanisMemoryEntry>>
}>

export class ArtanisMemoryValidationError extends Error {
  readonly _tag = 'ArtanisMemoryValidationError'
  constructor(reason: string) {
    super(reason)
    this.name = 'ArtanisMemoryValidationError'
  }
}

const requireNonEmptyOwner = (ownerId: string): string => {
  const cleaned = ownerId?.trim?.() ?? ''
  if (cleaned.length === 0) {
    throw new ArtanisMemoryValidationError('ownerId must be a non-empty string')
  }
  return cleaned
}

const normalizeLimit = (
  limit: number | undefined,
  fallback: number = ARTANIS_MEMORY_DEFAULT_LIMIT,
): number => {
  const requested =
    limit === undefined || !Number.isFinite(limit) ? fallback : Math.trunc(limit)
  return Math.min(Math.max(requested, 1), ARTANIS_MEMORY_MAX_LIMIT)
}

const validateBody = (body: string): string => {
  const cleaned = (body ?? '').trim()
  if (cleaned.length === 0) {
    throw new ArtanisMemoryValidationError('memory body must be a non-empty string')
  }
  if (cleaned.length > ARTANIS_MEMORY_MAX_BODY_CHARS) {
    throw new ArtanisMemoryValidationError(
      `memory body must be ${ARTANIS_MEMORY_MAX_BODY_CHARS} characters or fewer`,
    )
  }
  return cleaned
}

// Validate + normalize a turn into the columns the store persists. Throws a
// typed validation error rather than silently dropping bad shapes.
export const normalizeArtanisMemoryTurn = (
  turn: ArtanisMemoryTurn,
): Readonly<{
  kind: 'turn' | 'note'
  role: ArtanisMemoryRole | null
  noteCategory: ArtanisMemoryNoteCategory | null
  body: string
}> => {
  const body = validateBody(turn.body)
  if (turn.kind === 'turn') {
    if (turn.role !== 'owner' && turn.role !== 'artanis') {
      throw new ArtanisMemoryValidationError(
        "turn role must be 'owner' or 'artanis'",
      )
    }
    return { body, kind: 'turn', noteCategory: null, role: turn.role }
  }
  if (turn.kind === 'note') {
    if (
      turn.noteCategory !== 'decision' &&
      turn.noteCategory !== 'preference' &&
      turn.noteCategory !== 'fact'
    ) {
      throw new ArtanisMemoryValidationError(
        "note category must be 'decision', 'preference', or 'fact'",
      )
    }
    return {
      body,
      kind: 'note',
      noteCategory: turn.noteCategory,
      role: null,
    }
  }
  throw new ArtanisMemoryValidationError('memory turn kind must be turn or note')
}

type ArtanisMemoryRow = Readonly<{
  memory_ref: string
  owner_id: string
  kind: string
  role: string | null
  note_category: string | null
  body: string
  created_at: string
}>

const rowToEntry = (row: ArtanisMemoryRow): ArtanisMemoryEntry => ({
  body: row.body,
  createdAt: row.created_at,
  kind: row.kind === 'note' ? 'note' : 'turn',
  memoryRef: row.memory_ref,
  noteCategory:
    row.note_category === 'decision' ||
    row.note_category === 'preference' ||
    row.note_category === 'fact'
      ? row.note_category
      : null,
  ownerId: row.owner_id,
  role: row.role === 'owner' || row.role === 'artanis' ? row.role : null,
})

const memoryRef = (makeRef?: () => string): string =>
  makeRef?.() ?? `artanis_memory:${compactRandomId('mem')}`

// D1-backed store. Every read is filtered by owner_id in the SQL, so the
// owner-scoping invariant is enforced at the query, not by a caller convention.
export const makeD1ArtanisOwnerMemoryStore = (
  database: ArtanisDatabase,
): ArtanisOwnerMemoryStore => {
  // The authoritative D1 handle; appends mirror to Postgres through the
  // KS-8.6 seam (fail-soft). Reads stay owner-scoped on D1 authority.
  const db = artanisAuthorityDb(database)
  return {
  append: async input => {
    const normalized = normalizeArtanisMemoryTurn(input.turn)
    await db
      .prepare(
        `INSERT INTO artanis_owner_memory (
            memory_ref,
            owner_id,
            kind,
            role,
            note_category,
            body,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        input.memoryRef,
        input.ownerId,
        normalized.kind,
        normalized.role,
        normalized.noteCategory,
        normalized.body,
        input.createdAt,
      )
      .run()
    await mirrorArtanisRows(database, 'artanis_owner_memory', 'memory_ref', [
      input.memoryRef,
    ])

    return {
      body: normalized.body,
      createdAt: input.createdAt,
      kind: normalized.kind,
      memoryRef: input.memoryRef,
      noteCategory: normalized.noteCategory,
      ownerId: input.ownerId,
      role: normalized.role,
    }
  },

  load: async input => {
    const limit = normalizeLimit(input.limit)
    const query =
      input.notesOnly === true
        ? db
            .prepare(
              `SELECT memory_ref, owner_id, kind, role, note_category, body, created_at
                 FROM artanis_owner_memory
                WHERE owner_id = ? AND kind = 'note'
                ORDER BY created_at DESC
                LIMIT ?`,
            )
            .bind(input.ownerId, limit)
        : db
            .prepare(
              `SELECT memory_ref, owner_id, kind, role, note_category, body, created_at
                 FROM artanis_owner_memory
                WHERE owner_id = ?
                ORDER BY created_at DESC
                LIMIT ?`,
            )
            .bind(input.ownerId, limit)

    const rows = await query.all<ArtanisMemoryRow>()
    return rows.results.map(rowToEntry)
  },
  }
}

// Contract surface consumed by the Artanis operator core (#6359).
//
// loadArtanisMemory(ownerId, limit?) -> owner's recent memory, most recent first.
// Owner-scoped: only that owner's entries are ever returned.
export const loadArtanisMemory = async (
  store: ArtanisOwnerMemoryStore,
  ownerId: string,
  limit?: number,
): Promise<ReadonlyArray<ArtanisMemoryEntry>> => {
  const owner = requireNonEmptyOwner(ownerId)
  return store.load({ limit: normalizeLimit(limit), ownerId: owner })
}

// loadArtanisMemoryNotes(ownerId, limit?) -> only the durable notes
// (decisions/preferences/facts), most recent first. Convenience read for the
// operator core when it wants stated preferences without the full transcript.
export const loadArtanisMemoryNotes = async (
  store: ArtanisOwnerMemoryStore,
  ownerId: string,
  limit?: number,
): Promise<ReadonlyArray<ArtanisMemoryEntry>> => {
  const owner = requireNonEmptyOwner(ownerId)
  return store.load({
    limit: normalizeLimit(limit),
    notesOnly: true,
    ownerId: owner,
  })
}

export type AppendArtanisMemoryOptions = Readonly<{
  makeMemoryRef?: (() => string) | undefined
  nowIso?: (() => string) | undefined
}>

// appendArtanisMemory(ownerId, turn) -> persists one conversation turn or
// durable note to the owner's private memory and returns the stored entry.
export const appendArtanisMemory = async (
  store: ArtanisOwnerMemoryStore,
  ownerId: string,
  turn: ArtanisMemoryTurn,
  options: AppendArtanisMemoryOptions = {},
): Promise<ArtanisMemoryEntry> => {
  const owner = requireNonEmptyOwner(ownerId)
  const nowIso = options.nowIso ?? currentIsoTimestamp
  return store.append({
    createdAt: nowIso(),
    memoryRef: memoryRef(options.makeMemoryRef),
    ownerId: owner,
    turn,
  })
}
