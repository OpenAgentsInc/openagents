-- Identity keys for Nostr/Bitcoin-derived credential encryption

ALTER TABLE users ADD COLUMN nostr_public_key TEXT;
ALTER TABLE users ADD COLUMN nostr_npub TEXT;
ALTER TABLE users ADD COLUMN nostr_private_key_encrypted TEXT;
ALTER TABLE users ADD COLUMN bitcoin_xpriv_encrypted TEXT;
