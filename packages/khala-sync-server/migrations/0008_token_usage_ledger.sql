-- KS-8.2 (#8308): token ledger domain — Postgres twins of the D1 tables
-- `token_usage_events`, `token_usage_leaderboard_preferences`, and the
-- three `public_khala_tokens_served_*` rollups (worker migrations
-- 0137/0138/0232/0262/0263/0264/0265/0269).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §1 (universal porting rules);
-- template: 0005_pylon_dispatch.sql (KS-8.1).
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps (they sort correctly as text),
-- 0/1 booleans as smallint, JSON metadata as text (NOT jsonb: row-hash
-- reconciliation compares exact bytes). Token counts are bigint (the SUMs
-- reconcile in the billions); the store casts read results with Number().
-- Tightening to native types is a post-retirement cleanup, never
-- mid-migration.
--
-- IDEMPOTENCY KEYS PORT EXACTLY: D1's dedupe-SELECT-then-INSERT on
-- `idempotency_key OR id` collapses to a bare `ON CONFLICT DO NOTHING`
-- (covers BOTH the id primary key and the idempotency_key unique — the
-- same key set, no TOCTOU window).
--
-- ROLLUPS ARE MIRRORED, NOT RECOMPUTED: the three rollup tables are exact
-- twins of the D1 rollups and are maintained by the SAME rule the D1 side
-- uses — the ledger `ingestEvent` path increments them in the insert
-- transaction; the low-volume direct-insert paths do NOT (matching D1's
-- live behavior since worker migrations 0264/0265 backfilled them). The
-- backfill CLI copies the D1 rollup rows verbatim (converge upsert), so
-- Postgres rollups reconcile against D1 rollups byte-for-byte, not against
-- a recomputation with different coverage.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS — deliberately NOT the
-- 14 D1 indexes on token_usage_events (the MIGRATION_PLAN §1 cautionary
-- example). What actually reads this table on the Postgres side after
-- KS-8.2 + KS-6.3 (#8304):
--   * the public headline counter rides the `khala_sync_public_counters`
--     projection (#8304) — no full-table SUM on the hot path;
--   * history + model-mix + channel-mix serve from the rollup tables
--     (their PRIMARY KEYs are the whole read: timezone+day / day+provider+
--     model / day+demand_channel range scans);
--   * the only raw-event reads are bounded observed_at range scans: the
--     history/mix partial first-day and live last-day windows, the
--     demand-mix windowed GROUP BY, MIN(observed_at), and the
--     backfill/reconcile verification SUMs.
-- KEPT indexes and why:
--   * token_usage_events PRIMARY KEY (id) — write-path dedupe by event id.
--   * token_usage_events UNIQUE (idempotency_key) — the exact-once write
--     guard and the #8304 counter correlation key.
--   * token_usage_events_observed_at_idx — every raw read above filters or
--     groups on an observed_at range first.
--   * rollup-table PRIMARY KEYs — the entire rollup read pattern.
--   * preference PRIMARY KEY (subject_kind, subject_ref) — the upsert and
--     point read.
-- DROPPED D1 indexes (no Postgres read in this domain uses them; the
-- internal admin/leaderboard aggregates stay on D1 until the decommission
-- lane moves them WITH their own re-derived indexes):
--   * idx_token_usage_events_provider_model, _source_route, _actor_user,
--     _actor_team, _leaderboard, _role_ref (admin readAggregates /
--     readLeaderboards remain D1-served in this lane)
--   * idx_token_usage_events_demand_kind, _demand_source, _demand_client
--     (the public demand mix groups over an observed_at window — the
--     observed_at index serves it; the per-kind prefixes were for D1's
--     scan-avoidance under write pressure)
--   * idx_token_usage_events_demand_channel (channel mix reads rollups;
--     the partial-day raw read is an observed_at range)
--   * idx_token_usage_events_public_observed_tokens,
--     _public_observed_provider_model, _public_observed_channel (D1
--     0263's covering indexes for live full-window SUMs — those reads now
--     ride the #8304 projection and the rollup tables)
--   * idx_token_usage_leaderboard_preferences_participation (the
--     leaderboard NOT EXISTS probes stay on D1 in this lane; the tiny
--     table's PK serves the ported point reads/upserts)
--
-- NO FOREIGN KEYS (same as D1 for these tables): dual-write mirrors and
-- the backfill land per-row; integrity is verified by reconciliation.

CREATE TABLE IF NOT EXISTS token_usage_events (
  id                    text NOT NULL PRIMARY KEY,
  idempotency_key       text NOT NULL UNIQUE,
  observed_at           text NOT NULL,
  ingested_at           text NOT NULL,
  producer_system       text NOT NULL,
  source_route          text NOT NULL,
  role_ref              text,
  actor_user_id         text,
  actor_team_id         text,
  account_ref           text,
  anonymized_source_ref text,
  run_ref               text,
  session_ref           text,
  task_ref              text,
  repository_ref        text,
  provider              text,
  model                 text,
  backend_profile       text,
  input_tokens          bigint NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
  output_tokens         bigint NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
  reasoning_tokens      bigint NOT NULL DEFAULT 0 CHECK (reasoning_tokens >= 0),
  cache_read_tokens     bigint NOT NULL DEFAULT 0 CHECK (cache_read_tokens >= 0),
  cache_write_5m_tokens bigint NOT NULL DEFAULT 0 CHECK (cache_write_5m_tokens >= 0),
  cache_write_1h_tokens bigint NOT NULL DEFAULT 0 CHECK (cache_write_1h_tokens >= 0),
  total_tokens          bigint NOT NULL DEFAULT 0 CHECK (total_tokens >= 0),
  usage_truth           text NOT NULL,
  cost_amount           double precision,
  currency              text,
  demand_channel        text NOT NULL DEFAULT 'khala_api'
    CHECK (demand_channel IN ('khala_api', 'direct_local')),
  demand_kind           text NOT NULL DEFAULT 'unlabeled',
  demand_source         text,
  demand_client         text,
  leaderboard_eligible  smallint NOT NULL DEFAULT 1
    CHECK (leaderboard_eligible IN (0, 1)),
  privacy_opt_out       smallint NOT NULL DEFAULT 0
    CHECK (privacy_opt_out IN (0, 1)),
  safe_metadata_json    text NOT NULL DEFAULT '{}'
);

-- Bounded raw reads: history/mix partial-day windows, demand-mix windowed
-- GROUP BY, MIN(observed_at), reconcile/verify range SUMs.
CREATE INDEX IF NOT EXISTS token_usage_events_observed_at_idx
  ON token_usage_events (observed_at);

-- Exact twin of D1 public_khala_tokens_served_daily_rollups (0264).
CREATE TABLE IF NOT EXISTS public_khala_tokens_served_daily_rollups (
  timezone      text   NOT NULL,
  day           text   NOT NULL,
  tokens_served bigint NOT NULL DEFAULT 0 CHECK (tokens_served >= 0),
  usage_events  bigint NOT NULL DEFAULT 0 CHECK (usage_events >= 0),
  updated_at    text   NOT NULL,
  PRIMARY KEY (timezone, day)
);

-- Exact twin of D1 public_khala_tokens_served_model_daily_rollups (0265).
CREATE TABLE IF NOT EXISTS public_khala_tokens_served_model_daily_rollups (
  day           text   NOT NULL,
  provider      text   NOT NULL DEFAULT '',
  model         text   NOT NULL DEFAULT '',
  tokens_served bigint NOT NULL DEFAULT 0 CHECK (tokens_served >= 0),
  usage_events  bigint NOT NULL DEFAULT 0 CHECK (usage_events >= 0),
  updated_at    text   NOT NULL,
  PRIMARY KEY (day, provider, model)
);

-- Exact twin of D1 public_khala_tokens_served_channel_daily_rollups (0265).
CREATE TABLE IF NOT EXISTS public_khala_tokens_served_channel_daily_rollups (
  day            text   NOT NULL,
  demand_channel text   NOT NULL DEFAULT 'khala_api',
  tokens_served  bigint NOT NULL DEFAULT 0 CHECK (tokens_served >= 0),
  usage_events   bigint NOT NULL DEFAULT 0 CHECK (usage_events >= 0),
  updated_at     text   NOT NULL,
  PRIMARY KEY (day, demand_channel)
);

-- Exact twin of D1 token_usage_leaderboard_preferences (0138).
CREATE TABLE IF NOT EXISTS token_usage_leaderboard_preferences (
  subject_kind              text NOT NULL
    CHECK (subject_kind IN ('account', 'team', 'user')),
  subject_ref               text NOT NULL,
  leaderboard_participation text NOT NULL DEFAULT 'eligible'
    CHECK (leaderboard_participation IN ('eligible', 'opted_out')),
  leaderboard_visibility    text NOT NULL DEFAULT 'internal'
    CHECK (leaderboard_visibility IN ('internal', 'private')),
  updated_at                text NOT NULL,
  updated_by_user_id        text,
  PRIMARY KEY (subject_kind, subject_ref)
);
