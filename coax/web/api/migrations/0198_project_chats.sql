-- Associations do not change the frozen v1 saved transcript record.
CREATE TABLE chat.project_threads_v1 (
 account text NOT NULL, thread text NOT NULL, project text NOT NULL,
 PRIMARY KEY (account,thread),
 FOREIGN KEY (account,thread) REFERENCES chat.account_threads_v1(account,id) ON UPDATE CASCADE ON DELETE CASCADE,
 CHECK (project ~ '^[a-z0-9][a-z0-9-]{0,63}$')
);
CREATE INDEX project_threads_lookup_v1 ON chat.project_threads_v1(account,project);

INSERT INTO auth.life_columns (tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('chat.project_threads_v1','account','account','move','delete',true,'{}','account-owned project chat associations; transcript remains v1');
