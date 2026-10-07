//! Private crew job descriptions and evidence recommendations. These are
//! data, not access grants, approvals, or independently verified judgments.

use serde::{Deserialize, Serialize};

use crate::{Code, Result, fail};

pub const CHARTER_SCHEMA: &str = "openagents.crew-charter.v1";
pub const VERDICT_SCHEMA: &str = "openagents.crew-verdict.v1";
/// Keeps the largest typed collection within a native agent reply.
pub const MAX_VERDICTS: usize = 24;

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
