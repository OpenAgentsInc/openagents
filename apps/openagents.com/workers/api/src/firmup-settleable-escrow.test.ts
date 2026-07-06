import { DatabaseSync } from 'node:sqlite'

import { describe, expect, it } from 'vitest'

import { readFirmupSettleableEscrow } from './firmup-settleable-escrow'
import { paymentsLedgerDbFromD1, type D1LikeDatabase } from './test/payments-ledger-sqlite'
import { reserveLaborEscrowStatements } from './labor-escrow'
import { runLedgerStatements } from './payments-ledger'

// Minimal real-SQL D1 adapter backed by node:sqlite (same pattern as the
// negotiation-live harness). We exercise the resolver against genuine SQL so the
// reserved-escrow + accepted-offer fail-closed gates are real, not modeled.
type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
    return { results }
  }

  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

/**
 * Proper-runner test for the firm-up settleable-escrow resolver (#5459) against
 * genuine SQL. It proves the resolver is the fail-closed SOURCE OF TRUTH: it
 * settles only a real, reserved firm-up escrow that has both a work request
 * (verification command) and an accepted offer (provider actor).
 */

const SCHEMA = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0,
  held_msat INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE labor_escrows (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL,
  state TEXT NOT NULL,
  funding_source TEXT NOT NULL,
  job_event_id TEXT NOT NULL,
  acceptance_event_ref TEXT,
  reserve_receipt_ref TEXT NOT NULL,
  release_receipt_ref TEXT UNIQUE,
  refund_receipt_ref TEXT UNIQUE,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  released_at TEXT,
  refunded_at TEXT,
  archived_at TEXT
);
CREATE TABLE labor_escrow_receipts (
  id TEXT PRIMARY KEY,
  escrow_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  transition_kind TEXT NOT NULL,
  work_request_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL,
  receipt_ref TEXT NOT NULL,
  evidence_ref TEXT,
  state_after TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE forum_work_requests (
  id TEXT PRIMARY KEY,
  verification_command_ref TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE forum_work_request_acceptances (
  id TEXT PRIMARY KEY,
  work_request_id TEXT NOT NULL,
  provider_actor_ref TEXT NOT NULL,
  archived_at TEXT
);
`

const WORK_REQUEST_ID = 'wr_firmup_5459'
const ESCROW_ID = 'escrow_5459'
const REQUESTER_REF = 'pylon.public.requester.artanis'
const PROVIDER_REF = 'pylon.public.worker.orrery'
const VERIFICATION_COMMAND_REF = 'command.public.firmup.5459.bun_test'

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const seedReservedEscrow = async (db: D1Database): Promise<void> => {
  const ledgerDb = paymentsLedgerDbFromD1(db as unknown as D1LikeDatabase)
  await db
    .prepare(
      `INSERT INTO agent_balances (actor_ref, balance_msat, held_msat, created_at, updated_at)
       VALUES (?, ?, 0, ?, ?)`,
    )
    .bind(REQUESTER_REF, 1_000_000, '2026-06-18T10:00:00.000Z', '2026-06-18T10:00:00.000Z')
    .run()

  await runLedgerStatements(
    ledgerDb,
    reserveLaborEscrowStatements({
      amountMsat: 50_000,
      escrowId: ESCROW_ID,
      fundingSource: { kind: 'ledger_balance' },
      idempotencyKey: 'idem.firmup.5459',
      jobEventId: 'jobevent_5459',
      nowIso: '2026-06-18T10:00:00.000Z',
      requesterActorRef: REQUESTER_REF,
      reserveReceiptId: 'reserve_receipt_5459',
      reserveReceiptRef: 'receipt.labor_escrow.reserve.5459',
      workRequestId: WORK_REQUEST_ID,
    }),
  )

  await db
    .prepare(
      `INSERT INTO forum_work_requests (id, verification_command_ref) VALUES (?, ?)`,
    )
    .bind(WORK_REQUEST_ID, VERIFICATION_COMMAND_REF)
    .run()
}

const seedAcceptance = async (db: D1Database): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO forum_work_request_acceptances (id, work_request_id, provider_actor_ref)
       VALUES (?, ?, ?)`,
    )
    .bind('acc_5459', WORK_REQUEST_ID, PROVIDER_REF)
    .run()
}

describe('readFirmupSettleableEscrow (#5459, real SQL)', () => {
  it('resolves a reserved escrow with a work request + accepted offer', async () => {
    const db = makeDb()
    await seedReservedEscrow(db)
    await seedAcceptance(db)

    const projection = await readFirmupSettleableEscrow(
      { db, ledgerDb: paymentsLedgerDbFromD1(db as unknown as D1LikeDatabase) },
      `labor_escrow.public.${ESCROW_ID}`,
    )

    expect(projection).toBeDefined()
    expect(projection!.amountSats).toBe(50)
    expect(projection!.providerActorRef).toBe(PROVIDER_REF)
    expect(projection!.verificationCommandRef).toBe(VERIFICATION_COMMAND_REF)
    expect(projection!.escrowRef).toBe(`labor_escrow.public.${ESCROW_ID}`)
    expect(projection!.workRequestRef).toBe(
      `work_request.public.${WORK_REQUEST_ID}`,
    )
  })

  it('fails closed when there is no accepted offer (job was never firmed up)', async () => {
    const db = makeDb()
    await seedReservedEscrow(db)
    // No acceptance row.

    const projection = await readFirmupSettleableEscrow(
      { db, ledgerDb: paymentsLedgerDbFromD1(db as unknown as D1LikeDatabase) },
      `labor_escrow.public.${ESCROW_ID}`,
    )

    expect(projection).toBeUndefined()
  })

  it('fails closed for an unknown escrow', async () => {
    const db = makeDb()

    const projection = await readFirmupSettleableEscrow(
      { db, ledgerDb: paymentsLedgerDbFromD1(db as unknown as D1LikeDatabase) },
      'labor_escrow.public.does_not_exist',
    )

    expect(projection).toBeUndefined()
  })
})
