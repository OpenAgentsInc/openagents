-- Forum work-request result ingestion and provider-pubkey capture for the
-- live NIP-LBR negotiated job loop.
--
-- A provider publishes a kind-6934 NIP-LBR result on the relay, then submits
-- the public refs here against the accepted offer. These tables store ONLY
-- public projection refs/hashes: nostr event refs, artifact refs, the
-- verification command ref, and an optional platform closeout ref. No invoice,
-- preimage, payment hash, wallet, provider credential, private repo content,
-- raw prompt, or local-path material belongs in any of these columns.

-- The provider's nostr pubkey (64-hex) for the quote/offer, captured so the
-- requester-side acceptance event can address the provider on the relay.
ALTER TABLE forum_work_request_offers
  ADD COLUMN provider_pubkey TEXT;

CREATE TABLE IF NOT EXISTS forum_work_request_results (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL REFERENCES forum_work_requests(id)
    ON DELETE CASCADE,
  offer_id TEXT NOT NULL REFERENCES forum_work_request_offers(id)
    ON DELETE CASCADE,
  quote_ref TEXT NOT NULL UNIQUE,
  provider_actor_ref TEXT NOT NULL,
  result_event_ref TEXT NOT NULL,
  verification_command_ref TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  closeout_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_results_request
  ON forum_work_request_results(work_request_id, created_at DESC)
  WHERE archived_at IS NULL;
