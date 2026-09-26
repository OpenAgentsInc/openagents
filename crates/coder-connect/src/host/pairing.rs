//! Atomic invitation consumption shares the existing private admission book.
use super::*;
use crate::pairing::{self, Invitation};
use nostr::contracts::digest_bytes;
const MAX_PAIR_REPLIES: usize = 32;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RetainedInvitation {
    capability_digest: String,
    pub(super) relay: String,
    issued_at: u64,
    pub(super) expires_at: u64,
    grant_expires_at: u64,
    roots: Vec<Root>,
    pub(super) cancelled: bool,
    grant: Option<String>,
    replies: BTreeMap<String, RetainedReply>,
}
impl Host {
    /// Admit exact roots, then return a five-minute capability for local display.
    /// The returned code is intentionally shareable only with the pairing phone.
    pub fn invite(
        &self,
        relay: &str,
        config: coder_history::Config,
        now: u64,
        grant_expires_at: u64,
    ) -> Result<String> {
        self.policy.validate(relay)?;
        window(now, grant_expires_at, MAX_GRANT_LIFETIME)?;
        let expires_at = now
            .checked_add(pairing::LIFETIME)
            .ok_or_else(|| Error::new(ErrorCode::Malformed, "invitation lifetime overflow"))?;
        if expires_at >= grant_expires_at {
            return fail(
                ErrorCode::Expired,
                "grant must outlive the five-minute invitation",
            );
        }
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
        if book.invitations.len() >= 64 {
            return fail(
                ErrorCode::Bounds,
                "pairing invitation retention limit reached",
            );
        }
        let invitation = Invitation {
            host: pubkey(&secret),
            id: random_id(),
            capability: random_id(),
            relay: relay.into(),
            issued_at: now,
            expires_at,
        };
        invitation.validate(now, self.policy)?;
        let code = invitation.encode()?;
        book.invitations.insert(
            invitation.id,
            RetainedInvitation {
                capability_digest: digest_bytes(invitation.capability.as_bytes()),
                relay: relay.into(),
                issued_at: now,
                expires_at,
                grant_expires_at,
                roots,
                cancelled: false,
                grant: None,
                replies: BTreeMap::new(),
            },
        );
        store.save(&book)?;
        Ok(code)
    }
    /// Cancel only an unused invitation. A paired grant needs explicit revocation.
    pub fn cancel_invitation(&self, id: &str) -> Result<()> {
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let mut book = self.book(&store, &secret, false)?;
        let invitation = book.invitations.get_mut(id).ok_or_else(|| {
            Error::new(ErrorCode::Unavailable, "pairing invitation is not retained")
        })?;
        if invitation.grant.is_none() {
            invitation.cancelled = true;
            invitation.replies.clear();
        }
        store.save(&book)
    }
    /// The original grant ID, once consumption has been committed durably.
    pub fn invitation_grant(&self, id: &str) -> Result<Option<String>> {
        let store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let book = self.book(&store, &secret, false)?;
        book.invitations
            .get(id)
            .map(|i| i.grant.clone())
            .ok_or_else(|| Error::new(ErrorCode::Unavailable, "pairing invitation is not retained"))
    }
    pub(super) fn is_pairing(&self, event: &Event) -> Result<bool> {
        if event.content.len() > 400 * 1024 {
            return fail(
                ErrorCode::Bounds,
                "observer encrypted event exceeds its byte bound",
            );
        }
        let opened = nostr::private_artifact::open(event, &self.key()?)
            .map_err(|_| Error::new(ErrorCode::Forbidden, "invalid encrypted observer request"))?;
        Ok(opened.artifact().schema.as_deref() == Some(pairing::REQUEST))
    }
    pub(crate) fn redeem_with_clock(
        &self,
        event: &Event,
        relay: &str,
        mut clock: impl FnMut() -> Result<u64>,
    ) -> Result<Event> {
        self.policy.validate(relay)?;
        let mut store = Store::open(&self.directory, false)?;
        let secret = store.key(false)?;
        let host = pubkey(&secret);
        let mut book = self.book(&store, &secret, false)?;
        let request: pairing::Request =
            open(event, &secret, &event.pubkey, &host, pairing::REQUEST)?;
        request.validate(self.policy)?;
        let now = clock()?;
        fresh(request.issued_at, request.expires_at, now)?;
        if event.pubkey == host
            || request.relay != relay
            || event.tag_values("h").collect::<Vec<_>>() != [request.request.as_str()]
        {
            return fail(
                ErrorCode::Forbidden,
                "pairing request target or mailbox differs",
            );
        }
        let invitation = book.invitations.get(&request.invitation).ok_or_else(|| {
            Error::new(ErrorCode::Forbidden, "pairing invitation is not admitted")
        })?;
        if !same_digest(
            &invitation.capability_digest,
            &digest_bytes(request.capability.as_bytes()),
        ) || invitation.relay != relay
        {
            return fail(ErrorCode::Forbidden, "pairing capability or relay differs");
        }
        let state = if invitation.cancelled {
            Err(ErrorCode::Revoked)
        } else if now >= invitation.expires_at || now >= invitation.grant_expires_at {
            Err(ErrorCode::Expired)
        } else if request.issued_at < invitation.issued_at
            || request.expires_at > invitation.expires_at
        {
            Err(ErrorCode::Forbidden)
        } else if let Some(grant) = &invitation.grant {
            let admission = book.admissions.get(grant).ok_or_else(|| {
                Error::new(
                    ErrorCode::Unavailable,
                    "consumed pairing grant is unavailable",
                )
            })?;
            if admission.grant.client != event.pubkey {
                Err(ErrorCode::Forbidden)
            } else if admission.revoked_at.is_some() {
                Err(ErrorCode::Revoked)
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        let state = state.and_then(|()| {
            invitation
                .roots
                .iter()
                .try_for_each(|r| r.current().map_err(|e| e.code))
        });
        // Filesystem observation can block. Admission uses the fresh host clock.
        let now = clock()?;
        fresh(request.issued_at, request.expires_at, now)?;
        let state = state.and({
            if now >= invitation.expires_at || now >= invitation.grant_expires_at {
                Err(ErrorCode::Expired)
            } else {
                Ok(())
            }
        });
        if let Some(retained) = invitation.replies.get(&request.request) {
            if retained.request_event != event.id {
                return fail(ErrorCode::Conflict, "pairing request identity was reused");
            }
            if state.is_ok() {
                return Ok(retained.event.clone());
            }
        }
        if invitation.replies.len() >= MAX_PAIR_REPLIES {
            return fail(ErrorCode::RateLimited, "pairing reply limit reached");
        }
        let mut created = None;
        let result = match state {
            Err(code) => pairing::Outcome::Refused { code },
            Ok(()) => {
                let admission = if let Some(grant) = &invitation.grant {
                    book.admissions.get(grant).expect("validated grant")
                } else {
                    if book.admissions.len() >= 64 {
                        return fail(ErrorCode::Bounds, "observer grant retention limit reached");
                    }
                    let grant = Grant {
                        v: GRANT.into(),
                        requires: vec![],
                        grant: random_id(),
                        host: host.clone(),
                        client: event.pubkey.clone(),
                        relay: relay.into(),
                        sources: invitation.roots.iter().map(|r| r.scope.clone()).collect(),
                        issued_at: now,
                        expires_at: invitation.grant_expires_at,
                    };
                    grant.validate(self.policy)?;
                    let authorization = seal(
                        &grant,
                        GRANT,
                        &secret,
                        &event.pubkey,
                        &grant.grant,
                        now,
                        grant.expires_at,
                    )?;
                    created = Some(Admission {
                        grant,
                        authorization,
                        roots: invitation.roots.clone(),
                        revoked_at: None,
                        window_start: now,
                        reads: 0,
                        replies: BTreeMap::new(),
                    });
                    created.as_ref().expect("new admission")
                };
                pairing::Outcome::Ok {
                    connection: Box::new(connection(admission)),
                }
            }
        };
        let reply = pairing::Reply {
            v: pairing::REPLY.into(),
            requires: vec![],
            request: request.request.clone(),
            request_event: event.id.clone(),
            invitation: request.invitation.clone(),
            issued_at: now,
            expires_at: request.expires_at,
            result,
        };
        let event = seal(
            &reply,
            pairing::REPLY,
            &secret,
            &event.pubkey,
            &request.request,
            now,
            request.expires_at,
        )?;
        let invitation = book
            .invitations
            .get_mut(&request.invitation)
            .expect("validated invitation");
        if let Some(admission) = created {
            invitation.grant = Some(admission.grant.grant.clone());
            book.admissions
                .insert(admission.grant.grant.clone(), admission);
        }
        invitation.replies.insert(
            request.request,
            RetainedReply {
                request_event: reply.request_event,
                expires_at: request.expires_at,
                event: event.clone(),
            },
        );
        // Consumption, grant, and exact reply commit together; no success escapes
        // a failed save. A retry reopens this book rather than reusing memory.
        store.save(&book)?;
        Ok(event)
    }
    pub(super) fn validate_invitations(&self, book: &Book, secret: &SecretKey) -> Result<()> {
        if book.invitations.len() > 64 {
            return fail(
                ErrorCode::Bounds,
                "pairing invitation retention limit reached",
            );
        }
        for (id, i) in &book.invitations {
            identity(id)?;
            // digest_bytes uses a prefixed SHA-256 digest; compare its exact shape.
            if i.capability_digest.len() != digest_bytes(b"").len()
                || i.roots.is_empty()
                || i.roots.len() > 2
                || i.replies.len() > MAX_PAIR_REPLIES
            {
                return fail(ErrorCode::Malformed, "retained pairing bounds differ");
            }
            window(i.issued_at, i.expires_at, pairing::LIFETIME)?;
            window(i.issued_at, i.grant_expires_at, MAX_GRANT_LIFETIME)?;
            if i.expires_at - i.issued_at != pairing::LIFETIME || i.grant_expires_at <= i.expires_at
            {
                return fail(ErrorCode::Malformed, "retained pairing lifetime differs");
            }
            self.policy.validate(&i.relay)?;
            if let Some(grant) = &i.grant {
                let a = book.admissions.get(grant).ok_or_else(|| {
                    Error::new(
                        ErrorCode::Unavailable,
                        "consumed pairing grant is unavailable",
                    )
                })?;
                if a.grant.relay != i.relay
                    || a.grant.expires_at != i.grant_expires_at
                    || encoded(&a.roots)? != encoded(&i.roots)?
                {
                    return fail(ErrorCode::Forbidden, "consumed pairing scope differs");
                }
            }
            for (request, r) in &i.replies {
                let recipient = r.event.tag_values("p").next().ok_or_else(|| {
                    Error::new(ErrorCode::Malformed, "pairing reply recipient absent")
                })?;
                let reply: pairing::Reply =
                    open(&r.event, secret, &book.host, recipient, pairing::REPLY)?;
                schema(&reply.v, pairing::REPLY, &reply.requires)?;
                if reply.request != *request
                    || reply.request_event != r.request_event
                    || reply.invitation != *id
                    || reply.expires_at != r.expires_at
                    || r.event.tag_values("h").collect::<Vec<_>>() != [request.as_str()]
                {
                    return fail(ErrorCode::Forbidden, "retained pairing reply differs");
                }
                if let pairing::Outcome::Ok { connection: code } = reply.result {
                    let admission = i
                        .grant
                        .as_ref()
                        .and_then(|id| book.admissions.get(id))
                        .ok_or_else(|| {
                            Error::new(ErrorCode::Unavailable, "paired grant missing")
                        })?;
                    if encoded(&*code)? != encoded(&connection(admission))? {
                        return fail(ErrorCode::Forbidden, "retained pairing connection differs");
                    }
                }
            }
        }
        Ok(())
    }
}
fn same_digest(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.as_bytes()
            .iter()
            .zip(b.as_bytes())
            .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}
fn connection(a: &Admission) -> ConnectionCode {
    ConnectionCode {
        v: CONNECTION.into(),
        requires: vec![],
        host: a.grant.host.clone(),
        client: a.grant.client.clone(),
        relay: a.grant.relay.clone(),
        grant: a.grant.grant.clone(),
        sources: a.grant.sources.clone(),
        expires_at: a.grant.expires_at,
        authorization: a.authorization.clone(),
    }
}
