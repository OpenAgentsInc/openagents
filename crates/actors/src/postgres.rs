//! PostgreSQL ownership for actor transitions and their durable outputs.
//!
//! Handlers run synchronously without I/O. A transaction commits state, events,
//! queued messages, and execution intents together. Execution happens elsewhere.
//! Queued public authority must be refreshed by the embedding service before a
//! handler runs; retaining a caller document is not a membership check.
use crate::{Pool, Registry, types::*};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio_postgres::{Row, Transaction};

const MIGRATION: &str = include_str!("../migrations/0001.sql");
const MAX_BYTES: usize = 256 * 1024;
const MAX_PENDING: i64 = 1024;
const MAX_COMMANDS: usize = 100;
const RECEIPT_MS: i64 = 86_400_000;
const TRANSACTION_MS: u128 = 1900;
/// Re-resolves a previously authenticated principal against current authority.
pub type Revalidator =
    Arc<dyn Fn(Caller) -> Pin<Box<dyn Future<Output = Result<Caller>> + Send>> + Send + Sync>;

#[derive(Clone)]
pub struct PgStore {
    pub(crate) pool: Pool,
    pub(crate) registry: Arc<Registry>,
    revalidator: Option<Revalidator>,
}
impl PgStore {
    pub fn new(pool: Pool, registry: Arc<Registry>) -> Self {
        Self {
            pool,
            registry,
            revalidator: None,
        }
    }
    #[must_use]
    pub fn with_revalidator(mut self, revalidator: Revalidator) -> Self {
        self.revalidator = Some(revalidator);
        self
    }
    /// Applies this crate's independently namespaced migration under a database lock.
    pub async fn migrate(&self) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        tx.execute("SELECT pg_advisory_xact_lock(1868652916, 1633907827)", &[])
            .await?;
        tx.batch_execute("CREATE SCHEMA IF NOT EXISTS actor; CREATE TABLE IF NOT EXISTS actor.schema_migrations (version integer PRIMARY KEY, digest text NOT NULL, applied_at bigint NOT NULL)").await?;
        let digest = digest_bytes(MIGRATION.as_bytes());
        if let Some(row) = tx
            .query_opt(
                "SELECT digest FROM actor.schema_migrations WHERE version=1",
                &[],
            )
            .await?
        {
            if row.get::<_, String>(0) != digest {
                return Err(ActorError::new(
                    "migration_conflict",
                    "The installed storage version differs from this build.",
                ));
            }
        } else {
            tx.batch_execute(MIGRATION).await?;
            tx.execute(
                "INSERT INTO actor.schema_migrations(version,digest,applied_at) VALUES(1,$1,$2)",
                &[&digest, &timestamp_ms()],
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Calls a public action, creating and calling atomically when input is supplied.
    pub async fn call(&self, caller: &Caller, req: ActionRequest) -> Result<ActionReply> {
        validate_id(&req.id)?;
        validate_caller(caller, &req.id)?;
        if req.message.origin != Origin::Action {
            return Err(forbidden());
        }
        bounded(&req.message)?;
        if let Some(input) = &req.input {
            bounded(input)?;
        }
        validate_key(req.idempotency_key.as_deref())?;
        let hash = request_hash(
            &json!({"operation":"call", "request":req, "principal":caller.principal}),
        )?;
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let started = Instant::now();
        let mut snapshot = lookup_tx(&tx, &req.id, true).await?;
        let now = database_now(&tx).await?;
        let mut creation_commands = Vec::new();
        if snapshot.is_none() {
            let input = req.input.clone().ok_or_else(not_found)?;
            let uid = random_id()?;
            let created = self.registry.create(&req.id, &uid, input, caller, now)?;
            validate_prepared(&created)?;
            let owner = if self.registry.is_private(&req.id.actor_type)? {
                Some(caller.account_id.clone().ok_or_else(forbidden)?)
            } else {
                None
            };
            let state_version = checked_i32(created.state_version)?;
            let inserted = tx.execute("INSERT INTO actor.instances(uid,workspace_id,actor_type,actor_key,owner,state_version,state,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$8) ON CONFLICT(workspace_id,actor_type,actor_key) DO NOTHING", &[&uid,&req.id.workspace_id,&req.id.actor_type,&req.id.key,&owner,&state_version,&created.state,&now]).await?;
            snapshot = lookup_tx(&tx, &req.id, true).await?;
            if inserted == 1 {
                creation_commands = created.commands;
            }
        }
        let mut snapshot = snapshot.ok_or_else(not_found)?;
        let now = database_now(&tx).await?;
        ensure_private(&snapshot, caller)?;
        ensure_live(&snapshot)?;
        self.registry.authorize(&snapshot, &req.message, caller)?;
        if let Some(key) = &req.idempotency_key {
            if let Some(row) = tx.query_opt("SELECT request_hash,reply FROM actor.receipts WHERE uid=$1 AND principal=$2 AND idempotency_key=$3", &[&snapshot.uid,&caller.principal,key]).await? {
                if row.get::<_,String>(0) != hash { return Err(idempotency_conflict()); }
                let mut reply: ActionReply = serde_json::from_value(row.get(1))?;
                reply.replayed = true;
                tx.commit().await?;
                return Ok(reply);
            }
        }
        if req
            .expected_version
            .is_some_and(|version| version != snapshot.version)
        {
            return Err(ActorError::new(
                "version_conflict",
                "The record changed. Refresh it and try again.",
            ));
        }
        let fenced = match &req.fence {
            Some(fence) => {
                Some(crate::queue_store::verify_fence(&tx, caller, &snapshot.uid, fence).await?)
            }
            None => None,
        };
        let mut prepared =
            self.registry
                .apply_fenced(&snapshot, &req.message, caller, now, fenced)?;
        if !creation_commands.is_empty() {
            if prepared.read_only {
                return Err(ActorError::new(
                    "invalid_handler",
                    "Creation cannot be completed by this read-only action.",
                ));
            }
            creation_commands.append(&mut prepared.commands);
            prepared.commands = creation_commands;
        }
        let reply_value = prepared.reply.clone();
        persist_transition(
            &tx,
            &self.registry,
            &mut snapshot,
            prepared,
            caller,
            &req.message,
            &hash,
            now,
        )
        .await?;
        deadline(started)?;
        let reply = ActionReply {
            reply: reply_value,
            version: snapshot.version,
            event_seq: snapshot.event_seq,
            replayed: false,
        };
        if let Some(key) = &req.idempotency_key {
            tx.execute("INSERT INTO actor.receipts(uid,principal,idempotency_key,request_hash,reply,created_at,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&snapshot.uid,&caller.principal,key,&hash,&serde_json::to_value(&reply)?,&now,&now.saturating_add(RECEIPT_MS)]).await?;
        }
        notify(&tx, &snapshot.uid).await?;
        deadline(started)?;
        tx.commit().await?;
        Ok(reply)
    }

    pub async fn view(&self, caller: &Caller, id: &ActorId) -> Result<ViewReply> {
        let snapshot = self.read_authorized(caller, id).await?;
        Ok(ViewReply {
            view: self.registry.view(&snapshot, caller)?,
            version: snapshot.version,
            event_seq: snapshot.event_seq,
        })
    }

    pub async fn enqueue(
        &self,
        caller: &Caller,
        id: &ActorId,
        message: Envelope,
        idem: Option<&str>,
    ) -> Result<InboxReceipt> {
        validate_id(id)?;
        validate_caller(caller, id)?;
        validate_key(idem)?;
        bounded(&message)?;
        if message.origin != Origin::Inbox {
            return Err(forbidden());
        }
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let snapshot = lookup_tx(&tx, id, true).await?.ok_or_else(not_found)?;
        let now = database_now(&tx).await?;
        ensure_live(&snapshot)?;
        ensure_private(&snapshot, caller)?;
        self.registry.authorize(&snapshot, &message, caller)?;
        let result = enqueue_tx(&tx, &snapshot.uid, &message, caller, idem, now).await?;
        notify(&tx, &snapshot.uid).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn inbox(&self, caller: &Caller, id: &ActorId, seq: u64) -> Result<InboxReceipt> {
        let snapshot = self.read_authorized(caller, id).await?;
        let connection = self.pool.acquire().await?;
        let row = connection.query_opt("SELECT seq,state,reply,error,message FROM actor.inbox WHERE uid=$1 AND seq=$2 AND principal=$3", &[&snapshot.uid,&checked_i64(seq)?,&caller.principal]).await?.ok_or_else(not_found)?;
        let message: Envelope = serde_json::from_value(row.get("message"))?;
        self.registry.authorize(&snapshot, &message, caller)?;
        receipt_from_row(&row)
    }

    /// Reads raw event payloads for an explicitly authorized operator.
    pub async fn events(
        &self,
        caller: &Caller,
        id: &ActorId,
        after: u64,
        limit: usize,
    ) -> Result<EventPage> {
        let snapshot = self.inspect(caller, id).await?;
        let connection = self.pool.acquire().await?;
        let oldest: Option<i64> = connection
            .query_one(
                "SELECT min(seq) FROM actor.events WHERE uid=$1",
                &[&snapshot.uid],
            )
            .await?
            .get(0);
        let reset = after > snapshot.event_seq
            || oldest.is_some_and(|first| after.saturating_add(1) < first as u64)
            || (oldest.is_none() && after < snapshot.event_seq);
        let rows = connection.query("SELECT uid,seq,version,name,payload,at FROM actor.events WHERE uid=$1 AND seq>$2 AND seq<=$3 ORDER BY seq LIMIT $4", &[&snapshot.uid,&checked_i64(after)?,&checked_i64(snapshot.event_seq)?,&bounded_limit(limit)?]).await?;
        let events = rows
            .into_iter()
            .map(|row| {
                Ok(EventRecord {
                    uid: row.get("uid"),
                    seq: nonnegative(row.get("seq"))?,
                    version: nonnegative(row.get("version"))?,
                    event: Event {
                        name: row.get("name"),
                        payload: row.get("payload"),
                    },
                    at: row.get("at"),
                })
            })
            .collect::<Result<_>>()?;
        Ok(EventPage {
            events,
            reset,
            latest_seq: snapshot.event_seq,
        })
    }

    /// Processes at most one ordered message per selected actor. Slow/revoked callers
    /// are resolved before a row is locked, and all handler outputs share its commit.
    pub async fn dispatch_once(&self, limit: usize) -> Result<usize> {
        let connection = self.pool.acquire().await?;
        let now: i64 = connection
            .query_one(
                "SELECT (extract(epoch FROM clock_timestamp())*1000)::bigint",
                &[],
            )
            .await?
            .get(0);
        let rows = connection.query("SELECT a.uid FROM actor.instances a WHERE a.status='live' AND (SELECT i.retry_at FROM actor.inbox i WHERE i.uid=a.uid AND i.state='pending' ORDER BY i.seq LIMIT 1)<=$1 ORDER BY a.last_dispatch_at,a.uid LIMIT $2 FOR UPDATE OF a SKIP LOCKED", &[&now,&bounded_limit(limit)?]).await?;
        drop(connection);
        let mut dispatched = 0;
        for row in rows {
            if self.dispatch_actor(&row.get::<_, String>(0)).await? {
                dispatched += 1;
            }
        }
        Ok(dispatched)
    }

    async fn dispatch_actor(&self, uid: &str) -> Result<bool> {
        let connection = self.pool.acquire().await?;
        let Some(observed) = connection.query_opt("SELECT seq,caller,origin FROM actor.inbox WHERE uid=$1 AND state='pending' ORDER BY seq LIMIT 1", &[&uid]).await? else { return Ok(false); };
        let seq: i64 = observed.get("seq");
        let saved: Caller = serde_json::from_value(observed.get("caller"))?;
        let internal = observed.get::<_, String>("origin") == "internal";
        drop(connection);
        let refreshed = if internal {
            if saved.role != Role::Service {
                Err(forbidden())
            } else {
                Ok(saved.clone())
            }
        } else if let Some(check) = &self.revalidator {
            match tokio::time::timeout(Duration::from_secs(2), check(saved.clone())).await {
                Ok(Ok(fresh))
                    if fresh.principal == saved.principal
                        && fresh.workspace_id == saved.workspace_id
                        && fresh.account_id == saved.account_id =>
                {
                    Ok(fresh)
                }
                Ok(Ok(_)) => Err(forbidden()),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(ActorError::retry(
                    "authorization_unavailable",
                    "Access could not be checked. Try again shortly.",
                )),
            }
        } else {
            Err(ActorError::retry(
                "authorization_unavailable",
                "Queued access must be checked by the hosting service.",
            ))
        };
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let started = Instant::now();
        let Some(row) = tx.query_opt("SELECT * FROM actor.instances WHERE uid=$1 AND status='live' FOR UPDATE SKIP LOCKED", &[&uid]).await? else { return Ok(false); };
        let mut snapshot = snapshot_from_row(&row)?;
        let now = database_now(&tx).await?;
        let Some(row) = tx.query_opt("SELECT * FROM actor.inbox WHERE uid=$1 AND state='pending' ORDER BY seq LIMIT 1 FOR UPDATE", &[&uid]).await? else { return Ok(false); };
        if row.get::<_, i64>("seq") != seq || row.get::<_, i64>("retry_at") > now {
            return Ok(false);
        }
        let message: Envelope = serde_json::from_value(row.get("message"))?;
        let attempts: i32 = row.get("attempts");
        let hash: String = row.get("payload_hash");
        tx.batch_execute("SAVEPOINT actor_transition").await?;
        let result = match refreshed {
            Ok(caller) => {
                match ensure_private(&snapshot, &caller)
                    .and_then(|()| self.registry.apply(&snapshot, &message, &caller, now))
                {
                    Ok(prepared) => {
                        let reply = prepared.reply.clone();
                        persist_transition(
                            &tx,
                            &self.registry,
                            &mut snapshot,
                            prepared,
                            &caller,
                            &message,
                            &hash,
                            now,
                        )
                        .await
                        .map(|()| reply)
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(reply) => {
                tx.execute("UPDATE actor.inbox SET state='done',reply=$3,error=NULL,attempts=attempts+1,updated_at=$4 WHERE uid=$1 AND seq=$2", &[&uid,&seq,&reply,&now]).await?;
            }
            Err(error) => {
                tx.batch_execute("ROLLBACK TO SAVEPOINT actor_transition")
                    .await?;
                let version_ahead = matches!(error.code.as_str(), "version_ahead" | "unknown_type");
                let unknown = error.code == "unknown_message";
                let skew_grace =
                    unknown && now.saturating_sub(row.get::<_, i64>("created_at")) < 3_600_000;
                let deferred = version_ahead || skew_grace;
                let retry = error.retryable && attempts < 7;
                let state = if deferred {
                    "pending"
                } else if unknown {
                    "dead"
                } else if retry {
                    "pending"
                } else if error.retryable {
                    "dead"
                } else {
                    "error"
                };
                let delay = if unknown {
                    30_000
                } else {
                    1000_i64.saturating_mul(1_i64 << attempts.clamp(0, 6))
                };
                let increment: i32 = if deferred { 0 } else { 1 };
                tx.execute("UPDATE actor.inbox SET state=$3,error=$4,attempts=attempts+$7,retry_at=$5,updated_at=$6 WHERE uid=$1 AND seq=$2", &[&uid,&seq,&state,&serde_json::to_value(&error)?,&now.saturating_add(delay),&now,&increment]).await?;
                // Old processes defer to a newer revision without consuming an attempt.
                if state == "dead" {
                    tx.execute(
                        "UPDATE actor.instances SET status='blocked' WHERE uid=$1",
                        &[&uid],
                    )
                    .await?;
                }
            }
        }
        tx.execute(
            "UPDATE actor.instances SET last_dispatch_at=$2 WHERE uid=$1",
            &[&uid, &now],
        )
        .await?;
        deadline(started)?;
        notify(&tx, uid).await?;
        tx.commit().await?;
        Ok(true)
    }

    async fn read_authorized(&self, caller: &Caller, id: &ActorId) -> Result<Snapshot> {
        validate_id(id)?;
        validate_caller(caller, id)?;
        let connection = self.pool.acquire().await?;
        let row = connection.query_opt("SELECT * FROM actor.instances WHERE workspace_id=$1 AND actor_type=$2 AND actor_key=$3", &[&id.workspace_id,&id.actor_type,&id.key]).await?.ok_or_else(not_found)?;
        let snapshot = snapshot_from_row(&row)?;
        ensure_private(&snapshot, caller)?;
        if snapshot.status == Status::Destroyed {
            return Err(not_found());
        }
        self.registry.view(&snapshot, caller)?;
        Ok(snapshot)
    }

    pub async fn inspect(&self, caller: &Caller, id: &ActorId) -> Result<Snapshot> {
        require_admin(caller, id)?;
        let connection = self.pool.acquire().await?;
        let row = connection.query_opt("SELECT * FROM actor.instances WHERE workspace_id=$1 AND actor_type=$2 AND actor_key=$3", &[&id.workspace_id,&id.actor_type,&id.key]).await?.ok_or_else(not_found)?;
        let snapshot = snapshot_from_row(&row)?;
        ensure_private(&snapshot, caller)?;
        Ok(snapshot)
    }
    pub async fn list(
        &self,
        caller: &Caller,
        actor_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Snapshot>> {
        if caller.role != Role::Admin {
            return Err(forbidden());
        }
        let connection = self.pool.acquire().await?;
        connection.query("SELECT * FROM actor.instances WHERE workspace_id=$1 AND ($2::text IS NULL OR actor_type=$2) AND (owner IS NULL OR owner=$4) ORDER BY actor_type,actor_key LIMIT $3", &[&caller.workspace_id,&actor_type,&bounded_limit(limit)?,&caller.account_id]).await?.iter().map(snapshot_from_row).collect()
    }
    pub async fn history(
        &self,
        caller: &Caller,
        id: &ActorId,
        after: i64,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let snapshot = self.inspect(caller, id).await?;
        let connection = self.pool.acquire().await?;
        Ok(connection.query("SELECT to_jsonb(h) FROM actor.history h WHERE uid=$1 AND id>$2 ORDER BY id LIMIT $3", &[&snapshot.uid,&after,&bounded_limit(limit)?]).await?.into_iter().map(|row| row.get(0)).collect())
    }
    /// The caller's own private actors of `actor_type` in its workspace,
    /// newest first, each with the view the caller is allowed. A record the
    /// caller can't view is left out. This is how a host lists one account's
    /// records without an index actor; it reads one bounded page.
    pub async fn list_own(
        &self,
        caller: &Caller,
        actor_type: &str,
        limit: usize,
    ) -> Result<Vec<(ActorId, ViewReply)>> {
        let account = caller
            .account_id
            .as_deref()
            .filter(|account| !account.is_empty())
            .ok_or_else(forbidden)?;
        if caller.workspace_id.is_empty() || caller.principal.is_empty() {
            return Err(not_found());
        }
        if !self.registry.is_private(actor_type)? {
            return Err(forbidden());
        }
        let connection = self.pool.acquire().await?;
        let rows = connection
            .query(
                "SELECT * FROM actor.instances WHERE workspace_id=$1 AND actor_type=$2 AND owner=$3 \
                 AND status<>'destroyed' ORDER BY created_at DESC, uid LIMIT $4",
                &[&caller.workspace_id, &actor_type, &account, &bounded_limit(limit)?],
            )
            .await?;
        drop(connection);
        let mut found = Vec::with_capacity(rows.len());
        for row in &rows {
            let snapshot = snapshot_from_row(row)?;
            if let Ok(view) = self.registry.view(&snapshot, caller) {
                found.push((
                    snapshot.id.clone(),
                    ViewReply {
                        view,
                        version: snapshot.version,
                        event_seq: snapshot.event_seq,
                    },
                ));
            }
        }
        Ok(found)
    }

    pub async fn admin_block(&self, caller: &Caller, id: &ActorId, blocked: bool) -> Result<()> {
        require_admin(caller, id)?;
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let snapshot = lookup_tx(&tx, id, true).await?.ok_or_else(not_found)?;
        ensure_private(&snapshot, caller)?;
        if snapshot.status == Status::Destroyed {
            return Err(not_found());
        }
        let status = if blocked { "blocked" } else { "live" };
        let now = database_now(&tx).await?;
        tx.execute(
            "UPDATE actor.instances SET status=$2,updated_at=$3 WHERE uid=$1",
            &[&snapshot.uid, &status, &now],
        )
        .await?;
        write_history(
            &tx,
            &snapshot,
            caller,
            if blocked {
                "admin:block"
            } else {
                "admin:unblock"
            },
            &request_hash(&json!({"blocked":blocked}))?,
            now,
        )
        .await?;
        notify(&tx, &snapshot.uid).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Explicitly retries retained failed work; the dispatcher rechecks current authority.
    pub async fn retry_inbox(&self, caller: &Caller, id: &ActorId, seq: u64) -> Result<()> {
        require_admin(caller, id)?;
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let snapshot = lookup_tx(&tx, id, true).await?.ok_or_else(not_found)?;
        ensure_private(&snapshot, caller)?;
        if snapshot.status == Status::Destroyed {
            return Err(not_found());
        }
        let seq = checked_i64(seq)?;
        let row = tx
            .query_opt(
                "SELECT state,error,attempts FROM actor.inbox WHERE uid=$1 AND seq=$2 FOR UPDATE",
                &[&snapshot.uid, &seq],
            )
            .await?
            .ok_or_else(not_found)?;
        if row.get::<_, String>("state") == "done" {
            return Err(ActorError::new(
                "conflict",
                "That request already finished.",
            ));
        }
        let now = database_now(&tx).await?;
        let hash = request_hash(
            &json!({"seq":seq,"state":row.get::<_,String>("state"),"error":row.get::<_,Option<Value>>("error"),"attempts":row.get::<_,i32>("attempts")}),
        )?;
        pending_inbox_bound_for_retry(&tx, &snapshot.uid, seq).await?;
        tx.execute("UPDATE actor.inbox SET state='pending',reply=NULL,error=NULL,attempts=0,retry_at=$3,updated_at=$3 WHERE uid=$1 AND seq=$2", &[&snapshot.uid,&seq,&now]).await?;
        tx.execute(
            "UPDATE actor.instances SET status='live',updated_at=$2 WHERE uid=$1",
            &[&snapshot.uid, &now],
        )
        .await?;
        write_history(&tx, &snapshot, caller, "admin:retry_inbox", &hash, now).await?;
        notify(&tx, &snapshot.uid).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn destroy(&self, caller: &Caller, id: &ActorId) -> Result<()> {
        require_admin(caller, id)?;
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let snapshot = lookup_tx(&tx, id, true).await?.ok_or_else(not_found)?;
        ensure_private(&snapshot, caller)?;
        let now = database_now(&tx).await?;
        destroy_tx(&tx, &snapshot.uid, now).await?;
        write_history(
            &tx,
            &snapshot,
            caller,
            "admin:destroy",
            &request_hash(id)?,
            now,
        )
        .await?;
        notify(&tx, &snapshot.uid).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Exports one owner, refusing oversized exports rather than silently truncating.
    pub async fn export(&self, caller: &Caller, id: &ActorId) -> Result<Value> {
        require_admin(caller, id)?;
        let mut connection = self.pool.acquire().await?;
        let tx = connection
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await?;
        let snapshot = lookup_tx(&tx, id, false).await?.ok_or_else(not_found)?;
        ensure_private(&snapshot, caller)?;
        let mut tables = serde_json::Map::new();
        let mut total = serde_json::to_vec(&snapshot)?.len() as i64;
        for table in [
            "inbox", "events", "receipts", "history", "alarms", "work", "effects",
        ] {
            // Count bytes inside PostgreSQL before bringing any payloads into memory.
            // The same repeatable-read transaction fixes the rows for both queries.
            let budget_query = format!(
                "SELECT count(*)::bigint AS count,COALESCE(sum(octet_length(to_jsonb(t)::text)),0)::bigint AS bytes FROM (SELECT * FROM actor.{table} WHERE uid=$1 LIMIT 2049) t"
            );
            let budget = tx.query_one(&budget_query, &[&snapshot.uid]).await?;
            let count: i64 = budget.get("count");
            total = total
                .saturating_add(budget.get::<_, i64>("bytes"))
                .saturating_add(count * 2 + 128);
            if count > 2048 || total > 16 * 1024 * 1024 {
                return Err(ActorError::new(
                    "export_too_large",
                    "This export needs a paginated operator backup.",
                ));
            }
            let query = format!("SELECT to_jsonb(t) FROM actor.{table} t WHERE uid=$1 LIMIT 2048");
            let rows = tx.query(&query, &[&snapshot.uid]).await?;
            let values: Vec<Value> = rows.into_iter().map(|row| row.get(0)).collect();
            tables.insert(table.to_string(), Value::Array(values));
        }
        tx.commit().await?;
        Ok(
            json!({"schema":"openagents.actor.export.v1","snapshot":snapshot,"tables":tables,"contains_private_data":true}),
        )
    }
    /// Removes bounded expired history. Business deduplication must outlive RPC receipts.
    pub async fn retention(
        &self,
        event_before: i64,
        history_before: i64,
        receipt_before: i64,
        limit: usize,
    ) -> Result<u64> {
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let limit = bounded_limit(limit)?;
        let mut removed = tx.execute("DELETE FROM actor.events WHERE ctid IN (SELECT ctid FROM actor.events WHERE at<$1 ORDER BY at LIMIT $2)", &[&event_before,&limit]).await?;
        removed += tx.execute("DELETE FROM actor.history WHERE id IN (SELECT id FROM actor.history WHERE at<$1 ORDER BY id LIMIT $2)", &[&history_before,&limit]).await?;
        removed += tx.execute("DELETE FROM actor.receipts WHERE ctid IN (SELECT ctid FROM actor.receipts WHERE expires_at<$1 ORDER BY expires_at LIMIT $2)", &[&receipt_before,&limit]).await?;
        tx.commit().await?;
        Ok(removed)
    }
}

async fn persist_transition(
    tx: &Transaction<'_>,
    registry: &Registry,
    snapshot: &mut Snapshot,
    prepared: Prepared,
    caller: &Caller,
    message: &Envelope,
    input_hash: &str,
    now: i64,
) -> Result<()> {
    validate_prepared(&prepared)?;
    if prepared.read_only {
        if !prepared.commands.is_empty()
            || prepared.state != snapshot.state
            || prepared.state_version != snapshot.state_version
        {
            return Err(ActorError::new(
                "invalid_handler",
                "A read-only action attempted to change data.",
            ));
        }
        return Ok(());
    }
    snapshot.version = snapshot.version.checked_add(1).ok_or_else(exhausted)?;
    snapshot.state = prepared.state;
    snapshot.state_version = prepared.state_version;
    snapshot.updated_at = now;
    let mut destroyed = false;
    for command in prepared.commands {
        if destroyed {
            return Err(ActorError::new(
                "invalid_handler",
                "Destroy must be the last command.",
            ));
        }
        match command {
            Command::Emit { event } => {
                validate_name(&event.name, 128)?;
                bounded(&event.payload)?;
                snapshot.event_seq = snapshot.event_seq.checked_add(1).ok_or_else(exhausted)?;
                tx.execute("INSERT INTO actor.events(uid,seq,version,name,payload,at) VALUES($1,$2,$3,$4,$5,$6)", &[&snapshot.uid,&checked_i64(snapshot.event_seq)?,&checked_i64(snapshot.version)?,&event.name,&event.payload,&now]).await?;
            }
            Command::Send {
                to,
                mut message,
                idempotency_key,
            } => {
                validate_id(&to)?;
                if to.workspace_id != snapshot.id.workspace_id || message.origin == Origin::Internal
                {
                    return Err(forbidden());
                }
                message.origin = Origin::Inbox;
                let target = lookup_tx(tx, &to, true).await?.ok_or_else(not_found)?;
                ensure_live(&target)?;
                ensure_private(&target, caller)?;
                registry.authorize(&target, &message, caller)?;
                enqueue_tx(
                    tx,
                    &target.uid,
                    &message,
                    caller,
                    Some(&idempotency_key),
                    now,
                )
                .await?;
                notify(tx, &target.uid).await?;
            }
            Command::Schedule { mut alarm } => {
                validate_name(&alarm.name, 128)?;
                bounded(&alarm.message)?;
                if alarm
                    .interval_ms
                    .is_some_and(|interval| !(1..=31_622_400_000).contains(&interval))
                    || alarm.due_at < 0
                    || alarm.message.origin == Origin::Internal
                {
                    return Err(ActorError::new(
                        "bad_args",
                        "The alarm settings are invalid.",
                    ));
                }
                alarm.message.origin = Origin::Inbox;
                registry.authorize(snapshot, &alarm.message, caller)?;
                let count: i64 = tx
                    .query_one(
                        "SELECT count(*) FROM actor.alarms WHERE uid=$1 AND name<>$2",
                        &[&snapshot.uid, &alarm.name],
                    )
                    .await?
                    .get(0);
                if count >= 128 {
                    return Err(exhausted());
                }
                tx.execute("INSERT INTO actor.alarms(uid,name,due_at,interval_ms,message,caller,generation) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(uid,name) DO UPDATE SET due_at=EXCLUDED.due_at,interval_ms=EXCLUDED.interval_ms,message=EXCLUDED.message,caller=EXCLUDED.caller,generation=EXCLUDED.generation", &[&snapshot.uid,&alarm.name,&alarm.due_at,&alarm.interval_ms,&serde_json::to_value(&alarm.message)?,&serde_json::to_value(caller)?,&checked_i64(snapshot.version)?]).await?;
            }
            Command::CancelAlarm { name } => {
                tx.execute(
                    "DELETE FROM actor.alarms WHERE uid=$1 AND name=$2",
                    &[&snapshot.uid, &name],
                )
                .await?;
            }
            Command::Effect { effect } => {
                validate_name(&effect.id, 256)?;
                validate_name(&effect.kind, 128)?;
                crate::core::validate_json(&effect.payload, 64 * 1024)?;
                if !(1..=600_000).contains(&effect.timeout_ms)
                    || !(1..=100).contains(&effect.max_attempts)
                {
                    return Err(ActorError::new(
                        "bad_args",
                        "The effect limits are invalid.",
                    ));
                }
                let retry = retry_name(effect.retry);
                if let Some(row) = tx.query_opt("SELECT kind,payload,timeout_ms,max_attempts,retry_policy FROM actor.effects WHERE uid=$1 AND effect_id=$2", &[&snapshot.uid,&effect.id]).await? {
                    if row.get::<_,String>("kind") != effect.kind || row.get::<_,Value>("payload") != effect.payload || row.get::<_,i64>("timeout_ms") != effect.timeout_ms || row.get::<_,i32>("max_attempts") != checked_i32(effect.max_attempts)? || row.get::<_,String>("retry_policy") != retry { return Err(idempotency_conflict()); }
                } else {
                    pending_bound(tx,&snapshot.uid,"effects").await?;
                    tx.execute("INSERT INTO actor.effects(uid,effect_id,workspace_id,kind,payload,timeout_ms,max_attempts,retry_policy,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)", &[&snapshot.uid,&effect.id,&snapshot.id.workspace_id,&effect.kind,&effect.payload,&effect.timeout_ms,&checked_i32(effect.max_attempts)?,&retry,&now]).await?;
                }
            }
            Command::Work { work } => {
                validate_name(&work.item_id, 256)?;
                validate_name(&work.queue, 256)?;
                crate::core::validate_json(&work.payload, 64 * 1024)?;
                if let Some(target) = &work.target {
                    validate_name(target, 256)?;
                }
                if !(1..=86_400_000).contains(&work.lease_ms)
                    || !(1..=100).contains(&work.max_attempts)
                {
                    return Err(ActorError::new("bad_args", "The work limits are invalid."));
                }
                let retry = retry_name(work.retry);
                if let Some(row) = tx.query_opt("SELECT queue,target,payload,lease_ms,max_attempts,retry_policy FROM actor.work WHERE uid=$1 AND item_id=$2", &[&snapshot.uid,&work.item_id]).await? {
                    if row.get::<_,String>("queue") != work.queue || row.get::<_,Option<String>>("target") != work.target || row.get::<_,Value>("payload") != work.payload || row.get::<_,i64>("lease_ms") != work.lease_ms || row.get::<_,i32>("max_attempts") != checked_i32(work.max_attempts)? || row.get::<_,String>("retry_policy") != retry { return Err(idempotency_conflict()); }
                } else {
                    pending_bound(tx,&snapshot.uid,"work").await?;
                    tx.execute("INSERT INTO actor.work(uid,item_id,workspace_id,queue,target,payload,lease_ms,max_attempts,retry_policy,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10)", &[&snapshot.uid,&work.item_id,&snapshot.id.workspace_id,&work.queue,&work.target,&work.payload,&work.lease_ms,&checked_i32(work.max_attempts)?,&retry,&now]).await?;
                }
            }
            Command::CancelWork { item_id } => {
                tx.execute("UPDATE actor.work SET cancel=true,epoch=CASE WHEN state='pending' THEN epoch+1 ELSE epoch END,state=CASE WHEN state='pending' THEN 'cancelled' ELSE state END,updated_at=$3 WHERE uid=$1 AND item_id=$2 AND state IN ('pending','claimed','uncertain')", &[&snapshot.uid,&item_id,&now]).await?;
            }
            Command::Destroy => {
                destroy_tx(tx, &snapshot.uid, now).await?;
                snapshot.status = Status::Destroyed;
                destroyed = true;
            }
        }
    }
    tx.execute("UPDATE actor.instances SET state=$2,state_version=$3,version=$4,event_seq=$5,updated_at=$6 WHERE uid=$1", &[&snapshot.uid,&snapshot.state,&checked_i32(snapshot.state_version)?,&checked_i64(snapshot.version)?,&checked_i64(snapshot.event_seq)?,&now]).await?;
    write_history(tx, snapshot, caller, &message.name, input_hash, now).await?;
    Ok(())
}

async fn pending_inbox_bound_for_retry(tx: &Transaction<'_>, uid: &str, seq: i64) -> Result<()> {
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM actor.inbox WHERE uid=$1 AND seq<>$2 AND state='pending'",
            &[&uid, &seq],
        )
        .await?
        .get(0);
    if count >= MAX_PENDING {
        return Err(exhausted());
    }
    Ok(())
}
async fn pending_bound(tx: &Transaction<'_>, uid: &str, table: &str) -> Result<()> {
    // `table` is selected only by the two fixed call sites above.
    let query = format!(
        "SELECT count(*) FROM actor.{table} WHERE uid=$1 AND state IN ('pending','claimed','uncertain')"
    );
    let count: i64 = tx.query_one(&query, &[&uid]).await?.get(0);
    if count >= MAX_PENDING {
        return Err(exhausted());
    }
    Ok(())
}
async fn destroy_tx(tx: &Transaction<'_>, uid: &str, now: i64) -> Result<()> {
    tx.execute(
        "UPDATE actor.instances SET status='destroyed',updated_at=$2 WHERE uid=$1",
        &[&uid, &now],
    )
    .await?;
    tx.execute("UPDATE actor.inbox SET state='dead',error=$2,updated_at=$3 WHERE uid=$1 AND state='pending'", &[&uid,&serde_json::to_value(ActorError::new("destroyed", "The record was deleted."))?,&now]).await?;
    tx.execute("DELETE FROM actor.alarms WHERE uid=$1", &[&uid])
        .await?;
    tx.execute("UPDATE actor.work SET cancel=true,epoch=epoch+1,state=CASE WHEN state IN ('claimed','uncertain') THEN 'uncertain' ELSE 'cancelled' END,heartbeat_until=NULL,updated_at=$2 WHERE uid=$1 AND state IN ('pending','claimed','uncertain')", &[&uid,&now]).await?;
    tx.execute("UPDATE actor.effects SET cancel=true,state=CASE WHEN state IN ('claimed','uncertain') THEN 'uncertain' ELSE 'cancelled' END,token=NULL,claimed_until=NULL,updated_at=$2 WHERE uid=$1 AND state IN ('pending','claimed','uncertain')", &[&uid,&now]).await?;
    Ok(())
}
async fn write_history(
    tx: &Transaction<'_>,
    snapshot: &Snapshot,
    caller: &Caller,
    operation: &str,
    input_hash: &str,
    now: i64,
) -> Result<()> {
    tx.execute("INSERT INTO actor.history(uid,version,principal,operation,input_hash,state_hash,at) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&snapshot.uid,&checked_i64(snapshot.version)?,&caller.principal,&operation,&input_hash,&request_hash(&snapshot.state)?,&now]).await?;
    Ok(())
}
async fn lookup_tx(tx: &Transaction<'_>, id: &ActorId, lock: bool) -> Result<Option<Snapshot>> {
    let query = if lock {
        "SELECT * FROM actor.instances WHERE workspace_id=$1 AND actor_type=$2 AND actor_key=$3 FOR UPDATE"
    } else {
        "SELECT * FROM actor.instances WHERE workspace_id=$1 AND actor_type=$2 AND actor_key=$3"
    };
    tx.query_opt(query, &[&id.workspace_id, &id.actor_type, &id.key])
        .await?
        .as_ref()
        .map(snapshot_from_row)
        .transpose()
}

/// Called only by trusted queue completion code while the actor row is locked.
pub(crate) async fn enqueue_internal_tx(
    tx: &Transaction<'_>,
    uid: &str,
    message: &Envelope,
    idempotency_key: &str,
    now: i64,
) -> Result<()> {
    let row = tx
        .query_one(
            "SELECT workspace_id,owner,status FROM actor.instances WHERE uid=$1",
            &[&uid],
        )
        .await?;
    if row.get::<_, String>("status") == "destroyed" {
        return Err(not_found());
    }
    let caller = Caller {
        principal: "actor-runtime".into(),
        workspace_id: row.get("workspace_id"),
        account_id: row.get("owner"),
        role: Role::Service,
        executor: None,
    };
    let mut message = message.clone();
    message.origin = Origin::Internal;
    enqueue_tx(tx, uid, &message, &caller, Some(idempotency_key), now).await?;
    notify(tx, uid).await
}
/// An alarm preserves the authority of the caller who scheduled it.
pub(crate) async fn enqueue_alarm_tx(
    tx: &Transaction<'_>,
    uid: &str,
    message: &Envelope,
    caller: &Caller,
    idempotency_key: &str,
    now: i64,
) -> Result<()> {
    let mut message = message.clone();
    message.origin = Origin::Inbox;
    enqueue_tx(tx, uid, &message, caller, Some(idempotency_key), now).await?;
    notify(tx, uid).await
}
async fn enqueue_tx(
    tx: &Transaction<'_>,
    uid: &str,
    message: &Envelope,
    caller: &Caller,
    idem: Option<&str>,
    now: i64,
) -> Result<InboxReceipt> {
    validate_key(idem)?;
    bounded(message)?;
    if serde_json::to_vec(caller)?.len() > 24 * 1024 {
        return Err(exhausted());
    }
    let hash = request_hash(&json!({"message":message,"principal":caller.principal}))?;
    if let Some(key) = idem {
        if let Some(row) = tx.query_opt("SELECT seq,state,reply,error,payload_hash FROM actor.inbox WHERE uid=$1 AND principal=$2 AND idempotency_key=$3", &[&uid,&caller.principal,&key]).await? {
            if row.get::<_,String>("payload_hash") != hash { return Err(idempotency_conflict()); }
            return receipt_from_row(&row);
        }
    }
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM actor.inbox WHERE uid=$1 AND state='pending'",
            &[&uid],
        )
        .await?
        .get(0);
    if count >= MAX_PENDING {
        return Err(exhausted());
    }
    let seq: i64 = tx
        .query_one(
            "UPDATE actor.instances SET inbox_seq=inbox_seq+1 WHERE uid=$1 RETURNING inbox_seq",
            &[&uid],
        )
        .await?
        .get(0);
    let origin = match message.origin {
        Origin::Internal => "internal",
        Origin::Inbox => "inbox",
        Origin::Action => return Err(forbidden()),
    };
    tx.execute("INSERT INTO actor.inbox(uid,seq,message,caller,origin,payload_hash,idempotency_key,principal,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)", &[&uid,&seq,&serde_json::to_value(message)?,&serde_json::to_value(caller)?,&origin,&hash,&idem,&caller.principal,&now]).await?;
    Ok(InboxReceipt {
        seq: nonnegative(seq)?,
        state: "pending".into(),
        reply: None,
        error: None,
    })
}
pub(crate) fn snapshot_from_row(row: &Row) -> Result<Snapshot> {
    let status = match row.get::<_, String>("status").as_str() {
        "live" => Status::Live,
        "blocked" => Status::Blocked,
        "destroyed" => Status::Destroyed,
        _ => return Err(ActorError::new("corrupt", "Stored status is invalid.")),
    };
    Ok(Snapshot {
        uid: row.get("uid"),
        id: ActorId {
            workspace_id: row.get("workspace_id"),
            actor_type: row.get("actor_type"),
            key: row.get("actor_key"),
        },
        owner: row.get("owner"),
        status,
        state_version: u32::try_from(row.get::<_, i32>("state_version"))
            .map_err(|_| exhausted())?,
        version: nonnegative(row.get("version"))?,
        event_seq: nonnegative(row.get("event_seq"))?,
        inbox_seq: nonnegative(row.get("inbox_seq"))?,
        state: row.get("state"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
fn receipt_from_row(row: &Row) -> Result<InboxReceipt> {
    Ok(InboxReceipt {
        seq: nonnegative(row.get("seq"))?,
        state: row.get("state"),
        reply: row.get("reply"),
        error: row
            .get::<_, Option<Value>>("error")
            .map(serde_json::from_value)
            .transpose()?,
    })
}
async fn database_now(tx: &Transaction<'_>) -> Result<i64> {
    Ok(tx
        .query_one(
            "SELECT (extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .await?
        .get(0))
}
async fn notify(tx: &Transaction<'_>, uid: &str) -> Result<()> {
    tx.execute("SELECT pg_notify('actors_changed',$1)", &[&uid])
        .await?;
    Ok(())
}
pub(crate) fn timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}
fn random_id() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| ActorError::retry("unavailable", "A new record could not be created."))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
fn request_hash(value: &impl serde::Serialize) -> Result<String> {
    let value = canonical(serde_json::to_value(value)?);
    Ok(digest_bytes(&serde_json::to_vec(&value)?))
}
fn canonical(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: std::collections::BTreeMap<_, _> = map
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
        other => other,
    }
}
fn digest_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn validate_prepared(prepared: &Prepared) -> Result<()> {
    bounded(&prepared.state)?;
    bounded(&prepared.reply)?;
    if prepared.state_version == 0 || prepared.commands.len() > MAX_COMMANDS {
        return Err(ActorError::new(
            "invalid_handler",
            "The action returned invalid data.",
        ));
    }
    // Bound the entire transaction's queued content, not only each individual item.
    if serde_json::to_vec(&prepared.commands)?.len() > 1024 * 1024 {
        return Err(exhausted());
    }
    Ok(())
}
fn bounded(value: &impl serde::Serialize) -> Result<()> {
    if serde_json::to_vec(value)?.len() > MAX_BYTES {
        return Err(ActorError::new(
            "too_large",
            "The data is too large. Store its contents separately.",
        ));
    }
    Ok(())
}
fn validate_id(id: &ActorId) -> Result<()> {
    crate::core::validate_actor_id(id)
}
fn validate_name(value: &str, max: usize) -> Result<()> {
    if value.is_empty()
        || value.len() > max
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
    {
        return Err(ActorError::new("bad_args", "The record name is invalid."));
    }
    Ok(())
}
fn validate_key(key: Option<&str>) -> Result<()> {
    if key.is_some_and(|key| key.is_empty() || key.len() > 256 || key.chars().any(char::is_control))
    {
        return Err(ActorError::new("bad_args", "The request key is invalid."));
    }
    Ok(())
}
fn validate_caller(caller: &Caller, id: &ActorId) -> Result<()> {
    if caller.workspace_id != id.workspace_id {
        return Err(not_found());
    }
    if caller.principal.is_empty() || caller.principal.len() > 256 {
        return Err(forbidden());
    }
    Ok(())
}
fn ensure_private(snapshot: &Snapshot, caller: &Caller) -> Result<()> {
    validate_caller(caller, &snapshot.id)?;
    if snapshot.owner.is_some() && snapshot.owner != caller.account_id {
        return Err(not_found());
    }
    Ok(())
}
fn ensure_live(snapshot: &Snapshot) -> Result<()> {
    match snapshot.status {
        Status::Live => Ok(()),
        Status::Blocked => Err(ActorError::new(
            "blocked",
            "This record is paused for review.",
        )),
        Status::Destroyed => Err(not_found()),
    }
}
fn require_admin(caller: &Caller, id: &ActorId) -> Result<()> {
    validate_id(id)?;
    validate_caller(caller, id)?;
    if caller.role != Role::Admin {
        Err(forbidden())
    } else {
        Ok(())
    }
}
fn checked_i64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| exhausted())
}
fn checked_i32(value: u32) -> Result<i32> {
    i32::try_from(value).map_err(|_| exhausted())
}
fn nonnegative(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|_| ActorError::new("corrupt", "Stored counters are invalid."))
}
fn bounded_limit(limit: usize) -> Result<i64> {
    if limit == 0 || limit > 1024 {
        Err(ActorError::new(
            "bad_args",
            "Choose a limit between 1 and 1024.",
        ))
    } else {
        Ok(limit as i64)
    }
}
fn retry_name(retry: RetryPolicy) -> &'static str {
    match retry {
        RetryPolicy::Idempotent => "idempotent",
        RetryPolicy::Reconcile => "reconcile",
    }
}
fn deadline(start: Instant) -> Result<()> {
    if start.elapsed().as_millis() >= TRANSACTION_MS {
        Err(ActorError::retry(
            "deadline",
            "The change took too long. Try again.",
        ))
    } else {
        Ok(())
    }
}
fn exhausted() -> ActorError {
    ActorError::retry("capacity", "This record has reached its current limit.")
}
fn forbidden() -> ActorError {
    ActorError::new("forbidden", "You do not have access to this record.")
}
fn not_found() -> ActorError {
    ActorError::new("not_found", "The record was not found.")
}
fn idempotency_conflict() -> ActorError {
    ActorError::new(
        "idempotency_conflict",
        "That request key was already used for different input.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes_are_order_independent_and_semantically_bound() {
        let left = serde_json::from_str::<Value>(r#"{"x":1,"nested":{"a":2,"b":3}}"#).unwrap();
        let right = serde_json::from_str::<Value>(r#"{"nested":{"b":3,"a":2},"x":1}"#).unwrap();
        assert_eq!(request_hash(&left).unwrap(), request_hash(&right).unwrap());
        assert_ne!(
            request_hash(&left).unwrap(),
            request_hash(&json!({"x":2})).unwrap()
        );
    }
    #[test]
    fn names_and_limits_do_not_accept_unbounded_or_sql_like_input() {
        assert!(validate_name("host/one:run-1", 256).is_ok());
        assert!(validate_name("x'; DROP TABLE actor.instances", 256).is_err());
        assert!(validate_key(Some("\n")).is_err());
        assert!(bounded_limit(1025).is_err());
        assert!(checked_i64(u64::MAX).is_err());
    }
    #[test]
    fn private_authority_does_not_follow_shared_membership() {
        let id = ActorId {
            workspace_id: "shared".into(),
            actor_type: "example".into(),
            key: "a".into(),
        };
        let caller = Caller {
            principal: "alice".into(),
            workspace_id: "shared".into(),
            account_id: Some("alice".into()),
            role: Role::Member,
            executor: None,
        };
        let snapshot = Snapshot {
            uid: "one".into(),
            id,
            owner: Some("bob".into()),
            status: Status::Live,
            state_version: 1,
            version: 0,
            event_seq: 0,
            inbox_seq: 0,
            state: json!({}),
            created_at: 0,
            updated_at: 0,
        };
        assert!(ensure_private(&snapshot, &caller).is_err());
    }
}

#[cfg(test)]
mod postgres_tests {
    use super::*;
    use crate::{Actor, Ctx, Definition, Handles, Message};
    use serde::Deserialize;

    struct Probe;
    impl Actor for Probe {
        const TYPE: &'static str = "test.transaction";
        const STATE_VERSION: u32 = 1;
        const PRIVATE: bool = true;
        type State = i64;
        type Input = Value;
        fn create(_: Value, _: &mut Ctx) -> Result<i64> {
            Ok(0)
        }
        fn wake(_: &i64) -> Result<Self> {
            Ok(Self)
        }
        fn view(state: &i64, _: &Caller) -> Result<Value> {
            Ok(json!(state))
        }
    }
    #[derive(Deserialize)]
    struct Read;
    impl Message for Read {
        const NAME: &'static str = "read@1";
        const READ_ONLY: bool = true;
        type Reply = i64;
    }
    impl Handles<Read> for Probe {
        fn handle(&mut self, state: &mut i64, _: Read, _: &mut Ctx) -> Result<i64> {
            Ok(*state)
        }
    }
    #[derive(Deserialize)]
    struct Stage {
        target: ActorId,
    }
    impl Message for Stage {
        const NAME: &'static str = "stage@1";
        type Reply = i64;
    }
    impl Handles<Stage> for Probe {
        fn handle(&mut self, state: &mut i64, message: Stage, ctx: &mut Ctx) -> Result<i64> {
            *state += 1;
            ctx.emit("changed@1", json!({"count":*state}))?;
            ctx.work(WorkSpec {
                item_id: String::new(),
                queue: "test".into(),
                target: None,
                payload: json!({"task":"one"}),
                lease_ms: 1000,
                max_attempts: 2,
                retry: RetryPolicy::Reconcile,
            })?;
            ctx.effect(EffectSpec {
                id: String::new(),
                kind: "test.effect".into(),
                payload: json!({"input":"one"}),
                timeout_ms: 1000,
                max_attempts: 2,
                retry: RetryPolicy::Reconcile,
            })?;
            ctx.schedule(AlarmSpec {
                name: "later".into(),
                due_at: ctx.now() + 60_000,
                message: Envelope {
                    name: "read@1".into(),
                    args: Value::Null,
                    origin: Origin::Inbox,
                },
                interval_ms: None,
            })?;
            ctx.send(
                message.target,
                Envelope {
                    name: "add@1".into(),
                    args: json!({"delta":1}),
                    origin: Origin::Inbox,
                },
            )?;
            Ok(*state)
        }
    }
    struct Shared;
    impl Actor for Shared {
        const TYPE: &'static str = "test.shared";
        const STATE_VERSION: u32 = 1;
        type State = Value;
        type Input = Value;
        fn create(input: Value, _: &mut Ctx) -> Result<Value> {
            Ok(input)
        }
        fn wake(_: &Value) -> Result<Self> {
            Ok(Self)
        }
        fn view(_: &Value, _: &Caller) -> Result<Value> {
            Ok(json!({"public":true}))
        }
    }
    #[derive(Deserialize)]
    struct OwnerRead;
    impl Message for OwnerRead {
        const NAME: &'static str = "owner.read@1";
        const ACCESS: Access = Access::Owner;
        const READ_ONLY: bool = true;
        type Reply = Value;
    }
    impl Handles<OwnerRead> for Shared {
        fn handle(&mut self, state: &mut Value, _: OwnerRead, _: &mut Ctx) -> Result<Value> {
            Ok(state.clone())
        }
    }
    #[derive(Deserialize)]
    struct ScheduleOnly {
        due_at: i64,
    }
    impl Message for ScheduleOnly {
        const NAME: &'static str = "schedule@1";
        type Reply = ();
    }
    impl Handles<ScheduleOnly> for Probe {
        fn handle(&mut self, _: &mut i64, message: ScheduleOnly, ctx: &mut Ctx) -> Result<()> {
            ctx.schedule(AlarmSpec {
                name: "later".into(),
                due_at: message.due_at,
                message: Envelope {
                    name: "read@1".into(),
                    args: Value::Null,
                    origin: Origin::Inbox,
                },
                interval_ms: None,
            })
        }
    }
    fn action(
        id: &ActorId,
        key: &str,
        name: &str,
        args: Value,
        input: Option<Value>,
    ) -> ActionRequest {
        ActionRequest {
            id: id.clone(),
            message: Envelope {
                name: name.into(),
                args,
                origin: Origin::Action,
            },
            input,
            idempotency_key: Some(key.into()),
            expected_version: None,
            fence: None,
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn postgres_atomic_transitions_replays_authority_order_and_destroy() {
        let Ok(dsn) = std::env::var("ACTORS_TEST_DATABASE_URL") else {
            eprintln!("PostgreSQL integration test requires ACTORS_TEST_DATABASE_URL");
            return;
        };
        let pool = Pool::new(&dsn, 4).unwrap();
        let mut registry = crate::example::registry().unwrap();
        registry
            .register(
                Definition::<Probe>::new()
                    .message::<Read>()
                    .message::<Stage>()
                    .message::<ScheduleOnly>(),
            )
            .unwrap();
        registry
            .register(Definition::<Shared>::new().message::<OwnerRead>())
            .unwrap();
        let store = PgStore::new(pool.clone(), Arc::new(registry))
            .with_revalidator(Arc::new(|caller| Box::pin(async move { Ok(caller) })));
        store.migrate().await.unwrap();
        store.migrate().await.unwrap();
        let workspace = format!("test-store-{}", random_id().unwrap());
        let caller = Caller {
            principal: "alice".into(),
            workspace_id: workspace.clone(),
            account_id: Some("alice".into()),
            role: Role::Member,
            executor: None,
        };
        let mut admin = caller.clone();
        admin.role = Role::Admin;
        let id = ActorId {
            workspace_id: workspace.clone(),
            actor_type: "example.counter".into(),
            key: "counter".into(),
        };
        let request = action(
            &id,
            "start",
            "add@1",
            json!({"delta":1}),
            Some(json!({"initial":0})),
        );
        let first = store.call(&caller, request.clone()).await.unwrap();
        assert_eq!(first.reply, json!(1));
        assert_eq!(first.version, 1);
        let again = store.call(&caller, request).await.unwrap();
        assert!(again.replayed);
        assert_eq!(again.version, 1);
        let conflict = store
            .call(
                &caller,
                action(
                    &id,
                    "start",
                    "add@1",
                    json!({"delta":2}),
                    Some(json!({"initial":0})),
                ),
            )
            .await
            .unwrap_err();
        assert_eq!(conflict.code, "idempotency_conflict");
        let mut tasks = Vec::new();
        for index in 0..8 {
            let store = store.clone();
            let caller = caller.clone();
            let id = id.clone();
            tasks.push(tokio::spawn(async move {
                store
                    .call(
                        &caller,
                        action(
                            &id,
                            &format!("parallel-{index}"),
                            "add@1",
                            json!({"delta":1}),
                            None,
                        ),
                    )
                    .await
            }));
        }
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(store.view(&caller, &id).await.unwrap().view["value"], 9);
        let mut stranger = caller.clone();
        stranger.principal = "bob".into();
        stranger.account_id = Some("bob".into());
        assert!(store.view(&stranger, &id).await.is_err());
        stranger.role = Role::Admin;
        assert!(store.inspect(&stranger, &id).await.is_err());
        assert!(store.list(&stranger, None, 10).await.unwrap().is_empty());
        let pending = store
            .enqueue(
                &caller,
                &id,
                Envelope {
                    name: "add@1".into(),
                    args: json!({"delta":2}),
                    origin: Origin::Inbox,
                },
                Some("queue-first"),
            )
            .await
            .unwrap();
        let duplicate = store
            .enqueue(
                &caller,
                &id,
                Envelope {
                    name: "add@1".into(),
                    args: json!({"delta":2}),
                    origin: Origin::Inbox,
                },
                Some("queue-first"),
            )
            .await
            .unwrap();
        assert_eq!(pending.seq, duplicate.seq);
        let second = store
            .enqueue(
                &caller,
                &id,
                Envelope {
                    name: "add@1".into(),
                    args: json!({"delta":3}),
                    origin: Origin::Inbox,
                },
                Some("queue-second"),
            )
            .await
            .unwrap();
        let uid = store.inspect(&admin, &id).await.unwrap().uid;
        pool.acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.inbox SET retry_at=9223372036854775807 WHERE uid=$1 AND seq=$2",
                &[&uid, &(pending.seq as i64)],
            )
            .await
            .unwrap();
        assert!(!store.dispatch_actor(&uid).await.unwrap());
        assert_eq!(
            store.inbox(&caller, &id, second.seq).await.unwrap().state,
            "pending"
        );
        pool.acquire()
            .await
            .unwrap()
            .execute("UPDATE actor.inbox SET retry_at=0 WHERE uid=$1", &[&uid])
            .await
            .unwrap();
        assert!(store.dispatch_actor(&uid).await.unwrap());
        assert!(store.dispatch_actor(&uid).await.unwrap());
        assert_eq!(store.view(&caller, &id).await.unwrap().view["value"], 14);
        let denied = store
            .enqueue(
                &caller,
                &id,
                Envelope {
                    name: "add@1".into(),
                    args: json!({"delta":10}),
                    origin: Origin::Inbox,
                },
                Some("revoked"),
            )
            .await
            .unwrap();
        let revoked = store
            .clone()
            .with_revalidator(Arc::new(|_| Box::pin(async { Err(forbidden()) })));
        revoked.dispatch_actor(&uid).await.unwrap();
        assert_eq!(
            store.inbox(&caller, &id, denied.seq).await.unwrap().state,
            "error"
        );
        assert_eq!(store.view(&caller, &id).await.unwrap().view["value"], 14);

        // A safe shared view does not grant access to another principal's raw reply.
        let shared = ActorId {
            workspace_id: workspace.clone(),
            actor_type: Shared::TYPE.into(),
            key: "shared".into(),
        };
        let mut owner = caller.clone();
        owner.role = Role::Owner;
        store
            .call(
                &owner,
                action(
                    &shared,
                    "create-shared",
                    "owner.read@1",
                    Value::Null,
                    Some(json!({"secret":42})),
                ),
            )
            .await
            .unwrap();
        let owner_reply = store
            .enqueue(
                &owner,
                &shared,
                Envelope {
                    name: "owner.read@1".into(),
                    args: Value::Null,
                    origin: Origin::Inbox,
                },
                None,
            )
            .await
            .unwrap();
        let shared_uid = store.inspect(&admin, &shared).await.unwrap().uid;
        store.dispatch_actor(&shared_uid).await.unwrap();
        assert_eq!(
            store.view(&stranger, &shared).await.unwrap().view,
            json!({"public":true})
        );
        assert_eq!(
            store
                .inbox(&stranger, &shared, owner_reply.seq)
                .await
                .unwrap_err()
                .code,
            "not_found"
        );
        assert_eq!(
            store
                .inbox(&caller, &shared, owner_reply.seq)
                .await
                .unwrap_err()
                .code,
            "forbidden"
        );
        assert_eq!(
            store
                .inbox(&owner, &shared, owner_reply.seq)
                .await
                .unwrap()
                .reply,
            Some(json!({"secret":42}))
        );
        assert_eq!(
            store
                .events(&caller, &shared, 0, 10)
                .await
                .unwrap_err()
                .code,
            "forbidden"
        );
        assert!(store.events(&admin, &shared, 0, 10).await.is_ok());

        // A late command failure must roll back state, events, work, effects and alarms.
        let probe = ActorId {
            workspace_id: workspace.clone(),
            actor_type: Probe::TYPE.into(),
            key: "atomic".into(),
        };
        store
            .call(
                &caller,
                action(&probe, "create", "read@1", Value::Null, Some(Value::Null)),
            )
            .await
            .unwrap();
        let probe_uid = store.inspect(&admin, &probe).await.unwrap().uid;
        // A rolling deployment must not poison messages that this process cannot read.
        let read = Envelope {
            name: "read@1".into(),
            args: Value::Null,
            origin: Origin::Inbox,
        };
        let skewed = store
            .enqueue(&caller, &probe, read.clone(), Some("version-skew"))
            .await
            .unwrap();
        pool.acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.instances SET state_version=2 WHERE uid=$1",
                &[&probe_uid],
            )
            .await
            .unwrap();
        store.dispatch_actor(&probe_uid).await.unwrap();
        let row=pool.acquire().await.unwrap().query_one("SELECT a.status,i.state,i.attempts FROM actor.instances a JOIN actor.inbox i USING(uid) WHERE uid=$1 AND seq=$2", &[&probe_uid,&(skewed.seq as i64)]).await.unwrap();
        assert_eq!(row.get::<_, String>("status"), "live");
        assert_eq!(row.get::<_, String>("state"), "pending");
        assert_eq!(row.get::<_, i32>("attempts"), 0);
        pool.acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.instances SET state_version=1 WHERE uid=$1",
                &[&probe_uid],
            )
            .await
            .unwrap();
        pool.acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.inbox SET retry_at=0 WHERE uid=$1",
                &[&probe_uid],
            )
            .await
            .unwrap();
        store.dispatch_actor(&probe_uid).await.unwrap();
        let future = store
            .enqueue(&caller, &probe, read.clone(), Some("message-skew"))
            .await
            .unwrap();
        let future_message = Envelope {
            name: "future@2".into(),
            ..read.clone()
        };
        pool.acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.inbox SET message=$3 WHERE uid=$1 AND seq=$2",
                &[
                    &probe_uid,
                    &(future.seq as i64),
                    &serde_json::to_value(future_message).unwrap(),
                ],
            )
            .await
            .unwrap();
        store.dispatch_actor(&probe_uid).await.unwrap();
        let row=pool.acquire().await.unwrap().query_one("SELECT state,attempts,retry_at-updated_at AS delay FROM actor.inbox WHERE uid=$1 AND seq=$2", &[&probe_uid,&(future.seq as i64)]).await.unwrap();
        assert_eq!(row.get::<_, String>("state"), "pending");
        assert_eq!(row.get::<_, i32>("attempts"), 0);
        assert_eq!(row.get::<_, i64>("delay"), 30_000);
        pool.acquire().await.unwrap().execute("UPDATE actor.inbox SET retry_at=0,created_at=updated_at-3600001 WHERE uid=$1 AND seq=$2", &[&probe_uid,&(future.seq as i64)]).await.unwrap();
        store.dispatch_actor(&probe_uid).await.unwrap();
        let dead: String = pool
            .acquire()
            .await
            .unwrap()
            .query_one(
                "SELECT state FROM actor.inbox WHERE uid=$1 AND seq=$2",
                &[&probe_uid, &(future.seq as i64)],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(dead, "dead");
        pool.acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.inbox SET message=$3 WHERE uid=$1 AND seq=$2",
                &[
                    &probe_uid,
                    &(future.seq as i64),
                    &serde_json::to_value(read).unwrap(),
                ],
            )
            .await
            .unwrap();
        store.retry_inbox(&admin, &probe, future.seq).await.unwrap();
        store.dispatch_actor(&probe_uid).await.unwrap();
        assert_eq!(
            store
                .inbox(&caller, &probe, future.seq)
                .await
                .unwrap()
                .state,
            "done"
        );
        let missing = ActorId {
            key: "missing".into(),
            ..id.clone()
        };
        assert!(
            store
                .call(
                    &caller,
                    action(
                        &probe,
                        "bad-stage",
                        "stage@1",
                        json!({"target":missing}),
                        None
                    )
                )
                .await
                .is_err()
        );
        assert_eq!(store.view(&caller, &probe).await.unwrap().view, 0);
        for table in ["events", "work", "effects", "alarms"] {
            let count: i64 = pool
                .acquire()
                .await
                .unwrap()
                .query_one(
                    &format!("SELECT count(*) FROM actor.{table} WHERE uid=$1"),
                    &[&probe_uid],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(count, 0, "partial commit in {table}");
        }
        store
            .call(
                &caller,
                action(&probe, "good-stage", "stage@1", json!({"target":id}), None),
            )
            .await
            .unwrap();
        for table in ["events", "work", "effects", "alarms"] {
            let count: i64 = pool
                .acquire()
                .await
                .unwrap()
                .query_one(
                    &format!("SELECT count(*) FROM actor.{table} WHERE uid=$1"),
                    &[&probe_uid],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(count, 1, "missing commit in {table}");
        }
        let history = store.history(&admin, &probe, 0, 100).await.unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].get("input_hash").is_some());
        assert!(history[0].get("payload").is_none());
        let export = store.export(&admin, &probe).await.unwrap();
        assert_eq!(export["tables"]["work"].as_array().unwrap().len(), 1);
        // Recreating a fired one-shot at the same time must use a fresh occurrence key.
        let previous = pool
            .acquire()
            .await
            .unwrap()
            .query_one(
                "SELECT generation,due_at FROM actor.alarms WHERE uid=$1",
                &[&probe_uid],
            )
            .await
            .unwrap();
        let old_generation: i64 = previous.get("generation");
        let same_due: i64 = previous.get("due_at");
        pool.acquire()
            .await
            .unwrap()
            .execute("DELETE FROM actor.alarms WHERE uid=$1", &[&probe_uid])
            .await
            .unwrap();
        store
            .call(
                &caller,
                action(
                    &probe,
                    "schedule-again",
                    "schedule@1",
                    json!({"due_at":same_due}),
                    None,
                ),
            )
            .await
            .unwrap();
        let new_generation: i64 = pool
            .acquire()
            .await
            .unwrap()
            .query_one(
                "SELECT generation FROM actor.alarms WHERE uid=$1",
                &[&probe_uid],
            )
            .await
            .unwrap()
            .get(0);
        assert!(new_generation > old_generation);
        store.admin_block(&admin, &probe, true).await.unwrap();
        assert!(
            store
                .call(
                    &caller,
                    action(&probe, "blocked", "read@1", Value::Null, None)
                )
                .await
                .is_err()
        );
        store.admin_block(&admin, &probe, false).await.unwrap();
        store.destroy(&admin, &probe).await.unwrap();
        assert!(store.view(&caller, &probe).await.is_err());
        let destroyed = store.inspect(&admin, &probe).await.unwrap();
        assert_eq!(destroyed.status, Status::Destroyed);
        assert_eq!(destroyed.state, 1); // Resource references remain for explicit reconciliation.
        let state: String = pool
            .acquire()
            .await
            .unwrap()
            .query_one("SELECT state FROM actor.work WHERE uid=$1", &[&probe_uid])
            .await
            .unwrap()
            .get(0);
        assert_eq!(state, "cancelled");
        // A failed create+call must leave no owner or child records.
        let absent = ActorId {
            key: "creation-failed".into(),
            ..probe.clone()
        };
        assert!(
            store
                .call(
                    &caller,
                    action(
                        &absent,
                        "failed-create",
                        "stage@1",
                        json!({"target":missing}),
                        Some(Value::Null)
                    )
                )
                .await
                .is_err()
        );
        assert!(store.inspect(&admin, &absent).await.is_err());
        pool.acquire()
            .await
            .unwrap()
            .execute(
                "DELETE FROM actor.instances WHERE workspace_id=$1",
                &[&workspace],
            )
            .await
            .unwrap();
    }
}
