CREATE TABLE agent_proposals (
  id TEXT PRIMARY KEY,
  receipt_ref TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL CHECK (status IN ('pending', 'rejected', 'promoted')),
  kind TEXT NOT NULL CHECK (kind IN ('site_improvement', 'public_proof_note', 'forum_topic_draft', 'order_request_draft', 'workroom_artifact_draft', 'other')),
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  body_text TEXT NOT NULL,
  source_urls_json TEXT NOT NULL,
  target_json TEXT NOT NULL,
  author_json TEXT NOT NULL,
  client_fingerprint_hash TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  promotion_kind TEXT,
  promoted_target_ref TEXT,
  operator_note TEXT,
  operator_user_id TEXT,
  decided_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX agent_proposals_status_created_idx
  ON agent_proposals(status, created_at DESC);

CREATE INDEX agent_proposals_client_created_idx
  ON agent_proposals(client_fingerprint_hash, created_at DESC);
