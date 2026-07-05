// KS-8.6 (#8317): Artanis domain repository CONTRACT suite.
//
// One behavioral spec, TWO real engines:
//   - D1: real SQLite (node:sqlite — the engine D1 is built on), schema
//     condensed from the live worker migrations (0119/0120/0161/0163/0164/
//     0165/0169/0213/0215/0245/0248/0249/0256) below.
//   - Postgres: a throwaway local Postgres (initdb/pg_ctl), schema from
//     khala-sync-server migration 0010. Skipped when no local Postgres
//     binaries exist.
//
// The load-bearing properties:
//   1. REGISTRY FIDELITY: every one of the twenty artanis tables accepts a
//      registry-shaped row on BOTH engines, and `mirrorArtanisRows`
//      converges the Postgres twin to be column-for-column equal to the
//      resolved D1 row (the dual-write contract).
//   2. MIRROR IDEMPOTENCY: re-mirroring an unchanged row is a no-op; a
//      double-fired cron tick (same scheduled_at / same idempotency key)
//      leaves ONE identical row on both sides — the issue's landing
//      requirement that tick double-fire is a no-op survives the seam.
//   3. MUTATION CONVERGENCE: after a D1 state transition (responder action
//      proposed→responded; loop tick open→closed), the mirror converges the
//      full row — no stale column left behind.
//   4. READ EQUIVALENCE: the persistence reads (`readArtanisPersistedRecord`,
//      `readLatestArtanisPersistedRows`) decode byte-identically from D1 and
//      Postgres — the evidence that licenses the compare/postgres read modes.

import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'

import {
  ARTANIS_DOMAIN_TABLES,
  makeArtanisDomainHandle,
  makePostgresArtanisDomainStore,
  mirrorArtanisRows,
  type ArtanisDomainDiagnostic,
  type ArtanisDomainDiagnosticEvent,
  type ArtanisDomainHandle,
  type ArtanisDomainRow,
  type ArtanisDomainTable,
  type PostgresArtanisDomainStore,
} from './artanis-domain-store'
import { RESPONDER_STATE_SCAN_UPDATE_COLUMNS } from './artanis-forum-responder'
import { exampleArtanisLoopLedger } from './artanis-loop'
import {
  closeArtanisPersistedLoopTick,
  readArtanisPersistedRecord,
  readLatestArtanisPersistedRows,
  saveArtanisLoopTick,
} from './artanis-persistence'
import { RESPONDER_STATE_COMPOSE_UPDATE_COLUMNS } from './artanis-reply-composer'
import {
  SCAN_TICK_UPDATE_COLUMNS,
  recordArtanisResponderComposeTick,
  recordArtanisResponderScanTick,
} from './artanis-responder-ticks'
import { makeSqliteD1, type SqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// The D1 side of the schema (condensed from the live worker migrations)
// ---------------------------------------------------------------------------

const LEDGER_TABLE_DDL = (table: string): string => `
CREATE TABLE ${table} (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  record_ref TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 0,
  source_kind TEXT NOT NULL,
  scope_ref TEXT,
  parent_ref TEXT,
  record_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  closeout_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT
);`

export const ARTANIS_DOMAIN_D1_SCHEMA = `
${[
  'artanis_runtime_snapshots',
  'artanis_loop_records',
  'artanis_loop_ticks',
  'artanis_approval_gates',
  'artanis_health_snapshots',
  'artanis_work_routing_proposals',
  'artanis_forum_publication_intents',
  'artanis_nexus_pylon_adapter_dispatches',
]
  .map(LEDGER_TABLE_DDL)
  .join('\n')}

CREATE TABLE artanis_responder_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  scan_cursor_iso TEXT NOT NULL,
  responses_today INTEGER NOT NULL DEFAULT 0,
  responses_day TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL
);

CREATE TABLE artanis_responder_actions (
  id TEXT PRIMARY KEY,
  topic_id TEXT NOT NULL UNIQUE,
  first_post_id TEXT,
  question_class TEXT,
  state TEXT NOT NULL,
  proposal_json TEXT NOT NULL DEFAULT '{}',
  reply_post_id TEXT,
  asked_at TEXT,
  replied_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  tip_receipt_ref TEXT,
  tip_pay_in_id TEXT,
  tip_ladder_rung TEXT,
  tip_ladder_reason TEXT,
  asker_actor_ref TEXT,
  asker_provenance TEXT
);

CREATE TABLE artanis_responder_ticks (
  tick_ref TEXT PRIMARY KEY,
  scheduled_at TEXT NOT NULL UNIQUE,
  scan_state TEXT NOT NULL DEFAULT 'pending',
  scan_scanned INTEGER NOT NULL DEFAULT 0,
  scan_proposed INTEGER NOT NULL DEFAULT 0,
  scan_blocked INTEGER NOT NULL DEFAULT 0,
  scan_skipped INTEGER NOT NULL DEFAULT 0,
  scan_skipped_reason TEXT,
  compose_state TEXT NOT NULL DEFAULT 'pending',
  compose_considered INTEGER NOT NULL DEFAULT 0,
  compose_responded INTEGER NOT NULL DEFAULT 0,
  compose_blocked INTEGER NOT NULL DEFAULT 0,
  compose_tipped INTEGER NOT NULL DEFAULT 0,
  compose_skipped_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE artanis_admin_tick_decisions (
  id TEXT PRIMARY KEY,
  state TEXT NOT NULL,
  action_json TEXT NOT NULL DEFAULT '{}',
  assignment_ref TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE artanis_closeout_verdicts (
  id TEXT PRIMARY KEY,
  assignment_ref TEXT NOT NULL UNIQUE,
  outcome TEXT NOT NULL,
  claimed_trace_digest_prefix TEXT,
  accept_state TEXT NOT NULL,
  detail TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE TABLE artanis_fleet_overseer_decisions (
  id TEXT PRIMARY KEY,
  state TEXT NOT NULL,
  action_json TEXT NOT NULL,
  context_json TEXT NOT NULL,
  approval_gate_ref TEXT,
  health_snapshot_ref TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE artanis_standing_spend_grants (
  grant_ref TEXT PRIMARY KEY,
  per_payout_cap_sat INTEGER NOT NULL,
  per_day_cap_sat INTEGER NOT NULL,
  authority_ref TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE TABLE artanis_spend_decisions (
  id TEXT PRIMARY KEY,
  grant_ref TEXT NOT NULL,
  state TEXT NOT NULL,
  intended_amount_sat INTEGER NOT NULL,
  paid_amount_sat INTEGER,
  destination_source_ref TEXT NOT NULL,
  recipient_ref TEXT NOT NULL,
  rationale TEXT NOT NULL DEFAULT '',
  payment_ref TEXT,
  policy_applied TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE artanis_labor_unattended_receipts (
  receipt_ref TEXT PRIMARY KEY,
  serialized_json TEXT NOT NULL,
  terminal_state TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE artanis_owner_memory (
  memory_ref TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  role TEXT,
  note_category TEXT,
  body TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE artanis_threads (
  thread_ref TEXT PRIMARY KEY,
  caller_id TEXT NOT NULL,
  caller_kind TEXT NOT NULL,
  subject_agent_ref TEXT NOT NULL,
  subject_agent_kind TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open',
  source_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  last_message_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE artanis_messages (
  message_ref TEXT PRIMARY KEY,
  thread_ref TEXT NOT NULL,
  caller_id TEXT NOT NULL,
  author_id TEXT NOT NULL,
  author_kind TEXT NOT NULL,
  body TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);
`

// ---------------------------------------------------------------------------
// Fixtures — one registry-shaped sample row per table
// ---------------------------------------------------------------------------

const ISO = '2026-07-04T12:00:00.000Z'
const ALL_TABLES = Object.keys(
  ARTANIS_DOMAIN_TABLES,
) as ReadonlyArray<ArtanisDomainTable>

const ledgerRow = (recordRef: string): ArtanisDomainRow => ({
  active: 0,
  agent_id: 'agent_artanis',
  closed_at: null,
  closeout_json: null,
  content_hash: `hash-${recordRef}`,
  created_at: ISO,
  id: `kind:${recordRef}`,
  idempotency_key: `idem-${recordRef}`,
  parent_ref: 'loop.artanis.contract',
  public_projection_json: '{}',
  record_json: '{}',
  record_ref: recordRef,
  scope_ref: 'scope.public.artanis.contract',
  source_kind: 'loop_tick',
  state: 'running',
  updated_at: ISO,
})

const sampleRow = (table: ArtanisDomainTable): ArtanisDomainRow => {
  switch (table) {
    case 'artanis_runtime_snapshots':
    case 'artanis_loop_records':
    case 'artanis_loop_ticks':
    case 'artanis_approval_gates':
    case 'artanis_health_snapshots':
    case 'artanis_work_routing_proposals':
    case 'artanis_forum_publication_intents':
    case 'artanis_nexus_pylon_adapter_dispatches':
      return ledgerRow(`${table}.contract.1`)
    case 'artanis_responder_state':
      return {
        id: 1,
        responses_day: '2026-07-04',
        responses_today: 3,
        scan_cursor_iso: ISO,
        updated_at: ISO,
      }
    case 'artanis_responder_actions':
      return {
        asked_at: ISO,
        asker_actor_ref: 'actor.forum.contract',
        asker_provenance: 'external',
        created_at: ISO,
        first_post_id: 'post-contract-1',
        id: 'action-contract-1',
        proposal_json: '{"verdict":null}',
        question_class: 'payout',
        replied_at: null,
        reply_post_id: null,
        state: 'proposed',
        tip_ladder_reason: null,
        tip_ladder_rung: null,
        tip_pay_in_id: null,
        tip_receipt_ref: null,
        topic_id: 'topic-contract-1',
        updated_at: ISO,
      }
    case 'artanis_responder_ticks':
      return {
        compose_blocked: 0,
        compose_considered: 0,
        compose_responded: 0,
        compose_skipped_reason: null,
        compose_state: 'pending',
        compose_tipped: 0,
        created_at: ISO,
        scan_blocked: 0,
        scan_proposed: 1,
        scan_scanned: 2,
        scan_skipped: 0,
        scan_skipped_reason: null,
        scan_state: 'ran',
        scheduled_at: '2026-07-04T12:34:00.000Z',
        tick_ref: 'receipt.artanis_responder.tick.contract',
        updated_at: ISO,
      }
    case 'artanis_admin_tick_decisions':
      return {
        action_json: '{"kind":"no_action"}',
        assignment_ref: null,
        created_at: ISO,
        id: 'admin-decision-contract-1',
        state: 'no_action',
      }
    case 'artanis_closeout_verdicts':
      return {
        accept_state: 'accepted',
        assignment_ref: 'assignment.artanis_admin.contract.1',
        claimed_trace_digest_prefix: 'abcd1234abcd1234',
        created_at: ISO,
        detail: 'contract sample',
        id: 'verdict-contract-1',
        outcome: 'verified',
      }
    case 'artanis_fleet_overseer_decisions':
      return {
        action_json: '{"kind":"report_only"}',
        approval_gate_ref: null,
        context_json: '{}',
        created_at: ISO,
        health_snapshot_ref: null,
        id: 'fleet-decision-contract-1',
        state: 'no_action',
      }
    case 'artanis_standing_spend_grants':
      return {
        active: 1,
        authority_ref: 'authority.owner.contract',
        created_at: ISO,
        grant_ref: 'grant-contract-1',
        per_day_cap_sat: 10_000,
        per_payout_cap_sat: 1_000,
        revoked_at: null,
      }
    case 'artanis_spend_decisions':
      return {
        created_at: ISO,
        destination_source_ref: 'source.tip_recipient.contract',
        grant_ref: 'grant-contract-1',
        id: 'spend-contract-1',
        intended_amount_sat: 500,
        paid_amount_sat: 500,
        payment_ref: 'payment-contract-1',
        policy_applied: null,
        rationale: 'contract sample',
        recipient_ref: 'recipient-contract-1',
        state: 'paid',
        updated_at: ISO,
      }
    case 'artanis_labor_unattended_receipts':
      return {
        created_at: ISO,
        receipt_ref: 'receipt.artanis_labor.contract.1',
        serialized_json: '{}',
        terminal_state: 'settled',
      }
    case 'artanis_owner_memory':
      return {
        body: 'contract sample memory',
        created_at: ISO,
        kind: 'note',
        memory_ref: 'artanis_memory:contract-1',
        note_category: 'fact',
        owner_id: 'owner-contract',
        role: null,
      }
    case 'artanis_threads':
      return {
        caller_id: 'owner-contract',
        caller_kind: 'owner',
        created_at: ISO,
        last_message_at: ISO,
        metadata_json: '{}',
        source_ref: null,
        status: 'open',
        subject_agent_kind: 'artanis',
        subject_agent_ref: 'artanis',
        thread_ref: 'artanis_thread:contract-1',
        title: 'Contract thread',
        updated_at: ISO,
      }
    case 'artanis_messages':
      return {
        author_id: 'owner-contract',
        author_kind: 'owner',
        body: 'contract sample message',
        caller_id: 'owner-contract',
        created_at: ISO,
        message_ref: 'artanis_message:contract-1',
        metadata_json: '{}',
        thread_ref: 'artanis_thread:contract-1',
      }
  }
}

/** Normalize a row to the registry column set for cross-engine equality. */
const canonical = (
  table: ArtanisDomainTable,
  row: ArtanisDomainRow,
): Record<string, string | null> =>
  Object.fromEntries(
    ARTANIS_DOMAIN_TABLES[table].columns.map(column => {
      const value = row[column]
      return [column, value === null || value === undefined ? null : String(value)]
    }),
  )

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

type PgClient = {
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    text: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}

const MIGRATION_0011 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0011_artanis_domain.sql',
)

type LoggedDiagnostic = readonly [
  ArtanisDomainDiagnosticEvent,
  ArtanisDomainDiagnostic,
]

describe.skipIf(!hasLocalPostgres())(
  'artanis domain repository contract — D1 authority + Postgres mirror',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: SqliteD1
    let postgresStore: PostgresArtanisDomainStore
    let handle: ArtanisDomainHandle
    let diagnostics: Array<LoggedDiagnostic>

    const pgRow = async (
      table: ArtanisDomainTable,
      keyColumn: string,
      key: string | number,
    ): Promise<ArtanisDomainRow | undefined> => {
      const rows = await client!.unsafe(
        `SELECT * FROM ${table} WHERE ${keyColumn} = $1`,
        [key],
      )
      return rows[0]
    }

    const d1Row = async (
      table: ArtanisDomainTable,
      keyColumn: string,
      key: string | number,
    ): Promise<ArtanisDomainRow | undefined> => {
      const row = await sqlite.db
        .prepare(`SELECT * FROM ${table} WHERE ${keyColumn} = ?`)
        .bind(key)
        .first<ArtanisDomainRow>()
      return row ?? undefined
    }

    const expectConverged = async (
      table: ArtanisDomainTable,
      keyColumn: string,
      key: string | number,
    ): Promise<void> => {
      const d1 = await d1Row(table, keyColumn, key)
      const postgres = await pgRow(table, keyColumn, key)
      expect(d1, `${table} D1 row ${String(key)}`).toBeDefined()
      expect(postgres, `${table} Postgres row ${String(key)}`).toBeDefined()
      expect(canonical(table, postgres!)).toEqual(canonical(table, d1!))
    }

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE artanis_domain_contract')
      await admin.end({ timeout: 5 })

      const raw = postgres(pg.urlFor('artanis_domain_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await client.unsafe(readFileSync(MIGRATION_0011, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(ARTANIS_DOMAIN_D1_SCHEMA)

      postgresStore = makePostgresArtanisDomainStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: client as never,
          }),
      })
      diagnostics = []
      handle = makeArtanisDomainHandle({
        d1: sqlite.db,
        flags: { dualWrite: true, reads: 'd1' },
        log: (event, fields) => diagnostics.push([event, fields]),
        postgres: postgresStore,
        wait: () => Promise.resolve(),
      })
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    test('registry fidelity: every table mirrors a D1 row into an equal Postgres twin', async () => {
      for (const table of ALL_TABLES) {
        const spec = ARTANIS_DOMAIN_TABLES[table]
        const row = sampleRow(table)
        const columns = spec.columns
        await sqlite.db
          .prepare(
            `INSERT INTO ${table} (${columns.join(', ')}) VALUES (${columns
              .map(() => '?')
              .join(', ')})`,
          )
          .bind(...columns.map(column => row[column] ?? null))
          .run()

        const key = row[spec.conflictKey] as string | number
        await mirrorArtanisRows(handle, table, spec.conflictKey, [key])
        await expectConverged(table, spec.conflictKey, key)
      }
      expect(diagnostics).toEqual([])
    })

    test('mirror idempotency: re-mirroring an unchanged row is a no-op', async () => {
      const table = 'artanis_owner_memory'
      const before = await pgRow(table, 'memory_ref', 'artanis_memory:contract-1')
      await mirrorArtanisRows(handle, table, 'memory_ref', [
        'artanis_memory:contract-1',
      ])
      const after = await pgRow(table, 'memory_ref', 'artanis_memory:contract-1')
      expect(after).toEqual(before)
      expect(diagnostics).toEqual([])
    })

    test('cron tick double-fire is a no-op: one identical responder-tick row on both engines', async () => {
      const nowIso = '2026-07-04T13:00:00.000Z'
      const outcome = {
        blocked: 0,
        dailyBudgetLeft: 17,
        proposed: 1,
        scanned: 4,
        skipped: 3,
        skippedReason: null,
      }
      await recordArtanisResponderScanTick(handle, { nowIso, outcome })
      await recordArtanisResponderScanTick(handle, { nowIso, outcome })

      const d1Count = await sqlite.db
        .prepare(
          `SELECT COUNT(*) AS n FROM artanis_responder_ticks WHERE scheduled_at = ?`,
        )
        .bind(nowIso)
        .first<{ n: number }>()
      expect(d1Count?.n).toBe(1)
      const pgCount = await client!.unsafe(
        `SELECT COUNT(*)::int AS n FROM artanis_responder_ticks WHERE scheduled_at = $1`,
        [nowIso],
      )
      expect(Number(pgCount[0]?.['n'])).toBe(1)
      await expectConverged('artanis_responder_ticks', 'scheduled_at', nowIso)
    })

    test('mutation convergence: a D1 state transition re-converges the full Postgres row', async () => {
      await sqlite.db
        .prepare(
          `UPDATE artanis_responder_actions
              SET state = 'responded', reply_post_id = 'post-reply-1',
                  replied_at = ?, updated_at = ?
            WHERE id = 'action-contract-1'`,
        )
        .bind('2026-07-04T13:05:00.000Z', '2026-07-04T13:05:00.000Z')
        .run()
      await mirrorArtanisRows(handle, 'artanis_responder_actions', 'id', [
        'action-contract-1',
      ])
      await expectConverged(
        'artanis_responder_actions',
        'topic_id',
        'topic-contract-1',
      )
      const postgres = await pgRow(
        'artanis_responder_actions',
        'topic_id',
        'topic-contract-1',
      )
      expect(postgres?.['state']).toBe('responded')
      expect(postgres?.['reply_post_id']).toBe('post-reply-1')
    })

    test('persistence tick lifecycle dual-writes: save + idempotent replay + closeout converge', async () => {
      const tick = exampleArtanisLoopLedger().loops[0]!.ticks[0]!
      const nowIso = tick.updatedAtIso

      const first = await Effect.runPromise(
        saveArtanisLoopTick(handle, tick, nowIso),
      )
      expect(first.idempotent).toBe(false)
      await expectConverged('artanis_loop_ticks', 'record_ref', tick.tickRef)

      // Same tick again (double-fire): idempotent retry, rows unchanged.
      const second = await Effect.runPromise(
        saveArtanisLoopTick(handle, tick, nowIso),
      )
      expect(second.idempotent).toBe(true)
      await expectConverged('artanis_loop_ticks', 'record_ref', tick.tickRef)

      const closed = await Effect.runPromise(
        closeArtanisPersistedLoopTick(handle, tick.tickRef, {
          closedAtIso: '2026-07-04T13:10:00.000Z',
          closeoutReceiptRefs: ['receipt.public.artanis.contract.closeout'],
          state: 'completed',
          updatedAtIso: '2026-07-04T13:10:00.000Z',
        }),
      )
      expect(closed.state).toBe('closed')
      await expectConverged('artanis_loop_ticks', 'record_ref', tick.tickRef)
      const postgres = await pgRow(
        'artanis_loop_ticks',
        'record_ref',
        tick.tickRef,
      )
      expect(postgres?.['closed_at']).toBe('2026-07-04T13:10:00.000Z')
      expect(diagnostics).toEqual([])
    })

    test('read equivalence: persistence reads decode identically from D1 and Postgres', async () => {
      const tick = exampleArtanisLoopLedger().loops[0]!.ticks[0]!
      const postgresReadsHandle = makeArtanisDomainHandle({
        d1: sqlite.db,
        flags: { dualWrite: true, reads: 'postgres' },
        log: (event, fields) => diagnostics.push([event, fields]),
        postgres: postgresStore,
        wait: () => Promise.resolve(),
      })

      const fromD1 = await Effect.runPromise(
        readArtanisPersistedRecord(handle, 'loop_tick', tick.tickRef),
      )
      const fromPostgres = await Effect.runPromise(
        readArtanisPersistedRecord(
          postgresReadsHandle,
          'loop_tick',
          tick.tickRef,
        ),
      )
      expect(fromPostgres).toEqual(fromD1)
      expect(fromD1).not.toBeNull()

      const latestD1 = await Effect.runPromise(
        readLatestArtanisPersistedRows(handle, 'loop_tick', 10),
      )
      const latestPostgres = await Effect.runPromise(
        readLatestArtanisPersistedRows(postgresReadsHandle, 'loop_tick', 10),
      )
      expect(latestPostgres).toEqual(latestD1)
      // No postgres-read failures or fallbacks fired.
      expect(diagnostics).toEqual([])
    })

    test('compare mode serves D1 and stays silent when the engines agree', async () => {
      const tick = exampleArtanisLoopLedger().loops[0]!.ticks[0]!
      const compareHandle = makeArtanisDomainHandle({
        d1: sqlite.db,
        flags: { dualWrite: true, reads: 'compare' },
        log: (event, fields) => diagnostics.push([event, fields]),
        postgres: postgresStore,
        wait: () => Promise.resolve(),
      })
      const record = await Effect.runPromise(
        readArtanisPersistedRecord(compareHandle, 'loop_tick', tick.tickRef),
      )
      expect(record).not.toBeNull()
      expect(
        diagnostics.filter(
          ([event]) => event === 'khala_sync_artanis_read_compare_mismatch',
        ),
      ).toEqual([])
    })

    // -------------------------------------------------------------------
    // #8409 regression: two independent-writer columns on the SAME natural
    // key must both survive a Postgres mirror race, no matter which
    // writer's D1-read-back + Postgres-upsert round trip lands last.
    // -------------------------------------------------------------------

    test('#8409 regression: interleaved scan/compose tick mirrors do not clobber each other\'s columns', async () => {
      const nowIso = '2026-07-05T06:08:32.000Z' // the exact production-incident scheduled_at from #8409
      const scanOutcome = {
        blocked: 0,
        dailyBudgetLeft: 10,
        proposed: 1,
        scanned: 4,
        skipped: 0,
        skippedReason: null,
      }
      const composeOutcome = {
        blocked: 0,
        considered: 1,
        responded: 1,
        skippedReason: null,
        tipped: 0,
      }

      // 1. The scan tick writes its columns to D1 (scan_state='ran'), then
      //    mirrors. Capture the STALE full-row D1 snapshot right here — this
      //    is the exact snapshot a delayed scan-mirror Postgres round trip
      //    would carry (taken BEFORE the compose tick's D1 write lands).
      await recordArtanisResponderScanTick(handle, {
        nowIso,
        outcome: scanOutcome,
      })
      const staleSnapshotAfterScan = await d1Row(
        'artanis_responder_ticks',
        'scheduled_at',
        nowIso,
      )
      expect(staleSnapshotAfterScan?.['scan_state']).toBe('ran')
      expect(staleSnapshotAfterScan?.['compose_state']).toBe('pending')

      // 2. The compose tick writes its columns to D1 for the SAME
      //    scheduled_at (compose_state='ran') and its OWN mirror lands
      //    normally (a fresh, in-order read-back + scoped upsert).
      await recordArtanisResponderComposeTick(handle, {
        nowIso,
        outcome: composeOutcome,
      })
      await expectConverged('artanis_responder_ticks', 'scheduled_at', nowIso)
      const afterComposeMirror = await pgRow(
        'artanis_responder_ticks',
        'scheduled_at',
        nowIso,
      )
      expect(afterComposeMirror?.['scan_state']).toBe('ran')
      expect(afterComposeMirror?.['compose_state']).toBe('ran')

      // 3. Now simulate the STALE scan mirror's delayed Postgres round trip
      //    finally landing — AFTER compose's fresher upsert. With the
      //    #8409 fix (column-scoped `ON CONFLICT DO UPDATE`, using the
      //    exact production SCAN_TICK_UPDATE_COLUMNS scope), this must NOT
      //    revert compose_state back to the stale snapshot's 'pending'.
      await postgresStore.upsertRows(
        'artanis_responder_ticks',
        [staleSnapshotAfterScan!],
        SCAN_TICK_UPDATE_COLUMNS,
      )
      const finalPostgres = await pgRow(
        'artanis_responder_ticks',
        'scheduled_at',
        nowIso,
      )
      expect(finalPostgres?.['scan_state']).toBe('ran')
      expect(finalPostgres?.['compose_state']).toBe('ran')
      expect(Number(finalPostgres?.['compose_responded'])).toBe(1)
      await expectConverged('artanis_responder_ticks', 'scheduled_at', nowIso)

      // 4. Prove this is a genuine regression guard, not vacuously true: the
      //    OLD full-row upsert (no column scope — the pre-fix behavior)
      //    WOULD have clobbered compose_state back to the stale snapshot.
      await postgresStore.upsertRows('artanis_responder_ticks', [
        staleSnapshotAfterScan!,
      ])
      const clobbered = await pgRow(
        'artanis_responder_ticks',
        'scheduled_at',
        nowIso,
      )
      expect(clobbered?.['compose_state']).toBe('pending')

      // Restore convergence for later tests sharing this Postgres database.
      await mirrorArtanisRows(handle, 'artanis_responder_ticks', 'scheduled_at', [
        nowIso,
      ])
      await expectConverged('artanis_responder_ticks', 'scheduled_at', nowIso)
    })

    test('#8409 regression: interleaved responder_state scan/compose column updates do not clobber each other', async () => {
      // artanis_responder_state (the id=1 singleton) has the SAME
      // two-independent-writer shape: the scan stage owns scan_cursor_iso,
      // the compose stage owns responses_today/responses_day, both racing
      // on one row every minute.
      const table = 'artanis_responder_state' as const
      const before = await pgRow(table, 'id', 1)
      expect(before).toBeDefined()

      // Stale snapshot as scan currently sees it (before compose's write).
      const staleScanSnapshot = { ...before! }

      // Compose's D1 write + fresh in-order mirror.
      await sqlite.db
        .prepare(
          `UPDATE artanis_responder_state
             SET responses_today = 9, responses_day = ?, updated_at = ?
           WHERE id = 1`,
        )
        .bind('2026-07-05', '2026-07-05T06:09:00.000Z')
        .run()
      await mirrorArtanisRows(
        handle,
        table,
        'id',
        [1],
        RESPONDER_STATE_COMPOSE_UPDATE_COLUMNS,
      )
      const afterCompose = await pgRow(table, 'id', 1)
      expect(Number(afterCompose?.['responses_today'])).toBe(9)

      // The stale scan mirror finally lands, scoped to scan-owned columns
      // only — must not revert responses_today/responses_day.
      await postgresStore.upsertRows(
        table,
        [staleScanSnapshot],
        RESPONDER_STATE_SCAN_UPDATE_COLUMNS,
      )
      const final = await pgRow(table, 'id', 1)
      expect(Number(final?.['responses_today'])).toBe(9)
      expect(final?.['scan_cursor_iso']).toBe(staleScanSnapshot['scan_cursor_iso'])

      // Regression guard: the OLD full-row upsert would have clobbered it.
      await postgresStore.upsertRows(table, [staleScanSnapshot])
      const clobbered = await pgRow(table, 'id', 1)
      expect(Number(clobbered?.['responses_today'])).not.toBe(9)

      // Restore convergence for later tests.
      await mirrorArtanisRows(handle, table, 'id', [1])
      await expectConverged(table, 'id', 1)
    })
  },
)
