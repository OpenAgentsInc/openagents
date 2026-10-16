//! Read-only storage observations for allocations whose deletion is recorded.

use super::evidence;
use crate::db::DbError;
use crate::registry::Drivers;
use crate::store::{COLUMNS, Entry, Store, entry};
use computer_driver::{Driver, Outcome, StorageObservation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio_postgres::{Row, Transaction};

const JOB_COLUMNS: &str = "id,computer,binding_fingerprint,create_operation,resource,lease_generation,lease_holder,attempts";

/// A claim on one historical allocation, independent of its lifecycle lease.
#[derive(Clone, Debug)]
pub struct DeletedStorageJob {
    pub id: i64,
    pub computer: String,
    pub binding_fingerprint: String,
    pub operation: String,
    pub resource: String,
    pub generation: i64,
    pub holder: String,
    pub attempts: i32,
}

impl DeletedStorageJob {
    fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0),
            computer: row.get(1),
            binding_fingerprint: row.get(2),
            operation: row.get(3),
            resource: row.get(4),
            generation: row.get(5),
            holder: row.get(6),
            attempts: row.get(7),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Context {
    owner: String,
    deployment: String,
    driver: String,
    size: String,
    shape: Option<Value>,
    deletion_ms: u64,
    provider_operation: String,
}

fn unavailable(_: impl std::fmt::Display) -> DbError {
    DbError("The storage observer record is unavailable.".into())
}

fn fenced() -> DbError {
    DbError("The storage observer claim is no longer current.".into())
}

fn digest(value: &impl Serialize) -> Result<String, DbError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(unavailable)?)
    ))
}

fn valid_resource(resource: &str) -> bool {
    !resource.is_empty()
        && resource.len() <= 256
        && resource
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}

fn deleted<'a>(
    e: &'a Entry,
    operation: &str,
    resource: &str,
) -> Option<&'a coder_working_computer::Fact> {
    e.record.creates.iter().find_map(|create| {
        (create.operation == operation && create.resource.as_deref() == Some(resource))
            .then_some(create.deletion.as_ref())
            .flatten()
            .filter(|fact| fact.is_done())
    })
}

impl Store {
    /// Discover a bounded page of deleted allocations. The durable cursor wraps
    /// so an older row that gains a deletion later is eventually visited again.
    pub async fn seed_deleted_storage(&self, limit: i64) -> Result<u64, DbError> {
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await.map_err(unavailable)?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='10s'; SET LOCAL idle_in_transaction_session_timeout='15s'")
            .await
            .map_err(unavailable)?;
        let Some(cursor) = tx.query_opt(
            "SELECT after_computer FROM computers.deleted_storage_scan WHERE singleton FOR UPDATE SKIP LOCKED", &[],
        ).await.map_err(unavailable)? else { return Ok(0) };
        let after: String = cursor.get(0);
        let rows = tx
            .query(
                &format!(
                    "SELECT {COLUMNS} FROM computers.computers WHERE id>$1 ORDER BY id LIMIT $2"
                ),
                &[&after, &limit.clamp(1, 100)],
            )
            .await
            .map_err(unavailable)?;
        let mut candidates = Vec::new();
        for row in &rows {
            // A malformed or unassigned legacy record cannot establish authority.
            let Ok(e) = entry(row) else { continue };
            let Some(binding) = &e.binding else { continue };
            if binding.owner != e.owner || e.record.validate().is_err() {
                continue;
            }
            let binding_snapshot = serde_json::to_value(binding).map_err(unavailable)?;
            for create in &e.record.creates {
                let Some(resource) = create.resource.as_deref().filter(|r| valid_resource(r))
                else {
                    continue;
                };
                let Some(fact @ coder_working_computer::Fact::Done { at_ms, .. }) =
                    create.deletion.as_ref()
                else {
                    continue;
                };
                if create.operation.is_empty() || create.operation.len() > 1024 {
                    continue;
                }
                let context = Context {
                    owner: e.owner.clone(),
                    deployment: binding.deployment.clone(),
                    driver: e.driver.clone(),
                    size: e.size.clone(),
                    shape: binding
                        .shape
                        .as_ref()
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(unavailable)?,
                    deletion_ms: *at_ms,
                    provider_operation: binding.operation(&create.operation),
                };
                candidates.push(json!({
                    "computer":e.id,"binding_fingerprint":binding.fingerprint,
                    "binding_snapshot":binding_snapshot,"create_operation":create.operation,
                    "resource":resource,"deletion_fingerprint":digest(fact)?,"context":context,
                }));
            }
        }
        let inserted = tx.execute(
            "INSERT INTO computers.deleted_storage_jobs(computer,binding_fingerprint,binding_snapshot,create_operation,resource,deletion_fingerprint,context)
             SELECT computer,binding_fingerprint,binding_snapshot,create_operation,resource,deletion_fingerprint,context
             FROM jsonb_to_recordset($1) AS candidate(computer text,binding_fingerprint text,binding_snapshot jsonb,
                 create_operation text,resource text,deletion_fingerprint text,context jsonb)
             ON CONFLICT(computer,binding_fingerprint,create_operation,resource) DO NOTHING",
            &[&json!(candidates)],
        ).await.map_err(unavailable)?;
        let next: String = rows.last().map(|row| row.get(0)).unwrap_or_default();
        tx.execute(
            "UPDATE computers.deleted_storage_scan SET after_computer=$1 WHERE singleton",
            &[&next],
        )
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(inserted)
    }

    pub async fn claim_deleted_storage(
        &self,
        holder: &str,
        lease: Duration,
    ) -> Result<Option<DeletedStorageJob>, DbError> {
        let secs = lease.as_secs_f64().clamp(1.0, 120.0);
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await.map_err(unavailable)?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='10s'; SET LOCAL idle_in_transaction_session_timeout='15s'")
            .await.map_err(unavailable)?;
        let row = tx.query_opt(
            &format!("UPDATE computers.deleted_storage_jobs SET lease_holder=$1,
                lease_generation=lease_generation+1,lease_until=clock_timestamp()+make_interval(secs=>$2)
             WHERE id=(SELECT id FROM computers.deleted_storage_jobs WHERE next_attempt_at<=clock_timestamp()
                AND (lease_until IS NULL OR lease_until<=clock_timestamp()) ORDER BY next_attempt_at,id
                LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING {JOB_COLUMNS}"),
            &[&holder,&secs],
        ).await.map_err(unavailable)?;
        let job = row.as_ref().map(DeletedStorageJob::from_row);
        tx.commit().await.map_err(unavailable)?;
        Ok(job)
    }

    /// Resolve only the saved account/deployment and the exact deleted create.
    pub async fn load_deleted_storage(&self, job: &DeletedStorageJob) -> Result<Entry, DbError> {
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await.map_err(unavailable)?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='10s'; SET LOCAL idle_in_transaction_session_timeout='15s'")
            .await
            .map_err(unavailable)?;
        let row = job_row(&tx, job, false).await?;
        let e = authority(&tx, job, &row).await?;
        check_expiry(&tx, job).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(e)
    }

    /// Append a historical-resource fact without changing lifecycle or allowances.
    /// Every outcome remains scheduled; this observer never declares cleanup done.
    pub async fn observe_deleted_storage(
        &self,
        job: &DeletedStorageJob,
        outcome: &Outcome<StorageObservation>,
        observed_ms: u64,
        retry: Duration,
    ) -> Result<(), DbError> {
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await.map_err(unavailable)?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='10s'; SET LOCAL idle_in_transaction_session_timeout='15s'")
            .await.map_err(unavailable)?;
        crate::usage::lock(&tx).await?;
        let row = job_row(&tx, job, true).await?;
        let current = authority(&tx, job, &row).await?;
        let context: Context = serde_json::from_value(row.get("context")).map_err(unavailable)?;
        let mut value = evidence(&job.resource, outcome);
        value["observer"] = json!("deleted_allocation");
        value["deployment_fingerprint"] = json!(job.binding_fingerprint);
        value["provider_operation"] = json!(context.provider_operation);
        value["allocation_deleted_ms"] = json!(context.deletion_ms);
        let key = format!("storage-history:{}:{}", job.id, digest(&value)?);
        value["lifecycle_ms"] = json!(current.record.updated_ms);
        value["provider_observed_ms"] = json!(observed_ms);
        // Check after every lock wait, including the account usage lock.
        check_expiry(&tx, job).await?;
        tx.execute(
            "INSERT INTO computers.usage_events(computer,owner,revision,intent_generation,kind,event_key,
                deployment,driver,size,shape,resource,boot,phase,desired,observed_ms,evidence,operation,turn_generation)
             VALUES($1,$2,$3,$4,'observed',$5,$6,$7,$8,$9,$10,NULL,$11,$12,
                floor(extract(epoch FROM clock_timestamp())*1000)::bigint,$13,$14,0)
             ON CONFLICT(computer,event_key) DO NOTHING",
            &[&job.computer,&context.owner,&i64::try_from(current.record.revision).map_err(unavailable)?,&current.intent_generation,&key,
              &context.deployment,&context.driver,&context.size,&context.shape,&job.resource,&current.phase,
              &current.desired.as_str(),&value,&job.operation],
        ).await.map_err(unavailable)?;
        finish(
            &tx,
            job,
            retry,
            value["status"].as_str().unwrap_or("unknown"),
        )
        .await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(())
    }

    /// Back off a claim that could not resolve authority, without retaining errors.
    pub async fn retry_deleted_storage(
        &self,
        job: &DeletedStorageJob,
        retry: Duration,
    ) -> Result<(), DbError> {
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await.map_err(unavailable)?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='10s'; SET LOCAL idle_in_transaction_session_timeout='15s'")
            .await
            .map_err(unavailable)?;
        job_row(&tx, job, true).await?;
        check_expiry(&tx, job).await?;
        finish(&tx, job, retry, "unknown").await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(())
    }
}

async fn job_row(
    tx: &Transaction<'_>,
    job: &DeletedStorageJob,
    lock: bool,
) -> Result<Row, DbError> {
    tx.query_opt(
        &format!("SELECT * FROM computers.deleted_storage_jobs WHERE id=$1 AND computer=$2 AND binding_fingerprint=$3
            AND create_operation=$4 AND resource=$5 AND lease_generation=$6 AND lease_holder=$7{}",if lock { " FOR UPDATE" } else { "" }),
        &[&job.id,&job.computer,&job.binding_fingerprint,&job.operation,&job.resource,&job.generation,&job.holder],
    ).await.map_err(unavailable)?.ok_or_else(fenced)
}

async fn authority(
    tx: &Transaction<'_>,
    job: &DeletedStorageJob,
    row: &Row,
) -> Result<Entry, DbError> {
    let row_entry = tx
        .query_opt(
            &format!("SELECT {COLUMNS} FROM computers.computers WHERE id=$1 FOR SHARE"),
            &[&job.computer],
        )
        .await
        .map_err(unavailable)?
        .ok_or_else(fenced)?;
    let e = entry(&row_entry).map_err(unavailable)?;
    let context: Context = serde_json::from_value(row.get("context")).map_err(unavailable)?;
    let binding = e.binding.as_ref().ok_or_else(fenced)?;
    if context.owner != e.owner
        || binding.owner != e.owner
        || binding.fingerprint != job.binding_fingerprint
        || serde_json::to_value(binding).map_err(unavailable)?
            != row.get::<_, Value>("binding_snapshot")
        || deleted(&e, &job.operation, &job.resource)
            .map(digest)
            .transpose()?
            != Some(row.get::<_, String>("deletion_fingerprint"))
    {
        return Err(fenced());
    }
    Ok(e)
}

async fn check_expiry(tx: &Transaction<'_>, job: &DeletedStorageJob) -> Result<(), DbError> {
    if !tx.query_one("SELECT EXISTS(SELECT 1 FROM computers.deleted_storage_jobs WHERE id=$1 AND lease_generation=$2 AND lease_holder=$3 AND lease_until>clock_timestamp())",&[&job.id,&job.generation,&job.holder]).await.map_err(unavailable)?.get::<_,bool>(0) { return Err(fenced()) }
    Ok(())
}

async fn finish(
    tx: &Transaction<'_>,
    job: &DeletedStorageJob,
    retry: Duration,
    status: &str,
) -> Result<(), DbError> {
    let secs = retry.as_secs_f64().clamp(1.0, 3600.0);
    let n=tx.execute("UPDATE computers.deleted_storage_jobs SET lease_holder=NULL,lease_until=NULL,
        next_attempt_at=clock_timestamp()+make_interval(secs=>$4),attempts=CASE WHEN $5='done' THEN 0 ELSE LEAST(attempts::bigint+1,2147483647)::integer END,last_status=$5
        WHERE id=$1 AND lease_generation=$2 AND lease_holder=$3 AND lease_until>clock_timestamp()",&[&job.id,&job.generation,&job.holder,&secs,&status]).await.map_err(unavailable)?;
    if n != 1 {
        return Err(fenced());
    }
    Ok(())
}

/// A separate resident loop: no lifecycle claims, provider mutations, or releases.
pub struct DeletedStorageObserver<D> {
    pub store: Store,
    pub driver: D,
    pub holder: String,
}

impl<D: Drivers> DeletedStorageObserver<D> {
    pub async fn visit(&self, job: &DeletedStorageJob) {
        let outcome = match self.store.load_deleted_storage(job).await {
            Ok(e) => match self.driver.resolve(&e) {
                Ok(driver) => {
                    tokio::time::timeout(Duration::from_secs(10), driver.storage(&job.resource))
                        .await
                        .unwrap_or_else(|_| Outcome::unknown("Storage observation timed out."))
                }
                Err(_) => Outcome::unknown("The storage service is unavailable."),
            },
            Err(_) => {
                let _ = self
                    .store
                    .retry_deleted_storage(job, Duration::from_secs(300))
                    .await;
                return;
            }
        };
        let retry = if matches!(&outcome,Outcome::Done{value} if value.computer==job.resource && value.validate().is_ok())
        {
            Duration::from_secs(60)
        } else {
            Duration::from_secs(30 * (1u64 << job.attempts.clamp(0, 7)))
                .min(Duration::from_secs(3600))
        };
        if self
            .store
            .observe_deleted_storage(job, &outcome, crate::reconcile::now_ms(), retry)
            .await
            .is_err()
        {
            eprintln!("oa-computers: storage observer could not save an observation");
        }
    }

    pub async fn run(self) {
        let mut scanned = tokio::time::Instant::now() - Duration::from_secs(30);
        loop {
            if scanned.elapsed() >= Duration::from_secs(30) {
                if self.store.seed_deleted_storage(100).await.is_err() {
                    eprintln!("oa-computers: storage observer could not scan allocations");
                }
                scanned = tokio::time::Instant::now();
            }
            for _ in 0..8 {
                match self
                    .store
                    .claim_deleted_storage(&self.holder, Duration::from_secs(60))
                    .await
                {
                    Ok(Some(job)) => self.visit(&job).await,
                    Ok(None) => break,
                    Err(_) => {
                        eprintln!("oa-computers: storage observer could not claim an allocation");
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
