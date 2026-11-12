-- The one-tap claim for name@openagents.com (docs/auth/lightning-addresses.md "The one-tap claim",
-- #13209).
--
-- pay.addresses.nip05_pubkey  the key that signed the person's Nostr profile naming this address
--                             as its nip05 (lnaddr.c checks the signed kind 0, then sets it); served
--                             at /.well-known/nostr.json while that key is still on the account.
--                             We never sign a profile ourselves.
-- pay.notices kinds           'destination': a claimed name's first destination (it had none, so it
--                             takes effect at once); 'profile': the person's signed profile passed on
ALTER TABLE pay.addresses ADD COLUMN IF NOT EXISTS nip05_pubkey text CHECK (nip05_pubkey ~ '^[0-9a-f]{64}$');
INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  ('pay.addresses', 'nip05_pubkey', 'pubkey', 'follows', 'keep', true, '{}', 'the NIP-05 name for a key on the account; the row goes with the address')
ON CONFLICT DO NOTHING;

ALTER TABLE pay.notices DROP CONSTRAINT IF EXISTS notices_kind_check;
ALTER TABLE pay.notices ADD CONSTRAINT notices_kind_check
  CHECK (kind IN ('change.requested', 'change.applied', 'change.cancelled', 'claimed', 'off', 'on', 'destination', 'profile'));
