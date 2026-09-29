//! Portable device client. It holds no host store and cannot issue access.
//! Build with `default-features = false` for a mobile or thin client.
use crate::protocol::*;
use crate::{Code, Error, Result, Rights, fail, unix_time};
use coder_connect::RelayPolicy;
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

/// The exact signed packet. Retry it unchanged; a new request ID is a new operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub request: Request,
    pub event: Event,
}

/// A device holding a host grant, or the host's locally established owner.
pub struct Client {
    host: String,
    relay: String,
    secret: SecretKey,
    policy: RelayPolicy,
    access: Option<Access>,
}
impl Client {
    /// A device client. Verification is offline; the host still checks revocation.
    pub fn device(access: Access, secret: SecretKey, policy: RelayPolicy) -> Result<Self> {
        access.verify(&secret, unix_time()?, policy)?;
        Ok(Self {
            host: access.grant.host.clone(),
            relay: access.grant.relay.clone(),
            secret,
            policy,
            access: Some(access),
        })
    }
    /// The owner signs without a grant. The host admits it only when its own
    /// locally established owner key matches the signer.
    pub fn owner(host: &str, relay: &str, secret: SecretKey, policy: RelayPolicy) -> Result<Self> {
        public(host)?;
        policy.validate(relay).map_err(Error::from)?;
        if pubkey(&secret) == host {
            return fail(Code::Forbidden, "owner and host keys must differ");
        }
        Ok(Self {
            host: host.into(),
            relay: relay.into(),
            secret,
            policy,
            access: None,
        })
    }
    pub fn access(&self) -> Option<&Access> {
        self.access.as_ref()
    }
    pub fn host(&self) -> &str {
        &self.host
    }
    /// The relay this client's requests name.
    pub fn relay(&self) -> &str {
        &self.relay
    }
    pub(crate) fn policy(&self) -> RelayPolicy {
        self.policy
    }
    pub(crate) fn secret(&self) -> &SecretKey {
        &self.secret
    }
    pub fn prepare(&self, op: Operation, now: u64) -> Result<Pending> {
        if matches!(op, Operation::Redeem { .. }) {
            return fail(Code::Malformed, "redeem an invitation with `redeem`");
        }
        let mut expires_at = now.saturating_add(MAX_REQUEST_LIFETIME);
        if let Some(access) = &self.access {
            access.verify(&self.secret, now, self.policy)?;
            expires_at = expires_at.min(access.grant.expires_at);
        }
        let request = Request {
            v: REQUEST.into(),
            requires: vec![],
            request: random_id(),
            host: self.host.clone(),
            grant: self.access.as_ref().map(|a| a.grant.grant.clone()),
            epoch: self.access.as_ref().map(|a| a.grant.epoch),
            relay: self.relay.clone(),
            issued_at: now,
            expires_at,
            op,
        };
        request.validate(self.policy)?;
        let event = seal(
            &request,
            REQUEST,
            &self.secret,
            &self.host,
            &request.request,
            now,
            request.expires_at,
        )?;
        Ok(Pending { request, event })
    }
    pub fn verify_reply(&self, pending: &Pending, event: &Event, now: u64) -> Result<Outcome> {
        verify_reply(&self.secret, &self.host, pending, event, now)
    }
    pub async fn send(&self, pending: &Pending) -> Result<Outcome> {
        let event = exchange(&self.relay, &self.secret, pending, &self.host, self.policy).await?;
        self.verify_reply(pending, &event, unix_time()?)
    }
    pub async fn call(&self, op: Operation) -> Result<Outcome> {
        let pending = self.prepare(op, unix_time()?)?;
        self.send(&pending).await
    }
}

fn verify_reply(
    secret: &SecretKey,
    host: &str,
    pending: &Pending,
    event: &Event,
    now: u64,
) -> Result<Outcome> {
    let me = pubkey(secret);
    let reply: Reply = open(event, secret, host, &me, REPLY)?;
    schema(&reply.v, REPLY, &reply.requires)?;
    fresh(reply.issued_at, reply.expires_at, now)?;
    if reply.request != pending.request.request
        || reply.request_event != pending.event.id
        || reply.host != host
        || reply.expires_at != pending.request.expires_at
        // A host clock up to the skew behind this device's may date its
        // reply just before the request.
        || reply.issued_at.saturating_add(coder_connect::protocol::CLOCK_SKEW)
            < pending.request.issued_at
        || event.tag_values("h").collect::<Vec<_>>() != [pending.request.request.as_str()]
    {
        return fail(Code::Forbidden, "host reply does not match this request");
    }
    match reply.result {
        ReplyResult::Refused { code, missing } => {
            if (code == Code::MissingRight) != missing.is_some() {
                return fail(Code::Malformed, "refusal and missing right disagree");
            }
            Err(Error {
                code,
                missing,
                message: format!("host refused `{}`", pending.request.op.name()),
            })
        }
        ReplyResult::Ok { outcome } => {
            if !outcome.answers(&pending.request.op) {
                return fail(Code::Forbidden, "host answered with another operation");
            }
            outcome.validate()?;
            if let Outcome::Granted { authorization } = &outcome {
                nostr::private_artifact::admit(authorization)
                    .map_err(|_| Error::new(Code::Forbidden, "grant envelope is invalid"))?;
                if authorization.pubkey != host {
                    return fail(Code::Forbidden, "grant envelope signer differs from host");
                }
            }
            Ok(outcome)
        }
    }
}

async fn exchange(
    relay: &str,
    secret: &SecretKey,
    pending: &Pending,
    host: &str,
    policy: RelayPolicy,
) -> Result<Event> {
    tokio::time::timeout(Duration::from_secs(12), async {
        let mut session = coder_connect::transport::Session::connect(relay, secret, policy).await?;
        session
            .exchange_event(
                &pending.event,
                &pending.request.request,
                (pending.request.issued_at, pending.request.expires_at),
                host,
                &pubkey(secret),
            )
            .await
    })
    .await
    .map_err(|_| Error::new(Code::Transport, "host access exchange timed out"))?
    .map_err(Error::from)
}

/// Build the signed redemption for a scanned or pasted host invitation.
pub fn prepare_redeem(
    invitation: &HostInvitation,
    secret: &SecretKey,
    now: u64,
    policy: RelayPolicy,
) -> Result<Pending> {
    let inner = &invitation.0;
    if pubkey(secret) == inner.host {
        return fail(Code::Forbidden, "device and host keys must differ");
    }
    fresh(inner.issued_at, inner.expires_at, now)?;
    let request = Request {
        v: REQUEST.into(),
        requires: vec![],
        request: random_id(),
        host: inner.host.clone(),
        grant: None,
        epoch: None,
        relay: inner.relay.clone(),
        issued_at: now,
        expires_at: now
            .saturating_add(MAX_REQUEST_LIFETIME)
            .min(inner.expires_at),
        op: Operation::Redeem {
            invitation: inner.id.clone(),
            capability: inner.capability().into(),
        },
    };
    request.validate(policy)?;
    let event = seal(
        &request,
        REQUEST,
        secret,
        &inner.host,
        &request.request,
        now,
        request.expires_at,
    )?;
    Ok(Pending { request, event })
}
/// Check the host's signed reply and return the device's access record.
pub fn finish_redeem(
    invitation: &HostInvitation,
    pending: &Pending,
    event: &Event,
    secret: &SecretKey,
    now: u64,
    policy: RelayPolicy,
) -> Result<Access> {
    let Outcome::Granted { authorization } =
        verify_reply(secret, invitation.host(), pending, event, now)?
    else {
        return fail(Code::Forbidden, "host did not answer with a grant");
    };
    let access =
        Access::from_authorization(*authorization, secret, invitation.host(), now, policy)?;
    if access.grant.relay != invitation.relay() {
        return fail(Code::Forbidden, "grant relay differs from the invitation");
    }
    Ok(access)
}
/// Redeem a host invitation with this device's protected key. Save the result
/// only on success; a failure leaves any existing access unchanged.
pub async fn redeem(code: &str, secret: &SecretKey, policy: RelayPolicy) -> Result<Access> {
    let invitation = HostInvitation::parse(code, unix_time()?, policy)?;
    let pending = prepare_redeem(&invitation, secret, unix_time()?, policy)?;
    let event = exchange(
        invitation.relay(),
        secret,
        &pending,
        invitation.host(),
        policy,
    )
    .await?;
    finish_redeem(&invitation, &pending, &event, secret, unix_time()?, policy)
}

/// An enrollment request this approver opened from a pinned host.
#[derive(Clone, Debug)]
pub struct OpenedEnrollment {
    pub enrollment: Enrollment,
    pub digest: String,
    pub event: Event,
}
impl OpenedEnrollment {
    /// Approve the exact request with the code shown on the host's screen.
    /// Rights must fit both the request and the approver's own rights.
    pub fn approve(
        &self,
        code: &str,
        device: &str,
        rights: Rights,
        grant_expires_at: u64,
    ) -> Operation {
        Operation::Approve {
            enrollment: self.enrollment.enrollment.clone(),
            request_digest: self.digest.clone(),
            code: code.into(),
            device: device.into(),
            rights,
            grant_expires_at,
        }
    }
    pub fn deny(&self) -> Operation {
        Operation::Deny {
            enrollment: self.enrollment.enrollment.clone(),
            request_digest: self.digest.clone(),
        }
    }
}
pub fn open_enrollment(
    event: &Event,
    secret: &SecretKey,
    host: &str,
    now: u64,
    policy: RelayPolicy,
) -> Result<OpenedEnrollment> {
    let enrollment: Enrollment = open(event, secret, host, &pubkey(secret), ENROLLMENT)?;
    enrollment.validate(policy)?;
    if enrollment.host != host
        || event.tag_values("h").collect::<Vec<_>>() != [enrollment.enrollment.as_str()]
    {
        return fail(Code::Forbidden, "enrollment request identity differs");
    }
    fresh(enrollment.issued_at, enrollment.expires_at, now)?;
    Ok(OpenedEnrollment {
        digest: enrollment.digest()?,
        enrollment,
        event: event.clone(),
    })
}
/// Fetch current enrollment requests a pinned host addressed to this key.
pub async fn pending_enrollments(
    relay: &str,
    secret: &SecretKey,
    host: &str,
    policy: RelayPolicy,
) -> Result<Vec<OpenedEnrollment>> {
    policy.validate(relay).map_err(Error::from)?;
    public(host)?;
    let transport = |_| Error::new(Code::Transport, "enrollment fetch is unavailable");
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut socket =
            nostr_transport::Connection::connect(relay, secret, Duration::from_secs(10))
                .await
                .map_err(transport)?;
        let me = pubkey(secret);
        socket
            .send(json!(["REQ","host-enrollments",{"kinds":[3188],"authors":[host],"#p":[me],"limit":64}]))
            .await
            .map_err(transport)?;
        let mut found = Vec::new();
        loop {
            let frame = socket.next().await.map_err(transport)?;
            if frame[0] == "EOSE" && frame[1] == "host-enrollments" {
                break;
            }
            if frame[0] == "CLOSED" {
                return fail(Code::Transport, "relay closed the enrollment fetch");
            }
            if frame[0] == "EVENT" && frame[1] == "host-enrollments" {
                let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                    continue;
                };
                if schema_of(&event, secret).ok().as_deref() != Some(ENROLLMENT) {
                    continue;
                }
                // Expired and malformed requests are skipped, never approved.
                if let Ok(opened) = open_enrollment(&event, secret, host, unix_time()?, policy) {
                    found.push(opened);
                }
            }
        }
        socket.close().await.map_err(transport)?;
        Ok(found)
    })
    .await
    .map_err(|_| Error::new(Code::Transport, "enrollment fetch timed out"))?
}
