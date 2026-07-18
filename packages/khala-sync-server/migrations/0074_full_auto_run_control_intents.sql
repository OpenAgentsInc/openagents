-- MOB-FA-02 (#8994): durable Pause/Resume/Stop control intents dispatched
-- from OpenAgents mobile toward a Desktop-owned FullAutoRun.
--
-- This mirrors `desktop_full_auto_run_projections` (0073)'s owner-scoped
-- discipline rather than `sarah_fleet_run_steering_intents`'s Sarah-domain
-- shape: every row is `owner_user_id`-scoped, `intent_id` is the primary key
-- (mobile mints it client-side so a retried POST is idempotent), and a
-- unique `(owner_user_id, idempotency_key)` guards a double-tap on the phone
-- from creating two rows. There is no `consumed` flag: Desktop's poll reads
-- every row with `status = 'pending'` and reports back `applied`/`rejected`
-- via the same route -- "pending" is a durable status column, not a queue
-- watermark, because v1 has exactly one FullAutoRun per owner (no fan-out
-- across runs that would need a resumable cursor).
--
-- Every column is exactly the public-safe field set in
-- packages/khala-sync/src/full-auto-run-control-intent.ts
-- (`FullAutoRunControlIntent`) -- no raw prompts, tool output, local file
-- paths, or credentials.

CREATE TABLE IF NOT EXISTS desktop_full_auto_run_control_intents (
  intent_id               text PRIMARY KEY,
  owner_user_id           text NOT NULL,
  idempotency_key         text NOT NULL,
  run_ref                 text NOT NULL,
  action                  text NOT NULL
    CHECK (action IN ('pause', 'resume', 'stop')),
  surface                 text NOT NULL
    CHECK (surface IN ('mobile')),
  status                  text NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'applied', 'rejected')),
  applied_at              text,
  rejection_reason        text
    CHECK (rejection_reason IS NULL OR rejection_reason IN (
      'run_not_found', 'illegal_transition', 'workspace_mismatch',
      'lane_not_eligible', 'desktop_unreachable', 'storage_unavailable'
    )),
  result_lifecycle_state  text
    CHECK (result_lifecycle_state IS NULL OR result_lifecycle_state IN (
      'draft', 'running', 'pausing', 'paused', 'retrying',
      'stalled', 'completed', 'failed', 'stopped', 'cap_reached'
    )),
  created_at              text NOT NULL,
  CONSTRAINT desktop_full_auto_run_control_intents_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT desktop_full_auto_run_control_intents_intent_id_shape
    CHECK (intent_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$'),
  CONSTRAINT desktop_full_auto_run_control_intents_idempotency_key_shape
    CHECK (idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$'),
  CONSTRAINT desktop_full_auto_run_control_intents_run_ref_shape
    CHECK (run_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,179}$'),
  CONSTRAINT desktop_full_auto_run_control_intents_applied_requires_result
    CHECK (status <> 'applied' OR (applied_at IS NOT NULL)),
  CONSTRAINT desktop_full_auto_run_control_intents_rejected_requires_reason
    CHECK (status <> 'rejected' OR (rejection_reason IS NOT NULL)),
  CONSTRAINT desktop_full_auto_run_control_intents_owner_idempotency_unique
    UNIQUE (owner_user_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS desktop_full_auto_run_control_intents_owner_created_idx
  ON desktop_full_auto_run_control_intents (owner_user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS desktop_full_auto_run_control_intents_owner_pending_idx
  ON desktop_full_auto_run_control_intents (owner_user_id)
  WHERE status = 'pending';
