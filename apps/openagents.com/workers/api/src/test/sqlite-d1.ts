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
 * The D1 DDL for the KS-8.10 forum content domain (worker migrations
 * 0101/0102/0103/0105/0110/0111/0112, condensed to the live column set —
 * FK clauses dropped so the contract suite seeds rows directly; the
 * UNIQUE dedupe keys are kept EXACTLY because they are load-bearing for
 * the INSERT OR IGNORE write paths).
 */
export const FORUM_CONTENT_D1_SCHEMA = `
CREATE TABLE forum_boards (
  id TEXT PRIMARY KEY NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  description_ref TEXT,
  visibility TEXT NOT NULL DEFAULT 'public',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_categories (
  id TEXT PRIMARY KEY NOT NULL,
  board_id TEXT NOT NULL,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  description_ref TEXT,
  order_index INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  discoverability TEXT NOT NULL DEFAULT 'listed',
  UNIQUE (board_id, slug)
);

CREATE TABLE forum_forums (
  id TEXT PRIMARY KEY NOT NULL,
  board_id TEXT NOT NULL,
  category_id TEXT NOT NULL,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  description_ref TEXT,
  visibility TEXT NOT NULL DEFAULT 'public',
  locked INTEGER NOT NULL DEFAULT 0,
  topic_count INTEGER NOT NULL DEFAULT 0,
  post_count INTEGER NOT NULL DEFAULT 0,
  latest_topic_id TEXT,
  latest_post_id TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  discoverability TEXT NOT NULL DEFAULT 'listed',
  UNIQUE (category_id, slug)
);

CREATE TABLE forum_topics (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  forum_id TEXT NOT NULL,
  actor_ref TEXT NOT NULL,
  actor_json TEXT NOT NULL DEFAULT '{}',
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  first_post_id TEXT NOT NULL,
  latest_post_id TEXT NOT NULL,
  post_count INTEGER NOT NULL DEFAULT 1,
  pin_state TEXT NOT NULL DEFAULT 'normal',
  state TEXT NOT NULL DEFAULT 'open',
  score_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (forum_id, slug)
);

CREATE TABLE forum_posts (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  topic_id TEXT NOT NULL,
  forum_id TEXT NOT NULL,
  actor_ref TEXT NOT NULL,
  actor_json TEXT NOT NULL DEFAULT '{}',
  content_ref TEXT NOT NULL,
  parent_post_id TEXT,
  quote_post_id TEXT,
  post_number INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'visible',
  revision_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (topic_id, post_number)
);

CREATE TABLE forum_post_bodies (
  post_id TEXT PRIMARY KEY NOT NULL,
  content_kind TEXT NOT NULL DEFAULT 'plain_text',
  body_text TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_post_revisions (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  post_id TEXT NOT NULL,
  actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL,
  previous_body_text TEXT,
  next_body_text TEXT,
  previous_state TEXT NOT NULL,
  next_state TEXT NOT NULL,
  reason_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_actor_follows (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  target_actor_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, target_actor_ref)
);

CREATE TABLE forum_watches (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  forum_id TEXT,
  topic_id TEXT,
  watch_kind TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, watch_kind, forum_id, topic_id)
);

CREATE TABLE forum_bookmarks (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  topic_id TEXT,
  post_id TEXT,
  bookmark_kind TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, bookmark_kind, topic_id, post_id)
);

CREATE TABLE forum_reports (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  reporter_actor_ref TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  reason_ref TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_moderation_events (
  id TEXT PRIMARY KEY NOT NULL,
  moderator_actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  reason_ref TEXT NOT NULL,
  report_id TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  idempotency_key TEXT
);

CREATE UNIQUE INDEX idx_forum_moderation_events_idempotency
  ON forum_moderation_events(idempotency_key)
  WHERE idempotency_key IS NOT NULL
    AND archived_at IS NULL;

CREATE TABLE forum_context_links (
  id TEXT PRIMARY KEY NOT NULL,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  forum_id TEXT NOT NULL,
  topic_id TEXT,
  post_id TEXT,
  context_kind TEXT NOT NULL,
  context_id TEXT NOT NULL,
  context_slug TEXT,
  context_title TEXT,
  public_url TEXT,
  source_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (target_kind, target_id, context_kind, context_id)
);
`

/**
 * The D1 DDL for the KS-8.10 remainder domain (#8338): the eleven
 * remainder forum tables (worker migrations 0101/0113/0166/0168/0179),
 * with `provider_pubkey` appended to offers as migration 0179 does. FKs
 * dropped so the contract suite seeds rows directly.
 */
export const FORUM_REMAINDER_D1_SCHEMA = `
CREATE TABLE forum_private_message_threads (
  id TEXT PRIMARY KEY NOT NULL,
  subject TEXT NOT NULL,
  slug TEXT NOT NULL,
  created_by_actor_ref TEXT NOT NULL,
  participant_refs_json TEXT NOT NULL DEFAULT '[]',
  latest_message_id TEXT,
  message_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_private_messages (
  id TEXT PRIMARY KEY NOT NULL,
  thread_id TEXT NOT NULL,
  sender_actor_ref TEXT NOT NULL,
  recipient_actor_ref TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_acl_grants (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  forum_id TEXT,
  permission TEXT NOT NULL,
  scope_ref TEXT NOT NULL,
  granted_by_actor_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  revoked_at TEXT,
  UNIQUE (actor_ref, forum_id, permission, scope_ref)
);

CREATE TABLE forum_score_snapshots (
  id TEXT PRIMARY KEY NOT NULL,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  positive_bitcoin_sats INTEGER NOT NULL DEFAULT 0,
  boost_bitcoin_sats INTEGER NOT NULL DEFAULT 0,
  down_signal_bitcoin_sats INTEGER NOT NULL DEFAULT 0,
  reply_count INTEGER NOT NULL DEFAULT 0,
  net_investment_sats INTEGER NOT NULL DEFAULT 0,
  score_ref TEXT NOT NULL,
  rebuilt_from_event_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_notification_reads (
  id TEXT PRIMARY KEY,
  actor_ref TEXT NOT NULL,
  notification_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  read_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX idx_forum_notification_reads_actor_notification
  ON forum_notification_reads(actor_ref, notification_id)
  WHERE archived_at IS NULL;

CREATE UNIQUE INDEX idx_forum_notification_reads_actor_idempotency
  ON forum_notification_reads(actor_ref, idempotency_key)
  WHERE archived_at IS NULL;

CREATE TABLE forum_work_requests (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  topic_id TEXT NOT NULL UNIQUE,
  first_post_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  title TEXT NOT NULL,
  objective_ref TEXT NOT NULL,
  verification_command_ref TEXT NOT NULL,
  repository_refs_json TEXT NOT NULL DEFAULT '[]',
  required_capability_refs_json TEXT NOT NULL DEFAULT '[]',
  budget_sats INTEGER NOT NULL,
  budget_msats INTEGER NOT NULL,
  deadline_ref TEXT NOT NULL,
  relay_url TEXT NOT NULL,
  job_event_id TEXT NOT NULL UNIQUE,
  job_event_kind INTEGER NOT NULL,
  job_result_kind INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'open',
  quote_count INTEGER NOT NULL DEFAULT 0,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_relay_links (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL UNIQUE,
  topic_id TEXT NOT NULL UNIQUE,
  job_event_id TEXT NOT NULL UNIQUE,
  job_event_kind INTEGER NOT NULL,
  relay_url TEXT NOT NULL,
  relay_ref TEXT NOT NULL,
  bridge_actor_ref TEXT NOT NULL,
  event_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_offers (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL,
  quote_ref TEXT NOT NULL UNIQUE,
  provider_actor_ref TEXT NOT NULL,
  amount_sats INTEGER NOT NULL,
  amount_msats INTEGER NOT NULL,
  capability_refs_json TEXT NOT NULL DEFAULT '[]',
  relay_event_ref TEXT,
  state TEXT NOT NULL DEFAULT 'offered',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  provider_pubkey TEXT
);

CREATE TABLE forum_work_request_lifecycle_posts (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL,
  topic_id TEXT NOT NULL,
  post_id TEXT NOT NULL UNIQUE,
  idempotency_key TEXT NOT NULL UNIQUE,
  lifecycle_kind TEXT NOT NULL,
  receipt_ref TEXT NOT NULL,
  state_after TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_acceptances (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE,
  offer_id TEXT NOT NULL,
  quote_ref TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT NOT NULL,
  amount_msats INTEGER NOT NULL,
  escrow_id TEXT NOT NULL UNIQUE,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  acceptance_event_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE forum_work_request_results (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL,
  offer_id TEXT NOT NULL,
  quote_ref TEXT NOT NULL UNIQUE,
  provider_actor_ref TEXT NOT NULL,
  result_event_ref TEXT NOT NULL,
  verification_command_ref TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  closeout_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
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

/**
 * The D1 DDL for the KS-8.7 billing/Stripe/pay-ins domain (worker
 * migrations 0016/0018/0019/0031/0052/0114/0160/0169/0170/0211/0226/0290,
 * condensed to the live column sets). FK clauses to out-of-domain tables
 * (users/teams/agent_runs/software_orders) are dropped — the contract
 * suite exercises the billing rows, not the user graph. `agent_balances`
 * is included because the pay-in statement plans move balances in the
 * same batch (it is NOT a migrating table).
 */
export const BILLING_DOMAIN_D1_SCHEMA = `
CREATE TABLE billing_accounts (
  user_id TEXT PRIMARY KEY NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE billing_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  run_id TEXT,
  source TEXT NOT NULL CHECK (
    source IN (
      'trial_grant', 'coupon', 'credit_card_placeholder', 'stripe_checkout',
      'stripe_auto_top_up', 'container_usage', 'codex_usage',
      'manual_adjustment'
    )
  ),
  description TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  quantity INTEGER,
  unit TEXT,
  unit_rate_cents INTEGER,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE billing_usage_cursors (
  run_id TEXT NOT NULL,
  meter TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  last_billed_at TEXT NOT NULL,
  total_billed_quantity INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, meter)
);

CREATE TABLE billing_coupon_redemptions (
  user_id TEXT NOT NULL,
  coupon_code TEXT NOT NULL,
  ledger_entry_id TEXT NOT NULL,
  redeemed_at TEXT NOT NULL,
  PRIMARY KEY (user_id, coupon_code)
);

CREATE TABLE billing_credit_notifications (
  user_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('out_of_credits')),
  email TEXT,
  display_name TEXT NOT NULL,
  balance_cents INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'sent', 'failed')),
  resend_email_id TEXT,
  error_message TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, kind)
);

CREATE TABLE billing_auto_top_up_policies (
  user_id TEXT NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  threshold_cents INTEGER NOT NULL,
  amount_cents INTEGER NOT NULL,
  monthly_cap_cents INTEGER NOT NULL,
  spent_this_month_cents INTEGER NOT NULL DEFAULT 0,
  cap_period_yyyymm TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active', 'disabled', 'paused')),
  pause_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, currency)
);

CREATE TABLE billing_auto_top_up_events (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN (
      'succeeded', 'declined', 'cap_reached', 'skipped',
      'requires_payment_method'
    )
  ),
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  balance_before_cents INTEGER,
  balance_after_cents INTEGER,
  stripe_payment_intent_id TEXT,
  ledger_entry_id TEXT,
  reason TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE stripe_customers (
  user_id TEXT NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  stripe_customer_id TEXT NOT NULL,
  livemode INTEGER NOT NULL DEFAULT 0 CHECK (livemode IN (0, 1)),
  email_snapshot TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, currency, livemode),
  UNIQUE (stripe_customer_id, livemode)
);

CREATE TABLE stripe_checkout_sessions (
  session_id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  package_id TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  payment_status TEXT NOT NULL,
  fulfillment_status TEXT NOT NULL CHECK (
    fulfillment_status IN ('pending', 'fulfilled', 'unpaid', 'expired', 'mismatched')
  ),
  ledger_entry_id TEXT,
  stripe_customer_id TEXT NOT NULL,
  checkout_url TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE stripe_webhook_events (
  event_id TEXT PRIMARY KEY NOT NULL,
  type TEXT NOT NULL,
  processing_status TEXT NOT NULL CHECK (
    processing_status IN ('received', 'processed', 'ignored', 'failed')
  ),
  checkout_session_id TEXT,
  received_at TEXT NOT NULL,
  processed_at TEXT
);

CREATE TABLE stripe_saved_payment_methods (
  user_id TEXT NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  livemode INTEGER NOT NULL DEFAULT 0 CHECK (livemode IN (0, 1)),
  stripe_customer_id TEXT NOT NULL,
  stripe_payment_method_id TEXT NOT NULL,
  setup_intent_id TEXT,
  brand TEXT,
  last4 TEXT,
  exp_month INTEGER,
  exp_year INTEGER,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'detached', 'failed', 'requires_action')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, currency, livemode),
  UNIQUE (stripe_payment_method_id, livemode)
);

CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0),
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0)
);

CREATE TABLE pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL CHECK (
    pay_in_type IN (
      'tip', 'sweep', 'buffer_funding', 'reward', 'adjustment',
      'usd_credit_grant', 'lightning_charge'
    )
  ),
  payer_ref TEXT NOT NULL,
  cost_msat INTEGER NOT NULL CHECK (cost_msat > 0),
  state TEXT NOT NULL CHECK (
    state IN ('pending', 'forwarding', 'paid', 'failed')
  ),
  failure_reason TEXT,
  rung TEXT CHECK (rung IN ('credited', 'direct_bolt12') OR rung IS NULL),
  context_ref TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL,
  public_receipt_ref TEXT
);

CREATE TABLE pay_in_legs (
  id TEXT PRIMARY KEY,
  pay_in_id TEXT NOT NULL REFERENCES pay_ins (id),
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  kind TEXT NOT NULL CHECK (kind IN ('balance', 'lightning')),
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  goal TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'queued',
  started_at TEXT,
  updated_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE users (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL DEFAULT '',
  primary_email TEXT
);

CREATE TABLE khala_code_paid_plan_payment_intents (
  purchase_ref TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  rail TEXT NOT NULL CHECK (rail IN ('stripe_checkout', 'lightning_mpp')),
  status TEXT NOT NULL CHECK (status IN ('requires_payment', 'fulfilled', 'failed', 'expired')),
  plan_id TEXT NOT NULL,
  amount_cents INTEGER,
  amount_sats INTEGER,
  stripe_checkout_session_id TEXT UNIQUE,
  stripe_checkout_url TEXT,
  lightning_payment_hash TEXT UNIQUE,
  lightning_invoice TEXT,
  lightning_network TEXT CHECK (lightning_network IS NULL OR lightning_network IN ('mainnet', 'regtest', 'signet')),
  lightning_invoice_expires_at TEXT,
  entitlement_receipt_ref TEXT,
  failure_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  fulfilled_at TEXT
);
`

/**
 * KS-8.15 (#8326): the seven training-domain D1 tables, condensed from
 * worker migrations 0156/0157/0174/0175/0185/0188 (current live shape,
 * post-ALTER columns inlined). Used by the training domain repository
 * contract suite.
 */
export const TRAINING_DOMAIN_D1_SCHEMA = `
CREATE TABLE training_runs (
  id TEXT PRIMARY KEY,
  training_run_ref TEXT NOT NULL UNIQUE,
  promise_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('planned', 'active', 'sealed', 'reconciled')),
  max_allowed_stale INTEGER NOT NULL DEFAULT 5,
  seal_publication_cadence_windows INTEGER NOT NULL DEFAULT 1,
  seal_in_flight_at TEXT,
  manifest_json TEXT,
  source_refs_json TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE training_windows (
  id TEXT PRIMARY KEY,
  window_ref TEXT NOT NULL UNIQUE,
  training_run_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('planned', 'active', 'sealed', 'reconciled')),
  homework_kind TEXT NOT NULL CHECK (homework_kind IN ('admin_dispatched_homework', 'operator_planned_homework', 'auto_starter')),
  priority INTEGER NOT NULL,
  dataset_refs_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL,
  seal_metadata_json TEXT,
  public_projection_json TEXT NOT NULL,
  planned_at TEXT NOT NULL,
  activated_at TEXT,
  sealed_at TEXT,
  reconciled_at TEXT,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE training_window_events (
  id TEXT PRIMARY KEY,
  window_ref TEXT NOT NULL,
  transition_kind TEXT NOT NULL,
  state_from TEXT,
  state_to TEXT NOT NULL,
  actor_ref TEXT NOT NULL,
  receipt_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE training_window_leases (
  id TEXT PRIMARY KEY,
  lease_ref TEXT NOT NULL UNIQUE,
  window_ref TEXT NOT NULL,
  training_run_ref TEXT NOT NULL,
  pylon_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'released')),
  receipt_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  claimed_at TEXT NOT NULL,
  lease_expires_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE training_verification_challenges (
  id TEXT PRIMARY KEY,
  challenge_ref TEXT NOT NULL UNIQUE,
  training_run_ref TEXT NOT NULL,
  window_ref TEXT,
  contribution_ref TEXT,
  homework_kind TEXT NOT NULL,
  verification_class TEXT NOT NULL CHECK (verification_class IN (
    'deterministic_recompute',
    'exact_trace_replay',
    'freivalds_merkle',
    'seeded_replication',
    'statistical_cross_check'
  )),
  sampling_policy TEXT NOT NULL CHECK (sampling_policy IN ('aggregate', 'per_contribution')),
  state TEXT NOT NULL CHECK (state IN ('Queued', 'Leased', 'Retrying', 'Verified', 'Rejected', 'TimedOut')),
  attempt_count INTEGER NOT NULL,
  max_attempts INTEGER NOT NULL,
  lease_ref TEXT,
  leased_to_ref TEXT,
  lease_expires_at TEXT,
  payload_json TEXT NOT NULL,
  commitment_refs_json TEXT NOT NULL,
  failure_codes_json TEXT NOT NULL,
  verdict_refs_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  verified_at TEXT,
  rejected_at TEXT,
  timed_out_at TEXT,
  archived_at TEXT
);

CREATE TABLE training_verification_events (
  id TEXT PRIMARY KEY,
  challenge_ref TEXT NOT NULL,
  transition_kind TEXT NOT NULL,
  state_from TEXT,
  state_to TEXT NOT NULL,
  validator_ref TEXT,
  failure_codes_json TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE training_trace_contributions (
  id TEXT PRIMARY KEY,
  contribution_ref TEXT NOT NULL UNIQUE,
  lease_ref TEXT NOT NULL,
  window_ref TEXT NOT NULL,
  training_run_ref TEXT NOT NULL,
  pylon_ref TEXT NOT NULL,
  workload_family TEXT NOT NULL,
  assignment_ref TEXT NOT NULL,
  pylon_device_ref TEXT NOT NULL,
  trace_commitment_digest_ref TEXT NOT NULL,
  sampled_window_ref TEXT NOT NULL,
  sampled_window_start_step INTEGER NOT NULL,
  sampled_window_end_step INTEGER NOT NULL,
  worker_receipt_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('pending', 'paired')),
  validator_device_ref TEXT,
  replay_digest_ref TEXT,
  verification_challenge_ref TEXT,
  public_projection_json TEXT NOT NULL,
  submitted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (lease_ref, workload_family)
);
`

/**
 * The D1 DDL for the KS-8.12 sites content-core domain (worker migrations
 * 0032/0038/0082/0083/0084/0085, condensed to the live column set — FK
 * clauses dropped so the contract suite seeds rows directly; the UNIQUE
 * dedupe keys and D1-authority partial uniques are kept EXACTLY because
 * they are load-bearing for the INSERT OR IGNORE builder write paths and
 * the one-active-deployment / active-slug invariants).
 */
export const SITES_CONTENT_D1_SCHEMA = `
CREATE TABLE site_projects (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT,
  owner_user_id TEXT NOT NULL,
  team_id TEXT,
  project_id TEXT,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  prompt TEXT NOT NULL,
  status TEXT NOT NULL,
  access_mode TEXT NOT NULL,
  visibility TEXT NOT NULL,
  source_repository_provider TEXT,
  source_repository_owner TEXT,
  source_repository_name TEXT,
  source_repository_ref TEXT,
  active_version_id TEXT,
  active_deployment_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX site_projects_slug_active_idx
  ON site_projects(slug)
  WHERE archived_at IS NULL;

CREATE UNIQUE INDEX site_projects_order_active_idx
  ON site_projects(software_order_id)
  WHERE software_order_id IS NOT NULL AND archived_at IS NULL;

CREATE TABLE site_versions (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  source_commit_sha TEXT,
  source_archive_r2_key TEXT,
  artifact_manifest_r2_key TEXT,
  build_log_r2_key TEXT,
  build_status TEXT NOT NULL,
  build_command TEXT,
  worker_module_r2_key TEXT,
  static_assets_manifest_json TEXT NOT NULL DEFAULT '{}',
  d1_binding_name TEXT,
  r2_binding_name TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_by_user_id TEXT,
  created_by_run_id TEXT,
  created_at TEXT NOT NULL,
  saved_at TEXT,
  rejected_at TEXT
);

CREATE TABLE site_deployments (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL,
  version_id TEXT NOT NULL,
  slug TEXT NOT NULL,
  url TEXT NOT NULL,
  runtime_kind TEXT NOT NULL,
  runtime_script_name TEXT,
  dispatch_namespace TEXT,
  status TEXT NOT NULL,
  deployed_by_user_id TEXT,
  external_deployment_id TEXT,
  started_at TEXT,
  activated_at TEXT,
  failed_at TEXT,
  disabled_at TEXT,
  rolled_back_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX site_deployments_site_active_idx
  ON site_deployments(site_id)
  WHERE status = 'active';

CREATE TABLE site_deployment_attempts (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL,
  version_id TEXT NOT NULL,
  deployment_id TEXT,
  runtime_kind TEXT NOT NULL,
  runtime_script_name TEXT,
  dispatch_namespace TEXT,
  external_deployment_id TEXT,
  status TEXT NOT NULL,
  upload_receipt_ref TEXT,
  health_status TEXT NOT NULL,
  health_url TEXT,
  health_ref TEXT,
  rollback_ref TEXT,
  observability_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE site_access_grants (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL,
  principal_kind TEXT NOT NULL,
  principal_ref TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE UNIQUE INDEX site_access_grants_active_principal_idx
  ON site_access_grants(site_id, principal_kind, principal_ref, role)
  WHERE revoked_at IS NULL;

CREATE TABLE site_events (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT NOT NULL,
  version_id TEXT,
  deployment_id TEXT,
  type TEXT NOT NULL,
  summary TEXT NOT NULL,
  actor_user_id TEXT,
  actor_run_id TEXT,
  payload_json TEXT,
  created_at TEXT NOT NULL,
  email_message_id TEXT
);

CREATE TABLE site_builder_sessions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  site_id TEXT,
  order_id TEXT,
  workroom_id TEXT,
  owner_user_id TEXT NOT NULL,
  customer_user_id TEXT,
  created_by_actor_ref TEXT NOT NULL,
  status TEXT NOT NULL,
  prompt_summary TEXT NOT NULL,
  source_site_version_id TEXT,
  source_revision_id TEXT,
  active_preview_id TEXT,
  active_artifact_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE site_builder_messages (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  actor_kind TEXT NOT NULL,
  visibility TEXT NOT NULL,
  body TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX idx_site_builder_messages_session_sequence
  ON site_builder_messages(session_id, sequence);

CREATE TABLE site_builder_events (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  event_kind TEXT NOT NULL,
  phase_kind TEXT,
  visibility TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  source_ref TEXT,
  payload_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX idx_site_builder_events_session_sequence
  ON site_builder_events(session_id, sequence);

CREATE TABLE site_builder_phase_runs (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  phase_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX idx_site_builder_phase_runs_session_sequence
  ON site_builder_phase_runs(session_id, sequence);

CREATE TABLE site_builder_file_snapshots (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  path TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  language TEXT,
  content_hash TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  source_ref TEXT,
  artifact_ref TEXT,
  preview_text TEXT,
  visibility TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX idx_site_builder_file_snapshots_session_path_sequence
  ON site_builder_file_snapshots(session_id, path, sequence);

CREATE TABLE site_builder_previews (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  preview_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  preview_url TEXT,
  version_ref TEXT,
  artifact_ref TEXT,
  health_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE site_builder_artifacts (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  artifact_kind TEXT NOT NULL,
  artifact_ref TEXT NOT NULL,
  content_hash TEXT,
  byte_size INTEGER,
  manifest_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE site_builder_repair_attempts (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  preview_id TEXT,
  phase_kind TEXT,
  attempt_number INTEGER NOT NULL,
  retry_budget INTEGER NOT NULL,
  status TEXT NOT NULL,
  failure_kind TEXT NOT NULL,
  redacted_summary TEXT NOT NULL,
  stop_reason TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  completed_at TEXT,
  archived_at TEXT
);

CREATE UNIQUE INDEX idx_site_builder_repair_attempts_session_attempt
  ON site_builder_repair_attempts(session_id, attempt_number);

CREATE TABLE site_builder_saved_versions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  site_id TEXT NOT NULL,
  site_version_id TEXT NOT NULL,
  preview_id TEXT,
  artifact_ref TEXT,
  build_receipt_ref TEXT,
  source_hash TEXT,
  notes TEXT,
  site_metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);
`

/**
 * Condensed live D1 schema for the KS-8.12 REMAINDER (#8357) contract
 * suite — the subset of remainder tables the suite exercises: secrets
 * (site_environment_values, WITH plain_value so the test can prove the
 * mirror DROPS it), idempotency dedupe (site_provisioning_plans,
 * targeted_site_*), and the money/referral tables (payment_events,
 * revenue_share_links, referral_sources, payout_ledger). D1-authority
 * dedupe UNIQUEs are KEPT so the suite exercises real dedupe rejection.
 */
export const SITES_REMAINDER_D1_SCHEMA = `
CREATE TABLE site_environment_values (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT,
  key TEXT,
  kind TEXT,
  secret_ref TEXT,
  plain_value TEXT,
  created_at TEXT,
  updated_at TEXT,
  deleted_at TEXT
);

CREATE TABLE site_provisioning_plans (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT,
  site_id TEXT,
  status TEXT,
  requested_by_user_id TEXT,
  reviewed_by_user_id TEXT,
  resource_manifest_json TEXT,
  receipt_json TEXT,
  created_at TEXT,
  reviewed_at TEXT,
  updated_at TEXT,
  archived_at TEXT,
  UNIQUE(idempotency_key)
);

CREATE TABLE targeted_site_campaigns (
  id TEXT PRIMARY KEY NOT NULL,
  slug TEXT,
  name TEXT,
  owner_user_id TEXT,
  operator_user_id TEXT,
  vertical TEXT,
  geography TEXT,
  source_authority_ref TEXT,
  budget_cap_ref TEXT,
  suppression_policy_ref TEXT,
  operator_state TEXT,
  metadata_json TEXT,
  created_at TEXT,
  updated_at TEXT,
  archived_at TEXT,
  UNIQUE(slug)
);

CREATE TABLE targeted_site_prospects (
  id TEXT PRIMARY KEY NOT NULL,
  campaign_id TEXT,
  idempotency_key TEXT,
  normalized_domain TEXT,
  origin_url TEXT,
  company_name TEXT,
  site_name TEXT,
  contact_refs_json TEXT,
  vertical TEXT,
  geography TEXT,
  source_ref TEXT,
  discovery_confidence NUMERIC,
  suppression_state TEXT,
  capture_state TEXT,
  review_state TEXT,
  metadata_json TEXT,
  discovered_at TEXT,
  created_at TEXT,
  updated_at TEXT,
  archived_at TEXT,
  UNIQUE(idempotency_key),
  UNIQUE(campaign_id, normalized_domain)
);

CREATE TABLE site_referral_sources (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT,
  site_version_id TEXT,
  referrer_user_id TEXT,
  public_source_ref TEXT,
  public_slug TEXT,
  campaign_ref TEXT,
  source_label TEXT,
  policy_state TEXT,
  created_at TEXT,
  updated_at TEXT,
  archived_at TEXT,
  UNIQUE(public_source_ref)
);

CREATE TABLE site_referral_payout_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  payout_ref TEXT,
  idempotency_key TEXT,
  referral_attribution_id TEXT,
  referral_source_id TEXT,
  referral_invite_id TEXT,
  referrer_user_id TEXT,
  referred_user_id TEXT,
  qualifying_event_ref TEXT,
  qualifying_event_kind TEXT,
  qualifying_amount_sats INTEGER,
  amount_sats INTEGER,
  period_key TEXT,
  state TEXT,
  state_reason_ref TEXT,
  previous_entry_id TEXT,
  reversal_of_entry_id TEXT,
  evidence_refs_json TEXT,
  policy_refs_json TEXT,
  caveat_refs_json TEXT,
  created_at TEXT,
  archived_at TEXT,
  UNIQUE(idempotency_key)
);

CREATE TABLE site_commerce_payment_events (
  id TEXT PRIMARY KEY NOT NULL,
  site_id TEXT,
  site_version_id TEXT,
  software_order_id TEXT,
  product_id TEXT,
  paid_action_id TEXT,
  customer_ref TEXT,
  referral_source_ref TEXT,
  payment_evidence_ref TEXT,
  entitlement_ref TEXT,
  public_receipt_ref TEXT,
  event_kind TEXT,
  amount NUMERIC,
  asset TEXT,
  created_at TEXT,
  UNIQUE(public_receipt_ref)
);

CREATE TABLE site_commerce_revenue_share_links (
  id TEXT PRIMARY KEY NOT NULL,
  payment_event_id TEXT,
  accepted_work_ref TEXT,
  requested_contributor_asset TEXT,
  provider_payout_claimed INTEGER,
  nexus_receipt_ref TEXT,
  treasury_receipt_ref TEXT,
  ldk_settlement_receipt_ref TEXT,
  referral_reward_trigger TEXT,
  provider_payout_eligibility_state TEXT,
  withdrawal_posture TEXT,
  projection_json TEXT,
  created_at TEXT
);
`

/**
 * Condensed live D1 schema for the KS-8.16 forge domain contract suite
 * (worker migrations 0251/0252/0253/0254/0255/0256/0259/0260/0284,
 * post-ALTER final shape). The D1-authority uniques/partials are KEPT
 * here — the contract suite exercises the real lease-conflict and
 * held-lock behavior of the authoritative engine.
 */
export const FORGE_DOMAIN_D1_SCHEMA = `
CREATE TABLE forge_coordination_issues (
  tenant_ref TEXT NOT NULL,
  issue_ref TEXT NOT NULL,
  github_issue_number INTEGER,
  title TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('open', 'closed', 'draft')),
  priority_ref TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  git_token_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, issue_ref)
);
CREATE UNIQUE INDEX idx_forge_coordination_issues_github_number
  ON forge_coordination_issues (tenant_ref, github_issue_number)
  WHERE github_issue_number IS NOT NULL;

CREATE TABLE forge_coordination_prs (
  tenant_ref TEXT NOT NULL,
  pr_ref TEXT NOT NULL,
  issue_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('draft', 'open', 'ready', 'blocked', 'applied', 'closed')),
  base_head TEXT NOT NULL,
  patch_head TEXT NOT NULL,
  verification_ref TEXT,
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, pr_ref)
);
CREATE UNIQUE INDEX idx_forge_coordination_prs_change_ref
  ON forge_coordination_prs (tenant_ref, change_ref);

CREATE TABLE forge_coordination_status (
  tenant_ref TEXT NOT NULL,
  status_ref TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  nip34_kind INTEGER NOT NULL CHECK (nip34_kind IN (1630, 1631, 1632, 1633)),
  state TEXT NOT NULL CHECK (state IN ('open', 'applied', 'closed', 'draft')),
  actor_ref TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, status_ref)
);

CREATE TABLE forge_dispatch_leases (
  tenant_ref TEXT NOT NULL,
  lease_ref TEXT NOT NULL,
  work_ref TEXT NOT NULL,
  owner_agent_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'released', 'expired', 'cancelled')),
  idempotency_key_hash TEXT,
  acquired_at TEXT NOT NULL,
  heartbeat_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  released_at TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, lease_ref)
);
CREATE UNIQUE INDEX idx_forge_dispatch_leases_active_work
  ON forge_dispatch_leases (tenant_ref, work_ref)
  WHERE state = 'active';
CREATE UNIQUE INDEX idx_forge_dispatch_leases_idempotency
  ON forge_dispatch_leases (tenant_ref, idempotency_key_hash)
  WHERE idempotency_key_hash IS NOT NULL;

CREATE TABLE forge_merge_queue_ledger (
  tenant_ref TEXT NOT NULL,
  queue_ref TEXT NOT NULL,
  base_head TEXT NOT NULL,
  actual_head TEXT NOT NULL,
  virtual_head TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('projected', 'blocked', 'promoting', 'promoted', 'superseded')),
  next_promotion_ref TEXT,
  ready_json TEXT NOT NULL DEFAULT '[]',
  blocked_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, queue_ref)
);

CREATE TABLE forge_git_packfile_archives (
  tenant_ref TEXT NOT NULL,
  packfile_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  change_ref TEXT,
  receive_pack_ref TEXT,
  artifact_r2_key TEXT NOT NULL,
  packfile_sha256 TEXT NOT NULL,
  packfile_bytes INTEGER NOT NULL CHECK (packfile_bytes >= 0),
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256', 'unknown')),
  command_count INTEGER NOT NULL CHECK (command_count >= 0),
  capabilities_json TEXT NOT NULL DEFAULT '[]',
  ref_updates_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  content_type TEXT NOT NULL DEFAULT 'application/x-git-packed-objects',
  visibility TEXT NOT NULL CHECK (visibility = 'operator_only'),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, packfile_ref)
);
CREATE UNIQUE INDEX idx_forge_git_packfile_archives_digest
  ON forge_git_packfile_archives (tenant_ref, packfile_sha256);
CREATE UNIQUE INDEX idx_forge_git_packfile_archives_r2_key
  ON forge_git_packfile_archives (artifact_r2_key);

CREATE TABLE forge_tenants (
  tenant_ref TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'suspended')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  confidential_workspace_mode TEXT,
  attestation_ref TEXT,
  encrypted_knowledge_pack_ref TEXT,
  refusal_reason TEXT,
  retention_policy_ref TEXT
);

CREATE TABLE forge_git_access_tokens (
  tenant_ref TEXT NOT NULL,
  token_ref TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'revoked', 'expired')),
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  ref_restrictions_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, token_ref)
);
CREATE UNIQUE INDEX idx_forge_git_access_tokens_hash
  ON forge_git_access_tokens (token_hash);

CREATE TABLE forge_git_access_token_scopes (
  tenant_ref TEXT NOT NULL,
  token_ref TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('git:upload-pack', 'git:receive-pack', 'git:admin')),
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, token_ref, scope)
);

CREATE TABLE forge_verification_receipts (
  tenant_ref TEXT NOT NULL,
  verification_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  base_ref TEXT NOT NULL,
  base_head TEXT NOT NULL,
  head_ref TEXT NOT NULL,
  head_head TEXT NOT NULL,
  packfile_ref TEXT NOT NULL,
  packfile_sha256 TEXT NOT NULL,
  executor_identity_ref TEXT NOT NULL,
  command_ref TEXT NOT NULL,
  command_args_json TEXT NOT NULL DEFAULT '[]',
  exit_code INTEGER,
  verdict TEXT NOT NULL CHECK (verdict IN ('passed', 'failed', 'timed_out', 'cancelled', 'errored')),
  started_at TEXT NOT NULL,
  completed_at TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  log_sha256 TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, verification_ref)
);

CREATE TABLE forge_promotion_decisions (
  tenant_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  queue_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (decision IN ('approved', 'blocked', 'superseded')),
  base_head TEXT NOT NULL,
  candidate_head TEXT NOT NULL,
  promoted_head TEXT,
  verification_ref TEXT,
  gate_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  decided_by_ref TEXT NOT NULL,
  decided_at TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  target_ref TEXT NOT NULL DEFAULT '',
  queue_position INTEGER NOT NULL DEFAULT 0,
  gate_results_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, promotion_ref)
);

CREATE TABLE forge_git_receive_pack_intakes (
  tenant_ref TEXT NOT NULL,
  receive_pack_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  token_ref TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  change_ref TEXT,
  packfile_ref TEXT,
  packfile_sha256 TEXT,
  packfile_bytes INTEGER NOT NULL CHECK (packfile_bytes >= 0),
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256', 'unknown')),
  state TEXT NOT NULL CHECK (state IN ('accepted', 'rejected')),
  command_count INTEGER NOT NULL CHECK (command_count >= 0),
  ref_updates_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  rejection_code TEXT,
  rejection_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, receive_pack_ref)
);

CREATE TABLE forge_git_refs (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  ref_name TEXT NOT NULL,
  object_id TEXT,
  previous_object_id TEXT,
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256', 'unknown')),
  state TEXT NOT NULL CHECK (state IN ('active', 'deleted')),
  updated_by_change_ref TEXT NOT NULL,
  updated_by_packfile_ref TEXT NOT NULL,
  updated_by_receive_pack_ref TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, repository_ref, ref_name),
  CHECK (
    (state = 'active' AND object_id IS NOT NULL)
    OR (state = 'deleted' AND object_id IS NULL)
  )
);

CREATE TABLE forge_git_objects (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  object_id TEXT NOT NULL,
  object_format TEXT NOT NULL CHECK (object_format IN ('sha1', 'sha256')),
  packfile_ref TEXT NOT NULL,
  packfile_sha256 TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  latest_seen_at TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, repository_ref, object_id)
);

CREATE TABLE forge_git_ref_locks (
  tenant_ref TEXT NOT NULL,
  lock_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  ref_name TEXT NOT NULL,
  receive_pack_ref TEXT NOT NULL,
  expected_old_object_id TEXT NOT NULL,
  new_object_id TEXT NOT NULL,
  action TEXT NOT NULL CHECK (action IN ('create', 'update', 'delete')),
  state TEXT NOT NULL CHECK (state IN ('held', 'applied', 'rejected')),
  acquired_at TEXT NOT NULL,
  released_at TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, lock_ref)
);
CREATE UNIQUE INDEX idx_forge_git_ref_locks_held_ref
  ON forge_git_ref_locks (tenant_ref, repository_ref, ref_name)
  WHERE state = 'held';

CREATE TABLE forge_github_mirror_receipts (
  tenant_ref TEXT NOT NULL,
  mirror_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  source_canonical_ref TEXT NOT NULL,
  destination_github_repository TEXT NOT NULL,
  destination_github_ref TEXT NOT NULL,
  commit_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('mirrored', 'refused', 'failed')),
  attempt_count INTEGER NOT NULL DEFAULT 1 CHECK (attempt_count >= 1),
  first_attempted_at TEXT NOT NULL,
  last_attempted_at TEXT NOT NULL,
  completed_at TEXT,
  refusal_reason TEXT,
  error_reason TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, mirror_ref),
  UNIQUE (
    tenant_ref,
    promotion_ref,
    destination_github_repository,
    destination_github_ref
  )
);
`

// KS-8.14 (#8325): the 32 live business funnel / orders / referrals
// tables, condensed from the worker migrations (FKs to tables outside the
// domain dropped; constraints and uniques kept — the contract suite's
// INSERT OR IGNORE / ON CONFLICT semantics depend on them).
export const BUSINESS_DOMAIN_D1_SCHEMA = `
CREATE TABLE IF NOT EXISTS business_signup_requests (
  id                      TEXT NOT NULL PRIMARY KEY,
  business_name           TEXT NOT NULL,
  contact_email           TEXT NOT NULL,
  website                 TEXT,
  phone                   TEXT NOT NULL,
  help_with               TEXT,
  request_slack_channel   INTEGER NOT NULL DEFAULT 0
    CHECK (request_slack_channel IN (0, 1)),
  slack_connect_status    TEXT NOT NULL CHECK (
    slack_connect_status IN (
      'not_requested', 'manual_invite_pending', 'invite_sent', 'accepted',
      'declined'
    )
  ),
  source_route            TEXT NOT NULL DEFAULT '/business',
  created_at              TEXT NOT NULL,
  updated_at              TEXT NOT NULL,
  referral_code           TEXT,
  referral_attribution_id TEXT,
  fulfillment_status      TEXT NOT NULL DEFAULT 'pending'
    CHECK (fulfillment_status IN ('pending', 'invited', 'operator_parked')),
  fulfillment_ref         TEXT,
  fulfillment_reason      TEXT,
  source_ref              TEXT NOT NULL DEFAULT 'direct',
  linked_pipeline_ref     TEXT
);

CREATE TABLE IF NOT EXISTS business_signup_fulfillments (
  id                         TEXT NOT NULL PRIMARY KEY,
  business_signup_request_id TEXT NOT NULL UNIQUE,
  status                     TEXT NOT NULL
    CHECK (status IN ('invited', 'operator_parked')),
  reason                     TEXT,
  enrichment_ref             TEXT NOT NULL,
  team_id                    TEXT,
  project_id                 TEXT,
  workspace_id               TEXT,
  invite_id                  TEXT,
  email_message_id           TEXT,
  email_delivery_status      TEXT NOT NULL CHECK (
    email_delivery_status IN (
      'accepted', 'disabled', 'failed', 'missing_config', 'not_attempted'
    )
  ),
  metadata_json              TEXT NOT NULL DEFAULT '{}',
  created_at                 TEXT NOT NULL,
  updated_at                 TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS business_signup_referral_attributions (
  business_signup_request_id TEXT NOT NULL PRIMARY KEY,
  referral_attribution_id    TEXT NOT NULL,
  referral_source_id         TEXT NOT NULL,
  referral_invite_id         TEXT,
  capture_path               TEXT NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                     TEXT NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  linked_at                  TEXT NOT NULL,
  policy_state               TEXT NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at                 TEXT NOT NULL,
  updated_at                 TEXT NOT NULL,
  archived_at                TEXT
);

CREATE TABLE IF NOT EXISTS business_funnel_events (
  id          TEXT NOT NULL PRIMARY KEY,
  event_ref   TEXT NOT NULL UNIQUE,
  stage       TEXT NOT NULL CHECK (
    stage IN (
      'visit', 'signup', 'intake_spec', 'payment', 'provisioned',
      'first_outcome', 'retained', 'referred_engagement'
    )
  ),
  source_kind TEXT NOT NULL CHECK (
    source_kind IN (
      'content', 'outbound', 'ai_search', 'referral', 'direct', 'unknown'
    )
  ),
  source_ref  TEXT,
  occurred_at TEXT NOT NULL,
  observed_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS business_service_promises (
  id                           TEXT NOT NULL PRIMARY KEY,
  promise_ref                  TEXT NOT NULL UNIQUE,
  accepted_outcome_contract_id TEXT,
  workspace_ref                TEXT NOT NULL,
  crm_state_ref                TEXT NOT NULL,
  stakeholder_refs_json        TEXT NOT NULL DEFAULT '[]',
  state                        TEXT NOT NULL
    CHECK (state IN ('active', 'paused', 'blocked', 'closed')),
  cadence                      TEXT NOT NULL
    CHECK (cadence IN ('daily', 'weekly')),
  next_motion_due_at           TEXT,
  last_motion_receipt_ref      TEXT,
  source_refs_json             TEXT NOT NULL DEFAULT '[]',
  metadata_json                TEXT NOT NULL DEFAULT '{}',
  created_at                   TEXT NOT NULL,
  updated_at                   TEXT NOT NULL,
  blocking_reason_ref          TEXT,
  blocked_at                   TEXT,
  last_escalation_page_ref     TEXT
);

CREATE TABLE IF NOT EXISTS business_fulfillment_motion_receipts (
  id                                   TEXT NOT NULL PRIMARY KEY,
  promise_id                           TEXT NOT NULL,
  promise_ref                          TEXT NOT NULL,
  motion_date                          TEXT NOT NULL,
  receipt_ref                          TEXT NOT NULL UNIQUE,
  agent_definition_ref                 TEXT NOT NULL,
  crm_state_ref                        TEXT NOT NULL,
  stakeholder_refs_json                TEXT NOT NULL DEFAULT '[]',
  stakeholder_flag_refs_json           TEXT NOT NULL DEFAULT '[]',
  forward_motion_ref                   TEXT NOT NULL,
  client_comms_draft_ref               TEXT NOT NULL,
  approval_gate_ref                    TEXT NOT NULL,
  outbound_allowed                     INTEGER NOT NULL DEFAULT 0
    CHECK (outbound_allowed IN (0, 1)),
  blocker_refs_json                    TEXT NOT NULL DEFAULT '[]',
  source_refs_json                     TEXT NOT NULL DEFAULT '[]',
  created_at                           TEXT NOT NULL,
  cadence                              TEXT NOT NULL DEFAULT 'daily'
    CHECK (cadence IN ('daily', 'weekly')),
  client_comms_email_ledger_ref        TEXT,
  customer_visible_workroom_update_ref TEXT,
  UNIQUE (promise_id, motion_date)
);

CREATE TABLE IF NOT EXISTS business_fulfillment_escalation_pages (
  id                     TEXT NOT NULL PRIMARY KEY,
  promise_id             TEXT NOT NULL,
  promise_ref            TEXT NOT NULL,
  escalation_date        TEXT NOT NULL,
  receipt_ref            TEXT NOT NULL UNIQUE,
  page_ref               TEXT NOT NULL UNIQUE,
  owner_notification_ref TEXT NOT NULL,
  agent_definition_ref   TEXT NOT NULL,
  blocking_reason_ref    TEXT NOT NULL,
  blocked_at             TEXT NOT NULL,
  workspace_ref          TEXT NOT NULL,
  stakeholder_refs_json  TEXT NOT NULL DEFAULT '[]',
  source_refs_json       TEXT NOT NULL DEFAULT '[]',
  created_at             TEXT NOT NULL,
  UNIQUE (promise_id, escalation_date)
);

CREATE TABLE IF NOT EXISTS business_checkout_kickoffs (
  checkout_session_id         TEXT NOT NULL PRIMARY KEY,
  business_signup_request_id  TEXT NOT NULL,
  user_id                     TEXT NOT NULL,
  total_amount_cents          INTEGER NOT NULL CHECK (total_amount_cents >= 0),
  setup_fee_cents             INTEGER NOT NULL CHECK (setup_fee_cents >= 0),
  credit_grant_cents          INTEGER NOT NULL CHECK (credit_grant_cents >= 0),
  workspace_id                TEXT NOT NULL,
  service_promise_contract_id TEXT NOT NULL,
  public_receipt_ref          TEXT NOT NULL,
  created_at                  TEXT NOT NULL,
  updated_at                  TEXT NOT NULL,
  CHECK (setup_fee_cents + credit_grant_cents = total_amount_cents)
);

CREATE TABLE IF NOT EXISTS business_commitment_ledger (
  id                   TEXT NOT NULL PRIMARY KEY,
  commitment_ref       TEXT NOT NULL UNIQUE,
  engagement_ref       TEXT NOT NULL,
  owner_ref            TEXT NOT NULL,
  vertical_ref         TEXT NOT NULL,
  promised_object_ref  TEXT NOT NULL,
  commitment_kind      TEXT NOT NULL
    CHECK (commitment_kind IN ('deliverable', 'send')),
  due_state            TEXT NOT NULL
    CHECK (due_state IN ('due', 'blocked', 'shipped', 'parked')),
  due_at               TEXT NOT NULL,
  shipped_at           TEXT,
  weekly_review_ref    TEXT NOT NULL,
  source_refs_json     TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json    TEXT NOT NULL DEFAULT '[]',
  evidence_refs_json   TEXT NOT NULL DEFAULT '[]',
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL,
  pipeline_ref         TEXT
);

CREATE TABLE IF NOT EXISTS business_pipeline_rows (
  pipeline_ref                 TEXT NOT NULL PRIMARY KEY,
  vertical                     TEXT NOT NULL,
  source_ref                   TEXT NOT NULL,
  stage                        TEXT NOT NULL CHECK (
    stage IN (
      'intake_received', 'scope_scheduled', 'scope_completed',
      'receipt_plan_sent', 'closed_won', 'closed_lost', 'quick_win_started'
    )
  ),
  quoted_min_usd_cents         INTEGER NOT NULL DEFAULT 0
    CHECK (quoted_min_usd_cents >= 0),
  quoted_max_usd_cents         INTEGER NOT NULL DEFAULT 0
    CHECK (quoted_max_usd_cents >= quoted_min_usd_cents),
  quoted_band_label            TEXT NOT NULL DEFAULT 'unquoted',
  owner_role                   TEXT NOT NULL CHECK (
    owner_role IN ('operator', 'reviewer', 'fulfillment_agent', 'owner')
  ),
  next_action_due_at           TEXT,
  blocker_ref                  TEXT,
  receipt_refs_json            TEXT NOT NULL DEFAULT '[]',
  partner_route_flag           INTEGER NOT NULL DEFAULT 0
    CHECK (partner_route_flag IN (0, 1)),
  created_at                   TEXT NOT NULL,
  updated_at                   TEXT NOT NULL,
  stage_updated_at             TEXT NOT NULL,
  business_signup_request_id   TEXT,
  partner_route_state          TEXT NOT NULL DEFAULT 'none' CHECK (
    partner_route_state IN (
      'none', 'candidate', 'offered', 'accepted', 'declined'
    )
  ),
  partner_peer_ref             TEXT,
  partner_approval_receipt_ref TEXT,
  partner_offer_ref            TEXT,
  partner_scope_summary_ref    TEXT,
  partner_due_window_ref       TEXT,
  partner_budget_range_ref     TEXT,
  partner_privacy_tier_ref     TEXT,
  partner_route_updated_at     TEXT
);

CREATE TABLE IF NOT EXISTS business_starter_credit_grants (
  grant_ref                    TEXT NOT NULL PRIMARY KEY,
  pipeline_ref                 TEXT NOT NULL,
  account_ref                  TEXT NOT NULL,
  engagement_ref               TEXT NOT NULL,
  attribution_kind             TEXT NOT NULL DEFAULT 'sales_starter_credit'
    CHECK (attribution_kind = 'sales_starter_credit'),
  transfer_policy              TEXT NOT NULL DEFAULT 'non_transferable'
    CHECK (transfer_policy = 'non_transferable'),
  amount_usd_cents             INTEGER NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat                  INTEGER NOT NULL CHECK (amount_msat > 0),
  amount_cap_usd_cents         INTEGER NOT NULL
    CHECK (amount_cap_usd_cents > 0),
  window_ref                   TEXT NOT NULL,
  window_grant_cap             INTEGER NOT NULL CHECK (window_grant_cap > 0),
  credit_receipt_ref           TEXT NOT NULL UNIQUE,
  redemption_receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json             TEXT NOT NULL DEFAULT '[]',
  created_at                   TEXT NOT NULL,
  updated_at                   TEXT NOT NULL,
  CHECK (amount_usd_cents <= amount_cap_usd_cents)
);

CREATE TABLE IF NOT EXISTS business_affiliate_codes (
  code         TEXT NOT NULL PRIMARY KEY,
  source_ref   TEXT NOT NULL UNIQUE,
  owner_ref    TEXT NOT NULL,
  issued_by_ref TEXT NOT NULL,
  policy_state TEXT NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'paused', 'archived')),
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL,
  archived_at  TEXT
);

CREATE TABLE IF NOT EXISTS business_affiliate_attributions (
  attribution_ref            TEXT NOT NULL PRIMARY KEY,
  code                       TEXT NOT NULL,
  source_ref                 TEXT NOT NULL,
  owner_ref                  TEXT NOT NULL,
  business_signup_request_id TEXT NOT NULL UNIQUE,
  pipeline_ref               TEXT,
  payment_receipt_ref        TEXT,
  policy_state               TEXT NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'archived')),
  created_at                 TEXT NOT NULL,
  updated_at                 TEXT NOT NULL,
  archived_at                TEXT
);

CREATE TABLE IF NOT EXISTS software_orders (
  id                              TEXT NOT NULL PRIMARY KEY,
  user_id                         TEXT NOT NULL,
  status                          TEXT NOT NULL DEFAULT 'submitted' CHECK (
    status IN (
      'submitted', 'scoping', 'free_slice_ready', 'quote_ready',
      'agent_queued', 'agent_running', 'delivered', 'needs_customer_input',
      'declined', 'unavailable'
    )
  ),
  visibility                      TEXT NOT NULL DEFAULT 'public'
    CHECK (visibility IN ('public')),
  request                         TEXT NOT NULL,
  repository_provider             TEXT CHECK (
    repository_provider IS NULL OR repository_provider IN ('github')
  ),
  repository_owner                TEXT,
  repository_name                 TEXT,
  repository_full_name            TEXT,
  repository_private              INTEGER CHECK (
    repository_private IS NULL OR repository_private IN (0, 1)
  ),
  repository_default_branch       TEXT,
  repository_html_url             TEXT,
  public_work_acknowledged_at     TEXT NOT NULL,
  data_use_acknowledged_at        TEXT NOT NULL,
  compute_payment_acknowledged_at TEXT NOT NULL,
  provider_account_required       INTEGER NOT NULL DEFAULT 0
    CHECK (provider_account_required IN (0, 1)),
  free_slice_cents                INTEGER NOT NULL DEFAULT 5000,
  quote_cents                     INTEGER,
  current_run_id                  TEXT,
  agent_started_at                TEXT,
  created_at                      TEXT NOT NULL,
  updated_at                      TEXT NOT NULL,
  archived_at                     TEXT,
  agent_idempotency_key           TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS software_orders_agent_idempotency_idx
  ON software_orders (user_id, agent_idempotency_key)
  WHERE agent_idempotency_key IS NOT NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS order_triage_records (
  id                   TEXT NOT NULL PRIMARY KEY,
  software_order_id    TEXT NOT NULL,
  classification       TEXT NOT NULL CHECK (
    classification IN (
      'runnable_site', 'runnable_general_autopilot', 'needs_clarification',
      'smoke_or_test', 'legal_sensitive_policy_review',
      'unavailable_or_declined'
    )
  ),
  operator_priority    INTEGER NOT NULL DEFAULT 100,
  first_batch_eligible INTEGER NOT NULL DEFAULT 0
    CHECK (first_batch_eligible IN (0, 1)),
  hold_reason          TEXT,
  next_action          TEXT NOT NULL,
  customer_safe_status TEXT NOT NULL,
  customer_safe_summary TEXT NOT NULL,
  reviewer_user_id     TEXT,
  reviewed_at          TEXT,
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL,
  archived_at          TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS order_triage_records_active_order_idx
  ON order_triage_records (software_order_id)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS order_triage_events (
  id                TEXT NOT NULL PRIMARY KEY,
  triage_record_id  TEXT NOT NULL,
  software_order_id TEXT NOT NULL,
  site_id           TEXT,
  assignment_id     TEXT,
  event_type        TEXT NOT NULL,
  visibility        TEXT NOT NULL
    CHECK (visibility IN ('private', 'team', 'public')),
  summary           TEXT NOT NULL,
  actor_user_id     TEXT,
  payload_json      TEXT,
  created_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS order_fulfillment_artifacts (
  id                   TEXT NOT NULL PRIMARY KEY,
  software_order_id    TEXT NOT NULL,
  assignment_id        TEXT,
  run_id               TEXT,
  kind                 TEXT NOT NULL CHECK (
    kind IN (
      'pull_request', 'branch', 'commit', 'diff', 'preview', 'notes',
      'attachment'
    )
  ),
  title                TEXT NOT NULL,
  summary              TEXT NOT NULL,
  url                  TEXT,
  repository_full_name TEXT,
  source_branch        TEXT,
  target_branch        TEXT,
  commit_sha           TEXT,
  status               TEXT NOT NULL CHECK (
    status IN (
      'draft', 'customer_review_ready', 'customer_accepted', 'superseded',
      'rejected'
    )
  ),
  visibility           TEXT NOT NULL
    CHECK (visibility IN ('private', 'team', 'public')),
  metadata_json        TEXT NOT NULL DEFAULT '{}',
  created_by_user_id   TEXT,
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL,
  archived_at          TEXT
);

CREATE TABLE IF NOT EXISTS order_fulfillment_feedback (
  id                     TEXT NOT NULL PRIMARY KEY,
  software_order_id      TEXT NOT NULL,
  artifact_id            TEXT,
  author_user_id         TEXT NOT NULL,
  body                   TEXT NOT NULL,
  status                 TEXT NOT NULL CHECK (
    status IN (
      'submitted', 'queued', 'running', 'addressed', 'closed', 'rejected'
    )
  ),
  source                 TEXT NOT NULL
    CHECK (source IN ('customer_order_ui', 'operator', 'agent')),
  visibility             TEXT NOT NULL
    CHECK (visibility IN ('private', 'team', 'public')),
  adjutant_assignment_id TEXT,
  adjutant_adjustment_id TEXT,
  created_at             TEXT NOT NULL,
  updated_at             TEXT NOT NULL,
  archived_at            TEXT
);

CREATE TABLE IF NOT EXISTS order_github_write_authority_receipts (
  id                   TEXT NOT NULL PRIMARY KEY,
  software_order_id    TEXT NOT NULL,
  assignment_id        TEXT,
  user_id              TEXT NOT NULL,
  repository_full_name TEXT NOT NULL,
  repository_private   INTEGER NOT NULL CHECK (repository_private IN (0, 1)),
  requested_operation  TEXT NOT NULL CHECK (
    requested_operation IN (
      'create_branch', 'push_commit', 'open_pull_request',
      'open_fork_pull_request'
    )
  ),
  decision             TEXT NOT NULL CHECK (decision IN ('allowed', 'blocked')),
  authority_mode       TEXT CHECK (
    authority_mode IS NULL OR authority_mode IN (
      'customer_grant', 'openagents_fork', 'openagents_app'
    )
  ),
  blocked_reason       TEXT,
  connection_ref       TEXT,
  grant_ref            TEXT,
  approval_source      TEXT CHECK (
    approval_source IS NULL OR approval_source IN (
      'customer_action', 'operator_action', 'system_policy'
    )
  ),
  approved_at          TEXT,
  customer_message     TEXT NOT NULL,
  metadata_json        TEXT NOT NULL DEFAULT '{}',
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS referral_invites (
  id                 TEXT NOT NULL PRIMARY KEY,
  referral_source_id TEXT NOT NULL,
  public_invite_ref  TEXT NOT NULL UNIQUE,
  token_hash         TEXT NOT NULL,
  scope              TEXT NOT NULL
    CHECK (scope IN ('site_join', 'order_start', 'agent_claim')),
  audience_path      TEXT NOT NULL CHECK (audience_path IN ('human', 'agent')),
  policy_state       TEXT NOT NULL DEFAULT 'active' CHECK (
    policy_state IN ('active', 'redeemed', 'expired', 'disabled', 'disputed')
  ),
  expires_at         TEXT,
  created_at         TEXT NOT NULL,
  updated_at         TEXT NOT NULL,
  archived_at        TEXT
);

CREATE TABLE IF NOT EXISTS referral_attributions (
  id                 TEXT NOT NULL PRIMARY KEY,
  referral_source_id TEXT NOT NULL,
  referral_invite_id TEXT,
  public_source_ref  TEXT NOT NULL,
  public_invite_ref  TEXT,
  capture_path       TEXT NOT NULL CHECK (capture_path IN ('human', 'agent')),
  target             TEXT NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  policy_state       TEXT NOT NULL DEFAULT 'pending' CHECK (
    policy_state IN (
      'pending', 'claimed', 'expired', 'disabled', 'disputed', 'archived'
    )
  ),
  first_verified_at  TEXT,
  claimed_user_id    TEXT,
  expires_at         TEXT NOT NULL,
  created_at         TEXT NOT NULL,
  updated_at         TEXT NOT NULL,
  archived_at        TEXT
);

CREATE TABLE IF NOT EXISTS user_referral_attributions (
  user_id                 TEXT NOT NULL PRIMARY KEY,
  referral_attribution_id TEXT NOT NULL UNIQUE,
  referral_source_id      TEXT NOT NULL,
  referral_invite_id      TEXT,
  capture_path            TEXT NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                  TEXT NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  first_verified_at       TEXT NOT NULL,
  policy_state            TEXT NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at              TEXT NOT NULL,
  updated_at              TEXT NOT NULL,
  archived_at             TEXT
);

CREATE TABLE IF NOT EXISTS order_referral_attributions (
  software_order_id       TEXT NOT NULL PRIMARY KEY,
  user_id                 TEXT NOT NULL,
  referral_attribution_id TEXT NOT NULL,
  referral_source_id      TEXT NOT NULL,
  referral_invite_id      TEXT,
  capture_path            TEXT NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                  TEXT NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  linked_at               TEXT NOT NULL,
  policy_state            TEXT NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at              TEXT NOT NULL,
  updated_at              TEXT NOT NULL,
  archived_at             TEXT
);

CREATE TABLE IF NOT EXISTS agent_referral_attributions (
  agent_user_id           TEXT NOT NULL PRIMARY KEY,
  owner_user_id           TEXT,
  referral_attribution_id TEXT NOT NULL,
  referral_source_id      TEXT NOT NULL,
  referral_invite_id      TEXT,
  capture_path            TEXT NOT NULL
    CHECK (capture_path IN ('human', 'agent')),
  target                  TEXT NOT NULL
    CHECK (target IN ('home', 'order', 'agent_claim')),
  claimed_at              TEXT NOT NULL,
  policy_state            TEXT NOT NULL DEFAULT 'active'
    CHECK (policy_state IN ('active', 'disputed', 'archived')),
  created_at              TEXT NOT NULL,
  updated_at              TEXT NOT NULL,
  archived_at             TEXT
);

CREATE TABLE IF NOT EXISTS referral_workflow_events (
  id                      TEXT NOT NULL PRIMARY KEY,
  idempotency_key         TEXT NOT NULL UNIQUE,
  event_kind              TEXT NOT NULL CHECK (
    event_kind IN (
      'paid_usage', 'site_checkout', 'l402_redemption', 'accepted_outcome',
      'refund', 'reversal', 'eligibility_hold', 'dispute_hold',
      'operator_adjustment'
    )
  ),
  referral_attribution_id TEXT NOT NULL,
  referral_source_id      TEXT NOT NULL,
  referral_invite_id      TEXT,
  public_source_ref       TEXT NOT NULL,
  public_invite_ref       TEXT,
  software_order_id       TEXT,
  site_id                 TEXT,
  site_version_id         TEXT,
  product_id              TEXT,
  paid_action_id          TEXT,
  payment_event_id        TEXT,
  payment_evidence_ref    TEXT,
  entitlement_ref         TEXT,
  accepted_work_ref       TEXT,
  related_event_id        TEXT,
  public_receipt_ref      TEXT NOT NULL,
  policy_state            TEXT NOT NULL CHECK (
    policy_state IN (
      'recorded', 'eligible', 'held', 'disputed', 'refunded', 'reversed',
      'ignored'
    )
  ),
  amount                  NUMERIC NOT NULL DEFAULT 0 CHECK (amount >= 0),
  asset                   TEXT NOT NULL
    CHECK (asset IN ('none', 'credits', 'sats', 'usd')),
  metadata_json           TEXT NOT NULL DEFAULT '{}',
  occurred_at             TEXT NOT NULL,
  created_at              TEXT NOT NULL,
  archived_at             TEXT,
  CHECK (
    event_kind NOT IN ('refund', 'reversal') OR related_event_id IS NOT NULL
  )
);

CREATE TABLE IF NOT EXISTS viral_agent_funnel_events (
  id               TEXT NOT NULL PRIMARY KEY,
  event_kind       TEXT NOT NULL CHECK (
    event_kind IN (
      'capability_manifest_read', 'openapi_read', 'agent_doc_read',
      'skill_doc_read', 'public_proof_read', 'public_challenge_read',
      'first_scoped_action_attempt'
    )
  ),
  route            TEXT NOT NULL,
  actor_class      TEXT NOT NULL CHECK (
    actor_class IN (
      'public_anonymous', 'signed_in_browser_possible',
      'scoped_agent_possible'
    )
  ),
  user_agent_class TEXT NOT NULL
    CHECK (user_agent_class IN ('agent_or_cli', 'browser', 'crawler', 'unknown')),
  site_slug        TEXT,
  proof_ref        TEXT,
  metadata_json    TEXT NOT NULL DEFAULT '{}',
  created_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS qa_swarm_first_engagements (
  receipt_ref                     TEXT NOT NULL PRIMARY KEY,
  idempotency_key                 TEXT NOT NULL UNIQUE,
  package_kind                    TEXT NOT NULL
    CHECK (package_kind IN ('swarm_audit')),
  payment_path                    TEXT NOT NULL CHECK (
    payment_path IN (
      'operator_sales_deposit_invoice', 'checkout_kickoff_receipt'
    )
  ),
  business_signup_request_id      TEXT NOT NULL,
  user_id                         TEXT NOT NULL,
  committed_amount_cents          INTEGER NOT NULL CHECK (
    committed_amount_cents >= 100000 AND committed_amount_cents <= 500000
  ),
  intake_receipt_ref              TEXT NOT NULL,
  checkout_or_deposit_receipt_ref TEXT NOT NULL,
  target_adapter_review_ref       TEXT NOT NULL,
  package_contract_ref            TEXT NOT NULL,
  workspace_id                    TEXT NOT NULL,
  service_promise_contract_id     TEXT NOT NULL,
  commitment_ref                  TEXT NOT NULL UNIQUE,
  first_report_due_at             TEXT NOT NULL,
  recorded_at                     TEXT NOT NULL,
  created_at                      TEXT NOT NULL,
  updated_at                      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS promise_transition_receipts (
  id                 TEXT NOT NULL PRIMARY KEY,
  promise_id         TEXT NOT NULL,
  from_state         TEXT NOT NULL,
  to_state           TEXT NOT NULL,
  registry_version   TEXT NOT NULL,
  result             TEXT NOT NULL,
  checks_json        TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL,
  exception_json     TEXT,
  checked_at         TEXT NOT NULL,
  created_at         TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS buy_mode_campaigns (
  campaign_id          TEXT NOT NULL PRIMARY KEY,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  state                TEXT NOT NULL
    CHECK (state IN ('disabled', 'enabled', 'halted')),
  spend_enabled        INTEGER NOT NULL CHECK (spend_enabled IN (0, 1)),
  per_job_cap_msats    INTEGER NOT NULL CHECK (per_job_cap_msats > 0),
  daily_cap_msats      INTEGER NOT NULL CHECK (daily_cap_msats > 0),
  spent_today_msats    INTEGER NOT NULL DEFAULT 0
    CHECK (spent_today_msats >= 0),
  day_key              TEXT NOT NULL,
  operator_user_id     TEXT NOT NULL,
  relay_url            TEXT NOT NULL,
  last_alert_ref       TEXT,
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS buy_mode_jobs (
  job_id               TEXT NOT NULL PRIMARY KEY,
  campaign_id          TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  request_event_id     TEXT NOT NULL UNIQUE,
  result_event_id      TEXT UNIQUE,
  provider_pubkey      TEXT,
  amount_msats         INTEGER NOT NULL CHECK (amount_msats > 0),
  state                TEXT NOT NULL CHECK (
    state IN ('issued', 'settled', 'settlement_blocked', 'settlement_failed')
  ),
  receipt_ref          TEXT,
  bolt11_ref           TEXT,
  content_digest_ref   TEXT,
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS buy_mode_alerts (
  alert_id    TEXT NOT NULL PRIMARY KEY,
  campaign_id TEXT NOT NULL,
  reason_ref  TEXT NOT NULL,
  created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS customer_one_cohort_rows (
  team_cohort_ref       TEXT NOT NULL PRIMARY KEY,
  state                 TEXT NOT NULL,
  candidate_ref         TEXT,
  invite_ref            TEXT,
  vertical_ref          TEXT,
  template_ref          TEXT,
  workspace_ref         TEXT,
  routing_ref           TEXT,
  run_ref               TEXT,
  artifact_ref          TEXT,
  review_ref            TEXT,
  verification_ref      TEXT,
  completion_bundle_ref TEXT,
  privacy_review_ref    TEXT,
  blocker_refs_json     TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json      TEXT NOT NULL DEFAULT '[]',
  updated_at            TEXT NOT NULL,
  created_at            TEXT NOT NULL
);
`

/**
 * KS-8.17 (#8328): condensed D1 (SQLite) schema for the supervision long-tail
 * domain — the 29 `adjutant_*` / `omni_*` / `autopilot_*` / `relay_health_*` /
 * `backend_incident_events` / `hygiene_debt_receipts` tables. Columns match
 * the shared registry order; D1 uniqueness constraints that the mirror lane
 * deliberately does NOT port to Postgres are KEPT here (D1 is the authority).
 * FKs and CHECK enums are dropped — they are not needed to drive the D1
 * authority in the contract test.
 */
export const SUPERVISION_LONGTAIL_D1_SCHEMA = `
CREATE TABLE adjutant_assignments (
  id TEXT PRIMARY KEY NOT NULL, software_order_id TEXT, site_id TEXT,
  goal_id TEXT, current_run_id TEXT, team_id TEXT, project_id TEXT,
  agent_id TEXT NOT NULL, assigned_by_user_id TEXT, assignment_kind TEXT NOT NULL,
  status TEXT NOT NULL, visibility TEXT NOT NULL, task_spec_path TEXT,
  commit_sha TEXT, objective TEXT NOT NULL, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, completed_at TEXT, blocked_at TEXT, archived_at TEXT
);
CREATE TABLE adjutant_assignment_events (
  id TEXT PRIMARY KEY NOT NULL, assignment_id TEXT NOT NULL,
  software_order_id TEXT, site_id TEXT, goal_id TEXT, run_id TEXT,
  event_type TEXT NOT NULL, visibility TEXT NOT NULL, summary TEXT NOT NULL,
  actor_user_id TEXT, payload_json TEXT, created_at TEXT NOT NULL,
  email_message_id TEXT
);
CREATE TABLE adjutant_adjustment_requests (
  id TEXT PRIMARY KEY NOT NULL, assignment_id TEXT NOT NULL,
  software_order_id TEXT, site_id TEXT NOT NULL, goal_id TEXT,
  requested_by_user_id TEXT, instruction TEXT NOT NULL, status TEXT NOT NULL,
  continuation_mode TEXT, source_run_id TEXT, continuation_run_id TEXT,
  resulting_version_id TEXT, visibility TEXT NOT NULL, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, completed_at TEXT, archived_at TEXT
);
CREATE TABLE adjutant_public_source_refs (
  id TEXT PRIMARY KEY NOT NULL, assignment_id TEXT NOT NULL,
  software_order_id TEXT, site_id TEXT, kind TEXT NOT NULL, status TEXT NOT NULL,
  url TEXT NOT NULL, normalized_domain TEXT NOT NULL, label TEXT,
  public_safe INTEGER NOT NULL DEFAULT 0, proposed_by_user_id TEXT,
  reviewed_by_user_id TEXT, review_reason TEXT, approved_at TEXT,
  rejected_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE adjutant_usage_receipts (
  id TEXT PRIMARY KEY NOT NULL, assignment_id TEXT NOT NULL,
  software_order_id TEXT, site_id TEXT, adjustment_id TEXT, run_id TEXT,
  category TEXT NOT NULL, visibility TEXT NOT NULL, billing_mode TEXT NOT NULL,
  summary TEXT NOT NULL, quantity INTEGER NOT NULL DEFAULT 0, unit TEXT NOT NULL,
  credits_charged_cents INTEGER NOT NULL DEFAULT 0, currency TEXT NOT NULL DEFAULT 'USD',
  billing_ledger_entry_id TEXT, public_receipt_json TEXT NOT NULL DEFAULT '{}',
  team_receipt_json TEXT NOT NULL DEFAULT '{}', idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);
CREATE TABLE adjutant_research_briefs (
  id TEXT PRIMARY KEY NOT NULL, assignment_id TEXT NOT NULL, enrichment_run_id TEXT,
  status TEXT NOT NULL, summary TEXT NOT NULL,
  grounded_facts_json TEXT NOT NULL DEFAULT '[]',
  suggested_sections_json TEXT NOT NULL DEFAULT '[]',
  unknowns_json TEXT NOT NULL DEFAULT '[]',
  claims_needing_review_json TEXT NOT NULL DEFAULT '[]',
  source_cards_json TEXT NOT NULL DEFAULT '[]', created_by_user_id TEXT,
  reviewed_by_user_id TEXT, review_reason TEXT, approved_at TEXT, rejected_at TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE adjutant_assignment_research_policies (
  assignment_id TEXT PRIMARY KEY NOT NULL, policy_mode TEXT NOT NULL,
  reason TEXT NOT NULL, customer_safe_summary TEXT NOT NULL, actor_user_id TEXT,
  source_authority_ref TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE adjutant_enrichment_jobs (
  id TEXT PRIMARY KEY NOT NULL, assignment_id TEXT NOT NULL, enrichment_run_id TEXT,
  status TEXT NOT NULL, trigger_kind TEXT NOT NULL, refresh INTEGER NOT NULL DEFAULT 0,
  requested_by_user_id TEXT, request_json TEXT, error_code TEXT, error_summary TEXT,
  started_at TEXT, completed_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE adjutant_task_packet_freshness (
  assignment_id TEXT PRIMARY KEY NOT NULL, task_spec_path TEXT NOT NULL,
  commit_sha TEXT, status TEXT NOT NULL, research_brief_id TEXT,
  research_brief_approved_at TEXT, source_card_count INTEGER NOT NULL DEFAULT 0,
  operator_keep_reason TEXT, customer_safe_summary TEXT, actor_user_id TEXT,
  stale_at TEXT, kept_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE adjutant_assignment_enrichments (
  assignment_id TEXT NOT NULL, enrichment_run_id TEXT NOT NULL, research_brief_id TEXT,
  status TEXT NOT NULL, required_for_launch INTEGER NOT NULL DEFAULT 0,
  approved_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  PRIMARY KEY (assignment_id, enrichment_run_id)
);
CREATE TABLE omni_accepted_outcome_contracts (
  id TEXT PRIMARY KEY NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
  work_kind TEXT NOT NULL, subject_ref TEXT NOT NULL, customer_ref TEXT,
  expected_artifacts_json TEXT NOT NULL DEFAULT '[]', review_policy TEXT NOT NULL,
  acceptance_state TEXT NOT NULL, proof_policy TEXT NOT NULL, economic_state TEXT NOT NULL,
  closeout_requirements_json TEXT NOT NULL DEFAULT '[]',
  legal_sensitive INTEGER NOT NULL DEFAULT 0, public_receipt_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, archived_at TEXT,
  committed_deliverables_json TEXT NOT NULL DEFAULT '[]',
  service_promise_state TEXT NOT NULL DEFAULT 'not_promised',
  sla_terms_json TEXT NOT NULL DEFAULT '[]',
  fulfillment_receipts_json TEXT NOT NULL DEFAULT '[]'
);
CREATE TABLE omni_workrooms (
  id TEXT PRIMARY KEY NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
  software_order_id TEXT NOT NULL, accepted_outcome_contract_id TEXT, site_id TEXT,
  assignment_id TEXT, work_kind TEXT NOT NULL, status TEXT NOT NULL,
  visibility TEXT NOT NULL, customer_intent_ref TEXT NOT NULL, task_packet_ref TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]', artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  email_refs_json TEXT NOT NULL DEFAULT '[]', receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]', public_receipt_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, archived_at TEXT,
  data_classification TEXT NOT NULL DEFAULT 'customer',
  trust_tier TEXT NOT NULL DEFAULT 'unverified',
  classification_caveat_ref TEXT NOT NULL DEFAULT 'classification_caveat_unreviewed'
);
CREATE TABLE omni_evidence_bundles (
  id TEXT PRIMARY KEY NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL, work_kind TEXT NOT NULL, status TEXT NOT NULL,
  legal_sensitive INTEGER NOT NULL DEFAULT 0, summary_ref TEXT NOT NULL,
  source_authority_caveat_ref TEXT, entries_json TEXT NOT NULL DEFAULT '[]',
  public_receipt_ref TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE omni_workroom_lifecycle_decisions (
  id TEXT PRIMARY KEY NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL, work_kind TEXT NOT NULL, actor_kind TEXT NOT NULL,
  decision_kind TEXT NOT NULL, resulting_state TEXT NOT NULL,
  customer_safe_explanation_ref TEXT NOT NULL, receipt_ref TEXT NOT NULL,
  site_revision_feedback_ref TEXT, followup_request_ref TEXT, artifact_ref TEXT,
  no_settlement_implication INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE omni_accepted_outcome_economics (
  id TEXT PRIMARY KEY NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL, accepted_outcome_contract_id TEXT, work_kind TEXT NOT NULL,
  funding_mode TEXT NOT NULL, buyer_price_asset TEXT NOT NULL,
  buyer_price_cents INTEGER NOT NULL DEFAULT 0, credits_charged INTEGER NOT NULL DEFAULT 0,
  sats_charged INTEGER NOT NULL DEFAULT 0, runner_cost_cents INTEGER NOT NULL DEFAULT 0,
  provider_cost_cents INTEGER NOT NULL DEFAULT 0, retry_cost_cents INTEGER NOT NULL DEFAULT 0,
  review_minutes INTEGER NOT NULL DEFAULT 0, review_cost_cents INTEGER NOT NULL DEFAULT 0,
  artifact_cost_cents INTEGER NOT NULL DEFAULT 0, total_cost_cents INTEGER NOT NULL DEFAULT 0,
  accepted_value_cents INTEGER NOT NULL DEFAULT 0, gross_margin_cents INTEGER NOT NULL DEFAULT 0,
  public_caveat_ref TEXT NOT NULL, internal_caveat_ref TEXT,
  no_settlement_implication INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE omni_route_scorecards (
  id TEXT PRIMARY KEY NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL, work_kind TEXT NOT NULL, selected_route_ref TEXT NOT NULL,
  selected_provider_ref TEXT NOT NULL, selected_account_ref TEXT,
  selected_model_ref TEXT NOT NULL, selected_runtime_ref TEXT NOT NULL,
  rejected_candidates_json TEXT NOT NULL DEFAULT '[]',
  decision_reason_refs_json TEXT NOT NULL DEFAULT '[]', observed_result_kind TEXT NOT NULL,
  observed_result_ref TEXT NOT NULL, post_closeout_score INTEGER,
  cost_cents INTEGER NOT NULL DEFAULT 0, latency_ms INTEGER NOT NULL DEFAULT 0,
  privacy_tier TEXT NOT NULL, trust_tier TEXT NOT NULL, public_caveat_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE omni_public_proof_bundles (
  id TEXT PRIMARY KEY NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL, work_kind TEXT NOT NULL, status TEXT NOT NULL,
  legal_sensitive INTEGER NOT NULL DEFAULT 0, source_refs_json TEXT NOT NULL DEFAULT '[]',
  artifact_refs_json TEXT NOT NULL DEFAULT '[]', receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  review_state_ref TEXT NOT NULL, acceptance_state_ref TEXT NOT NULL,
  economics_caveat_ref TEXT NOT NULL, legal_caveat_ref TEXT, privacy_caveat_ref TEXT NOT NULL,
  public_receipt_ref TEXT NOT NULL, no_settlement_implication INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE omni_market_memory_hooks (
  id TEXT PRIMARY KEY, idempotency_key TEXT NOT NULL UNIQUE, workroom_id TEXT NOT NULL,
  lifecycle_decision_id TEXT NOT NULL, work_kind TEXT NOT NULL, outcome_state TEXT NOT NULL,
  category TEXT NOT NULL, memory_ref TEXT NOT NULL, evidence_ref TEXT NOT NULL,
  source_ref TEXT NOT NULL, public_caveat_ref TEXT NOT NULL, route_scorecard_ref TEXT,
  economics_ref TEXT, authority_boundary TEXT NOT NULL DEFAULT 'evidence_only',
  no_routing_mutation INTEGER NOT NULL DEFAULT 1, no_payout_mutation INTEGER NOT NULL DEFAULT 1,
  no_public_claim_mutation INTEGER NOT NULL DEFAULT 1, no_module_promotion INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE omni_idempotency_keys (
  key TEXT PRIMARY KEY, scope TEXT NOT NULL, result_json TEXT NOT NULL,
  created_at TEXT NOT NULL, expires_at TEXT
);
CREATE TABLE autopilot_token_usage (
  id TEXT PRIMARY KEY, run_id TEXT NOT NULL, event_id TEXT NOT NULL, user_id TEXT NOT NULL,
  team_id TEXT, provider TEXT, model TEXT, input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0, reasoning_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0, cache_write_5m_tokens INTEGER NOT NULL DEFAULT 0,
  cache_write_1h_tokens INTEGER NOT NULL DEFAULT 0, total_tokens INTEGER NOT NULL DEFAULT 0,
  source TEXT NOT NULL, source_ref TEXT NOT NULL, created_at TEXT NOT NULL, account_ref TEXT,
  UNIQUE(run_id, source_ref)
);
CREATE TABLE autopilot_work_orders (
  id TEXT PRIMARY KEY, work_order_ref TEXT NOT NULL UNIQUE, owner_user_id TEXT NOT NULL,
  agent_user_id TEXT NOT NULL, agent_credential_id TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL, client_request_ref TEXT NOT NULL,
  request_json TEXT NOT NULL, state TEXT NOT NULL, task_refs_json TEXT NOT NULL,
  access_request_refs_json TEXT NOT NULL, payment_challenge_ref TEXT,
  status_url_ref TEXT NOT NULL, event_stream_ref TEXT NOT NULL, created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL, archived_at TEXT, buyer_payment_proof_ref TEXT,
  placement_policy_json TEXT, execution_closeout_json TEXT, review_decision_json TEXT,
  scheduled_launch_json TEXT, UNIQUE(owner_user_id, idempotency_key_hash)
);
CREATE TABLE autopilot_decision_closeout_receipts (
  closeout_ref TEXT PRIMARY KEY, decision_ref TEXT NOT NULL, work_order_ref TEXT NOT NULL,
  action TEXT NOT NULL, resolved_state TEXT NOT NULL, outcome TEXT NOT NULL,
  actor_agent_user_id TEXT NOT NULL, decided_at TEXT NOT NULL, receipt_refs_json TEXT NOT NULL,
  has_answer INTEGER NOT NULL DEFAULT 0, line TEXT NOT NULL, receipt_json TEXT NOT NULL
);
CREATE TABLE autopilot_continuation_policies (
  user_id TEXT PRIMARY KEY, enabled INTEGER NOT NULL DEFAULT 0,
  max_continuations_per_run INTEGER NOT NULL DEFAULT 2,
  max_continuations_per_day INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE autopilot_continuation_events (
  id TEXT PRIMARY KEY, user_id TEXT NOT NULL, run_id TEXT NOT NULL, goal_id TEXT,
  mode TEXT NOT NULL, decision TEXT NOT NULL, reason_ref TEXT NOT NULL,
  attempt INTEGER NOT NULL, created_at TEXT NOT NULL, UNIQUE (run_id, attempt)
);
CREATE TABLE autopilot_onboarding_sessions (
  id TEXT PRIMARY KEY NOT NULL, vertical_overlay TEXT,
  status TEXT NOT NULL DEFAULT 'interviewing', transcript_json TEXT NOT NULL DEFAULT '[]',
  output_spec_json TEXT NOT NULL DEFAULT '{}', turn_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE relay_health_probes (
  id TEXT PRIMARY KEY, relay_url TEXT NOT NULL, probed_at TEXT NOT NULL,
  nip11_outcome TEXT NOT NULL, nip11_http_status INTEGER, nip11_latency_ms INTEGER,
  nip11_relay_name TEXT, ws_outcome TEXT NOT NULL, ws_latency_ms INTEGER,
  status TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE relay_health_transitions (
  id TEXT PRIMARY KEY, relay_url TEXT NOT NULL, occurred_at TEXT NOT NULL, kind TEXT NOT NULL,
  from_status TEXT NOT NULL, to_status TEXT NOT NULL, probe_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE backend_incident_events (
  id TEXT PRIMARY KEY, incident_ref TEXT NOT NULL UNIQUE, observed_at TEXT NOT NULL,
  source TEXT NOT NULL, kind TEXT NOT NULL, severity TEXT NOT NULL,
  route_pattern TEXT NOT NULL DEFAULT 'unknown', method TEXT NOT NULL DEFAULT 'UNKNOWN',
  status_code INTEGER, error_name TEXT NOT NULL DEFAULT 'unknown',
  runtime_name TEXT NOT NULL DEFAULT 'cloudflare_workers', occurrence_count INTEGER NOT NULL DEFAULT 1,
  safe_metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL
);
CREATE TABLE hygiene_debt_receipts (
  debt_receipt_key TEXT PRIMARY KEY, state TEXT NOT NULL DEFAULT 'payable',
  debt_receipt_ref TEXT NOT NULL, repo_baseline_ref TEXT NOT NULL, scope_digest TEXT NOT NULL,
  objective_digest TEXT NOT NULL, merged_pr_ref TEXT NOT NULL, reviewer_acceptance_ref TEXT NOT NULL,
  baseline_metric_refs_json TEXT NOT NULL, target_metric_refs_json TEXT NOT NULL,
  verification_command_refs_json TEXT NOT NULL, settlement_authority_actor_ref TEXT,
  budget_cap_sats INTEGER NOT NULL, payable_sats INTEGER NOT NULL, settlement_input_json TEXT NOT NULL,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, retired_at TEXT, settlement_receipt_ref TEXT
);
`

/**
 * KS-8.15 remainder (#8355): D1 schema for the gym / mullet / blueprint /
 * replay-clip / mirrorcode eval remainder (worker migrations 0100/0132/0133/
 * 0136/0208/0233/0239/0240/0246/0256/0266), condensed to the live column set.
 * FK clauses are dropped so the contract suite seeds rows directly; the UNIQUE
 * dedupe keys the INSERT OR IGNORE / converge write paths rely on are kept
 * EXACTLY (harbor artifact_sha256, mutalisk job_ref, blueprint
 * idempotency_key, the mullet hourly/candidate order keys).
 */
export const GYM_EVALS_DOMAIN_D1_SCHEMA = `
CREATE TABLE gym_harbor_full_trace_archives (
  archive_ref TEXT PRIMARY KEY,
  run_ref TEXT NOT NULL,
  job_ref TEXT NOT NULL,
  source_kind TEXT NOT NULL DEFAULT 'harbor_job_tarball',
  artifact_r2_key TEXT NOT NULL,
  artifact_sha256 TEXT NOT NULL UNIQUE,
  artifact_bytes INTEGER NOT NULL,
  content_type TEXT NOT NULL DEFAULT 'application/gzip',
  capture_started_at TEXT,
  capture_completed_at TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'operator_only',
  contains_raw_prompts INTEGER NOT NULL DEFAULT 1,
  contains_raw_logs INTEGER NOT NULL DEFAULT 1,
  contains_private_material INTEGER NOT NULL DEFAULT 1,
  demand_kind TEXT NOT NULL DEFAULT 'internal',
  demand_source TEXT NOT NULL DEFAULT 'harbor_terminal_bench',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE gym_ladder_leaderboard_snapshots (
  ladder_ref TEXT PRIMARY KEY,
  ladder_json TEXT NOT NULL,
  published_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE gym_mutalisk_khala_delegation_jobs (
  run_ref TEXT PRIMARY KEY,
  job_ref TEXT NOT NULL UNIQUE,
  job_json TEXT NOT NULL,
  projection_json TEXT NOT NULL,
  latest_stage TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE gym_mutalisk_khala_delegation_progress (
  run_ref TEXT NOT NULL,
  stage TEXT NOT NULL,
  progress_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_ref, stage)
);
CREATE TABLE gym_mutalisk_khala_delegation_summaries (
  run_ref TEXT PRIMARY KEY,
  candidate_manifest_ref TEXT NOT NULL,
  candidate_ref TEXT NOT NULL,
  summary_json TEXT NOT NULL,
  admission_json TEXT NOT NULL,
  bridge_output_json TEXT NOT NULL,
  metric_value_bps INTEGER NOT NULL,
  admission_decision TEXT NOT NULL,
  ingested_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE gym_run_progress_snapshots (
  run_ref TEXT PRIMARY KEY,
  progress_json TEXT NOT NULL,
  last_updated_at TEXT NOT NULL,
  ingested_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE mullet_scenarios (
  id TEXT PRIMARY KEY,
  owner_user_id TEXT NOT NULL,
  owner_email TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  scenario_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  provenance_summary_json TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'private',
  export_redaction_state TEXT NOT NULL DEFAULT 'not_checked',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);
CREATE TABLE mullet_simulation_runs (
  id TEXT PRIMARY KEY,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  owner_email TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  status TEXT NOT NULL,
  run_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  provenance_summary_json TEXT NOT NULL,
  provider_settlement_state TEXT NOT NULL,
  power_data_state TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'private',
  export_redaction_state TEXT NOT NULL DEFAULT 'not_checked',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  deleted_at TEXT
);
CREATE TABLE mullet_run_hourly_results (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  hour_index INTEGER NOT NULL,
  timestamp TEXT NOT NULL,
  selected_mode TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  energy_mwh REAL NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_mullet_run_hourly_results_run_hour
  ON mullet_run_hourly_results (run_id, hour_index);
CREATE TABLE mullet_run_candidate_modes (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  hourly_result_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  hour_index INTEGER NOT NULL,
  candidate_index INTEGER NOT NULL,
  timestamp TEXT NOT NULL,
  mode TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  risk_adjusted_net_usd_per_mwh REAL NOT NULL,
  clears_readiness INTEGER NOT NULL,
  clears_demand INTEGER NOT NULL,
  clears_provider_floor INTEGER NOT NULL,
  candidate_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_mullet_run_candidate_modes_run_hour_candidate
  ON mullet_run_candidate_modes (run_id, hour_index, candidate_index);
CREATE TABLE mullet_run_exports (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  owner_email TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  format TEXT NOT NULL,
  export_json TEXT NOT NULL,
  private_visibility INTEGER NOT NULL DEFAULT 1,
  redaction_status TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE blueprint_program_runs (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  purpose_ref TEXT NOT NULL,
  program_type_id TEXT NOT NULL,
  program_signature_id TEXT NOT NULL,
  module_version_id TEXT NOT NULL,
  input_snapshot_hash TEXT NOT NULL,
  typed_output_json TEXT NOT NULL DEFAULT '{}',
  confidence REAL NOT NULL,
  route_ref TEXT NOT NULL,
  cost_ref TEXT NOT NULL,
  latency_ms INTEGER NOT NULL,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  authority_boundary TEXT NOT NULL DEFAULT 'evidence_only',
  direct_mutation_disabled INTEGER NOT NULL DEFAULT 1,
  no_deploy INTEGER NOT NULL DEFAULT 1,
  no_email INTEGER NOT NULL DEFAULT 1,
  no_spend INTEGER NOT NULL DEFAULT 1,
  no_source_mutation INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE blueprint_action_submissions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  action_kind TEXT NOT NULL,
  approval_policy_ref TEXT NOT NULL,
  approval_receipt_ref TEXT,
  approval_state TEXT NOT NULL,
  approved_by_ref TEXT,
  content_redacted INTEGER NOT NULL DEFAULT 1,
  context_pack_refs_json TEXT NOT NULL DEFAULT '[]',
  direct_execution INTEGER NOT NULL DEFAULT 0,
  direct_program_run_execution_allowed INTEGER NOT NULL DEFAULT 0,
  dry_run_receipt_ref TEXT,
  dry_run_required INTEGER NOT NULL DEFAULT 1,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  execution_receipt_ref TEXT,
  failure_ref TEXT,
  model_confidence_bypass_disabled INTEGER NOT NULL DEFAULT 1,
  program_run_authority_boundary TEXT NOT NULL DEFAULT 'evidence_only',
  proposal_only INTEGER NOT NULL DEFAULT 1,
  proposed_by_program_run_id TEXT NOT NULL,
  proposed_effect_ref TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  source_authority_refs_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL,
  summary_ref TEXT NOT NULL,
  tool_refs_json TEXT NOT NULL DEFAULT '[]',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE blueprint_probe_contributions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  contribution_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  review_status TEXT NOT NULL,
  release_gate_ready INTEGER NOT NULL DEFAULT 0,
  candidate_runtime_allowed INTEGER NOT NULL DEFAULT 0,
  production_runtime_allowed INTEGER NOT NULL DEFAULT 0,
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  release_gate_refs_json TEXT NOT NULL DEFAULT '[]',
  fixture_refs_json TEXT NOT NULL DEFAULT '[]',
  retained_failure_refs_json TEXT NOT NULL DEFAULT '[]',
  target_refs_json TEXT NOT NULL DEFAULT '[]',
  signature_contribution_json TEXT,
  developer_package_contribution_json TEXT,
  projection_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE replay_clip_jobs (
  job_ref TEXT PRIMARY KEY NOT NULL,
  status TEXT NOT NULL,
  request_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  manifest_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE mirrorcode_runs (
  run_id TEXT PRIMARY KEY,
  run_json TEXT NOT NULL,
  bucket TEXT NOT NULL,
  grade TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);
`

// KS-8.18 (#8329): identity/auth core domain D1 schema (condensed from
// worker migrations 0002/0003/0004/0009/0011/0044-0050/0173/0234/0237/0283
// — final canonical columns only; FKs/CHECKs/UNIQUEs omitted for the
// contract fixture, PKs kept). Secret-bearing columns are present exactly
// as in D1 (no widening).
export const IDENTITY_AUTH_DOMAIN_D1_SCHEMA = `
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  display_name TEXT NOT NULL,
  primary_email TEXT,
  avatar_url TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE auth_identities (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  provider_subject TEXT NOT NULL,
  email TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  provider_username TEXT
);

CREATE TABLE openauth_storage (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  expires_at INTEGER,
  updated_at TEXT NOT NULL
);

CREATE TABLE openauth_agent_links (
  id TEXT PRIMARY KEY,
  openauth_user_id TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  agent_credential_id TEXT,
  link_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE TABLE github_write_connections (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  github_id TEXT NOT NULL,
  github_login TEXT NOT NULL,
  connection_ref TEXT NOT NULL,
  secret_ref TEXT,
  scopes_json TEXT NOT NULL,
  status TEXT NOT NULL,
  health TEXT NOT NULL,
  connected_at TEXT,
  disconnected_at TEXT,
  last_status_at TEXT NOT NULL,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE github_write_connection_attempts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  state TEXT NOT NULL,
  expected_github_id TEXT NOT NULL,
  expected_github_login TEXT NOT NULL,
  redirect_after TEXT,
  scopes_json TEXT NOT NULL,
  status TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  completed_at TEXT,
  failed_at TEXT,
  failure_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE github_write_auth_grants (
  id TEXT PRIMARY KEY,
  connection_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  runner_session_id TEXT,
  connection_ref TEXT NOT NULL,
  secret_ref TEXT NOT NULL,
  grant_ref TEXT NOT NULL,
  status TEXT NOT NULL,
  requested_action TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT,
  revoked_at TEXT,
  failed_at TEXT
);

CREATE TABLE provider_accounts (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL,
  auth_mode TEXT NOT NULL,
  status TEXT NOT NULL,
  health TEXT NOT NULL,
  provider_account_ref TEXT NOT NULL,
  secret_ref TEXT,
  account_label TEXT,
  plan_type TEXT,
  connected_at TEXT,
  disconnected_at TEXT,
  denied_at TEXT,
  last_status_at TEXT NOT NULL,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  last_sanity_check_at TEXT,
  last_sanity_check_result TEXT,
  operator_priority INTEGER NOT NULL DEFAULT 100,
  cooldown_until TEXT,
  low_credit_flag INTEGER NOT NULL DEFAULT 0,
  recent_failure_class TEXT,
  last_selected_at TEXT,
  operator_label TEXT,
  lease_limit INTEGER NOT NULL DEFAULT 1,
  last_parallel_probe_at TEXT,
  last_parallel_probe_result TEXT,
  last_successful_launch_at TEXT,
  last_failed_launch_at TEXT,
  reauth_required_reason TEXT,
  operator_note TEXT,
  refill_note TEXT
);

CREATE TABLE provider_account_connection_attempts (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL,
  method TEXT NOT NULL,
  source TEXT NOT NULL,
  login_ref TEXT,
  verification_url TEXT,
  user_code TEXT,
  status TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  completed_at TEXT,
  failed_at TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE provider_account_auth_grants (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  provider TEXT NOT NULL,
  provider_account_ref TEXT NOT NULL,
  provider_secret_ref TEXT NOT NULL,
  grant_ref TEXT NOT NULL,
  status TEXT NOT NULL,
  requested_action TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT,
  revoked_at TEXT,
  failed_at TEXT
);

CREATE TABLE provider_account_events (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT,
  auth_grant_id TEXT,
  user_id TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT,
  workroom_id TEXT,
  runner_session_id TEXT,
  kind TEXT NOT NULL,
  summary TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL,
  target_ref TEXT,
  metadata_json TEXT,
  actor_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE provider_account_sanity_checks (
  id TEXT PRIMARY KEY,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL,
  provider_account_ref TEXT NOT NULL,
  classification TEXT NOT NULL,
  summary TEXT NOT NULL,
  grant_ref TEXT,
  created_at TEXT NOT NULL,
  metadata_json TEXT
);

CREATE TABLE provider_account_parallel_probe_receipts (
  id TEXT PRIMARY KEY,
  probe_run_id TEXT NOT NULL,
  probe_id TEXT NOT NULL,
  lease_id TEXT NOT NULL,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider_account_ref TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  terminal_status TEXT NOT NULL,
  classification TEXT NOT NULL,
  collision_class TEXT NOT NULL,
  metadata_json TEXT
);

CREATE TABLE provider_account_leases (
  id TEXT PRIMARY KEY,
  lease_ref TEXT NOT NULL,
  provider_account_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  provider TEXT NOT NULL,
  provider_account_ref TEXT NOT NULL,
  requested_action TEXT NOT NULL,
  run_id TEXT,
  assignment_id TEXT,
  selected_by_policy_version TEXT NOT NULL,
  selection_reason TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  released_at TEXT,
  terminal_outcome TEXT,
  metadata_json TEXT,
  order_id TEXT,
  selected_by_actor TEXT,
  last_touched_at TEXT,
  failure_class TEXT
);

CREATE TABLE provider_account_failover_receipts (
  id TEXT PRIMARY KEY,
  run_id TEXT,
  assignment_id TEXT,
  requested_action TEXT NOT NULL,
  previous_lease_ref TEXT,
  previous_provider_account_ref TEXT,
  next_lease_ref TEXT,
  next_provider_account_ref TEXT,
  failure_class TEXT NOT NULL,
  account_state_action TEXT NOT NULL,
  outcome TEXT NOT NULL,
  attempt_number INTEGER NOT NULL,
  max_attempts INTEGER NOT NULL,
  customer_safe_status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  metadata_json TEXT,
  order_id TEXT,
  policy_version TEXT NOT NULL DEFAULT 'provider-account-lease-policy:v1',
  cooldown_until TEXT,
  operator_summary TEXT NOT NULL DEFAULT 'Provider account failover was recorded.',
  customer_safe_summary TEXT
);

CREATE TABLE provider_account_token_custody (
  provider_account_ref TEXT PRIMARY KEY,
  owner_user_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  secret_ref TEXT NOT NULL,
  refresh_ciphertext_b64 TEXT NOT NULL,
  refresh_iv_b64 TEXT NOT NULL,
  refresh_key_id TEXT NOT NULL,
  access_ciphertext_b64 TEXT NOT NULL,
  access_iv_b64 TEXT NOT NULL,
  access_key_id TEXT NOT NULL,
  access_expires_at TEXT NOT NULL,
  account_id TEXT,
  id_token_ciphertext_b64 TEXT,
  id_token_iv_b64 TEXT,
  id_token_key_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_refreshed_at TEXT
);

CREATE TABLE provider_account_token_custody_audit (
  id TEXT PRIMARY KEY,
  provider_account_ref TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  event_kind TEXT NOT NULL,
  status TEXT NOT NULL,
  actor_ref TEXT,
  source_ref TEXT,
  error_tag TEXT,
  error_message TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL
);
`
