-- Account-private NIP-17 read positions. No decrypted message or relay receipt.
CREATE TABLE auth.nostr_read_markers_v1 (
 namespace text NOT NULL,
 account text NOT NULL,
 identity text NOT NULL,
 conversation text NOT NULL,
 created_at bigint NOT NULL,
 event text NOT NULL,
 PRIMARY KEY(namespace,conversation),
 FOREIGN KEY(account,namespace) REFERENCES auth.nostr_journals_v1alpha1(account,namespace) ON UPDATE CASCADE ON DELETE CASCADE
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('auth.nostr_read_markers_v1','account','account','move','delete',true,'{}','private conversation read positions; no plaintext and never published as relay read receipts');
