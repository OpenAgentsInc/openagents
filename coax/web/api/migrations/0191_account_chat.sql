-- Email-person conversations own their account directly, with no synthetic
-- Nostr authenticator and no widening of the legacy admin chat API.
CREATE TABLE chat.account_threads_v1 (
  account text NOT NULL REFERENCES auth.accounts(id) ON DELETE CASCADE,
  id text NOT NULL CHECK (id ~ '^m_[0-9a-f]{20}$'),
  record text NOT NULL CHECK (octet_length(record) <= 65536),
  revision bigint NOT NULL CHECK (revision > 0),
  title text NOT NULL,
  updated_ms bigint NOT NULL,
  PRIMARY KEY(account,id)
);
CREATE INDEX account_threads_v1_recent ON chat.account_threads_v1(account,updated_ms DESC);
INSERT INTO auth.life_columns (tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('chat.account_threads_v1','account','account','move','delete',true,'{}','account-owned mobile chat transcripts, independent of Nostr authenticators');
