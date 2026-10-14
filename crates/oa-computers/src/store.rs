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
use std::time::Duration;
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
    pub phase: String,
    pub record: Computer,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_unix: i64,
    pub updated_unix: i64,
}

const COLUMNS: &str = "id, owner, chat, driver, image, size, desired, phase, record, attempts, \
     last_error, extract(epoch from created_at)::bigint, extract(epoch from updated_at)::bigint";

fn entry(row: &Row) -> Result<Entry, DbError> {
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
        phase: row.get(7),
        record,
        attempts: row.get(9),
        last_error: row.get(10),
        created_unix: row.get(11),
        updated_unix: row.get(12),
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

#[derive(Clone)]
pub struct Store {
    pub db: Db,
}

impl Store {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Record intent. Returns the row and whether it was made now: a live
    /// computer for the same chat, or the same idempotency key, is
    /// returned as it is instead of a second one.
    pub async fn create(&self, n: &NewComputer<'_>) -> Result<(Entry, bool), DbError> {
        let c = self.db.client().await?;
        let record = serde_json::to_value(n.record).map_err(|e| DbError(e.to_string()))?;
        let inserted = c
            .query_opt(
                &format!(
                    "INSERT INTO computers.computers
                       (id, owner, chat, idempotency_key, driver, image, size, desired, phase, revision, record)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, 'running', $8, $9, $10)
                     ON CONFLICT DO NOTHING
                     RETURNING {COLUMNS}"
                ),
                &[
                    &n.record.id,
                    &n.owner,
                    &n.chat,
                    &n.idempotency_key,
                    &n.driver,
                    &n.image,
                    &n.record.size,
                    &phase_name(&n.record.phase),
                    &(n.record.revision as i64),
                    &record,
                ],
            )
            .await?;
        if let Some(row) = inserted {
            return Ok((entry(&row)?, true));
        }
        let existing = c
            .query_opt(
                &format!(
                    "SELECT {COLUMNS} FROM computers.computers
                     WHERE owner = $1 AND (
                       ($2::text IS NOT NULL AND chat = $2 AND desired <> 'deleted')
                       OR ($3::text IS NOT NULL AND idempotency_key = $3))
                     ORDER BY created_at DESC LIMIT 1"
                ),
                &[&n.owner, &n.chat, &n.idempotency_key],
            )
            .await?;
        match existing {
            Some(row) => Ok((entry(&row)?, false)),
            None => Err(DbError("the computer could not be recorded".into())),
        }
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
        let c = self.db.client().await?;
        c.query(
            &format!(
                "SELECT {COLUMNS} FROM computers.computers
                 WHERE owner = $1 AND NOT done
                 ORDER BY created_at DESC LIMIT 200"
            ),
            &[&owner],
        )
        .await?
        .iter()
        .map(entry)
        .collect()
    }

    /// Change intent and ask the reconciler to look now. A deleted
    /// computer's intent never changes back.
    pub async fn set_desired(&self, owner: &str, id: &str, d: Desired) -> Result<bool, DbError> {
        let c = self.db.client().await?;
        let n = c
            .execute(
                "UPDATE computers.computers
                 SET desired = $3, next_attempt_at = now(), attempts = 0, updated_at = now()
                 WHERE id = $1 AND owner = $2 AND (desired <> 'deleted' OR $3 = 'deleted')",
                &[&id, &owner, &d.as_str()],
            )
            .await?;
        Ok(n == 1)
    }

    /// The reconciler's own intent change (a lifecycle policy stopped it).
    pub async fn settle_desired(
        &self,
        id: &str,
        from: Desired,
        to: Desired,
    ) -> Result<(), DbError> {
        let c = self.db.client().await?;
        c.execute(
            "UPDATE computers.computers SET desired = $3, updated_at = now()
             WHERE id = $1 AND desired = $2",
            &[&id, &from.as_str(), &to.as_str()],
        )
        .await?;
        Ok(())
    }

    /// Claim the next due computer for `holder` for `lease`.
    pub async fn claim(&self, holder: &str, lease: Duration) -> Result<Option<Entry>, DbError> {
        let c = self.db.client().await?;
        let secs = lease.as_secs_f64();
        c.query_opt(
            &format!(
                "UPDATE computers.computers
                 SET lease_holder = $1, lease_until = now() + make_interval(secs => $2)
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
        next.validate().map_err(StoreError::Corrupt)?;
        if !current.preserves_history_of(next) {
            return Err(StoreError::Immutable);
        }
        let record = serde_json::to_value(next)
            .map_err(|_| StoreError::Io("Cannot encode the computer."))?;
        let c = self.store.db.client().await.map_err(io)?;
        let n = c
            .execute(
                "UPDATE computers.computers
                 SET record = $3, revision = $4, phase = $5, updated_at = now(),
                     lease_until = now() + make_interval(secs => $7)
                 WHERE id = $1 AND revision = $2 AND lease_holder = $6 AND lease_until > now()",
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
        if n == 1 {
            Ok(())
        } else {
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
                self.commit(&current, &next).await?;
                Ok(*next)
            }
            Applied::Unchanged => Ok(current),
        }
    }
}
