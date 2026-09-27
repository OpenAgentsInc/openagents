//! NIP-PL executor state: transactional lease rows, the durable match
//! cursor, and wake jobs.
//!
//! A lease row commits in the same transaction as its `kind:30350` event.
//! The matcher reads the ingest sequence from a durable cursor and inserts
//! jobs idempotently in the same transaction that advances the cursor, so a
//! restart neither loses nor duplicates a match. A job is claimed by at most
//! one worker at a time, and every state change after a claim is fenced by
//! the claim token.

use serde_json::Value;
use tokio_postgres::{Transaction, types::ToSql};

use nostr::push_lease::ALLOWED_SKEW;

use crate::domain::Event;

use super::{StoreError, StoredEvent, decode_event_row, pg_i64, pg_limit, statements::Statements};

const LEASE_LOCK_SQL: &str =
    "SELECT pg_advisory_xact_lock(hashtextextended('nip-pl:' || $1 || ':' || $2, 0))";
const LEASE_ROW_SQL: &str = r#"
SELECT generation FROM push_lease
WHERE origin = $1 AND author = $2 AND installation = $3
FOR UPDATE
"#;
const RETIRE_EXPIRED_SQL: &str = r#"
UPDATE push_lease
SET active = FALSE, app_profile = NULL, transport = NULL, endpoint_hash = NULL,
    subscriptions = NULL, endpoint_invalid_at = NULL, updated_at = clock_timestamp()
WHERE origin = $1 AND author = $2 AND active AND expires_at <= $3
"#;
const ENDPOINT_TAKEN_SQL: &str = r#"
SELECT EXISTS (
    SELECT 1 FROM push_lease
    WHERE origin = $1 AND author = $2 AND active AND installation <> $3
      AND app_profile = $4 AND transport = $5 AND endpoint_hash = $6
)
"#;
const ACTIVE_OTHERS_SQL: &str = r#"
SELECT count(*) FROM push_lease
WHERE origin = $1 AND author = $2 AND active AND installation <> $3
"#;
const ACTIVE_PROFILE_OTHERS_SQL: &str = r#"
SELECT count(*) FROM push_lease
WHERE origin = $1 AND author = $2 AND active AND installation <> $3 AND app_profile = $4
"#;
const UPSERT_LEASE_SQL: &str = r#"
INSERT INTO push_lease (
    origin, author, installation, event_id, created_at, expires_at, generation,
    active, app_profile, transport, endpoint_hash, subscriptions, retain_until
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::text::jsonb, $13)
ON CONFLICT (origin, author, installation) DO UPDATE SET
    event_id = EXCLUDED.event_id,
    created_at = EXCLUDED.created_at,
    expires_at = EXCLUDED.expires_at,
    generation = EXCLUDED.generation,
    active = EXCLUDED.active,
    app_profile = EXCLUDED.app_profile,
    transport = EXCLUDED.transport,
    endpoint_hash = EXCLUDED.endpoint_hash,
    subscriptions = EXCLUDED.subscriptions,
    endpoint_invalid_at = NULL,
    retain_until = GREATEST(push_lease.retain_until, EXCLUDED.retain_until),
    updated_at = clock_timestamp()
"#;
const SUPPRESS_REPLACED_SQL: &str = r#"
UPDATE push_delivery_job
SET state = 'suppressed', last_error = 'lease_replaced', claim_token = NULL,
    claimed_until = NULL, finished_at = $5
WHERE origin = $1 AND author = $2 AND installation = $3
  AND state IN ('pending', 'claimed') AND generation < $4
"#;
const CURSOR_INIT_SQL: &str = r#"
INSERT INTO push_match_cursor (origin, ingest_seq)
VALUES ($1, (SELECT COALESCE(max(ingest_seq), 0) FROM nostr_event))
ON CONFLICT (origin) DO NOTHING
"#;
const CURSOR_LOCK_SQL: &str =
    "SELECT ingest_seq FROM push_match_cursor WHERE origin = $1 FOR UPDATE";
const CURSOR_ADVANCE_SQL: &str = "UPDATE push_match_cursor SET ingest_seq = $2 WHERE origin = $1";
const EVENTS_AFTER_SQL: &str = r#"
SELECT id, pubkey, created_at, kind, tags::text, content, sig, ingest_seq
FROM nostr_event
WHERE ingest_seq > $1 AND (expires_at IS NULL OR expires_at > $2)
ORDER BY ingest_seq ASC
LIMIT $3
"#;
const LATEST_SEQ_SQL: &str = "SELECT COALESCE(max(ingest_seq), 0) FROM nostr_event";
const ACTIVE_LEASES_SQL: &str = r#"
SELECT author, installation, event_id, generation, expires_at, app_profile,
       transport, endpoint_hash, subscriptions::text
FROM push_lease
WHERE origin = $1 AND active AND expires_at > $2 AND endpoint_invalid_at IS NULL
ORDER BY author, installation
"#;
const READER_POLICY_SQL: &str = r#"
SELECT
    (SELECT closed_membership FROM relay_policy WHERE singleton = TRUE),
    EXISTS (SELECT 1 FROM relay_member_pubkey WHERE pubkey = $1),
    EXISTS (SELECT 1 FROM relay_blocked_pubkey WHERE pubkey = $1)
"#;
const INSERT_JOB_SQL: &str = r#"
INSERT INTO push_delivery_job (
    job_id, origin, author, installation, generation, app_profile, transport,
    endpoint_hash, event_id, state, next_attempt_at, created_at, expires_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10, $10, $11)
ON CONFLICT DO NOTHING
"#;
const EXPIRE_JOBS_SQL: &str = r#"
UPDATE push_delivery_job
SET state = 'dead', last_error = 'expired', claim_token = NULL,
    claimed_until = NULL, finished_at = $2
WHERE origin = $1 AND state IN ('pending', 'claimed') AND expires_at <= $2
"#;
const CLAIM_JOBS_SQL: &str = r#"
UPDATE push_delivery_job job
SET state = 'claimed', claim_token = $2, claimed_until = $4,
    attempts = job.attempts + 1
WHERE job.job_id IN (
    SELECT job_id FROM push_delivery_job
    WHERE origin = $1
      AND ((state = 'pending' AND next_attempt_at <= $3)
        OR (state = 'claimed' AND claimed_until <= $3))
    ORDER BY next_attempt_at, job_id
    LIMIT $5
    FOR UPDATE SKIP LOCKED
)
RETURNING job.job_id, job.origin, job.author, job.installation, job.generation,
          job.app_profile, job.transport, job.endpoint_hash, job.event_id,
          job.attempts, job.expires_at
"#;
const LEASE_FOR_JOB_SQL: &str = r#"
SELECT event_id, generation, active, expires_at, endpoint_hash, endpoint_invalid_at
FROM push_lease
WHERE origin = $1 AND author = $2 AND installation = $3
"#;
const FINISH_JOB_SQL: &str = r#"
UPDATE push_delivery_job
SET state = $3::text, last_error = $4::text, claim_token = NULL, claimed_until = NULL,
    next_attempt_at = COALESCE($5::bigint, next_attempt_at),
    finished_at = CASE WHEN $3::text = 'pending' THEN NULL ELSE $6::bigint END
WHERE job_id = $1 AND claim_token = $2 AND state = 'claimed'
"#;
const INVALIDATE_ENDPOINT_SQL: &str = r#"
UPDATE push_lease SET endpoint_invalid_at = $5, updated_at = clock_timestamp()
WHERE origin = $1 AND author = $2 AND installation = $3 AND generation = $4 AND active
"#;
const SUPPRESS_GENERATION_SQL: &str = r#"
UPDATE push_delivery_job
SET state = 'suppressed', last_error = 'endpoint_invalid', claim_token = NULL,
    claimed_until = NULL, finished_at = $5
WHERE origin = $1 AND author = $2 AND installation = $3 AND generation = $4
  AND state = 'pending'
"#;
const JOBS_SQL: &str = r#"
SELECT job_id, author, installation, generation, event_id, state, attempts,
       next_attempt_at, last_error
FROM push_delivery_job
WHERE origin = $1
ORDER BY created_at, job_id
LIMIT $2
"#;
const LEASE_STATE_SQL: &str = r#"
SELECT event_id, generation, active, expires_at, endpoint_hash, endpoint_invalid_at, retain_until
FROM push_lease
WHERE origin = $1 AND author = $2 AND installation = $3
"#;
const SWEEP_LEASES_SQL: &str = "DELETE FROM push_lease WHERE NOT active AND retain_until < $1";
const SWEEP_JOBS_SQL: &str =
    "DELETE FROM push_delivery_job WHERE finished_at IS NOT NULL AND finished_at < $1";

/// A lease the executor decrypted and validated, ready to commit with its event.
#[derive(Clone, PartialEq, Eq)]
pub struct PushLeaseWrite {
    /// Canonical origin. The tenant key.
    pub origin: String,
    /// Lease author.
    pub author: String,
    /// The lease's `d` value.
    pub installation: String,
    /// Plaintext generation.
    pub generation: u64,
    /// False for a revocation tombstone.
    pub active: bool,
    /// Public `expiration`.
    pub expires_at: u64,
    /// Present on an active lease.
    pub app_profile: Option<String>,
    /// Present on an active lease.
    pub transport: Option<String>,
    /// SHA-256 of the endpoint, present on an active lease.
    pub endpoint_hash: Option<String>,
    /// Normalized subscriptions, present on an active lease.
    pub subscriptions: Option<Value>,
    /// Active lease addresses per author at this origin.
    pub max_active_leases: usize,
    /// Active lease addresses per author for this lease's application
    /// profile, when the profile sets its own quota.
    pub max_profile_leases: Option<usize>,
    /// Advertised maximum lease lifetime, used for watermark retention.
    pub max_lease_ttl: u64,
}

impl std::fmt::Debug for PushLeaseWrite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PushLeaseWrite")
            .field("origin", &self.origin)
            .field("author", &self.author)
            .field("generation", &self.generation)
            .field("active", &self.active)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

/// An active, unexpired lease as the matcher sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePushLease {
    /// Lease author.
    pub author: String,
    /// The lease's `d` value.
    pub installation: String,
    /// Accepted lease event.
    pub event_id: String,
    /// Accepted generation.
    pub generation: u64,
    /// Public expiration.
    pub expires_at: u64,
    /// Application profile.
    pub app_profile: String,
    /// Transport.
    pub transport: String,
    /// SHA-256 of the endpoint.
    pub endpoint_hash: String,
    /// Normalized subscriptions.
    pub subscriptions: Value,
}

/// One claimed wake job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushJob {
    /// Stable UUID. The gateway request ID.
    pub job_id: String,
    /// Origin selected at match time.
    pub origin: String,
    /// Lease author.
    pub author: String,
    /// Lease `d` value.
    pub installation: String,
    /// Lease generation selected at match time.
    pub generation: u64,
    /// Application profile.
    pub app_profile: String,
    /// Transport.
    pub transport: String,
    /// SHA-256 of the endpoint.
    pub endpoint_hash: String,
    /// Matched event.
    pub event_id: String,
    /// Attempts, including this claim.
    pub attempts: u32,
    /// Delivery deadline.
    pub expires_at: u64,
    /// The claim fence.
    pub claim_token: String,
}

/// Why a claimed job may or may not be sent now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushDeliveryCheck {
    /// Current authority holds. The lease event decrypts to the endpoint.
    Send {
        /// The currently accepted lease event.
        lease_event: Box<Event>,
        /// The matched event, visible to the lease author.
        event: Box<Event>,
    },
    /// Suppress without revealing why to anyone but the operator.
    Suppress(&'static str),
}

/// The fenced result of one attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushDisposition {
    /// The gateway accepted the wake.
    Delivered,
    /// Authority no longer holds.
    Suppressed(&'static str),
    /// Try again at this Unix second.
    Retry {
        /// Next attempt.
        next_attempt_at: u64,
        /// Operator-facing cause.
        reason: &'static str,
    },
    /// Permanent failure. The dead-letter state.
    Dead(&'static str),
    /// The endpoint is permanently invalid for this lease generation.
    InvalidEndpoint,
}

/// Operator view of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushJobRecord {
    /// Stable UUID.
    pub job_id: String,
    /// Lease author.
    pub author: String,
    /// Lease `d` value.
    pub installation: String,
    /// Lease generation.
    pub generation: u64,
    /// Matched event.
    pub event_id: String,
    /// `pending`, `claimed`, `delivered`, `suppressed`, or `dead`.
    pub state: String,
    /// Attempts so far.
    pub attempts: u32,
    /// Earliest next attempt.
    pub next_attempt_at: u64,
    /// Bounded cause code.
    pub last_error: Option<String>,
}

/// Operator view of a lease address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushLeaseState {
    /// Accepted event.
    pub event_id: String,
    /// Generation watermark.
    pub generation: u64,
    /// False for a tombstone or an expired lease.
    pub active: bool,
    /// Public expiration.
    pub expires_at: u64,
    /// SHA-256 of the endpoint, absent for an inactive lease.
    pub endpoint_hash: Option<String>,
    /// When the transport reported the endpoint permanently invalid.
    pub endpoint_invalid_at: Option<u64>,
    /// When the watermark may be discarded.
    pub retain_until: u64,
}

/// Result of one matcher batch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PushMatchReport {
    /// Events read past the cursor.
    pub events: usize,
    /// Jobs inserted. Existing jobs are not counted.
    pub jobs: usize,
}

/// Validate a lease inside the admission transaction. Returns a refusal
/// reason, without the `invalid:` prefix, when the lease must not commit.
pub(super) async fn check_lease(
    transaction: &Transaction<'_>,
    lease: &PushLeaseWrite,
    now: u64,
) -> Result<Option<&'static str>, StoreError> {
    let now = pg_i64(now, "now")?;
    let generation = pg_i64(lease.generation, "generation")?;
    transaction
        .query_one(LEASE_LOCK_SQL, &[&lease.origin, &lease.author])
        .await?;
    if let Some(row) = transaction
        .query_opt(
            LEASE_ROW_SQL,
            &[&lease.origin, &lease.author, &lease.installation],
        )
        .await?
        && generation <= row.get::<_, i64>(0)
    {
        return Ok(Some("stale generation"));
    }
    if !lease.active {
        return Ok(None);
    }
    transaction
        .execute(RETIRE_EXPIRED_SQL, &[&lease.origin, &lease.author, &now])
        .await?;
    let taken = transaction
        .query_one(
            ENDPOINT_TAKEN_SQL,
            &[
                &lease.origin,
                &lease.author,
                &lease.installation,
                &lease.app_profile,
                &lease.transport,
                &lease.endpoint_hash,
            ],
        )
        .await?
        .get::<_, bool>(0);
    if taken {
        return Ok(Some("endpoint already leased"));
    }
    let others = transaction
        .query_one(
            ACTIVE_OTHERS_SQL,
            &[&lease.origin, &lease.author, &lease.installation],
        )
        .await?
        .get::<_, i64>(0);
    if usize::try_from(others).unwrap_or(usize::MAX) >= lease.max_active_leases {
        return Ok(Some("lease quota exceeded"));
    }
    if let Some(quota) = lease.max_profile_leases {
        let others = transaction
            .query_one(
                ACTIVE_PROFILE_OTHERS_SQL,
                &[
                    &lease.origin,
                    &lease.author,
                    &lease.installation,
                    &lease.app_profile,
                ],
            )
            .await?
            .get::<_, i64>(0);
        if usize::try_from(others).unwrap_or(usize::MAX) >= quota {
            return Ok(Some("lease quota exceeded"));
        }
    }
    Ok(None)
}

/// Commit the effective lease state and watermark with the stored event.
pub(super) async fn write_lease(
    transaction: &Transaction<'_>,
    lease: &PushLeaseWrite,
    event_id: &str,
    created_at: i64,
    now: u64,
) -> Result<(), StoreError> {
    let expires_at = pg_i64(lease.expires_at, "expiration")?;
    let generation = pg_i64(lease.generation, "generation")?;
    let retain = if lease.active {
        lease.expires_at.saturating_add(ALLOWED_SKEW)
    } else {
        lease
            .expires_at
            .max(now.saturating_add(lease.max_lease_ttl))
            .saturating_add(ALLOWED_SKEW)
    };
    let retain = pg_i64(retain, "retention")?;
    let subscriptions = lease
        .subscriptions
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| StoreError::Serialization(error.to_string()))?;
    let params: &[&(dyn ToSql + Sync)] = &[
        &lease.origin,
        &lease.author,
        &lease.installation,
        &event_id,
        &created_at,
        &expires_at,
        &generation,
        &lease.active,
        &lease.app_profile,
        &lease.transport,
        &lease.endpoint_hash,
        &subscriptions,
        &retain,
    ];
    transaction.execute(UPSERT_LEASE_SQL, params).await?;
    let now = pg_i64(now, "now")?;
    transaction
        .execute(
            SUPPRESS_REPLACED_SQL,
            &[
                &lease.origin,
                &lease.author,
                &lease.installation,
                &generation,
                &now,
            ],
        )
        .await?;
    Ok(())
}

impl super::Store {
    /// Admit a `kind:30350` lease and its executor state in one transaction.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails. A lease that loses either
    /// replacement ordering is `Rejected` and leaves all state unchanged.
    pub async fn admit_push_lease(
        &mut self,
        event: &Event,
        now: u64,
        lease: &PushLeaseWrite,
        relay_signer: Option<&crate::domain::RelaySigner>,
    ) -> Result<super::AdmissionOutcome, StoreError> {
        self.admit_inner(
            event,
            now,
            relay_signer,
            None,
            super::AdmissionMode::Public,
            Some(lease),
        )
        .await
    }

    /// Match stored events past the durable cursor against active leases and
    /// insert wake jobs, advancing the cursor in the same transaction.
    ///
    /// `matches` applies the protocol filter and envelope visibility rules.
    /// This method adds the relay's current read authorization for the lease
    /// author. `job_id` must be deterministic for `(lease, event)` so a
    /// retried batch reproduces the same identifiers.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails; the cursor does not move.
    pub async fn push_match(
        &mut self,
        origin: &str,
        now: u64,
        limit: usize,
        job_ttl: u64,
        matches: impl Fn(&Event, &ActivePushLease) -> bool,
        job_id: impl Fn(&ActivePushLease, &Event) -> String,
    ) -> Result<PushMatchReport, StoreError> {
        self.ensure_current()?;
        let statements = self.statements.clone();
        let pg_now = pg_i64(now, "now")?;
        let limit = pg_limit(limit)?;
        let transaction = self.client.transaction().await?;
        transaction.execute(CURSOR_INIT_SQL, &[&origin]).await?;
        let cursor = transaction
            .query_one(CURSOR_LOCK_SQL, &[&origin])
            .await?
            .get::<_, i64>(0);
        let rows = transaction
            .query(EVENTS_AFTER_SQL, &[&cursor, &pg_now, &limit])
            .await?;
        let events = rows
            .into_iter()
            .map(decode_event_row)
            .collect::<Result<Vec<StoredEvent>, _>>()?;
        let mut report = PushMatchReport {
            events: events.len(),
            jobs: 0,
        };
        let Some(last) = events.last().map(|stored| stored.ingest_seq) else {
            transaction.commit().await?;
            return Ok(report);
        };
        let leases = transaction
            .query(ACTIVE_LEASES_SQL, &[&origin, &pg_now])
            .await?
            .into_iter()
            .map(active_lease_row)
            .collect::<Result<Vec<_>, _>>()?;
        for stored in &events {
            for lease in &leases {
                if !matches(&stored.event, lease)
                    || !reader_authorized(
                        &transaction,
                        &statements,
                        &stored.event.id,
                        &lease.author,
                        pg_now,
                    )
                    .await?
                {
                    continue;
                }
                let deadline = lease.expires_at.min(now.saturating_add(job_ttl));
                let deadline = pg_i64(deadline, "job deadline")?;
                let generation = pg_i64(lease.generation, "generation")?;
                let id = job_id(lease, &stored.event);
                let params: &[&(dyn ToSql + Sync)] = &[
                    &id,
                    &origin,
                    &lease.author,
                    &lease.installation,
                    &generation,
                    &lease.app_profile,
                    &lease.transport,
                    &lease.endpoint_hash,
                    &stored.event.id,
                    &pg_now,
                    &deadline,
                ];
                report.jobs += usize::try_from(transaction.execute(INSERT_JOB_SQL, params).await?)
                    .unwrap_or(0);
            }
        }
        transaction
            .execute(CURSOR_ADVANCE_SQL, &[&origin, &last])
            .await?;
        transaction.commit().await?;
        Ok(report)
    }

    /// The highest committed ingest sequence the matcher could read.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails.
    pub async fn push_latest_seq(&self) -> Result<i64, StoreError> {
        self.ensure_current()?;
        Ok(self.client.query_one(LATEST_SEQ_SQL, &[]).await?.get(0))
    }

    /// Claim up to `limit` due jobs for `token` until `now + claim_seconds`.
    /// Jobs past their deadline move to the dead-letter state first. A job
    /// whose claim expired without a fenced finish is claimable again.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails.
    pub async fn claim_push_jobs(
        &mut self,
        origin: &str,
        token: &str,
        now: u64,
        claim_seconds: u64,
        limit: usize,
    ) -> Result<Vec<PushJob>, StoreError> {
        self.ensure_current()?;
        let pg_now = pg_i64(now, "now")?;
        let until = pg_i64(now.saturating_add(claim_seconds), "claim deadline")?;
        let limit = pg_limit(limit)?;
        let transaction = self.client.transaction().await?;
        transaction
            .execute(EXPIRE_JOBS_SQL, &[&origin, &pg_now])
            .await?;
        let rows = transaction
            .query(CLAIM_JOBS_SQL, &[&origin, &token, &pg_now, &until, &limit])
            .await?;
        transaction.commit().await?;
        rows.into_iter()
            .map(|row| {
                Ok(PushJob {
                    job_id: row.get(0),
                    origin: row.get(1),
                    author: row.get(2),
                    installation: row.get(3),
                    generation: unsigned(row.get::<_, i64>(4), "job generation")?,
                    app_profile: row.get(5),
                    transport: row.get(6),
                    endpoint_hash: row.get(7),
                    event_id: row.get(8),
                    attempts: u32::try_from(row.get::<_, i32>(9))
                        .map_err(|_| StoreError::CorruptRow("job attempts".to_owned()))?,
                    expires_at: unsigned(row.get::<_, i64>(10), "job deadline")?,
                    claim_token: token.to_owned(),
                })
            })
            .collect()
    }

    /// Recheck current authority for a claimed job immediately before a send:
    /// the lease is active, unexpired, at the job's generation and endpoint,
    /// and the author may still read the event at this origin.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails.
    pub async fn push_delivery_check(
        &self,
        job: &PushJob,
        now: u64,
    ) -> Result<PushDeliveryCheck, StoreError> {
        self.ensure_current()?;
        if job.expires_at <= now {
            return Ok(PushDeliveryCheck::Suppress("expired"));
        }
        let pg_now = pg_i64(now, "now")?;
        let Some(row) = self
            .client
            .query_opt(
                LEASE_FOR_JOB_SQL,
                &[&job.origin, &job.author, &job.installation],
            )
            .await?
        else {
            return Ok(PushDeliveryCheck::Suppress("lease_missing"));
        };
        let lease_event_id = row.get::<_, String>(0);
        let generation = unsigned(row.get::<_, i64>(1), "lease generation")?;
        let active = row.get::<_, bool>(2);
        let expires_at = unsigned(row.get::<_, i64>(3), "lease expiration")?;
        let endpoint_hash = row.get::<_, Option<String>>(4);
        let invalid = row.get::<_, Option<i64>>(5).is_some();
        if !active {
            return Ok(PushDeliveryCheck::Suppress("lease_revoked"));
        }
        if generation != job.generation {
            return Ok(PushDeliveryCheck::Suppress("lease_replaced"));
        }
        if expires_at <= now {
            return Ok(PushDeliveryCheck::Suppress("lease_expired"));
        }
        if invalid || endpoint_hash.as_deref() != Some(job.endpoint_hash.as_str()) {
            return Ok(PushDeliveryCheck::Suppress("endpoint_invalid"));
        }
        let statements = self.statements.clone();
        let readers = Some(vec![job.author.clone()]);
        let ids = Some(vec![job.event_id.clone()]);
        let none_text: Option<Vec<String>> = None;
        let none_kinds: Option<Vec<i32>> = None;
        let none_i64: Option<i64> = None;
        let limit = 1_i64;
        let params: &[&(dyn ToSql + Sync)] = &[
            &ids,
            &none_text,
            &none_kinds,
            &none_i64,
            &none_i64,
            &"{}",
            &pg_now,
            &limit,
            &i64::MAX,
            &readers,
            &none_text,
        ];
        let Some(event_row) = self
            .client
            .query_opt(&statements.query_filter, params)
            .await?
        else {
            return Ok(PushDeliveryCheck::Suppress("not_authorized"));
        };
        let policy = self
            .client
            .query_one(READER_POLICY_SQL, &[&job.author])
            .await?;
        if !reader_policy_allows(&policy) {
            return Ok(PushDeliveryCheck::Suppress("not_authorized"));
        }
        let event = decode_event_row(event_row)?.event;
        let Some(lease_row) = self
            .client
            .query_opt(&statements.event_by_id, &[&lease_event_id, &pg_now])
            .await?
        else {
            return Ok(PushDeliveryCheck::Suppress("lease_missing"));
        };
        let lease_event = decode_event_row(lease_row)?.event;
        Ok(PushDeliveryCheck::Send {
            lease_event: Box::new(lease_event),
            event: Box::new(event),
        })
    }

    /// Record the outcome of a claimed attempt. Returns `false` when the
    /// claim no longer holds, for example after a lease replacement.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails.
    pub async fn finish_push_job(
        &mut self,
        job: &PushJob,
        disposition: PushDisposition,
        now: u64,
    ) -> Result<bool, StoreError> {
        self.ensure_current()?;
        let pg_now = pg_i64(now, "now")?;
        let (state, error, next): (&str, Option<&str>, Option<i64>) = match disposition {
            PushDisposition::Delivered => ("delivered", None, None),
            PushDisposition::Suppressed(reason) => ("suppressed", Some(reason), None),
            PushDisposition::Retry {
                next_attempt_at,
                reason,
            } => (
                "pending",
                Some(reason),
                Some(pg_i64(next_attempt_at, "next attempt")?),
            ),
            PushDisposition::Dead(reason) => ("dead", Some(reason), None),
            PushDisposition::InvalidEndpoint => ("dead", Some("endpoint_invalid"), None),
        };
        let transaction = self.client.transaction().await?;
        let updated = transaction
            .execute(
                FINISH_JOB_SQL,
                &[
                    &job.job_id,
                    &job.claim_token,
                    &state,
                    &error,
                    &next,
                    &pg_now,
                ],
            )
            .await?;
        if updated == 1 && disposition == PushDisposition::InvalidEndpoint {
            let generation = pg_i64(job.generation, "generation")?;
            let params: &[&(dyn ToSql + Sync)] = &[
                &job.origin,
                &job.author,
                &job.installation,
                &generation,
                &pg_now,
            ];
            transaction.execute(INVALIDATE_ENDPOINT_SQL, params).await?;
            transaction.execute(SUPPRESS_GENERATION_SQL, params).await?;
        }
        transaction.commit().await?;
        Ok(updated == 1)
    }

    /// Jobs at `origin`, oldest first, for operators and tests.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails.
    pub async fn push_jobs(
        &self,
        origin: &str,
        limit: usize,
    ) -> Result<Vec<PushJobRecord>, StoreError> {
        self.ensure_current()?;
        let limit = pg_limit(limit)?;
        self.client
            .query(JOBS_SQL, &[&origin, &limit])
            .await?
            .into_iter()
            .map(|row| {
                Ok(PushJobRecord {
                    job_id: row.get(0),
                    author: row.get(1),
                    installation: row.get(2),
                    generation: unsigned(row.get::<_, i64>(3), "job generation")?,
                    event_id: row.get(4),
                    state: row.get(5),
                    attempts: u32::try_from(row.get::<_, i32>(6))
                        .map_err(|_| StoreError::CorruptRow("job attempts".to_owned()))?,
                    next_attempt_at: unsigned(row.get::<_, i64>(7), "next attempt")?,
                    last_error: row.get(8),
                })
            })
            .collect()
    }

    /// The effective state and watermark for one lease address.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails.
    pub async fn push_lease_state(
        &self,
        origin: &str,
        author: &str,
        installation: &str,
    ) -> Result<Option<PushLeaseState>, StoreError> {
        self.ensure_current()?;
        self.client
            .query_opt(LEASE_STATE_SQL, &[&origin, &author, &installation])
            .await?
            .map(|row| {
                Ok(PushLeaseState {
                    event_id: row.get(0),
                    generation: unsigned(row.get::<_, i64>(1), "lease generation")?,
                    active: row.get(2),
                    expires_at: unsigned(row.get::<_, i64>(3), "lease expiration")?,
                    endpoint_hash: row.get(4),
                    endpoint_invalid_at: row
                        .get::<_, Option<i64>>(5)
                        .map(|value| unsigned(value, "endpoint invalidation"))
                        .transpose()?,
                    retain_until: unsigned(row.get::<_, i64>(6), "lease retention")?,
                })
            })
            .transpose()
    }

    /// Delete tombstones past their watermark retention and finished jobs
    /// older than `finished_before`.
    ///
    /// # Errors
    ///
    /// Returns a store error when Postgres fails.
    pub async fn sweep_push_state(&self, now: u64, finished_before: u64) -> Result<(), StoreError> {
        self.ensure_current()?;
        let now = pg_i64(now, "now")?;
        let finished_before = pg_i64(finished_before, "retention")?;
        self.client.execute(SWEEP_LEASES_SQL, &[&now]).await?;
        self.client
            .execute(SWEEP_JOBS_SQL, &[&finished_before])
            .await?;
        Ok(())
    }
}

async fn reader_authorized(
    transaction: &Transaction<'_>,
    statements: &Statements,
    event_id: &str,
    author: &str,
    now: i64,
) -> Result<bool, StoreError> {
    let policy = transaction.query_one(READER_POLICY_SQL, &[&author]).await?;
    if !reader_policy_allows(&policy) {
        return Ok(false);
    }
    let readers = Some(vec![author.to_owned()]);
    let ids = Some(vec![event_id.to_owned()]);
    let none_text: Option<Vec<String>> = None;
    let none_kinds: Option<Vec<i32>> = None;
    let none_i64: Option<i64> = None;
    let limit = 1_i64;
    let params: &[&(dyn ToSql + Sync)] = &[
        &ids,
        &none_text,
        &none_kinds,
        &none_i64,
        &none_i64,
        &"{}",
        &now,
        &limit,
        &i64::MAX,
        &readers,
        &none_text,
    ];
    Ok(transaction
        .query_opt(&statements.query_filter, params)
        .await?
        .is_some())
}

fn reader_policy_allows(row: &tokio_postgres::Row) -> bool {
    let closed = row.get::<_, Option<bool>>(0).unwrap_or(true);
    let member = row.get::<_, bool>(1);
    let blocked = row.get::<_, bool>(2);
    !blocked && (!closed || member)
}

fn active_lease_row(row: tokio_postgres::Row) -> Result<ActivePushLease, StoreError> {
    let subscriptions = serde_json::from_str(&row.get::<_, String>(8))
        .map_err(|error| StoreError::CorruptRow(format!("lease subscriptions: {error}")))?;
    Ok(ActivePushLease {
        author: row.get(0),
        installation: row.get(1),
        event_id: row.get(2),
        generation: unsigned(row.get::<_, i64>(3), "lease generation")?,
        expires_at: unsigned(row.get::<_, i64>(4), "lease expiration")?,
        app_profile: row.get(5),
        transport: row.get(6),
        endpoint_hash: row.get(7),
        subscriptions,
    })
}

fn unsigned(value: i64, field: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::CorruptRow(format!("negative {field}")))
}
