-- Owner-approved account-managed Nostr identities. Not login authenticators,
-- not wallet seeds. The immutable pubkey is the sealing AD's row identity.
CREATE TABLE auth.account_identity_keys_v1 (
 account text NOT NULL REFERENCES auth.accounts(id),
 pubkey text PRIMARY KEY CHECK (pubkey ~ '^[0-9a-f]{64}$'),
 sealed text NOT NULL CHECK (sealed ~ '^[0-9a-f]{210}$'),
 created_ms bigint NOT NULL DEFAULT (extract(epoch FROM clock_timestamp())*1000)::bigint CHECK (created_ms >= 0)
);
CREATE INDEX ON auth.account_identity_keys_v1 (account,created_ms,pubkey);
-- Per-account FOR UPDATE serializes ordinary provisioning. A merge retains
-- both keys as aliases, so account is deliberately not UNIQUE.
INSERT INTO auth.life_columns (tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('auth.account_identity_keys_v1','account','account','move','delete',true,'{sealed}','account-managed identities; ciphertext never exported');
CREATE OR REPLACE FUNCTION auth.life_values(acct text, holds_ text) RETURNS text[] LANGUAGE sql STABLE AS $$
 SELECT CASE holds_
  WHEN 'account' THEN ARRAY[acct]
  WHEN 'verse' THEN (SELECT array_agg(a.verse_account) FROM auth.accounts a WHERE a.id=acct)
  WHEN 'player' THEN (SELECT array_agg(p.id) FROM verse.players p JOIN auth.accounts a ON a.verse_account=p.account WHERE a.id=acct)
  WHEN 'pubkey' THEN (SELECT array_agg(x.pk) FROM (
    SELECT k.pubkey AS pk FROM auth.nostr_keys k WHERE k.account=acct
    UNION SELECT o.owner FROM drops.account_owners_v1 o WHERE o.account=acct
    UNION SELECT k.pubkey FROM auth.account_identity_keys_v1 k WHERE k.account=acct) x)
  WHEN 'world' THEN (SELECT array_agg(w.id) FROM verse.worlds w JOIN auth.accounts a ON a.verse_account=w.owner_account WHERE a.id=acct)
 END
$$;
