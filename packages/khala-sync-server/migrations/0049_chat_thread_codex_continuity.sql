ALTER TABLE khala_sync_chat_threads
  ADD COLUMN IF NOT EXISTS codex_continuity_provider TEXT,
  ADD COLUMN IF NOT EXISTS codex_continuity_provider_account_ref TEXT,
  ADD COLUMN IF NOT EXISTS codex_continuity_auth_grant_ref TEXT,
  ADD COLUMN IF NOT EXISTS codex_continuity_account_ref_hash TEXT,
  ADD COLUMN IF NOT EXISTS codex_continuity_pinned_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_khala_sync_chat_threads_codex_continuity
  ON khala_sync_chat_threads (owner_user_id, codex_continuity_provider)
  WHERE codex_continuity_provider IS NOT NULL;
