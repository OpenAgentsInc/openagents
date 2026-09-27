//! Reverse enrollment for a host without a screen, through the resident
//! host.
//!
//! A headless host can't show an invitation to a camera, so it asks instead:
//! [`request`] records a NIP-HOST enrollment request in the host's access
//! store, publishes one sealed copy to the owner and to each device that
//! holds `access_admin`, and returns the short code for the operator to
//! read out, for example in an SSH session. The request never carries the
//! code.
//!
//! The running `coder host serve` answers the approval or denial on the
//! request's relay, because it serves every NIP-HOST request addressed to
//! its key there. The access store decides the outcome: the code digest,
//! the five-attempt limit, the approver's `access_admin` right and its
//! bounds, and the request's five-minute expiry. [`wait`] reads that
//! outcome from the store; it answers nothing itself, so exactly one
//! process serves the host key.

use std::path::Path;
use std::time::{Duration, Instant};

use coder_access::host::{EnrollmentStatus, Host};
use coder_access::{Code, RelayPolicy, Rights};

use crate::Result;

/// How long an operation waits while another local process, such as the
/// resident host, holds the access store.
const BUSY_WAIT: Duration = Duration::from_secs(5);

/// A published enrollment request.
#[derive(Clone, Debug)]
pub struct Requested {
    /// The request ID, which is also its mailbox.
    pub id: String,
    /// The short code to show on this host only. It is never published.
    pub code: String,
    /// When the request expires, in Unix seconds.
    pub expires_at: u64,
    /// How many recipients the request was sealed to: the owner and each
    /// current administrator.
    pub recipients: usize,
}

/// Records an enrollment request asking for `rights` on `relay`, publishes
/// it, and returns the code to display. The request is durable before it is
/// published, and the code is returned only after both.
///
/// # Errors
/// Refuses a relay the policy forbids, a full request table, or a relay
/// that did not accept the request.
pub async fn request(
    state: &Path,
    policy: RelayPolicy,
    relay: &str,
    rights: Rights,
) -> Result<Requested> {
    let host = Host::new(state, policy);
    let pending =
        busy_retry(|| host.request_enrollment(relay, rights.clone(), coder_access::unix_time()?))?;
    host.publish(relay, &pending.events).await?;
    Ok(Requested {
        id: pending.id,
        code: pending.code,
        expires_at: pending.expires_at,
        recipients: pending.events.len(),
    })
}

/// The request's current outcome in the access store.
///
/// # Errors
/// Refuses a request the store no longer retains.
pub fn status(state: &Path, policy: RelayPolicy, id: &str) -> Result<EnrollmentStatus> {
    let host = Host::new(state, policy);
    Ok(busy_retry(|| {
        host.enrollment_status(id, coder_access::unix_time()?)
    })?)
}

/// Waits until the request is approved, denied, closed after wrong codes,
/// or expired, checking the store every `every`, and returns that outcome.
///
/// # Errors
/// Refuses a request the store no longer retains.
pub async fn wait(
    state: &Path,
    policy: RelayPolicy,
    id: &str,
    every: Duration,
) -> Result<EnrollmentStatus> {
    loop {
        match status(state, policy, id)? {
            EnrollmentStatus::Pending => tokio::time::sleep(every).await,
            decided => return Ok(decided),
        }
    }
}

fn busy_retry<T>(
    mut operation: impl FnMut() -> coder_access::Result<T>,
) -> coder_access::Result<T> {
    let started = Instant::now();
    loop {
        match operation() {
            Err(error) if error.code == Code::Conflict && started.elapsed() < BUSY_WAIT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

/// A short description of an outcome, for operator output.
#[must_use]
pub fn describe(status: &EnrollmentStatus) -> &'static str {
    match status {
        EnrollmentStatus::Pending => "pending",
        EnrollmentStatus::Approved { .. } => "approved",
        EnrollmentStatus::Denied => "denied",
        EnrollmentStatus::Closed => "closed after too many wrong codes",
        EnrollmentStatus::Expired => "expired",
    }
}
