//! Local operator admission and finite disclosure. This host never controls engines.
use crate::{Error, ErrorCode, Result, fail, protocol::*, store::Store};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::MetadataExt;

pub const MAX_READS_PER_MINUTE: u32 = 240;
const MAX_REPLIES: usize = 256;
/// The book retains at most this many grants, live or revoked.
const MAX_ADMISSIONS: usize = 64;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
mod pairing;

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
        let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| {
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
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    fn current(&self) -> Result<()> {
        let m = std::fs::symlink_metadata(&self.path).map_err(|_| {
            Error::new(
                ErrorCode::SourceChanged,
                "admitted history root is unavailable",
            )
        })?;
        if !m.is_dir()
            || m.dev() != self.device
            || m.ino() != self.inode
            || self.path.canonicalize().ok().as_deref() != Some(self.path.as_path())
        {
            return fail(ErrorCode::SourceChanged, "admitted history root changed");
        }
        Ok(())
    }
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
    pub fn key(&self) -> Result<SecretKey> {
        let gate = self.gate();
        let _serial = gate.lock().unwrap_or_else(|poison| poison.into_inner());
        Store::open(&self.directory, false)?.key(false)
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
        let gate = self.gate();
        let _serial = gate.lock().unwrap_or_else(|poison| poison.into_inner());
        let now = clock()?;
        let route = match via {
            Via::Relay(relay) => {
                self.policy.validate(relay)?;
                Route::Relay
            }
            Via::Direct => Route::Direct,
        };
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let host = pubkey(&secret);
        let mut book = self.book(&store, &secret, false)?;
        let request: Request = open(event, &secret, &event.pubkey, &host, REQUEST)?;
        request.validate_for(route)?;
        fresh(request.issued_at, request.expires_at, now)?;
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
        // A reply is retained only through its request's lifetime: drop every
        // grant's expired replies, so the book stays small.
        for admission in book.admissions.values_mut() {
            admission.replies.retain(|_, r| r.expires_at > now);
        }
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
        let current = if admission.revoked_at.is_some() {
            Err(ErrorCode::Revoked)
        } else if admission.grant.expires_at <= now {
            Err(ErrorCode::Expired)
        } else if request.expires_at > admission.grant.expires_at
            || request.issued_at < admission.grant.issued_at
        {
            Err(ErrorCode::Forbidden)
        } else {
            Ok(())
        };
        if let Err(code) = current {
            return self
                .reply(&secret, event, &request, ReplyResult::Refused { code }, now)
                .map(Handled::from);
        }
        if let Some(old) = admission.replies.get(&request.request) {
            if old.request_event == event.id {
                // A direct reply's bytes are kept in memory through its
                // request's lifetime, for a direct retry; the book holds a
                // signed conflict in their place, which any other retry gets.
                let kept = kept()
                    .get(&(self.directory.clone(), request.request.clone()))
                    .filter(|kept| {
                        route == Route::Direct
                            && kept.request_event == event.id
                            && kept.expires_at > now
                    })
                    .map(|kept| Handled {
                        reply: kept.reply.clone(),
                        payload: Some(kept.payload.clone()),
                        read: None,
                    });
                return Ok(kept.unwrap_or_else(|| Handled::from(old.event.clone())));
            }
            return self
                .reply(
                    &secret,
                    event,
                    &request,
                    ReplyResult::Refused {
                        code: ErrorCode::Conflict,
                    },
                    now,
                )
                .map(Handled::from);
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
        let mut answered = None;
        let result = if admission.reads >= MAX_READS_PER_MINUTE {
            ReplyResult::Refused {
                code: ErrorCode::RateLimited,
            }
        } else {
            admission.reads += 1;
            match read(&admission.roots, &request.query, route.limits()) {
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
            }
        };
        // A slow reader must not extend a request's grant or freshness window.
        let final_now = clock()?;
        fresh(request.issued_at, request.expires_at, final_now)?;
        let (response, payload) = match route {
            Route::Relay => (
                self.reply(&secret, event, &request, result, final_now)?,
                None,
            ),
            Route::Direct => {
                let (reply, payload) =
                    self.reply_detached(&secret, event, &request, result, final_now)?;
                (reply, Some(payload))
            }
        };
        if admission.replies.len() >= MAX_REPLIES {
            return fail(ErrorCode::Bounds, "observer reply retention limit reached");
        }
        // The book binds the request to this exact event either way. A
        // direct reply, up to four times a relay reply's size, stays out of
        // the book, which is written on every read.
        let retained = match route {
            Route::Relay => response.clone(),
            Route::Direct => self.reply(
                &secret,
                event,
                &request,
                ReplyResult::Refused {
                    code: ErrorCode::Conflict,
                },
                final_now,
            )?,
        };
        admission.replies.insert(
            request.request.clone(),
            RetainedReply {
                request_event: event.id.clone(),
                expires_at: request.expires_at,
                event: retained,
            },
        );
        let read = answered.map(|length| Read {
            grant: request.grant.clone(),
            query: request.query.clone(),
            sources: sources(&admission.roots),
            length,
        });
        self.save(&mut store, &book)?;
        if let Some(payload) = &payload {
            let mut kept = kept();
            kept.retain(|_, kept| kept.expires_at > final_now);
            if kept.len() < 64 * MAX_REPLIES {
                kept.insert(
                    (self.directory.clone(), request.request.clone()),
                    Kept {
                        request_event: event.id.clone(),
                        expires_at: request.expires_at,
                        reply: response.clone(),
                        payload: payload.clone(),
                    },
                );
            }
        }
        Ok(Handled {
            reply: response,
            payload,
            read,
        })
    }
    /// This host's lock within the process, around every use of its store:
    /// the relay loop and direct connections share one store.
    fn gate(&self) -> std::sync::Arc<std::sync::Mutex<()>> {
        static GATES: std::sync::OnceLock<
            std::sync::Mutex<
                std::collections::HashMap<PathBuf, std::sync::Arc<std::sync::Mutex<()>>>,
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
        let book = self.load(store, secret, initialize)?;
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

/// A direct reply kept for an exact retry through its request's lifetime.
struct Kept {
    request_event: String,
    expires_at: u64,
    reply: Event,
    payload: String,
}
fn kept() -> std::sync::MutexGuard<'static, std::collections::HashMap<(PathBuf, String), Kept>> {
    static KEPT: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<(PathBuf, String), Kept>>,
    > = std::sync::OnceLock::new();
    KEPT.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
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

fn read(roots: &[Root], query: &Query, limits: coder_history::Limits) -> Result<Observation> {
    for root in roots {
        root.current()?;
    }
    let history = coder_history::History::open(sources(roots)).map_err(history_error)?;
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
    use std::os::unix::fs::DirBuilderExt;
    if let Some(parent) = directory.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|_| {
                Error::new(
                    ErrorCode::Unavailable,
                    "cannot create observer parent directory",
                )
            })?;
    }
    Ok(())
}
