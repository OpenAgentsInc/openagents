-- KS-8.15 (#8326): training domain CORE — Postgres twins of the seven
-- `training_*` D1 tables: `training_runs`, `training_windows`,
-- `training_window_events`, `training_window_leases`,
-- `training_verification_challenges`, `training_verification_events`,
-- `training_trace_contributions` (worker migrations 0156/0157/0174/0175/
-- 0185/0188).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.12 (universal porting rules
-- in §1); templates: 0010_agent_runtime.sql (KS-8.5) and
-- 0014_forum_content.sql (KS-8.10).
--
-- SCOPE NOTE: the KS-8.15 issue also names `gym_*` (11), `mullet_*` (5),
-- `blueprint_*` (3), `replay_clip_jobs`, and `mirrorcode_runs`. Those move
-- in the follow-up remainder lane (gym_harbor_full_trace_archives needs
-- its R2 payload split confirmed first; gym leaderboard snapshots are
-- derived and verified by recomputation). This migration deliberately
-- covers only the correctness-bearing training core: runs, windows,
-- window leases (double-lease = double-payout risk upstream),
-- verification chains, and trace contributions.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's
-- byte representation — TEXT ISO-8601 timestamps, JSON payload columns as
-- text (NOT jsonb: row-hash reconciliation compares exact bytes; training
-- receipts feed PUBLIC claims and must round-trip byte-exact). Counters
-- are bigint; stores cast reads with Number(). Tightening to native types
-- is a post-retirement cleanup, never mid-migration.
--
-- IDEMPOTENCY / ARBITER KEYS PORT EXACTLY (MIGRATION_PLAN §1):
--   * training_runs / training_windows / training_window_leases /
--     training_verification_challenges: PK(id) plus the UNIQUE ref column
--     (training_run_ref / window_ref / lease_ref / challenge_ref) that
--     every live UPDATE keys on — the Postgres store converges on the ref
--     (the live update arbiter), never on rowid order.
--   * training_window_events / training_verification_events: append-only
--     ledgers, PK(id); the store inserts with a bare ON CONFLICT DO
--     NOTHING (exact-replay dedupe on id).
--   * training_trace_contributions: D1 `INSERT OR IGNORE` keyed by
--     UNIQUE(lease_ref, workload_family) — one pending worker contribution
--     per lease+family; ported KEY-EXACTLY alongside
--     UNIQUE(contribution_ref).
--
-- LEASE SEMANTICS NOTE: in this lane D1 keeps write authority and the
-- lease-claim anti-join stays on D1; the Postgres twin is a byte-exact
-- mirror. At read/write cutover the claim path becomes a real
-- `SELECT ... FOR UPDATE` row-lock transaction (ported deliberately in
-- the cutover follow-up — do NOT emulate the D1 dance then).
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule),
-- all live reads filter `archived_at IS NULL`, hence the partial indexes:
--   kept/re-derived:
--   * training_runs (updated_at DESC) partial — listRuns.
--   * training_windows (training_run_ref, planned_at DESC) partial —
--     listWindowsForRun orders planned_at DESC (D1's index ordered
--     updated_at and never matched the read; re-derived).
--   * training_windows claimable partial (priority DESC, planned_at ASC)
--     WHERE state='active' — the listClaimableWindows scan (homework_kind
--     rank is a CASE expression, deliberately not indexed).
--   * training_window_events (window_ref, created_at, id) — the
--     window/lease chain-equality read.
--   * training_window_leases (training_run_ref, claimed_at DESC) partial —
--     listWindowLeasesForRun (D1 had NO index for this read; added).
--   * training_window_leases (window_ref, state, lease_expires_at)
--     partial — the claimable-scan anti-join probe.
--   * training_verification_challenges (training_run_ref, updated_at
--     DESC) partial — listVerificationChallengesForRun.
--   * training_verification_challenges (verification_class, created_at)
--     partial WHERE state IN ('Queued','Retrying') — listLeaseCandidates
--     (with and without a class filter; created_at ASC order).
--   * training_verification_events (challenge_ref, created_at, id) — the
--     verification-event chain-contiguity read.
--   * training_trace_contributions (training_run_ref, submitted_at)
--     partial WHERE state='pending' — listPendingContributions.
--   * training_trace_contributions (pylon_device_ref, submitted_at DESC)
--     — readMostRecentPylonRefByDeviceRef (D1 had NO index; added).
--   dropped D1 artifacts (no live reader in the Worker):
--   * idx_training_runs_promise_ref — nothing reads runs by promise_ref.
--   * idx_training_window_leases_pylon_ref — nothing lists leases by
--     pylon_ref.
--   * idx_training_window_leases_active (state, lease_expires_at) —
--     superseded by the window-scoped anti-join index above.
--   * idx_training_verification_challenges_window — nothing lists
--     challenges by window_ref; re-derive with the read if one appears.
--
-- NO FOREIGN KEYS (dual-write mirrors and the backfill land per-row;
-- integrity is verified by reconciliation — same as 0005/0008/0010).

CREATE TABLE IF NOT EXISTS training_runs (
  id                               text NOT NULL PRIMARY KEY,
  training_run_ref                 text NOT NULL UNIQUE,
  promise_ref                      text NOT NULL,
  state                            text NOT NULL
    CHECK (state IN ('planned', 'active', 'sealed', 'reconciled')),
  max_allowed_stale                bigint NOT NULL DEFAULT 5,
  seal_publication_cadence_windows bigint NOT NULL DEFAULT 1,
  seal_in_flight_at                text,
  manifest_json                    text,
  source_refs_json                 text NOT NULL,
  receipt_refs_json                text NOT NULL,
  public_projection_json           text NOT NULL,
  created_at                       text NOT NULL,
  updated_at                       text NOT NULL,
  archived_at                      text
);

CREATE INDEX IF NOT EXISTS training_runs_active_updated_idx
  ON training_runs (updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS training_windows (
  id                     text NOT NULL PRIMARY KEY,
  window_ref             text NOT NULL UNIQUE,
  training_run_ref       text NOT NULL,
  state                  text NOT NULL
    CHECK (state IN ('planned', 'active', 'sealed', 'reconciled')),
  homework_kind          text NOT NULL
    CHECK (homework_kind IN (
      'admin_dispatched_homework', 'operator_planned_homework', 'auto_starter'
    )),
  priority               bigint NOT NULL,
  dataset_refs_json      text NOT NULL,
  source_refs_json       text NOT NULL,
  receipt_refs_json      text NOT NULL,
  seal_metadata_json     text,
  public_projection_json text NOT NULL,
  planned_at             text NOT NULL,
  activated_at           text,
  sealed_at              text,
  reconciled_at          text,
  updated_at             text NOT NULL,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS training_windows_run_planned_idx
  ON training_windows (training_run_ref, planned_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS training_windows_claimable_idx
  ON training_windows (priority DESC, planned_at ASC)
  WHERE state = 'active' AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS training_window_events (
  id              text NOT NULL PRIMARY KEY,
  window_ref      text NOT NULL,
  transition_kind text NOT NULL,
  state_from      text,
  state_to        text NOT NULL,
  actor_ref       text NOT NULL,
  receipt_ref     text NOT NULL,
  created_at      text NOT NULL,
  archived_at     text
);

CREATE INDEX IF NOT EXISTS training_window_events_chain_idx
  ON training_window_events (window_ref, created_at, id);

CREATE TABLE IF NOT EXISTS training_window_leases (
  id                     text NOT NULL PRIMARY KEY,
  lease_ref              text NOT NULL UNIQUE,
  window_ref             text NOT NULL,
  training_run_ref       text NOT NULL,
  pylon_ref              text NOT NULL,
  state                  text NOT NULL CHECK (state IN ('active', 'released')),
  receipt_refs_json      text NOT NULL,
  public_projection_json text NOT NULL,
  claimed_at             text NOT NULL,
  lease_expires_at       text NOT NULL,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS training_window_leases_run_claimed_idx
  ON training_window_leases (training_run_ref, claimed_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS training_window_leases_window_state_idx
  ON training_window_leases (window_ref, state, lease_expires_at)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS training_verification_challenges (
  id                     text NOT NULL PRIMARY KEY,
  challenge_ref          text NOT NULL UNIQUE,
  training_run_ref       text NOT NULL,
  window_ref             text,
  contribution_ref       text,
  homework_kind          text NOT NULL,
  verification_class     text NOT NULL
    CHECK (verification_class IN (
      'deterministic_recompute',
      'exact_trace_replay',
      'freivalds_merkle',
      'seeded_replication',
      'statistical_cross_check'
    )),
  sampling_policy        text NOT NULL
    CHECK (sampling_policy IN ('aggregate', 'per_contribution')),
  state                  text NOT NULL
    CHECK (state IN (
      'Queued', 'Leased', 'Retrying', 'Verified', 'Rejected', 'TimedOut'
    )),
  attempt_count          bigint NOT NULL,
  max_attempts           bigint NOT NULL,
  lease_ref              text,
  leased_to_ref          text,
  lease_expires_at       text,
  payload_json           text NOT NULL,
  commitment_refs_json   text NOT NULL,
  failure_codes_json     text NOT NULL,
  verdict_refs_json      text NOT NULL,
  public_projection_json text NOT NULL,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  verified_at            text,
  rejected_at            text,
  timed_out_at           text,
  archived_at            text
);

CREATE INDEX IF NOT EXISTS training_verification_challenges_run_updated_idx
  ON training_verification_challenges (training_run_ref, updated_at DESC)
  WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS training_verification_challenges_lease_scan_idx
  ON training_verification_challenges (verification_class, created_at)
  WHERE state IN ('Queued', 'Retrying') AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS training_verification_events (
  id                 text NOT NULL PRIMARY KEY,
  challenge_ref      text NOT NULL,
  transition_kind    text NOT NULL,
  state_from         text,
  state_to           text NOT NULL,
  validator_ref      text,
  failure_codes_json text NOT NULL,
  receipt_refs_json  text NOT NULL,
  created_at         text NOT NULL,
  archived_at        text
);

CREATE INDEX IF NOT EXISTS training_verification_events_chain_idx
  ON training_verification_events (challenge_ref, created_at, id);

CREATE TABLE IF NOT EXISTS training_trace_contributions (
  id                          text NOT NULL PRIMARY KEY,
  contribution_ref            text NOT NULL UNIQUE,
  lease_ref                   text NOT NULL,
  window_ref                  text NOT NULL,
  training_run_ref            text NOT NULL,
  pylon_ref                   text NOT NULL,
  workload_family             text NOT NULL,
  assignment_ref              text NOT NULL,
  pylon_device_ref            text NOT NULL,
  trace_commitment_digest_ref text NOT NULL,
  sampled_window_ref          text NOT NULL,
  sampled_window_start_step   bigint NOT NULL,
  sampled_window_end_step     bigint NOT NULL,
  worker_receipt_ref          text NOT NULL,
  state                       text NOT NULL
    CHECK (state IN ('pending', 'paired')),
  validator_device_ref        text,
  replay_digest_ref           text,
  verification_challenge_ref  text,
  public_projection_json      text NOT NULL,
  submitted_at                text NOT NULL,
  updated_at                  text NOT NULL,
  archived_at                 text,
  UNIQUE (lease_ref, workload_family)
);

CREATE INDEX IF NOT EXISTS training_trace_contributions_pending_idx
  ON training_trace_contributions (training_run_ref, submitted_at)
  WHERE state = 'pending' AND archived_at IS NULL;
CREATE INDEX IF NOT EXISTS training_trace_contributions_device_idx
  ON training_trace_contributions (pylon_device_ref, submitted_at DESC);
