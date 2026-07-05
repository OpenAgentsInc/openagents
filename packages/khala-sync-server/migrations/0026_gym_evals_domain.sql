-- KS-8.15 remainder (#8355): gym / mullet / blueprint / replay-clip /
-- mirrorcode eval domain — Postgres twins of the 21 D1 tables the training
-- CORE lane (#8326, migration 0019_training_domain.sql) deliberately left for
-- this follow-up. Plan: docs/khala-sync/MIGRATION_PLAN.md §3.12 (universal
-- porting rules in §1). Template: 0019_training_domain.sql — same registry /
-- dual-write / backfill machinery, extended here.
--
-- TABLES (21):
--   gym_* (11):
--     gym_agentcl_eval_runs, gym_agentcl_eval_phase_metrics,
--     gym_agentcl_eval_gain_metrics, gym_agentcl_eval_run_state_events,
--     gym_agentcl_eval_prompt_mutations, gym_harbor_full_trace_archives,
--     gym_ladder_leaderboard_snapshots,
--     gym_mutalisk_khala_delegation_jobs,
--     gym_mutalisk_khala_delegation_progress,
--     gym_mutalisk_khala_delegation_summaries, gym_run_progress_snapshots
--   mullet_* (5):
--     mullet_scenarios, mullet_simulation_runs, mullet_run_hourly_results,
--     mullet_run_candidate_modes, mullet_run_exports
--   blueprint_* (3):
--     blueprint_program_runs, blueprint_action_submissions,
--     blueprint_probe_contributions
--   replay_clip_jobs, mirrorcode_runs
--
-- R2 PAYLOAD SPLIT (confirmed before porting — the issue's gate):
--   gym_harbor_full_trace_archives carries ONLY refs/metadata. The archive
--   BODY (the harbor job tarball) is written to R2 by
--   makeD1R2HarborFullTraceArchiveStore.putArchive (bucket.put), and D1 keeps
--   only artifact_r2_key + artifact_sha256 + artifact_bytes. The D1 row is
--   therefore fully portable; this migration carries the metadata twin and
--   NEVER an archive body. No table is skipped — every remainder table's D1
--   row is refs/metadata/public-safe-projection JSON, so all 21 port.
--
-- LEADERBOARD SNAPSHOTS ARE DERIVED — VERIFY BY COPY-EQUALITY, DO NOT
-- RECOMPUTE IN POSTGRES: gym_ladder_leaderboard_snapshots.ladder_json and
-- gym_run_progress_snapshots.progress_json already hold the public-safe
-- projection the D1 write path built (buildGymLadderLeaderboard /
-- buildGymRunProgress, public-safety-asserted BEFORE storage). The backfill
-- copies those bytes verbatim and the verifier proves "leaderboard
-- recomputation equality" as newest-N full-row sha256 equality between the
-- stores — Postgres never recomputes a leaderboard (the acceptance from
-- #8326: verify-compare, don't recompute).
--
-- WRITE-DEAD (KS-8.17 short path): the five gym_agentcl_eval_* tables have NO
-- live production writer in the Worker (only fixtures/migration tests touch
-- them). They get NO dual-write phase — the backfill lands + verifies a
-- byte-exact copy here, and the destructive snapshot-to-R2 + D1 drop stays in
-- KS-8.19 (#8330). Their Postgres twins exist so the copy can land and verify.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): TEXT stays text, JSON payload
-- columns stay text (NOT jsonb — row-hash reconciliation compares exact
-- bytes; gym/ladder/mirrorcode rows feed PUBLIC projections and must
-- round-trip byte-exact). INTEGER counters/flags become bigint (stores cast
-- reads with Number()). The genuinely-numeric REAL columns (mullet energy /
-- risk-adjusted net, agentcl gain/rate) become double precision; the
-- registry's normalizeValue compares them by JS Number identity, the same as
-- the training verifier. Tightening to native types is post-retirement
-- cleanup, never mid-migration.
--
-- IDEMPOTENCY / ARBITER KEYS PORT EXACTLY (MIGRATION_PLAN §1) — each table's
-- keyColumns in gym-evals-domain-tables.ts is the SAME unique key the live D1
-- writer converges on (PK, the upsert ON CONFLICT target, or the composite
-- PK). Append-only / insert-once tables (harbor archives, agentcl event +
-- mutation ledgers, mullet child rows) use insert-if-absent (bare ON CONFLICT
-- DO NOTHING) so exact replay is a no-op that never clobbers.
--
-- INDEXES ARE RE-DERIVED FROM THE LIVE READS (the KS-8.2 rule) and ported
-- from D1; D1 partial indexes (WHERE deleted_at IS NULL / archived_at) port
-- as Postgres partial indexes. Per-table justification is inline below.
--
-- NO FOREIGN KEYS (dual-write mirrors + backfill land per-row; integrity is
-- proven by reconciliation — same as 0005/0008/0010/0019). CHECK constraints
-- that encode public-safety invariants (collapse_gains=0, contains_*=1,
-- visibility/private_visibility guards) are kept: source rows already satisfy
-- them and they harden the twin.

-- ===========================================================================
-- gym_agentcl_eval_* (write-dead; copy + verify only) — worker migration 0256
-- ===========================================================================

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_runs (
  eval_ref                        text NOT NULL PRIMARY KEY,
  schema_version                  text NOT NULL,
  environment_ref                 text NOT NULL,
  experiment_id                   text NOT NULL,
  stream_kind                     text NOT NULL
    CHECK (stream_kind IN ('naive', 'compositional')),
  run_ref                         text NOT NULL,
  task_set_ref                    text,
  verifier_ref                    text,
  runner_config_id                text,
  seam_id                         text,
  seam_can_spend                  bigint NOT NULL DEFAULT 0
    CHECK (seam_can_spend IN (0, 1)),
  state                           text NOT NULL
    CHECK (state IN ('planned', 'running', 'completed', 'aborted', 'rejected')),
  decision_grade                  bigint NOT NULL DEFAULT 0
    CHECK (decision_grade IN (0, 1)),
  public_claim_eligible           bigint NOT NULL DEFAULT 0
    CHECK (public_claim_eligible IN (0, 1)),
  collapse_gains_into_one_number  bigint NOT NULL DEFAULT 0
    CHECK (collapse_gains_into_one_number = 0),
  run_metadata_json               text NOT NULL,
  proof_refs_json                 text NOT NULL,
  caveat_refs_json                text NOT NULL,
  blocker_refs_json               text NOT NULL,
  started_at                      text,
  completed_at                    text,
  created_at                      text NOT NULL,
  updated_at                      text NOT NULL
);

-- listRuns orders (updated_at DESC, eval_ref ASC); experiment lens filters
-- experiment_id then updated_at DESC. Both ported.
CREATE INDEX IF NOT EXISTS gym_agentcl_eval_runs_updated_idx
  ON gym_agentcl_eval_runs (updated_at DESC, eval_ref ASC);
CREATE INDEX IF NOT EXISTS gym_agentcl_eval_runs_experiment_idx
  ON gym_agentcl_eval_runs (experiment_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_phase_metrics (
  eval_ref             text NOT NULL,
  phase                text NOT NULL
    CHECK (phase IN (
      'baseline', 'first_pass', 'frozen_second_pass',
      'held_out_baseline', 'held_out_pass'
    )),
  task_role            text NOT NULL
    CHECK (task_role IN ('source', 'complex', 'held_out')),
  task_count           bigint NOT NULL CHECK (task_count > 0),
  accepted_outcome_rate double precision NOT NULL
    CHECK (accepted_outcome_rate >= 0 AND accepted_outcome_rate <= 1),
  score_bps            bigint NOT NULL CHECK (score_bps >= 0 AND score_bps <= 10000),
  report_ref           text,
  receipt_ref          text,
  metric_metadata_json text NOT NULL DEFAULT '{}',
  created_at           text NOT NULL,
  PRIMARY KEY (eval_ref, phase)
);

-- The per-role phase lens read (task_role, phase). Ported.
CREATE INDEX IF NOT EXISTS gym_agentcl_eval_phase_metrics_role_idx
  ON gym_agentcl_eval_phase_metrics (task_role, phase);

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_gain_metrics (
  eval_ref             text NOT NULL,
  gain_kind            text NOT NULL
    CHECK (gain_kind IN ('plasticity', 'stability', 'generalization')),
  gain_value           double precision NOT NULL,
  gain_bps             bigint NOT NULL,
  baseline_phase       text NOT NULL,
  comparison_phase     text NOT NULL,
  evidence_refs_json   text NOT NULL,
  metric_metadata_json text NOT NULL DEFAULT '{}',
  created_at           text NOT NULL,
  PRIMARY KEY (eval_ref, gain_kind)
);
-- Read only by eval_ref (the PK prefix) — no secondary index in D1, none added.

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_run_state_events (
  event_ref           text NOT NULL PRIMARY KEY,
  eval_ref            text NOT NULL,
  event_index         bigint NOT NULL CHECK (event_index >= 0),
  state               text NOT NULL
    CHECK (state IN ('planned', 'running', 'completed', 'aborted', 'rejected')),
  observed_at         text NOT NULL,
  state_metadata_json text NOT NULL DEFAULT '{}'
);

-- The append-only per-run ordering (eval_ref, event_index) — UNIQUE in D1;
-- ported UNIQUE so exact replay dedupes on it too.
CREATE UNIQUE INDEX IF NOT EXISTS gym_agentcl_eval_run_state_events_order_idx
  ON gym_agentcl_eval_run_state_events (eval_ref, event_index);

CREATE TABLE IF NOT EXISTS gym_agentcl_eval_prompt_mutations (
  mutation_ref            text NOT NULL PRIMARY KEY,
  eval_ref                text NOT NULL,
  run_ref                 text NOT NULL,
  pass                    text NOT NULL
    CHECK (pass IN ('baseline', 'first_pass', 'frozen_second_pass', 'held_out_pass')),
  task_ref                text NOT NULL,
  step_index              bigint NOT NULL CHECK (step_index >= 0),
  template_ref            text NOT NULL,
  memory_before_refs_json text NOT NULL,
  memory_after_refs_json  text NOT NULL,
  feedback_ref            text NOT NULL,
  mutation_json           text NOT NULL,
  created_at              text NOT NULL
);

-- Ordered replay per run (eval_ref, step_index, mutation_ref) and the
-- per-task-pass read (task_ref, pass, step_index). Both ported.
CREATE INDEX IF NOT EXISTS gym_agentcl_eval_prompt_mutations_eval_step_idx
  ON gym_agentcl_eval_prompt_mutations (eval_ref, step_index ASC, mutation_ref ASC);
CREATE INDEX IF NOT EXISTS gym_agentcl_eval_prompt_mutations_task_idx
  ON gym_agentcl_eval_prompt_mutations (task_ref, pass, step_index ASC);

-- ===========================================================================
-- gym_harbor_full_trace_archives (R2 body split — metadata twin) — 0239
-- ===========================================================================

CREATE TABLE IF NOT EXISTS gym_harbor_full_trace_archives (
  archive_ref              text NOT NULL PRIMARY KEY,
  run_ref                  text NOT NULL,
  job_ref                  text NOT NULL,
  source_kind              text NOT NULL DEFAULT 'harbor_job_tarball'
    CHECK (source_kind = 'harbor_job_tarball'),
  artifact_r2_key          text NOT NULL,
  artifact_sha256          text NOT NULL UNIQUE,
  artifact_bytes           bigint NOT NULL CHECK (artifact_bytes > 0),
  content_type             text NOT NULL DEFAULT 'application/gzip',
  capture_started_at       text,
  capture_completed_at     text NOT NULL,
  visibility               text NOT NULL DEFAULT 'operator_only'
    CHECK (visibility = 'operator_only'),
  contains_raw_prompts     bigint NOT NULL DEFAULT 1
    CHECK (contains_raw_prompts = 1),
  contains_raw_logs        bigint NOT NULL DEFAULT 1
    CHECK (contains_raw_logs = 1),
  contains_private_material bigint NOT NULL DEFAULT 1
    CHECK (contains_private_material = 1),
  demand_kind              text NOT NULL DEFAULT 'internal'
    CHECK (demand_kind = 'internal'),
  demand_source            text NOT NULL DEFAULT 'harbor_terminal_bench',
  created_at               text NOT NULL,
  updated_at               text NOT NULL
);

-- listArchives(runRef) filter + capture_completed_at DESC. Ported. The
-- job-scoped variant is kept for the operator job-trace read.
CREATE INDEX IF NOT EXISTS gym_harbor_full_trace_archives_run_capture_idx
  ON gym_harbor_full_trace_archives (run_ref, capture_completed_at DESC);
CREATE INDEX IF NOT EXISTS gym_harbor_full_trace_archives_job_capture_idx
  ON gym_harbor_full_trace_archives (job_ref, capture_completed_at DESC);

-- ===========================================================================
-- gym_ladder_leaderboard_snapshots (derived) — 0240
-- ===========================================================================

CREATE TABLE IF NOT EXISTS gym_ladder_leaderboard_snapshots (
  ladder_ref   text NOT NULL PRIMARY KEY,
  ladder_json  text NOT NULL,
  published_at text NOT NULL,
  created_at   text NOT NULL
);

-- The latest-snapshot read (published_at DESC, ladder_ref ASC). Ported.
CREATE INDEX IF NOT EXISTS gym_ladder_leaderboard_snapshots_published_idx
  ON gym_ladder_leaderboard_snapshots (published_at DESC, ladder_ref ASC);

-- ===========================================================================
-- gym_mutalisk_khala_delegation_* — 0266
-- ===========================================================================

CREATE TABLE IF NOT EXISTS gym_mutalisk_khala_delegation_jobs (
  run_ref         text NOT NULL PRIMARY KEY,
  job_ref         text NOT NULL UNIQUE,
  job_json        text NOT NULL,
  projection_json text NOT NULL,
  latest_stage    text NOT NULL,
  updated_at      text NOT NULL,
  created_at      text NOT NULL
);

-- listRunProjections orders (updated_at DESC, run_ref ASC). Ported.
CREATE INDEX IF NOT EXISTS gym_mutalisk_khala_delegation_jobs_updated_idx
  ON gym_mutalisk_khala_delegation_jobs (updated_at DESC, run_ref ASC);

CREATE TABLE IF NOT EXISTS gym_mutalisk_khala_delegation_progress (
  run_ref       text NOT NULL,
  stage         text NOT NULL,
  progress_json text NOT NULL,
  updated_at    text NOT NULL,
  PRIMARY KEY (run_ref, stage)
);

CREATE INDEX IF NOT EXISTS gym_mutalisk_khala_delegation_progress_updated_idx
  ON gym_mutalisk_khala_delegation_progress (updated_at DESC, run_ref ASC);

CREATE TABLE IF NOT EXISTS gym_mutalisk_khala_delegation_summaries (
  run_ref                text NOT NULL PRIMARY KEY,
  candidate_manifest_ref text NOT NULL,
  candidate_ref          text NOT NULL,
  summary_json           text NOT NULL,
  admission_json         text NOT NULL,
  bridge_output_json     text NOT NULL,
  metric_value_bps       bigint NOT NULL,
  admission_decision     text NOT NULL,
  ingested_at            text NOT NULL,
  updated_at             text NOT NULL
);

-- Candidate lens read (candidate_manifest_ref, candidate_ref). Ported.
CREATE INDEX IF NOT EXISTS gym_mutalisk_khala_delegation_summaries_candidate_idx
  ON gym_mutalisk_khala_delegation_summaries (candidate_manifest_ref, candidate_ref);

-- ===========================================================================
-- gym_run_progress_snapshots (derived) — 0233
-- ===========================================================================

CREATE TABLE IF NOT EXISTS gym_run_progress_snapshots (
  run_ref         text NOT NULL PRIMARY KEY,
  progress_json   text NOT NULL,
  last_updated_at text NOT NULL,
  ingested_at     text NOT NULL,
  created_at      text NOT NULL
);

-- listRunProgress orders (last_updated_at DESC, run_ref ASC). Ported.
CREATE INDEX IF NOT EXISTS gym_run_progress_snapshots_last_updated_idx
  ON gym_run_progress_snapshots (last_updated_at DESC, run_ref ASC);

-- ===========================================================================
-- mullet_* (owner-private simulations) — 0136
-- ===========================================================================

CREATE TABLE IF NOT EXISTS mullet_scenarios (
  id                       text NOT NULL PRIMARY KEY,
  owner_user_id            text NOT NULL,
  owner_email              text NOT NULL,
  schema_version           text NOT NULL,
  name                     text NOT NULL,
  kind                     text NOT NULL,
  scenario_json            text NOT NULL,
  source_refs_json         text NOT NULL,
  provenance_summary_json  text NOT NULL,
  visibility               text NOT NULL DEFAULT 'private'
    CHECK (visibility = 'private'),
  export_redaction_state   text NOT NULL DEFAULT 'not_checked'
    CHECK (export_redaction_state IN ('not_checked', 'passed', 'failed')),
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  deleted_at               text
);

-- listScenarios(ownerUserId) orders updated_at DESC over live rows. Partial
-- index ported.
CREATE INDEX IF NOT EXISTS mullet_scenarios_owner_updated_idx
  ON mullet_scenarios (owner_user_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS mullet_simulation_runs (
  id                        text NOT NULL PRIMARY KEY,
  scenario_id               text NOT NULL,
  owner_user_id             text NOT NULL,
  owner_email               text NOT NULL,
  schema_version            text NOT NULL,
  status                    text NOT NULL
    CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
  run_json                  text NOT NULL,
  source_refs_json          text NOT NULL,
  provenance_summary_json   text NOT NULL,
  provider_settlement_state text NOT NULL,
  power_data_state          text NOT NULL,
  visibility                text NOT NULL DEFAULT 'private'
    CHECK (visibility = 'private'),
  export_redaction_state    text NOT NULL DEFAULT 'not_checked'
    CHECK (export_redaction_state IN ('not_checked', 'passed', 'failed')),
  created_at                text NOT NULL,
  updated_at                text NOT NULL,
  completed_at              text,
  deleted_at                text
);

CREATE INDEX IF NOT EXISTS mullet_simulation_runs_owner_updated_idx
  ON mullet_simulation_runs (owner_user_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS mullet_simulation_runs_scenario_idx
  ON mullet_simulation_runs (scenario_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS mullet_run_hourly_results (
  id            text NOT NULL PRIMARY KEY,
  run_id        text NOT NULL,
  scenario_id   text NOT NULL,
  owner_user_id text NOT NULL,
  hour_index    bigint NOT NULL,
  timestamp     text NOT NULL,
  selected_mode text NOT NULL,
  reason_code   text NOT NULL,
  energy_mwh    double precision NOT NULL,
  result_json   text NOT NULL,
  created_at    text NOT NULL
);

-- The per-run ordered read (run_id, hour_index) — UNIQUE in D1; ported UNIQUE.
CREATE UNIQUE INDEX IF NOT EXISTS mullet_run_hourly_results_run_hour_idx
  ON mullet_run_hourly_results (run_id, hour_index);

CREATE TABLE IF NOT EXISTS mullet_run_candidate_modes (
  id                           text NOT NULL PRIMARY KEY,
  run_id                       text NOT NULL,
  hourly_result_id             text NOT NULL,
  scenario_id                  text NOT NULL,
  owner_user_id                text NOT NULL,
  hour_index                   bigint NOT NULL,
  candidate_index              bigint NOT NULL,
  timestamp                    text NOT NULL,
  mode                         text NOT NULL,
  reason_code                  text NOT NULL,
  risk_adjusted_net_usd_per_mwh double precision NOT NULL,
  clears_readiness             bigint NOT NULL CHECK (clears_readiness IN (0, 1)),
  clears_demand                bigint NOT NULL CHECK (clears_demand IN (0, 1)),
  clears_provider_floor        bigint NOT NULL CHECK (clears_provider_floor IN (0, 1)),
  candidate_json               text NOT NULL,
  created_at                   text NOT NULL
);

-- listRunCandidateModes reads (run_id, hour_index, candidate_index). D1 had a
-- UNIQUE and a redundant non-unique copy of the same tuple; the UNIQUE alone
-- serves the read, so the redundant D1 index is DROPPED here.
CREATE UNIQUE INDEX IF NOT EXISTS mullet_run_candidate_modes_run_hour_candidate_idx
  ON mullet_run_candidate_modes (run_id, hour_index, candidate_index);

CREATE TABLE IF NOT EXISTS mullet_run_exports (
  id                 text NOT NULL PRIMARY KEY,
  run_id             text NOT NULL,
  scenario_id        text NOT NULL,
  owner_user_id      text NOT NULL,
  owner_email        text NOT NULL,
  schema_version     text NOT NULL,
  format             text NOT NULL CHECK (format IN ('markdown', 'json')),
  export_json        text NOT NULL,
  private_visibility bigint NOT NULL DEFAULT 1 CHECK (private_visibility = 1),
  redaction_status   text NOT NULL
    CHECK (redaction_status IN ('not_checked', 'passed', 'failed')),
  content_ref        text NOT NULL,
  created_at         text NOT NULL
);

-- getLatestRunExport reads (run_id, created_at DESC). Ported.
CREATE INDEX IF NOT EXISTS mullet_run_exports_run_idx
  ON mullet_run_exports (run_id, created_at DESC);

-- ===========================================================================
-- blueprint_* (program runs + proposals) — 0100 / 0132 / 0133
-- ===========================================================================

CREATE TABLE IF NOT EXISTS blueprint_program_runs (
  id                      text NOT NULL PRIMARY KEY,
  idempotency_key         text NOT NULL UNIQUE,
  actor_ref               text NOT NULL,
  purpose_ref             text NOT NULL,
  program_type_id         text NOT NULL,
  program_signature_id    text NOT NULL,
  module_version_id       text NOT NULL,
  input_snapshot_hash     text NOT NULL,
  typed_output_json       text NOT NULL DEFAULT '{}',
  confidence              double precision NOT NULL,
  route_ref               text NOT NULL,
  cost_ref                text NOT NULL,
  latency_ms              bigint NOT NULL,
  evidence_refs_json      text NOT NULL DEFAULT '[]',
  receipt_refs_json       text NOT NULL DEFAULT '[]',
  authority_boundary      text NOT NULL DEFAULT 'evidence_only',
  direct_mutation_disabled bigint NOT NULL DEFAULT 1,
  no_deploy               bigint NOT NULL DEFAULT 1,
  no_email                bigint NOT NULL DEFAULT 1,
  no_spend                bigint NOT NULL DEFAULT 1,
  no_source_mutation      bigint NOT NULL DEFAULT 1,
  metadata_json           text NOT NULL DEFAULT '{}',
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  archived_at             text
);

-- Program-run lenses filter on (program_signature_id, archived_at) and
-- (module_version_id, archived_at). Ported.
CREATE INDEX IF NOT EXISTS blueprint_program_runs_signature_idx
  ON blueprint_program_runs (program_signature_id, archived_at);
CREATE INDEX IF NOT EXISTS blueprint_program_runs_module_version_idx
  ON blueprint_program_runs (module_version_id, archived_at);

CREATE TABLE IF NOT EXISTS blueprint_action_submissions (
  id                                    text NOT NULL PRIMARY KEY,
  idempotency_key                       text NOT NULL UNIQUE,
  action_kind                           text NOT NULL,
  approval_policy_ref                   text NOT NULL,
  approval_receipt_ref                  text,
  approval_state                        text NOT NULL,
  approved_by_ref                       text,
  content_redacted                      bigint NOT NULL DEFAULT 1,
  context_pack_refs_json                text NOT NULL DEFAULT '[]',
  direct_execution                      bigint NOT NULL DEFAULT 0,
  direct_program_run_execution_allowed  bigint NOT NULL DEFAULT 0,
  dry_run_receipt_ref                   text,
  dry_run_required                      bigint NOT NULL DEFAULT 1,
  evidence_refs_json                    text NOT NULL DEFAULT '[]',
  execution_receipt_ref                 text,
  failure_ref                           text,
  model_confidence_bypass_disabled      bigint NOT NULL DEFAULT 1,
  program_run_authority_boundary        text NOT NULL DEFAULT 'evidence_only',
  proposal_only                         bigint NOT NULL DEFAULT 1,
  proposed_by_program_run_id            text NOT NULL,
  proposed_effect_ref                   text NOT NULL,
  receipt_refs_json                     text NOT NULL DEFAULT '[]',
  source_authority_refs_json            text NOT NULL DEFAULT '[]',
  status                                text NOT NULL,
  summary_ref                           text NOT NULL,
  tool_refs_json                        text NOT NULL DEFAULT '[]',
  metadata_json                         text NOT NULL DEFAULT '{}',
  created_at                            text NOT NULL,
  updated_at                            text NOT NULL,
  archived_at                           text
);

-- Lenses: by proposing program run, by status, by action kind — each with
-- archived_at. Ported.
CREATE INDEX IF NOT EXISTS blueprint_action_submissions_program_run_idx
  ON blueprint_action_submissions (proposed_by_program_run_id, archived_at);
CREATE INDEX IF NOT EXISTS blueprint_action_submissions_status_idx
  ON blueprint_action_submissions (status, archived_at);
CREATE INDEX IF NOT EXISTS blueprint_action_submissions_action_kind_idx
  ON blueprint_action_submissions (action_kind, archived_at);

CREATE TABLE IF NOT EXISTS blueprint_probe_contributions (
  id                                    text NOT NULL PRIMARY KEY,
  idempotency_key                       text NOT NULL UNIQUE,
  contribution_kind                     text NOT NULL,
  status                                text NOT NULL,
  review_status                         text NOT NULL,
  release_gate_ready                    bigint NOT NULL DEFAULT 0,
  candidate_runtime_allowed             bigint NOT NULL DEFAULT 0,
  production_runtime_allowed            bigint NOT NULL DEFAULT 0,
  blocker_refs_json                     text NOT NULL DEFAULT '[]',
  release_gate_refs_json                text NOT NULL DEFAULT '[]',
  fixture_refs_json                     text NOT NULL DEFAULT '[]',
  retained_failure_refs_json            text NOT NULL DEFAULT '[]',
  target_refs_json                      text NOT NULL DEFAULT '[]',
  signature_contribution_json           text,
  developer_package_contribution_json   text,
  projection_json                       text NOT NULL DEFAULT '{}',
  metadata_json                         text NOT NULL DEFAULT '{}',
  created_at                            text NOT NULL,
  updated_at                            text NOT NULL,
  archived_at                           text
);

-- Lenses: by kind, by release-gate readiness, by (status, review_status) —
-- each with archived_at. Ported.
CREATE INDEX IF NOT EXISTS blueprint_probe_contributions_kind_idx
  ON blueprint_probe_contributions (contribution_kind, archived_at);
CREATE INDEX IF NOT EXISTS blueprint_probe_contributions_release_gate_idx
  ON blueprint_probe_contributions (release_gate_ready, archived_at);
CREATE INDEX IF NOT EXISTS blueprint_probe_contributions_status_idx
  ON blueprint_probe_contributions (status, review_status, archived_at);

-- ===========================================================================
-- replay_clip_jobs — 0208
-- ===========================================================================

CREATE TABLE IF NOT EXISTS replay_clip_jobs (
  job_ref          text NOT NULL PRIMARY KEY,
  status           text NOT NULL
    CHECK (status IN ('queued', 'rendering', 'succeeded', 'failed', 'blocked')),
  request_json     text NOT NULL,
  source_refs_json text NOT NULL DEFAULT '[]',
  caveat_refs_json text NOT NULL DEFAULT '[]',
  blocker_refs_json text NOT NULL DEFAULT '[]',
  manifest_ref     text,
  created_at       text NOT NULL,
  updated_at       text NOT NULL
);

-- The render box claims by status; listRecent orders updated_at DESC. Both
-- ported.
CREATE INDEX IF NOT EXISTS replay_clip_jobs_status_idx
  ON replay_clip_jobs (status, updated_at DESC);
CREATE INDEX IF NOT EXISTS replay_clip_jobs_updated_idx
  ON replay_clip_jobs (updated_at DESC);

-- ===========================================================================
-- mirrorcode_runs (derived) — 0246
-- ===========================================================================

CREATE TABLE IF NOT EXISTS mirrorcode_runs (
  run_id     text NOT NULL PRIMARY KEY,
  run_json   text NOT NULL,
  bucket     text NOT NULL,
  grade      text NOT NULL,
  status     text NOT NULL,
  started_at text NOT NULL,
  updated_at text NOT NULL,
  created_at text NOT NULL
);

-- listRuns orders (started_at DESC, run_id ASC). Ported.
CREATE INDEX IF NOT EXISTS mirrorcode_runs_started_idx
  ON mirrorcode_runs (started_at DESC, run_id ASC);
