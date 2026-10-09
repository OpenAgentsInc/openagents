//! A create-only public-intake capability over the same private pipeline.
//! The owner accepts standing responsibility; visitors grant bounded email
//! follow-up permission. Neither permission authorizes outbound automation.

use super::*;

pub const OFFER: &str = "openagents.sales.coder-pilot.v1";
pub const POLICY_SCHEMA: &str = "openagents.sales.intake-policy.v1";
pub const USE: &str = "Review this Coder pilot request and follow up by email about this request only; no marketing, model disclosure, training, or public examples";
pub const MAX_RETENTION: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub id: String,
    pub offer: String,
    /// Exact public origin; HTTPS except for isolated loopback development.
    pub origin: String,
    pub public_owner: String,
    pub support_email: String,
    pub commercial_approval: String,
    pub responsibility_acceptance: String,
    pub consent_version: String,
    pub expires_at: u64,
    pub retention_seconds: u64,
    pub review_within_seconds: u64,
    /// Lifetime admission cap. Deletion and duplicate requests do not reset it.
    pub max_leads: u64,
}

#[cfg(test)]
mod tests;

pub fn email(value: &str) -> Result<String> {
    if value.is_empty()
        || value.len() > 254
        || !value.is_ascii()
        || value
            .bytes()
            .any(|b| b <= b' ' || b >= 127 || matches!(b, b'<' | b'>' | b'"' | b':' | b'\\'))
        || value.matches('@').count() != 1
    {
        return Err("invalid bounded email address".into());
    }
    let (local, domain) = value.split_once('@').unwrap();
    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || !domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
    {
        return Err("invalid bounded email address".into());
    }
    Ok(value.to_ascii_lowercase())
}

fn valid_origin(value: &str) -> bool {
    let (scheme, authority) = match value.split_once("://") {
        Some(parts) => parts,
        None => return false,
    };
    let (host, port) = match authority.split_once(':') {
        Some((host, raw)) => match raw.parse::<u16>() {
            Ok(port) if port > 0 => (host, Some(port)),
            _ => return false,
        },
        None => (authority, None),
    };
    if host.is_empty()
        || host != host.to_ascii_lowercase()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
    {
        return false;
    }
    match scheme {
        // Explicit non-default ports keep browser origin serialization exact.
        "http" => matches!(host, "127.0.0.1" | "localhost") && port.is_some_and(|p| p != 80),
        "https" => port != Some(443),
        _ => false,
    }
}

impl Policy {
    fn validate(&self, now: u64) -> Result<()> {
        id(&self.id)?;
        id(&self.consent_version)?;
        for value in [&self.commercial_approval, &self.responsibility_acceptance] {
            text(value, 256)?;
        }
        text(&self.public_owner, 80)?;
        email(&self.support_email)?;
        if self.schema != POLICY_SCHEMA
            || self.offer != OFFER
            || !valid_origin(&self.origin)
            || self.expires_at <= now
            || self.expires_at > now.saturating_add(90 * 24 * 60 * 60)
            || self.retention_seconds == 0
            || self.retention_seconds % 86400 != 0
            || self.retention_seconds > MAX_RETENTION
            || self.review_within_seconds == 0
            || self.review_within_seconds > 7 * 24 * 60 * 60
            || self.review_within_seconds > self.retention_seconds
            || !(1..=32).contains(&self.max_leads)
        {
            return Err("invalid or expired intake policy".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Grant {
    policy: Policy,
    owner: String,
    token_digest: String,
    active: bool,
    admitted: u64,
}

/// A credential-derived capability that cannot read or update private leads.
pub struct Access {
    id: String,
    token_digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Immutable grant terms retained in the canonical pipeline state.
    pub policy: String,
    pub offer: String,
    pub origin: String,
    pub consent_version: String,
    /// An unverified attribution identifier, never an earned commission.
    pub referral: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    pub request: String,
    pub issued_at: u64,
    pub email: String,
    pub account: String,
    pub jurisdiction: String,
    pub workflow: String,
    pub referral: Option<String>,
    pub consent_version: String,
    pub consent: bool,
}

/// Safe to show the submitting browser: no private record or contact identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Acknowledgment {
    pub reference: String,
    pub received_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RecordedSubmission {
    digest: String,
    acknowledgment: Acknowledgment,
}

impl Store {
    /// The authenticated pipeline owner accepts responsibility for all requests
    /// within these fixed terms. The web process receives only the new token.
    pub fn issue_intake(
        &mut self,
        owner: &super::Access,
        policy: Policy,
        credential: &Path,
    ) -> Result<()> {
        self.refresh()?;
        self.admin(owner)?;
        policy.validate((self.clock)())?;
        let secret = self.credential(credential)?;
        let hash = digest(secret.as_bytes());
        if self
            .state
            .principals
            .values()
            .any(|p| p.token_digest == hash)
        {
            return Err("intake must use a distinct create-only credential".into());
        }
        if let Some(prior) = self.state.intakes.get(&policy.id) {
            return if prior.active
                && prior.owner == owner.principal
                && prior.token_digest == hash
                && prior.policy == policy
            {
                Ok(())
            } else {
                Err("intake policy identity is already recorded".into())
            };
        }
        if self.state.intakes.len() >= MAX_PRINCIPALS
            || self.state.intakes.values().any(|p| p.token_digest == hash)
        {
            return Err("intake grant bound or credential conflict".into());
        }
        let mut next = self.state.clone();
        next.intakes.insert(
            policy.id.clone(),
            Grant {
                policy,
                owner: owner.principal.clone(),
                token_digest: hash,
                active: true,
                admitted: 0,
            },
        );
        self.persist(next)
    }

    pub fn revoke_intake(&mut self, owner: &super::Access, id: &str) -> Result<()> {
        self.refresh()?;
        self.admin(owner)?;
        let mut next = self.state.clone();
        next.intakes
            .get_mut(id)
            .ok_or("intake is unavailable")?
            .active = false;
        self.persist(next)
    }

    pub fn authenticate_intake(&mut self, secret: &str) -> Result<Access> {
        self.refresh()?;
        token(secret)?;
        let hash = digest(secret.as_bytes());
        let (id, _) = self
            .state
            .intakes
            .iter()
            .find(|(_, g)| g.active && g.token_digest == hash)
            .ok_or("intake is unavailable")?;
        let access = Access {
            id: id.clone(),
            token_digest: hash,
        };
        self.intake_policy(&access)?;
        Ok(access)
    }

    pub fn intake_policy(&mut self, access: &Access) -> Result<Policy> {
        self.refresh()?;
        let grant = self
            .state
            .intakes
            .get(&access.id)
            .filter(|g| g.active && g.token_digest == access.token_digest)
            .ok_or("intake is unavailable")?;
        if !self
            .state
            .principals
            .get(&grant.owner)
            .is_some_and(|p| p.active && p.role == Role::Owner)
        {
            return Err("intake responsibility is unavailable".into());
        }
        grant.policy.validate((self.clock)())?;
        Ok(grant.policy.clone())
    }

    pub fn submit_intake(
        &mut self,
        access: &Access,
        submission: &Submission,
    ) -> Result<Acknowledgment> {
        let policy = self.intake_policy(access)?;
        let bytes = serde_json::to_vec(submission).map_err(|_| "invalid intake submission")?;
        if bytes.len() > 8192 {
            return Err("intake submission exceeds bound".into());
        }
        token(&submission.request)?;
        let key = digest(
            format!(
                "{}:intake:{}:{}",
                self.state.salt, access.id, submission.request
            )
            .as_bytes(),
        );
        let input_digest =
            digest(format!("{}:{}", self.state.salt, String::from_utf8_lossy(&bytes)).as_bytes());
        if let Some(prior) = self.state.intake_submissions.get(&key) {
            return if prior.digest == input_digest {
                Ok(prior.acknowledgment.clone())
            } else {
                Err("intake retry changed its submission".into())
            };
        }
        let now = (self.clock)();
        if !submission.consent
            || submission.consent_version != policy.consent_version
            || submission.issued_at > now
            || now - submission.issued_at > 30 * 60
        {
            return Err("intake needs current explicit consent and a fresh request".into());
        }
        let address = format!("email:{}", email(&submission.email)?);
        text(&submission.account, 128)?;
        text(&submission.jurisdiction, 128)?;
        text(&submission.workflow, 2048)?;
        if let Some(referral) = &submission.referral {
            id(referral)?;
        }
        if self
            .state
            .suppressions
            .contains_key(&Self::suppression(&self.state, &address)?)
        {
            return Err("intake is unavailable for this request".into());
        }
        if self.state.receipts.len() >= self.ordinary_history_limit(0)
            || self.state.audit.len() >= self.ordinary_history_limit(0)
            || self.state.intake_submissions.len() >= MAX_RECEIPTS - MAX_LEADS
        {
            return Err("intake history bound reached".into());
        }
        let duplicate = self
            .state
            .leads
            .values()
            .find(|l| {
                contact(&l.contact).ok().as_deref() == Some(address.as_str())
                    && l.intake.as_ref().is_none_or(|p| p.offer == policy.offer)
            })
            .map(|l| l.id.clone());
        if duplicate.as_ref().is_some_and(|id| {
            let existing = &self.state.leads[id];
            existing.intake.is_none()
                || existing.details.stage == Stage::Closed
                || existing.details.permission.state != PermissionState::Granted
                || existing.details.permission.expires_at <= now
        }) {
            return Err("intake is unavailable for this request".into());
        }
        let grant = &self.state.intakes[&access.id];
        if duplicate.is_none()
            && (grant.admitted >= policy.max_leads || self.state.leads.len() >= MAX_LEADS)
        {
            return Err("intake admission cap reached".into());
        }
        let owner = grant.owner.clone();
        let mut next = self.state.clone();
        let lead_id = duplicate.clone().unwrap_or_else(|| format!("lead_{key}"));
        if duplicate.is_none() {
            let until = now
                .checked_add(policy.retention_seconds)
                .ok_or("intake retention overflow")?;
            let details = Details {
                account: submission.account.clone(), jurisdiction: submission.jurisdiction.clone(), scope: None,
                permission: Permission { state: PermissionState::Granted, reference: format!("self-asserted-intake:{}:{}", policy.consent_version, submission.request), recorded_at: now, expires_at: until, channels: vec!["email".into()] },
                workflow: submission.workflow.clone(), baseline_reference: "No comparative evidence supplied; scope and installation need private qualification".into(),
                data: DataBoundary { recipients: vec![format!("human:{owner}")], permitted_use: USE.into(), retain_until: until },
                stage: Stage::New, next: Some(NextAction { description: "Human reviews this pilot request and follows up only under recorded email permission".into(), due_at: now + policy.review_within_seconds }), customer_decision: None, readers: vec![],
            };
            validate(&details, now)?;
            self.validate_readers(&details, &owner)?;
            next.leads.insert(
                lead_id.clone(),
                Lead {
                    schema: LEAD_SCHEMA.into(),
                    id: lead_id.clone(),
                    revision: 1,
                    contact: address,
                    source: format!(
                        "Permissioned public pilot request at {}/pilot",
                        policy.origin
                    ),
                    source_at: submission.issued_at,
                    created_at: now,
                    updated_at: now,
                    responsible_human: owner.clone(),
                    ownership_acceptance: policy.responsibility_acceptance.clone(),
                    details,
                    proposed_handoff: None,
                    service_sales: BTreeMap::new(),
                    offboarding: BTreeMap::new(),
                    partner_assignments: BTreeMap::new(),
                    funnel_journeys: BTreeMap::new(),
                    agent_records: super::agents::LeadRecords::default(),
                    intake: Some(Provenance {
                        policy: policy.id.clone(),
                        offer: policy.offer,
                        origin: policy.origin,
                        consent_version: policy.consent_version,
                        referral: submission.referral.clone(),
                    }),
                    acquisition: None,
                },
            );
            next.intakes.get_mut(&access.id).unwrap().admitted += 1;
        }
        next.sequence = next
            .sequence
            .checked_add(1)
            .ok_or("sales sequence overflow")?;
        let acknowledgment = Acknowledgment {
            reference: submission.request.clone(),
            received_at: now,
        };
        // The private audit names the canonical record; public acknowledgment
        // stays identical for new and duplicate requests and reveals no lead ID.
        let receipt = Receipt {
            schema: RECEIPT_SCHEMA.into(),
            command_digest: key.clone(),
            lead: lead_id.clone(),
            revision: next.leads[&lead_id].revision,
            sequence: next.sequence,
            at: now,
            outcome: "intake_received".into(),
        };
        next.receipts.insert(
            key.clone(),
            Recorded {
                actor: owner.clone(),
                input_digest: input_digest.clone(),
                receipt,
            },
        );
        next.audit.push(Audit {
            sequence: next.sequence,
            at: now,
            actor: owner,
            lead: lead_id,
            operation: "intake_received".into(),
            reference_digest: digest(submission.request.as_bytes()),
        });
        next.intake_submissions.insert(
            key,
            RecordedSubmission {
                digest: input_digest,
                acknowledgment: acknowledgment.clone(),
            },
        );
        self.persist(next)?;
        Ok(acknowledgment)
    }
}
