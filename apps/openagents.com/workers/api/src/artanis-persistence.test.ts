import { readFileSync } from 'node:fs'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  exampleArtanisApprovalGateLedger,
} from './artanis-approval-gates'
import {
  exampleArtanisForumPublicationQueue,
} from './artanis-forum-publication'
import {
  exampleArtanisHealthSnapshot,
} from './artanis-health'
import {
  exampleArtanisLoopLedger,
} from './artanis-loop'
import { handlePublicArtanisReportApi } from './artanis-public-report-routes'
import {
  ArtanisPersistenceError,
  closeArtanisPersistedLoopTick,
  readArtanisPersistedRecord,
  saveArtanisApprovalGate,
  saveArtanisForumPublicationIntent,
  saveArtanisHealthSnapshot,
  saveArtanisLoopRecord,
  saveArtanisLoopTick,
  saveArtanisRuntimeSnapshot,
  saveArtanisWorkRoutingProposal,
} from './artanis-persistence'
import {
  exampleArtanisRuntime,
} from './artanis-runtime'
import {
  exampleArtanisWorkRoutingLedger,
} from './artanis-work-routing'

type PersistenceRow = Readonly<{
  active: number
  agent_id: string
  closed_at: string | null
  closeout_json: string | null
  content_hash: string
  created_at: string
  id: string
  idempotency_key: string
  parent_ref: string | null
  public_projection_json: string
  record_json: string
  record_ref: string
  scope_ref: string | null
  source_kind: string
  state: string
  updated_at: string
}>

class ArtanisPersistenceStore {
  tables = new Map<string, Array<PersistenceRow>>()

  rows(table: string): Array<PersistenceRow> {
    const existing = this.tables.get(table)

    if (existing !== undefined) {
      return existing
    }

    const rows: Array<PersistenceRow> = []
    this.tables.set(table, rows)

    return rows
  }
}

class ArtanisPersistenceStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ArtanisPersistenceStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    const table = tableName(this.query)
    const rows = this.store.rows(table)

    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        rows.find(item => item.idempotency_key === idempotencyKey) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('WHERE record_ref = ?')) {
      const recordRef = String(this.values[0])
      const row = rows.find(item => item.record_ref === recordRef) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first query: ${this.query}`))
  }

  raw<T = unknown[]>(options: { columnNames: true }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(
    options?: { columnNames?: boolean },
  ): Promise<T[] | [string[], ...T[]]> {
    return options?.columnNames === true
      ? Promise.resolve([[]] as [string[], ...T[]])
      : Promise.resolve([] as T[])
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableName(this.query)
    const rows = this.store.rows(table)

    if (this.query.includes('INSERT INTO')) {
      if (
        rows.every(
          row =>
            row.record_ref !== String(this.values[2]) &&
            row.idempotency_key !== String(this.values[3]),
        )
      ) {
        rows.push({
          active: Number(this.values[5]),
          agent_id: String(this.values[1]),
          closed_at: this.values[15] === null ? null : String(this.values[15]),
          closeout_json:
            this.values[12] === null ? null : String(this.values[12]),
          content_hash: String(this.values[11]),
          created_at: String(this.values[13]),
          id: String(this.values[0]),
          idempotency_key: String(this.values[3]),
          parent_ref: this.values[8] === null ? null : String(this.values[8]),
          public_projection_json: String(this.values[10]),
          record_json: String(this.values[9]),
          record_ref: String(this.values[2]),
          scope_ref: this.values[7] === null ? null : String(this.values[7]),
          source_kind: String(this.values[6]),
          state: String(this.values[4]),
          updated_at: String(this.values[14]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE artanis_loop_ticks')) {
      const recordRef = String(this.values[7])
      const index = rows.findIndex(
        row => row.record_ref === recordRef && row.closed_at === null,
      )

      if (index !== -1) {
        const existing = rows[index]!
        rows[index] = {
          ...existing,
          closed_at: String(this.values[6]),
          closeout_json: String(this.values[1]),
          content_hash: String(this.values[4]),
          public_projection_json: String(this.values[3]),
          record_json: String(this.values[2]),
          state: String(this.values[0]),
          updated_at: String(this.values[5]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run query: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableName(this.query)
    const rows = this.store.rows(table)

    if (this.query.includes('ORDER BY updated_at DESC')) {
      return Promise.resolve({
        results: [...rows]
          .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
          .slice(0, Number(this.values[0])),
      } as unknown as D1Result<T>)
    }

    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }
}

const tableName = (query: string): string => {
  const match =
    /\bFROM\s+([a-z_]+)\b/i.exec(query) ??
    /\bINTO\s+([a-z_]+)\b/i.exec(query) ??
    /\bUPDATE\s+([a-z_]+)\b/i.exec(query)

  if (match === null) {
    throw new Error(`No table name found in query: ${query}`)
  }

  return match[1]!
}

const artanisDb = (store: ArtanisPersistenceStore): D1Database =>
  ({
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run())) as Promise<
        Array<D1Result<T>>
      >,
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) =>
      new ArtanisPersistenceStatement(query, store),
    withSession: () => artanisDb(store),
  }) as unknown as D1Database

const nowIso = '2026-06-07T05:00:00.000Z'

describe('Artanis persistence', () => {
  test('migration creates the Artanis persistence table family', async () => {
    const migration = readFileSync(
      new URL('../migrations/0119_artanis_persistence.sql', import.meta.url),
      'utf8',
    )

    expect(migration).toContain('CREATE TABLE IF NOT EXISTS artanis_runtime_snapshots')
    expect(migration).toContain('CREATE TABLE IF NOT EXISTS artanis_loop_records')
    expect(migration).toContain('CREATE TABLE IF NOT EXISTS artanis_loop_ticks')
    expect(migration).toContain('CREATE TABLE IF NOT EXISTS artanis_approval_gates')
    expect(migration).toContain('CREATE TABLE IF NOT EXISTS artanis_health_snapshots')
    expect(migration).toContain('CREATE TABLE IF NOT EXISTS artanis_work_routing_proposals')
    expect(migration).toContain('CREATE TABLE IF NOT EXISTS artanis_forum_publication_intents')
    expect(migration).toContain('idx_artanis_loop_records_one_active_scope')
    expect(migration).toContain('idempotency_key TEXT NOT NULL UNIQUE')
    expect(migration).toContain('public_projection_json TEXT NOT NULL')
  })

  test('inserts, idempotently retries, suppresses duplicate refs, and reads public projection', async () => {
    const store = new ArtanisPersistenceStore()
    const db = artanisDb(store)
    const runtime = exampleArtanisRuntime()

    const inserted = await Effect.runPromise(
      saveArtanisRuntimeSnapshot(
        db,
        runtime,
        'artanis-runtime:20260607T0036',
        nowIso,
      ),
    )
    const retried = await Effect.runPromise(
      saveArtanisRuntimeSnapshot(
        db,
        runtime,
        'artanis-runtime:20260607T0036',
        nowIso,
      ),
    )
    const duplicateRef = await Effect.runPromise(
      saveArtanisRuntimeSnapshot(
        db,
        runtime,
        'artanis-runtime:20260607T0036:duplicate-ref',
        nowIso,
      ),
    )
    const stored = await Effect.runPromise(
      readArtanisPersistedRecord(
        db,
        'runtime_snapshot',
        runtime.runtimeRef,
      ),
    )

    expect(inserted).toMatchObject({
      executableAuthority: false,
      idempotent: false,
      kind: 'runtime_snapshot',
      state: 'inserted',
    })
    expect(retried).toMatchObject({
      executableAuthority: false,
      idempotent: true,
      state: 'retried',
    })
    expect(duplicateRef).toMatchObject({
      executableAuthority: false,
      idempotent: true,
      state: 'retried',
    })
    expect(stored?.publicProjection).toMatchObject({
      agentId: 'agent_artanis',
      audience: 'public',
      walletSpendAllowed: false,
    })
    expect(JSON.stringify(stored?.publicProjection)).not.toMatch(
      /evidence\.private|operator\.private|wallet_secret|raw_log/i,
    )
    expect(store.rows('artanis_runtime_snapshots')).toHaveLength(1)
  })

  test('rejects conflicting idempotency retries', async () => {
    const store = new ArtanisPersistenceStore()
    const db = artanisDb(store)
    const runtime = exampleArtanisRuntime()
    const changedRuntime = {
      ...runtime,
      state: 'idle' as const,
      updatedAtIso: '2026-06-07T00:40:00.000Z',
    }

    await Effect.runPromise(
      saveArtanisRuntimeSnapshot(
        db,
        runtime,
        'artanis-runtime:conflict',
        nowIso,
      ),
    )

    await expect(
      Effect.runPromise(
        saveArtanisRuntimeSnapshot(
          db,
          changedRuntime,
          'artanis-runtime:conflict',
          nowIso,
        ),
      ),
    ).rejects.toBeInstanceOf(ArtanisPersistenceError)
  })

  test('persists every Artanis evidence family without granting executable authority', async () => {
    const store = new ArtanisPersistenceStore()
    const db = artanisDb(store)
    const loop = exampleArtanisLoopLedger().loops[0]!
    const tick = loop.ticks[0]!
    const gate = exampleArtanisApprovalGateLedger.gates[0]!
    const health = exampleArtanisHealthSnapshot
    const proposal = exampleArtanisWorkRoutingLedger.proposals[1]!
    const intent = exampleArtanisForumPublicationQueue().intents[0]!

    const receipts = await Promise.all([
      Effect.runPromise(saveArtanisLoopRecord(
        db,
        loop,
        'artanis-loop:primary',
        nowIso,
      )),
      Effect.runPromise(saveArtanisLoopTick(db, tick, nowIso)),
      Effect.runPromise(saveArtanisApprovalGate(db, gate, nowIso)),
      Effect.runPromise(saveArtanisHealthSnapshot(db, health, nowIso)),
      Effect.runPromise(saveArtanisWorkRoutingProposal(db, proposal, nowIso)),
      Effect.runPromise(saveArtanisForumPublicationIntent(db, intent, nowIso)),
    ])

    expect(receipts.map(receipt => receipt.executableAuthority)).toEqual([
      false,
      false,
      false,
      false,
      false,
      false,
    ])
    expect(receipts.map(receipt => receipt.kind)).toEqual([
      'loop_record',
      'loop_tick',
      'approval_gate',
      'health_snapshot',
      'work_routing_proposal',
      'forum_publication_intent',
    ])
    expect(
      store.rows('artanis_approval_gates')[0]!.public_projection_json,
    ).not.toMatch(/evidence\.private|receipt\.operator|wallet_secret|raw_log/i)
  })

  test('closes loop ticks idempotently and rejects conflicting closeouts', async () => {
    const store = new ArtanisPersistenceStore()
    const db = artanisDb(store)
    const tick = exampleArtanisLoopLedger().loops[0]!.ticks[0]!

    await Effect.runPromise(saveArtanisLoopTick(db, tick, nowIso))

    const closed = await Effect.runPromise(
      closeArtanisPersistedLoopTick(db, tick.tickRef, {
        closedAtIso: '2026-06-07T05:01:00.000Z',
        closeoutReceiptRefs: ['receipt.public.artanis.tick_closeout'],
        state: 'completed',
        updatedAtIso: '2026-06-07T05:01:00.000Z',
      }),
    )
    const retry = await Effect.runPromise(
      closeArtanisPersistedLoopTick(db, tick.tickRef, {
        closedAtIso: '2026-06-07T05:01:00.000Z',
        closeoutReceiptRefs: ['receipt.public.artanis.tick_closeout'],
        state: 'completed',
        updatedAtIso: '2026-06-07T05:02:00.000Z',
      }),
    )
    const stored = await Effect.runPromise(
      readArtanisPersistedRecord(db, 'loop_tick', tick.tickRef),
    )
    const reportResponse = await Effect.runPromise(
      handlePublicArtanisReportApi(
        new Request('https://openagents.com/api/public/artanis/report'),
        {
          OPENAGENTS_DB: db,
          store: { listRegistrations: () => Promise.resolve([]) },
        },
      ),
    )
    const report = (await reportResponse.json()) as Readonly<{
      autonomousLoop?: Readonly<{
        latestTickRef?: string | null
        latestTickState?: string | null
        receiptRefs?: ReadonlyArray<string>
        tickCount?: number
      }>
    }>

    await expect(
      Effect.runPromise(
        closeArtanisPersistedLoopTick(db, tick.tickRef, {
          closedAtIso: '2026-06-07T05:03:00.000Z',
          closeoutReceiptRefs: ['receipt.public.artanis.changed_closeout'],
          state: 'completed',
          updatedAtIso: '2026-06-07T05:03:00.000Z',
        }),
      ),
    ).rejects.toBeInstanceOf(ArtanisPersistenceError)

    expect(closed).toMatchObject({
      closedAtIso: '2026-06-07T05:01:00.000Z',
      executableAuthority: false,
      state: 'closed',
    })
    expect(closed.publicProjection).toMatchObject({
      loops: [
        {
          ticks: [
            {
              closeoutReceiptRefs: [
                'receipt.public.artanis.tick_closeout',
              ],
              state: 'completed',
            },
          ],
        },
      ],
    })
    expect(retry).toMatchObject({
      closedAtIso: '2026-06-07T05:01:00.000Z',
      idempotent: true,
      state: 'closed',
    })
    expect(stored).toMatchObject({
      closedAtIso: '2026-06-07T05:01:00.000Z',
      state: 'completed',
      record: {
        closeoutReceiptRefs: ['receipt.public.artanis.tick_closeout'],
        state: 'completed',
      },
      publicProjection: {
        loops: [
          {
            ticks: [
              {
                closeoutReceiptRefs: [
                  'receipt.public.artanis.tick_closeout',
                ],
                state: 'completed',
              },
            ],
          },
        ],
      },
    })
    expect(reportResponse.status).toBe(200)
    expect(report.autonomousLoop).toMatchObject({
      latestTickRef: tick.tickRef,
      latestTickState: 'completed',
      receiptRefs: expect.arrayContaining([
        'receipt.public.artanis.tick_closeout',
      ]),
      tickCount: 1,
    })
  })
})
