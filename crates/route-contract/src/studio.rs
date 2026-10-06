//! Declared Studio routes beside the frozen route-family contract.
//!
//! These operations grant no execution, disclosure, or spending authority.
//! The adapter sends the existing host operation under its existing policy.
use crate::binding::WorkbenchBinding;
use crate::{AdmissionSnapshot, Digest, digest_of};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.route.studio.v1";
pub const RESULT_SCHEMA: &str = "openagents.route.studio-result.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudioRoute {
    pub schema: String,
    pub request: String,
    pub snapshot: Digest,
    pub binding: Digest,
    pub intent: Intent,
}

/// The existing host wire operations. Command identities are minted before
/// admission and retained unchanged across transport recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Intent {
    #[serde(rename = "studio.goal.submit")]
    Goal {
        text: String,
        workspace: String,
        lead: Option<String>,
    },
    #[serde(rename = "studio.decision.answer")]
    Answer {
        decision: String,
        based_on: u64,
        text: String,
        command: String,
        issued_at: u64,
    },
    #[serde(rename = "studio.decision.always")]
    Always {
        decision: String,
        based_on: u64,
        rule: String,
        command: String,
        issued_at: u64,
    },
    #[serde(rename = "studio.review.open")]
    Review { task: String },
    #[serde(rename = "studio.merge.decide")]
    Decide { decision: MergeDecision },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeDecision {
    pub task: String,
    pub base: String,
    pub head_commit: String,
    pub head: String,
    pub verdict: Verdict,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    pub command: String,
    pub issued_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Merge,
    RequestChanges,
    Reject,
}

impl StudioRoute {
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }

    /// Bind a declared operation to its immutable admission and placement.
    pub fn check(
        &self,
        snapshot: &AdmissionSnapshot,
        binding: &WorkbenchBinding,
    ) -> Result<(), String> {
        if self.schema != SCHEMA
            || self.request.len() != 64
            || !self
                .request
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("malformed Studio route".into());
        }
        if self.request != snapshot.identity.request
            || self.snapshot != snapshot.digest()
            || self.binding != binding.digest()
            || !snapshot.route.explicit
            || snapshot.input.request != digest_of(&self.intent)
        {
            return Err("Studio route does not match its declared admission".into());
        }
        binding.check(snapshot).map_err(|e| format!("{e:?}"))?;
        if let Intent::Goal { workspace, .. } = &self.intent
            && snapshot.placement.workspace.as_ref().map(|w| &w.project) != Some(workspace)
        {
            return Err("Studio goal names another workspace".into());
        }
        Ok(())
    }
}
