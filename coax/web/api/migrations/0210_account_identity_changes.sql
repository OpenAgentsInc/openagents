-- Public selection and idempotency metadata, separate from the frozen v1 key store.
CREATE TABLE auth.account_identity_selection_v1 (
 account text PRIMARY KEY REFERENCES auth.accounts(id),
 pubkey text NOT NULL REFERENCES auth.account_identity_keys_v1(pubkey) ON DELETE CASCADE,
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 2147483647)
);
CREATE TABLE auth.account_identity_changes_v1 (
 account text NOT NULL REFERENCES auth.accounts(id),
 command text NOT NULL CHECK (command ~ '^[0-9a-f]{32}$'),
 digest text NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
 expected text NOT NULL CHECK (expected ~ '^[0-9a-f]{64}$'),
 pubkey text REFERENCES auth.account_identity_keys_v1(pubkey) ON DELETE CASCADE,
 mode text NOT NULL CHECK (mode IN ('rotate','restore','cancel')),
 cancelled boolean NOT NULL DEFAULT false,
 CHECK ((cancelled AND mode='cancel' AND pubkey IS NULL) OR (NOT cancelled AND mode<>'cancel' AND pubkey IS NOT NULL)),
 PRIMARY KEY(account,command)
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('auth.account_identity_selection_v1','account','account','move','delete',true,'{}','current managed identity; survivor selection wins merge'),
 ('auth.account_identity_changes_v1','account','account','move','delete',true,'{}','public identity change receipts; no private request');
