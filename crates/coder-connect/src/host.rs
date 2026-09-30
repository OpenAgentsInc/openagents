//! Local operator admission and finite disclosure. This host never controls engines.
use crate::{Error, ErrorCode, Result, fail, protocol::*, store::Store};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub const MAX_READS_PER_MINUTE: u32 = 240;
const MAX_REPLIES: usize = 256;
/// The book retains at most this many grants, live or revoked.
const MAX_ADMISSIONS: usize = 64;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
mod log;
mod pairing;
use log::{Answer, Binding, Reads};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Root {
    scope: SourceScope,
    path: PathBuf,
    device: u64,
    inode: u64,
}
impl Root {
    fn admit(path: PathBuf, kind: SourceKind) -> Result<Self> {
        let canonical = path.canonicalize().map_err(|_| {
            Error::new(
                ErrorCode::Unavailable,
                "selected history root is unavailable",
            )
        })?;
        let (metadata, (device, inode)) = root_identity(&canonical).map_err(|_| {
            Error::new(
                ErrorCode::Unavailable,
                "selected history root is unavailable",
            )
        })?;
        if !metadata.is_dir() {
            return fail(
                ErrorCode::Forbidden,
                "selected history root is not a directory",
            );
        }
        let label = match kind {
            SourceKind::Codex => "Codex retained history",
            SourceKind::Claude => "Claude retained history",
            SourceKind::Coder => "Coder task history",
            SourceKind::OpenCode => "OpenCode session history",
            SourceKind::Devin => "Devin session history",
        };
        Ok(Self {
            scope: SourceScope {
                id: random_id(),
                label: label.into(),
                kind,
            },
            path: canonical,
            device,
            inode,
        })
    }
    fn current(&self) -> Result<()> {
        let (m, id) = root_identity(&self.path).map_err(|_| {
            Error::new(
                ErrorCode::SourceChanged,
                "admitted history root is unavailable",
            )
        })?;
        if !m.is_dir()
            || id != (self.device, self.inode)
            || self.path.canonicalize().ok().as_deref() != Some(self.path.as_path())
        {
            return fail(ErrorCode::SourceChanged, "admitted history root changed");
        }
        Ok(())
    }
}
/// A history root's metadata, not followed through a link, and its
/// identity: device and inode, or on Windows volume and file index.
#[cfg(unix)]
fn root_identity(path: &Path) -> std::io::Result<(std::fs::Metadata, (u64, u64))> {
    let m = std::fs::symlink_metadata(path)?;
    let id = (m.dev(), m.ino());
    Ok((m, id))
}
/// A history root's metadata, not followed through a link, and its
/// identity: device and inode, or on Windows volume and file index.
#[cfg(windows)]
fn root_identity(path: &Path) -> std::io::Result<(std::fs::Metadata, (u64, u64))> {
    let (id, m) = private_fs::identity_of(path)?;
    Ok((m, (u64::from(id.volume), id.index)))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedReply {
    request_event: String,
    expires_at: u64,
    event: Event,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    grant: Grant,
    authorization: Event,
    roots: Vec<Root>,
    revoked_at: Option<u64>,
    window_start: u64,
    reads: u32,
    replies: BTreeMap<String, RetainedReply>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    host: String,
    admissions: BTreeMap<String, Admission>,
    #[serde(default)]
    invitations: BTreeMap<String, pairing::RetainedInvitation>,
}

/// Each call opens a short exclusive local lock, allowing concurrent CLI revocation.
#[derive(Clone)]
pub struct Host {
    directory: PathBuf,
    policy: RelayPolicy,
    /// Serve only a grant's Coder task roots ([`Host::coder_only`]).
    coder_only: bool,
}
impl Host {
    pub fn new(directory: impl Into<PathBuf>, policy: RelayPolicy) -> Self {
        Self {
            directory: directory.into(),
            policy,
            coder_only: false,
        }
    }
    /// Serve only Coder task chats: every read uses only the grant's Coder
    /// roots, so a grant made when the host also offered Codex, Claude,
    /// OpenCode, or Devin history lists only its Coder chats (none when it
    /// names no Coder root), and a read of any other source is refused as
    /// missing. `coder host` serves this way
    /// ([#9920](https://github.com/OpenAgentsInc/openagents/issues/9920)).
    #[must_use]
    pub fn coder_only(mut self) -> Self {
        self.coder_only = true;
        self
    }
    /// The roots of a grant this host reads.
    fn served<'a>(&self, roots: &'a [Root]) -> Vec<&'a Root> {
        roots
            .iter()
            .filter(|root| !self.coder_only || root.scope.kind == SourceKind::Coder)
            .collect()
    }
    /// The host key, read from the store once per process.
    pub fn key(&self) -> Result<SecretKey> {
        let gate = self.gate();
        let mut reads = lock(&gate);
        if let Some(secret) = reads.secret {
            return Ok(secret);
        }
        let secret = Store::open(&self.directory, false)?.key(false)?;
        reads.secret = Some(secret);
        Ok(secret)
    }
    pub fn pair(
        &self,
        client: &str,
        relay: &str,
        config: coder_history::Config,
        now: u64,
        expires_at: u64,
    ) -> Result<ConnectionCode> {
        self.policy.validate(relay)?;
        let gate = self.gate();
        let _serial = gate.lock().unwrap_or_else(|poison| poison.into_inner());
        window(now, expires_at, MAX_GRANT_LIFETIME)?;
        let mut roots = Vec::new();
        if let Some(path) = config.codex {
            roots.push(Root::admit(path, SourceKind::Codex)?);
        }
        if let Some(path) = config.claude {
            roots.push(Root::admit(path, SourceKind::Claude)?);
        }
        if let Some(path) = config.coder {
            roots.push(Root::admit(path, SourceKind::Coder)?);
        }
        if let Some(path) = config.opencode {
            roots.push(Root::admit(path, SourceKind::OpenCode)?);
        }
        if let Some(path) = config.devin {
            roots.push(Root::admit(path, SourceKind::Devin)?);
        }
        if roots.is_empty() {
            return fail(
                ErrorCode::Forbidden,
                "pairing requires an explicitly selected source root",
            );
        }
        let mut store = Store::open(&self.directory, true)?;
        let secret = store.key(true)?;
        let mut book = self.book(&store, &secret, true)?;
        book.invitations
            .retain(|_, i| i.expires_at.saturating_add(MAX_REQUEST_LIFETIME) > now);
        book.admissions
            .retain(|_, a| a.grant.expires_at.saturating_add(MAX_REQUEST_LIFETIME) > now);
        supersede(&mut book.admissions, &book.invitations, client, now)?;
        let grant = Grant {
            v: GRANT.into(),
            requires: vec![],
            grant: random_id(),
            host: pubkey(&secret),
            client: client.into(),
            relay: relay.into(),
            sources: roots.iter().map(|r| r.scope.clone()).collect(),
            issued_at: now,
            expires_at,
        };
        grant.validate(self.policy)?;
        let authorization = seal(
            &grant,
            GRANT,
            &secret,
            client,
            &grant.grant,
            now,
            expires_at,
        )?;
        let code = ConnectionCode {
            v: CONNECTION.into(),
            requires: vec![],
            host: grant.host.clone(),
            client: client.into(),
            relay: relay.into(),
            grant: grant.grant.clone(),
            sources: grant.sources.clone(),
            expires_at,
            authorization: authorization.clone(),
        };
        book.admissions.insert(
            grant.grant.clone(),
            Admission {
                grant,
                authorization,
                roots,
                revoked_at: None,
                window_start: now,
                reads: 0,
                replies: BTreeMap::new(),
            },
        );
        self.save(&mut store, &book)?;
        Ok(code)
    }
    pub fn revoke(&self, grant: &str, source: Option<&str>, now: u64) -> Result<()> {
        let gate = self.gate();
        let _serial = gate.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let mut book = self.book(&store, &secret, false)?;
        let admission = book
            .admissions
            .get_mut(grant)
            .ok_or_else(|| Error::new(ErrorCode::Unavailable, "observer grant not retained"))?;
        if source.is_some_and(|id| !admission.roots.iter().any(|r| r.scope.id == id)) {
            return fail(ErrorCode::Forbidden, "source is not part of this grant");
        }
        admission.revoked_at.get_or_insert(now);
        // Previously encrypted replies can no longer be served by this host.
        admission.replies.clear();
        self.save(&mut store, &book)
    }
    pub fn relays(&self, now: u64) -> Result<Vec<String>> {
        let gate = self.gate();
        let _serial = gate.lock().unwrap_or_else(|poison| poison.into_inner());
        let store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let book = self.book(&store, &secret, false)?;
        let mut relays: Vec<_> = book
            .admissions
            .values()
            .filter(|a| a.revoked_at.is_none() && a.grant.expires_at > now)
            .map(|a| a.grant.relay.clone())
            .chain(
                book.invitations
                    .values()
                    .filter(|i| !i.cancelled && i.expires_at > now)
                    .map(|i| i.relay.clone()),
            )
            .collect();
        relays.sort();
        relays.dedup();
        Ok(relays)
    }
    pub fn handle(&self, event: &Event, relay: &str, now: u64) -> Result<Event> {
        if self.is_pairing(event)? {
            return self.redeem_with_clock(event, relay, || Ok(now));
        }
        self.handle_with_clock(event, relay, || Ok(now))
    }
    /// Recheck the clock after the bounded source read, while admission is locked.
    pub fn handle_current(&self, event: &Event, relay: &str) -> Result<Event> {
        if self.is_pairing(event)? {
            return self.redeem_with_clock(event, relay, crate::unix_time);
        }
        self.handle_with_clock(event, relay, crate::unix_time)
    }
    pub(crate) fn handle_with_clock(
        &self,
        event: &Event,
        relay: &str,
        clock: impl FnMut() -> Result<u64>,
    ) -> Result<Event> {
        self.handle_via(event, Via::Relay(relay), clock)
            .map(|handled| handled.reply)
    }
    /// Answer a sealed observation request that arrived on a direct tailnet
    /// connection rather than through the grant's relay. Every check is the
    /// same as [`Host::handle_current`] except the relay binding, and a
    /// reply may be as large as [`Route::Direct`] allows. A pairing
    /// redemption is refused here: it travels only through a relay.
    pub fn handle_direct(&self, event: &Event) -> Result<Handled> {
        if self.is_pairing(event)? {
            return fail(
                ErrorCode::Forbidden,
                "pairing redemption travels only through its relay",
            );
        }
        self.handle_via(event, Via::Direct, crate::unix_time)
    }
    pub(crate) fn handle_via(
        &self,
        event: &Event,
        via: Via<'_>,
        mut clock: impl FnMut() -> Result<u64>,
    ) -> Result<Handled> {
        let route = match via {
            Via::Relay(relay) => {
                self.policy.validate(relay)?;
                Route::Relay
            }
            Via::Direct => Route::Direct,
        };
        let secret = self.key()?;
        let host = pubkey(&secret);
        let request: Request = open(event, &secret, &event.pubkey, &host, REQUEST)?;
        request.validate_for(route)?;
        let envelope = nostr::private_artifact::open(event, &secret)
            .map_err(|_| Error::new(ErrorCode::Forbidden, "request envelope unavailable"))?;
        if event.tag_values("h").collect::<Vec<_>>() != [request.request.as_str()]
            || envelope.body().issued_at != request.issued_at
            || envelope.body().retain_until != request.expires_at
        {
            return fail(
                ErrorCode::Forbidden,
                "request lifetime or mailbox differs from envelope",
            );
        }
        let now = clock()?;
        fresh(request.issued_at, request.expires_at, now)?;
        // Admission holds this store's gate and lock only to check and bind
        // the request; the read and the signing run outside them, so
        // independent reads run at once.
        let roots = match self.admit(event, &request, via, route, &secret, now)? {
            Admitted::Read(roots) => roots,
            Admitted::Retained(handled) => return Ok(*handled),
            Admitted::Refused(code) => {
                return self
                    .reply(&secret, event, &request, ReplyResult::Refused { code }, now)
                    .map(Handled::from);
            }
        };
        let answered = self.answer(event, &request, route, &secret, &roots, clock);
        let gate = self.gate();
        let mut reads = lock(&gate);
        let (reply, payload, length) = match answered {
            Ok(answered) => answered,
            Err(error) => {
                // No reply exists: this process holds the request no longer.
                reads.bindings.remove(&request.request);
                return Err(error);
            }
        };
        let answer = match &payload {
            Some(payload) => Answer::Direct {
                reply: reply.clone(),
                payload: payload.clone(),
            },
            None => {
                // The exact relay reply is recorded before it is sent.
                let record = Store::open(&self.directory, false).and_then(|store| {
                    let mut memo = memo();
                    let book = self.current(&store, &secret, &mut memo, &mut reads)?;
                    reads.catch_up(store.directory(), book)?;
                    let admission = book.admissions.get(&request.grant);
                    reads.append(
                        store.directory(),
                        &log::Record {
                            request: request.request.clone(),
                            grant: request.grant.clone(),
                            request_event: event.id.clone(),
                            expires_at: request.expires_at,
                            window_start: admission.map_or(0, |a| a.window_start),
                            reads: admission.map_or(0, |a| a.reads),
                            reply: Some(reply.clone()),
                        },
                    )
                });
                if let Err(error) = record {
                    reads.bindings.remove(&request.request);
                    return Err(error);
                }
                Answer::Relay(reply.clone())
            }
        };
        if let Some(binding) = reads.bindings.get_mut(&request.request) {
            binding.answer = answer;
        }
        drop(reads);
        let read = length.map(|length| Read {
            grant: request.grant.clone(),
            query: request.query.clone(),
            sources: sources(&roots),
            length,
        });
        Ok(Handled {
            reply,
            payload,
            read,
        })
    }
    /// Check a request against current admission and bind it to its exact
    /// event, in memory and in the request log, before any read.
    fn admit(
        &self,
        event: &Event,
        request: &Request,
        via: Via<'_>,
        route: Route,
        secret: &SecretKey,
        now: u64,
    ) -> Result<Admitted> {
        let gate = self.gate();
        let mut reads = lock(&gate);
        let store = Store::open(&self.directory, false)?;
        let mut memo = memo();
        let book = self.current(&store, secret, &mut memo, &mut reads)?;
        reads.catch_up(store.directory(), book)?;
        // A binding is held only through its request's lifetime.
        reads.bindings.retain(|_, b| b.expires_at > now);
        let admission = book
            .admissions
            .get_mut(&request.grant)
            .ok_or_else(|| Error::new(ErrorCode::Forbidden, "request has no admitted grant"))?;
        // A relay request binds the grant's exact relay. A direct request
        // arrives on the host's own tailnet listener, which is no relay.
        let relay_differs = match via {
            Via::Relay(relay) => admission.grant.relay != relay,
            Via::Direct => false,
        };
        if admission.grant.client != event.pubkey
            || relay_differs
            || admission.authorization.id != request.authorization
        {
            return fail(
                ErrorCode::Forbidden,
                "request differs from original admitted client, relay, or grant",
            );
        }
        if admission.revoked_at.is_some() {
            return Ok(Admitted::Refused(ErrorCode::Revoked));
        } else if admission.grant.expires_at <= now {
            return Ok(Admitted::Refused(ErrorCode::Expired));
        } else if request.expires_at > admission.grant.expires_at
            || request.issued_at < admission.grant.issued_at
        {
            return Ok(Admitted::Refused(ErrorCode::Forbidden));
        }
        if let Some(old) = reads.bindings.get(&request.request) {
            if old.request_event != event.id {
                return Ok(Admitted::Refused(ErrorCode::Conflict));
            }
            return match (&old.answer, route) {
                (Answer::Relay(reply), _) => Ok(Admitted::Retained(Box::new(reply.clone().into()))),
                (Answer::Direct { reply, payload }, Route::Direct) => {
                    Ok(Admitted::Retained(Box::new(Handled {
                        reply: reply.clone(),
                        payload: Some(payload.clone()),
                        read: None,
                    })))
                }
                // A direct reply's bytes travel only on a direct connection.
                (Answer::Direct { .. }, Route::Relay) => Ok(Admitted::Refused(ErrorCode::Conflict)),
                // Whoever admitted it answers it; no second read, no reply here.
                (Answer::Pending | Answer::Claimed, _) => fail(
                    ErrorCode::Unavailable,
                    "this request is already being answered",
                ),
            };
        }
        if now < admission.window_start {
            return fail(
                ErrorCode::Unavailable,
                "clock moved behind the durable observation window",
            );
        }
        if now - admission.window_start >= 60 {
            admission.window_start = now;
            admission.reads = 0;
        }
        if admission.reads >= MAX_READS_PER_MINUTE {
            return Ok(Admitted::Refused(ErrorCode::RateLimited));
        }
        let grant = &request.grant;
        if reads
            .bindings
            .values()
            .filter(|b| &b.grant == grant)
            .count()
            >= MAX_REPLIES
        {
            return fail(ErrorCode::Bounds, "observer reply retention limit reached");
        }
        admission.reads += 1;
        let record = log::Record {
            request: request.request.clone(),
            grant: grant.clone(),
            request_event: event.id.clone(),
            expires_at: request.expires_at,
            window_start: admission.window_start,
            reads: admission.reads,
            reply: None,
        };
        let roots: Vec<Root> = self.served(&admission.roots).into_iter().cloned().collect();
        if let Err(error) = reads.append(store.directory(), &record) {
            if let Some(admission) = book.admissions.get_mut(grant) {
                admission.reads -= 1;
            }
            return Err(error);
        }
        reads.bindings.insert(
            request.request.clone(),
            Binding {
                grant: grant.clone(),
                request_event: event.id.clone(),
                expires_at: request.expires_at,
                answer: Answer::Pending,
            },
        );
        reads.compact(store.directory(), book, now)?;
        Ok(Admitted::Read(roots))
    }
    /// Read and sign the reply to an admitted request, with no lock held.
    #[allow(clippy::type_complexity)]
    fn answer(
        &self,
        event: &Event,
        request: &Request,
        route: Route,
        secret: &SecretKey,
        roots: &[Root],
        mut clock: impl FnMut() -> Result<u64>,
    ) -> Result<(Event, Option<String>, Option<Option<(String, u64)>>)> {
        let mut answered = None;
        let result = match read(&self.directory, roots, &request.query, route.limits()) {
            Ok(observation) => {
                answered = Some(match &observation {
                    Observation::Page(page) => {
                        Some((page.incarnation.clone(), page.snapshot_bytes))
                    }
                    Observation::Catalog(_) => None,
                });
                ReplyResult::Ok {
                    observation: Box::new(observation),
                }
            }
            Err(error) => ReplyResult::Refused { code: error.code },
        };
        // A slow reader must not extend a request's grant or freshness window.
        let final_now = clock()?;
        fresh(request.issued_at, request.expires_at, final_now)?;
        let (reply, payload) = match route {
            Route::Relay => (self.reply(secret, event, request, result, final_now)?, None),
            Route::Direct => {
                let (reply, payload) =
                    self.reply_detached(secret, event, request, result, final_now)?;
                (reply, Some(payload))
            }
        };
        Ok((reply, payload, answered))
    }
    /// This host's gate within the process, around every use of its store
    /// and around its read state: the relay loop and direct connections
    /// share one store.
    fn gate(&self) -> std::sync::Arc<std::sync::Mutex<Reads>> {
        static GATES: std::sync::OnceLock<
            std::sync::Mutex<
                std::collections::HashMap<PathBuf, std::sync::Arc<std::sync::Mutex<Reads>>>,
            >,
        > = std::sync::OnceLock::new();
        GATES
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .entry(self.directory.clone())
            .or_default()
            .clone()
    }
    /// The validated book in `memo`, read again only when its file changed.
    /// Replies an older book retained move into the read state.
    fn current<'m>(
        &self,
        store: &Store,
        secret: &SecretKey,
        memo: &'m mut std::collections::HashMap<PathBuf, Memo>,
        reads: &mut Reads,
    ) -> Result<&'m mut Book> {
        let stamp = store
            .stamp()
            .ok_or_else(|| Error::new(ErrorCode::Unavailable, "observer store is incomplete"))?;
        let directory = store.directory();
        let known = memo
            .get(directory)
            .is_some_and(|m| m.stamp == stamp && m.book.host == pubkey(secret));
        if !known {
            let mut book = self.load(store, secret, false)?;
            if let Some(old) = memo.get(directory) {
                keep_windows(&old.book, &mut book);
            }
            memo.insert(directory.to_path_buf(), Memo { stamp, book });
        }
        let book = &mut memo
            .get_mut(directory)
            .ok_or_else(|| Error::new(ErrorCode::Unavailable, "observer store is incomplete"))?
            .book;
        for (grant, admission) in &mut book.admissions {
            for (request, retained) in std::mem::take(&mut admission.replies) {
                reads.bindings.entry(request).or_insert(Binding {
                    grant: grant.clone(),
                    request_event: retained.request_event,
                    expires_at: retained.expires_at,
                    answer: Answer::Relay(retained.event),
                });
            }
        }
        Ok(book)
    }
    fn reply(
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
            grant: request.grant.clone(),
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
    /// A reply whose body travels beside its sealed envelope, as a direct
    /// connection carries it: up to [`Route::Direct`]'s body bound.
    fn reply_detached(
        &self,
        secret: &SecretKey,
        event: &Event,
        request: &Request,
        result: ReplyResult,
        now: u64,
    ) -> Result<(Event, String)> {
        let reply = Reply {
            v: REPLY.into(),
            requires: vec![],
            request: request.request.clone(),
            request_event: event.id.clone(),
            grant: request.grant.clone(),
            issued_at: now,
            expires_at: request.expires_at,
            result,
        };
        seal_detached(
            &reply,
            REPLY,
            secret,
            &event.pubkey,
            (&request.request, now, request.expires_at),
            Route::Direct.body(),
        )
    }
    /// Save `book` and remember it as the validated state of the new file.
    fn save(&self, store: &mut Store, book: &Book) -> Result<()> {
        let mut memo = memo();
        memo.remove(store.directory());
        store.save(book)?;
        if let Some(stamp) = store.stamp() {
            memo.insert(
                store.directory().to_path_buf(),
                Memo {
                    stamp,
                    book: book.clone(),
                },
            );
        }
        Ok(())
    }
    /// The store's book, validated. A book this process saved or validated is
    /// reused while its file is unchanged; any other writer's save replaces
    /// the file, which is then read and validated again.
    fn book(&self, store: &Store, secret: &SecretKey, initialize: bool) -> Result<Book> {
        let stamp = store.stamp();
        if let Some(stamp) = stamp
            && let Some(known) = memo().get(store.directory())
            && known.stamp == stamp
            && known.book.host == pubkey(secret)
        {
            return Ok(known.book.clone());
        }
        let mut book = self.load(store, secret, initialize)?;
        if let Some(old) = memo().get(store.directory()) {
            keep_windows(&old.book, &mut book);
        }
        if let Some(stamp) = stamp {
            memo().insert(
                store.directory().to_path_buf(),
                Memo {
                    stamp,
                    book: book.clone(),
                },
            );
        }
        Ok(book)
    }
    fn load(&self, store: &Store, secret: &SecretKey, initialize: bool) -> Result<Book> {
        let book: Book = match store.load()? {
            Some(book) => book,
            None if initialize => Book {
                v: "coder-connect.store.v1".into(),
                host: pubkey(secret),
                admissions: BTreeMap::new(),
                invitations: BTreeMap::new(),
            },
            None => return fail(ErrorCode::Unavailable, "observer store is incomplete"),
        };
        if book.v != "coder-connect.store.v1"
            || book.host != pubkey(secret)
            || book.admissions.len() > MAX_ADMISSIONS
        {
            return fail(
                ErrorCode::Malformed,
                "observer store identity or bounds differ",
            );
        }
        for (id, a) in &book.admissions {
            a.grant.validate(self.policy)?;
            let signed: Grant = open(&a.authorization, secret, &book.host, &a.grant.client, GRANT)?;
            if id != &a.grant.grant
                || encoded(&signed)? != encoded(&a.grant)?
                || a.roots.iter().map(|r| &r.scope).collect::<Vec<_>>()
                    != a.grant.sources.iter().collect::<Vec<_>>()
                || a.replies.len() > MAX_REPLIES
                || a.reads > MAX_READS_PER_MINUTE
            {
                return fail(
                    ErrorCode::Malformed,
                    "retained observer admission differs from signed bytes",
                );
            }
            for (request, retained) in &a.replies {
                let reply: Reply =
                    open(&retained.event, secret, &book.host, &a.grant.client, REPLY)?;
                schema(&reply.v, REPLY, &reply.requires)?;
                if reply.request != *request
                    || reply.request_event != retained.request_event
                    || reply.grant != *id
                    || reply.expires_at != retained.expires_at
                    || retained.event.tag_values("h").collect::<Vec<_>>() != [request.as_str()]
                {
                    return fail(
                        ErrorCode::Malformed,
                        "retained observer reply differs from signed bytes",
                    );
                }
            }
        }
        self.validate_invitations(&book, secret)?;
        Ok(book)
    }
}
/// Make room for a new grant to `client`: that device's earlier grants are
/// superseded, so one device key holds one grant; and while the book is
/// full, the longest-revoked grant leaves it, so a revoked or superseded
/// grant never blocks a new pairing. A grant that a retained invitation
/// names stays in the book, revoked, until the invitation leaves it. A
/// request under a grant that left the book is refused and reads nothing.
///
/// # Errors
/// Refuses with `Bounds` when every retained grant is live.
fn supersede(
    admissions: &mut BTreeMap<String, Admission>,
    invitations: &BTreeMap<String, pairing::RetainedInvitation>,
    client: &str,
    now: u64,
) -> Result<()> {
    let named: std::collections::BTreeSet<&str> = invitations
        .values()
        .filter_map(|i| i.grant.as_deref())
        .collect();
    admissions.retain(|id, a| a.grant.client != client || named.contains(id.as_str()));
    for admission in admissions.values_mut() {
        if admission.grant.client == client {
            admission.revoked_at.get_or_insert(now);
            // Previously encrypted replies can no longer be served by this host.
            admission.replies.clear();
        }
    }
    while admissions.len() >= MAX_ADMISSIONS {
        let oldest = admissions
            .iter()
            .filter(|(id, a)| a.revoked_at.is_some() && !named.contains(id.as_str()))
            .min_by_key(|(_, a)| a.revoked_at)
            .map(|(id, _)| id.clone());
        match oldest {
            Some(id) => {
                admissions.remove(&id);
            }
            None => return fail(ErrorCode::Bounds, "observer grant retention limit reached"),
        }
    }
    Ok(())
}
/// A validated book and the exact file it was read from or saved to.
struct Memo {
    stamp: crate::store::Stamp,
    book: Book,
}
fn memo() -> std::sync::MutexGuard<'static, std::collections::HashMap<PathBuf, Memo>> {
    static MEMO: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, Memo>>> =
        std::sync::OnceLock::new();
    MEMO.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}
/// What a request read, when the host answered it with an observation.
#[derive(Clone, Debug)]
pub struct Read {
    pub grant: String,
    pub query: Query,
    /// The grant's admitted roots.
    pub sources: coder_history::Config,
    /// A transcript page's source incarnation and the length it read.
    pub length: Option<(String, u64)>,
}

/// A signed reply, and what it read when it answered with an observation.
#[derive(Clone, Debug)]
pub struct Handled {
    pub reply: Event,
    /// The reply's body, when it travels beside its envelope
    /// ([`seal_detached`]).
    pub payload: Option<String>,
    pub read: Option<Read>,
}
impl From<Event> for Handled {
    fn from(reply: Event) -> Self {
        Self {
            reply,
            payload: None,
            read: None,
        }
    }
}

/// How a request travelled, for its relay binding.
#[derive(Clone, Copy)]
pub(crate) enum Via<'a> {
    Relay(&'a str),
    Direct,
}

/// What admission decided for a request.
enum Admitted {
    /// Bound to its event: read for it.
    Read(Vec<Root>),
    /// An exact retry: the reply this host keeps.
    Retained(Box<Handled>),
    /// A signed refusal, which binds nothing.
    Refused(ErrorCode),
}

/// Read windows this process advanced stay advanced when another writer's
/// save replaces the book it validated.
fn keep_windows(old: &Book, new: &mut Book) {
    for (grant, admission) in &mut new.admissions {
        if let Some(known) = old.admissions.get(grant) {
            log::merge_window(admission, known.window_start, known.reads);
        }
    }
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn sources(roots: &[Root]) -> coder_history::Config {
    let mut config = coder_history::Config::default();
    for root in roots {
        match root.scope.kind {
            SourceKind::Codex => config.codex = Some(root.path.clone()),
            SourceKind::Claude => config.claude = Some(root.path.clone()),
            SourceKind::Coder => config.coder = Some(root.path.clone()),
            SourceKind::OpenCode => config.opencode = Some(root.path.clone()),
            SourceKind::Devin => config.devin = Some(root.path.clone()),
        }
    }
    config
}

/// The snapshot of an empty chat list: the SHA-256 of nothing.
const EMPTY_SNAPSHOT: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Where the chat list keeps what it read of each saved chat, so a host that
/// restarts lists without reading every chat again.
pub fn catalog_index(directory: &Path) -> PathBuf {
    directory.join("catalog-index.json")
}

fn read(
    directory: &Path,
    roots: &[Root],
    query: &Query,
    limits: coder_history::Limits,
) -> Result<Observation> {
    if roots.is_empty() {
        // A grant with nothing this host serves: an empty chat list, and
        // no transcript.
        return match query {
            Query::Catalog(q) if q.cursor.is_none() => {
                Ok(Observation::Catalog(coder_history::CatalogPage {
                    snapshot: EMPTY_SNAPSHOT.into(),
                    entries: Vec::new(),
                    next: None,
                    notices: Vec::new(),
                }))
            }
            Query::Catalog(_) => fail(ErrorCode::Conflict, "admitted history read refused"),
            Query::Page(_) => fail(ErrorCode::Unavailable, "admitted history read refused"),
        };
    }
    for root in roots {
        root.current()?;
    }
    let history = coder_history::History::open(sources(roots))
        .map_err(history_error)?
        .with_catalog_index(catalog_index(directory));
    let observation = match query {
        Query::Catalog(q) => Observation::Catalog(
            history
                .catalog_within(q.clone(), limits)
                .map_err(history_error)?,
        ),
        Query::Page(q) => Observation::Page(
            history
                .transcript_within(q.clone(), limits)
                .map_err(history_error)?,
        ),
    };
    for root in roots {
        root.current()?;
    }
    crate::client::check_observation_within(query, &observation, limits)?;
    Ok(observation)
}
fn history_error(error: coder_history::Error) -> Error {
    use coder_history::Error as H;
    let code = match error {
        H::SourceChanged => ErrorCode::SourceChanged,
        H::CursorStale => ErrorCode::Conflict,
        H::ResourceLimit => ErrorCode::Bounds,
        H::InvalidRequest => ErrorCode::Malformed,
        H::UnsupportedPlatform => ErrorCode::Unsupported,
        _ => ErrorCode::Unavailable,
    };
    Error::new(code, "admitted history read refused")
}

pub fn ensure_parent(directory: &Path) -> Result<()> {
    #[cfg(unix)]
    fn create(parent: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
    }
    #[cfg(windows)]
    fn create(parent: &Path) -> std::io::Result<()> {
        private_fs::create_dir_all(parent)
    }
    if let Some(parent) = directory.parent() {
        create(parent).map_err(|_| {
            Error::new(
                ErrorCode::Unavailable,
                "cannot create observer parent directory",
            )
        })?;
    }
    Ok(())
}
