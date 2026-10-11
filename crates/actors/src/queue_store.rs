//! Durable queue transitions. Domain adapters reconcile uncertain external effects.
use crate::postgres::{PgStore, enqueue_alarm_tx, enqueue_internal_tx};
use crate::types::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::{Row, Transaction};

const MAX_BATCH: u32 = 64;
const MAX_PAYLOAD: usize = 64 * 1024;
const EFFECT_LEASE_MS: i64 = 60_000;

fn refused(code: &str) -> ActorError {
    ActorError::new(code, "This operation is no longer available.")
}
fn bounded(value: &Value) -> Result<()> {
    if serde_json::to_vec(value)?.len() > MAX_PAYLOAD {
        return Err(ActorError::new("too_large", "The result is too large."));
    }
    Ok(())
}
fn identifier(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(ActorError::new("bad_args", "The identifier is invalid."));
    }
    Ok(())
}
fn integer(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| refused("bad_args"))
}
fn grant(caller: &Caller, now: i64) -> Result<&ExecutorGrant> {
    let grant = caller
        .executor
        .as_ref()
        .ok_or_else(|| refused("forbidden"))?;
    if grant.expires_at <= now || grant.max_claims == 0 || grant.id.is_empty() {
        return Err(refused("forbidden"));
    }
    integer(grant.generation)?;
    Ok(grant)
}
fn allowed(values: &[String], value: &str) -> bool {
    values.iter().any(|entry| entry == value)
}
fn work_scope(caller: &Caller, actor: &Row, row: &Row, now: i64) -> Result<()> {
    actor_scope(caller, actor)?;
    let grant = grant(caller, now)?;
    let queue: String = row.get("queue");
    let target: Option<String> = row.get("target");
    if !allowed(&grant.queues, &queue)
        || target
            .as_ref()
            .is_some_and(|target| !allowed(&grant.targets, target))
    {
        return Err(refused("forbidden"));
    }
    Ok(())
}
fn actor_access(caller: &Caller, row: &Row) -> Result<()> {
    let owner: Option<String> = row.get("owner");
    if row.get::<_, String>("workspace_id") != caller.workspace_id
        || owner
            .as_ref()
            .is_some_and(|owner| caller.account_id.as_ref() != Some(owner))
    {
        return Err(refused("not_found"));
    }
    Ok(())
}
fn actor_scope(caller: &Caller, row: &Row) -> Result<()> {
    actor_access(caller, row)?;
    if row.get::<_, String>("status") != "live" {
        return Err(refused("not_live"));
    }
    Ok(())
}
fn identity(row: &Row) -> ActorId {
    ActorId {
        workspace_id: row.get("workspace_id"),
        actor_type: row.get("actor_type"),
        key: row.get("actor_key"),
    }
}
fn work_fence(caller: &Caller, row: &Row, epoch: u64, now: i64) -> Result<()> {
    let grant = grant(caller, now)?;
    if row.get::<_, i64>("epoch") != integer(epoch)?
        || row.get::<_, Option<String>>("owner").as_deref() != Some(grant.id.as_str())
        || row.get::<_, i64>("executor_generation") != integer(grant.generation)?
    {
        return Err(refused("stale_claim"));
    }
    Ok(())
}
fn active_work(row: &Row, now: i64) -> Result<()> {
    if row.get::<_, String>("state") != "claimed"
        || row.get::<_, bool>("cancel")
        || row.get::<_, Option<i64>>("heartbeat_until").unwrap_or(0) <= now
    {
        return Err(refused("stale_claim"));
    }
    Ok(())
}
fn active_effect(row: &Row, token: &str, now: i64) -> Result<()> {
    if row.get::<_, String>("state") != "claimed"
        || row.get::<_, bool>("cancel")
        || row.get::<_, Option<String>>("token").as_deref() != Some(token)
        || row.get::<_, Option<i64>>("claimed_until").unwrap_or(0) <= now
    {
        return Err(refused("stale_claim"));
    }
    Ok(())
}
async fn now(tx: &Transaction<'_>) -> Result<i64> {
    Ok(tx
        .query_one(
            "SELECT (extract(epoch FROM clock_timestamp()) * 1000)::bigint AS now",
            &[],
        )
        .await?
        .get("now"))
}
async fn actor(tx: &Transaction<'_>, uid: &str) -> Result<Row> {
    tx.query_opt(
        "SELECT * FROM actor.instances WHERE uid=$1 FOR UPDATE",
        &[&uid],
    )
    .await?
    .ok_or_else(|| refused("not_found"))
}
async fn work(tx: &Transaction<'_>, uid: &str, item: &str) -> Result<Row> {
    tx.query_opt(
        "SELECT * FROM actor.work WHERE uid=$1 AND item_id=$2 FOR UPDATE",
        &[&uid, &item],
    )
    .await?
    .ok_or_else(|| refused("not_found"))
}
async fn effect(tx: &Transaction<'_>, uid: &str, id: &str) -> Result<Row> {
    tx.query_opt(
        "SELECT * FROM actor.effects WHERE uid=$1 AND effect_id=$2 FOR UPDATE",
        &[&uid, &id],
    )
    .await?
    .ok_or_else(|| refused("not_found"))
}
async fn internal(
    tx: &Transaction<'_>,
    uid: &str,
    name: &str,
    args: Value,
    key: &str,
    at: i64,
) -> Result<()> {
    let key = format!("queue:{:x}", Sha256::digest(key.as_bytes()));
    enqueue_internal_tx(
        tx,
        uid,
        &Envelope {
            name: name.into(),
            args,
            origin: Origin::Internal,
        },
        &key,
        at,
    )
    .await
}
async fn inbox_room(tx: &Transaction<'_>, uid: &str) -> Result<bool> {
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM actor.inbox WHERE uid=$1 AND state='pending'",
            &[&uid],
        )
        .await?
        .get(0);
    Ok(count < 1024)
}
async fn resolution_audit(
    tx: &Transaction<'_>,
    a: &Row,
    caller: &Caller,
    operation: &str,
    subject: &str,
    retry: bool,
    outcome: &Option<Value>,
    at: i64,
) -> Result<()> {
    let input_hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(subject, retry, outcome))?)
    );
    let state: Value = a.get("state");
    let state_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&state)?));
    let uid: String = a.get("uid");
    let version: i64 = a.get("version");
    tx.execute("INSERT INTO actor.history(uid,version,principal,operation,input_hash,state_hash,at) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&uid,&version,&caller.principal,&operation,&input_hash,&state_hash,&at]).await?;
    Ok(())
}
fn token() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| ActorError::retry("unavailable", "Try again shortly."))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Check a fenced action's claim inside the action's transaction, with the
/// actor row already locked: the caller's grant holds the item's queue and
/// target, the claim is this executor's at this epoch and generation, and
/// its lease has not ended. A live, uncancelled claim's lease is renewed, so
/// a fenced call counts as a heartbeat; a cancelled one is reported, not
/// renewed, so the executor learns of it and releases the claim.
pub(crate) async fn verify_fence(
    tx: &Transaction<'_>,
    caller: &Caller,
    uid: &str,
    fence: &WorkFence,
) -> Result<Fenced> {
    identifier(&fence.item_id)?;
    let a = actor(tx, uid).await?;
    let row = work(tx, uid, &fence.item_id).await?;
    let at = now(tx).await?;
    work_scope(caller, &a, &row, at)?;
    work_fence(caller, &row, fence.epoch, at)?;
    let until = row.get::<_, Option<i64>>("heartbeat_until").unwrap_or(0);
    if row.get::<_, String>("state") != "claimed" || until <= at {
        return Err(refused("stale_claim"));
    }
    if row.get::<_, bool>("cancel") {
        return Ok(Fenced {
            item_id: fence.item_id.clone(),
            epoch: fence.epoch,
            cancel: true,
            heartbeat_until: until,
        });
    }
    let renewed = at
        .saturating_add(row.get::<_, i64>("lease_ms"))
        .min(grant(caller, at)?.expires_at)
        .max(until);
    tx.execute(
        "UPDATE actor.work SET heartbeat_until=$3,updated_at=$4 WHERE uid=$1 AND item_id=$2",
        &[&uid, &fence.item_id, &renewed, &at],
    )
    .await?;
    Ok(Fenced {
        item_id: fence.item_id.clone(),
        epoch: fence.epoch,
        cancel: false,
        heartbeat_until: renewed,
    })
}

impl PgStore {
    /// Claim work for the authenticated executor. Capacity includes unresolved work.
    pub async fn claim_work(
        &self,
        caller: &Caller,
        queue: &str,
        target: Option<&str>,
        max: u32,
    ) -> Result<Vec<ClaimedWork>> {
        identifier(queue)?;
        if let Some(target) = target {
            identifier(target)?;
        }
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let initial_now = now(&tx).await?;
        let executor = grant(caller, initial_now)?;
        if !allowed(&executor.queues, queue)
            || target.is_some_and(|target| !allowed(&executor.targets, target))
        {
            return Err(refused("forbidden"));
        }
        // Serialize capacity admission across different actors for this executor.
        let capacity_key =
            serde_json::to_string(&("actors.executor", &caller.workspace_id, &executor.id))?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&capacity_key],
        )
        .await?;
        let occupied: i64 = tx.query_one(
            "SELECT count(*) FROM actor.work WHERE workspace_id=$1 AND owner=$2 AND state IN ('claimed','uncertain')",
            &[&caller.workspace_id, &executor.id],
        ).await?.get(0);
        let limit = max.min(MAX_BATCH).min(
            executor
                .max_claims
                .saturating_sub(u32::try_from(occupied).unwrap_or(u32::MAX)),
        );
        if limit == 0 {
            tx.commit().await?;
            return Ok(vec![]);
        }
        let candidates = tx.query(
            "SELECT w.uid,w.item_id FROM actor.work w JOIN actor.instances a USING(uid)
             WHERE w.workspace_id=$1 AND a.workspace_id=$1 AND w.queue=$2 AND w.state='pending' AND NOT w.cancel
             AND a.status='live' AND (a.owner IS NULL OR a.owner=$3)
             AND (w.target IS NULL OR w.target=ANY($4))
             AND ($5::text IS NULL OR w.target IS NULL OR w.target=$5)
             ORDER BY w.updated_at,w.uid,w.item_id LIMIT 512 FOR UPDATE OF a SKIP LOCKED",
            &[&caller.workspace_id, &queue, &caller.account_id, &executor.targets, &target],
        ).await?;
        let mut claimed = Vec::new();
        for candidate in candidates {
            if claimed.len() >= limit as usize {
                break;
            }
            let uid: String = candidate.get("uid");
            let item: String = candidate.get("item_id");
            let Some(a) = tx
                .query_opt(
                    "SELECT * FROM actor.instances WHERE uid=$1 FOR UPDATE SKIP LOCKED",
                    &[&uid],
                )
                .await?
            else {
                continue;
            };
            let row = work(&tx, &uid, &item).await?;
            let at = now(&tx).await?;
            work_scope(caller, &a, &row, at)?;
            if row.get::<_, String>("state") != "pending" || row.get::<_, bool>("cancel") {
                continue;
            }
            let lease: i64 = row.get("lease_ms");
            let until = at.saturating_add(lease).min(executor.expires_at);
            let row = tx.query_one(
                "UPDATE actor.work SET state='claimed',epoch=epoch+1,owner=$3,executor_generation=$4,
                 heartbeat_until=$5,attempts=attempts+1,progress_seq=0,progress=NULL,updated_at=$6
                 WHERE uid=$1 AND item_id=$2 RETURNING *",
                &[&uid, &item, &executor.id, &integer(executor.generation)?, &until, &at],
            ).await?;
            claimed.push(ClaimedWork {
                actor: identity(&a),
                uid,
                item_id: item,
                queue: row.get("queue"),
                payload: row.get("payload"),
                epoch: row.get::<_, i64>("epoch") as u64,
                heartbeat_until: until,
                cancel: false,
                attempts: row.get::<_, i32>("attempts") as u32,
            });
        }
        tx.commit().await?;
        Ok(claimed)
    }

    /// [`PgStore::claim_work`], waiting up to `wait` (at most 30 seconds)
    /// for work to arrive when none is ready: a long-poll claim. Each look
    /// is its own short transaction, so waiting holds no connection or lock;
    /// the grant is checked again on every look.
    pub async fn claim_work_wait(
        &self,
        caller: &Caller,
        queue: &str,
        target: Option<&str>,
        max: u32,
        wait: std::time::Duration,
    ) -> Result<Vec<ClaimedWork>> {
        let until = tokio::time::Instant::now() + wait.min(std::time::Duration::from_secs(30));
        let mut pause = std::time::Duration::from_millis(100);
        loop {
            let claimed = self.claim_work(caller, queue, target, max).await?;
            let left = until.saturating_duration_since(tokio::time::Instant::now());
            if !claimed.is_empty() || left.is_zero() {
                return Ok(claimed);
            }
            tokio::time::sleep(pause.min(left)).await;
            pause = (pause * 2).min(std::time::Duration::from_secs(1));
        }
    }

    /// Renew an unexpired claim and atomically record ordered progress.
    pub async fn heartbeat(
        &self,
        caller: &Caller,
        uid: &str,
        item: &str,
        epoch: u64,
        progress: Option<Progress>,
    ) -> Result<HeartbeatReply> {
        if let Some(progress) = &progress {
            bounded(&progress.value)?;
            integer(progress.seq)?;
        }
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        let row = work(&tx, uid, item).await?;
        let at = now(&tx).await?;
        work_scope(caller, &a, &row, at)?;
        work_fence(caller, &row, epoch, at)?;
        if row.get::<_, String>("state") == "claimed" && row.get::<_, bool>("cancel") {
            let until = row.get::<_, Option<i64>>("heartbeat_until").unwrap_or(0);
            if until <= at {
                return Err(refused("stale_claim"));
            }
            let reply = HeartbeatReply {
                heartbeat_until: until,
                cancel: true,
                progress_seq: row.get::<_, i64>("progress_seq") as u64,
            };
            tx.commit().await?;
            return Ok(reply);
        }
        active_work(&row, at)?;
        let previous: i64 = row.get("progress_seq");
        let mut sequence = previous;
        let mut value: Option<Value> = row.get("progress");
        if let Some(progress) = progress {
            let next = integer(progress.seq)?;
            if next == previous && value.as_ref() == Some(&progress.value) {
                // A lost acknowledgement can repeat exactly the same progress.
            } else if next == previous.saturating_add(1) {
                sequence = next;
                value = Some(progress.value);
            } else {
                return Err(refused("progress_conflict"));
            }
        }
        let until = at
            .saturating_add(row.get::<_, i64>("lease_ms"))
            .min(grant(caller, at)?.expires_at);
        tx.execute("UPDATE actor.work SET heartbeat_until=$3,progress_seq=$4,progress=$5,updated_at=$6 WHERE uid=$1 AND item_id=$2", &[&uid, &item, &until, &sequence, &value, &at]).await?;
        tx.commit().await?;
        Ok(HeartbeatReply {
            heartbeat_until: until,
            cancel: false,
            progress_seq: sequence as u64,
        })
    }

    /// Complete one claim and enqueue its internal completion in the same transaction.
    pub async fn finish_work(
        &self,
        caller: &Caller,
        uid: &str,
        item: &str,
        epoch: u64,
        outcome: Value,
    ) -> Result<()> {
        bounded(&outcome)?;
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        let row = work(&tx, uid, item).await?;
        let at = now(&tx).await?;
        work_scope(caller, &a, &row, at)?;
        work_fence(caller, &row, epoch, at)?;
        if row.get::<_, String>("state") == "completed" {
            if row.get::<_, Option<Value>>("result").as_ref() != Some(&outcome) {
                return Err(refused("result_conflict"));
            }
            tx.commit().await?;
            return Ok(());
        }
        active_work(&row, at)?;
        tx.execute("UPDATE actor.work SET state='completed',result=$3,updated_at=$4 WHERE uid=$1 AND item_id=$2", &[&uid, &item, &outcome, &at]).await?;
        internal(
            &tx,
            uid,
            "WorkDone",
            json!({"item_id":item,"epoch":epoch,"outcome":outcome}),
            &format!("work.done:{item}:{epoch}"),
            at,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Relinquish an idempotent job; non-repeatable work requires reconciliation.
    pub async fn release_work(
        &self,
        caller: &Caller,
        uid: &str,
        item: &str,
        epoch: u64,
        reason: &str,
    ) -> Result<()> {
        if reason.len() > 1024 {
            return Err(refused("too_large"));
        }
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        let row = work(&tx, uid, item).await?;
        let at = now(&tx).await?;
        work_scope(caller, &a, &row, at)?;
        work_fence(caller, &row, epoch, at)?;
        if row.get::<_, String>("state") == "claimed"
            && row.get::<_, bool>("cancel")
            && row.get::<_, Option<i64>>("heartbeat_until").unwrap_or(0) > at
        {
            // The executor confirms it stopped cancelled work: the claim
            // ends without uncertainty and stops counting against capacity.
            tx.execute("UPDATE actor.work SET state='cancelled',epoch=epoch+1,heartbeat_until=NULL,updated_at=$3 WHERE uid=$1 AND item_id=$2", &[&uid, &item, &at]).await?;
            internal(
                &tx,
                uid,
                "WorkExpired",
                json!({"item_id":item,"epoch":epoch,"uncertain":false,"cancelled":true,"state":"cancelled","reason":reason}),
                &format!("work.released:{item}:{epoch}"),
                at,
            )
            .await?;
            tx.commit().await?;
            return Ok(());
        }
        active_work(&row, at)?;
        let uncertain = row.get::<_, String>("retry_policy") != "idempotent";
        let state = if uncertain { "uncertain" } else { "pending" };
        tx.execute("UPDATE actor.work SET state=$3,epoch=epoch+1,heartbeat_until=NULL,attempts=GREATEST(0,attempts-$4),updated_at=$5 WHERE uid=$1 AND item_id=$2", &[&uid, &item, &state, &(if uncertain { 0_i32 } else { 1_i32 }), &at]).await?;
        internal(
            &tx,
            uid,
            "WorkExpired",
            json!({"item_id":item,"epoch":epoch,"uncertain":uncertain,"reason":reason}),
            &format!("work.released:{item}:{epoch}"),
            at,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Fence expired claims before requeueing only explicitly repeatable work.
    pub async fn expire_work(&self, limit: u32) -> Result<usize> {
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let at = now(&tx).await?;
        let candidates = tx.query("SELECT w.uid,w.item_id FROM actor.work w JOIN actor.instances a USING(uid) WHERE w.state='claimed' AND w.heartbeat_until<=$1 AND (a.status='destroyed' OR (SELECT count(*) FROM actor.inbox i WHERE i.uid=a.uid AND i.state='pending') < 1024) ORDER BY w.heartbeat_until,w.uid,w.item_id LIMIT $2 FOR UPDATE OF a SKIP LOCKED", &[&at, &(limit.min(MAX_BATCH) as i64)]).await?;
        let mut count = 0;
        for candidate in candidates {
            let uid: String = candidate.get("uid");
            let item: String = candidate.get("item_id");
            let Some(a) = tx
                .query_opt(
                    "SELECT * FROM actor.instances WHERE uid=$1 FOR UPDATE SKIP LOCKED",
                    &[&uid],
                )
                .await?
            else {
                continue;
            };
            let row = work(&tx, &uid, &item).await?;
            let at = now(&tx).await?;
            if row.get::<_, String>("state") != "claimed"
                || row
                    .get::<_, Option<i64>>("heartbeat_until")
                    .unwrap_or(i64::MAX)
                    > at
            {
                continue;
            }
            let destroyed = a.get::<_, String>("status") == "destroyed";
            if !destroyed && !inbox_room(&tx, &uid).await? {
                continue;
            }
            let cancelled = destroyed || row.get::<_, bool>("cancel");
            let uncertain = cancelled || row.get::<_, String>("retry_policy") != "idempotent";
            let attempts: i32 = row.get("attempts");
            let epoch: i64 = row.get("epoch");
            let state = if uncertain {
                "uncertain"
            } else if attempts >= row.get::<_, i32>("max_attempts") {
                "failed"
            } else {
                "pending"
            };
            tx.execute("UPDATE actor.work SET state=$3,epoch=epoch+1,heartbeat_until=NULL,updated_at=$4 WHERE uid=$1 AND item_id=$2", &[&uid, &item, &state, &at]).await?;
            if !destroyed {
                internal(&tx, &uid, "WorkExpired", json!({"item_id":item,"epoch":epoch,"attempts":attempts,"uncertain":uncertain,"cancelled":cancelled,"state":state}), &format!("work.expired:{item}:{epoch}"), at).await?;
            }
            count += 1;
        }
        tx.commit().await?;
        Ok(count)
    }

    /// An authorized operator records an observed outcome or explicitly authorizes a retry.
    pub async fn resolve_work(
        &self,
        caller: &Caller,
        uid: &str,
        item: &str,
        expected_epoch: u64,
        retry: bool,
        outcome: Option<Value>,
    ) -> Result<()> {
        if !matches!(caller.role, Role::Service | Role::Admin) || retry == outcome.is_some() {
            return Err(refused("forbidden"));
        }
        if let Some(outcome) = &outcome {
            bounded(outcome)?;
        }
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        actor_access(caller, &a)?;
        let row = work(&tx, uid, item).await?;
        if row.get::<_, i64>("epoch") != integer(expected_epoch)? {
            return Err(refused("stale_claim"));
        }
        if row.get::<_, String>("state") != "uncertain"
            || retry && (row.get::<_, bool>("cancel") || a.get::<_, String>("status") != "live")
        {
            return Err(refused("conflict"));
        }
        let at = now(&tx).await?;
        if retry && row.get::<_, i32>("attempts") >= row.get::<_, i32>("max_attempts") {
            return Err(refused("attempts_exhausted"));
        }
        let state = if retry { "pending" } else { "completed" };
        tx.execute(
            "UPDATE actor.work SET state=$3,result=$4,updated_at=$5 WHERE uid=$1 AND item_id=$2",
            &[&uid, &item, &state, &outcome, &at],
        )
        .await?;
        resolution_audit(
            &tx,
            &a,
            caller,
            "work.resolve",
            &format!("{item}@{expected_epoch}"),
            retry,
            &outcome,
            at,
        )
        .await?;
        if let Some(outcome) = outcome.filter(|_| a.get::<_, String>("status") != "destroyed") {
            let epoch: i64 = row.get("epoch");
            internal(
                &tx,
                uid,
                "WorkDone",
                json!({"item_id":item,"epoch":epoch,"outcome":outcome,"reconciled":true}),
                &format!("work.resolved:{item}:{epoch}"),
                at,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn expire_effects(&self, limit: u32) -> Result<usize> {
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let at = now(&tx).await?;
        let candidates = tx.query("SELECT e.uid,e.effect_id FROM actor.effects e JOIN actor.instances a USING(uid) WHERE e.state='claimed' AND e.claimed_until<=$1 AND (a.status='destroyed' OR (SELECT count(*) FROM actor.inbox i WHERE i.uid=a.uid AND i.state='pending') < 1024) ORDER BY e.claimed_until,e.uid,e.effect_id LIMIT $2 FOR UPDATE OF a SKIP LOCKED", &[&at, &(limit.min(MAX_BATCH) as i64)]).await?;
        let mut count = 0;
        for candidate in candidates {
            let uid: String = candidate.get("uid");
            let id: String = candidate.get("effect_id");
            let Some(a) = tx
                .query_opt(
                    "SELECT * FROM actor.instances WHERE uid=$1 FOR UPDATE SKIP LOCKED",
                    &[&uid],
                )
                .await?
            else {
                continue;
            };
            let row = effect(&tx, &uid, &id).await?;
            let at = now(&tx).await?;
            if row.get::<_, String>("state") != "claimed"
                || row
                    .get::<_, Option<i64>>("claimed_until")
                    .unwrap_or(i64::MAX)
                    > at
            {
                continue;
            }
            let destroyed = a.get::<_, String>("status") == "destroyed";
            if !destroyed && !inbox_room(&tx, &uid).await? {
                continue;
            }
            let cancelled = destroyed || row.get::<_, bool>("cancel");
            let uncertain = cancelled || row.get::<_, String>("retry_policy") != "idempotent";
            let terminal = row.get::<_, i32>("attempts") >= row.get::<_, i32>("max_attempts");
            let state = if uncertain {
                "uncertain"
            } else if terminal {
                "failed"
            } else {
                "pending"
            };
            let old_token: Option<String> = row.get("token");
            tx.execute("UPDATE actor.effects SET state=$3,token=NULL,claimed_until=NULL,next_attempt_at=$4,updated_at=$4 WHERE uid=$1 AND effect_id=$2", &[&uid, &id, &state, &at]).await?;
            count += 1;
            if !destroyed && state != "pending" {
                internal(&tx, &uid, "EffectFailed", json!({"effect_id":id,"error":"lease_expired","uncertain":uncertain,"cancelled":cancelled}), &format!("effect.expired:{id}:{}", old_token.unwrap_or_default()), at).await?;
            }
        }
        tx.commit().await?;
        Ok(count)
    }

    /// Internal runner API. External clients must not have direct access to this method.
    pub async fn claim_effects(&self, limit: u32) -> Result<Vec<ClaimedEffect>> {
        self.claim_effects_matching(limit, None).await
    }

    /// A rolling runtime claims only the effect kinds it can execute.
    pub async fn claim_effects_for_kinds(
        &self,
        limit: u32,
        kinds: &[String],
    ) -> Result<Vec<ClaimedEffect>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        for kind in kinds {
            identifier(kind)?;
        }
        self.claim_effects_matching(limit, Some(kinds)).await
    }

    async fn claim_effects_matching(
        &self,
        limit: u32,
        kinds: Option<&[String]>,
    ) -> Result<Vec<ClaimedEffect>> {
        self.expire_effects(limit).await?;
        let filtered = kinds.is_some();
        let kinds: Vec<String> = kinds.unwrap_or_default().to_vec();
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let at = now(&tx).await?;
        let candidates = tx.query("SELECT e.uid,e.effect_id FROM actor.effects e JOIN actor.instances a USING(uid) WHERE e.state='pending' AND NOT e.cancel AND a.status='live' AND e.next_attempt_at<=$1 AND (NOT $2 OR e.kind=ANY($3)) ORDER BY e.next_attempt_at,e.uid,e.effect_id LIMIT 512 FOR UPDATE OF a SKIP LOCKED", &[&at, &filtered, &kinds]).await?;
        let mut claimed = Vec::new();
        for candidate in candidates {
            if claimed.len() >= limit.min(MAX_BATCH) as usize {
                break;
            }
            let uid: String = candidate.get("uid");
            let id: String = candidate.get("effect_id");
            let Some(a) = tx
                .query_opt(
                    "SELECT * FROM actor.instances WHERE uid=$1 FOR UPDATE SKIP LOCKED",
                    &[&uid],
                )
                .await?
            else {
                continue;
            };
            let row = effect(&tx, &uid, &id).await?;
            let at = now(&tx).await?;
            if a.get::<_, String>("status") != "live"
                || row.get::<_, String>("state") != "pending"
                || row.get::<_, bool>("cancel")
                || row.get::<_, i64>("next_attempt_at") > at
            {
                continue;
            }
            let token = token()?;
            let until = at.saturating_add(EFFECT_LEASE_MS);
            let row = tx.query_one("UPDATE actor.effects SET state='claimed',token=$3,claimed_until=$4,attempts=attempts+1,updated_at=$5 WHERE uid=$1 AND effect_id=$2 RETURNING *", &[&uid, &id, &token, &until, &at]).await?;
            claimed.push(ClaimedEffect {
                actor: identity(&a),
                uid,
                id,
                kind: row.get("kind"),
                payload: row.get("payload"),
                token,
                claimed_until: until,
                timeout_ms: row.get("timeout_ms"),
                attempts: row.get::<_, i32>("attempts") as u32,
            });
        }
        tx.commit().await?;
        Ok(claimed)
    }

    /// Renew only the original unexpired effect claim.
    pub async fn renew_effect(&self, uid: &str, id: &str, token: &str) -> Result<Timestamp> {
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        if a.get::<_, String>("status") != "live" {
            return Err(refused("not_live"));
        }
        let row = effect(&tx, uid, id).await?;
        let at = now(&tx).await?;
        active_effect(&row, token, at)?;
        let until = at.saturating_add(EFFECT_LEASE_MS);
        tx.execute(
            "UPDATE actor.effects SET claimed_until=$3,updated_at=$4 WHERE uid=$1 AND effect_id=$2",
            &[&uid, &id, &until, &at],
        )
        .await?;
        tx.commit().await?;
        Ok(until)
    }

    /// Record a completion and its internal message atomically; exact retries are harmless.
    pub async fn finish_effect(
        &self,
        uid: &str,
        id: &str,
        token: &str,
        outcome: Value,
    ) -> Result<()> {
        bounded(&outcome)?;
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        if a.get::<_, String>("status") != "live" {
            return Err(refused("not_live"));
        }
        let row = effect(&tx, uid, id).await?;
        let at = now(&tx).await?;
        if row.get::<_, String>("state") == "completed"
            && row.get::<_, Option<String>>("token").as_deref() == Some(token)
        {
            if row.get::<_, Option<Value>>("result").as_ref() != Some(&outcome) {
                return Err(refused("result_conflict"));
            }
            tx.commit().await?;
            return Ok(());
        }
        active_effect(&row, token, at)?;
        tx.execute("UPDATE actor.effects SET state='completed',result=$3,updated_at=$4 WHERE uid=$1 AND effect_id=$2", &[&uid, &id, &outcome, &at]).await?;
        internal(
            &tx,
            uid,
            "EffectDone",
            json!({"effect_id":id,"outcome":outcome}),
            &format!("effect.done:{id}"),
            at,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Record failure without guessing that an external operation did not occur.
    pub async fn fail_effect(
        &self,
        uid: &str,
        id: &str,
        token: &str,
        error: &str,
        retryable: bool,
    ) -> Result<()> {
        if error.len() > 1024 {
            return Err(refused("too_large"));
        }
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        if a.get::<_, String>("status") != "live" {
            return Err(refused("not_live"));
        }
        let row = effect(&tx, uid, id).await?;
        let at = now(&tx).await?;
        active_effect(&row, token, at)?;
        let attempts: i32 = row.get("attempts");
        let uncertain = row.get::<_, String>("retry_policy") != "idempotent";
        let retry = retryable && !uncertain && attempts < row.get::<_, i32>("max_attempts");
        let state = if uncertain {
            "uncertain"
        } else if retry {
            "pending"
        } else {
            "failed"
        };
        let next = at.saturating_add(1000_i64.saturating_mul(1_i64 << attempts.clamp(0, 8)));
        let result = json!({"error":error,"uncertain":uncertain});
        tx.execute("UPDATE actor.effects SET state=$3,token=NULL,claimed_until=NULL,next_attempt_at=$4,result=$5,updated_at=$6 WHERE uid=$1 AND effect_id=$2", &[&uid, &id, &state, &next, &result, &at]).await?;
        if !retry {
            internal(
                &tx,
                uid,
                "EffectFailed",
                json!({"effect_id":id,"error":error,"uncertain":uncertain}),
                &format!("effect.failed:{id}:{token}"),
                at,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Resolve an uncertain effect using an explicit authorized operator decision.
    pub async fn resolve_effect(
        &self,
        caller: &Caller,
        uid: &str,
        id: &str,
        expected_attempt: u32,
        retry: bool,
        outcome: Option<Value>,
    ) -> Result<()> {
        if !matches!(caller.role, Role::Service | Role::Admin) || retry == outcome.is_some() {
            return Err(refused("forbidden"));
        }
        if let Some(outcome) = &outcome {
            bounded(outcome)?;
        }
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let a = actor(&tx, uid).await?;
        actor_access(caller, &a)?;
        let row = effect(&tx, uid, id).await?;
        if i64::from(row.get::<_, i32>("attempts")) != i64::from(expected_attempt) {
            return Err(refused("stale_claim"));
        }
        if row.get::<_, String>("state") != "uncertain"
            || retry && (row.get::<_, bool>("cancel") || a.get::<_, String>("status") != "live")
        {
            return Err(refused("conflict"));
        }
        if retry && row.get::<_, i32>("attempts") >= row.get::<_, i32>("max_attempts") {
            return Err(refused("attempts_exhausted"));
        }
        let at = now(&tx).await?;
        let state = if retry { "pending" } else { "completed" };
        tx.execute("UPDATE actor.effects SET state=$3,result=$4,next_attempt_at=$5,updated_at=$5 WHERE uid=$1 AND effect_id=$2", &[&uid, &id, &state, &outcome, &at]).await?;
        resolution_audit(
            &tx,
            &a,
            caller,
            "effect.resolve",
            &format!("{id}@{expected_attempt}"),
            retry,
            &outcome,
            at,
        )
        .await?;
        if let Some(outcome) = outcome.filter(|_| a.get::<_, String>("status") != "destroyed") {
            internal(
                &tx,
                uid,
                "EffectDone",
                json!({"effect_id":id,"outcome":outcome,"reconciled":true}),
                &format!("effect.done:{id}"),
                at,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Deliver each due occurrence once; coalesce missed recurring occurrences.
    pub async fn fire_alarms(&self, limit: u32) -> Result<usize> {
        let mut connection = self.pool.acquire().await?;
        let tx = connection.transaction().await?;
        let at = now(&tx).await?;
        let candidates = tx.query("SELECT l.uid,l.name FROM actor.alarms l JOIN actor.instances a USING(uid) WHERE l.due_at<=$1 AND a.status='live' AND (a.status='destroyed' OR (SELECT count(*) FROM actor.inbox i WHERE i.uid=a.uid AND i.state='pending') < 1024) ORDER BY l.due_at,l.uid,l.name LIMIT $2 FOR UPDATE OF a SKIP LOCKED", &[&at, &(limit.min(MAX_BATCH) as i64)]).await?;
        let mut count = 0;
        for candidate in candidates {
            let uid: String = candidate.get("uid");
            let name: String = candidate.get("name");
            let Some(a) = tx
                .query_opt(
                    "SELECT * FROM actor.instances WHERE uid=$1 FOR UPDATE SKIP LOCKED",
                    &[&uid],
                )
                .await?
            else {
                continue;
            };
            if a.get::<_, String>("status") != "live" {
                continue;
            }
            let Some(row) = tx
                .query_opt(
                    "SELECT * FROM actor.alarms WHERE uid=$1 AND name=$2 FOR UPDATE",
                    &[&uid, &name],
                )
                .await?
            else {
                continue;
            };
            let at = now(&tx).await?;
            let due: i64 = row.get("due_at");
            if due > at {
                continue;
            }
            if !inbox_room(&tx, &uid).await? {
                continue;
            }
            let generation: i64 = row.get("generation");
            let message: Envelope = serde_json::from_value(row.get("message"))?;
            let caller: Caller = serde_json::from_value(row.get("caller"))?;
            let key = format!(
                "alarm:{:x}",
                Sha256::digest(serde_json::to_vec(&(&name, generation, due))?)
            );
            enqueue_alarm_tx(&tx, &uid, &message, &caller, &key, at).await?;
            if let Some(interval) = row.get::<_, Option<i64>>("interval_ms") {
                if interval <= 0 {
                    return Err(refused("bad_interval"));
                }
                let next = due.saturating_add(
                    (at.saturating_sub(due) / interval)
                        .saturating_add(1)
                        .saturating_mul(interval),
                );
                if next <= at {
                    return Err(refused("bad_interval"));
                }
                tx.execute(
                    "UPDATE actor.alarms SET due_at=$3 WHERE uid=$1 AND name=$2",
                    &[&uid, &name, &next],
                )
                .await?;
            } else {
                tx.execute(
                    "DELETE FROM actor.alarms WHERE uid=$1 AND name=$2",
                    &[&uid, &name],
                )
                .await?;
            }
            count += 1;
        }
        tx.commit().await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pool, Registry};
    use std::sync::Arc;

    struct Fixture {
        store: PgStore,
        pool: Pool,
        uid: String,
        caller: Caller,
    }
    impl Fixture {
        async fn new() -> Option<Self> {
            let dsn = match std::env::var("ACTORS_TEST_DATABASE_URL") {
                Ok(dsn) => dsn,
                Err(_) => {
                    eprintln!("queue database test skipped: ACTORS_TEST_DATABASE_URL is unset");
                    return None;
                }
            };
            let pool = Pool::new(&dsn, 8).unwrap();
            let store = PgStore::new(pool.clone(), Arc::new(Registry::new()));
            store.migrate().await.unwrap();
            let uid = format!("queue-test-{}", token().unwrap());
            let workspace = format!("workspace-{uid}");
            let connection = pool.acquire().await.unwrap();
            let at: i64 = connection
                .query_one(
                    "SELECT (extract(epoch FROM clock_timestamp())*1000)::bigint",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            connection.execute("INSERT INTO actor.instances(uid,workspace_id,actor_type,actor_key,owner,state_version,state,created_at,updated_at) VALUES($1,$2,'queue-test','one','account',1,'{}',$3,$3)", &[&uid,&workspace,&at]).await.unwrap();
            let caller = Caller {
                principal: "person".into(),
                workspace_id: workspace,
                account_id: Some("account".into()),
                role: Role::Service,
                executor: Some(ExecutorGrant {
                    id: format!("executor-{uid}"),
                    queues: vec!["jobs".into()],
                    targets: vec!["computer".into()],
                    generation: 7,
                    expires_at: at + 600_000,
                    max_claims: 2,
                }),
            };
            Some(Self {
                store,
                pool,
                uid,
                caller,
            })
        }
        async fn work(&self, item: &str, policy: &str) {
            self.pool.acquire().await.unwrap().execute("INSERT INTO actor.work(uid,item_id,workspace_id,queue,target,payload,lease_ms,max_attempts,retry_policy,created_at,updated_at) VALUES($1,$2,$3,'jobs','computer','{}',60000,3,$4,0,0)", &[&self.uid,&item,&self.caller.workspace_id,&policy]).await.unwrap();
        }
        async fn effect(&self, id: &str, policy: &str) {
            self.pool.acquire().await.unwrap().execute("INSERT INTO actor.effects(uid,effect_id,workspace_id,kind,payload,timeout_ms,max_attempts,retry_policy,created_at,updated_at) VALUES($1,$2,$3,'test','{}',600000,3,$4,0,0)", &[&self.uid,&id,&self.caller.workspace_id,&policy]).await.unwrap();
        }
        async fn inbox(&self, name: &str) -> i64 {
            self.pool
                .acquire()
                .await
                .unwrap()
                .query_one(
                    "SELECT count(*) FROM actor.inbox WHERE uid=$1 AND message->>'name'=$2",
                    &[&self.uid, &name],
                )
                .await
                .unwrap()
                .get(0)
        }
        async fn clean(self) {
            self.pool
                .acquire()
                .await
                .unwrap()
                .execute("DELETE FROM actor.instances WHERE uid=$1", &[&self.uid])
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn concurrent_claims_share_capacity_and_enforce_private_scope() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        for item in ["a", "b", "c", "d"] {
            f.work(item, "idempotent").await;
        }
        let mut wildcard = f.caller.clone();
        wildcard.executor.as_mut().unwrap().queues = vec!["*".into()];
        assert!(
            f.store
                .claim_work(&wildcard, "jobs", Some("computer"), 1)
                .await
                .is_err()
        );
        wildcard = f.caller.clone();
        wildcard.executor.as_mut().unwrap().targets = vec!["*".into()];
        assert!(
            f.store
                .claim_work(&wildcard, "jobs", Some("computer"), 1)
                .await
                .is_err()
        );
        assert!(
            f.store
                .claim_work(&wildcard, "jobs", None, 1)
                .await
                .unwrap()
                .is_empty()
        );
        let mut caller = f.caller.clone();
        caller.executor.as_mut().unwrap().max_claims = 1;
        let (a, b) = tokio::join!(
            f.store.claim_work(&caller, "jobs", Some("computer"), 8),
            f.store.claim_work(&caller, "jobs", Some("computer"), 8)
        );
        let mut claims = a.unwrap();
        claims.extend(b.unwrap());
        assert_eq!(claims.len(), 1);
        let held = &claims[0];
        let mut foreign = caller.clone();
        foreign.account_id = Some("another-account".into());
        assert_eq!(
            f.store
                .heartbeat(&foreign, &f.uid, &held.item_id, held.epoch, None)
                .await
                .unwrap_err()
                .code,
            "not_found"
        );
        foreign = caller.clone();
        foreign.workspace_id = "another-workspace".into();
        assert_eq!(
            f.store
                .finish_work(&foreign, &f.uid, &held.item_id, held.epoch, json!("done"))
                .await
                .unwrap_err()
                .code,
            "not_found"
        );
        let mut revoked = caller.clone();
        revoked.executor.as_mut().unwrap().generation += 1;
        assert_eq!(
            f.store
                .heartbeat(&revoked, &f.uid, &held.item_id, held.epoch, None)
                .await
                .unwrap_err()
                .code,
            "stale_claim"
        );
        assert!(
            f.store
                .claim_work(&caller, "forbidden", None, 1)
                .await
                .is_err()
        );
        assert!(
            f.store
                .claim_work(&caller, "jobs", Some("other-computer"), 1)
                .await
                .is_err()
        );
        f.clean().await;
    }

    #[tokio::test]
    async fn progress_and_completion_retries_must_have_identical_content() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.work("one", "idempotent").await;
        let claim = f
            .store
            .claim_work(&f.caller, "jobs", Some("computer"), 1)
            .await
            .unwrap()
            .remove(0);
        let progress = Progress {
            seq: 1,
            value: json!({"step":"checking"}),
        };
        f.store
            .heartbeat(
                &f.caller,
                &f.uid,
                "one",
                claim.epoch,
                Some(progress.clone()),
            )
            .await
            .unwrap();
        f.store
            .heartbeat(&f.caller, &f.uid, "one", claim.epoch, Some(progress))
            .await
            .unwrap();
        assert_eq!(
            f.store
                .heartbeat(
                    &f.caller,
                    &f.uid,
                    "one",
                    claim.epoch,
                    Some(Progress {
                        seq: 1,
                        value: json!("different")
                    })
                )
                .await
                .unwrap_err()
                .code,
            "progress_conflict"
        );
        assert!(
            f.store
                .heartbeat(
                    &f.caller,
                    &f.uid,
                    "one",
                    claim.epoch,
                    Some(Progress {
                        seq: 3,
                        value: json!("gap")
                    })
                )
                .await
                .is_err()
        );
        let outcome = json!({"artifact":"sha256:result"});
        f.store
            .finish_work(&f.caller, &f.uid, "one", claim.epoch, outcome.clone())
            .await
            .unwrap();
        f.store
            .finish_work(&f.caller, &f.uid, "one", claim.epoch, outcome)
            .await
            .unwrap();
        assert_eq!(f.inbox("WorkDone").await, 1);
        assert_eq!(
            f.store
                .finish_work(&f.caller, &f.uid, "one", claim.epoch, json!("changed"))
                .await
                .unwrap_err()
                .code,
            "result_conflict"
        );
        assert!(
            f.store
                .finish_work(
                    &f.caller,
                    &f.uid,
                    "one",
                    claim.epoch,
                    json!("x".repeat(MAX_PAYLOAD))
                )
                .await
                .is_err()
        );
        f.clean().await;
    }

    #[tokio::test]
    async fn expiry_fences_before_reclaim_and_requires_explicit_reconciliation() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.work("repeatable", "idempotent").await;
        f.work("payment", "reconcile").await;
        let claims = f
            .store
            .claim_work(&f.caller, "jobs", Some("computer"), 2)
            .await
            .unwrap();
        f.pool
            .acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.work SET heartbeat_until=0 WHERE uid=$1",
                &[&f.uid],
            )
            .await
            .unwrap();
        for claim in &claims {
            assert_eq!(
                f.store
                    .finish_work(
                        &f.caller,
                        &f.uid,
                        &claim.item_id,
                        claim.epoch,
                        json!("late")
                    )
                    .await
                    .unwrap_err()
                    .code,
                "stale_claim"
            );
            assert!(
                f.store
                    .heartbeat(&f.caller, &f.uid, &claim.item_id, claim.epoch, None)
                    .await
                    .is_err()
            );
        }
        f.store.expire_work(64).await.unwrap();
        let rows = f
            .pool
            .acquire()
            .await
            .unwrap()
            .query(
                "SELECT item_id,state FROM actor.work WHERE uid=$1 ORDER BY item_id",
                &[&f.uid],
            )
            .await
            .unwrap();
        assert_eq!(rows[0].get::<_, String>("state"), "uncertain");
        assert_eq!(rows[1].get::<_, String>("state"), "pending");
        let second = f
            .store
            .claim_work(&f.caller, "jobs", Some("computer"), 2)
            .await
            .unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].item_id, "repeatable");
        let original = claims
            .iter()
            .find(|claim| claim.item_id == "repeatable")
            .unwrap();
        assert!(second[0].epoch > original.epoch);
        assert!(
            f.store
                .finish_work(
                    &f.caller,
                    &f.uid,
                    "repeatable",
                    original.epoch,
                    json!("stale")
                )
                .await
                .is_err()
        );
        let mut member = f.caller.clone();
        member.role = Role::Member;
        assert!(
            f.store
                .resolve_work(&member, &f.uid, "payment", 2, true, None)
                .await
                .is_err()
        );
        assert_eq!(
            f.store
                .resolve_work(&f.caller, &f.uid, "payment", 1, true, None)
                .await
                .unwrap_err()
                .code,
            "stale_claim"
        );
        f.store
            .resolve_work(
                &f.caller,
                &f.uid,
                "payment",
                2,
                false,
                Some(json!({"observed":"paid"})),
            )
            .await
            .unwrap();
        assert_eq!(f.inbox("WorkDone").await, 1);
        assert_eq!(f.inbox("WorkExpired").await, 2);
        f.clean().await;
    }

    #[tokio::test]
    async fn destroyed_and_cancelled_work_cannot_complete() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.work("one", "idempotent").await;
        let claim = f
            .store
            .claim_work(&f.caller, "jobs", Some("computer"), 1)
            .await
            .unwrap()
            .remove(0);
        f.pool
            .acquire()
            .await
            .unwrap()
            .execute("UPDATE actor.work SET cancel=true WHERE uid=$1", &[&f.uid])
            .await
            .unwrap();
        assert!(
            f.store
                .finish_work(&f.caller, &f.uid, "one", claim.epoch, json!("done"))
                .await
                .is_err()
        );
        let cancelled = f
            .store
            .heartbeat(&f.caller, &f.uid, "one", claim.epoch, None)
            .await
            .unwrap();
        assert!(cancelled.cancel);
        assert_eq!(cancelled.heartbeat_until, claim.heartbeat_until);
        f.pool
            .acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.work SET heartbeat_until=0 WHERE uid=$1",
                &[&f.uid],
            )
            .await
            .unwrap();
        f.store.expire_work(64).await.unwrap();
        let row = f
            .pool
            .acquire()
            .await
            .unwrap()
            .query_one("SELECT state,owner FROM actor.work WHERE uid=$1", &[&f.uid])
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>("state"), "uncertain");
        assert_eq!(
            row.get::<_, Option<String>>("owner"),
            Some(f.caller.executor.as_ref().unwrap().id.clone())
        );
        f.pool
            .acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.instances SET status='destroyed' WHERE uid=$1",
                &[&f.uid],
            )
            .await
            .unwrap();
        assert!(
            f.store
                .heartbeat(&f.caller, &f.uid, "one", claim.epoch, None)
                .await
                .is_err()
        );
        assert_eq!(f.inbox("WorkDone").await, 0);
        assert!(
            f.store
                .resolve_work(&f.caller, &f.uid, "one", claim.epoch + 1, true, None)
                .await
                .is_err()
        );
        f.store
            .resolve_work(
                &f.caller,
                &f.uid,
                "one",
                claim.epoch + 1,
                false,
                Some(json!({"observed":"stopped"})),
            )
            .await
            .unwrap();
        assert_eq!(f.inbox("WorkDone").await, 0);
        f.clean().await;
    }

    #[tokio::test]
    async fn effect_claims_renew_fence_reconcile_and_commit_one_completion() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.effect("one", "reconcile").await;
        let claim = f
            .store
            .claim_effects(64)
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.uid == f.uid)
            .unwrap();
        assert!(
            f.store
                .renew_effect(&f.uid, "one", "wrong-token")
                .await
                .is_err()
        );
        assert!(
            f.store
                .renew_effect(&f.uid, "one", &claim.token)
                .await
                .unwrap()
                >= claim.claimed_until
        );
        f.pool
            .acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.effects SET claimed_until=0 WHERE uid=$1",
                &[&f.uid],
            )
            .await
            .unwrap();
        assert!(
            f.store
                .finish_effect(&f.uid, "one", &claim.token, json!("late"))
                .await
                .is_err()
        );
        f.store.expire_effects(64).await.unwrap();
        assert!(
            !f.store
                .claim_effects(64)
                .await
                .unwrap()
                .iter()
                .any(|c| c.uid == f.uid)
        );
        assert_eq!(
            f.store
                .resolve_effect(&f.caller, &f.uid, "one", claim.attempts + 1, true, None)
                .await
                .unwrap_err()
                .code,
            "stale_claim"
        );
        f.store
            .resolve_effect(&f.caller, &f.uid, "one", claim.attempts, true, None)
            .await
            .unwrap();
        let next = f
            .store
            .claim_effects(64)
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.uid == f.uid)
            .unwrap();
        assert_ne!(next.token, claim.token);
        assert!(
            f.store
                .finish_effect(&f.uid, "one", &claim.token, json!("old"))
                .await
                .is_err()
        );
        f.store
            .finish_effect(&f.uid, "one", &next.token, json!("done"))
            .await
            .unwrap();
        f.store
            .finish_effect(&f.uid, "one", &next.token, json!("done"))
            .await
            .unwrap();
        assert!(
            f.store
                .finish_effect(&f.uid, "one", &next.token, json!("changed"))
                .await
                .is_err()
        );
        assert_eq!(f.inbox("EffectDone").await, 1);
        assert_eq!(f.inbox("EffectFailed").await, 1);
        f.clean().await;
    }

    #[tokio::test]
    async fn alarm_occurrence_is_atomic_and_preserves_the_original_caller() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let message = serde_json::to_value(Envelope {
            name: "tick".into(),
            args: json!({}),
            origin: Origin::Action,
        })
        .unwrap();
        let caller = serde_json::to_value(&f.caller).unwrap();
        f.pool.acquire().await.unwrap().execute("INSERT INTO actor.alarms(uid,name,due_at,interval_ms,message,caller,generation) VALUES($1,'every',0,60000,$2,$3,1)", &[&f.uid,&message,&caller]).await.unwrap();
        let (a, b) = tokio::join!(f.store.fire_alarms(64), f.store.fire_alarms(64));
        a.unwrap();
        b.unwrap();
        assert_eq!(f.inbox("tick").await, 1);
        let connection = f.pool.acquire().await.unwrap();
        let row = connection
            .query_one(
                "SELECT origin,caller FROM actor.inbox WHERE uid=$1",
                &[&f.uid],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>("origin"), "inbox");
        assert_eq!(row.get::<_, Value>("caller"), caller);
        let due: i64 = connection
            .query_one("SELECT due_at FROM actor.alarms WHERE uid=$1", &[&f.uid])
            .await
            .unwrap()
            .get(0);
        let at: i64 = connection
            .query_one(
                "SELECT (extract(epoch FROM clock_timestamp())*1000)::bigint",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        assert!(due > at);
        drop(connection);
        f.clean().await;
    }
    #[tokio::test]
    async fn a_locked_prefix_does_not_hide_another_ready_actor() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        let other = format!("{}-other", f.uid);
        let connection = f.pool.acquire().await.unwrap();
        connection.execute("INSERT INTO actor.instances(uid,workspace_id,actor_type,actor_key,owner,state_version,state,created_at,updated_at) VALUES($1,$2,'queue-test','two','account',1,'{}',0,0)", &[&other,&f.caller.workspace_id]).await.unwrap();
        connection.execute("INSERT INTO actor.work(uid,item_id,workspace_id,queue,target,payload,lease_ms,max_attempts,retry_policy,created_at,updated_at) SELECT $1,'held-'||n::text,$2,'jobs','computer','{}',60000,3,'idempotent',0,0 FROM generate_series(1,513) n", &[&f.uid,&f.caller.workspace_id]).await.unwrap();
        connection.execute("INSERT INTO actor.work(uid,item_id,workspace_id,queue,target,payload,lease_ms,max_attempts,retry_policy,created_at,updated_at) VALUES($1,'ready',$2,'jobs','computer','{}',60000,3,'idempotent',1,1)", &[&other,&f.caller.workspace_id]).await.unwrap();
        drop(connection);
        let mut held = f.pool.acquire().await.unwrap();
        let tx = held.transaction().await.unwrap();
        tx.query_one(
            "SELECT uid FROM actor.instances WHERE uid=$1 FOR UPDATE",
            &[&f.uid],
        )
        .await
        .unwrap();
        let claims = f
            .store
            .claim_work(&f.caller, "jobs", Some("computer"), 1)
            .await
            .unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].uid, other);
        tx.rollback().await.unwrap();
        drop(held);
        f.pool
            .acquire()
            .await
            .unwrap()
            .execute("DELETE FROM actor.instances WHERE uid=$1", &[&other])
            .await
            .unwrap();
        f.clean().await;
    }

    #[tokio::test]
    async fn only_idempotent_effects_retry_and_attempts_are_bounded() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.effect("repeatable", "idempotent").await;
        for attempt in 1..=3 {
            let claim = f
                .store
                .claim_effects(64)
                .await
                .unwrap()
                .into_iter()
                .find(|c| c.uid == f.uid)
                .unwrap();
            assert_eq!(claim.attempts, attempt);
            f.store
                .fail_effect(&f.uid, "repeatable", &claim.token, "unavailable", true)
                .await
                .unwrap();
            f.pool
                .acquire()
                .await
                .unwrap()
                .execute(
                    "UPDATE actor.effects SET next_attempt_at=0 WHERE uid=$1",
                    &[&f.uid],
                )
                .await
                .unwrap();
        }
        assert!(
            !f.store
                .claim_effects(64)
                .await
                .unwrap()
                .iter()
                .any(|c| c.uid == f.uid)
        );
        assert_eq!(f.inbox("EffectFailed").await, 1);
        let row = f
            .pool
            .acquire()
            .await
            .unwrap()
            .query_one("SELECT state FROM actor.effects WHERE uid=$1", &[&f.uid])
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>(0), "failed");
        f.clean().await;
    }
    #[tokio::test]
    async fn rolling_executor_does_not_claim_an_unknown_effect_kind() {
        let Some(f) = Fixture::new().await else {
            return;
        };
        f.effect("old", "idempotent").await;
        f.effect("new", "idempotent").await;
        f.pool
            .acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE actor.effects SET kind='newer' WHERE uid=$1 AND effect_id='new'",
                &[&f.uid],
            )
            .await
            .unwrap();
        assert!(
            f.store
                .claim_effects_for_kinds(64, &[])
                .await
                .unwrap()
                .is_empty()
        );
        let claims = f
            .store
            .claim_effects_for_kinds(64, &["test".into()])
            .await
            .unwrap();
        let ours: Vec<_> = claims.iter().filter(|c| c.uid == f.uid).collect();
        assert_eq!(ours.len(), 1);
        assert_eq!(ours[0].id, "old");
        let state: String = f
            .pool
            .acquire()
            .await
            .unwrap()
            .query_one(
                "SELECT state FROM actor.effects WHERE uid=$1 AND effect_id='new'",
                &[&f.uid],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(state, "pending");
        f.clean().await;
    }

    #[tokio::test]
    async fn saturated_actor_does_not_block_other_expirations_or_alarms() {
        let Some(full) = Fixture::new().await else {
            return;
        };
        let Some(ready) = Fixture::new().await else {
            return;
        };
        for f in [&full, &ready] {
            f.work("work", "reconcile").await;
            f.store
                .claim_work(&f.caller, "jobs", Some("computer"), 1)
                .await
                .unwrap();
            f.effect("effect", "reconcile").await;
        }
        full.store.claim_effects(64).await.unwrap();
        let connection = full.pool.acquire().await.unwrap();
        let message = serde_json::to_value(Envelope {
            name: "pending".into(),
            args: json!({}),
            origin: Origin::Inbox,
        })
        .unwrap();
        let caller = serde_json::to_value(&full.caller).unwrap();
        connection.execute("INSERT INTO actor.inbox(uid,seq,message,caller,origin,payload_hash,principal,created_at,updated_at) SELECT $1,n,$2,$3,'inbox','fixture','fixture',0,0 FROM generate_series(1,1024)n", &[&full.uid,&message,&caller]).await.unwrap();
        connection
            .execute(
                "UPDATE actor.instances SET inbox_seq=1024 WHERE uid=$1",
                &[&full.uid],
            )
            .await
            .unwrap();
        for (f, deadline) in [(&full, 0_i64), (&ready, 1_i64)] {
            connection
                .execute(
                    "UPDATE actor.work SET heartbeat_until=$2 WHERE uid=$1",
                    &[&f.uid, &deadline],
                )
                .await
                .unwrap();
            connection
                .execute(
                    "UPDATE actor.effects SET claimed_until=$2 WHERE uid=$1",
                    &[&f.uid, &deadline],
                )
                .await
                .unwrap();
            connection.execute("INSERT INTO actor.alarms(uid,name,due_at,message,caller) VALUES($1,'wake',$2,$3,$4)", &[&f.uid,&deadline,&message,&serde_json::to_value(&f.caller).unwrap()]).await.unwrap();
        }
        drop(connection);
        assert_eq!(full.store.expire_work(1).await.unwrap(), 1);
        assert_eq!(full.store.expire_effects(1).await.unwrap(), 1);
        assert_eq!(full.store.fire_alarms(1).await.unwrap(), 1);
        assert_eq!(ready.inbox("WorkExpired").await, 1);
        assert_eq!(ready.inbox("EffectFailed").await, 1);
        assert_eq!(ready.inbox("pending").await, 1);
        assert!(
            full.store
                .heartbeat(&full.caller, &full.uid, "work", 1, None)
                .await
                .is_err()
        );
        let state: String = full
            .pool
            .acquire()
            .await
            .unwrap()
            .query_one("SELECT state FROM actor.work WHERE uid=$1", &[&full.uid])
            .await
            .unwrap()
            .get(0);
        assert_eq!(state, "claimed"); // Still fenced by its elapsed lease, never made ready.
        full.clean().await;
        ready.clean().await;
    }
}
