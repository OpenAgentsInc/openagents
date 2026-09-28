//! What the agent knows: the environment, the issue, and every step so
//! far.

use serde::Serialize;

/// The state every step reads: the judge, the prompt, and the generator
/// all see this one value.
#[derive(Debug, Clone, Serialize)]
pub struct State {
    pub environment: Environment,
    pub issue: Issue,
    /// Files the survey judged relevant before the first step, with their
    /// contents then. Empty unless a survey ran.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub survey: Vec<Surveyed>,
    pub history: Vec<Turn>,
}

impl State {
    /// A state with no history yet.
    pub fn new(environment: Environment, issue: Issue) -> Self {
        Self {
            environment,
            issue,
            survey: Vec::new(),
            history: Vec::new(),
        }
    }
}

/// One file the survey put in front of the generator.
#[derive(Debug, Clone, Serialize)]
pub struct Surveyed {
    pub path: String,
    /// Jev's probability that reading or editing the file helps.
    pub relevance: f64,
    /// Jev's probability that resolving the task edits the file.
    pub edit: f64,
    /// The file's contents, capped, with a marker when cut.
    pub content: String,
}

/// Where the agent works.
#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    /// The repository, as `owner/name`.
    pub repository: String,
    /// The checkout the agent's commands run in.
    pub workdir: String,
    /// The operating system, as Rust names it.
    pub os: String,
}

/// The GitHub issue the run addresses.
#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub url: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

/// One step the agent took.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Turn {
    /// A command ran, and this is what it produced.
    Shell {
        command: String,
        /// The model's note on why it ran the command.
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        observation: Observation,
    },
    /// The generator's reply was not a valid action. The error goes back
    /// to the generator on the next step.
    Malformed { reply: String, error: String },
}

/// What a command produced.
#[derive(Debug, Clone, Serialize)]
pub struct Observation {
    /// The exit code, or `None` when the command ended without one, for
    /// example because its deadline killed it.
    pub exit: Option<i32>,
    /// Captured stdout and stderr, held to the shell's cap.
    pub output: String,
    /// Whether the cap cut the output.
    pub truncated: bool,
}
