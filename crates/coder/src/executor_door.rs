//! A door that answers through a local executor instead of a model API.
//!
//! `coder-worker` normally answers a NIP-CJ job through an Open Responses
//! door. On a machine whose operator has approved a capability such as
//! `devin-local`, the same job can be handed to that executor through
//! [`crate::delegate`], under the approval and the filesystem boundary a
//! delegation always runs under. This module is that door: it renders the
//! job as one bounded [`Task`], runs it, and returns what the executor
//! printed as the answer.
//!
//! The door is asked for with `CODER_EXECUTOR=<slug>`. The slug names a
//! manifest in `capabilities/`, and the manifest must be approved with
//! `capability-trust approve <slug>`; an unapproved or absent capability
//! refuses to build the door rather than answering with a stub.

use std::env;
use std::path::{Path, PathBuf};

use crate::delegate::{Bounds, Delegator, Status, Task};
use crate::generate::{Generate, GenerateError, Message, Meta, Role, Usage};
use crate::survey::Survey;

/// The variable that asks for this door, by capability slug.
pub const EXECUTOR_VAR: &str = "CODER_EXECUTOR";

/// The variable naming the directory the executor works in. Defaults to
/// the current directory.
pub const WORKDIR_VAR: &str = "CODER_EXECUTOR_WORKDIR";

/// The variable bounding one job, in minutes. Defaults to [`DEFAULT_MINUTES`].
pub const MINUTES_VAR: &str = "CODER_EXECUTOR_MINUTES";

/// How long one job may run when the environment does not say.
pub const DEFAULT_MINUTES: u64 = 10;

/// An approved local executor, ready to take jobs.
#[derive(Debug)]
pub struct ExecutorDoor {
    slug: String,
    delegator: Delegator,
    minutes: u64,
}

impl ExecutorDoor {
    /// Builds the door `CODER_EXECUTOR` asks for, or `None` when it is unset.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the slug names no capability, the
    /// capability is not present on this machine, its approval is missing,
    /// or the workdir does not exist. Every case is the operator's to fix
    /// and none of them falls through to another door.
    pub fn from_env() -> Result<Option<Self>, String> {
        let Some(slug) = env::var(EXECUTOR_VAR).ok().filter(|slug| !slug.is_empty()) else {
            return Ok(None);
        };
        let workdir = match env::var(WORKDIR_VAR) {
            Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => env::current_dir().map_err(|error| format!("{WORKDIR_VAR}: {error}"))?,
        };
        let minutes = match env::var(MINUTES_VAR) {
            Ok(text) if !text.is_empty() => text
                .parse::<u64>()
                .ok()
                .filter(|minutes| *minutes > 0)
                .ok_or_else(|| format!("{MINUTES_VAR} must be a positive whole number"))?,
            _ => DEFAULT_MINUTES,
        };
        Self::open(&slug, &workdir, minutes).map(Some)
    }

    /// Surveys `workdir` and builds the door for `slug`.
    ///
    /// # Errors
    ///
    /// See [`ExecutorDoor::from_env`].
    pub fn open(slug: &str, workdir: &Path, minutes: u64) -> Result<Self, String> {
        let workdir = workdir
            .canonicalize()
            .map_err(|error| format!("{}: {error}", workdir.display()))?;
        let repository = workdir
            .ancestors()
            .find(|dir| dir.join(".git").exists())
            .map(Path::to_path_buf);
        let survey = Survey::read(repository.as_deref(), &workdir);
        let found = survey.capability(slug).ok_or_else(|| {
            format!("{EXECUTOR_VAR}={slug} names no capability this host can see")
        })?;
        let executor = crate::survey::executor(found).ok_or_else(|| {
            format!(
                "{EXECUTOR_VAR}={slug} is not a route in {}: {}",
                workdir.display(),
                found.message()
            )
        })?;
        let mut delegator = Delegator::new(executor).in_directory(&workdir);
        if let Some(root) = repository {
            delegator = delegator.in_repository(root);
        }
        Ok(ExecutorDoor {
            slug: slug.to_string(),
            delegator,
            minutes,
        })
    }

    /// The capability slug, which stands where a model name would.
    #[must_use]
    pub fn slug(&self) -> &str {
        &self.slug
    }

    /// The directory jobs run in.
    #[must_use]
    pub fn workdir(&self) -> &Path {
        self.delegator.workdir()
    }
}

/// Renders instructions and a transcript as one prompt for an executor
/// that takes a single string.
#[must_use]
pub fn prompt(instructions: &str, input: &[Message]) -> String {
    let mut prompt = String::new();
    if !instructions.trim().is_empty() {
        prompt.push_str(instructions.trim());
        prompt.push_str("\n\n");
    }
    let (earlier, last) = match input.split_last() {
        Some((last, earlier)) => (earlier, Some(last)),
        None => (input, None),
    };
    if !earlier.is_empty() {
        prompt.push_str("Conversation so far:\n");
        for message in earlier {
            let who = match message.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            };
            prompt.push_str(who);
            prompt.push_str(": ");
            prompt.push_str(message.text.trim());
            prompt.push('\n');
        }
        prompt.push('\n');
    }
    if let Some(last) = last {
        prompt.push_str(last.text.trim());
    }
    prompt
}

impl Generate for ExecutorDoor {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        _meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let task =
            Task::asking(&prompt(instructions, input)).bounded(Bounds::minutes(self.minutes));
        let delegation = self.delegator.run(task).await;
        match delegation.status.clone() {
            Status::Answered => {
                let text = delegation.output.trim().to_string();
                if text.is_empty() {
                    return Err(GenerateError::Stream(format!(
                        "{} exited cleanly and printed nothing",
                        self.slug
                    )));
                }
                sink(&text);
                Ok((text, None))
            }
            Status::Refused(code) => Err(GenerateError::Refused {
                code,
                message: delegation.recorded_output(),
            }),
            Status::TimedOut => Err(GenerateError::Quiet {
                heard: !delegation.output.trim().is_empty(),
                reason: format!("{} ran past its {} minute bound", self.slug, self.minutes),
            }),
            Status::Failed(code) => Err(GenerateError::Stream(format!(
                "{} exited {code}: {}",
                self.slug,
                delegation.recorded_output()
            ))),
            Status::Harness(why) => Err(GenerateError::Stream(format!(
                "{} could not run: {why}",
                self.slug
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: Role, text: &str) -> Message {
        Message {
            role,
            text: text.to_string(),
        }
    }

    #[test]
    fn the_prompt_carries_instructions_history_and_the_last_turn() {
        let rendered = prompt(
            "Answer briefly.",
            &[
                message(Role::User, "hi"),
                message(Role::Assistant, "hello"),
                message(Role::User, "what is two plus two?"),
            ],
        );
        assert_eq!(
            rendered,
            "Answer briefly.\n\nConversation so far:\nUser: hi\nAssistant: hello\n\nwhat is two plus two?"
        );
    }

    #[test]
    fn a_lone_turn_is_the_prompt() {
        assert_eq!(prompt("", &[message(Role::User, "ping")]), "ping");
    }

    #[test]
    fn an_unknown_slug_refuses_to_open() {
        let dir = tempfile::tempdir().unwrap();
        let why = ExecutorDoor::open("no-such-capability", dir.path(), 1).unwrap_err();
        assert!(why.contains("no-such-capability"), "{why}");
    }
}
