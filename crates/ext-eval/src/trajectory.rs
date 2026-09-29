//! What graders read from a run's rendered ATIF document.
//!
//! A run writes an ATIF v1.8 log with `crates/atif`, and graders read the
//! rendered document: its steps, the calls those steps made, and Coder's
//! final assistant text. Operation identity is read from the call's
//! bounded fields only — its function name, a Wasm guest's `operation`
//! argument, and a program step's `step` extra — never from prose.

use serde_json::{Value, json};

use crate::artifact::{ArtifactRef, JSON};

/// Steps a door sees from each end of a trajectory.
pub const DOOR_STEPS: usize = 12;

/// One run's trajectory: the document and the reference to its exact bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct Trajectory {
    /// The rendered document, relabelled to the current ATIF version.
    pub document: Value,
    /// The exact bytes' reference (schema `ATIF-v1.8`).
    pub artifact: ArtifactRef,
}

impl Trajectory {
    /// Reads a rendered ATIF document from its exact bytes.
    ///
    /// # Errors
    ///
    /// Returns why the bytes are not a supported ATIF document.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut document: Value = serde_json::from_slice(bytes)
            .map_err(|error| format!("the trajectory is not JSON: {error}"))?;
        atif::upgrade(&mut document)?;
        let problems = atif::validate(&document);
        if let Some(first) = problems.first() {
            return Err(format!(
                "the trajectory is not a valid ATIF document: {first}"
            ));
        }
        Ok(Self {
            document,
            artifact: ArtifactRef::of(bytes, JSON, Some(atif::SCHEMA_VERSION)),
        })
    }

    /// The steps, in order.
    #[must_use]
    pub fn steps(&self) -> &[Value] {
        self.document
            .get("steps")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    /// Coder's final assistant text: the last agent step with a message.
    #[must_use]
    pub fn final_message(&self) -> Option<&str> {
        self.steps().iter().rev().find_map(|step| {
            (step.get("source").and_then(Value::as_str) == Some("agent"))
                .then(|| step.get("message").and_then(Value::as_str))
                .flatten()
                .filter(|message| !message.trim().is_empty())
        })
    }

    /// Every call the trajectory records, in step order.
    #[must_use]
    pub fn calls(&self) -> Vec<OperationCall> {
        let mut calls = Vec::new();
        for (index, step) in self.steps().iter().enumerate() {
            if let Some(tool_calls) = step.get("tool_calls").and_then(Value::as_array) {
                for call in tool_calls {
                    calls.push(OperationCall::of(index, call, "function_name"));
                }
            }
            if let Some(call) = step.get("extra").and_then(|extra| extra.get("call")) {
                calls.push(OperationCall::of(index, call, "function_name"));
            }
        }
        calls
    }

    /// What a door sees of the trajectory: the first and last twelve steps
    /// and the final message.
    #[must_use]
    pub fn door_view(&self) -> Value {
        let steps = self.steps();
        let (first, last, omitted) = if steps.len() <= DOOR_STEPS * 2 {
            (steps, &steps[steps.len()..], 0)
        } else {
            (
                &steps[..DOOR_STEPS],
                &steps[steps.len() - DOOR_STEPS..],
                steps.len() - DOOR_STEPS * 2,
            )
        };
        json!({
            "first_steps": first,
            "omitted_steps": omitted,
            "last_steps": last,
            "final_message": self.final_message(),
        })
    }
}

/// One call a trajectory records.
#[derive(Clone, Debug, PartialEq)]
pub struct OperationCall {
    /// The index of the step that made it.
    pub step: usize,
    /// The call's function name.
    pub name: String,
    /// Its arguments.
    pub arguments: Value,
    /// Its `extra`, when any.
    pub extra: Value,
}

impl OperationCall {
    fn of(step: usize, call: &Value, name_key: &str) -> Self {
        Self {
            step,
            name: call
                .get(name_key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            arguments: call.get("arguments").cloned().unwrap_or(Value::Null),
            extra: call.get("extra").cloned().unwrap_or(Value::Null),
        }
    }

    /// Whether the call is `operation`: its function name, a Wasm guest's
    /// `operation` argument, or a program step's `step` extra.
    #[must_use]
    pub fn is(&self, operation: &str) -> bool {
        self.name == operation
            || self.arguments.get("operation").and_then(Value::as_str) == Some(operation)
            || self.extra.get("step").and_then(Value::as_str) == Some(operation)
    }

    /// The text an `input_match` pattern reads: a shell call's command, and
    /// the JSON of the arguments otherwise.
    #[must_use]
    pub fn input(&self) -> String {
        match self.arguments.get("command").and_then(Value::as_str) {
            Some(command) => command.to_string(),
            None => serde_json::to_string(&self.arguments).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(steps: &[atif::Step]) -> Trajectory {
        let session = atif::Session::opening("s1", "model", "door", "/repo", "0.1.0");
        let document = atif::document(&session, steps);
        Trajectory::from_bytes(&serde_json::to_vec(&document).unwrap()).expect("valid")
    }

    fn shell(command: &str) -> atif::Step {
        atif::Step::called(atif::Call {
            id: "c".into(),
            name: "shell".into(),
            arguments: json!({ "command": command, "workdir": "/repo" }),
            output: String::new(),
            outcome: atif::Outcome::Completed,
            milliseconds: 1,
            purpose: None,
            extra: serde_json::Map::new(),
        })
    }

    #[test]
    fn the_final_message_is_the_last_agent_text() {
        let trajectory = session(&[
            atif::Step::said(atif::Source::User, "go"),
            atif::Step::said(atif::Source::Agent, "first"),
            shell("ls"),
            atif::Step::said(atif::Source::Agent, "done"),
        ]);
        assert_eq!(trajectory.final_message(), Some("done"));
        let calls = trajectory.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].is("shell"));
        assert_eq!(calls[0].input(), "ls");
    }

    #[test]
    fn a_door_sees_both_ends_of_a_long_trajectory() {
        let steps: Vec<atif::Step> = (0..30)
            .map(|index| atif::Step::said(atif::Source::Agent, &format!("step {index}")))
            .collect();
        let view = session(&steps).door_view();
        assert_eq!(view["first_steps"].as_array().unwrap().len(), 12);
        assert_eq!(view["last_steps"].as_array().unwrap().len(), 12);
        assert_eq!(view["omitted_steps"], 6);
        assert_eq!(view["final_message"], "step 29");
    }

    #[test]
    fn a_document_that_is_not_atif_is_refused() {
        assert!(Trajectory::from_bytes(b"{}").is_err());
        assert!(Trajectory::from_bytes(b"not json").is_err());
        assert!(Trajectory::from_bytes(br#"{"schema_version":"ATIF-v9.0"}"#).is_err());
    }
}
