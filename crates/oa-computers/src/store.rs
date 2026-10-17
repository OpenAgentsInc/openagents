//! Intent and observed state in Postgres.
//!
//! The API writes intent only (a new row, `desired`). The reconciler is the
//! only writer of observed state (`record`, `phase`, `revision`), and only
//! through a [`PgLease`]: a row lease (`lease_holder`, `lease_until`)
//! claimed with `FOR UPDATE SKIP LOCKED`, so two reconcilers (or a
//! restarted one) never drive the same computer at once, and every write is
//! fenced on the record's revision and on still holding the lease. The
//! lifecycle core's history rules (`Computer::preserves_history_of`) hold
//! here exactly as in its file store.

use crate::db::{Db, DbError};
use coder_working_computer::driver::Held;
use coder_working_computer::store::{Result as StoreResult, StoreError};
use coder_working_computer::{Applied, Command, Computer, Phase, apply};
use serde::Serialize;
use std::{collections::BTreeMap, time::Duration};
use tokio_postgres::Row;

/// What the API and reconciler want a computer to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Desired {
    Running,
    Stopped,
    Deleted,
}
impl Desired {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Deleted => "deleted",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "stopped" => Some(Self::Stopped),
            "deleted" => Some(Self::Deleted),
            _ => None,
        }
    }
}

/// One row.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub owner: String,
    pub chat: Option<String>,
    pub driver: String,
    pub image: Option<String>,
    pub size: String,
    pub desired: Desired,
    pub intent_generation: i64,
    pub phase: String,
    pub record: Computer,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_unix: i64,
    pub updated_unix: i64,
    pub binding: Option<crate::registry::Binding>,
    pub stopped_retention_ms: Option<u64>,
}

pub(crate) const COLUMNS: &str = "id, owner, chat, driver, image, size, desired, phase, record, attempts, \
     last_error, extract(epoch from created_at)::bigint, extract(epoch from updated_at)::bigint, binding, intent_generation, stopped_retention_ms";

pub(crate) fn entry(row: &Row) -> Result<Entry, DbError> {
    let record: serde_json::Value = row.get(8);
    let record: Computer = serde_json::from_value(record)
        .map_err(|e| DbError(format!("a computer record does not decode: {e}")))?;
    Ok(Entry {
        id: row.get(0),
        owner: row.get(1),
        chat: row.get(2),
        driver: row.get(3),
        image: row.get(4),
        size: row.get(5),
        desired: Desired::parse(row.get(6)).unwrap_or(Desired::Stopped),
        intent_generation: row.get(14),
        stopped_retention_ms: row
            .get::<_, Option<i64>>(15)
            .map(u64::try_from)
            .transpose()
            .map_err(|_| DbError("The computer retention period is invalid.".into()))?,
        phase: row.get(7),
        record,
        attempts: row.get(9),
        last_error: row.get(10),
        created_unix: row.get(11),
        updated_unix: row.get(12),
        binding: row
            .get::<_, Option<serde_json::Value>>(13)
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| DbError("The saved computer settings could not be read.".into()))?,
    })
}

/// The phase's name as stored (`new`, `awake`, `turn`, …).
pub fn phase_name(p: &Phase) -> String {
    serde_json::to_value(p)
        .ok()
        .and_then(|v| v["phase"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A new computer's intent.
pub struct NewComputer<'a> {
    pub owner: &'a str,
    pub chat: Option<&'a str>,
    pub idempotency_key: Option<&'a str>,
    pub driver: &'a str,
    pub image: Option<&'a str>,
    pub record: &'a Computer,
}

struct Insert {
    driver: String,
    image: Option<String>,
    record: Computer,
    binding: Option<crate::registry::Binding>,
}

/// Admission failures are distinct from temporary database failures.
#[derive(Debug)]
pub enum CreateError {
    Database(DbError),
    Conflict,
    Limit,
    Busy,
    Admission(crate::registry::Refused),
}

impl From<DbError> for CreateError {
    fn from(value: DbError) -> Self {
        Self::Database(value)
    }
}

impl From<tokio_postgres::Error> for CreateError {
    fn from(value: tokio_postgres::Error) -> Self {
        Self::Database(value.into())
    }
}

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(error) => error.fmt(f),
            Self::Conflict => f.write_str("That request already names a different computer."),
            Self::Limit => f.write_str("Delete an unused computer before creating another."),
            Self::Busy => f.write_str("Computers are busy. Try again shortly."),
            Self::Admission(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CreateError {}

/// Includes resources still awaiting deletion, so queued cleanup cannot bypass admission.
pub const MAX_ACCOUNT_COMPUTERS: i64 = 20;

#[derive(Clone)]
pub struct Store {
    pub db: Db,
    admission: std::sync::Arc<tokio::sync::Semaphore>,
}

impl Store {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            admission: std::sync::Arc::new(tokio::sync::Semaphore::new(32)),
        }
    }

    /// Assign old rows only through an explicit previous-deployment mapping.
    /// Image omission and operation identities remain exactly as issued.
    pub async fn pin_legacy(&self, catalog: &crate::registry::Catalog) -> Result<u64, DbError> {
        let drivers: Vec<String> = catalog
            .targets
            .values()
            .filter_map(|target| target.legacy_driver.clone())
            .collect();
        if drivers.is_empty() {
            return Ok(0);
        }
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")
            .await?;
        tx.query_one("SELECT pg_advisory_xact_lock(11809)", &[])
            .await?;
        let rows=tx.query(&format!("SELECT {COLUMNS} FROM computers.computers WHERE binding IS NULL AND driver=ANY($1) ORDER BY created_at LIMIT 1000 FOR UPDATE SKIP LOCKED"),&[&drivers]).await?;
        let mut count = 0;
        for row in rows {
            let entry = entry(&row)?;
            let binding = catalog
                .legacy_binding(&entry)
                .map_err(|_| DbError("The previous computer service is not configured.".into()))?;
            let value = serde_json::to_value(binding)
                .map_err(|_| DbError("The computer settings could not be recorded.".into()))?;
            count += tx
                .execute(
                    "UPDATE computers.computers SET binding=$2 WHERE id=$1 AND binding IS NULL",
                    &[&entry.id, &value],
                )
                .await?;
        }
        tx.commit().await?;
        Ok(count)
    }

    /// Record intent. Returns the row and whether it was made now: a live
    /// computer for the same chat, or the same idempotency key, is
    /// returned as it is instead of a second one.
    pub async fn create(&self, n: &NewComputer<'_>) -> Result<(Entry, bool), CreateError> {
        self.create_inner(
            n.owner,
            n.chat,
            n.idempotency_key,
            false,
            |e| {
                if n.idempotency_key.is_some()
                    && (e.chat.as_deref() != n.chat
                        || e.driver != n.driver
                        || e.image.as_deref() != n.image
                        || e.size != n.record.size
                        || e.record.base != n.record.base)
                {
                    Err(CreateError::Conflict)
                } else {
                    Ok(())
                }
            },
            |_| {
                Ok(Insert {
                    driver: n.driver.into(),
                    image: n.image.map(str::to_owned),
                    record: n.record.clone(),
                    binding: None,
                })
            },
        )
        .await
    }

    /// Retry lookup precedes today's placement choice, including when the
    /// original deployment was removed from the current configuration.
    pub async fn create_placed(
        &self,
        owner: &str,
        intent: &crate::registry::CreateIntent,
        catalog: &crate::registry::Catalog,
    ) -> Result<(Entry, bool), CreateError> {
        self.create_inner(
            owner,
            intent.chat.as_deref(),
            intent.idempotency_key.as_deref(),
            true,
            |entry| {
                if intent.legacy_ensure() {
                    return Ok(());
                }
                if let Some(binding) = entry.binding.as_ref().filter(|binding| !binding.migrated) {
                    if binding.request_fingerprint == intent.fingerprint(&binding.size) {
                        return Ok(());
                    }
                    return Err(CreateError::Conflict);
                }
                // Old rows have no original placement request. Preserve the old
                // immutable settings; never infer acceptance of a new requirement.
                if intent.placement.is_some()
                    || intent.needs.is_some()
                    || intent.egress_profile.is_some()
                    || intent.enrolled_host.is_some()
                    || entry.chat != intent.chat
                    || entry.image != intent.image
                    || intent.size.as_ref().is_some_and(|size| size != &entry.size)
                    || entry
                        .record
                        .base
                        .as_ref()
                        .map(|base| (&base.environment, &base.version))
                        != intent.environment.as_ref().zip(intent.version.as_ref())
                {
                    return Err(CreateError::Conflict);
                }
                Ok(())
            },
            |counts| {
                // Unassigned legacy rows conservatively consume every deployment
                // capacity until their previous connection is explicitly pinned.
                let mut counts = counts.clone();
                let unassigned = counts.remove("").unwrap_or(0);
                for id in catalog.targets.keys() {
                    *counts.entry(id.clone()).or_default() += unassigned;
                }
                let binding = catalog
                    .choose(owner, intent, &counts)
                    .map_err(CreateError::Admission)?;
                let target = &catalog.targets[&binding.deployment];
                let body = crate::api::CreateBody {
                    chat: intent.chat.clone(),
                    image: intent.image.clone(),
                    size: Some(binding.size.clone()),
                    idempotency_key: intent.idempotency_key.clone(),
                    environment: intent.environment.clone(),
                    version: intent.version.clone(),
                    free_allowance: intent.free_allowance,
                    ..Default::default()
                };
                let record = crate::api::admit_create(&target.caps, owner, &body)
                    .map_err(|_| CreateError::Admission(crate::registry::Refused::Invalid))?;
                let image = binding.artifact.as_ref().and_then(|artifact| {
                    (artifact.kind != computer_driver::ArtifactKind::Test
                        || artifact.reference != "test:empty")
                        .then(|| artifact.reference.clone())
                });
                Ok(Insert {
                    driver: binding.driver.clone(),
                    image,
                    record,
                    binding: Some(binding),
                })
            },
        )
        .await
    }

    async fn create_inner<Existing, New>(
        &self,
        owner: &str,
        chat: Option<&str>,
        key: Option<&str>,
        placement_admission: bool,
        matches_existing: Existing,
        make_new: New,
    ) -> Result<(Entry, bool), CreateError>
    where
        Existing: Fn(&Entry) -> Result<(), CreateError>,
        New: FnOnce(&BTreeMap<String, i64>) -> Result<Insert, CreateError>,
    {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| CreateError::Busy)?;
        let mut c = self.db.independent().await?;
        let tx = c.transaction().await?;
        tx.batch_execute(
            "SET LOCAL lock_timeout = '5s';
             SET LOCAL statement_timeout = '10s';
             SET LOCAL idle_in_transaction_session_timeout = '15s'",
        )
        .await?;
        // All creates for an account share this transaction lock. It covers
        // retry lookup and quota admission, including simultaneous API requests.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 11801))",
            &[&owner],
        )
        .await?;
        let existing = tx
            .query(
                &format!(
                    "SELECT {COLUMNS} FROM computers.computers
                     WHERE owner = $1 AND (
                       ($2::text IS NOT NULL AND chat = $2 AND desired <> 'deleted'
                        AND NOT EXISTS (SELECT 1 FROM computers.create_requests
                                        WHERE owner = $1 AND request_key = $3)
                        AND NOT EXISTS (SELECT 1 FROM computers.computers
                                        WHERE owner = $1 AND idempotency_key = $3))
                       OR ($3::text IS NOT NULL AND idempotency_key = $3)
                       OR id IN (SELECT computer FROM computers.create_requests
                                  WHERE owner = $1 AND request_key = $3))
                     ORDER BY created_at DESC LIMIT 2"
                ),
                &[&owner, &chat, &key],
            )
            .await?;
        if existing.len() > 1 {
            return Err(CreateError::Conflict);
        }
        if let Some(row) = existing.first() {
            let e = entry(row)?;
            matches_existing(&e)?;
            remember_request(&tx, owner, key, &e.id).await?;
            tx.commit().await?;
            return Ok((e, false));
        }
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM computers.computers WHERE owner = $1 AND NOT done",
                &[&owner],
            )
            .await?
            .get(0);
        if count >= MAX_ACCOUNT_COMPUTERS {
            return Err(CreateError::Limit);
        }
        let mut counts = BTreeMap::new();
        if placement_admission {
            // Serialize placement capacity across accounts, after the account lock.
            tx.query_one("SELECT pg_advisory_xact_lock(11809)", &[])
                .await?;
            for row in tx.query("SELECT binding->>'deployment', count(*) FROM computers.computers WHERE NOT done GROUP BY binding->>'deployment'", &[]).await? {
                counts.insert(row.get::<_,Option<String>>(0).unwrap_or_default(), row.get::<_,i64>(1));
            }
        }
        let insert = make_new(&counts)?;
        let record = serde_json::to_value(&insert.record).map_err(|e| DbError(e.to_string()))?;
        let binding = insert
            .binding
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| DbError("The computer settings could not be recorded.".into()))?;
        let inserted = tx
            .query_opt(
                &format!(
                    "INSERT INTO computers.computers
                       (id, owner, chat, idempotency_key, driver, image, size, desired, phase, revision, record, binding,usage_revision,usage_intent_generation)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, 'running', $8, $9, $10, $11, $9,0)
                     ON CONFLICT DO NOTHING
                     RETURNING {COLUMNS}"
                ),
                &[
                    &insert.record.id,
                    &owner,
                    &chat,
                    &key,
                    &insert.driver,
                    &insert.image,
                    &insert.record.size,
                    &phase_name(&insert.record.phase),
                    &(insert.record.revision as i64),
                    &record,
                    &binding,
                ],
            )
            .await?;
        if let Some(row) = inserted {
            let e = entry(&row)?;
            crate::usage::lock(&tx).await?;
            crate::usage::reserve(&tx, &e, &e.record)
                .await
                .map_err(|error| {
                    if error.0 == crate::usage::QUOTA_ERROR {
                        CreateError::Limit
                    } else {
                        CreateError::Database(error)
                    }
                })?;
            crate::usage::record(
                &tx,
                &e,
                "accepted",
                "create",
                e.record.created_ms as i64,
                crate::usage::lifecycle_evidence(),
            )
            .await?;
            remember_request(&tx, owner, key, &e.id).await?;
            tx.commit().await?;
            return Ok((e, true));
        }
        Err(CreateError::Conflict)
    }

    /// Find an accepted create without issuing a new allocation intent.
    pub async fn by_request(&self, owner: &str, key: &str) -> Result<Option<Entry>, DbError> {
        self.db.client().await?.query_opt(
            &format!("SELECT {COLUMNS} FROM computers.computers WHERE owner = $1 AND idempotency_key = $2"),
            &[&owner, &key],
        ).await?.map(|row| entry(&row)).transpose()
    }

    pub async fn get(&self, owner: &str, id: &str) -> Result<Option<Entry>, DbError> {
        let c = self.db.client().await?;
        c.query_opt(
            &format!("SELECT {COLUMNS} FROM computers.computers WHERE id = $1 AND owner = $2"),
            &[&id, &owner],
        )
        .await?
        .map(|r| entry(&r))
        .transpose()
    }

    /// The owner's computers, newest first; deleted ones only while they
    /// are still being torn down.
    pub async fn list(&self, owner: &str) -> Result<Vec<Entry>, DbError> {
        self.list_page(owner, None, 200).await
    }

    /// Stable keyset pagination. The cursor is resolved under the same owner.
    pub async fn list_page(
        &self,
        owner: &str,
        after: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Entry>, DbError> {
        let c = self.db.client().await?;
        c.query(
            &format!(
                "SELECT {COLUMNS} FROM computers.computers
                 WHERE owner = $1 AND NOT done
                   AND ($2::text IS NULL OR (created_at, id) < (
                     SELECT created_at, id FROM computers.computers WHERE owner = $1 AND id = $2))
                 ORDER BY created_at DESC, id DESC LIMIT $3"
            ),
            &[&owner, &after, &limit.clamp(1, 201)],
        )
        .await?
        .iter()
        .map(entry)
        .collect()
    }

    /// Change intent and ask the reconciler to look now. A deleted
    /// computer's intent never changes back.
    pub async fn set_desired(&self, owner: &str, id: &str, d: Desired) -> Result<bool, DbError> {
        let mut c = self.db.independent().await?;
        let tx = c.transaction().await?;
        crate::usage::lock(&tx).await?;
        let row = tx.query_one(
            "WITH changed AS (
                UPDATE computers.computers SET desired=$3,intent_generation=intent_generation+1,usage_intent_generation=intent_generation+1,next_attempt_at=now(),attempts=0,updated_at=now()
                WHERE id=$1 AND owner=$2 AND (desired<>'deleted' OR $3='deleted') RETURNING id
             ), cancelled AS (
                UPDATE computers.commands SET cancel_requested=true,
                    status=CASE WHEN status='queued' THEN 'cancelled' ELSE status END,
                    finished_at=CASE WHEN status='queued' THEN now() ELSE finished_at END
                WHERE computer IN (SELECT id FROM changed) AND $3<>'running' AND finished_at IS NULL
                RETURNING turn,status
             ), ended AS (
                UPDATE computers.turns SET state='failed',finished_at=now()
                WHERE id IN (SELECT turn FROM cancelled WHERE status='cancelled') RETURNING id
             ) SELECT count(*)::bigint FROM changed",
            &[&id,&owner,&d.as_str()]).await?;
        let changed = row.get::<_, i64>(0) == 1;
        if changed {
            let e = entry(
                &tx.query_one(
                    &format!("SELECT {COLUMNS} FROM computers.computers WHERE id=$1 AND owner=$2"),
                    &[&id, &owner],
                )
                .await?,
            )?;
            if d == Desired::Running {
                crate::usage::reserve(&tx, &e, &e.record).await?;
            }
            crate::usage::record(
                &tx,
                &e,
                "accepted",
                &format!("intent:{}", e.intent_generation),
                crate::reconcile::now_ms() as i64,
                crate::usage::lifecycle_evidence(),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(changed)
    }

    /// Change an automatic intent only if no newer request has arrived.
    pub async fn settle_desired(
        &self,
        id: &str,
        intent_generation: i64,
        from: Desired,
        to: Desired,
    ) -> Result<bool, DbError> {
        let mut c = self.db.independent().await?;
        let tx = c.transaction().await?;
        crate::usage::lock(&tx).await?;
        let row = tx
            .query_opt(
                "UPDATE computers.computers SET desired=$3, intent_generation=intent_generation+1,
                usage_intent_generation=intent_generation+1,updated_at=now()
             WHERE id=$1 AND desired=$2 AND intent_generation=$4 RETURNING owner",
                &[&id, &from.as_str(), &to.as_str(), &intent_generation],
            )
            .await?;
        let changed = row.is_some();
        if let Some(row) = row {
            crate::usage::record_intent(&tx, row.get(0), id).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }

    /// Claim the next due computer for `holder` for `lease`.
    pub async fn claim(&self, holder: &str, lease: Duration) -> Result<Option<Entry>, DbError> {
        let c = self.db.client().await?;
        let secs = lease.as_secs_f64();
        c.query_opt(
            &format!(
                "UPDATE computers.computers
                 SET lease_holder = $1, lease_until = now() + make_interval(secs => $2),
                     binding_lease_until = now() + make_interval(secs => $2)
                 WHERE id = (
                   SELECT id FROM computers.computers
                   WHERE NOT done AND next_attempt_at <= now()
                     AND (lease_until IS NULL OR lease_until < now())
                   ORDER BY next_attempt_at
                   LIMIT 1
                   FOR UPDATE SKIP LOCKED)
                 RETURNING {COLUMNS}"
            ),
            &[&holder, &secs],
        )
        .await?
        .map(|r| entry(&r))
        .transpose()
    }

    /// Give a claimed computer back with when to look again.
    pub async fn release(
        &self,
        id: &str,
        holder: &str,
        again_in: Duration,
        attempts: i32,
        error: Option<&str>,
        done: bool,
    ) -> Result<(), DbError> {
        let c = self.db.client().await?;
        c.execute(
            "UPDATE computers.computers
             SET lease_holder = NULL, lease_until = NULL,
                 next_attempt_at = GREATEST(next_attempt_at, now() + make_interval(secs => $3)),
                 attempts = $4, last_error = $5, done = $6, updated_at = now()
             WHERE id = $1 AND lease_holder = $2",
            &[
                &id,
                &holder,
                &again_in.as_secs_f64(),
                &attempts,
                &error,
                &done,
            ],
        )
        .await?;
        Ok(())
    }

    /// Keep holding a lease while a long effect runs.
    pub async fn renew(&self, id: &str, holder: &str, lease: Duration) -> Result<bool, DbError> {
        let c = self.db.client().await?;
        let n = c
            .execute(
                "UPDATE computers.computers
                 SET lease_until = now() + make_interval(secs => $3)
                 WHERE id = $1 AND lease_holder = $2",
                &[&id, &holder, &lease.as_secs_f64()],
            )
            .await?;
        Ok(n == 1)
    }

    /// Ask the reconciler to visit `id` now (a turn is waiting).
    pub async fn poke(&self, id: &str) -> Result<(), DbError> {
        let c = self.db.client().await?;
        c.execute(
            "UPDATE computers.computers SET next_attempt_at = now() WHERE id = $1 AND NOT done",
            &[&id],
        )
        .await?;
        Ok(())
    }
}

async fn remember_request(
    tx: &tokio_postgres::Transaction<'_>,
    owner: &str,
    key: Option<&str>,
    id: &str,
) -> Result<(), CreateError> {
    if let Some(key) = key {
        let known = tx
            .query_opt(
                "SELECT 1 FROM computers.create_requests WHERE owner = $1 AND request_key = $2",
                &[&owner, &key],
            )
            .await?
            .is_some();
        if !known {
            let aliases: i64 = tx
                .query_one(
                    "SELECT count(*) FROM computers.create_requests WHERE computer = $1",
                    &[&id],
                )
                .await?
                .get(0);
            if aliases >= 128 {
                return Err(CreateError::Limit);
            }
            tx.execute(
                "INSERT INTO computers.create_requests (owner, request_key, computer) VALUES ($1, $2, $3)",
                &[&owner, &key, &id],
            )
            .await?;
        }
    }
    Ok(())
}

/// A held computer: reads and fenced writes of its observed record.
pub struct PgLease {
    pub store: Store,
    pub id: String,
    pub holder: String,
    pub lease: Duration,
}

fn io(_: DbError) -> StoreError {
    StoreError::Io("The computer store is unavailable.")
}

impl PgLease {
    async fn current(&self) -> StoreResult<Computer> {
        let c = self.store.db.client().await.map_err(io)?;
        let row = c
            .query_opt(
                "SELECT record, lease_holder = $2 AND lease_until > now()
                 FROM computers.computers WHERE id = $1",
                &[&self.id, &self.holder],
            )
            .await
            .map_err(|e| io(e.into()))?
            .ok_or(StoreError::NotFound)?;
        let held: Option<bool> = row.get(1);
        if held != Some(true) {
            return Err(StoreError::Busy);
        }
        let record: serde_json::Value = row.get(0);
        let computer: Computer = serde_json::from_value(record)
            .map_err(|_| StoreError::Corrupt("Cannot decode the computer."))?;
        if computer.id != self.id {
            return Err(StoreError::Corrupt("Computer identity mismatch."));
        }
        Ok(computer)
    }

    /// Retain `next` if the stored record is still at `expected` and this
    /// holder still holds the lease.
    pub async fn commit(&self, current: &Computer, next: &Computer) -> StoreResult<()> {
        self.commit_evidence(current, next, None).await
    }

    // Keep the transaction future out of every branch of the lifecycle driver.
    fn commit_evidence<'a>(
        &'a self,
        current: &'a Computer,
        next: &'a Computer,
        meter: Option<serde_json::Value>,
    ) -> impl std::future::Future<Output = StoreResult<()>> + 'a {
        Box::pin(self.commit_evidence_inner(current, next, meter))
    }

    async fn commit_evidence_inner(
        &self,
        current: &Computer,
        next: &Computer,
        meter: Option<serde_json::Value>,
    ) -> StoreResult<()> {
        next.validate().map_err(StoreError::Corrupt)?;
        if !current.preserves_history_of(next) {
            return Err(StoreError::Immutable);
        }
        let record = serde_json::to_value(next)
            .map_err(|_| StoreError::Io("Cannot encode the computer."))?;
        let mut c = self.store.db.independent().await.map_err(io)?;
        let tx = c.transaction().await.map_err(|e| io(e.into()))?;
        crate::usage::lock(&tx).await.map_err(io)?;
        let row = tx
            .query_opt(
                &format!(
                    "UPDATE computers.computers
                 SET record = $3, revision = $4, usage_revision = $4, phase = $5, updated_at = now(),
                     lease_until = now() + make_interval(secs => $7)
                 WHERE id = $1 AND revision = $2 AND lease_holder = $6 AND lease_until > now()
                 RETURNING {COLUMNS}"
                ),
                &[
                    &self.id,
                    &(current.revision as i64),
                    &record,
                    &(next.revision as i64),
                    &phase_name(&next.phase),
                    &self.holder,
                    &self.lease.as_secs_f64(),
                ],
            )
            .await
            .map_err(|e| io(e.into()))?;
        if let Some(row) = row {
            let e = entry(&row).map_err(io)?;
            if matches!(next.phase, Phase::Creating | Phase::Booting) {
                crate::usage::reserve(&tx, &e, next).await.map_err(io)?;
            }
            let kind = if meter.is_some() { "meter" } else { "observed" };
            crate::usage::record(
                &tx,
                &e,
                kind,
                &format!("revision:{}", next.revision),
                crate::reconcile::now_ms() as i64,
                meter.unwrap_or_else(crate::usage::lifecycle_evidence),
            )
            .await
            .map_err(io)?;
            tx.commit().await.map_err(|e| io(e.into()))?;
            Ok(())
        } else {
            tx.rollback().await.map_err(|e| io(e.into()))?;
            Err(StoreError::Fence {
                expected: current.revision,
                current: self.current().await.map(|c| c.revision).unwrap_or(0),
            })
        }
    }
}

impl Held for PgLease {
    async fn read(&self) -> StoreResult<Computer> {
        self.current().await
    }
    async fn apply(&self, command: &Command, now_ms: u64) -> StoreResult<Computer> {
        let current = self.current().await?;
        match apply(&current, command, now_ms)? {
            Applied::Changed(next) => {
                let meter = match command {
                    Command::ObserveMeter { outcome, .. } => {
                        Some(crate::usage::meter_evidence(outcome))
                    }
                    _ => None,
                };
                self.commit_evidence(&current, &next, meter).await?;
                Ok(*next)
            }
            Applied::Unchanged => Ok(current),
        }
    }
}
