-- The result of an uncertain crypto request is retained exactly once.
-- Coax admits purposes, intent digests, source and capacity. SQL only
-- serializes identity-owned rows and commits the complete public ciphertext.
ALTER TABLE auth.nostr_journals_v1alpha1 ADD UNIQUE(account,namespace);
CREATE TABLE auth.nostr_operations_v1alpha1 (
 namespace text NOT NULL,
 account text NOT NULL,
 identity text NOT NULL,
 operation text NOT NULL,
 action smallint NOT NULL,
 digest text NOT NULL,
 source smallint NOT NULL,
 result text NOT NULL,
 result_sha256 text NOT NULL,
 PRIMARY KEY(namespace,operation),
 FOREIGN KEY(account,namespace) REFERENCES auth.nostr_journals_v1alpha1(account,namespace) ON UPDATE CASCADE ON DELETE CASCADE
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('auth.nostr_operations_v1alpha1','account','account','move','delete',true,'{}','exact signed public/ciphertext operation results; private opening is never cached');
