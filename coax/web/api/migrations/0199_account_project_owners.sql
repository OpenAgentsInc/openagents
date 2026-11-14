-- Compatibility owner handles are opaque account subjects, never Nostr keys.
CREATE TABLE drops.account_owners_v1 (
 account text PRIMARY KEY REFERENCES auth.accounts(id),
 owner text NOT NULL UNIQUE CHECK (owner ~ '^[0-9a-f]{64}$')
);
INSERT INTO auth.life_columns (tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('drops.account_owners_v1','account','account','move','delete',true,'{}','opaque account project ownership handles; no signing key');
-- Existing key owners retain their prior facts. Creator is the existing account grant.
CREATE OR REPLACE FUNCTION drops.may(pubkey text, admin bool) RETURNS bool LANGUAGE sql STABLE AS $$
 SELECT pubkey ~ '^[0-9a-f]{64}$' AND (admin
  OR EXISTS (SELECT 1 FROM projects.docs d, jsonb_array_elements(d.doc->'members') m WHERE m->>'pubkey' = pubkey)
  OR EXISTS (SELECT 1 FROM verse.accounts a WHERE a.npub = pubkey)
  OR EXISTS (SELECT 1 FROM drops.account_owners_v1 o JOIN auth.accounts a ON a.id=o.account JOIN access.creators c ON c.who=a.verse_account WHERE o.owner=pubkey))
$$;
