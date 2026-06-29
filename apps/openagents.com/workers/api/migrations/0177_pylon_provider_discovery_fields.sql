ALTER TABLE pylon_api_registrations
  ADD COLUMN provider_nostr_pubkey TEXT;

ALTER TABLE pylon_api_registrations
  ADD COLUMN provider_nostr_npub TEXT;

ALTER TABLE pylon_api_registrations
  ADD COLUMN provider_market_relay_refs_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE pylon_api_registrations
  ADD COLUMN provider_nip90_lane_refs_json TEXT NOT NULL DEFAULT '[]';
