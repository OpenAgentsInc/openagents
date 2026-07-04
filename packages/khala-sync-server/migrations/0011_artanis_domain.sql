-- KS-8.6 (#8317): Artanis supervision domain — Postgres twins of the 20
-- D1 `artanis_*` tables (worker migrations 0119/0120/0161/0163/0164/0165/
-- 0169/0213/0215/0245/0248/0249/0256). Plan: docs/khala-sync/MIGRATION_PLAN.md
-- §3.3 (Wave A). Six of the ~23 every-minute cron tasks are Artanis ticks;
-- this migration gives them a Postgres home so the dual-write mirror and the
-- backfill can converge rows before any read cutover.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps (they sort correctly as text),
-- 0/1 booleans as smallint, JSON payloads as text (NOT jsonb: row-hash
-- reconciliation compares exact bytes), integer counters as integer, sat
-- amounts as bigint. Tightening to native types is a post-retirement
-- cleanup, never mid-migration.
--
-- IDEMPOTENCY KEYS PORT EXACTLY: every D1 unique key ports as the SAME
-- Postgres unique constraint, so `ON CONFLICT ... DO NOTHING / DO UPDATE`
-- upserts converge on the exact keys the D1 authority dedupes on (ledger
-- tables: record_ref + idempotency_key; responder actions: topic_id;
-- responder ticks: scheduled_at; closeout verdicts: assignment_ref).
--
-- NO CROSS-TABLE FOREIGN KEYS (deliberate, unlike D1's
-- messages→threads and spend_decisions→grants FKs): dual-write mirrors and
-- the backfill land per-table and per-row; a message mirror may arrive
-- before its thread is backfilled. Referential integrity is verified by
-- set-membership at reconciliation (MIGRATION_PLAN §3.7 rule), not enforced
-- mid-migration.
--
-- D1's partial unique index `idx_artanis_loop_records_one_active_scope`
-- ((agent_id, scope_ref) WHERE active = 1) is deliberately NOT ported:
-- mirrors and backfill pages land per-row, so mid-convergence Postgres may
-- transiently hold two active rows for one scope that D1 never held
-- simultaneously. The invariant stays enforced by the D1 authority and is
-- re-checked at reconciliation; it is re-added at read cutover.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS in the owning worker
-- modules (artanis-persistence.ts, artanis-forum-responder.ts,
-- artanis-reply-composer.ts, artanis-responder-ticks.ts,
-- artanis-administrator-tick.ts, artanis-fleet-overseer-tick.ts,
-- artanis-spend.ts, artanis-labor-receipt-store.ts, artanis-owner-memory.ts,
-- artanis-operator-chat-routes.ts) — NOT blind-ported from D1.

-- ---------------------------------------------------------------------------
-- The eight uniform persistence-ledger tables (artanis-persistence.ts).
-- Identical column set; reads are by record_ref, idempotency_key, and
-- latest-N by updated_at.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS artanis_runtime_snapshots (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text,
  parent_ref             text,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_runtime_snapshots_updated
  ON artanis_runtime_snapshots (updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_loop_records (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text NOT NULL,
  parent_ref             text,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_loop_records_updated
  ON artanis_loop_records (updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_loop_ticks (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text,
  parent_ref             text NOT NULL,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_loop_ticks_updated
  ON artanis_loop_ticks (updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_approval_gates (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text,
  parent_ref             text,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_approval_gates_updated
  ON artanis_approval_gates (updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_health_snapshots (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text,
  parent_ref             text,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_health_snapshots_updated
  ON artanis_health_snapshots (updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_work_routing_proposals (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text,
  parent_ref             text,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_work_routing_proposals_updated
  ON artanis_work_routing_proposals (updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_forum_publication_intents (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text,
  parent_ref             text,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_forum_publication_intents_updated
  ON artanis_forum_publication_intents (updated_at DESC);

CREATE TABLE IF NOT EXISTS artanis_nexus_pylon_adapter_dispatches (
  id                     text NOT NULL,
  agent_id               text NOT NULL,
  record_ref             text PRIMARY KEY,
  idempotency_key        text NOT NULL UNIQUE,
  state                  text NOT NULL,
  active                 smallint NOT NULL DEFAULT 0 CHECK (active IN (0, 1)),
  source_kind            text NOT NULL,
  scope_ref              text,
  parent_ref             text,
  record_json            text NOT NULL,
  public_projection_json text NOT NULL,
  content_hash           text NOT NULL,
  closeout_json          text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL,
  closed_at              text
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_nexus_pylon_adapter_dispatches_updated
  ON artanis_nexus_pylon_adapter_dispatches (updated_at DESC);

-- ---------------------------------------------------------------------------
-- Forum responder (artanis-forum-responder.ts, artanis-reply-composer.ts,
-- artanis-responder-ticks.ts). The scan cursor is a single row (id = 1);
-- actions dedupe by topic_id; ticks upsert by scheduled_at.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS artanis_responder_state (
  id              integer PRIMARY KEY CHECK (id = 1),
  scan_cursor_iso text NOT NULL,
  responses_today integer NOT NULL DEFAULT 0,
  responses_day   text NOT NULL DEFAULT '',
  updated_at      text NOT NULL
);

CREATE TABLE IF NOT EXISTS artanis_responder_actions (
  id                text NOT NULL,
  topic_id          text PRIMARY KEY,
  first_post_id     text,
  question_class    text,
  state             text NOT NULL CHECK (
    state IN ('proposed', 'responded', 'tipped', 'skipped', 'blocked')
  ),
  proposal_json     text NOT NULL DEFAULT '{}',
  reply_post_id     text,
  asked_at          text,
  replied_at        text,
  created_at        text NOT NULL,
  updated_at        text NOT NULL,
  tip_receipt_ref   text,
  tip_pay_in_id     text,
  tip_ladder_rung   text,
  tip_ladder_reason text,
  asker_actor_ref   text,
  asker_provenance  text
);

-- Composer picks proposed actions oldest-first; readiness projections scan
-- by state newest-first.
CREATE INDEX IF NOT EXISTS idx_pg_artanis_responder_actions_state_created
  ON artanis_responder_actions (state, created_at DESC);

CREATE TABLE IF NOT EXISTS artanis_responder_ticks (
  tick_ref              text NOT NULL,
  scheduled_at          text PRIMARY KEY,
  scan_state            text NOT NULL DEFAULT 'pending' CHECK (
    scan_state IN ('pending', 'ran', 'skipped', 'error')
  ),
  scan_scanned          integer NOT NULL DEFAULT 0,
  scan_proposed         integer NOT NULL DEFAULT 0,
  scan_blocked          integer NOT NULL DEFAULT 0,
  scan_skipped          integer NOT NULL DEFAULT 0,
  scan_skipped_reason   text,
  compose_state         text NOT NULL DEFAULT 'pending' CHECK (
    compose_state IN ('pending', 'ran', 'skipped', 'error')
  ),
  compose_considered    integer NOT NULL DEFAULT 0,
  compose_responded     integer NOT NULL DEFAULT 0,
  compose_blocked       integer NOT NULL DEFAULT 0,
  compose_tipped        integer NOT NULL DEFAULT 0,
  compose_skipped_reason text,
  created_at            text NOT NULL,
  updated_at            text NOT NULL
);

-- ---------------------------------------------------------------------------
-- Administrator tick + closeout verifier (artanis-administrator-tick.ts).
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS artanis_admin_tick_decisions (
  id             text PRIMARY KEY,
  state          text NOT NULL CHECK (
    state IN ('dispatched', 'no_action', 'blocked', 'dispatch_failed')
  ),
  action_json    text NOT NULL DEFAULT '{}',
  assignment_ref text,
  created_at     text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_admin_tick_decisions_state_created
  ON artanis_admin_tick_decisions (state, created_at DESC);

CREATE TABLE IF NOT EXISTS artanis_closeout_verdicts (
  id                          text NOT NULL,
  assignment_ref              text PRIMARY KEY,
  outcome                     text NOT NULL CHECK (
    outcome IN ('verified', 'rejected', 'unreadable')
  ),
  claimed_trace_digest_prefix text,
  accept_state                text NOT NULL CHECK (
    accept_state IN ('accepted', 'rejected', 'accept_failed', 'skipped')
  ),
  detail                      text NOT NULL DEFAULT '',
  created_at                  text NOT NULL
);

-- The verifier excludes already-verified assignments by outcome.
CREATE INDEX IF NOT EXISTS idx_pg_artanis_closeout_verdicts_outcome
  ON artanis_closeout_verdicts (outcome);

-- ---------------------------------------------------------------------------
-- Fleet overseer decision ledger (artanis-fleet-overseer-tick.ts).
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS artanis_fleet_overseer_decisions (
  id                  text PRIMARY KEY,
  state               text NOT NULL CHECK (
    state IN (
      'reported',
      'autonomous_intent_recorded',
      'approval_requested',
      'no_action',
      'blocked',
      'skipped'
    )
  ),
  action_json         text NOT NULL,
  context_json        text NOT NULL,
  approval_gate_ref   text,
  health_snapshot_ref text,
  created_at          text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_fleet_overseer_decisions_state_created
  ON artanis_fleet_overseer_decisions (state, created_at DESC);

-- ---------------------------------------------------------------------------
-- Standing spend envelope (artanis-spend.ts). Spend decisions reference the
-- treasury by payment_ref ID ONLY (no cross-store joins) until KS-8.8.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS artanis_standing_spend_grants (
  grant_ref          text PRIMARY KEY,
  per_payout_cap_sat bigint NOT NULL CHECK (per_payout_cap_sat > 0),
  per_day_cap_sat    bigint NOT NULL CHECK (per_day_cap_sat > 0),
  authority_ref      text NOT NULL,
  active             smallint NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  created_at         text NOT NULL,
  revoked_at         text
);

CREATE TABLE IF NOT EXISTS artanis_spend_decisions (
  id                     text PRIMARY KEY,
  grant_ref              text NOT NULL,
  state                  text NOT NULL CHECK (
    state IN ('proposed', 'paid', 'refused', 'blocked_over_cap')
  ),
  intended_amount_sat    bigint NOT NULL CHECK (intended_amount_sat > 0),
  paid_amount_sat        bigint,
  destination_source_ref text NOT NULL,
  recipient_ref          text NOT NULL,
  rationale              text NOT NULL DEFAULT '',
  payment_ref            text,
  policy_applied         text,
  created_at             text NOT NULL,
  updated_at             text NOT NULL
);

-- Per-day accounting sums paid decisions since midnight.
CREATE INDEX IF NOT EXISTS idx_pg_artanis_spend_decisions_state_created
  ON artanis_spend_decisions (state, created_at DESC);

-- ---------------------------------------------------------------------------
-- Unattended labor receipts (artanis-labor-receipt-store.ts). Rows are
-- content-addressed by receipt_ref; the D1 list path orders by
-- (created_at, rowid) — Postgres orders by (created_at, receipt_ref), which
-- is equivalent for distinct refs and stable.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS artanis_labor_unattended_receipts (
  receipt_ref     text PRIMARY KEY,
  serialized_json text NOT NULL,
  terminal_state  text NOT NULL,
  created_at      text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_labor_unattended_receipts_created
  ON artanis_labor_unattended_receipts (created_at, receipt_ref);

-- ---------------------------------------------------------------------------
-- Owner memory + operator chat threads/messages (artanis-owner-memory.ts,
-- artanis-operator-chat-routes.ts). Strictly private/operator scoped —
-- never projected publicly.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS artanis_owner_memory (
  memory_ref    text PRIMARY KEY,
  owner_id      text NOT NULL,
  kind          text NOT NULL CHECK (kind IN ('turn', 'note')),
  role          text CHECK (role IN ('owner', 'artanis')),
  note_category text CHECK (note_category IN ('decision', 'preference', 'fact')),
  body          text NOT NULL,
  created_at    text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_owner_memory_owner_created
  ON artanis_owner_memory (owner_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_owner_memory_owner_kind_created
  ON artanis_owner_memory (owner_id, kind, created_at DESC);

CREATE TABLE IF NOT EXISTS artanis_threads (
  thread_ref         text PRIMARY KEY,
  caller_id          text NOT NULL,
  caller_kind        text NOT NULL CHECK (
    caller_kind IN ('owner', 'agent', 'operator', 'system')
  ),
  subject_agent_ref  text NOT NULL,
  subject_agent_kind text NOT NULL CHECK (
    subject_agent_kind IN ('artanis', 'claude', 'codex', 'other')
  ),
  title              text NOT NULL DEFAULT '',
  status             text NOT NULL DEFAULT 'open' CHECK (
    status IN ('open', 'archived')
  ),
  source_ref         text,
  metadata_json      text NOT NULL DEFAULT '{}',
  last_message_at    text NOT NULL,
  created_at         text NOT NULL,
  updated_at         text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_threads_caller_last_message
  ON artanis_threads (caller_id, last_message_at DESC);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_threads_caller_created
  ON artanis_threads (caller_id, created_at DESC);

CREATE TABLE IF NOT EXISTS artanis_messages (
  message_ref   text PRIMARY KEY,
  thread_ref    text NOT NULL,
  caller_id     text NOT NULL,
  author_id     text NOT NULL,
  author_kind   text NOT NULL CHECK (
    author_kind IN ('owner', 'agent', 'operator', 'system', 'tool')
  ),
  body          text NOT NULL,
  metadata_json text NOT NULL DEFAULT '{}',
  created_at    text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_messages_thread_created
  ON artanis_messages (thread_ref, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_pg_artanis_messages_caller_created
  ON artanis_messages (caller_id, created_at DESC);
