//! Typed, bounded stdio contract for an operator-installed Rust agent.
//! A definition identifies a runner; it never supplies its executable or arguments.
use super::AgentSpec;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Read, Write};

pub const VERSION: &str = "openagents.agent.runner.v1";
pub const MAX_FRAME: usize = 96 * 1024;
pub const MAX_OUTPUT: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    pub schema: String,
    pub assignment: String,
    pub conversation: String,
    pub agent: AgentSpec,
    pub task: String,
    pub cwd: String,
}
impl Turn {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != VERSION || !super::valid_id(&self.assignment) {
            return Err("Unsupported runner request.");
        }
        self.agent.validate()?;
        if self.agent.runtime != "sdk" || self.agent.model.is_none() {
            return Err("Choose an installed SDK runner.");
        }
        if self.task.trim().is_empty() || self.task.len() > 65536 || self.task.contains('\0') {
            return Err("Enter a task or shorten the text.");
        }
        if self.cwd.is_empty() || self.cwd.len() > 4096 || self.cwd.contains('\0') {
            return Err("Choose a working folder.");
        }
        if self.conversation.len() > 128 || self.conversation.is_empty() {
            return Err("Choose a conversation.");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Text { text: String },
    Completed { result: String },
    Failed { message: String },
}
impl Event {
    pub fn text(&self) -> &str {
        match self {
            Self::Text { text } => text,
            Self::Completed { result } => result,
            Self::Failed { message } => message,
        }
    }
    pub fn terminal(&self) -> bool {
        !matches!(self, Self::Text { .. })
    }
}
/// Read one bounded JSON request, invoke application code, and write one result.
/// Logs belong on stderr. The host terminates the process when the user stops it.
pub fn serve_once<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    run: impl FnOnce(Turn) -> Result<String, String>,
) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take((MAX_FRAME + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if bytes.len() > MAX_FRAME {
        return Err(std::io::Error::other("Runner request is too large."));
    }
    let turn: Turn = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    turn.validate().map_err(std::io::Error::other)?;
    let event = match run(turn) {
        Ok(result) => Event::Completed { result },
        Err(message) => Event::Failed { message },
    };
    let bytes = serde_json::to_vec(&event).map_err(std::io::Error::other)?;
    if bytes.len() > MAX_FRAME {
        return Err(std::io::Error::other("Runner result is too large."));
    }
    output.write_all(&bytes)?;
    output.write_all(b"\n")?;
    output.flush()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn turn() -> Turn {
        Turn {
            schema: VERSION.into(),
            assignment: "a".repeat(64),
            conversation: "chat".into(),
            agent: AgentSpec {
                name: "Review".into(),
                description: String::new(),
                instructions: "Find defects".into(),
                runtime: "sdk".into(),
                model: Some("review".into()),
                access: super::super::Access::HostDefault,
            },
            task: "Review patch".into(),
            cwd: "/workspace".into(),
        }
    }
    #[test]
    fn independent_runner_round_trip_and_limits() {
        let mut bytes = serde_json::to_vec(&turn()).unwrap();
        bytes.push(b'\n');
        let mut output = vec![];
        serve_once(bytes.as_slice(), &mut output, |t| {
            assert_eq!(t.agent.instructions, "Find defects");
            Ok(t.task)
        })
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<Event>(&output).unwrap(),
            Event::Completed {
                result: "Review patch".into()
            }
        );
        assert!(
            serve_once(
                vec![b'x'; MAX_FRAME + 1].as_slice(),
                &mut output,
                |_| panic!("oversized request executed")
            )
            .is_err()
        );
        let mut bad = turn();
        bad.schema = "future".into();
        assert!(bad.validate().is_err());
    }
}
