CREATE TABLE chat.account_library_v1 (
 account text PRIMARY KEY REFERENCES auth.accounts(id),
 revision bigint NOT NULL CHECK(revision BETWEEN 0 AND 9007199254740990),
 document text NOT NULL CHECK(octet_length(document)<=131072)
);
CREATE TABLE chat.account_library_commands_v1 (
 account text NOT NULL REFERENCES auth.accounts(id),
 command text NOT NULL CHECK(command ~ '^[0-9a-f]{32}$'),
 request text NOT NULL CHECK(octet_length(request)<=131072),
 response text NOT NULL CHECK(octet_length(response)<=131072),
 PRIMARY KEY(account,command)
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('chat.account_library_v1','account','account','move','delete',true,'{}','private chat aliases and pin/archive preferences; survivor wins merge collision'),
 ('chat.account_library_commands_v1','account','account','move','delete',true,'{}','private presentation change replay receipts');
