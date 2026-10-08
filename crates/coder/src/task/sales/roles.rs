//! Arthur's partner desk and Vanna's affiliate desk (REV-72). Each role reads
//! the canonical pipeline through its own binding and crew anchor. A brief,
//! an attribution view, or a review finding grants no contract, discount,
//! fulfillment, earnings promise, or payout authority.

use super::*;
use crate::task::agent;
use agents::Anchor;
use coder_host::access::crew::JobRole;
use receipts::service_sale::Reference;
use tenancy::accounts::referrals::{Kind, Outcome};

pub const SCHEMA: &str = "openagents.sales-role-desk.v1";
pub const MAX_ROWS: usize = 256;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Desk {
    Arthur,
    Vanna,
}
impl Desk {
    pub fn name(self) -> &'static str {
        match self {
            Self::Arthur => "arthur",
            Self::Vanna => "vanna",
        }
    }
    pub fn job_role(self) -> JobRole {
        match self {
            Self::Arthur => JobRole::SalesPartner,
            Self::Vanna => JobRole::SalesAffiliate,
        }
    }
    pub fn parse(text: &str) -> Result<Self> {
        match text {
            "arthur" => Ok(Self::Arthur),
            "vanna" => Ok(Self::Vanna),
            _ => Err("desk is arthur or vanna".into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub schema: String,
    pub desk: Desk,
    pub revision: u64,
    pub anchor: Anchor,
    pub owner_credential: PathBuf,
    /// The human who owns growth decisions: contracts, terms, and payouts.
    pub growth_owner: String,
    /// Daily model ceiling for this desk; a zero ceiling means research only.
    pub daily_usd_millionths: u64,
}
impl Binding {
    pub fn sha256(&self) -> Result<String> {
        if self.schema != SCHEMA
            || self.revision == 0
            || self.anchor.name != self.desk.name()
            || self.anchor.role != self.desk.job_role()
            || !self.owner_credential.is_absolute()
            || self.daily_usd_millionths > 5_000_000
        {
            return Err("desk binding needs the desk's own anchor, an absolute owner credential, and a ceiling at or under the floor's USD 5".into());
        }
        id(&self.growth_owner)?;
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "desk binding serialization failed")?,
        ))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub bindings: BTreeMap<Desk, Binding>,
    pub owner: Option<String>,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if !self.bindings.is_empty() && self.owner.is_none() {
            return Err("desk ownership is incomplete".into());
        }
        for (desk, b) in &self.bindings {
            if *desk != b.desk {
                return Err("desk binding is filed under another desk".into());
            }
            b.sha256()?;
        }
        if let Some(o) = &self.owner {
            id(o)?;
        }
        Ok(())
    }
}

/// What a desk may truthfully say about money today. Every field is false
/// until the owner publishes terms and qualifies accounting; the desk cannot
/// change them.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub terms_published: bool,
    pub commissions_available: bool,
    pub payouts_available: bool,
    pub contracts_by_agent: bool,
    pub discounts_by_agent: bool,
    pub fulfillment_by_agent: bool,
}
impl Limits {
    pub const DISCLOSURE: &'static str = "Commission and payout terms are not published; no earnings, contract, discount, or fulfillment is promised. The growth owner decides each.";
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Offering {
    pub lead_reference: String,
    pub assignment: String,
    pub kind: String,
    pub status: partners::Status,
    pub brief: Reference,
    pub handoff_pending: bool,
    pub owner_human: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Brief {
    pub schema: String,
    pub desk: Desk,
    pub binding_sha256: String,
    pub generated_at: u64,
    pub growth_owner: String,
    pub limits: Limits,
    pub disclosure: String,
    pub offerings: Vec<Offering>,
    pub introductions_awaiting_owner: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Earns {
    /// A published-terms referral by an outside person, partner, or author.
    ReferralWhenTermsPublish,
    /// Our own agent's link: attribution only, never a commission.
    Nothing,
    /// No referral captured, declined, disabled, or malformed.
    NotAttributed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Finding {
    SelfReferral,
    RepeatedReferrer,
    MalformedSource,
    UnknownSource,
    RecycledSettlement,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Attribution {
    pub lead_reference: String,
    pub outcome: Outcome,
    pub referrer_kind: Option<Kind>,
    pub referrer_reference: Option<String>,
    pub earns: Earns,
    pub settled_sales: u64,
    pub reversed_sales: u64,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttributionView {
    pub schema: String,
    pub desk: Desk,
    pub binding_sha256: String,
    pub generated_at: u64,
    pub growth_owner: String,
    pub limits: Limits,
    pub disclosure: String,
    pub rows: Vec<Attribution>,
    pub under_review: usize,
    pub payout_authority: bool,
}

/// What a desk's memory may carry: references and counts, never names,
/// contacts, or amounts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    pub schema: String,
    pub desk: Desk,
    pub binding_sha256: String,
    pub references: Vec<String>,
    pub lesson: &'static str,
}

fn reference(prefix: &str, value: &str) -> String {
    format!("{prefix}:{}", &digest(value.as_bytes())[..16])
}

impl Store {
    fn desk_owner(&mut self, binding: &Binding) -> Result<Access> {
        let owner = self.authenticate(&Self::read_credential(&binding.owner_credential)?)?;
        self.admin(&owner)?;
        if self
            .state
            .roles
            .owner
            .as_deref()
            .is_some_and(|p| p != owner.principal)
        {
            return Err("the desks' original owner changed".into());
        }
        if self.sales_agent_anchor(&owner, binding.desk.name())? != binding.anchor {
            return Err("the desk's current key or charter changed".into());
        }
        let root = self.dir.parent().ok_or("sales host root is unavailable")?;
        let native = agent::Store::with_keys(root, binding.desk.name(), self.native_keys.clone())?;
        let record = native.load()?.ok_or("the desk's agent is unavailable")?;
        crate::task::agent_memory::Memory::new(native.clone(), secret_screen::Screen::host())
            .entries()
            .map_err(|_| "the desk's memory is unreadable; repair it before resuming")?;
        if record.job_role != Some(binding.desk.job_role())
            || record.state != agent::State::Active
            || record.crew_charter.as_ref().is_none_or(|c| !c.drafting)
        {
            return Err("the desk's native role is stopped, paused, or holds another job".into());
        }
        Ok(owner)
    }

    pub fn configure_desk(
        &mut self,
        owner: &Access,
        binding: &Binding,
        approval: &str,
    ) -> Result<String> {
        self.refresh()?;
        self.admin(owner)?;
        let sha = binding.sha256()?;
        let next_revision = self
            .state
            .roles
            .bindings
            .get(&binding.desk)
            .map(|b| {
                b.revision
                    .checked_add(1)
                    .ok_or("desk binding revision overflow")
            })
            .transpose()?
            .unwrap_or(1);
        if sha != approval || binding.revision != next_revision {
            return Err("approve the exact next desk binding revision".into());
        }
        let original = self.desk_owner(binding)?;
        if original.principal != owner.principal {
            return Err("desk binding owner credential does not match the caller".into());
        }
        let mut next = self.state.clone();
        next.roles.bindings.insert(binding.desk, binding.clone());
        next.roles.owner = Some(owner.principal.clone());
        self.persist(next)?;
        Ok(sha)
    }

    fn current_desk(&mut self, owner: &Access, desk: Desk) -> Result<Binding> {
        self.refresh()?;
        self.admin(owner)?;
        let binding = self
            .state
            .roles
            .bindings
            .get(&desk)
            .cloned()
            .ok_or("the desk has no approved binding")?;
        self.desk_owner(&binding)?;
        Ok(binding)
    }

    /// Arthur's brief: cited partner offerings from accepted or proposed
    /// assignments, with the money limitation stated as it stands.
    pub fn partner_brief(&mut self, owner: &Access) -> Result<Brief> {
        let binding = self.current_desk(owner, Desk::Arthur)?;
        let now = (self.clock)();
        let mut offerings = Vec::new();
        let mut awaiting = 0;
        for lead in self.state.leads.values() {
            for (id, a) in &lead.partner_assignments {
                if offerings.len() >= MAX_ROWS {
                    return Err("partner brief exceeds the row bound".into());
                }
                let brief = match &a.proposal.terms {
                    partners::Terms::Discovery { brief, .. } => brief.clone(),
                    partners::Terms::Fulfillment { brief, .. } => brief.clone(),
                };
                let handoff_pending = a.handoff.as_ref().is_some_and(|h| h.expires_at > now);
                awaiting += usize::from(handoff_pending);
                offerings.push(Offering {
                    lead_reference: reference("lead", &lead.id),
                    assignment: id.clone(),
                    kind: match a.proposal.terms {
                        partners::Terms::Discovery { .. } => "discovery".into(),
                        partners::Terms::Fulfillment { .. } => "fulfillment".into(),
                    },
                    status: a.status,
                    brief,
                    handoff_pending,
                    owner_human: a.owner_human.clone(),
                });
            }
        }
        Ok(Brief {
            schema: SCHEMA.into(),
            desk: Desk::Arthur,
            binding_sha256: binding.sha256()?,
            generated_at: now,
            growth_owner: binding.growth_owner,
            limits: Limits::default(),
            disclosure: Limits::DISCLOSURE.into(),
            offerings,
            introductions_awaiting_owner: awaiting,
        })
    }

    /// Vanna's attribution view over the canonical acquisition sources, with
    /// bounded abuse findings and no payout authority.
    pub fn attribution_view(&mut self, owner: &Access) -> Result<AttributionView> {
        let binding = self.current_desk(owner, Desk::Vanna)?;
        let now = (self.clock)();
        let mut referrer_leads: BTreeMap<String, u64> = BTreeMap::new();
        for lead in self.state.leads.values() {
            if let Some(r) = lead
                .acquisition
                .as_ref()
                .and_then(|a| a.source.referrer.as_ref())
            {
                *referrer_leads.entry(r.id.clone()).or_default() += 1;
            }
        }
        let mut rows = Vec::new();
        for lead in self.state.leads.values() {
            if rows.len() >= MAX_ROWS {
                return Err("attribution view exceeds the row bound".into());
            }
            let Some(acq) = &lead.acquisition else {
                continue;
            };
            let source = &acq.source;
            let mut findings = Vec::new();
            let mut settled = 0;
            let mut reversed = 0;
            for sale in lead.service_sales.values() {
                let summary = sale
                    .summary()
                    .map_err(|_| "canonical sale summary failed")?;
                settled += u64::from(summary.paid_minor > 0);
                reversed += u64::from(summary.refunded_minor > 0);
                if summary.refunded_minor > summary.paid_minor {
                    findings.push(Finding::RecycledSettlement);
                }
            }
            let earns = match (source.outcome, &source.referrer) {
                (Outcome::Captured, Some(r)) if r.kind == Kind::Agent => Earns::Nothing,
                (Outcome::Captured, Some(_)) => Earns::ReferralWhenTermsPublish,
                _ => Earns::NotAttributed,
            };
            match source.outcome {
                Outcome::Malformed => findings.push(Finding::MalformedSource),
                Outcome::Unknown => findings.push(Finding::UnknownSource),
                _ => {}
            }
            if let Some(r) = &source.referrer {
                if r.id == lead.details.account || r.id == source.account {
                    findings.push(Finding::SelfReferral);
                }
                if referrer_leads.get(&r.id).copied().unwrap_or(0) > 3 {
                    findings.push(Finding::RepeatedReferrer);
                }
            }
            findings.sort();
            findings.dedup();
            rows.push(Attribution {
                lead_reference: reference("lead", &lead.id),
                outcome: source.outcome,
                referrer_kind: source.referrer.as_ref().map(|r| r.kind),
                referrer_reference: source
                    .referrer
                    .as_ref()
                    .map(|r| reference("referrer", &r.id)),
                earns,
                settled_sales: settled,
                reversed_sales: reversed,
                findings,
            });
        }
        let under_review = rows.iter().filter(|r| !r.findings.is_empty()).count();
        Ok(AttributionView {
            schema: SCHEMA.into(),
            desk: Desk::Vanna,
            binding_sha256: binding.sha256()?,
            generated_at: now,
            growth_owner: binding.growth_owner,
            limits: Limits::default(),
            disclosure: Limits::DISCLOSURE.into(),
            rows,
            under_review,
            payout_authority: false,
        })
    }

    /// The desk's memory projection: references only.
    pub fn desk_memory(&mut self, owner: &Access, desk: Desk) -> Result<Memory> {
        let binding = self.current_desk(owner, desk)?;
        let references = match desk {
            Desk::Arthur => self
                .partner_brief(owner)?
                .offerings
                .iter()
                .map(|o| o.lead_reference.clone())
                .collect(),
            Desk::Vanna => self
                .attribution_view(owner)?
                .rows
                .iter()
                .map(|r| r.lead_reference.clone())
                .collect(),
        };
        Ok(Memory {
            schema: SCHEMA.into(),
            desk,
            binding_sha256: binding.sha256()?,
            references,
            lesson: "owner review required before any contract, terms, or payout",
        })
    }
}

#[cfg(test)]
mod tests;
