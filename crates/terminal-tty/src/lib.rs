//! Hook-only requests over an ordinary shell; the shell keeps every ordinary key.
#[cfg(unix)]
pub mod runner;

use serde::{Deserialize, Serialize};
use terminal_core::{blocks::Block, proposals::Proposal};

/// An exact result sent to the existing thread with a stable request identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultRequest {
    pub proposal: Proposal,
    pub approval: String,
    pub block: Block,
}

impl ResultRequest {
    pub fn identity(&self) -> String {
        terminal_core::proposals::digest(&("terminal-result-v1", &self.proposal, &self.approval))
    }

    pub fn message(&self) -> Result<String, &'static str> {
        if !self.proposal.valid()
            || self.approval.is_empty()
            || self.block.command != self.proposal.command
            || self.block.cwd.as_deref()
                != Some(
                    self.proposal
                        .binding
                        .shell_directory
                        .as_deref()
                        .unwrap_or(&self.proposal.binding.cwd),
                )
            || self.block.end.is_none()
            || self.block.status.is_none()
            || self.block.output.len() > terminal_core::blocks::MAX_OUTPUT
        {
            return Err("terminal result does not match the approved proposal");
        }
        let mut retained = self.clone();
        retained.block.output = terminal_core::smart::scrub(&retained.block.output);
        retained.block.command = terminal_core::smart::scrub(&retained.block.command);
        retained.proposal.command = terminal_core::smart::scrub(&retained.proposal.command);
        let receipt = serde_json::json!({
            "v": "openagents.terminal-result.v1", "request": self.identity(),
            "proposal_digest": terminal_core::proposals::digest(&self.proposal),
            "result_digest": terminal_core::proposals::digest(&self.block),
            "result": retained,
        });
        Ok(format!(
            "Approved terminal proposal {} revision {} completed with exit {}.\n{}",
            self.proposal.id,
            self.proposal.revision,
            self.block.status.unwrap(),
            receipt
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::{blocks::Position, proposals::Binding};
    fn result() -> ResultRequest {
        ResultRequest {
            proposal: Proposal {
                thread: "thread".into(),
                id: "proposal".into(),
                revision: 1,
                command: "printf hello".into(),
                binding: Binding {
                    terminal: "term".into(),
                    generation: "generation".into(),
                    cwd: "/scratch".into(),
                    shell_directory: None,
                    context_digest: "context".into(),
                },
            },
            approval: "approval".into(),
            block: Block {
                id: 2,
                command: "printf hello".into(),
                cwd: Some("/scratch".into()),
                start: Position { line: 1, col: 0 },
                end: Some(Position { line: 2, col: 0 }),
                status: Some(0),
                started_ms: 1,
                elapsed_ms: Some(1),
                output: "hello".into(),
                truncated: false,
                collapsed: false,
                alternate: false,
            },
        }
    }
    #[test]
    fn a_result_binds_the_exact_command_directory_and_completed_block() {
        let mut result = result();
        assert!(result.message().is_ok());
        result.block.command = "another command".into();
        assert!(result.message().is_err());
        result.block.command = result.proposal.command.clone();
        result.block.cwd = Some("/other".into());
        assert!(result.message().is_err());
        result.block.cwd = Some("/scratch".into());
        result.block.end = None;
        assert!(result.message().is_err());
    }
    #[test]
    fn result_redelivery_keeps_the_same_shared_request_and_scrubs_secrets() {
        let mut result = result();
        let identity = result.identity();
        result.block.output = "password=private".into();
        assert_eq!(identity, result.identity());
        let message = result.message().unwrap();
        assert!(!message.contains("private"));
        assert!(message.contains("[redacted]"));
        result.proposal.revision += 1;
        assert_ne!(identity, result.identity());
    }
}
