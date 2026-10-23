//! Bounded edit intent and short-lived authoring permission. Tokens are not accounts.
use crate::{Action, Capability, Checkpoint, Error, Project, protocol::id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Hash the validated typed action identically in the native host and browser SDK.
pub fn action_digest(action: &Action) -> Result<String, Error> {
    action.validate()?;
    let bytes = serde_json::to_vec(action).map_err(|_| Error::Invalid("action"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScopedGrant {
    pub token: String,
    pub action_sha256: String,
    pub session: String,
    pub release: String,
    /// The loaded base world, independent of the person's private draft.
    pub project: Project,
    pub draft: Project,
    pub capability: Capability,
    pub expires_at: u64,
}
impl ScopedGrant {
    pub fn validate(&self) -> Result<(), Error> {
        self.project.validate()?;
        self.draft.validate()?;
        if self.action_sha256.len() != 64
            || !self
                .action_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || self.token.len() != 64
            || !self
                .token
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !id(&self.session)
            || !id(&self.release)
            || self.expires_at > crate::MAX_SAFE_INTEGER
            || !mutation(self.capability)
            || self.project.same_branch(&self.draft)
        {
            return Err(Error::Invalid("grant"));
        }
        Ok(())
    }
}
pub fn mutation(capability: Capability) -> bool {
    matches!(
        capability,
        Capability::Propose
            | Capability::Draft
            | Capability::Compile
            | Capability::Undo
            | Capability::Redo
            | Capability::Save
            | Capability::Publish
            | Capability::Share
    )
}
/// Host-origin session storage keeps this across sign-in; it contains no credential.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PendingChange {
    pub world: String,
    pub checkpoint: Checkpoint,
    pub selection: Option<String>,
    pub panels: Vec<String>,
    pub draft: String,
    pub action: Action,
}
impl PendingChange {
    pub fn validate(&self) -> Result<(), Error> {
        self.checkpoint.validate()?;
        self.action.validate()?;
        if !matches!(self.world.as_str(), "haunted-house" | "personal-planet")
            || self.selection.as_ref().is_some_and(|s| !id(s))
            || self.panels.len() > 8
            || self.panels.iter().any(|p| {
                !matches!(
                    p.as_str(),
                    "edit" | "mesh" | "materials" | "kiln" | "history"
                )
            })
            || self.draft.len() > 48_000
            || self.draft.contains('\0')
            || !self.action.capability().is_some_and(mutation)
        {
            return Err(Error::Invalid("pending change"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumePolicy {
    Keep,
    Reconfirm,
    Discard,
}
/// An expired login keeps its request; another account must start its own request.
pub fn resume_policy(
    owner: Option<&str>,
    previous: Option<&str>,
    current: Option<&str>,
) -> ResumePolicy {
    if current.is_some()
        && (owner.is_some_and(|o| Some(o) != current)
            || previous.is_some_and(|p| Some(p) != current))
    {
        ResumePolicy::Discard
    } else if previous != current {
        ResumePolicy::Reconfirm
    } else {
        ResumePolicy::Keep
    }
}
