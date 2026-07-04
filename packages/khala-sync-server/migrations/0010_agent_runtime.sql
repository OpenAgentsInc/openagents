-- KS-8.5 (#8316): agent runtime metadata domain — Postgres twins of the
-- eight core D1 agent-execution tables: `agent_definitions`,
-- `agent_definition_runs`, `agent_definition_triggers`, `agent_runs`,
-- `agent_run_events`, `agent_traces`, `agent_goals`, `agent_goal_events`
-- (worker migrations 0019/0022/0023/0027/0028/0029/0228/0229/0230/0236/
-- 0279/0280/0281/0282/0284).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.2 (universal porting rules in
-- §1); templates: 0005_pylon_dispatch.sql (KS-8.1) and
-- 0008_token_usage_ledger.sql (KS-8.2).
--
-- SCOPE NOTE: the KS-8.5 issue also names agent_profiles, agent_proposals,
-- agent_owner_claims (+ x-claim challenges), agent_credentials,
-- event_ledger_entries, and khala_acceptance_jobs/verdicts. Those move in
-- the follow-up remainder lane (credentials are secret-bearing — SPEC
-- invariant 9; event_ledger_entries needs its per-owner dense
-- ordering_sequence allocated inside the Postgres transaction). This
-- migration deliberately covers only the definitions/runs/goals/traces
-- METADATA core.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps, 0/1 booleans as smallint,
-- JSON payload columns as text (NOT jsonb: row-hash reconciliation
-- compares exact bytes). Counters are bigint; the stores cast reads with
-- Number(). Tightening to native types is a post-retirement cleanup,
-- never mid-migration.
--
-- IDEMPOTENCY KEYS PORT EXACTLY (MIGRATION_PLAN §1):
--   * agent_run_events: D1 `INSERT OR IGNORE` over PK(id) +
--     UNIQUE(run_id, sequence) + UNIQUE(run_id, external_event_id)
--     collapses to a bare `ON CONFLICT DO NOTHING` (covers all three).
--   * agent_goal_events: PK(id) + partial UNIQUE(goal_id,
--     external_event_id) WHERE external_event_id IS NOT NULL — same bare
--     DO NOTHING port.
--   * agent_traces: the trace dedupe keys feed training-consent and
--     trace-plugin revenue-share surfaces — ported KEY-EXACTLY: partial
--     UNIQUE(owner_user_id, idempotency_key) and partial
--     UNIQUE(owner_user_id, content_digest).
--   * agent_definition_triggers: the live upsert arbiters on
--     UNIQUE(owner_agent_user_id, trigger_ref) and REPLACES trigger_id —
--     the Postgres store uses the same arbiter.
--   * agent_goals: the one-active-goal-per-scope guard is the partial
--     expression unique index (agent_id, COALESCE(user_id,''),
--     COALESCE(team_id,''), COALESCE(project_id,'')) WHERE archived_at IS
--     NULL — ported as an expression index.
--
-- PRIVACY (agent_traces): rows are owner-private by default.
-- `visibility` ('public'|'unlisted'|'owner_only') and `owner_user_id` are
-- ported verbatim and enforced on read exactly as on D1. trajectory_json
-- holds ONLY the public-safe, ingest-tripwired ATIF projection (large
-- bodies live in R2 via trajectory_r2_key) — this table never receives
-- raw prompts, provider payloads, secrets, or PII, and migration
-- diagnostics reference trace_uuid keys only, never payloads.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule):
-- kept indexes serve the live reads (owner listings, due-trigger scan,
-- run/event chains, goal scope reads, trace owner/public/demand feeds).
-- Dropped D1 artifacts: agent_runs_user_created_idx (superseded by the
-- partial active listing index), agent_run_events_run_sequence_idx
-- (redundant with UNIQUE(run_id, sequence)),
-- idx_agent_definition_runs_forge_git_tokens (no read on the Postgres
-- side in this lane; moves with the decommission follow-up if the
-- revocation read re-homes).
--
-- NO FOREIGN KEYS (dual-write mirrors and the backfill land per-row;
-- integrity is verified by reconciliation — same as 0005/0008).

CREATE TABLE IF NOT EXISTS agent_definitions (
  id                   text NOT NULL PRIMARY KEY,
  owner_agent_user_id  text NOT NULL,
  owner_ref            text NOT NULL,
  schema_literal       text NOT NULL,
  name                 text NOT NULL,
  slug                 text NOT NULL,
  goal                 text NOT NULL,
  harness_json         text NOT NULL,
  toolset_json         text NOT NULL,
  triggers_json        text NOT NULL,
  lane                 text NOT NULL,
  budget_json          text NOT NULL,
  escalation_json      text NOT NULL,
  source_refs_json     text NOT NULL,
  definition_json      text NOT NULL,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  archived_at          text,
  UNIQUE (owner_agent_user_id, slug)
);

CREATE INDEX IF NOT EXISTS agent_definitions_owner_updated_idx
  ON agent_definitions (owner_agent_user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS agent_definition_runs (
  run_id                    text NOT NULL PRIMARY KEY,
  owner_agent_user_id       text NOT NULL,
  definition_id             text NOT NULL,
  definition_ref            text NOT NULL,
  trigger_ref               text NOT NULL,
  lane                      text NOT NULL,
  status                    text NOT NULL,
  pylon_ref                 text,
  assignment_ref            text,
  durable_request_id        text NOT NULL,
  durable_stream_url        text,
  forge_tenant_ref          text NOT NULL,
  forge_work_ref            text NOT NULL,
  forge_repository_ref      text,
  forge_git_token_refs_json text NOT NULL DEFAULT '[]',
  refusal_error             text,
  refusal_reason            text,
  evidence_refs_json        text NOT NULL,
  trigger_payload_json      text NOT NULL,
  runtime_run_json          text NOT NULL,
  initial_events_json       text NOT NULL,
  budget_credits_reserved   double precision NOT NULL DEFAULT 0,
  created_at                text NOT NULL,
  updated_at                text NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_definition_runs_owner_definition_updated_idx
  ON agent_definition_runs (owner_agent_user_id, definition_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS agent_definition_runs_assignment_idx
  ON agent_definition_runs (assignment_ref);
CREATE INDEX IF NOT EXISTS agent_definition_runs_forge_work_idx
  ON agent_definition_runs (forge_tenant_ref, forge_work_ref);
-- Daily budget usage read (owner+definition over a created_at window).
CREATE INDEX IF NOT EXISTS agent_definition_runs_owner_definition_created_idx
  ON agent_definition_runs (owner_agent_user_id, definition_id, created_at);

CREATE TABLE IF NOT EXISTS agent_definition_triggers (
  trigger_id           text NOT NULL PRIMARY KEY,
  owner_agent_user_id  text NOT NULL,
  owner_ref            text NOT NULL,
  definition_id        text NOT NULL,
  trigger_ref          text NOT NULL,
  trigger_kind         text NOT NULL,
  trigger_json         text NOT NULL,
  state                text NOT NULL CHECK (state IN ('enabled', 'paused')),
  consecutive_failures bigint NOT NULL DEFAULT 0
    CHECK (consecutive_failures >= 0),
  next_run_at          text,
  paused_at            text,
  pause_reason         text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  UNIQUE (owner_agent_user_id, trigger_ref)
);

CREATE INDEX IF NOT EXISTS agent_definition_triggers_owner_definition_idx
  ON agent_definition_triggers (owner_agent_user_id, definition_id);
-- The AgentDefinitionScheduler.tick due-scan (state + next_run_at range).
CREATE INDEX IF NOT EXISTS agent_definition_triggers_due_idx
  ON agent_definition_triggers (state, next_run_at);
CREATE INDEX IF NOT EXISTS agent_definition_triggers_kind_idx
  ON agent_definition_triggers (trigger_kind);

CREATE TABLE IF NOT EXISTS agent_runs (
  id                   text NOT NULL PRIMARY KEY,
  user_id              text NOT NULL,
  team_id              text,
  project_id           text,
  runtime              text NOT NULL CHECK (runtime IN ('opencode_codex', 'codex')),
  backend              text NOT NULL CHECK (backend IN ('shc_vm', 'gcloud_vm', 'local_fake')),
  runner_id            text NOT NULL,
  assignment_kind      text NOT NULL CHECK (assignment_kind IN ('workroom_agent')),
  repository_provider  text NOT NULL CHECK (repository_provider IN ('github')),
  repository_owner     text NOT NULL,
  repository_repo      text NOT NULL,
  repository_ref       text NOT NULL,
  goal                 text NOT NULL,
  goal_id              text,
  provider_account_ref text,
  auth_grant_ref       text,
  external_run_id      text,
  status               text NOT NULL CHECK (
    status IN ('queued', 'running', 'waiting_for_input', 'completed', 'failed', 'canceled')
  ),
  event_cursor         bigint NOT NULL DEFAULT 0,
  assignment_json      text NOT NULL,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  started_at           text,
  completed_at         text,
  failed_at            text,
  canceled_at          text,
  archived_at          text
);

CREATE INDEX IF NOT EXISTS agent_runs_user_active_created_idx
  ON agent_runs (user_id, created_at)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS agent_runs_status_idx
  ON agent_runs (status);
CREATE INDEX IF NOT EXISTS agent_runs_project_active_created_idx
  ON agent_runs (project_id, created_at)
  WHERE project_id IS NOT NULL AND archived_at IS NULL;
CREATE INDEX IF NOT EXISTS agent_runs_goal_created_idx
  ON agent_runs (goal_id, created_at)
  WHERE goal_id IS NOT NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS agent_run_events (
  id                 text NOT NULL PRIMARY KEY,
  run_id             text NOT NULL,
  sequence           bigint NOT NULL,
  type               text NOT NULL,
  summary            text NOT NULL,
  status             text,
  source             text NOT NULL,
  payload_json       text,
  artifact_refs_json text NOT NULL DEFAULT '[]',
  external_event_id  text,
  created_at         text NOT NULL,
  UNIQUE (run_id, sequence),
  UNIQUE (run_id, external_event_id)
);

CREATE TABLE IF NOT EXISTS agent_traces (
  trace_uuid         text NOT NULL PRIMARY KEY,
  owner_user_id      text NOT NULL,
  agent_ref          text NOT NULL,
  schema_version     text NOT NULL,
  trajectory_id      text NOT NULL,
  session_id         text,
  visibility         text NOT NULL DEFAULT 'unlisted'
    CHECK (visibility IN ('public', 'unlisted', 'owner_only')),
  step_count         bigint NOT NULL DEFAULT 0,
  trajectory_json    text NOT NULL DEFAULT '{}',
  trajectory_r2_key  text,
  blob_refs_json     text NOT NULL DEFAULT '[]',
  idempotency_key    text,
  training_consent   smallint NOT NULL DEFAULT 0 CHECK (training_consent IN (0, 1)),
  license            text,
  content_digest     text,
  reward_eligible    smallint NOT NULL DEFAULT 0 CHECK (reward_eligible IN (0, 1)),
  reward_amount_sats bigint,
  upload_source      text NOT NULL DEFAULT 'agent'
    CHECK (upload_source IN ('agent', 'user_session')),
  demand_kind        text CHECK (
    demand_kind IS NULL
    OR demand_kind IN ('external', 'internal', 'own_capacity', 'unlabeled')
  ),
  demand_source      text,
  created_at         text NOT NULL,
  updated_at         text NOT NULL
);

-- Idempotent ingest: at most one stored trace per (owner, Idempotency-Key).
CREATE UNIQUE INDEX IF NOT EXISTS agent_traces_owner_idempotency_uidx
  ON agent_traces (owner_user_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;
-- Per-owner content dedup (revenue-share: never a second reward per digest).
CREATE UNIQUE INDEX IF NOT EXISTS agent_traces_owner_digest_uidx
  ON agent_traces (owner_user_id, content_digest)
  WHERE content_digest IS NOT NULL;
CREATE INDEX IF NOT EXISTS agent_traces_owner_idx
  ON agent_traces (owner_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS agent_traces_public_idx
  ON agent_traces (visibility, created_at DESC);
CREATE INDEX IF NOT EXISTS agent_traces_demand_kind_idx
  ON agent_traces (demand_kind, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_goals (
  id                text NOT NULL PRIMARY KEY,
  agent_id          text NOT NULL,
  user_id           text,
  team_id           text,
  project_id        text,
  objective         text NOT NULL CHECK (length(objective) > 0),
  status            text NOT NULL CHECK (
    status IN ('active', 'paused', 'blocked', 'usage_limited', 'budget_limited', 'complete')
  ),
  visibility        text NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  current_run_id    text,
  token_budget      bigint CHECK (token_budget IS NULL OR token_budget > 0),
  tokens_used       bigint NOT NULL DEFAULT 0 CHECK (tokens_used >= 0),
  time_used_seconds bigint NOT NULL DEFAULT 0 CHECK (time_used_seconds >= 0),
  created_at        text NOT NULL,
  updated_at        text NOT NULL,
  completed_at      text,
  paused_at         text,
  blocked_at        text,
  archived_at       text
);

-- One ACTIVE goal per scope — the exact D1 partial expression unique.
CREATE UNIQUE INDEX IF NOT EXISTS agent_goals_current_scope_uidx
  ON agent_goals (
    agent_id,
    COALESCE(user_id, ''),
    COALESCE(team_id, ''),
    COALESCE(project_id, '')
  )
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS agent_goals_user_updated_idx
  ON agent_goals (user_id, updated_at)
  WHERE user_id IS NOT NULL AND archived_at IS NULL;
CREATE INDEX IF NOT EXISTS agent_goals_team_project_updated_idx
  ON agent_goals (team_id, project_id, updated_at)
  WHERE team_id IS NOT NULL AND archived_at IS NULL;
CREATE INDEX IF NOT EXISTS agent_goals_public_updated_idx
  ON agent_goals (agent_id, updated_at)
  WHERE visibility = 'public' AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS agent_goal_events (
  id                 text NOT NULL PRIMARY KEY,
  goal_id            text NOT NULL,
  run_id             text,
  expected_goal_id   text,
  caller_type        text NOT NULL
    CHECK (caller_type IN ('agent_tool', 'runtime', 'operator', 'browser')),
  event_type         text NOT NULL,
  status             text CHECK (
    status IS NULL OR status IN (
      'active', 'paused', 'blocked', 'usage_limited', 'budget_limited', 'complete'
    )
  ),
  token_delta        bigint NOT NULL DEFAULT 0,
  time_delta_seconds bigint NOT NULL DEFAULT 0,
  payload_json       text,
  external_event_id  text,
  created_at         text NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_goal_events_goal_sequence_idx
  ON agent_goal_events (goal_id, created_at, id);
CREATE UNIQUE INDEX IF NOT EXISTS agent_goal_events_goal_external_event_uidx
  ON agent_goal_events (goal_id, external_event_id)
  WHERE external_event_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS agent_goal_events_run_idx
  ON agent_goal_events (run_id)
  WHERE run_id IS NOT NULL;
