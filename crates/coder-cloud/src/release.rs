//! The user's own Claude credential, released for one turn of one job
//! (BYO-05, `docs/cloud/claude-code-byo.md`).
//!
//! The web server holds the credential in its custody vault and releases it
//! over the authenticated native channel (`cloud.release`) immediately
//! before it sends the effect that starts a turn: a submission, a
//! continuation, or a follow of a paused job. The operator keeps it here, in
//! memory only:
//!
//! 1. An **offer** waits at most [`dto::RELEASE_SECONDS`] for the effect of
//!    the same job from the same device; anything else drops it.
//! 2. The effect **arms** the job's next turn with it. The backend adds it
//!    to that turn's process environment (the private per-job file the
//!    launch script reads and removes) and redacts it from the turn's
//!    events, results, and artifacts.
//! 3. When the turn ends the driver **disarms** the job. A later turn needs
//!    a fresh release; without one a released-credential job does not start
//!    another turn with it, and a new turn runs on the plan login made
//!    inside the computer, one at a time.
//!
//! Nothing here is written to a job record, admission, request journal,
//! archive, checkpoint, or image. The job keeps only the credential type
//! (evidence) and a digest of the owning account, workspace, and membership
//! epoch, so another owner or a later epoch cannot release into it.

use crate::claude::OwnCredential;
use crate::runtime::Credentials;
use coder_access::cloud as dto;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// At most this many offers wait at once; the oldest is dropped.
const MAX_OFFERS: usize = 1024;

/// A released credential waiting for its job's effect.
pub(crate) struct Offer {
    pub device: String,
    pub owner: String,
    pub class: OwnCredential,
    pub credentials: Arc<Credentials>,
    expires_at: u64,
}

static OFFERS: Mutex<BTreeMap<String, Offer>> = Mutex::new(BTreeMap::new());
static TURNS: Mutex<BTreeMap<String, Arc<Credentials>>> = Mutex::new(BTreeMap::new());

fn lock<T>(m: &'static Mutex<T>) -> MutexGuard<'static, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Validate a released value as runtime credentials under its class name.
pub(crate) fn credentials(
    name: &str,
    value: &str,
) -> crate::Result<(OwnCredential, Arc<Credentials>)> {
    let class =
        OwnCredential::from_name(name).ok_or("The released credential class is invalid.")?;
    let credentials = Credentials::from_names(&[name.to_owned()], |_| Some(value.to_owned()))?;
    Ok((class, Arc::new(credentials)))
}

/// Hold a release for `job`'s next effect. Returns when it lapses.
pub(crate) fn offer(
    job: &str,
    device: &str,
    owner: &str,
    class: OwnCredential,
    credentials: Arc<Credentials>,
    now: u64,
) -> u64 {
    let expires_at = now + dto::RELEASE_SECONDS;
    let mut offers = lock(&OFFERS);
    offers.retain(|_, o| o.expires_at > now);
    while offers.len() >= MAX_OFFERS {
        let oldest = offers
            .iter()
            .min_by_key(|(_, o)| o.expires_at)
            .map(|(k, _)| k.clone());
        match oldest {
            Some(key) => offers.remove(&key),
            None => break,
        };
    }
    offers.insert(
        job.to_owned(),
        Offer {
            device: device.to_owned(),
            owner: owner.to_owned(),
            class,
            credentials,
            expires_at,
        },
    );
    expires_at
}

/// Take the job's waiting release, if one is current and came from `device`.
/// A release from another device is dropped, never handed over.
pub(crate) fn claim(job: &str, device: &str, now: u64) -> Option<Offer> {
    let offer = lock(&OFFERS).remove(job)?;
    (offer.device == device && offer.expires_at > now).then_some(offer)
}

/// Bind credentials to the job's next (or current) turn.
pub(crate) fn arm(job: &str, credentials: Arc<Credentials>) {
    lock(&TURNS).insert(job.to_owned(), credentials);
}

/// Whether the job's next turn holds a released credential.
#[must_use]
pub fn armed(job: &str) -> bool {
    lock(&TURNS).contains_key(job)
}

/// End the job's turn: its released credential is gone.
pub fn disarm(job: &str) {
    lock(&TURNS).remove(job);
}

/// The credentials a backend gives this job's current turn: the profile's
/// own, plus the user's released credential while the turn holds one.
#[must_use]
pub fn turn(base: &Credentials, job: &str) -> Credentials {
    match lock(&TURNS).get(job) {
        Some(released) => base.merged(released),
        None => base.clone(),
    }
}

/// The class of a job's current turn when it runs on a released credential
/// (not one the operator profile names).
#[must_use]
pub fn released_class(record: &crate::Record) -> Option<OwnCredential> {
    if !crate::claude_task::applies(&record.spec)
        || crate::claude::sign_in(record.spec.credential_names.iter().map(String::as_str))
            != crate::claude::SignIn::PlanLogin
    {
        return None;
    }
    match crate::claude_task::turn_sign_in(record) {
        crate::claude::SignIn::Own(class) => Some(class),
        crate::claude::SignIn::PlanLogin => None,
    }
}

/// The digest of the account, workspace, and membership epoch that own a
/// released credential.
#[must_use]
pub fn owner_digest(account: &str, workspace: &str, members_epoch: u64) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(
        serde_json::to_vec(&serde_json::json!({
            "schema": "openagents.claude.byo-owner.v1",
            "account": account,
            "workspace": workspace,
            "members_epoch": members_epoch,
        }))
        .expect("owner serializes"),
    );
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
