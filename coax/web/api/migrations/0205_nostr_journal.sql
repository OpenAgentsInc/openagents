-- The shared Nostr client journal. Coax owns its bytes and all admission;
-- PostgreSQL owns atomic replacement and serializes concurrent devices.
CREATE TABLE auth.nostr_journals_v1alpha1 (
 namespace text PRIMARY KEY,
 account text NOT NULL REFERENCES auth.accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
 origin_account text NOT NULL,
 identity text NOT NULL,
 revision bigint NOT NULL,
 pruned boolean NOT NULL DEFAULT false,
 document bytea NOT NULL,
 updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX nostr_journal_owner_v1alpha1 ON auth.nostr_journals_v1alpha1(account,identity,namespace);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('auth.nostr_journals_v1alpha1','account','account','move','delete',true,'{}','verified public originals, encrypted gift wraps and durable relay outcomes; no plaintext private messages'),
 ('auth.nostr_journals_v1alpha1','origin_account','account','keep','keep',true,'{}','immutable namespace provenance, not an owner or authenticator; row is removed with its current account owner');
