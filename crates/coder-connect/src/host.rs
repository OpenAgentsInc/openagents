//! Local operator admission and finite disclosure. This host never controls engines.
use crate::{Error, ErrorCode, Result, fail, protocol::*, store::Store};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::MetadataExt;

pub const MAX_READS_PER_MINUTE: u32 = 240;
const MAX_REPLIES: usize = 256;
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
#[derive(Serialize, Deserialize)]
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
        window(now, expires_at, MAX_GRANT_LIFETIME)?;
        let mut roots = Vec::new();
        if let Some(path) = config.codex {
            roots.push(Root::admit(path, SourceKind::Codex)?);
        }
        if let Some(path) = config.claude {
            roots.push(Root::admit(path, SourceKind::Claude)?);
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
        if book.admissions.len() >= 64 {
            return fail(ErrorCode::Bounds, "observer grant retention limit reached");
        }
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
        store.save(&book)?;
        Ok(code)
    }
    pub fn revoke(&self, grant: &str, source: Option<&str>, now: u64) -> Result<()> {
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
        store.save(&book)
    }
    pub fn relays(&self, now: u64) -> Result<Vec<String>> {
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
        mut clock: impl FnMut() -> Result<u64>,
    ) -> Result<Event> {
        let now = clock()?;
        self.policy.validate(relay)?;
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let host = pubkey(&secret);
        let mut book = self.book(&store, &secret, false)?;
        let request: Request = open(event, &secret, &event.pubkey, &host, REQUEST)?;
        request.validate()?;
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
        let admission = book
            .admissions
            .get_mut(&request.grant)
            .ok_or_else(|| Error::new(ErrorCode::Forbidden, "request has no admitted grant"))?;
        if admission.grant.client != event.pubkey
            || admission.grant.relay != relay
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
            return self.reply(&secret, event, &request, ReplyResult::Refused { code }, now);
        }
        admission.replies.retain(|_, r| r.expires_at > now);
        if let Some(old) = admission.replies.get(&request.request) {
            if old.request_event == event.id {
                return Ok(old.event.clone());
            }
            return self.reply(
                &secret,
                event,
                &request,
                ReplyResult::Refused {
                    code: ErrorCode::Conflict,
                },
                now,
            );
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
        let result = if admission.reads >= MAX_READS_PER_MINUTE {
            ReplyResult::Refused {
                code: ErrorCode::RateLimited,
            }
        } else {
            admission.reads += 1;
            match read(&admission.roots, &request.query) {
                Ok(observation) => ReplyResult::Ok {
                    observation: Box::new(observation),
                },
                Err(error) => ReplyResult::Refused { code: error.code },
            }
        };
        // A slow reader must not extend a request's grant or freshness window.
        let final_now = clock()?;
        fresh(request.issued_at, request.expires_at, final_now)?;
        let response = self.reply(&secret, event, &request, result, final_now)?;
        if admission.replies.len() >= MAX_REPLIES {
            return fail(ErrorCode::Bounds, "observer reply retention limit reached");
        }
        admission.replies.insert(
            request.request.clone(),
            RetainedReply {
                request_event: event.id.clone(),
                expires_at: request.expires_at,
                event: response.clone(),
            },
        );
        store.save(&book)?;
        Ok(response)
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
    fn book(&self, store: &Store, secret: &SecretKey, initialize: bool) -> Result<Book> {
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
            || book.admissions.len() > 64
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
fn read(roots: &[Root], query: &Query) -> Result<Observation> {
    let mut config = coder_history::Config {
        codex: None,
        claude: None,
    };
    for root in roots {
        root.current()?;
        match root.scope.kind {
            SourceKind::Codex => config.codex = Some(root.path.clone()),
            SourceKind::Claude => config.claude = Some(root.path.clone()),
        }
    }
    let history = coder_history::History::open(config).map_err(history_error)?;
    let observation = match query {
        Query::Catalog(q) => {
            Observation::Catalog(history.catalog(q.clone()).map_err(history_error)?)
        }
        Query::Page(q) => Observation::Page(history.transcript(q.clone()).map_err(history_error)?),
    };
    for root in roots {
        root.current()?;
    }
    crate::client::check_observation(query, &observation)?;
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
