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
