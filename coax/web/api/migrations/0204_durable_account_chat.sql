-- Durable owned chat. Coax decides admission, transition and recovery;
-- SQL serializes rows, compare-and-swap revisions, unique operations and events.
CREATE TABLE chat.conversations_v2 (
 id text PRIMARY KEY,
 account text NOT NULL REFERENCES auth.accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
 project text,
 revision bigint NOT NULL,
 title text NOT NULL,
 legacy_thread text, legacy_revision bigint, legacy_record text,
 created_ms bigint NOT NULL, updated_ms bigint NOT NULL,
 UNIQUE(account,id),
 FOREIGN KEY(account,project) REFERENCES projects.private_v1(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
CREATE INDEX conversations_v2_owner ON chat.conversations_v2(account,id);
CREATE INDEX conversations_v2_project ON chat.conversations_v2(account,project,id);
CREATE TABLE chat.turns_v2 (
 id text PRIMARY KEY, account text NOT NULL, conversation text NOT NULL,
 ordinal bigint NOT NULL, operation text NOT NULL, digest text NOT NULL,
 request_json text NOT NULL, input_json text NOT NULL,
 snapshot_id text, snapshot_json text,
 state int NOT NULL, stop_requested boolean NOT NULL DEFAULT false,
 sequence bigint NOT NULL DEFAULT 0, output text NOT NULL DEFAULT '',
 stop_reason text NOT NULL DEFAULT '', error text NOT NULL DEFAULT '',
 input_tokens bigint, output_tokens bigint, cache_read_tokens bigint, cache_write_tokens bigint, reasoning_tokens bigint,
 model text NOT NULL, provider text NOT NULL, effort text NOT NULL,
 worker text, runner_key text, claim_operation text, dispatch text,
 claim_until_ms bigint, dispatched_ms bigint, heartbeat_ms bigint,
 created_ms bigint NOT NULL, updated_ms bigint NOT NULL,
 UNIQUE(account,id), UNIQUE(account,operation), UNIQUE(conversation,ordinal),
 FOREIGN KEY(account,conversation) REFERENCES chat.conversations_v2(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
CREATE UNIQUE INDEX turns_v2_one_active ON chat.turns_v2(conversation) WHERE state IN (1,2);
CREATE UNIQUE INDEX turns_v2_claim ON chat.turns_v2(runner_key,claim_operation) WHERE claim_operation IS NOT NULL;
CREATE INDEX turns_v2_queue ON chat.turns_v2(state,created_ms,id);
-- A replaced claim remains replayable with its old fence. Repeating that
-- operation must never select a different turn after the first claim expires.
CREATE TABLE chat.turn_claims_v2 (
 account text NOT NULL, runner_key text NOT NULL, operation text NOT NULL,
 worker text NOT NULL, turn text NOT NULL, dispatch text NOT NULL,
 created_ms bigint NOT NULL,
 PRIMARY KEY(runner_key,operation),
 FOREIGN KEY(account,turn) REFERENCES chat.turns_v2(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
CREATE TABLE chat.turn_events_v2 (
 account text NOT NULL, turn text NOT NULL, sequence bigint NOT NULL, record text NOT NULL,
 PRIMARY KEY(turn,sequence),
 FOREIGN KEY(account,turn) REFERENCES chat.turns_v2(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
CREATE TABLE chat.turn_operations_v2 (
 account text NOT NULL, operation text NOT NULL, path text NOT NULL, digest text NOT NULL, turn text NOT NULL,
 PRIMARY KEY(account,operation),
 FOREIGN KEY(account,turn) REFERENCES chat.turns_v2(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('chat.conversations_v2','account','account','move','delete',true,'{}','owned rich conversation and pinned legacy provenance'),
 ('chat.turns_v2','account','account','move','delete',true,'{dispatch}','pinned private inputs and durable model outcomes; dispatch is an internal execution fence'),
 ('chat.turn_claims_v2','account','account','move','delete',false,'{}','stable runner claim receipts follow the owned turn and are removed with it'),
 ('chat.turn_events_v2','account','account','move','delete',true,'{}','immutable private replay events'),
 ('chat.turn_operations_v2','account','account','move','delete',true,'{}','idempotent chat operation identities');
