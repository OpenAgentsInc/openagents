//! Commands delivered through the run host's existing device channel.
use super::{Runs, valid_id};
use crate::{CancellationToken, Error};
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// A change requested from a run's host. Acceptance means the host recorded
/// the command; it does not mean a turn finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionCommand {
    /// Queue a message for the session.
    Message { text: String },
    /// Deliver input into the current turn, when its harness supports it.
    Steer { text: String },
    /// Stop the current turn.
    Stop,
    /// Stop the current turn, then start another with this brief or its last request.
    Restart { brief: Option<String> },
}
impl<'de> Deserialize<'de> for SessionCommand {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Empty struct variants reject extra fields; Serde's unit variants do not.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Message { text: String },
            Steer { text: String },
            Stop {},
            Restart { brief: Option<String> },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Message { text } => Self::Message { text },
            Wire::Steer { text } => Self::Steer { text },
            Wire::Stop {} => Self::Stop,
            Wire::Restart { brief } => Self::Restart { brief },
        })
    }
}
impl SessionCommand {
    /// Whether text fits the command's limits: nonblank, at most 16 KiB, no NUL.
    pub fn valid(&self) -> bool {
        let valid_text =
            |text: &str| !text.trim().is_empty() && text.len() <= 16 * 1024 && !text.contains('\0');
        match self {
            Self::Message { text } | Self::Steer { text } => valid_text(text),
            Self::Restart { brief } => brief.as_deref().is_none_or(valid_text),
            Self::Stop => true,
        }
    }
}

/// The body of POST /v1/runs/{id}/commands. Keep this exact body on retry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCommand {
    /// Caller-chosen identifier, unique within this run.
    pub request_id: String,
    /// What the host should do.
    pub command: SessionCommand,
}
impl RunCommand {
    /// Whether the request ID and command meet the API's bounds.
    pub fn valid(&self) -> bool {
        valid_id(&self.request_id) && self.command.valid()
    }
}

/// Why a host could not accept a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRefusal {
    /// No supported input channel exists for this run or harness.
    NotSteerable,
    /// Current device or agent authority does not permit this command.
    NoGrant,
    /// The run ended.
    RunEnded,
    /// The latest heartbeat is too old.
    Offline,
    /// The host has not confirmed delivery. Retry the same request ID.
    Unavailable,
}

/// The host's handling result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandOutcome {
    /// The host recorded this command in its existing session ledger.
    Accepted,
    /// The command was refused or delivery could not be confirmed.
    Refused { reason: CommandRefusal },
}

impl<'de> Deserialize<'de> for CommandOutcome {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Accepted {},
            Refused { reason: CommandRefusal },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Accepted {} => Self::Accepted,
            Wire::Refused { reason } => Self::Refused { reason },
        })
    }
}

/// A result bound to the caller's run and request ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandReceipt {
    /// The run addressed by this command.
    pub run: String,
    /// The unchanged caller-chosen request ID.
    pub request_id: String,
    /// Whether the host accepted the command.
    pub outcome: CommandOutcome,
}
impl Runs<'_> {
    /// Send a command through the host's NIP-HOST connection. A current operate
    /// grant and any calling agent's host grant are required. Keep the request ID
    /// and command unchanged on retry, including after an unavailable result.
    pub async fn command(
        &self,
        run: &str,
        request_id: &str,
        command: SessionCommand,
        cancel: &CancellationToken,
    ) -> Result<CommandReceipt, Error> {
        let request = RunCommand {
            request_id: request_id.into(),
            command,
        };
        if !valid_id(run) || !request.valid() {
            return Err(Error::InvalidReference);
        }
        let receipt: CommandReceipt = self
            .send(
                Method::POST,
                &["runs", run, "commands"],
                &[],
                Some(serde_json::to_value(&request).map_err(|_| Error::InvalidReference)?),
                cancel,
            )
            .await?;
        if receipt.run != run || receipt.request_id != request_id {
            return Err(Error::InvalidResponse);
        }
        Ok(receipt)
    }
}
