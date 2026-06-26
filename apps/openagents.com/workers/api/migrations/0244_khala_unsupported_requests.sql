-- Operator-maintained Khala unsupported-request triage ledger.
--
-- Rows are bounded public-safe summaries/refs only. Raw trace payloads, raw
-- feedback text, private paths, and provider payloads stay in their owner/admin
-- source systems.

CREATE TABLE IF NOT EXISTS khala_unsupported_requests (
  request_ref TEXT PRIMARY KEY,
  source_kind TEXT NOT NULL CHECK (
    source_kind IN ('trace_review', 'khala_feedback', 'forum', 'operator')
  ),
  source_ref TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL DEFAULT '',
  triage_kind TEXT NOT NULL CHECK (
    triage_kind IN ('needs_triage', 'bug', 'missing_capability', 'wont_do')
  ),
  status TEXT NOT NULL CHECK (
    status IN ('open', 'needs_issue', 'issue_opened', 'closed', 'wont_do')
  ),
  forum_topic_ref TEXT,
  github_issue_ref TEXT,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  suggested_issue_title TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(source_kind, source_ref)
);

CREATE INDEX IF NOT EXISTS idx_khala_unsupported_requests_status_updated
  ON khala_unsupported_requests(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_khala_unsupported_requests_triage_updated
  ON khala_unsupported_requests(triage_kind, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_khala_unsupported_requests_source
  ON khala_unsupported_requests(source_kind, source_ref);
