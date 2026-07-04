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

/**
 * The D1 DDL for the KS-8.2 token ledger domain (worker migrations
 * 0137/0138/0232/0262/0264/0265/0269, condensed to the live column set).
 */
export const TOKEN_LEDGER_D1_SCHEMA = `
CREATE TABLE token_usage_events (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  observed_at TEXT NOT NULL,
  ingested_at TEXT NOT NULL,
  producer_system TEXT NOT NULL,
  source_route TEXT NOT NULL,
  actor_user_id TEXT,
  actor_team_id TEXT,
  account_ref TEXT,
  anonymized_source_ref TEXT,
  run_ref TEXT,
  session_ref TEXT,
  task_ref TEXT,
  repository_ref TEXT,
  provider TEXT,
  model TEXT,
  backend_profile TEXT,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  reasoning_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_5m_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_1h_tokens INTEGER NOT NULL DEFAULT 0,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  usage_truth TEXT NOT NULL,
  cost_amount REAL,
  currency TEXT,
  leaderboard_eligible INTEGER NOT NULL DEFAULT 1,
  privacy_opt_out INTEGER NOT NULL DEFAULT 0,
  safe_metadata_json TEXT NOT NULL DEFAULT '{}',
  demand_kind TEXT NOT NULL DEFAULT 'unlabeled',
  demand_source TEXT,
  demand_client TEXT,
  demand_channel TEXT NOT NULL DEFAULT 'khala_api',
  role_ref TEXT
);

CREATE TABLE public_khala_tokens_served_daily_rollups (
  timezone TEXT NOT NULL,
  day TEXT NOT NULL,
  tokens_served INTEGER NOT NULL DEFAULT 0,
  usage_events INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (timezone, day)
);

CREATE TABLE public_khala_tokens_served_model_daily_rollups (
  day TEXT NOT NULL,
  provider TEXT NOT NULL DEFAULT '',
  model TEXT NOT NULL DEFAULT '',
  tokens_served INTEGER NOT NULL DEFAULT 0,
  usage_events INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (day, provider, model)
);

CREATE TABLE public_khala_tokens_served_channel_daily_rollups (
  day TEXT NOT NULL,
  demand_channel TEXT NOT NULL DEFAULT 'khala_api',
  tokens_served INTEGER NOT NULL DEFAULT 0,
  usage_events INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (day, demand_channel)
);

CREATE TABLE token_usage_leaderboard_preferences (
  subject_kind TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  leaderboard_participation TEXT NOT NULL DEFAULT 'eligible',
  leaderboard_visibility TEXT NOT NULL DEFAULT 'internal',
  updated_at TEXT NOT NULL,
  updated_by_user_id TEXT,
  PRIMARY KEY (subject_kind, subject_ref)
);
`

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

/**
 * The D1 DDL for the KS-8.5 agent runtime metadata domain (worker
 * migrations 0019/0022/0023/0027/0028/0029/0228/0229/0230/0236/0279/0280/
 * 0281/0282/0284, condensed to the live column set — FKs to users/teams
 * dropped so the contract suite seeds rows directly).
 */
export const AGENT_RUNTIME_D1_SCHEMA = `
CREATE TABLE agent_definitions (
  id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  schema_literal TEXT NOT NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  goal TEXT NOT NULL,
  harness_json TEXT NOT NULL,
  toolset_json TEXT NOT NULL,
  triggers_json TEXT NOT NULL,
  lane TEXT NOT NULL,
  budget_json TEXT NOT NULL,
  escalation_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  definition_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE(owner_agent_user_id, slug)
);

CREATE TABLE agent_definition_runs (
  run_id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  definition_id TEXT NOT NULL,
  definition_ref TEXT NOT NULL,
  trigger_ref TEXT NOT NULL,
  lane TEXT NOT NULL,
  status TEXT NOT NULL,
  pylon_ref TEXT,
  assignment_ref TEXT,
  durable_request_id TEXT NOT NULL,
  durable_stream_url TEXT,
  forge_tenant_ref TEXT NOT NULL,
  forge_work_ref TEXT NOT NULL,
  forge_repository_ref TEXT,
  forge_git_token_refs_json TEXT NOT NULL DEFAULT '[]',
  refusal_error TEXT,
  refusal_reason TEXT,
  evidence_refs_json TEXT NOT NULL,
  trigger_payload_json TEXT NOT NULL,
  runtime_run_json TEXT NOT NULL,
  initial_events_json TEXT NOT NULL,
  budget_credits_reserved REAL NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE agent_definition_triggers (
  trigger_id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  definition_id TEXT NOT NULL,
  trigger_ref TEXT NOT NULL,
  trigger_kind TEXT NOT NULL,
  trigger_json TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('enabled', 'paused')),
  consecutive_failures INTEGER NOT NULL DEFAULT 0,
  next_run_at TEXT,
  paused_at TEXT,
  pause_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(owner_agent_user_id, trigger_ref)
);

CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  project_id TEXT,
  runtime TEXT NOT NULL,
  backend TEXT NOT NULL,
  runner_id TEXT NOT NULL,
  assignment_kind TEXT NOT NULL,
  repository_provider TEXT NOT NULL,
  repository_owner TEXT NOT NULL,
  repository_repo TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  goal TEXT NOT NULL,
  goal_id TEXT,
  provider_account_ref TEXT,
  auth_grant_ref TEXT,
  external_run_id TEXT,
  status TEXT NOT NULL,
  event_cursor INTEGER NOT NULL DEFAULT 0,
  assignment_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  failed_at TEXT,
  canceled_at TEXT,
  archived_at TEXT
);

CREATE TABLE agent_run_events (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  type TEXT NOT NULL,
  summary TEXT NOT NULL,
  status TEXT,
  source TEXT NOT NULL,
  payload_json TEXT,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  external_event_id TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (run_id, sequence),
  UNIQUE (run_id, external_event_id)
);

CREATE TABLE agent_traces (
  trace_uuid TEXT PRIMARY KEY,
  owner_user_id TEXT NOT NULL,
  agent_ref TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  trajectory_id TEXT NOT NULL,
  session_id TEXT,
  visibility TEXT NOT NULL DEFAULT 'unlisted',
  step_count INTEGER NOT NULL DEFAULT 0,
  trajectory_json TEXT NOT NULL DEFAULT '{}',
  trajectory_r2_key TEXT,
  blob_refs_json TEXT NOT NULL DEFAULT '[]',
  idempotency_key TEXT,
  training_consent INTEGER NOT NULL DEFAULT 0,
  license TEXT,
  content_digest TEXT,
  reward_eligible INTEGER NOT NULL DEFAULT 0,
  reward_amount_sats INTEGER,
  upload_source TEXT NOT NULL DEFAULT 'agent',
  demand_kind TEXT,
  demand_source TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_agent_traces_idempotency
  ON agent_traces (owner_user_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE UNIQUE INDEX idx_agent_traces_owner_digest
  ON agent_traces (owner_user_id, content_digest)
  WHERE content_digest IS NOT NULL;

CREATE TABLE agent_goals (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  user_id TEXT,
  team_id TEXT,
  project_id TEXT,
  objective TEXT NOT NULL,
  status TEXT NOT NULL,
  visibility TEXT NOT NULL,
  current_run_id TEXT,
  token_budget INTEGER,
  tokens_used INTEGER NOT NULL DEFAULT 0,
  time_used_seconds INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  paused_at TEXT,
  blocked_at TEXT,
  archived_at TEXT
);

CREATE UNIQUE INDEX agent_goals_current_scope_idx
  ON agent_goals(
    agent_id,
    COALESCE(user_id, ''),
    COALESCE(team_id, ''),
    COALESCE(project_id, '')
  )
  WHERE archived_at IS NULL;

CREATE TABLE agent_goal_events (
  id TEXT PRIMARY KEY,
  goal_id TEXT NOT NULL,
  run_id TEXT,
  expected_goal_id TEXT,
  caller_type TEXT NOT NULL,
  event_type TEXT NOT NULL,
  status TEXT,
  token_delta INTEGER NOT NULL DEFAULT 0,
  time_delta_seconds INTEGER NOT NULL DEFAULT 0,
  payload_json TEXT,
  external_event_id TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_agent_goal_events_goal_external_event
  ON agent_goal_events(goal_id, external_event_id)
  WHERE external_event_id IS NOT NULL;
`
