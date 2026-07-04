-- Khala Sync fleet intents v2 (KS-3.2, #8292).
-- Spec: docs/khala-sync/SPEC.md §2.4; catalog: docs/khala-sync/MUTATORS.md.
--
-- Extends khala_sync_fleet_intents (0004) for the per-worker / inbox-flag /
-- terminal-stop operator mutators:
--
--   - `pause_worker` / `resume_worker` carry the target worker id
--     (`worker_id`, the public-safe dispatch-context ref).
--   - `acknowledge_inbox_flag` carries the target flag ref (`flag_ref`).
--   - `stop` is the confirmed terminal intent for a run (fleet.stopRun).
--
-- Same honest-v1 contract as 0004: an intent row is a durable operator
-- request recorded atomically with the projected post-image; Pylon-side
-- supervisor ENFORCEMENT (polling these rows via the internal
-- fleet-intents route and changing dispatch behavior) is a follow-up lane.

ALTER TABLE khala_sync_fleet_intents
  ADD COLUMN IF NOT EXISTS worker_id text,
  ADD COLUMN IF NOT EXISTS flag_ref  text;

-- Widen the intent vocabulary (0004's inline CHECK is auto-named
-- khala_sync_fleet_intents_intent_check).
ALTER TABLE khala_sync_fleet_intents
  DROP CONSTRAINT IF EXISTS khala_sync_fleet_intents_intent_check;
ALTER TABLE khala_sync_fleet_intents
  ADD CONSTRAINT khala_sync_fleet_intents_intent_check
  CHECK (intent IN (
    'set_desired_slots',
    'pause',
    'resume',
    'pause_worker',
    'resume_worker',
    'acknowledge_inbox_flag',
    'stop'
  ));

-- Per-worker intents carry a worker id; nothing else does.
ALTER TABLE khala_sync_fleet_intents
  DROP CONSTRAINT IF EXISTS khala_sync_fleet_intents_worker_shape;
ALTER TABLE khala_sync_fleet_intents
  ADD CONSTRAINT khala_sync_fleet_intents_worker_shape
  CHECK ((intent IN ('pause_worker', 'resume_worker')) = (worker_id IS NOT NULL));

-- Inbox-flag acks carry a flag ref; nothing else does.
ALTER TABLE khala_sync_fleet_intents
  DROP CONSTRAINT IF EXISTS khala_sync_fleet_intents_flag_shape;
ALTER TABLE khala_sync_fleet_intents
  ADD CONSTRAINT khala_sync_fleet_intents_flag_shape
  CHECK ((intent = 'acknowledge_inbox_flag') = (flag_ref IS NOT NULL));
