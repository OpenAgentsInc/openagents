-- One complete admitted native input and one immutable association per turn.
-- Coax owns routing, authorization, recovery and state transitions.
ALTER TABLE chat.turns_v2 ADD COLUMN native_input_json text;
CREATE TABLE chat.turn_tasks_v1 (
 account text NOT NULL, turn text NOT NULL, record text NOT NULL,
 phase int NOT NULL DEFAULT 0,
 status_json text, stop_json text,
 poll_after_ms bigint NOT NULL DEFAULT 0,
 created_ms bigint NOT NULL, updated_ms bigint NOT NULL,
 PRIMARY KEY(account,turn),
 FOREIGN KEY(account,turn) REFERENCES chat.turns_v2(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('chat.turn_tasks_v1','account','account','move','delete',true,'{}','immutable native task association and primary native status/control records follow their owned turn');
