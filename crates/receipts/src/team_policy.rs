//! Exact owner-reviewed team limits. These records narrow native grants.
use crate::execution::digest_request;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.team-policy.v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    SystemOne,
    Plugin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlacementKind {
    LocalGateway,
    CloudGateway,
    CustomerHost,
    LocalMember,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub kind: PlacementKind,
    pub identity: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPin {
    pub publisher: String,
    pub release: String,
    pub module: String,
}
/// The digest covers every state, question, and instruction in the input.
/// A caller's classification label cannot replace this reviewed provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub request: String,
    pub material: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub capability: Capability,
    pub release: String,
    pub model: Option<String>,
    pub plugin: Option<PluginPin>,
    pub recipients: Vec<String>,
    pub source: Source,
    pub placement: Placement,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub effect: Effect,
    pub data_classes: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub version: u64,
    pub expires_unix: u64,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub schema: String,
    pub workspace: String,
    pub terms: Terms,
    pub supersedes: Option<String>,
    pub reviewer: String,
    pub reviewer_epoch: u64,
    pub owner: String,
    pub owner_epoch: u64,
    pub reviewed_at: u64,
    pub digest: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub workspace: String,
    pub version: u64,
    pub digest: String,
    pub expires_unix: u64,
    pub owner: String,
    pub reviewer: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub policy: Reference,
    pub member: String,
    pub membership_epoch: u64,
    pub workspace_members_epoch: u64,
    pub credential_reference: String,
    pub rule: Rule,
    pub admitted_at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub expected_digest: Option<String>,
    pub terms: Terms,
}
pub fn hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
pub fn digest<T: Serialize>(value: &T) -> String {
    digest_request(&serde_json::to_value(value).expect("team policy serializes"))
}
impl Effect {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !hash(&self.release)
            || !hash(&self.source.request)
            || !hash(&self.source.material)
            || !hash(&self.placement.identity)
            || self.recipients.is_empty()
            || self.recipients.len() > 8
            || self.recipients.iter().any(|r| !hash(r))
            || self
                .recipients
                .iter()
                .enumerate()
                .any(|(i, r)| self.recipients[..i].contains(r))
        {
            return Err("Invalid exact team effect provenance.");
        }
        match self.capability {
            Capability::SystemOne
                if self.model.as_ref().is_some_and(|m| identifier(m)) && self.plugin.is_none() =>
            {
                ()
            }
            Capability::Plugin
                if self.model.is_none()
                    && self.plugin.as_ref().is_some_and(|p| {
                        p.publisher.len() == 64
                            && p.publisher
                                .bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                            && hash(&p.release)
                            && hash(&p.module)
                    }) =>
            {
                ()
            }
            _ => return Err("Invalid team model or plugin scope."),
        }
        Ok(())
    }
}
impl Terms {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version == 0 || self.expires_unix == 0 || self.rules.len() > 64 {
            return Err("Invalid team policy version or bound.");
        }
        for (i, r) in self.rules.iter().enumerate() {
            r.effect.validate()?;
            if r.data_classes.is_empty()
                || r.data_classes.len() > 16
                || r.data_classes.iter().any(|c| !identifier(c))
                || self.rules[..i].iter().any(|other| other.effect == r.effect)
            {
                return Err("Team data rights require unique exact reviewed provenance.");
            }
        }
        Ok(())
    }
    pub fn narrows(&self, parent: &Self) -> bool {
        self.expires_unix <= parent.expires_unix
            && self.rules.iter().all(|r| parent.rules.contains(r))
    }
}
impl Revision {
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("team policy serializes");
        value.as_object_mut().unwrap().remove("digest");
        digest_request(&value)
    }
    pub fn reference(&self) -> Reference {
        Reference {
            workspace: self.workspace.clone(),
            version: self.terms.version,
            digest: self.digest.clone(),
            expires_unix: self.terms.expires_unix,
            owner: self.owner.clone(),
            reviewer: self.reviewer.clone(),
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        self.terms.validate()?;
        if self.schema != SCHEMA
            || !identifier(&self.workspace)
            || !identifier(&self.reviewer)
            || !identifier(&self.owner)
            || self.supersedes.as_ref().is_some_and(|h| !hash(h))
            || self.digest != self.compute_digest()
        {
            return Err("Invalid retained team policy revision.");
        }
        Ok(())
    }
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.rule.effect.validate()?;
        if !identifier(&self.policy.workspace)
            || !hash(&self.policy.digest)
            || self.policy.version == 0
            || !identifier(&self.member)
            || !identifier(&self.credential_reference)
            || self.rule.data_classes.is_empty()
            || self.rule.data_classes.iter().any(|c| !identifier(c))
        {
            return Err("Invalid frozen team policy admission.");
        }
        Ok(())
    }
}
