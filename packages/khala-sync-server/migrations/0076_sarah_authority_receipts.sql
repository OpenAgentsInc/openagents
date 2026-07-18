-- Sarah owner-orchestrator revision 1: bounded private authority receipt for
-- stable principal bootstrap. No prompts, response text, credentials, paths,
-- customer payloads, or raw evidence are stored here.

CREATE TABLE IF NOT EXISTS sarah_authority_decision_receipts (
  receipt_ref        text PRIMARY KEY,
  owner_user_id      text NOT NULL,
  thread_ref         text NOT NULL,
  profile_ref        text NOT NULL,
  profile_revision   integer NOT NULL CHECK (profile_revision >= 1),
  grant_ref          text NOT NULL,
  action_ref         text NOT NULL,
  outcome            text NOT NULL CHECK (outcome IN ('succeeded', 'refused')),
  evidence_refs_json jsonb NOT NULL DEFAULT '[]'::jsonb
    CHECK (jsonb_typeof(evidence_refs_json) = 'array'),
  started_at         text NOT NULL,
  settled_at         text NOT NULL,
  CONSTRAINT sarah_authority_decision_receipts_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_authority_decision_receipts_thread_shape
    CHECK (thread_ref ~ '^thread\.sarah\.[0-9a-f]{24}$')
);

CREATE INDEX IF NOT EXISTS sarah_authority_decision_receipts_owner_settled_idx
  ON sarah_authority_decision_receipts(owner_user_id, settled_at DESC);
