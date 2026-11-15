-- Canonical private Projects; policy is projects-policy.coax, not SQL functions.
CREATE TABLE projects.private_v1 (
 id text PRIMARY KEY, account text NOT NULL REFERENCES auth.accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
 name text NOT NULL, description text NOT NULL, instructions text NOT NULL,
 revision bigint NOT NULL, instructions_revision bigint NOT NULL, instructions_updated_ms bigint NOT NULL,
 created_ms bigint NOT NULL, updated_ms bigint NOT NULL,
 UNIQUE(account,id)
);
CREATE INDEX private_project_owner_v1 ON projects.private_v1(account,id);
CREATE TABLE projects.knowledge_v1 (
 id text PRIMARY KEY, project text NOT NULL REFERENCES projects.private_v1(id) ON DELETE CASCADE,
 title text NOT NULL, body text NOT NULL, file_id text NOT NULL DEFAULT '', selected bool NOT NULL,
 revision bigint NOT NULL, created_ms bigint NOT NULL, updated_ms bigint NOT NULL
);
CREATE INDEX project_knowledge_v1 ON projects.knowledge_v1(project,id);
CREATE TABLE projects.thread_links_v1 (
 account text NOT NULL, thread text NOT NULL, project text NOT NULL,
 PRIMARY KEY(account,thread),
 FOREIGN KEY(account,project) REFERENCES projects.private_v1(account,id) ON UPDATE CASCADE ON DELETE CASCADE,
 FOREIGN KEY(account,thread) REFERENCES chat.account_threads_v1(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
CREATE TABLE projects.operations_v1 (
 account text NOT NULL REFERENCES auth.accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
 operation text NOT NULL, path text NOT NULL, digest text NOT NULL,
 state text NOT NULL, status int NOT NULL, reply text NOT NULL,
 created_ms bigint NOT NULL, PRIMARY KEY(account,operation)
);
CREATE TABLE projects.contexts_v1 (
 account text NOT NULL, project text NOT NULL, operation text NOT NULL,
 thread text NOT NULL, revision bigint NOT NULL, thread_revision bigint NOT NULL, snapshot text NOT NULL, input_json text NOT NULL,
 PRIMARY KEY(account,operation),
 UNIQUE(account,thread,thread_revision),
 FOREIGN KEY(account,project) REFERENCES projects.private_v1(account,id) ON UPDATE CASCADE ON DELETE CASCADE,
 FOREIGN KEY(account,thread) REFERENCES chat.account_threads_v1(account,id) ON UPDATE CASCADE ON DELETE CASCADE
);
INSERT INTO auth.life_columns(tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('projects.private_v1','account','account','move','delete',true,'{}','canonical private Projects; no signing identity'),
 ('projects.thread_links_v1','account','account','move','delete',true,'{}','private Project associations preserve frozen transcripts'),
 ('projects.operations_v1','account','account','move','delete',true,'{}','private mutation receipts; never dispatch an uncertain model call again'),
 ('projects.contexts_v1','account','account','move','delete',true,'{}','private admitted Project context snapshots');
-- Child knowledge has no account column: its project FK cascades deletion.
