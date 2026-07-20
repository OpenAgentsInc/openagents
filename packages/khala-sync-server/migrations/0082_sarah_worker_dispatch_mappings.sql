-- SARAH-PROACTIVE-1 (#9064): durable assignmentRef -> (ownerUserId,
-- threadRef) binding captured at Sarah's own `codex_workers_start` dispatch
-- time. The later Pylon `worker_closeout` event carries no threadId, and its
-- `ownerAgentUserId` is an agent-token identity not proven to share the
-- OpenAuth user id space push devices and Sarah threads are keyed on (see
-- SARAH-PUSH-2 #9063's skip note). Capturing the binding at the moment Sarah
-- herself dispatches the work -- when the owner/thread are already
-- authenticated and trustworthy -- avoids that cross-identity-space
-- inference entirely.
--
-- `consumed_at` makes closeout notification exactly-once: the closeout hook
-- atomically claims the row (`UPDATE ... WHERE consumed_at IS NULL
-- RETURNING`), so a retried or duplicate `worker_closeout` event finds
-- nothing left to consume and is a safe no-op.

CREATE TABLE IF NOT EXISTS sarah_worker_dispatch_mappings (
  assignment_ref  text PRIMARY KEY,
  owner_user_id   text NOT NULL,
  thread_ref      text NOT NULL,
  dispatched_at   text NOT NULL,
  consumed_at     text,
  CONSTRAINT sarah_worker_dispatch_mappings_assignment_ref_shape
    CHECK (assignment_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$'),
  CONSTRAINT sarah_worker_dispatch_mappings_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_worker_dispatch_mappings_thread_shape
    CHECK (thread_ref ~ '^thread\.sarah\.[0-9a-f]{24}$')
);

CREATE INDEX IF NOT EXISTS sarah_worker_dispatch_mappings_owner_idx
  ON sarah_worker_dispatch_mappings(owner_user_id, dispatched_at DESC);
