CREATE TABLE actor.instances (
 uid text PRIMARY KEY CHECK (length(uid) BETWEEN 1 AND 128),
 workspace_id text NOT NULL CHECK (length(workspace_id) BETWEEN 1 AND 256),
 actor_type text NOT NULL CHECK (length(actor_type) BETWEEN 1 AND 128),
 actor_key text NOT NULL CHECK (length(actor_key) BETWEEN 1 AND 256),
 owner text,
 status text NOT NULL DEFAULT 'live' CHECK (status IN ('live','blocked','destroyed')),
 state_version integer NOT NULL CHECK (state_version > 0),
 version bigint NOT NULL DEFAULT 0 CHECK (version >= 0),
 event_seq bigint NOT NULL DEFAULT 0 CHECK (event_seq >= 0),
 inbox_seq bigint NOT NULL DEFAULT 0 CHECK (inbox_seq >= 0),
 state jsonb NOT NULL CHECK (octet_length(state::text) <= 262144),
 created_at bigint NOT NULL, updated_at bigint NOT NULL,
 last_dispatch_at bigint NOT NULL DEFAULT 0,
 UNIQUE (workspace_id, actor_type, actor_key), UNIQUE (uid, workspace_id)
);
CREATE TABLE actor.inbox (
 uid text NOT NULL REFERENCES actor.instances(uid) ON DELETE CASCADE,
 seq bigint NOT NULL CHECK (seq > 0),
 message jsonb NOT NULL CHECK (octet_length(message::text) <= 262144),
 caller jsonb NOT NULL CHECK (octet_length(caller::text) <= 32768),
 origin text NOT NULL CHECK (origin IN ('inbox','internal')),
 payload_hash text NOT NULL, idempotency_key text, principal text NOT NULL,
 state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','done','error','dead')),
 reply jsonb, error jsonb, attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
 retry_at bigint NOT NULL DEFAULT 0, created_at bigint NOT NULL, updated_at bigint NOT NULL,
 PRIMARY KEY (uid,seq), UNIQUE (uid,principal,idempotency_key)
);
CREATE INDEX actor_inbox_ready ON actor.inbox(uid,seq) WHERE state='pending';
CREATE TABLE actor.events (
 uid text NOT NULL REFERENCES actor.instances(uid) ON DELETE CASCADE,
 seq bigint NOT NULL CHECK (seq > 0), version bigint NOT NULL CHECK (version >= 0),
 name text NOT NULL, payload jsonb NOT NULL CHECK (octet_length(payload::text) <= 262144), at bigint NOT NULL,
 PRIMARY KEY (uid,seq)
);
CREATE INDEX actor_events_age ON actor.events(at);
CREATE TABLE actor.receipts (
 uid text NOT NULL REFERENCES actor.instances(uid) ON DELETE CASCADE,
 principal text NOT NULL, idempotency_key text NOT NULL, request_hash text NOT NULL,
 reply jsonb NOT NULL, created_at bigint NOT NULL, expires_at bigint NOT NULL,
 PRIMARY KEY (uid,principal,idempotency_key)
);
CREATE INDEX actor_receipts_expiry ON actor.receipts(expires_at);
CREATE TABLE actor.history (
 id bigserial PRIMARY KEY,
 uid text NOT NULL REFERENCES actor.instances(uid) ON DELETE CASCADE,
 version bigint NOT NULL, principal text NOT NULL, operation text NOT NULL,
 input_hash text NOT NULL, state_hash text NOT NULL, at bigint NOT NULL
);
CREATE INDEX actor_history_owner ON actor.history(uid,id);
CREATE INDEX actor_history_age ON actor.history(at);
CREATE TABLE actor.alarms (
 uid text NOT NULL REFERENCES actor.instances(uid) ON DELETE CASCADE,
 name text NOT NULL, due_at bigint NOT NULL, interval_ms bigint CHECK (interval_ms > 0),
 message jsonb NOT NULL CHECK (octet_length(message::text) <= 262144),
 caller jsonb NOT NULL CHECK (octet_length(caller::text) <= 32768),
 generation bigint NOT NULL DEFAULT 1 CHECK (generation > 0),
 PRIMARY KEY(uid,name)
);
CREATE INDEX actor_alarms_due ON actor.alarms(due_at);
CREATE TABLE actor.work (
 uid text NOT NULL, item_id text NOT NULL CHECK (length(item_id) BETWEEN 1 AND 256),
 workspace_id text NOT NULL, queue text NOT NULL CHECK (length(queue) BETWEEN 1 AND 256), target text,
 payload jsonb NOT NULL CHECK (octet_length(payload::text) <= 262144),
 lease_ms bigint NOT NULL CHECK (lease_ms BETWEEN 1 AND 86400000),
 max_attempts integer NOT NULL CHECK (max_attempts BETWEEN 1 AND 100),
 retry_policy text NOT NULL CHECK (retry_policy IN ('idempotent','reconcile')),
 state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','claimed','completed','failed','uncertain','cancelled')),
 epoch bigint NOT NULL DEFAULT 0 CHECK (epoch >= 0), owner text,
 executor_generation bigint, heartbeat_until bigint, cancel boolean NOT NULL DEFAULT false,
 attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
 progress_seq bigint NOT NULL DEFAULT 0 CHECK (progress_seq >= 0), progress jsonb, result jsonb,
 created_at bigint NOT NULL, updated_at bigint NOT NULL,
 PRIMARY KEY(uid,item_id),
 FOREIGN KEY(uid,workspace_id) REFERENCES actor.instances(uid,workspace_id) ON DELETE CASCADE
);
CREATE INDEX actor_work_ready ON actor.work(workspace_id,queue,created_at) WHERE state='pending';
CREATE INDEX actor_work_expiry ON actor.work(heartbeat_until) WHERE state='claimed';
CREATE INDEX actor_work_executor ON actor.work(owner,executor_generation) WHERE state='claimed';
CREATE TABLE actor.effects (
 uid text NOT NULL, effect_id text NOT NULL CHECK (length(effect_id) BETWEEN 1 AND 256),
 workspace_id text NOT NULL, kind text NOT NULL CHECK (length(kind) BETWEEN 1 AND 128),
 payload jsonb NOT NULL CHECK (octet_length(payload::text) <= 262144),
 timeout_ms bigint NOT NULL CHECK (timeout_ms BETWEEN 1 AND 600000),
 max_attempts integer NOT NULL CHECK (max_attempts BETWEEN 1 AND 100),
 retry_policy text NOT NULL CHECK (retry_policy IN ('idempotent','reconcile')),
 state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','claimed','completed','failed','uncertain','cancelled')),
 token text, claimed_until bigint, attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
 cancel boolean NOT NULL DEFAULT false, result jsonb, next_attempt_at bigint NOT NULL DEFAULT 0,
 created_at bigint NOT NULL, updated_at bigint NOT NULL,
 PRIMARY KEY(uid,effect_id),
 FOREIGN KEY(uid,workspace_id) REFERENCES actor.instances(uid,workspace_id) ON DELETE CASCADE
);
CREATE INDEX actor_effects_ready ON actor.effects(next_attempt_at,created_at) WHERE state='pending';
CREATE INDEX actor_effects_expiry ON actor.effects(claimed_until) WHERE state='claimed';
