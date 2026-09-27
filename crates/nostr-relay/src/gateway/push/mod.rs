//! NIP-PL push executor.
//!
//! Lease acceptance decrypts the lease under the executor key, validates it,
//! and commits the event together with its effective state and generation
//! watermark. A background worker matches newly stored events against active
//! leases from a durable cursor, inserts one idempotent wake job per
//! `(origin, app profile, transport, endpoint hash, event)`, and delivers due
//! jobs through a [`WakeTransport`]. Before every send the worker rechecks
//! the lease's current generation, expiry, and endpoint, and the author's
//! current read access to the event. Transient failures retry with bounded
//! exponential backoff; exhausted, expired, and permanently refused jobs
//! move to the dead-letter state.
//!
//! A wake carries no event or lease content; see [`transport`].

pub mod transport;

use std::{collections::HashSet, sync::Arc, time::Duration};

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha256};
use tokio::sync::{Notify, watch};

use crate::{
    domain::Event,
    store::{
        ActivePushLease, PushDeliveryCheck, PushDisposition, PushJob, PushLeaseWrite,
        PushMatchReport, Store, StoreError,
    },
};
use nostr::push_lease::{
    self, AcceptedLease, LeaseLimits, PushDescriptor, endpoint_digest, subscriptions_from_json,
};

pub use transport::{
    ApnsGateway, FcmGateway, Platform, TestTransport, WakeFuture, WakeOutcome, WakeRequest,
    WakeTransport,
};

pub(crate) use super::server::unix_now;
use super::subscription::event_visible_to_reader;

/// Kinds a lease may name unless configured otherwise. `3188` carries
/// activity summaries to enrolled devices.
pub const DEFAULT_PUSH_KINDS: [u16; 5] = [1, 7, 9, 1_059, 3_188];
/// Longest a wake job stays deliverable after its match, in seconds.
pub const WAKE_TTL_SECONDS: u64 = 900;
/// How long one claim fences a job, in seconds. Longer than a transport's
/// own timeout, so a live worker finishes before another may reclaim.
pub const CLAIM_SECONDS: u64 = 60;
/// How long finished jobs remain for operators, in seconds.
pub const FINISHED_RETENTION_SECONDS: u64 = 604_800;
const MATCH_BATCH: usize = 256;
const DELIVERY_BATCH: usize = 32;
const SWEEP_INTERVAL_SECONDS: u64 = 300;

/// Bounded retry for transient transport failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Attempts before the dead-letter state, including the first.
    pub max_attempts: u32,
    /// Delay after the first failed attempt, in seconds.
    pub base_delay_seconds: u64,
    /// Ceiling on any delay, in seconds.
    pub max_delay_seconds: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay_seconds: 10,
            max_delay_seconds: 600,
        }
    }
}

impl RetryPolicy {
    /// The recorded disposition for `outcome` after `attempts` attempts.
    #[must_use]
    pub fn disposition(&self, attempts: u32, outcome: WakeOutcome, now: u64) -> PushDisposition {
        match outcome {
            WakeOutcome::Accepted => PushDisposition::Delivered,
            WakeOutcome::InvalidEndpoint => PushDisposition::InvalidEndpoint,
            WakeOutcome::Rejected(reason) => PushDisposition::Dead(reason),
            WakeOutcome::Retry {
                after_seconds,
                reason,
            } => {
                if attempts >= self.max_attempts {
                    return PushDisposition::Dead("retries_exhausted");
                }
                let exponent = attempts.saturating_sub(1).min(20);
                let backoff = self
                    .base_delay_seconds
                    .saturating_mul(1_u64 << exponent)
                    .min(self.max_delay_seconds);
                let delay = after_seconds
                    .unwrap_or(0)
                    .min(self.max_delay_seconds)
                    .max(backoff);
                PushDisposition::Retry {
                    next_attempt_at: now.saturating_add(delay),
                    reason,
                }
            }
        }
    }

    /// Check the policy's bounds.
    ///
    /// # Errors
    ///
    /// Returns a reason for a policy outside its bounds.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=20).contains(&self.max_attempts) {
            return Err("NOSTR_RELAY_PUSH_MAX_ATTEMPTS must be between 1 and 20");
        }
        if self.base_delay_seconds == 0
            || self.max_delay_seconds < self.base_delay_seconds
            || self.max_delay_seconds > 86_400
        {
            return Err(
                "NOSTR_RELAY_PUSH_RETRY_BASE_SECONDS must be positive and at most NOSTR_RELAY_PUSH_RETRY_MAX_SECONDS, which is at most 86400",
            );
        }
        Ok(())
    }
}

/// A configured NIP-PL executor.
#[derive(Clone)]
pub struct PushExecutor {
    /// Decrypts lease content. Never advertised or logged.
    pub secret: SecretKey,
    /// Advertised encryption pubkey.
    pub pubkey: String,
    /// Advertised key ID; leases name it in `exec`.
    pub key_id: String,
    /// Canonical origin, byte for byte the relay URL.
    pub origin: String,
    /// The one application profile this executor serves.
    pub app_profile: String,
    /// Kinds a lease may name.
    pub push_kinds: Vec<u16>,
    /// Advertised lease limits.
    pub limits: LeaseLimits,
    /// Retry bounds.
    pub retry: RetryPolicy,
    /// The platform transport. Its platform is the profile's transport.
    pub transport: Arc<dyn WakeTransport>,
}

impl PushExecutor {
    /// An executor with default kinds, limits, and retry policy.
    #[must_use]
    pub fn new(
        secret: SecretKey,
        origin: String,
        app_profile: String,
        transport: Arc<dyn WakeTransport>,
    ) -> Self {
        let pubkey = Keypair::from_secret_key(&Secp256k1::new(), &secret)
            .x_only_public_key()
            .0
            .to_string();
        Self {
            secret,
            pubkey,
            key_id: "current".to_owned(),
            origin,
            app_profile,
            push_kinds: DEFAULT_PUSH_KINDS.to_vec(),
            limits: LeaseLimits::default(),
            retry: RetryPolicy::default(),
            transport,
        }
    }

    /// The descriptor that NIP-11 advertises and leases are checked against.
    #[must_use]
    pub fn descriptor(&self) -> PushDescriptor {
        PushDescriptor {
            origin: self.origin.clone(),
            key_id: self.key_id.clone(),
            pubkey: self.pubkey.clone(),
            app_profile: self.app_profile.clone(),
            transport: self.transport.platform().as_str().to_owned(),
            push_kinds: self.push_kinds.clone(),
            limits: self.limits.clone(),
        }
    }

    /// Refuse an executor that cannot serve conforming leases.
    ///
    /// # Errors
    ///
    /// Returns a configuration reason.
    pub fn validate(&self) -> Result<(), String> {
        let derived = Keypair::from_secret_key(&Secp256k1::new(), &self.secret)
            .x_only_public_key()
            .0
            .to_string();
        if derived != self.pubkey {
            return Err("the push executor pubkey does not match its secret".to_owned());
        }
        if self.app_profile.is_empty() || self.app_profile.len() > self.limits.max_string_len {
            return Err("NOSTR_RELAY_PUSH_APP_PROFILE must be 1 to 512 bytes".to_owned());
        }
        push_lease::validate_descriptor(&self.descriptor())
            .map_err(|reason| format!("the push descriptor is not valid: {reason}"))?;
        self.retry.validate().map_err(str::to_owned)
    }

    fn open(&self, event: &Event) -> Result<String, String> {
        let author = xonly(&event.pubkey)?;
        push_lease::open_lease(&event.content, &self.secret, &author)
            .map_err(|_| "undecryptable content".to_owned())
    }
}

/// Decrypt and validate a lease, returning the state to commit with it.
///
/// Checks that must see other leases (generation watermark, endpoint
/// uniqueness, and quota) run inside the admission transaction.
///
/// # Errors
///
/// Returns the NIP-PL reason without the `invalid:` prefix.
pub fn lease_write(
    executor: &PushExecutor,
    event: &Event,
    now: u64,
) -> Result<PushLeaseWrite, String> {
    if event.content.len() > executor.limits.max_content_len {
        return Err("content too large".to_owned());
    }
    let installation = event.tag_values("d").next().unwrap_or_default();
    if installation.is_empty() || installation.len() > 64 {
        return Err("d must be 1 to 64 bytes".to_owned());
    }
    let plaintext = executor.open(event)?;
    let accepted =
        push_lease::accept_lease(event, &plaintext, now, &executor.descriptor(), None, &[], 0)?;
    let subscriptions = accepted.active.then(|| {
        serde_json::Value::Array(
            accepted
                .subscriptions
                .iter()
                .map(push_lease::LeaseSubscription::to_json)
                .collect(),
        )
    });
    Ok(PushLeaseWrite {
        origin: executor.origin.clone(),
        author: accepted.author,
        installation: accepted.installation,
        generation: accepted.generation,
        active: accepted.active,
        expires_at: accepted.expiration,
        app_profile: accepted.active.then(|| executor.app_profile.clone()),
        transport: accepted
            .active
            .then(|| executor.transport.platform().as_str().to_owned()),
        endpoint_hash: accepted.endpoint.as_deref().map(endpoint_digest),
        subscriptions,
        max_active_leases: executor.limits.max_leases_per_pubkey,
        max_lease_ttl: executor.limits.max_lease_ttl,
    })
}

/// The job UUID for one `(lease endpoint, event)` pair at this origin. It
/// is keyed by the executor secret, so it is stable across restarts and
/// retries but reveals neither the endpoint nor the event.
#[must_use]
pub fn job_id(executor: &PushExecutor, lease: &ActivePushLease, event_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"openagents.nip-pl.job.v1");
    hasher.update(executor.secret.secret_bytes());
    for part in [
        executor.origin.as_str(),
        lease.app_profile.as_str(),
        lease.transport.as_str(),
        lease.endpoint_hash.as_str(),
        event_id,
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let mut bytes: [u8; 16] = hasher.finalize()[..16]
        .try_into()
        .expect("SHA-256 has at least 16 bytes");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Match one batch of stored events past the cursor.
///
/// # Errors
///
/// Returns a store error when Postgres fails; the cursor does not move.
pub async fn match_events(
    store: &mut Store,
    executor: &PushExecutor,
    now: u64,
) -> Result<PushMatchReport, StoreError> {
    store
        .push_match(
            &executor.origin,
            now,
            MATCH_BATCH,
            WAKE_TTL_SECONDS,
            |event, lease| lease_matches(executor, lease, event, now),
            |lease, event| job_id(executor, lease, &event.id),
        )
        .await
}

/// Counts from one delivery pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliveryReport {
    /// Jobs claimed.
    pub claimed: usize,
    /// Accepted by the transport.
    pub delivered: usize,
    /// Scheduled for another attempt.
    pub retried: usize,
    /// Suppressed because authority no longer holds.
    pub suppressed: usize,
    /// Moved to the dead-letter state.
    pub dead: usize,
}

/// Claim and attempt due jobs once.
///
/// # Errors
///
/// Returns a store error when Postgres fails.
pub async fn deliver_due(
    store: &mut Store,
    executor: &PushExecutor,
    worker: &str,
    now: u64,
) -> Result<DeliveryReport, StoreError> {
    let jobs = store
        .claim_push_jobs(&executor.origin, worker, now, CLAIM_SECONDS, DELIVERY_BATCH)
        .await?;
    let mut report = DeliveryReport {
        claimed: jobs.len(),
        ..DeliveryReport::default()
    };
    for job in jobs {
        let disposition = attempt(store, executor, &job, now).await?;
        match disposition {
            PushDisposition::Delivered => report.delivered += 1,
            PushDisposition::Retry { .. } => report.retried += 1,
            PushDisposition::Suppressed(_) => report.suppressed += 1,
            PushDisposition::Dead(_) | PushDisposition::InvalidEndpoint => report.dead += 1,
        }
        store.finish_push_job(&job, disposition, now).await?;
    }
    Ok(report)
}

async fn attempt(
    store: &Store,
    executor: &PushExecutor,
    job: &PushJob,
    now: u64,
) -> Result<PushDisposition, StoreError> {
    let platform = executor.transport.platform();
    if job.transport != platform.as_str() || job.app_profile != executor.app_profile {
        return Ok(PushDisposition::Suppressed("profile_withdrawn"));
    }
    let (lease_event, event) = match store.push_delivery_check(job, now).await? {
        PushDeliveryCheck::Suppress(reason) => return Ok(PushDisposition::Suppressed(reason)),
        PushDeliveryCheck::Send { lease_event, event } => (lease_event, event),
    };
    if !push_lease::author_may_read(&event, &job.author)
        || !event_visible_to_reader(&event, &HashSet::from([job.author.clone()]))
    {
        return Ok(PushDisposition::Suppressed("not_authorized"));
    }
    let Some(endpoint) = endpoint_of(executor, &lease_event) else {
        return Ok(PushDisposition::Suppressed("lease_unreadable"));
    };
    if endpoint_digest(&endpoint) != job.endpoint_hash {
        return Ok(PushDisposition::Suppressed("endpoint_invalid"));
    }
    let request = WakeRequest {
        request_id: job.job_id.clone(),
        endpoint,
        expires_at: job.expires_at,
    };
    let outcome = executor.transport.deliver(&request).await;
    Ok(executor.retry.disposition(job.attempts, outcome, now))
}

fn endpoint_of(executor: &PushExecutor, lease_event: &Event) -> Option<String> {
    let plaintext = executor.open(lease_event).ok()?;
    let value: serde_json::Value = serde_json::from_str(&plaintext).ok()?;
    value
        .get("endpoint")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn lease_matches(
    executor: &PushExecutor,
    lease: &ActivePushLease,
    event: &Event,
    now: u64,
) -> bool {
    if !executor.push_kinds.contains(&event.kind)
        || lease.app_profile != executor.app_profile
        || lease.transport != executor.transport.platform().as_str()
    {
        return false;
    }
    let Ok(subscriptions) = subscriptions_from_json(&lease.subscriptions) else {
        return false;
    };
    let accepted = AcceptedLease {
        author: lease.author.clone(),
        installation: lease.installation.clone(),
        event_id: lease.event_id.clone(),
        created_at: 0,
        expiration: lease.expires_at,
        generation: lease.generation,
        active: true,
        endpoint: None,
        subscriptions,
    };
    push_lease::lease_matches(&accepted, event, now)
        && push_lease::author_may_read(event, &lease.author)
        && event_visible_to_reader(event, &HashSet::from([lease.author.clone()]))
}

/// Run the matcher and delivery loop until `stop` flips. `wake` shortens
/// the wait after an admission.
///
/// # Errors
///
/// Returns a store error when the dedicated connection is lost.
pub(crate) async fn run_worker(
    mut store: Store,
    executor: PushExecutor,
    worker: String,
    wake: Arc<Notify>,
    mut stop: watch::Receiver<bool>,
) -> Result<(), StoreError> {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_sweep = 0_u64;
    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return Ok(());
                }
            }
            _ = interval.tick() => {}
            () = wake.notified() => {}
        }
        let now = unix_now();
        let pass = async {
            loop {
                let report = match_events(&mut store, &executor, now).await?;
                if report.events < MATCH_BATCH {
                    break;
                }
            }
            loop {
                let report = deliver_due(&mut store, &executor, &worker, now).await?;
                if report.claimed < DELIVERY_BATCH {
                    break;
                }
            }
            if now.saturating_sub(last_sweep) >= SWEEP_INTERVAL_SECONDS {
                store
                    .sweep_push_state(now, now.saturating_sub(FINISHED_RETENTION_SECONDS))
                    .await?;
                last_sweep = now;
            }
            Ok::<(), StoreError>(())
        };
        if let Err(error) = pass.await {
            if !store.is_current() {
                return Err(error);
            }
            eprintln!("push executor: pass failed: {error}");
        }
    }
}

fn xonly(pubkey: &str) -> Result<XOnlyPublicKey, String> {
    pubkey
        .parse::<XOnlyPublicKey>()
        .map_err(|_| "the author public key is not valid".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_back_off_and_end_in_the_dead_letter_state() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_seconds: 10,
            max_delay_seconds: 25,
        };
        let retry = WakeOutcome::Retry {
            after_seconds: None,
            reason: "gateway_unavailable",
        };
        assert_eq!(
            policy.disposition(1, retry, 100),
            PushDisposition::Retry {
                next_attempt_at: 110,
                reason: "gateway_unavailable"
            }
        );
        assert_eq!(
            policy.disposition(2, retry, 100),
            PushDisposition::Retry {
                next_attempt_at: 120,
                reason: "gateway_unavailable"
            }
        );
        let later = WakeOutcome::Retry {
            after_seconds: Some(1_000),
            reason: "provider_retry",
        };
        assert_eq!(
            policy.disposition(1, later, 100),
            PushDisposition::Retry {
                next_attempt_at: 125,
                reason: "provider_retry"
            }
        );
        assert_eq!(
            policy.disposition(3, retry, 100),
            PushDisposition::Dead("retries_exhausted")
        );
        assert_eq!(
            policy.disposition(1, WakeOutcome::Rejected("invalid_grant"), 100),
            PushDisposition::Dead("invalid_grant")
        );
        assert_eq!(
            policy.disposition(1, WakeOutcome::InvalidEndpoint, 100),
            PushDisposition::InvalidEndpoint
        );
        assert!(RetryPolicy::default().validate().is_ok());
        assert!(
            RetryPolicy {
                max_attempts: 0,
                ..RetryPolicy::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn job_ids_are_stable_version_4_uuids() {
        let executor = PushExecutor::new(
            SecretKey::from_byte_array([5; 32]).unwrap(),
            "ws://relay.test".into(),
            "app.test/ios".into(),
            TestTransport::new(Platform::Apns),
        );
        let lease = ActivePushLease {
            author: "a".repeat(64),
            installation: "i".into(),
            event_id: "e".repeat(64),
            generation: 1,
            expires_at: 10,
            app_profile: "app.test/ios".into(),
            transport: "apns".into(),
            endpoint_hash: "0".repeat(64),
            subscriptions: serde_json::json!([]),
        };
        let first = job_id(&executor, &lease, &"1".repeat(64));
        assert_eq!(first, job_id(&executor, &lease, &"1".repeat(64)));
        assert_ne!(first, job_id(&executor, &lease, &"2".repeat(64)));
        assert_eq!(first.len(), 36);
        assert_eq!(&first[14..15], "4");
        assert!(matches!(&first[19..20], "8" | "9" | "a" | "b"));
        assert!(executor.validate().is_ok());
        let mut fcm = executor.clone();
        fcm.transport = TestTransport::new(Platform::Fcm);
        assert_eq!(fcm.descriptor().transport, "fcm");
        assert!(fcm.validate().is_ok());
        let mut empty = executor;
        empty.app_profile.clear();
        assert!(empty.validate().is_err());
    }
}
