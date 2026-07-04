// Real-SQL D1 test double over `node:sqlite` (KS-8.1, #8307).
//
// The pylon dispatch repository contract suite runs `makeD1PylonApiStore`
// against REAL SQLite (the engine D1 is built on) instead of hand-rolled
// row fakes, so the D1 side of the contract exercises the store's actual
// SQL. Only the D1Database surface the pylon store uses is implemented:
// `prepare().bind().first/all/run` and sequential `batch`.

import { DatabaseSync } from 'node:sqlite'

type SqlParam = null | number | bigint | string | Uint8Array

const asParams = (params: ReadonlyArray<unknown>): Array<SqlParam> =>
  params.map(value => {
    if (
      value === null ||
      typeof value === 'number' ||
      typeof value === 'bigint' ||
      typeof value === 'string' ||
      value instanceof Uint8Array
    ) {
      return value
    }
    if (value === undefined) {
      return null
    }
    throw new TypeError(
      `unsupported sqlite parameter of type ${typeof value} in sqlite D1 test double`,
    )
  })

class SqliteD1PreparedStatement {
  constructor(
    private readonly db: DatabaseSync,
    private readonly query: string,
    private readonly params: ReadonlyArray<unknown> = [],
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1PreparedStatement {
    return new SqliteD1PreparedStatement(this.db, this.query, values)
  }

  first<T>(): Promise<T | null> {
    const row = this.db.prepare(this.query).get(...asParams(this.params))
    return Promise.resolve(row === undefined ? null : (row as T))
  }

  all<T>(): Promise<{
    results: Array<T>
    success: true
    meta: Record<string, unknown>
  }> {
    const rows = this.db.prepare(this.query).all(...asParams(this.params))
    return Promise.resolve({
      meta: {},
      results: rows as Array<T>,
      success: true,
    })
  }

  run(): Promise<{ success: true; meta: { changes: number } }> {
    const result = this.db.prepare(this.query).run(...asParams(this.params))
    return Promise.resolve({
      meta: { changes: Number(result.changes) },
      success: true,
    })
  }

  raw(): Promise<Array<Array<unknown>>> {
    throw new Error('raw() is not implemented in the sqlite D1 test double')
  }
}

export type SqliteD1 = Readonly<{
  db: D1Database
  exec: (sql: string) => void
  close: () => void
}>

/** An in-memory D1Database backed by real SQLite. */
export const makeSqliteD1 = (): SqliteD1 => {
  const sqlite = new DatabaseSync(':memory:')

  const d1 = {
    batch: async (statements: ReadonlyArray<SqliteD1PreparedStatement>) => {
      const results = []
      for (const statement of statements) {
        results.push(await statement.run())
      }
      return results
    },
    dump: () => {
      throw new Error('dump() is not implemented in the sqlite D1 test double')
    },
    exec: (sql: string) => {
      sqlite.exec(sql)
      return Promise.resolve({ count: 0, duration: 0 })
    },
    prepare: (query: string) => new SqliteD1PreparedStatement(sqlite, query),
    withSession: () => {
      throw new Error(
        'withSession() is not implemented in the sqlite D1 test double',
      )
    },
  }

  return {
    close: () => sqlite.close(),
    db: d1 as unknown as D1Database,
    exec: sql => sqlite.exec(sql),
  }
}

/** The D1 DDL for the KS-8.1 domain tables (worker migrations, condensed). */
export const PYLON_DISPATCH_D1_SCHEMA = `
CREATE TABLE pylon_api_registrations (
  id TEXT PRIMARY KEY,
  pylon_ref TEXT NOT NULL UNIQUE,
  owner_agent_user_id TEXT NOT NULL,
  owner_agent_credential_id TEXT NOT NULL,
  owner_agent_token_prefix TEXT NOT NULL,
  display_name TEXT NOT NULL,
  status TEXT NOT NULL,
  resource_mode TEXT NOT NULL,
  capability_refs_json TEXT NOT NULL,
  client_version TEXT,
  client_protocol_version TEXT,
  wallet_ref TEXT,
  wallet_ready INTEGER NOT NULL DEFAULT 0,
  latest_heartbeat_at TEXT,
  latest_heartbeat_status TEXT,
  latest_resource_mode TEXT,
  latest_health_refs_json TEXT NOT NULL DEFAULT '[]',
  latest_load_refs_json TEXT NOT NULL DEFAULT '[]',
  latest_capacity_refs_json TEXT NOT NULL DEFAULT '[]',
  provider_nostr_pubkey TEXT,
  provider_nostr_npub TEXT,
  provider_market_relay_refs_json TEXT NOT NULL DEFAULT '[]',
  provider_nip90_lane_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE pylon_api_assignments (
  id TEXT PRIMARY KEY,
  assignment_ref TEXT NOT NULL UNIQUE,
  pylon_ref TEXT NOT NULL,
  owner_agent_user_id TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  job_kind TEXT NOT NULL,
  state TEXT NOT NULL,
  payment_mode TEXT NOT NULL DEFAULT 'unpaid_smoke',
  lease_expires_at TEXT NOT NULL,
  task_refs_json TEXT NOT NULL,
  acceptance_criteria_refs_json TEXT NOT NULL,
  result_expectation_refs_json TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL,
  proof_refs_json TEXT NOT NULL,
  accepted_work_refs_json TEXT NOT NULL,
  rejection_refs_json TEXT NOT NULL,
  closeout_refs_json TEXT NOT NULL,
  coding_assignment_json TEXT,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref)
);

CREATE TABLE pylon_api_events (
  id TEXT PRIMARY KEY,
  event_ref TEXT NOT NULL UNIQUE,
  pylon_ref TEXT NOT NULL,
  owner_agent_user_id TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  event_kind TEXT NOT NULL,
  assignment_ref TEXT,
  status TEXT NOT NULL,
  event_body_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref)
);

CREATE TABLE pylon_provider_job_lifecycle (
  id TEXT PRIMARY KEY,
  pylon_ref TEXT NOT NULL,
  assignment_ref TEXT NOT NULL UNIQUE,
  owner_agent_user_id TEXT NOT NULL,
  job_kind TEXT NOT NULL,
  stage TEXT NOT NULL,
  task_refs_json TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL,
  proof_refs_json TEXT NOT NULL,
  closeout_refs_json TEXT NOT NULL,
  accepted_work_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref),
  FOREIGN KEY (assignment_ref) REFERENCES pylon_api_assignments(assignment_ref)
);
`
