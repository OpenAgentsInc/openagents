-- Artanis closeout verifier (issue #4697): replay verdicts for
-- admin-tick-dispatched executor-trace closeouts. The full safe span:
-- dispatch (mind-decided, no-spend) -> execute (pylon) -> verify
-- (byte-identical replay in the worker) -> accept (digest predicate).

CREATE TABLE IF NOT EXISTS artanis_closeout_verdicts (
  id TEXT PRIMARY KEY,
  assignment_ref TEXT NOT NULL UNIQUE,
  outcome TEXT NOT NULL CHECK (outcome IN ('verified', 'rejected', 'unreadable')),
  claimed_trace_digest_prefix TEXT,
  accept_state TEXT NOT NULL CHECK (
    accept_state IN ('accepted', 'rejected', 'accept_failed', 'skipped')
  ),
  detail TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);
