//! Host invitations (host shows, device redeems) and reverse enrollment
//! (host shows a short code, an administrator approves the exact request).
use super::*;
use coder_connect::pairing::Invitation;
use nostr::contracts::digest_bytes;

const MAX_INVITATION_REPLIES: u32 = 32;
const MAX_ENROLLMENT_RECIPIENTS: usize = 16;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InvitationRecord {
    capability_digest: String,
    relay: String,
    issued_at: u64,
    pub(super) expires_at: u64,
    rights: Rights,
    grant_expires_at: u64,
    issuer: String,
    issuer_grant: Option<String>,
    cancelled: bool,
    pub(super) grant: Option<String>,
    replies: u32,
}

impl InvitationRecord {
    /// See `reparent` in the host module.
    pub(super) fn reparent(&mut self, device: &str, handed_on: &[String], current: &Grant) {
        let covered = current.rights.contains(Right::AccessAdmin)
            && self.rights.first_missing(&current.rights).is_none()
            && self.grant_expires_at <= current.expires_at;
        if self.issuer == device
            && self.grant.is_none()
            && !self.cancelled
            && covered
            && self
                .issuer_grant
                .as_ref()
                .is_some_and(|g| handed_on.contains(g))
        {
            self.issuer_grant = Some(current.grant.clone());
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum EnrollmentState {
    Pending {},
    Approved {
        device: String,
        grant: String,
        approver: String,
    },
    Denied {
        by: String,
    },
    /// Too many wrong codes. The request can never be approved.
    Closed {},
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EnrollmentRecord {
    artifact_digest: String,
    code_digest: String,
    relay: String,
    rights: Rights,
    issued_at: u64,
    pub(super) expires_at: u64,
    attempts: u32,
    state: EnrollmentState,
}

impl EnrollmentRecord {
    pub(super) fn approved_grant(&self) -> Option<String> {
        match &self.state {
            EnrollmentState::Approved { grant, .. } => Some(grant.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnrollmentStatus {
    Pending,
    Approved { device: String, grant: String },
    Denied,
    Closed,
    Expired,
}

/// Contains the invitation's capability. Show it only to the enrolling device.
pub struct IssuedInvitation {
    pub id: String,
    pub code: String,
    pub expires_at: u64,
}
/// The short code is shown on the host's own screen, never published.
pub struct PendingEnrollment {
    pub id: String,
    pub code: String,
    pub expires_at: u64,
    /// One sealed copy per recipient: the owner and each current administrator.
    pub events: Vec<Event>,
}

impl Host {
    /// A local operator invitation. It is persisted before this returns, so
    /// the caller can display it; single use, five-minute lifetime.
    pub fn invite(
        &self,
        relay: &str,
        rights: Rights,
        now: u64,
        grant_expires_at: u64,
    ) -> Result<IssuedInvitation> {
        self.policy.validate(relay).map_err(Error::from)?;
        let (mut store, _, mut book) = self.open()?;
        let host = book.host.clone();
        let issued =
            create_invitation(&mut book, relay, rights, grant_expires_at, now, &host, None)?;
        store.save(&book)?;
        Ok(issued)
    }
    /// Cancel an unused invitation. A redeemed one needs device revocation.
    pub fn cancel_invitation(&self, id: &str) -> Result<()> {
        let (mut store, _, mut book) = self.open()?;
        cancel(&mut book, id)?;
        Ok(store.save(&book)?)
    }
    /// Start reverse enrollment. The request is persisted before the code is
    /// returned for display. Publish the returned events to the relay.
    pub fn request_enrollment(
        &self,
        relay: &str,
        rights: Rights,
        now: u64,
    ) -> Result<PendingEnrollment> {
        self.policy.validate(relay).map_err(Error::from)?;
        let (mut store, secret, mut book) = self.open()?;
        book.prune(now);
        if book.enrollments.len() >= MAX_ENROLLMENTS {
            return fail(Code::Bounds, "enrollment request retention limit reached");
        }
        let enrollment = Enrollment {
            v: ENROLLMENT.into(),
            requires: vec![],
            enrollment: random_id(),
            host: book.host.clone(),
            owner: book.owner.clone(),
            relay: relay.into(),
            rights: rights.clone(),
            issued_at: now,
            expires_at: now + ENROLLMENT_LIFETIME,
        };
        enrollment.validate(self.policy)?;
        let code = short_code();
        let mut recipients = vec![book.owner.clone()];
        for record in book.grants.values() {
            let g = &record.grant;
            if record.revoked_at.is_none()
                && g.expires_at > now
                && g.epoch == book.epoch(&g.device)
                && g.rights.contains(Right::AccessAdmin)
                && !recipients.contains(&g.device)
                && recipients.len() < MAX_ENROLLMENT_RECIPIENTS
            {
                recipients.push(g.device.clone());
            }
        }
        let events = recipients
            .iter()
            .map(|recipient| {
                seal(
                    &enrollment,
                    ENROLLMENT,
                    &secret,
                    recipient,
                    &enrollment.enrollment,
                    now,
                    enrollment.expires_at,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        book.enrollments.insert(
            enrollment.enrollment.clone(),
            EnrollmentRecord {
                artifact_digest: enrollment.digest()?,
                code_digest: code_digest(&enrollment.enrollment, &code)?,
                relay: relay.into(),
                rights,
                issued_at: now,
                expires_at: enrollment.expires_at,
                attempts: 0,
                state: EnrollmentState::Pending {},
            },
        );
        store.save(&book)?;
        Ok(PendingEnrollment {
            id: enrollment.enrollment,
            code,
            expires_at: enrollment.expires_at,
            events,
        })
    }
    pub fn enrollment_status(&self, id: &str, now: u64) -> Result<EnrollmentStatus> {
        let (_, _, book) = self.open()?;
        let record = book
            .enrollments
            .get(id)
            .ok_or_else(|| Error::new(Code::Unavailable, "enrollment request is not retained"))?;
        Ok(match &record.state {
            EnrollmentState::Pending {} if now >= record.expires_at => EnrollmentStatus::Expired,
            EnrollmentState::Pending {} => EnrollmentStatus::Pending,
            EnrollmentState::Approved { device, grant, .. } => EnrollmentStatus::Approved {
                device: device.clone(),
                grant: grant.clone(),
            },
            EnrollmentState::Denied { .. } => EnrollmentStatus::Denied,
            EnrollmentState::Closed {} => EnrollmentStatus::Closed,
        })
    }

    /// Redemption is capability-based. An unknown invitation or wrong
    /// capability gets no signed reply at all.
    pub(super) fn redeem(
        &self,
        book: &mut Book,
        secret: &SecretKey,
        request: &Request,
        signer: &str,
        now: u64,
        retained: Option<&Retained>,
    ) -> Result<Step> {
        let Operation::Redeem {
            invitation: id,
            capability,
        } = &request.op
        else {
            return fail(Code::Malformed, "not a redemption");
        };
        let record = book
            .invitations
            .get(id)
            .cloned()
            .ok_or_else(|| Error::new(Code::Forbidden, "invitation is not admitted"))?;
        if !same_digest(
            &record.capability_digest,
            &digest_bytes(capability.as_bytes()),
        ) || record.relay != request.relay
        {
            return fail(Code::Forbidden, "invitation capability or relay differs");
        }
        let state = if signer == book.owner || signer == book.host {
            Err(Error::new(
                Code::Forbidden,
                "owner and host keys cannot redeem",
            ))
        } else if record.cancelled {
            Err(Error::new(Code::Revoked, "invitation was cancelled"))
        } else if now >= record.expires_at || now >= record.grant_expires_at {
            Err(Error::new(Code::Expired, "invitation has expired"))
        } else if request.issued_at < record.issued_at || request.expires_at > record.expires_at {
            Err(Error::new(
                Code::Forbidden,
                "request is outside the invitation",
            ))
        } else if !book.issuer_current(
            &record.issuer,
            record.issuer_grant.as_deref(),
            &record.rights,
            now,
        ) {
            Err(Error::new(
                Code::Revoked,
                "invitation issuer no longer holds its rights",
            ))
        } else if let Some(grant) = &record.grant {
            match book.grants.get(grant) {
                None => Err(Error::new(
                    Code::Unavailable,
                    "consumed grant is unavailable",
                )),
                Some(r) if r.grant.device != signer => Err(Error::new(
                    Code::Forbidden,
                    "invitation was redeemed by another device",
                )),
                Some(r) if r.revoked_at.is_some() => {
                    Err(Error::new(Code::Revoked, "redeemed grant is revoked"))
                }
                Some(r) => Ok(Some(r.authorization.clone())),
            }
        } else {
            Ok(None)
        };
        let existing = match state {
            Err(error) => return Ok(Step::Reply(refused(error), false)),
            Ok(existing) => existing,
        };
        if let Some(Retained {
            reply: Some(reply), ..
        }) = retained
        {
            return Ok(Step::Retained(reply.clone()));
        }
        if record.replies >= MAX_INVITATION_REPLIES {
            let error = Error::new(Code::RateLimited, "invitation reply limit reached");
            return Ok(Step::Reply(refused(error), false));
        }
        let authorization = match existing {
            Some(authorization) => authorization,
            None => {
                let origin = Origin {
                    kind: OriginKind::Invitation,
                    id: id.clone(),
                    issuer: record.issuer.clone(),
                };
                match issue(
                    book,
                    secret,
                    signer,
                    &record.relay,
                    record.rights.clone(),
                    origin,
                    now,
                    record.grant_expires_at,
                ) {
                    Ok(authorization) => authorization,
                    Err(error) => return Ok(Step::Reply(refused(error), false)),
                }
            }
        };
        let stored = book.invitations.get_mut(id).expect("validated invitation");
        stored.grant = authorization.tag_values("h").next().map(str::to_owned);
        stored.replies += 1;
        Ok(Step::Reply(
            ReplyResult::Ok {
                outcome: Outcome::Granted {
                    authorization: Box::new(authorization),
                },
            },
            true,
        ))
    }
}

fn create_invitation(
    book: &mut Book,
    relay: &str,
    rights: Rights,
    grant_expires_at: u64,
    now: u64,
    issuer: &str,
    issuer_grant: Option<String>,
) -> Result<IssuedInvitation> {
    window(now, grant_expires_at, MAX_GRANT_LIFETIME)?;
    let invitation = Invitation::issue(&book.host, relay, now)?;
    if grant_expires_at <= invitation.expires_at {
        return fail(
            Code::Expired,
            "the grant must outlive the five-minute invitation",
        );
    }
    book.prune(now);
    if book.invitations.len() >= MAX_INVITATIONS {
        return fail(Code::Bounds, "invitation retention limit reached");
    }
    let code = invitation.encode_prefixed(INVITATION_PREFIX)?;
    book.invitations.insert(
        invitation.id.clone(),
        InvitationRecord {
            capability_digest: digest_bytes(invitation.capability().as_bytes()),
            relay: relay.into(),
            issued_at: now,
            expires_at: invitation.expires_at,
            rights,
            grant_expires_at,
            issuer: issuer.into(),
            issuer_grant,
            cancelled: false,
            grant: None,
            replies: 0,
        },
    );
    Ok(IssuedInvitation {
        id: invitation.id.clone(),
        code,
        expires_at: invitation.expires_at,
    })
}

/// Delegation: an administrator invites only for rights it holds, and the
/// resulting grant cannot outlive the administrator's own grant.
pub(super) fn remote_invite(
    book: &mut Book,
    request: &Request,
    p: &Principal,
    rights: &Rights,
    grant_expires_at: u64,
    now: u64,
) -> std::result::Result<Outcome, Error> {
    if let Some(right) = rights.first_missing(&p.rights) {
        return Err(Error::missing(right));
    }
    if grant_expires_at > p.expires_at {
        return Err(Error::new(
            Code::Forbidden,
            "a delegated grant cannot outlive its issuer",
        ));
    }
    let issued = create_invitation(
        book,
        &request.relay,
        rights.clone(),
        grant_expires_at,
        now,
        &p.key,
        p.grant.clone(),
    )?;
    Ok(Outcome::Invitation {
        invitation: issued.id,
        code: issued.code,
        expires_at: issued.expires_at,
    })
}

pub(super) fn cancel(book: &mut Book, id: &str) -> std::result::Result<Outcome, Error> {
    let record = book
        .invitations
        .get_mut(id)
        .ok_or_else(|| Error::new(Code::Forbidden, "invitation is not retained"))?;
    if record.grant.is_some() {
        return Err(Error::new(
            Code::Conflict,
            "invitation was redeemed; revoke the device instead",
        ));
    }
    record.cancelled = true;
    Ok(Outcome::Cancelled {
        invitation: id.into(),
    })
}

/// Approve or deny one exact reverse-enrollment request.
pub(super) fn decide(
    book: &mut Book,
    secret: &SecretKey,
    request: &Request,
    p: &Principal,
    now: u64,
) -> Result<std::result::Result<Outcome, Error>> {
    let (id, digest) = match &request.op {
        Operation::Approve {
            enrollment,
            request_digest,
            ..
        }
        | Operation::Deny {
            enrollment,
            request_digest,
        } => (enrollment.clone(), request_digest.clone()),
        _ => return fail(Code::Malformed, "not an enrollment decision"),
    };
    let refuse = |code, message: &str| Ok(Err(Error::new(code, message)));
    let Some(record) = book.enrollments.get(&id).cloned() else {
        return refuse(
            Code::Forbidden,
            "enrollment request is not pending at this host",
        );
    };
    if record.relay != request.relay || !same_digest(&record.artifact_digest, &digest) {
        return refuse(Code::Forbidden, "decision names a different request");
    }
    let approving = match &request.op {
        Operation::Approve { device, .. } => Some(device.clone()),
        _ => None,
    };
    match (&approving, &record.state) {
        (_, EnrollmentState::Closed {}) => {
            return refuse(Code::RateLimited, "request closed after wrong codes");
        }
        (None, EnrollmentState::Denied { .. }) => return Ok(Ok(Outcome::Denied {})),
        (_, EnrollmentState::Denied { .. }) => return refuse(Code::Denied, "request was denied"),
        (None, EnrollmentState::Approved { .. }) => {
            return refuse(Code::Conflict, "request was already approved");
        }
        (
            Some(device),
            EnrollmentState::Approved {
                device: d, grant, ..
            },
        ) => {
            if device != d {
                return refuse(Code::Conflict, "request was approved for another device");
            }
            return Ok(match book.grants.get(grant) {
                Some(r) if r.revoked_at.is_none() => Ok(Outcome::Granted {
                    authorization: Box::new(r.authorization.clone()),
                }),
                Some(_) => Err(Error::new(Code::Revoked, "approved grant is revoked")),
                None => Err(Error::new(
                    Code::Expired,
                    "approved grant is no longer retained",
                )),
            });
        }
        (_, EnrollmentState::Pending {}) => {}
    }
    if now >= record.expires_at {
        return refuse(Code::Expired, "enrollment request has expired");
    }
    let Operation::Approve {
        code,
        device,
        rights,
        grant_expires_at,
        ..
    } = &request.op
    else {
        book.enrollments.get_mut(&id).expect("record").state =
            EnrollmentState::Denied { by: p.key.clone() };
        return Ok(Ok(Outcome::Denied {}));
    };
    let matches = code_digest(&id, code).is_ok_and(|d| same_digest(&d, &record.code_digest));
    if !matches {
        let stored = book.enrollments.get_mut(&id).expect("record");
        stored.attempts += 1;
        if stored.attempts >= MAX_CODE_ATTEMPTS {
            stored.state = EnrollmentState::Closed {};
        }
        return refuse(Code::WrongCode, "the short code does not match");
    }
    if *device == book.host || *device == book.owner {
        return refuse(
            Code::Forbidden,
            "the admitted device must be a distinct key",
        );
    }
    if let Some(right) = rights.first_missing(&p.rights) {
        return Ok(Err(Error::missing(right)));
    }
    if rights.first_missing(&record.rights).is_some() {
        return refuse(Code::Forbidden, "rights exceed the host's request");
    }
    if *grant_expires_at <= now
        || *grant_expires_at > now.saturating_add(MAX_GRANT_LIFETIME)
        || *grant_expires_at > p.expires_at
    {
        return refuse(
            Code::Forbidden,
            "grant expiry is outside the approver's bounds",
        );
    }
    let origin = Origin {
        kind: OriginKind::Approval,
        id: id.clone(),
        issuer: p.key.clone(),
    };
    let authorization = match issue(
        book,
        secret,
        device,
        &record.relay,
        rights.clone(),
        origin,
        now,
        *grant_expires_at,
    ) {
        Ok(authorization) => authorization,
        Err(error) => return Ok(Err(error)),
    };
    let grant = authorization
        .tag_values("h")
        .next()
        .map(str::to_owned)
        .unwrap_or_default();
    book.enrollments.get_mut(&id).expect("record").state = EnrollmentState::Approved {
        device: device.clone(),
        grant,
        approver: p.key.clone(),
    };
    Ok(Ok(Outcome::Granted {
        authorization: Box::new(authorization),
    }))
}

pub(super) fn validate(book: &Book, policy: RelayPolicy) -> Result<()> {
    let digest_len = digest_bytes(b"").len();
    for (id, i) in &book.invitations {
        identity(id).map_err(Error::from)?;
        policy.validate(&i.relay).map_err(Error::from)?;
        public(&i.issuer)?;
        window(i.issued_at, i.expires_at, INVITATION_LIFETIME)?;
        window(i.issued_at, i.grant_expires_at, MAX_GRANT_LIFETIME)?;
        if i.capability_digest.len() != digest_len
            || i.replies > MAX_INVITATION_REPLIES
            || i.grant_expires_at <= i.expires_at
            || i.grant
                .as_ref()
                .is_some_and(|g| !book.grants.contains_key(g))
        {
            return fail(
                Code::Malformed,
                "retained invitation differs from its bounds",
            );
        }
    }
    for (id, e) in &book.enrollments {
        identity(id).map_err(Error::from)?;
        policy.validate(&e.relay).map_err(Error::from)?;
        window(e.issued_at, e.expires_at, ENROLLMENT_LIFETIME)?;
        if e.artifact_digest.len() != digest_len
            || e.code_digest.len() != digest_len
            || e.attempts > MAX_CODE_ATTEMPTS
        {
            return fail(
                Code::Malformed,
                "retained enrollment differs from its bounds",
            );
        }
    }
    Ok(())
}
