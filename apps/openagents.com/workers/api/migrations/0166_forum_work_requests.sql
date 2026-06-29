-- Forum work requests are the public, ref-only Forum surface for NIP-LBR
-- labor requests. The tables intentionally store only public refs, event ids,
-- and public Forum projections. Escrow and payout state are handled by later
-- labor lanes.

INSERT OR IGNORE INTO forum_categories (
  id,
  board_id,
  slug,
  title,
  description_ref,
  order_index,
  discoverability,
  created_at,
  updated_at
)
VALUES (
  '99999999-7777-4777-8777-999999999999',
  '11111111-1111-4111-8111-111111111111',
  'labor',
  'Labor',
  'content.forum.category.labor.description',
  50,
  'listed',
  '2026-06-10T20:00:00.000Z',
  '2026-06-10T20:00:00.000Z'
);

INSERT OR IGNORE INTO forum_forums (
  id,
  board_id,
  category_id,
  slug,
  title,
  description_ref,
  visibility,
  discoverability,
  locked,
  topic_count,
  post_count,
  public_projection_json,
  created_at,
  updated_at
)
VALUES (
  '99999999-7778-4778-8778-999999999999',
  '11111111-1111-4111-8111-111111111111',
  '99999999-7777-4777-8777-999999999999',
  'work-requests',
  'Work Requests',
  'content.forum.work_requests.description',
  'public',
  'listed',
  0,
  0,
  0,
  '{"classificationCaveatRef":"classification.public_forum_projection","customerSafe":true,"dataClassification":"public","excludedPrivateRefs":[],"publicSafe":true,"redactionPolicyRef":"redaction.forum.public.v1","safeArtifactRefs":["artifact.forum.work_requests"],"safeReceiptRefs":[],"trustTier":"reviewed"}',
  '2026-06-10T20:00:00.000Z',
  '2026-06-10T20:00:00.000Z'
);

CREATE TABLE IF NOT EXISTS forum_work_requests (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  topic_id TEXT NOT NULL UNIQUE REFERENCES forum_topics(id) ON DELETE CASCADE,
  first_post_id TEXT NOT NULL REFERENCES forum_posts(id) ON DELETE CASCADE,
  requester_actor_ref TEXT NOT NULL,
  title TEXT NOT NULL,
  objective_ref TEXT NOT NULL,
  verification_command_ref TEXT NOT NULL,
  repository_refs_json TEXT NOT NULL DEFAULT '[]',
  required_capability_refs_json TEXT NOT NULL DEFAULT '[]',
  budget_sats INTEGER NOT NULL CHECK (budget_sats > 0),
  budget_msats INTEGER NOT NULL CHECK (budget_msats > 0),
  deadline_ref TEXT NOT NULL,
  relay_url TEXT NOT NULL,
  job_event_id TEXT NOT NULL UNIQUE,
  job_event_kind INTEGER NOT NULL CHECK (job_event_kind = 5934),
  job_result_kind INTEGER NOT NULL CHECK (job_result_kind = 6934),
  state TEXT NOT NULL DEFAULT 'open' CHECK (
    state IN (
      'open',
      'quote_received',
      'quote_accepted',
      'running',
      'delivered',
      'accepted',
      'settled',
      'cancelled',
      'expired'
    )
  ),
  quote_count INTEGER NOT NULL DEFAULT 0 CHECK (quote_count >= 0),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_work_requests_state_created
  ON forum_work_requests(state, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_work_requests_actor_created
  ON forum_work_requests(requester_actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_work_request_relay_links (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL UNIQUE REFERENCES forum_work_requests(id)
    ON DELETE CASCADE,
  topic_id TEXT NOT NULL UNIQUE REFERENCES forum_topics(id) ON DELETE CASCADE,
  job_event_id TEXT NOT NULL UNIQUE,
  job_event_kind INTEGER NOT NULL CHECK (job_event_kind = 5934),
  relay_url TEXT NOT NULL,
  relay_ref TEXT NOT NULL,
  bridge_actor_ref TEXT NOT NULL,
  event_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_relay_links_event
  ON forum_work_request_relay_links(job_event_id)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_work_request_lifecycle_posts (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL REFERENCES forum_work_requests(id)
    ON DELETE CASCADE,
  topic_id TEXT NOT NULL REFERENCES forum_topics(id) ON DELETE CASCADE,
  post_id TEXT NOT NULL UNIQUE REFERENCES forum_posts(id) ON DELETE CASCADE,
  idempotency_key TEXT NOT NULL UNIQUE,
  lifecycle_kind TEXT NOT NULL CHECK (
    lifecycle_kind IN (
      'quote_received',
      'quote_accepted',
      'running',
      'delivered',
      'accepted',
      'settled',
      'cancelled',
      'expired'
    )
  ),
  receipt_ref TEXT NOT NULL,
  state_after TEXT NOT NULL CHECK (
    state_after IN (
      'open',
      'quote_received',
      'quote_accepted',
      'running',
      'delivered',
      'accepted',
      'settled',
      'cancelled',
      'expired'
    )
  ),
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_lifecycle_posts_request
  ON forum_work_request_lifecycle_posts(work_request_id, created_at DESC)
  WHERE archived_at IS NULL;
