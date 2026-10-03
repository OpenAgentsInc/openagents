//! What the model proposes at each step, as typed records.
//!
//! The model answers with one JSON object. These types read it leniently
//! (unknown fields are ignored, most fields are optional) because the
//! machine checks and repairs everything it keeps: a proposal is a
//! suggestion the floor and the case parser decide on, never an
//! instruction.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// The tool at step 1: a description of an existing tool, or a proposed
/// chat-made one.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ToolProposal {
    /// What we say: what the tool is for, what it does, and what it
    /// doesn't; or, when we can't propose yet, one question.
    pub say: String,
    /// True when `say` is a question because we don't know enough yet.
    pub asking: bool,
    /// A made tool's name.
    pub name: Option<String>,
    /// A made tool's one-sentence summary.
    pub summary: Option<String>,
    /// A made tool's guidance: the skill Coder follows.
    pub skill: Option<String>,
    /// Catalog tools a made tool turns on, by name.
    pub uses: Vec<String>,
}

/// One proposed test.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TestProposal {
    /// A short kebab-case name for the task shape.
    pub id: String,
    /// `should-fire` or `should-not-fire`.
    pub kind: String,
    /// The task, in the words a person would give Coder.
    pub task: String,
    /// What a good outcome is, in a sentence: the starting check.
    pub good: Option<String>,
    /// The folder a files test starts in: a template name (`empty`,
    /// `rust-crate`, `python-package`, `node-package`). Absent for a test
    /// graded on Coder's reply.
    pub workspace: Option<String>,
}

/// Tests at step 3.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TestsProposal {
    /// What we say about them.
    pub say: String,
    /// The tests.
    pub tests: Vec<TestProposal>,
}

/// What a check reads.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum FocusProposal {
    /// `last_message`, `trajectory`, `files`, `changed`, or `diff`.
    Word(String),
    /// `{ "file": "<path>" }`.
    File {
        /// The path inside the run's workspace.
        file: String,
    },
}

/// One proposed check (a grader).
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraderProposal {
    /// Jev answers a yes-or-no question about the focus.
    Decision {
        /// A short name.
        #[serde(default)]
        name: String,
        /// The question.
        question: String,
        /// What it reads.
        #[serde(default)]
        focus: Option<FocusProposal>,
        /// The probability a vote has to reach.
        #[serde(default)]
        threshold: Option<f64>,
        /// What a successful run looks like.
        #[serde(default)]
        rubric: Option<String>,
    },
    /// A pattern over the focus.
    Regex {
        /// A short name.
        #[serde(default)]
        name: String,
        /// The pattern.
        pattern: String,
        /// `contains` or `not_contains`.
        #[serde(default, rename = "match")]
        matching: Option<String>,
        /// What it reads.
        #[serde(default)]
        target: Option<FocusProposal>,
        /// `i`, `m`, `s`, `x`.
        #[serde(default)]
        flags: Option<String>,
    },
    /// A created file matches a glob, or none does.
    FileExists {
        /// A short name.
        #[serde(default)]
        name: String,
        /// The glob.
        path: String,
        /// Whether a match is required.
        #[serde(default)]
        exists: Option<bool>,
    },
    /// A command run in a files test's folder after the turn exits with
    /// the expected code.
    Command {
        /// A short name.
        #[serde(default)]
        name: String,
        /// The command, such as `cargo test`.
        command: String,
        /// The exit code that passes; 0 when absent.
        #[serde(default)]
        exit_code: Option<i32>,
    },
    /// How often an operation ran.
    OperationUsed {
        /// A short name.
        #[serde(default)]
        name: String,
        /// The operation.
        operation: String,
        /// Fewest calls.
        #[serde(default)]
        min: Option<u32>,
        /// Most calls.
        #[serde(default)]
        max: Option<u32>,
    },
}

impl GraderProposal {
    /// The proposed name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Decision { name, .. }
            | Self::Regex { name, .. }
            | Self::FileExists { name, .. }
            | Self::Command { name, .. }
            | Self::OperationUsed { name, .. } => name,
        }
    }
}

/// One test's checks.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
pub struct CaseChecks {
    /// The test's id.
    pub test: String,
    /// Its checks, as written. [`CaseChecks::typed`] reads them; one the
    /// machine doesn't know is dropped, not the whole proposal.
    #[serde(default)]
    #[schemars(with = "Vec<GraderProposal>")]
    pub graders: Vec<serde_json::Value>,
}

impl CaseChecks {
    /// The checks this version knows, in order.
    #[must_use]
    pub fn typed(&self) -> Vec<GraderProposal> {
        self.graders
            .iter()
            .filter_map(|value| serde_json::from_value(value.clone()).ok())
            .collect()
    }
}

/// Checks at step 4.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ChecksProposal {
    /// What we say about them, in plain words.
    pub say: String,
    /// The checks per test.
    pub checks: Vec<CaseChecks>,
}

/// A fix after a try or a change request: new tests, new checks, or both.
/// Tests left out keep their checks; a test whose checks are left out gets
/// a starting check.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct FixProposal {
    /// What we say.
    pub say: String,
    /// The whole test list, when it changes.
    pub tests: Option<Vec<TestProposal>>,
    /// Checks for the tests they name.
    pub checks: Option<Vec<CaseChecks>>,
}

/// Words only.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SayProposal {
    /// What we say.
    pub say: String,
}

/// Reads the one JSON object in a model's answer: the text from the first
/// `{` to the last `}`, which tolerates a code fence around it.
///
/// # Errors
///
/// Why the answer doesn't hold the object.
pub fn parse<T: DeserializeOwned>(answer: &str) -> Result<T, String> {
    let start = answer
        .find('{')
        .ok_or_else(|| "the answer holds no JSON object".to_string())?;
    let end = answer
        .rfind('}')
        .filter(|end| *end > start)
        .ok_or_else(|| "the answer's JSON object never closes".to_string())?;
    serde_json::from_str(&answer[start..=end]).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_answer_reads_and_graders_are_typed() {
        let answer = "```json\n{\"say\": \"Here's how we'd check.\", \"checks\": [{\"test\": \"a\", \"graders\": [{\"type\": \"decision\", \"name\": \"outcome\", \"question\": \"Did it?\", \"focus\": {\"file\": \"CHANGELOG.md\"}}, {\"type\": \"operation_used\", \"operation\": \"repo_map\", \"max\": 0}, {\"type\": \"regex\", \"pattern\": \"x\", \"match\": \"not_contains\", \"extra\": 1}, {\"type\": \"telepathy\"}]}], \"note\": 3}\n```";
        let checks: ChecksProposal = parse(answer).unwrap();
        let graders = &checks.checks[0].typed();
        assert!(matches!(
            &graders[0],
            GraderProposal::Decision { focus: Some(FocusProposal::File { file }), .. } if file == "CHANGELOG.md"
        ));
        assert!(matches!(
            &graders[1],
            GraderProposal::OperationUsed { max: Some(0), .. }
        ));
        assert!(
            matches!(&graders[2], GraderProposal::Regex { matching: Some(m), .. } if m == "not_contains")
        );
        assert_eq!(
            graders.len(),
            3,
            "a check type the machine doesn't know is dropped"
        );
        assert!(parse::<SayProposal>("no object here").is_err());
    }
}

/// The structured output contract for one interview step.
pub fn schema(need: &super::Need) -> serde_json::Value {
    use super::Need;
    let mut schema = match need {
        Need::Tool { .. } => serde_json::to_value(schemars::schema_for!(ToolProposal)),
        Need::Tests { .. } => serde_json::to_value(schemars::schema_for!(TestsProposal)),
        Need::Checks { .. } => serde_json::to_value(schemars::schema_for!(ChecksProposal)),
        Need::Fix { .. } => serde_json::to_value(schemars::schema_for!(FixProposal)),
        _ => serde_json::to_value(schemars::schema_for!(SayProposal)),
    }
    .expect("proposal schema");
    schema["required"] = match need {
        Need::Tests { .. } => serde_json::json!(["say", "tests"]),
        Need::Checks { .. } => serde_json::json!(["say", "checks"]),
        _ => serde_json::json!(["say"]),
    };
    schema
}
