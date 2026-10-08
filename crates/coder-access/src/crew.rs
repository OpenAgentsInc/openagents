//! Private crew job descriptions and evidence recommendations. These are
//! data, not access grants, approvals, or independently verified judgments.

use serde::{Deserialize, Serialize};

use crate::{Code, Result, fail};

pub const CHARTER_SCHEMA: &str = "openagents.crew-charter.v1";
pub const VERDICT_SCHEMA: &str = "openagents.crew-verdict.v1";
/// Keeps the largest typed collection within a native agent reply.
pub const MAX_VERDICTS: usize = 24;

/// An owner action over a selected sales cohort, distinct from send approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlAction {
    Stop,
    Pause,
    Resume,
}

/// All native sales members, including later members, or a named exact subset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "members",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Selection {
    AllSales,
    Members(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub cohort: String,
    pub selection: Selection,
    pub action: ControlAction,
    pub expected: Option<String>,
    pub reason: String,
}
impl Control {
    pub fn validate(&self) -> Result<()> {
        crate::agent::name(&self.cohort)?;
        text(&self.reason, 512)?;
        if let Selection::Members(members) = &self.selection {
            let unique: std::collections::BTreeSet<_> = members.iter().collect();
            if members.is_empty() || members.len() > 32 || unique.len() != members.len() {
                return fail(
                    Code::Bounds,
                    "Select 1 to 32 distinct native sales members.",
                );
            }
            for member in members {
                crate::agent::name(member)?;
            }
        }
        if self.expected.as_ref().is_some_and(|value| {
            value.strip_prefix("sha256:").is_none_or(|hex| {
                hex.len() != 64
                    || !hex
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        }) || self.action == ControlAction::Resume && self.expected.is_none()
        {
            return fail(
                Code::Malformed,
                "Resume needs the exact current crew-control digest.",
            );
        }
        Ok(())
    }
}

/// A job, separate from NIP-SOV's authority, controller, and custodian.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobRole {
    SalesLead,
    SalesResearcher,
    SalesProspector,
    SalesDemo,
    SalesPartner,
    SalesAffiliate,
}

impl JobRole {
    pub fn name(self) -> &'static str {
        match self {
            Self::SalesLead => "sales-lead",
            Self::SalesResearcher => "sales-researcher",
            Self::SalesProspector => "sales-prospector",
            Self::SalesDemo => "sales-demo",
            Self::SalesPartner => "sales-partner",
            Self::SalesAffiliate => "sales-affiliate",
        }
    }

    pub fn preset(self) -> &'static str {
        match self {
            Self::SalesLead => "paul",
            Self::SalesResearcher => "erin",
            Self::SalesProspector => "frank",
            Self::SalesDemo => "pat",
            Self::SalesPartner => "arthur",
            Self::SalesAffiliate => "vanna",
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        [
            Self::SalesLead,
            Self::SalesResearcher,
            Self::SalesProspector,
            Self::SalesDemo,
            Self::SalesPartner,
            Self::SalesAffiliate,
        ]
        .into_iter()
        .find(|role| role.name() == text)
        .ok_or_else(|| crate::Error::new(Code::Malformed, "Choose a supported sales job role."))
    }
}

/// The enforced initial sales scope. There is deliberately no tool, workspace,
/// sending, payment, publication, or other-member approval capability here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charter {
    pub schema: String,
    pub revision: u64,
    pub drafting: bool,
    pub purpose: String,
}

impl Charter {
    pub fn initial(role: JobRole) -> Self {
        Self {
            schema: CHARTER_SCHEMA.into(),
            revision: 1,
            drafting: true,
            purpose: format!(
                "Draft {} recommendations from the owner's supplied request and this member's private memory.",
                role.name()
            ),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != CHARTER_SCHEMA || self.revision == 0 {
            return fail(
                Code::Malformed,
                "The crew charter schema or revision is unsupported.",
            );
        }
        text(&self.purpose, 2048)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// An opaque host reference. It is never opened or run by this contract.
    pub reference: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    pub kind: String,
    pub reference: String,
    pub revision: u64,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultKind {
    Recommend,
    Revise,
    Refuse,
    NeedsEvidence,
}

/// Owner-recorded recommendation input. A question-set pin does not assert
/// that Jev ran, or turn the recommendation into an approval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictInput {
    pub id: String,
    pub subject: Subject,
    pub evidence: Vec<Evidence>,
    pub result: ResultKind,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_set_sha256: Option<String>,
}

impl VerdictInput {
    pub fn validate(&self) -> Result<()> {
        crate::agent::name(&self.id)?;
        if !matches!(
            self.subject.kind.as_str(),
            "task" | "issue" | "deploy" | "campaign-finding"
        ) || self.subject.revision == 0
        {
            return fail(
                Code::Malformed,
                "A crew verdict needs a supported subject and exact revision.",
            );
        }
        reference(&self.subject.reference)?;
        digest(&self.subject.sha256)?;
        if self.evidence.is_empty() || self.evidence.len() > 16 {
            return fail(
                Code::Bounds,
                "A crew verdict needs 1 to 16 exact evidence references.",
            );
        }
        for evidence in &self.evidence {
            reference(&evidence.reference)?;
            digest(&evidence.sha256)?;
        }
        text(&self.reason, 1024)?;
        if let Some(pin) = &self.question_set_sha256 {
            digest(pin)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verdict {
    pub schema: String,
    pub agent: String,
    pub author: String,
    pub recorded_by: String,
    pub basis: String,
    pub charter_revision: u64,
    pub at: u64,
    pub input: VerdictInput,
    pub signature: String,
}

pub fn digest(text: &str) -> Result<()> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return fail(
            Code::Malformed,
            "Use a 64-character lowercase SHA-256 digest.",
        );
    }
    Ok(())
}

fn reference(value: &str) -> Result<()> {
    text(value, 256)?;
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b":/._-#@".contains(&b))
    {
        return fail(
            Code::Malformed,
            "Use a bounded opaque evidence reference, without instructions or query data.",
        );
    }
    Ok(())
}

fn text(value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return fail(
            Code::Bounds,
            "Crew text is empty, contains control characters, or exceeds its limit.",
        );
    }
    Ok(())
}

pub const HIRE_SCHEMA: &str = "openagents.crew-hire.v1";
/// Paul plus this many active hires, until a separate owner grant.
pub const MAX_ACTIVE_HIRES: usize = 3;
/// The whole floor's daily model ceiling, in millionths of a US dollar.
pub const FLOOR_USD_MILLIONTHS: u64 = 5_000_000;
pub const MAX_HIRE_PROPOSALS: usize = 64;

/// What a hire proposal asks for: a new member, or an existing one's end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum HireAction {
    Hire { name: String, role: JobRole },
    Retire { name: String },
}

/// A durable proposal to hire or retire. Only the owner's decision acts on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HireProposal {
    pub schema: String,
    pub id: String,
    pub action: HireAction,
    /// The member's daily model budget, in millionths of a US dollar.
    pub daily_usd_millionths: u64,
    pub charter_revision: u64,
    pub reason: String,
    pub evidence: Vec<Evidence>,
    pub expires_at: u64,
}
impl HireProposal {
    pub fn validate(&self) -> Result<()> {
        if self.schema != HIRE_SCHEMA {
            return fail(Code::Malformed, "Use the openagents.crew-hire.v1 schema.");
        }
        crate::agent::name(&self.id)?;
        text(&self.reason, 1024)?;
        match &self.action {
            HireAction::Hire { name, role } => {
                crate::agent::name(name)?;
                if *role == JobRole::SalesLead {
                    return fail(Code::Forbidden, "Paul's own binding is not a hire.");
                }
                if self.daily_usd_millionths == 0
                    || self.daily_usd_millionths > FLOOR_USD_MILLIONTHS
                {
                    return fail(
                        Code::Bounds,
                        "A hire's daily budget is between one millionth and the floor ceiling.",
                    );
                }
                if self.charter_revision == 0 {
                    return fail(
                        Code::Malformed,
                        "Name the charter revision the hire starts from.",
                    );
                }
                if self.evidence.is_empty() {
                    return fail(
                        Code::Malformed,
                        "A hire needs queue evidence, not an empty desk.",
                    );
                }
            }
            HireAction::Retire { name } => crate::agent::name(name)?,
        }
        if self.evidence.len() > 8 {
            return fail(Code::Bounds, "Cite at most 8 evidence references.");
        }
        for e in &self.evidence {
            reference(&e.reference)?;
            digest(&e.sha256)?;
        }
        if self.expires_at == 0 {
            return fail(Code::Malformed, "A proposal expires.");
        }
        Ok(())
    }
    pub fn sha256(&self) -> String {
        use sha2::Digest;
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        format!("{:x}", sha2::Sha256::digest(bytes))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HireVerdict {
    Confirm,
    Reject,
}

/// The owner's single-use answer, bound to one exact proposal digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HireDecision {
    pub proposal: String,
    pub expected_sha256: String,
    pub verdict: HireVerdict,
    pub reason: String,
}
impl HireDecision {
    pub fn validate(&self) -> Result<()> {
        crate::agent::name(&self.proposal)?;
        digest(&self.expected_sha256)?;
        text(&self.reason, 512)
    }
}
