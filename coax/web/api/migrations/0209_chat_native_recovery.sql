-- Retain existing primary transcript requests/pages and Stop requests verbatim.
-- Coax admits every record; these keys provide ownership, ordering and fences.
ALTER TABLE chat.turn_tasks_v1 ADD COLUMN capture_json text;
CREATE TABLE chat.task_capture_pages_v1 (
 account text NOT NULL, turn text NOT NULL, page int NOT NULL,
 record text NOT NULL, accepted_ms bigint NOT NULL,
 PRIMARY KEY(account,turn,page),
 FOREIGN KEY(account,turn) REFERENCES chat.turn_tasks_v1(account,turn) ON UPDATE CASCADE ON DELETE CASCADE
);
CREATE TABLE chat.task_stop_commands_v1 (
 account text NOT NULL, turn text NOT NULL, command text NOT NULL,
 record text NOT NULL, created_ms bigint NOT NULL,
 PRIMARY KEY(account,turn,command),
 FOREIGN KEY(account,turn) REFERENCES chat.turn_tasks_v1(account,turn) ON UPDATE CASCADE ON DELETE CASCADE
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('chat.task_capture_pages_v1','account','account','move','delete',true,'{}','private primary transcript pages follow their owned turn'),
 ('chat.task_stop_commands_v1','account','account','move','delete',true,'{}','retained primary Stop attempts follow their owned turn');
