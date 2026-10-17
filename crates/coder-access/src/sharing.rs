//! Owner-approved invitations for selected host workspaces.
use crate::{Code, Error, Result, Right, Rights};
use serde::{Deserialize, Serialize};

pub const CAPABILITY: &str = "project-sharing-v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    pub account: String,
    pub device: String,
    /// Host-configured workspace labels, never paths supplied by the caller.
    pub workspaces: Vec<String>,
    pub rights: Rights,
    pub expires_at: u64,
}
impl Invite {
    pub fn validate(&self) -> Result<()> {
        coder_connect::protocol::identity(&self.device).map_err(Error::from)?;
        if self.account.is_empty()
            || self.account.len() > 128
            || self.account.chars().any(char::is_control)
            || self.workspaces.is_empty()
            || self.workspaces.len() > 16
            || !self.workspaces.windows(2).all(|pair| pair[0] < pair[1])
            || self.workspaces.iter().any(|label| {
                label.is_empty() || label.len() > 128 || label.chars().any(char::is_control)
            })
            || !self.rights.contains(Right::Observe)
            || self
                .rights
                .iter()
                .any(|right| !matches!(right, Right::Observe | Right::Operate))
            || self.expires_at == 0
        {
            return Err(Error::new(
                Code::Malformed,
                "Check the recipient, projects, permissions, and expiry.",
            ));
        }
        Ok(())
    }
}
