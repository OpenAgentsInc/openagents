-- Profile and private preferences; never a login authenticator or key store.
CREATE TABLE auth.account_settings_v1 (
 account text PRIMARY KEY REFERENCES auth.accounts(id),
 revision bigint NOT NULL CHECK (revision > 0),
 document text NOT NULL CHECK (octet_length(document) <= 8192)
);
INSERT INTO auth.life_columns (tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('auth.account_settings_v1','account','account','move','delete',true,'{}','profile and preferences; survivor settings win a merge collision');
