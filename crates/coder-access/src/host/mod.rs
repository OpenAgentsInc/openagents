//! The host is the only issuer of access. Every operation checks the current
//! grant record under one private store lock before any effect or disclosure.
use crate::protocol::*;
use crate::{Code, Error, Result, Right, Rights, fail};
use coder_connect::{RelayPolicy, store::Store};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod enroll;
mod serve;
pub use enroll::{EnrollmentStatus, IssuedInvitation, PendingEnrollment};
pub use serve::{serve, serve_once};

const STORE: &str = "access";
const STORE_VERSION: &str = "coder-access.store.v1";
const MAX_GRANTS: usize = 128;
const MAX_INVITATIONS: usize = 64;
const MAX_ENROLLMENTS: usize = 32;
const MAX_REPLIES: usize = 1024;
const MAX_EPOCHS: usize = 1024;
/// Revocation tombstones outlive grant expiry by the request window.
const SKEW: u64 = MAX_REQUEST_LIFETIME;

/// Supplies the effects that other profiles own. `request` is the idempotency
/// key: a retry after an uncertain save calls it again with the same key.
pub trait Dispatch: Send {
    fn dispatch(
        &mut self,
        request: &str,
        device: &str,
        op: &Operation,
    ) -> std::result::Result<Receipt, Code>;
}
/// The default dispatcher: task and terminal effects are not connected.
pub struct Unconnected;
impl Dispatch for Unconnected {
    fn dispatch(&mut self, _: &str, _: &str, _: &Operation) -> std::result::Result<Receipt, Code> {
        Err(Code::Unavailable)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRecord {
    grant: Grant,
    authorization: Event,
    revoked_at: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    request_event: String,
    signer: String,
    expires_at: u64,
    /// `None` while a dispatched effect is uncertain.
    reply: Option<Event>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    host: String,
    owner: String,
    epochs: BTreeMap<String, u64>,
    grants: BTreeMap<String, GrantRecord>,
    invitations: BTreeMap<String, enroll::InvitationRecord>,
    enrollments: BTreeMap<String, enroll::EnrollmentRecord>,
    replies: BTreeMap<String, Retained>,
}
impl Book {
    fn epoch(&self, device: &str) -> u64 {
        self.epochs.get(device).copied().unwrap_or(0)
    }
    fn prune(&mut self, now: u64) {
        self.grants
            .retain(|_, g| g.grant.expires_at.saturating_add(SKEW) > now);
        self.replies.retain(|_, r| r.expires_at > now);
        // A consumed invitation's grant outlives it, so pruning keeps references valid.
        self.invitations
            .retain(|_, i| i.expires_at.saturating_add(SKEW) > now);
        self.enrollments
            .retain(|_, e| e.expires_at.saturating_add(SKEW) > now);
    }
    /// Whether a delegating issuer still holds its rights at this host.
    fn issuer_current(&self, issuer: &str, grant: Option<&str>, rights: &Rights, now: u64) -> bool {
        if issuer == self.host || issuer == self.owner {
            return true;
        }
        grant.and_then(|id| self.grants.get(id)).is_some_and(|r| {
            r.grant.device == issuer
                && r.revoked_at.is_none()
                && r.grant.expires_at > now
                && r.grant.epoch == self.epoch(issuer)
                && r.grant.rights.contains(Right::AccessAdmin)
                && rights.first_missing(&r.grant.rights).is_none()
        })
    }
}

/// The admitted signer of one request.
struct Principal {
    key: String,
    rights: Rights,
    grant: Option<String>,
    expires_at: u64,
}

pub struct Host {
    directory: PathBuf,
    policy: RelayPolicy,
}
impl Host {
    pub fn new(directory: impl Into<PathBuf>, policy: RelayPolicy) -> Self {
        Self {
            directory: directory.into(),
            policy,
        }
    }
    /// Establish the owner locally. An invitation or relay event cannot do this.
    /// Reinitializing with the same owner is a no-op; another owner refuses.
    pub fn init(&self, owner: &str) -> Result<String> {
        public(owner)?;
        ensure_parent(&self.directory)?;
        let mut store = Store::open_named(&self.directory, STORE, true)?;
        let secret = store.key(true)?;
        let host = pubkey(&secret);
        if host == owner {
            return fail(Code::Forbidden, "owner and host keys must differ");
        }
        match store.load::<serde_json::Value>()? {
            Some(_) => {
                let book = self.book(&store, &secret)?;
                if book.owner != owner {
                    return fail(
                        Code::Conflict,
                        "this host already has another owner; create a new store to change it",
                    );
                }
            }
            None => store.save(&Book {
                v: STORE_VERSION.into(),
                host: host.clone(),
                owner: owner.into(),
                epochs: BTreeMap::new(),
                grants: BTreeMap::new(),
                invitations: BTreeMap::new(),
                enrollments: BTreeMap::new(),
                replies: BTreeMap::new(),
            })?,
        }
        Ok(host)
    }
    pub fn public_key(&self) -> Result<String> {
        Ok(pubkey(&self.key()?))
    }
    pub(crate) fn key(&self) -> Result<SecretKey> {
        Ok(Store::open_named(&self.directory, STORE, false)?.key(false)?)
    }
    pub fn policy(&self) -> RelayPolicy {
        self.policy
    }
    pub fn owner(&self) -> Result<String> {
        let (_, _, book) = self.open()?;
        Ok(book.owner)
    }
    fn open(&self) -> Result<(Store, SecretKey, Book)> {
        let store = Store::open_named(&self.directory, STORE, false)?;
        let secret = store.key(false)?;
        let book = self.book(&store, &secret)?;
        Ok((store, secret, book))
    }
    /// Every enrolled grant with its current state.
    pub fn devices(&self, now: u64) -> Result<Vec<DeviceEntry>> {
        let (_, _, book) = self.open()?;
        Ok(devices(&book, now))
    }
    /// Revoke every grant a device holds and advance its epoch. Persisted
    /// before return; later operations and cached retries refuse.
    pub fn revoke(&self, device: &str, now: u64) -> Result<(u64, Vec<String>)> {
        public(device)?;
        let (mut store, _, mut book) = self.open()?;
        let result = revoke(&mut book, device, now)?;
        store.save(&book)?;
        Ok(result)
    }

    pub fn handle(
        &self,
        event: &Event,
        relay: &str,
        now: u64,
        dispatch: &mut dyn Dispatch,
    ) -> Result<Event> {
        self.handle_with_clock(event, relay, || Ok(now), dispatch)
    }
    pub fn handle_current(
        &self,
        event: &Event,
        relay: &str,
        dispatch: &mut dyn Dispatch,
    ) -> Result<Event> {
        self.handle_with_clock(event, relay, crate::unix_time, dispatch)
    }
    /// An `Err` means no signed reply exists: the request was unreadable,
    /// unauthenticated, stale, or its persistence was uncertain.
    pub fn handle_with_clock(
        &self,
        event: &Event,
        relay: &str,
        mut clock: impl FnMut() -> Result<u64>,
        dispatch: &mut dyn Dispatch,
    ) -> Result<Event> {
        self.policy.validate(relay).map_err(Error::from)?;
        let (mut store, secret, mut book) = self.open()?;
        let host = book.host.clone();
        if event.pubkey == host {
            return fail(Code::Forbidden, "the host does not answer itself");
        }
        let request: Request = open(event, &secret, &event.pubkey, &host, REQUEST)?;
        request.validate(self.policy)?;
        let now = clock()?;
        fresh(request.issued_at, request.expires_at, now)?;
        if request.host != host
            || request.relay != relay
            || event.tag_values("h").collect::<Vec<_>>() != [request.request.as_str()]
        {
            return fail(Code::Forbidden, "request host, relay, or mailbox differs");
        }
        book.prune(now);
        let signer = event.pubkey.clone();
        let retained = book.replies.get(&request.request).cloned();
        if retained
            .as_ref()
            .is_some_and(|r| r.request_event != event.id || r.signer != signer)
        {
            let refused = refused(Error::new(Code::Conflict, "request identity reused"));
            return self.seal_reply(&secret, event, &request, refused, clock()?);
        }
        let (result, retain) = if let Operation::Redeem { .. } = &request.op {
            match self.redeem(
                &mut book,
                &secret,
                &request,
                &signer,
                now,
                retained.as_ref(),
            )? {
                Step::Retained(reply) => return Ok(reply),
                Step::Reply(result, retain) => (result, retain),
            }
        } else {
            match principal(&book, &request, &signer, now) {
                Err(error) => (refused(error), false),
                Ok(p) => {
                    if let Some(right) = request.op.required().filter(|r| !p.rights.contains(*r)) {
                        (refused(Error::missing(right)), true)
                    } else if let Some(Retained {
                        reply: Some(reply), ..
                    }) = &retained
                    {
                        // Authority was rechecked above; the retained bytes are current.
                        return Ok(reply.clone());
                    } else {
                        let result = match self.execute(
                            &mut store, &mut book, &secret, &request, event, &p, now, dispatch,
                        )? {
                            Ok(outcome) => ReplyResult::Ok { outcome },
                            Err(error) => refused(error),
                        };
                        (result, true)
                    }
                }
            }
        };
        // A slow operation must not extend the request's freshness window.
        let reply_time = clock()?;
        fresh(request.issued_at, request.expires_at, reply_time)?;
        let reply = self.seal_reply(&secret, event, &request, result, reply_time)?;
        if retain {
            if book.replies.len() >= MAX_REPLIES && !book.replies.contains_key(&request.request) {
                return fail(Code::Bounds, "retained reply limit reached");
            }
            book.replies.insert(
                request.request.clone(),
                Retained {
                    request_event: event.id.clone(),
                    signer,
                    expires_at: request.expires_at,
                    reply: Some(reply.clone()),
                },
            );
            // Consumption, grant, and the exact reply commit together. No reply
            // escapes a failed save; a retry reopens the last committed book.
            store.save(&book)?;
        }
        Ok(reply)
    }

    #[allow(clippy::too_many_arguments)]
    fn execute(
        &self,
        store: &mut Store,
        book: &mut Book,
        secret: &SecretKey,
        request: &Request,
        event: &Event,
        p: &Principal,
        now: u64,
        dispatch: &mut dyn Dispatch,
    ) -> Result<std::result::Result<Outcome, Error>> {
        Ok(match &request.op {
            Operation::Redeem { .. } => Err(Error::new(Code::Malformed, "redeem has no grant")),
            Operation::Approve { .. } | Operation::Deny { .. } => {
                enroll::decide(book, secret, request, p, now)?
            }
            Operation::Invite {
                rights,
                grant_expires_at,
            } => enroll::remote_invite(book, request, p, rights, *grant_expires_at, now),
            Operation::CancelInvite { invitation } => enroll::cancel(book, invitation),
            Operation::ListDevices {} => Ok(Outcome::Devices {
                devices: devices(book, now),
            }),
            Operation::Revoke { device } => {
                revoke(book, device, now).map(|(epoch, grants)| Outcome::Revoked {
                    device: device.clone(),
                    epoch,
                    grants,
                })
            }
            Operation::CreateTask { .. } | Operation::OpenTerminal { .. } => {
                // Record the admitted intent before the effect. A crash after
                // dispatch replays the same idempotency key, never a new one.
                if book.replies.len() >= MAX_REPLIES {
                    return fail(Code::Bounds, "retained reply limit reached");
                }
                book.replies.insert(
                    request.request.clone(),
                    Retained {
                        request_event: event.id.clone(),
                        signer: event.pubkey.clone(),
                        expires_at: request.expires_at,
                        reply: None,
                    },
                );
                store.save(book)?;
                match dispatch.dispatch(&request.request, &p.key, &request.op) {
                    Ok(receipt)
                        if receipt.operation == request.op.name()
                            && !receipt.reference.is_empty()
                            && receipt.reference.len() <= 128 =>
                    {
                        Ok(Outcome::Dispatched { receipt })
                    }
                    Ok(_) => Err(Error::new(
                        Code::Unavailable,
                        "dispatcher receipt is invalid",
                    )),
                    Err(code) => Err(Error::new(code, "dispatcher refused the operation")),
                }
            }
        })
    }

    fn seal_reply(
        &self,
        secret: &SecretKey,
        event: &Event,
        request: &Request,
        result: ReplyResult,
        now: u64,
    ) -> Result<Event> {
        let reply = Reply {
            v: REPLY.into(),
            requires: vec![],
            request: request.request.clone(),
            request_event: event.id.clone(),
            host: pubkey(secret),
            issued_at: now,
            expires_at: request.expires_at,
            result,
        };
        seal(
            &reply,
            REPLY,
            secret,
            &event.pubkey,
            &request.request,
            now,
            request.expires_at,
        )
    }

    fn book(&self, store: &Store, secret: &SecretKey) -> Result<Book> {
        let book: Book = store.load()?.ok_or_else(|| {
            Error::new(Code::Unavailable, "initialize the host with `init` first")
        })?;
        let host = pubkey(secret);
        if book.v != STORE_VERSION
            || book.host != host
            || book.grants.len() > MAX_GRANTS
            || book.invitations.len() > MAX_INVITATIONS
            || book.enrollments.len() > MAX_ENROLLMENTS
            || book.replies.len() > MAX_REPLIES
            || book.epochs.len() > MAX_EPOCHS
        {
            return fail(
                Code::Malformed,
                "host access store identity or bounds differ",
            );
        }
        public(&book.owner)?;
        for (id, record) in &book.grants {
            record.grant.validate(self.policy)?;
            let signed: Grant = open(
                &record.authorization,
                secret,
                &host,
                &record.grant.device,
                GRANT,
            )?;
            if id != &record.grant.grant
                || encoded(&signed)? != encoded(&record.grant)?
                || record.grant.owner != book.owner
                || record.grant.host != host
            {
                return fail(
                    Code::Malformed,
                    "retained grant differs from its signed bytes",
                );
            }
        }
        for (id, retained) in &book.replies {
            if let Some(reply) = &retained.reply
                && (reply.pubkey != host
                    || reply.tag_values("h").collect::<Vec<_>>() != [id.as_str()]
                    || nostr::private_artifact::admit(reply).is_err())
            {
                return fail(
                    Code::Malformed,
                    "retained reply differs from its signed bytes",
                );
            }
        }
        enroll::validate(&book, self.policy)?;
        Ok(book)
    }
}

/// A new reply and whether to retain it, or the exact retained reply bytes.
enum Step {
    Reply(ReplyResult, bool),
    Retained(Event),
}

fn refused(error: Error) -> ReplyResult {
    ReplyResult::Refused {
        code: error.code,
        missing: error.missing,
    }
}

fn principal(
    book: &Book,
    request: &Request,
    signer: &str,
    now: u64,
) -> std::result::Result<Principal, Error> {
    let Some(id) = &request.grant else {
        if signer == book.owner {
            return Ok(Principal {
                key: signer.into(),
                rights: Rights::all(),
                grant: None,
                expires_at: u64::MAX,
            });
        }
        return Err(Error::new(Code::Forbidden, "this key holds no grant"));
    };
    let record = book
        .grants
        .get(id)
        .ok_or_else(|| Error::new(Code::Forbidden, "grant is not admitted at this host"))?;
    if record.grant.device != signer {
        // A copied grant is not a bearer credential.
        return Err(Error::new(
            Code::Forbidden,
            "grant belongs to another device",
        ));
    }
    if record.revoked_at.is_some() {
        return Err(Error::new(Code::Revoked, "grant is revoked"));
    }
    if record.grant.expires_at <= now {
        return Err(Error::new(Code::Expired, "grant has expired"));
    }
    if request.epoch != Some(record.grant.epoch) || record.grant.epoch != book.epoch(signer) {
        return Err(Error::new(Code::Stale, "grant epoch is not current"));
    }
    if request.expires_at > record.grant.expires_at || request.issued_at < record.grant.issued_at {
        return Err(Error::new(Code::Forbidden, "request is outside its grant"));
    }
    Ok(Principal {
        key: signer.into(),
        rights: record.grant.rights.clone(),
        grant: Some(id.clone()),
        expires_at: record.grant.expires_at,
    })
}

fn devices(book: &Book, now: u64) -> Vec<DeviceEntry> {
    book.grants
        .values()
        .map(|r| DeviceEntry {
            device: r.grant.device.clone(),
            grant: r.grant.grant.clone(),
            rights: r.grant.rights.clone(),
            epoch: r.grant.epoch,
            origin: r.grant.origin.kind,
            issued_at: r.grant.issued_at,
            expires_at: r.grant.expires_at,
            state: if r.revoked_at.is_some() {
                DeviceState::Revoked
            } else if r.grant.expires_at <= now || r.grant.epoch != book.epoch(&r.grant.device) {
                DeviceState::Expired
            } else {
                DeviceState::Active
            },
        })
        .collect()
}

fn revoke(
    book: &mut Book,
    device: &str,
    now: u64,
) -> std::result::Result<(u64, Vec<String>), Error> {
    let grants: Vec<String> = book
        .grants
        .values()
        .filter(|r| r.grant.device == device)
        .map(|r| r.grant.grant.clone())
        .collect();
    if grants.is_empty() {
        return Err(Error::new(
            Code::Forbidden,
            "this device holds no retained grant",
        ));
    }
    if !book.epochs.contains_key(device) && book.epochs.len() >= MAX_EPOCHS {
        return Err(Error::new(
            Code::Bounds,
            "revocation epoch retention limit reached",
        ));
    }
    for id in &grants {
        book.grants
            .get_mut(id)
            .expect("listed grant")
            .revoked_at
            .get_or_insert(now);
    }
    let epoch = book.epoch(device) + 1;
    book.epochs.insert(device.into(), epoch);
    // Cached replies for this device can no longer be served.
    book.replies.retain(|_, r| r.signer != device);
    Ok((epoch, grants))
}

/// Issue and seal a new grant. The caller commits it with its reply.
#[allow(clippy::too_many_arguments)]
fn issue(
    book: &mut Book,
    secret: &SecretKey,
    device: &str,
    relay: &str,
    rights: Rights,
    origin: Origin,
    now: u64,
    expires_at: u64,
) -> Result<Event> {
    book.prune(now);
    if book.grants.len() >= MAX_GRANTS {
        return fail(Code::Bounds, "grant retention limit reached");
    }
    let grant = Grant {
        v: GRANT.into(),
        requires: vec![],
        grant: random_id(),
        host: book.host.clone(),
        owner: book.owner.clone(),
        device: device.into(),
        relay: relay.into(),
        rights,
        epoch: book.epoch(device),
        origin,
        issued_at: now,
        expires_at,
    };
    // The caller validated the relay under the host's policy; this checks shape.
    grant.validate(RelayPolicy::LoopbackTest)?;
    let authorization = seal(&grant, GRANT, secret, device, &grant.grant, now, expires_at)?;
    book.grants.insert(
        grant.grant.clone(),
        GrantRecord {
            grant,
            authorization: authorization.clone(),
            revoked_at: None,
        },
    );
    Ok(authorization)
}

pub(crate) fn same_digest(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.as_bytes()
            .iter()
            .zip(b.as_bytes())
            .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

/// Create the private store's parent directory with owner-only permissions.
pub fn ensure_parent(directory: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if let Some(parent) = directory.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|_| {
                Error::new(
                    Code::Unavailable,
                    "cannot create the store's parent directory",
                )
            })?;
    }
    Ok(())
}
