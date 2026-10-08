//! One conditional public-reply channel: GitHub Issues on repositories the
//! owner names (REV-74). The adapter stays disabled: a grant admits fixture
//! operation only, a live grant is refused until the owner activates the
//! channel, and every reply binds one invited thread, one labeled account,
//! exact content, and an unexpired approval. A platform response is data.
use super::*;

pub const GRANT_SCHEMA: &str = "openagents.sales.public-reply-grant.v1";
pub const REPLY_SCHEMA: &str = "openagents.sales.public-reply.v1";
pub const API_VERSION: &str = "2022-11-28";
pub const MAX_BODY: usize = 4096;
pub const MAX_THREADS: usize = 32;
pub const MAX_PER_DAY: u32 = 5;
pub const MAX_DURATION_SECS: u64 = 30 * 86_400;
const MAX_RECORDS: usize = 512;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    GithubIssues,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Thread {
    pub owner: String,
    pub repository: String,
    pub number: u64,
}
fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        && !value.starts_with('.')
}
impl Thread {
    fn check(&self) -> Result<()> {
        if !segment(&self.owner) || !segment(&self.repository) || self.number == 0 {
            return Err("public reply thread names one repository issue".into());
        }
        Ok(())
    }
    pub fn path(&self) -> String {
        format!(
            "/repos/{}/{}/issues/{}/comments",
            self.owner, self.repository, self.number
        )
    }
}
/// The owner's exact scope for one agent account on one platform.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub schema: String,
    pub id: String,
    pub mode: Mode,
    pub platform: Platform,
    pub account: String,
    pub label: String,
    pub threads: std::collections::BTreeSet<Thread>,
    pub invitation_sha256: String,
    pub credential_sha256: String,
    pub replies_per_day: u32,
    pub expires_at: u64,
    pub owner_review_sha256: String,
}
impl Grant {
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "public reply grant serialization failed")?,
        ))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrantPhase {
    Active,
    Revoked,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRecord {
    pub grant: Grant,
    pub grant_sha256: String,
    pub owner: String,
    pub granted_at: u64,
    pub phase: GrantPhase,
    pub reference_sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub schema: String,
    pub id: String,
    pub grant: String,
    pub thread: Thread,
    /// Platform login of the person whose comment invited this reply.
    pub invited_by: String,
    pub in_reply_to_sha256: String,
    pub body: String,
    pub expires_at: u64,
}
impl Reply {
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "public reply serialization failed")?,
        ))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplyPhase {
    Proposed,
    Approved,
    Rejected,
    DispatchIntent,
    Posted,
    Failed,
    Unknown,
    Cancelled,
    Invalidated,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub reply_sha256: String,
    pub delivery: email::Delivery,
    pub status: Option<u16>,
    pub reference_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyRecord {
    pub reply: Reply,
    pub reply_sha256: String,
    pub actor: String,
    pub created_at: u64,
    pub decision: Option<Decision>,
    pub phase: ReplyPhase,
    pub attempt: Option<String>,
    pub observation: Option<Observation>,
}
/// A platform response, retained as untrusted data and never acted on.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub id: String,
    pub thread: Thread,
    pub author: String,
    pub text_sha256: String,
    pub opt_out: bool,
    pub directive_like: bool,
    pub received_at: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub grants: BTreeMap<String, GrantRecord>,
    pub replies: BTreeMap<String, ReplyRecord>,
    pub responses: BTreeMap<String, Response>,
    pub suppressed: std::collections::BTreeSet<String>,
    pub paused: bool,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.grants.len() > MAX_RECORDS
            || self.replies.len() > MAX_RECORDS
            || self.responses.len() > MAX_RECORDS
            || self.suppressed.len() > MAX_RECORDS
        {
            return Err("public reply history exceeds its bound".into());
        }
        for (id, r) in &self.grants {
            super::super::id(id)?;
            if r.grant.id != *id || r.grant.schema != GRANT_SCHEMA {
                return Err("public reply grant record is inconsistent".into());
            }
            token(&r.grant_sha256)?;
        }
        for (id, r) in &self.replies {
            super::super::id(id)?;
            if r.reply.id != *id || r.reply.schema != REPLY_SCHEMA {
                return Err("public reply record is inconsistent".into());
            }
            token(&r.reply_sha256)?;
        }
        Ok(())
    }
    pub fn enabled(&self, now: u64) -> bool {
        !self.paused
            && self
                .grants
                .values()
                .any(|r| r.phase == GrantPhase::Active && r.grant.expires_at > now)
    }
    fn active_grant(&self, id: &str, now: u64) -> Result<&Grant> {
        let record = self
            .grants
            .get(id)
            .ok_or("public reply grant is unavailable")?;
        if record.phase != GrantPhase::Active || record.grant.expires_at <= now {
            return Err("public reply grant expired or was revoked".into());
        }
        Ok(&record.grant)
    }
}
/// The exact HTTP submission, without the credential. The rendered body
/// carries the account label so the disclosure survives rendering.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}
pub fn request(reply: &Reply, grant: &Grant) -> Result<Request> {
    let rendered = format!("{}\n\n— {}", reply.body, grant.label);
    let body = serde_json::json!({ "body": rendered }).to_string();
    let mut headers = BTreeMap::new();
    headers.insert("accept".into(), "application/vnd.github+json".into());
    headers.insert("x-github-api-version".into(), API_VERSION.into());
    headers.insert(
        "user-agent".into(),
        format!("openagents-sales/{}", grant.account),
    );
    headers.insert("content-type".into(), "application/json".into());
    Ok(Request {
        method: "POST".into(),
        url: format!("https://api.github.com{}", reply.thread.path()),
        headers,
        body,
    })
}
/// A transport posts one request once. Only the fixture transport exists.
pub trait Transport {
    fn post(&mut self, request: &Request, cancel: &AtomicBool) -> Result<Observation>;
}
pub struct FakeTransport {
    pub delivery: email::Delivery,
    pub status: Option<u16>,
    pub posted: Vec<Request>,
}
impl Transport for FakeTransport {
    fn post(&mut self, request: &Request, cancel: &AtomicBool) -> Result<Observation> {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("public reply cancelled before submission".into());
        }
        self.posted.push(request.clone());
        Ok(Observation {
            reply_sha256: digest(request.body.as_bytes()),
            delivery: self.delivery,
            status: self.status,
            reference_sha256: digest(format!("fake-github:{}", self.posted.len()).as_bytes()),
        })
    }
}
fn directive_like(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "ignore previous",
        "ignore all",
        "you are now",
        "system:",
        "run `",
        "execute ",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}
fn opt_out(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "stop replying",
        "do not reply",
        "don't reply",
        "unsubscribe",
        "leave me alone",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

impl Store {
    pub fn public_reply_grant(&mut self, access: &Access, grant: Grant) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        let now = (self.clock)();
        super::super::id(&grant.id)?;
        token(&grant.invitation_sha256)?;
        token(&grant.credential_sha256)?;
        token(&grant.owner_review_sha256)?;
        for thread in &grant.threads {
            thread.check()?;
        }
        if grant.mode == Mode::Live {
            return Err("live public replies are not activated; the GitHub Issues adapter is disabled until the owner records platform permission".into());
        }
        if grant.schema != GRANT_SCHEMA
            || !segment(&grant.account)
            || grant.label.is_empty()
            || grant.label.len() > 128
            || !grant.label.contains(&grant.account)
            || !grant.label.to_ascii_lowercase().contains("ai")
            || grant.threads.is_empty()
            || grant.threads.len() > MAX_THREADS
            || grant.replies_per_day == 0
            || grant.replies_per_day > MAX_PER_DAY
            || grant.expires_at <= now
            || grant.expires_at > now + MAX_DURATION_SECS
            || self
                .state
                .outbox
                .public_replies
                .grants
                .contains_key(&grant.id)
        {
            return Err("public reply grant needs one labeled AI account, one to thirty-two invited threads, a daily cap of at most five, and a bounded expiry".into());
        }
        let mut next = self.state.clone();
        next.outbox.public_replies.grants.insert(
            grant.id.clone(),
            GrantRecord {
                grant_sha256: grant.sha256()?,
                grant,
                owner: access.principal().into(),
                granted_at: now,
                phase: GrantPhase::Active,
                reference_sha256: None,
            },
        );
        self.bump(next)
    }
    pub fn public_reply_revoke(
        &mut self,
        access: &Access,
        grant: &str,
        reference_sha256: &str,
    ) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        token(reference_sha256)?;
        let mut next = self.state.clone();
        let record = next
            .outbox
            .public_replies
            .grants
            .get_mut(grant)
            .ok_or("public reply grant is unavailable")?;
        record.phase = GrantPhase::Revoked;
        record.reference_sha256 = Some(reference_sha256.into());
        for r in next.outbox.public_replies.replies.values_mut() {
            if r.reply.grant == grant
                && matches!(r.phase, ReplyPhase::Proposed | ReplyPhase::Approved)
            {
                r.phase = ReplyPhase::Invalidated;
            }
        }
        self.bump(next)
    }
    pub fn public_reply_pause(&mut self, access: &Access, paused: bool) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        let mut next = self.state.clone();
        next.outbox.public_replies.paused = paused;
        self.bump(next)
    }
    pub fn public_reply_suppress(&mut self, access: &Access, account: &str) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        if !segment(account) {
            return Err("suppression names one platform account".into());
        }
        let mut next = self.state.clone();
        next.outbox.public_replies.suppressed.insert(account.into());
        for r in next.outbox.public_replies.replies.values_mut() {
            if r.reply.invited_by == account
                && matches!(r.phase, ReplyPhase::Proposed | ReplyPhase::Approved)
            {
                r.phase = ReplyPhase::Invalidated;
            }
        }
        self.bump(next)
    }
    /// Records what a platform returned. It can suppress; it can never grant.
    pub fn public_reply_response(
        &mut self,
        access: &Access,
        id: &str,
        thread: Thread,
        author: &str,
        text: &str,
    ) -> Result<Response> {
        self.refresh()?;
        self.admin(access)?;
        super::super::id(id)?;
        thread.check()?;
        if !segment(author) || text.len() > 64 * 1024 {
            return Err("public response needs a platform author and bounded text".into());
        }
        let response = Response {
            id: id.into(),
            thread,
            author: author.into(),
            text_sha256: digest(text.as_bytes()),
            opt_out: opt_out(text),
            directive_like: directive_like(text),
            received_at: (self.clock)(),
        };
        let mut next = self.state.clone();
        next.outbox
            .public_replies
            .responses
            .insert(response.id.clone(), response.clone());
        if response.opt_out {
            next.outbox
                .public_replies
                .suppressed
                .insert(response.author.clone());
            for r in next.outbox.public_replies.replies.values_mut() {
                if r.reply.invited_by == response.author
                    && matches!(r.phase, ReplyPhase::Proposed | ReplyPhase::Approved)
                {
                    r.phase = ReplyPhase::Invalidated;
                }
            }
        }
        self.bump(next)?;
        Ok(response)
    }
    pub fn public_reply_propose(&mut self, access: &Access, reply: Reply) -> Result<String> {
        self.refresh()?;
        self.admin(access)?;
        let now = (self.clock)();
        super::super::id(&reply.id)?;
        token(&reply.in_reply_to_sha256)?;
        reply.thread.check()?;
        let book = &self.state.outbox.public_replies;
        let grant = book.active_grant(&reply.grant, now)?;
        if reply.schema != REPLY_SCHEMA
            || reply.body.is_empty()
            || reply.body.len() > MAX_BODY
            || !reply.body.is_ascii() && reply.body.chars().any(char::is_control)
            || reply.body.bytes().any(|b| b < 0x20 && b != b'\n')
            || reply.expires_at <= now
            || reply.expires_at > grant.expires_at
            || book.replies.contains_key(&reply.id)
        {
            return Err("public reply needs bounded text and an expiry within its grant".into());
        }
        if !grant.threads.contains(&reply.thread) {
            return Err("public reply thread is outside the granted invited threads".into());
        }
        if !segment(&reply.invited_by)
            || book.suppressed.contains(&reply.invited_by)
            || reply.invited_by == grant.account
        {
            return Err("public reply needs an inviting account that is not suppressed".into());
        }
        let day = now / 86_400;
        let today = book
            .replies
            .values()
            .filter(|r| r.reply.grant == reply.grant && r.created_at / 86_400 == day)
            .filter(|r| !matches!(r.phase, ReplyPhase::Rejected | ReplyPhase::Invalidated))
            .count();
        if today >= grant.replies_per_day as usize {
            return Err("public reply grant reached its daily cap".into());
        }
        let reply_sha256 = reply.sha256()?;
        let id = reply.id.clone();
        let mut next = self.state.clone();
        next.outbox.public_replies.replies.insert(
            id.clone(),
            ReplyRecord {
                reply,
                reply_sha256: reply_sha256.clone(),
                actor: access.principal().into(),
                created_at: now,
                decision: None,
                phase: ReplyPhase::Proposed,
                attempt: None,
                observation: None,
            },
        );
        self.bump(next)?;
        Ok(reply_sha256)
    }
    pub fn public_reply_decide(
        &mut self,
        access: &Access,
        id: &str,
        reply_sha256: &str,
        approve: bool,
    ) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        let now = (self.clock)();
        let mut next = self.state.clone();
        let record = next
            .outbox
            .public_replies
            .replies
            .get_mut(id)
            .ok_or("public reply is unavailable")?;
        if record.phase != ReplyPhase::Proposed
            || record.reply_sha256 != reply_sha256
            || record.reply.sha256()? != reply_sha256
            || record.reply.expires_at <= now
        {
            return Err("public reply decision needs the exact unexpired proposal".into());
        }
        record.decision = Some(Decision {
            subject_sha256: reply_sha256.into(),
            owner: access.principal().into(),
            approved: approve,
            at: now,
        });
        record.phase = if approve {
            ReplyPhase::Approved
        } else {
            ReplyPhase::Rejected
        };
        self.bump(next)
    }
    pub fn public_reply_cancel(&mut self, access: &Access, id: &str) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        let mut next = self.state.clone();
        let record = next
            .outbox
            .public_replies
            .replies
            .get_mut(id)
            .ok_or("public reply is unavailable")?;
        if !matches!(record.phase, ReplyPhase::Proposed | ReplyPhase::Approved) {
            return Err("only an undispatched public reply can be cancelled".into());
        }
        record.phase = ReplyPhase::Cancelled;
        self.bump(next)
    }
    /// Rechecks every binding, persists a single-use attempt, posts once, and
    /// retains the observation. An unknown outcome stays unknown.
    pub fn public_reply_dispatch_fixture(
        &mut self,
        access: &Access,
        id: &str,
        reply_sha256: &str,
        transport: &mut dyn Transport,
        cancel: &AtomicBool,
    ) -> Result<ReplyRecord> {
        self.refresh()?;
        self.admin(access)?;
        let now = (self.clock)();
        let book = &self.state.outbox.public_replies;
        if book.paused {
            return Err("public replies are paused".into());
        }
        let record = book.replies.get(id).ok_or("public reply is unavailable")?;
        let grant = book.active_grant(&record.reply.grant, now)?;
        if grant.mode != Mode::Fixture {
            return Err("fixture transport cannot consume live public-reply authority".into());
        }
        if record.phase != ReplyPhase::Approved
            || record.attempt.is_some()
            || record.reply_sha256 != reply_sha256
            || record.reply.sha256()? != reply_sha256
            || record.decision.as_ref().map(|d| d.subject_sha256.as_str()) != Some(reply_sha256)
            || record.reply.expires_at <= now
            || !grant.threads.contains(&record.reply.thread)
            || book.suppressed.contains(&record.reply.invited_by)
        {
            return Err("public reply dispatch needs an exact approved, unexpired, unsuppressed reply with no earlier attempt".into());
        }
        let request = request(&record.reply, grant)?;
        let attempt = digest(format!("{id}:{reply_sha256}:{now}").as_bytes());
        let mut next = self.state.clone();
        let row = next.outbox.public_replies.replies.get_mut(id).unwrap();
        row.phase = ReplyPhase::DispatchIntent;
        row.attempt = Some(attempt);
        self.bump(next)?;
        let observation = transport
            .post(&request, cancel)
            .unwrap_or_else(|_| Observation {
                reply_sha256: digest(request.body.as_bytes()),
                delivery: if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    email::Delivery::Cancelled
                } else {
                    email::Delivery::Unknown
                },
                status: None,
                reference_sha256: digest(b"platform observation unavailable"),
            });
        let mut next = self.state.clone();
        let row = next.outbox.public_replies.replies.get_mut(id).unwrap();
        row.phase = match observation.delivery {
            email::Delivery::Accepted | email::Delivery::Delivered => ReplyPhase::Posted,
            email::Delivery::Failed
            | email::Delivery::HardBounce
            | email::Delivery::AuthenticationFailed => ReplyPhase::Failed,
            email::Delivery::Cancelled => ReplyPhase::Cancelled,
            email::Delivery::Unknown => ReplyPhase::Unknown,
        };
        row.observation = Some(observation);
        let record = row.clone();
        self.bump(next)?;
        Ok(record)
    }
    /// The owner states what the platform shows for an unknown attempt.
    pub fn public_reply_reconcile(
        &mut self,
        access: &Access,
        id: &str,
        attempt: &str,
        posted: bool,
        reference_sha256: &str,
    ) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        token(reference_sha256)?;
        let mut next = self.state.clone();
        let record = next
            .outbox
            .public_replies
            .replies
            .get_mut(id)
            .ok_or("public reply is unavailable")?;
        if record.phase != ReplyPhase::Unknown || record.attempt.as_deref() != Some(attempt) {
            return Err("reconciliation names the exact unknown attempt".into());
        }
        record.phase = if posted {
            ReplyPhase::Posted
        } else {
            ReplyPhase::Failed
        };
        if let Some(o) = record.observation.as_mut() {
            o.reference_sha256 = reference_sha256.into();
        }
        self.bump(next)
    }
    fn bump(&mut self, mut next: super::State) -> Result<u64> {
        next.outbox.revision = next
            .outbox
            .revision
            .checked_add(1)
            .ok_or("outbox revision overflow")?;
        let revision = next.outbox.revision;
        self.persist(next)?;
        Ok(revision)
    }
}
