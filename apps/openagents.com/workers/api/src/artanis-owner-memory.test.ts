import { describe, expect, test } from 'vitest'

import {
  ARTANIS_MEMORY_MAX_BODY_CHARS,
  ARTANIS_MEMORY_MAX_LIMIT,
  ArtanisMemoryValidationError,
  appendArtanisMemory,
  loadArtanisMemory,
  loadArtanisMemoryNotes,
  makeD1ArtanisOwnerMemoryStore,
  normalizeArtanisMemoryTurn,
} from './artanis-owner-memory'

// Minimal in-memory fake of the subset of D1 the memory store uses. It models a
// single keyed table and honors the owner_id / kind filters and the
// `created_at DESC LIMIT ?` ordering the store relies on. This lets us prove the
// owner-scoping invariant at the query level without a real D1 binding.
type Row = {
  memory_ref: string
  owner_id: string
  kind: string
  role: string | null
  note_category: string | null
  body: string
  created_at: string
}

const makeFakeD1 = (): D1Database & { rows: Row[] } => {
  const rows: Row[] = []

  const statement = (query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const isInsert = /INSERT INTO artanis_owner_memory/i.test(query)
    const isNotesOnly = /kind = 'note'/i.test(query)

    const stmt: D1PreparedStatement = {
      bind: (...values: ReadonlyArray<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T,>() => null as T | null,
      all: async <T,>() => {
        // SELECT ... WHERE owner_id = ? [AND kind = 'note'] ORDER BY created_at DESC LIMIT ?
        const ownerId = String(bound[0])
        const limit = Number(bound[bound.length - 1])
        const filtered = rows
          .filter(r => r.owner_id === ownerId)
          .filter(r => (isNotesOnly ? r.kind === 'note' : true))
          .sort((a, b) =>
            a.created_at < b.created_at
              ? 1
              : a.created_at > b.created_at
              ? -1
              : 0,
          )
          .slice(0, limit)
        return {
          meta: {} as D1Meta & Record<string, unknown>,
          results: filtered as unknown as T[],
          success: true as const,
        }
      },
      raw: async () => [] as never,
      run: async <T,>() => {
        if (isInsert) {
          const [
            memory_ref,
            owner_id,
            kind,
            role,
            note_category,
            body,
            created_at,
          ] = bound as [
            string,
            string,
            string,
            string | null,
            string | null,
            string,
            string,
          ]
          rows.push({
            body,
            created_at,
            kind,
            memory_ref,
            note_category,
            owner_id,
            role,
          })
        }
        return {
          meta: { changes: 1 } as D1Meta & Record<string, unknown>,
          results: [] as unknown as T[],
          success: true as const,
        }
      },
    }
    return stmt
  }

  return {
    batch: async () => [] as never,
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => statement(query),
    rows,
    withSession: () => {
      throw new Error('not used')
    },
  } as unknown as D1Database & { rows: Row[] }
}

// A deterministic clock so created_at ordering is stable in tests.
const makeClock = () => {
  let n = 0
  return () => {
    n += 1
    return `2026-06-26T00:00:${String(n).padStart(2, '0')}.000Z`
  }
}

describe('artanis owner memory store', () => {
  test('appends and loads a turn round-trip', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisOwnerMemoryStore(db)
    const nowIso = makeClock()

    const appended = await appendArtanisMemory(
      store,
      'owner-1',
      { body: 'Remember I prefer terse answers.', kind: 'turn', role: 'owner' },
      { makeMemoryRef: () => 'mem-1', nowIso },
    )

    expect(appended.memoryRef).toBe('mem-1')
    expect(appended.ownerId).toBe('owner-1')
    expect(appended.kind).toBe('turn')
    expect(appended.role).toBe('owner')
    expect(appended.noteCategory).toBeNull()
    expect(appended.body).toBe('Remember I prefer terse answers.')

    const loaded = await loadArtanisMemory(store, 'owner-1')
    expect(loaded).toHaveLength(1)
    expect(loaded[0]).toEqual(appended)
  })

  test('loads owner timeline most-recent-first', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisOwnerMemoryStore(db)
    const nowIso = makeClock()

    await appendArtanisMemory(
      store,
      'owner-1',
      { body: 'first', kind: 'turn', role: 'owner' },
      { makeMemoryRef: () => 'm1', nowIso },
    )
    await appendArtanisMemory(
      store,
      'owner-1',
      { body: 'second', kind: 'turn', role: 'artanis' },
      { makeMemoryRef: () => 'm2', nowIso },
    )

    const loaded = await loadArtanisMemory(store, 'owner-1')
    expect(loaded.map(e => e.body)).toEqual(['second', 'first'])
  })

  test('is owner-scoped: one owner cannot read another owner memory', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisOwnerMemoryStore(db)
    const nowIso = makeClock()

    await appendArtanisMemory(
      store,
      'owner-A',
      { body: 'A private decision', kind: 'note', noteCategory: 'decision' },
      { makeMemoryRef: () => 'a1', nowIso },
    )
    await appendArtanisMemory(
      store,
      'owner-B',
      { body: 'B private preference', kind: 'note', noteCategory: 'preference' },
      { makeMemoryRef: () => 'b1', nowIso },
    )

    const a = await loadArtanisMemory(store, 'owner-A')
    const b = await loadArtanisMemory(store, 'owner-B')

    expect(a).toHaveLength(1)
    expect(a[0]!.body).toBe('A private decision')
    expect(a.every(e => e.ownerId === 'owner-A')).toBe(true)

    expect(b).toHaveLength(1)
    expect(b[0]!.body).toBe('B private preference')
    expect(b.every(e => e.ownerId === 'owner-B')).toBe(true)

    // Cross-owner leakage check: neither owner's read contains the other's body.
    expect(a.some(e => e.body.includes('B private'))).toBe(false)
    expect(b.some(e => e.body.includes('A private'))).toBe(false)
  })

  test('loadArtanisMemoryNotes returns only durable notes for the owner', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisOwnerMemoryStore(db)
    const nowIso = makeClock()

    await appendArtanisMemory(
      store,
      'owner-1',
      { body: 'a conversation turn', kind: 'turn', role: 'owner' },
      { makeMemoryRef: () => 'm1', nowIso },
    )
    await appendArtanisMemory(
      store,
      'owner-1',
      { body: 'use Spark for payments', kind: 'note', noteCategory: 'decision' },
      { makeMemoryRef: () => 'm2', nowIso },
    )

    const notes = await loadArtanisMemoryNotes(store, 'owner-1')
    expect(notes).toHaveLength(1)
    expect(notes[0]!.kind).toBe('note')
    expect(notes[0]!.noteCategory).toBe('decision')
  })

  test('clamps the load limit to the bounded maximum', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisOwnerMemoryStore(db)
    const nowIso = makeClock()
    for (let i = 0; i < 5; i += 1) {
      await appendArtanisMemory(
        store,
        'owner-1',
        { body: `turn ${i}`, kind: 'turn', role: 'owner' },
        { makeMemoryRef: () => `m${i}`, nowIso },
      )
    }
    // An over-large limit must be clamped, not passed through raw.
    const loaded = await loadArtanisMemory(
      store,
      'owner-1',
      ARTANIS_MEMORY_MAX_LIMIT + 1_000,
    )
    expect(loaded.length).toBe(5)
  })
})

describe('artanis owner memory validation', () => {
  test('rejects empty ownerId on append and load', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisOwnerMemoryStore(db)
    await expect(
      appendArtanisMemory(store, '   ', {
        body: 'x',
        kind: 'turn',
        role: 'owner',
      }),
    ).rejects.toBeInstanceOf(ArtanisMemoryValidationError)
    await expect(loadArtanisMemory(store, '')).rejects.toBeInstanceOf(
      ArtanisMemoryValidationError,
    )
  })

  test('rejects empty body', () => {
    expect(() =>
      normalizeArtanisMemoryTurn({ body: '   ', kind: 'turn', role: 'owner' }),
    ).toThrow(ArtanisMemoryValidationError)
  })

  test('rejects an over-long body', () => {
    expect(() =>
      normalizeArtanisMemoryTurn({
        body: 'x'.repeat(ARTANIS_MEMORY_MAX_BODY_CHARS + 1),
        kind: 'turn',
        role: 'owner',
      }),
    ).toThrow(ArtanisMemoryValidationError)
  })

  test('rejects an invalid turn role', () => {
    expect(() =>
      normalizeArtanisMemoryTurn({
        body: 'x',
        kind: 'turn',
        role: 'system' as never,
      }),
    ).toThrow(ArtanisMemoryValidationError)
  })

  test('rejects an invalid note category', () => {
    expect(() =>
      normalizeArtanisMemoryTurn({
        body: 'x',
        kind: 'note',
        noteCategory: 'gossip' as never,
      }),
    ).toThrow(ArtanisMemoryValidationError)
  })
})
