//! Exact shell proposals owned by a terminal, with bounded remote reads.

use crate::wire::{Reason, Refusal, TerminalRef, common_id, version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub terminal: String,
    pub generation: String,
    pub cwd: String,
    /// Advisory logical PWD at the prompt; the OS cwd still binds admission.
    #[serde(default)]
    pub shell_directory: Option<String>,
    pub context_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub thread: String,
    pub id: String,
    pub revision: u64,
    pub command: String,
    pub binding: Binding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    /// The shared effect boundary classed the command read-only. Enter
    /// treats it as ordinary; only this class can auto-run
    /// ([`crate::autorun`]).
    ReadOnly,
    Ordinary,
    Destructive(String),
    Denied(String),
}

impl Proposal {
    pub fn key(&self) -> String {
        digest(&(&self.thread, &self.id, self.revision))
    }

    pub fn valid(&self) -> bool {
        [
            &self.thread,
            &self.id,
            &self.binding.terminal,
            &self.binding.generation,
            &self.binding.cwd,
            &self.binding.context_digest,
        ]
        .iter()
        .all(|word| !word.is_empty() && word.len() <= 8192 && !word.chars().any(char::is_control))
            && self.binding.shell_directory.as_ref().is_none_or(|path| {
                !path.is_empty() && path.len() <= 8192 && !path.chars().any(char::is_control)
            })
            && !self.command.is_empty()
            && self.command.len() <= 8192
            && !self.command.chars().any(char::is_control)
    }
}

pub const FEATURE: &str = "openagents.terminal-proposals.v1";
pub const READ: &str = "openagents.terminal-proposal-read.v1";
pub const OFFER: &str = "openagents.terminal-proposal-offer.v1";
pub const DECIDE: &str = "openagents.terminal-proposal-decide.v1";

pub fn digest(value: &impl Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("proposal values serialize"))
    )
}

/// Shared approval checks for local and remote surfaces. The caller must
/// retain the admitted disposition before writing any input.
pub fn admit(
    proposal: &Proposal,
    current: &Binding,
    principal: &str,
    nonce: &str,
    pending: bool,
    previous_warning: Option<&str>,
    effect: Effect,
) -> Result<Option<String>, &'static str> {
    if principal.is_empty() || nonce.is_empty() {
        return Err("approval identity is required");
    }
    if &proposal.binding != current {
        return Err("proposal target or displayed context changed");
    }
    if !pending {
        return Err("proposal already admitted; reconcile its result");
    }
    match effect {
        Effect::Denied(_) => return Err("host policy denied the command"),
        Effect::Destructive(warning) if previous_warning.is_none() => return Ok(Some(warning)),
        _ => {}
    }
    if previous_warning == Some(nonce) {
        return Err("a second explicit key is required");
    }
    Ok(None)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub action: Action,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Read {
        limit: u16,
    },
    Offer {
        proposal: Proposal,
    },
    Decide {
        thread: String,
        proposal: String,
        revision: u64,
        approve: bool,
        attachment: String,
    },
}
impl Request {
    pub fn new(request: String, terminal: TerminalRef, action: Action) -> Self {
        Self {
            v: match &action {
                Action::Read { .. } => READ,
                Action::Offer { .. } => OFFER,
                Action::Decide { .. } => DECIDE,
            }
            .into(),
            requires: vec![FEATURE.into()],
            request,
            terminal,
            action,
        }
    }
    pub fn check(&self, features: crate::ext::Features) -> Result<(), Refusal> {
        let expected = match self.action {
            Action::Read { .. } => READ,
            Action::Offer { .. } => OFFER,
            Action::Decide { .. } => DECIDE,
        };
        version(&self.v, expected)?;
        if !features.admit(&self.requires, &[FEATURE])? {
            return Err(Refusal::new(
                Reason::Malformed,
                "proposal feature is required",
            ));
        }
        common_id(&self.request, "request")?;
        self.terminal.check()?;
        let valid = match &self.action {
            Action::Read { limit } => (1..=8).contains(limit),
            Action::Offer { proposal } => {
                proposal.valid()
                    && proposal.thread.len() <= 128
                    && proposal.id.len() <= 128
                    && proposal.binding.terminal == self.terminal.terminal
                    && proposal.binding.generation == self.terminal.generation
            }
            Action::Decide {
                thread,
                proposal,
                attachment,
                ..
            } => {
                [thread, proposal]
                    .iter()
                    .all(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
                    && common_id(attachment, "attachment").is_ok()
            }
        };
        if !valid {
            return Err(Refusal::new(Reason::Malformed, "invalid proposal request"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    Warned { nonce: String },
    Rejected,
    Executing,
    Uncertain,
    Completed { block: u64 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub proposal: Proposal,
    pub effect: Effect,
    pub state: State,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub entries: Vec<Entry>,
    pub more: bool,
}
