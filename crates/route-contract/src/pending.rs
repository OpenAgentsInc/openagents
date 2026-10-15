//! Requests a task owner holds while an SDK callback waits for a person.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Digest, digest_of, observation::Scope};

pub const SCHEMA: &str = "openagents.coder.pending.v1";
pub const MAX_QUESTIONS: usize = 8;
pub const MAX_CHOICES: usize = 9;
pub const MAX_INPUT: usize = 32 * 1024;
pub const MAX_ANSWER: usize = 8 * 1024;
pub const SNAPSHOT_SCHEMA: &str = "openagents.coder.pending-snapshot.v1";
pub const MAX_REQUESTS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authority {
    pub intent: String,
    pub grant: String,
    pub checkout: String,
    /// A directory identity, not its modification time.
    pub target: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Medium,
    #[default]
    #[serde(other)]
    High,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prompt {
    pub command: String,
    pub reason: String,
    pub risk: Risk,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default)]
    pub free_text: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Questionnaire {
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Form {
    Approval,
    Questions { form: Questionnaire },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub schema: String,
    pub id: Digest,
    pub scope: Scope,
    pub authority: Authority,
    pub native_call: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
    pub tool: String,
    pub input: Value,
    pub input_digest: Digest,
    pub prompt: Prompt,
    pub form: Form,
    /// Unix seconds, checked by the task owner.
    pub expires_at: u64,
}

impl Request {
    pub fn seal(&mut self) {
        self.input_digest = digest_of(&self.input);
        self.id = self.identity();
    }

    fn identity(&self) -> Digest {
        digest_of(&(
            &self.schema,
            &self.scope,
            &self.authority,
            &self.native_call,
            &self.child,
            &self.tool,
            &self.input_digest,
            &self.prompt,
            &self.form,
            self.expires_at,
        ))
    }

    pub fn valid(&self) -> bool {
        self.schema == SCHEMA
            && self.id == self.identity()
            && self.input_digest == digest_of(&self.input)
            && self.scope.turn > 0
            && self.scope.generation > 0
            && bounded(&self.scope.host, 256)
            && bounded(&self.scope.task, 256)
            && bounded(&self.native_call, 256)
            && bounded(&self.tool, 256)
            && bounded(&self.authority.intent, 256)
            && bounded(&self.authority.grant, 256)
            && bounded(&self.authority.checkout, 4096)
            && bounded(&self.authority.target, 256)
            && self.child.as_ref().is_none_or(|child| bounded(child, 256))
            && self.prompt.command.len() <= MAX_INPUT
            && self.prompt.reason.len() <= 4096
            && serde_json::to_vec(&self.input).is_ok_and(|bytes| bytes.len() <= MAX_INPUT)
            && match &self.form {
                Form::Approval => true,
                Form::Questions { form } => form.valid(),
            }
    }

    pub fn accepts(&self, reply: &Reply) -> bool {
        self.valid()
            && reply.request == self.id
            && reply.scope == self.scope
            && reply.input_digest == self.input_digest
            && match (&self.form, &reply.answer) {
                (Form::Approval, Answer::AllowOnce | Answer::Deny) => true,
                (Form::Questions { form }, Answer::Questions { answers }) => form.accepts(answers),
                (_, Answer::Cancel) => true,
                _ => false,
            }
    }
}

impl Questionnaire {
    pub fn valid(&self) -> bool {
        !self.questions.is_empty()
            && self.questions.len() <= MAX_QUESTIONS
            && unique(self.questions.iter().map(|question| question.id.as_str()))
            && self.questions.iter().all(|question| {
                bounded(&question.id, 128)
                    && bounded(&question.text, 4096)
                    && question.choices.len() <= MAX_CHOICES
                    && (!question.choices.is_empty() || question.free_text)
                    && unique(question.choices.iter().map(|choice| choice.id.as_str()))
                    && question
                        .choices
                        .iter()
                        .all(|choice| bounded(&choice.id, 128) && bounded(&choice.label, 800))
            })
    }

    pub fn accepts(&self, answers: &[QuestionAnswer]) -> bool {
        self.valid()
            && answers.len() == self.questions.len()
            && unique(answers.iter().map(|answer| answer.question.as_str()))
            && self.questions.iter().all(|question| {
                let Some(answer) = answers.iter().find(|answer| answer.question == question.id)
                else {
                    return false;
                };
                let text = answer.text.as_deref().unwrap_or("");
                (question.multiple || answer.choices.len() <= 1)
                    && (question.free_text || text.is_empty())
                    && text.len() <= MAX_ANSWER
                    && (question.multiple || answer.choices.is_empty() || text.is_empty())
                    && (!answer.choices.is_empty() || !text.trim().is_empty())
                    && unique(answer.choices.iter().map(String::as_str))
                    && answer
                        .choices
                        .iter()
                        .all(|id| question.choices.iter().any(|choice| &choice.id == id))
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionAnswer {
    pub question: String,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Answer {
    AllowOnce,
    Deny,
    Questions { answers: Vec<QuestionAnswer> },
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub request: Digest,
    pub scope: Scope,
    pub input_digest: Digest,
    pub answer: Answer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Answered,
    Consumed,
    Cancelled,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub request: Digest,
    pub resolution: Resolution,
    /// An identical retry never resolves another callback.
    pub duplicate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub request: Request,
    /// None is waiting; every other state closes the answer controls.
    pub resolution: Option<Resolution>,
    pub answer: Option<Answer>,
}

/// A complete host snapshot, ordered separately from provider text updates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: String,
    pub scope: Scope,
    pub revision: u64,
    pub active: bool,
    pub entries: Vec<Entry>,
}

impl Snapshot {
    pub fn valid(&self) -> bool {
        self.schema == SNAPSHOT_SCHEMA
            && self.revision > 0
            && self.scope.turn > 0
            && self.scope.generation > 0
            && bounded(&self.scope.host, 256)
            && bounded(&self.scope.task, 256)
            && self.entries.len() <= MAX_REQUESTS
            && unique(self.entries.iter().map(|entry| entry.request.id.as_str()))
            && self.entries.iter().all(|entry| {
                entry.request.valid()
                    && entry.request.scope == self.scope
                    && (self.active || entry.resolution.is_some())
                    && match (&entry.resolution, &entry.answer) {
                        (None, None) => true,
                        (Some(Resolution::Answered | Resolution::Consumed), Some(answer))
                        | (Some(Resolution::Cancelled | Resolution::Expired), Some(answer)) => {
                            entry.request.accepts(&Reply {
                                request: entry.request.id.clone(),
                                scope: self.scope.clone(),
                                input_digest: entry.request.input_digest.clone(),
                                answer: answer.clone(),
                            })
                        }
                        (Some(Resolution::Cancelled | Resolution::Expired), None) => true,
                        _ => false,
                    }
            })
    }
}

fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max
}
fn unique<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|value| seen.insert(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Request {
        let mut request = Request {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            scope: Scope {
                host: "local".into(),
                task: "task".into(),
                turn: 1,
                generation: 1,
            },
            authority: Authority {
                intent: "intent".into(),
                grant: "grant".into(),
                checkout: "/work".into(),
                target: "directory-id".into(),
            },
            native_call: "call".into(),
            child: None,
            tool: "Bash".into(),
            input: json!({"command":"cargo test"}),
            input_digest: digest_of(&()),
            prompt: Prompt {
                command: "cargo test".into(),
                reason: "Run the checks".into(),
                risk: Risk::Medium,
            },
            form: Form::Approval,
            expires_at: 100,
        };
        request.seal();
        request
    }

    #[test]
    fn replies_bind_the_reviewed_input_scope_and_form() {
        let request = request();
        assert!(request.valid());
        let reply = Reply {
            request: request.id.clone(),
            scope: request.scope.clone(),
            input_digest: request.input_digest.clone(),
            answer: Answer::AllowOnce,
        };
        assert!(request.accepts(&reply));
        let mut changed = request.clone();
        changed.input["command"] = json!("deploy");
        assert!(!changed.valid());
        changed.seal();
        assert!(!changed.accepts(&reply));
        let mut changed = request.clone();
        changed.prompt.command = "another command".into();
        assert!(!changed.valid());
        let mut stale = reply;
        stale.scope.generation += 1;
        assert!(!request.accepts(&stale));
    }

    #[test]
    fn questions_validate_multiple_choices_free_text_and_exact_question_ids() {
        let form = Questionnaire {
            questions: vec![Question {
                id: "q".into(),
                text: "Which checks?".into(),
                multiple: true,
                free_text: true,
                choices: vec![
                    Choice {
                        id: "unit".into(),
                        label: "Unit tests".into(),
                    },
                    Choice {
                        id: "ui".into(),
                        label: "UI tests".into(),
                    },
                ],
            }],
        };
        let answer = QuestionAnswer {
            question: "q".into(),
            choices: vec!["unit".into(), "ui".into()],
            text: Some("And formatting".into()),
        };
        assert!(form.accepts(&[answer.clone()]));
        let mut single = form.clone();
        single.questions[0].multiple = false;
        assert!(!single.accepts(&[answer.clone()]));
        let mut invalid = answer.clone();
        invalid.choices.push("unknown".into());
        assert!(!form.accepts(&[invalid]));
        let mut invalid = answer;
        invalid.question = "other".into();
        assert!(!form.accepts(&[invalid]));
        assert!(!form.accepts(&[]));
    }
}
