//! The format: what a step is, and the document a list of steps renders as.
//!
//! A step is the unit a session writes down. A document is what a reader
//! asks for, and it is computed from the steps rather than stored, so the
//! two never disagree — see [`crate::log`] for why that matters.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// The version of the format this crate writes.
pub const SCHEMA_VERSION: &str = "ATIF-v1.7";

/// The agent name a consumer groups these trajectories under.
pub const AGENT_NAME: &str = "openagents-coder";

/// The exporter a document names, so a reader can tell which program wrote
/// it apart from which agent it describes.
pub const EXPORTER: &str = "openagents.atif.v1";

/// The schema a decision-model call carries in its `extra`. A `Call` with
/// this schema is a question put to a door — Jev, Kev, or Lev — rather than
/// a command run on the machine, and a reader that wants one and not the
/// other separates them by this field.
pub const DECISION_CALL_SCHEMA: &str = "openagents.decision-call.v1";

/// Who a step came from, which ATIF calls its source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// The host: instructions, notes about what the host did or could not do.
    System,
    /// The person at the terminal.
    User,
    /// The agent.
    Agent,
}

impl Source {
    /// The word the document spells this source with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Source::System => "system",
            Source::User => "user",
            Source::Agent => "agent",
        }
    }
}

/// How a call ended, which a consumer reads to tell a failed step from a
/// refused one without parsing the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// It ran and finished.
    Completed,
    /// It ran and failed, or could not be reached.
    Failed,
    /// It never ran: something refused it first.
    Cancelled,
}

impl Outcome {
    /// The word the document spells this outcome with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Failed => "failed",
            Outcome::Cancelled => "cancelled",
        }
    }
}

/// One call and what answered it.
///
/// A call is a shell command or a question put to a decision model; the two
/// are the same shape because they are the same thing from the document's
/// side — the agent asked something outside itself, and something answered.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Call {
    /// Unique within the session; the observation names it back.
    pub id: String,
    /// The tool's name: `shell`, `classify`, `shell_judge`.
    pub name: String,
    /// What the call was given, kept structured rather than flattened into
    /// prose so a later reader can filter on it.
    pub arguments: Value,
    /// What came back, in full. Nothing here is capped; see the crate docs.
    pub output: String,
    /// How it ended.
    pub outcome: Outcome,
    /// Wall time the call took.
    pub milliseconds: u64,
    /// What the call was for, in the words the surface showed beside it,
    /// when the surface said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// What the host knows about the call beyond the wire fields, exported
    /// as the tool call's `extra`. A decision call records the door, the
    /// model, the digest of the state it read, and the typed answers here.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

impl Call {
    /// Whether this call is a decision-model call rather than work done on
    /// the machine.
    #[must_use]
    pub fn is_decision(&self) -> bool {
        self.extra.get("schema").and_then(Value::as_str) == Some(DECISION_CALL_SCHEMA)
    }
}

/// One step of a session: a prompt, a message, or a call with its result.
///
/// A step is written down as it happens, so a session reads back its own
/// trail after the process that made it is gone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    /// Milliseconds since the epoch, which the document reports as ISO 8601.
    pub at: u64,
    /// Who the step came from.
    pub source: Source,
    /// What was said.
    pub message: String,
    /// What the model said it was thinking, when it says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// The model that produced this step, when it is not the one the
    /// session opened with.
    ///
    /// A session header names the model the door serves, which a door that
    /// forwards to somewhere else does not know until something answers.
    /// The step knows: it is written after the answer. A session that
    /// reaches two models therefore records two, and a reader takes the
    /// step's word over the session's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The call this step made, when it made one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<Call>,
    /// Input and output tokens of the turn this step belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<(u64, u64)>,
    /// How long the step took.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milliseconds: Option<u64>,
    /// Versioned host evidence, preserved through export.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub extensions: Map<String, Value>,
}

/// What a door reported for one turn. Cost is not among the fields: no door
/// this repository talks to publishes a price, and an unknown number
/// recorded as zero is worse than one that is absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Tokens the request consumed.
    pub prompt: u64,
    /// Tokens the response produced.
    pub completion: u64,
}

impl Step {
    /// A step someone said something in, stamped now.
    #[must_use]
    pub fn said(source: Source, message: &str) -> Self {
        Step {
            at: now_ms(),
            source,
            message: message.to_string(),
            reasoning: None,
            model: None,
            call: None,
            tokens: None,
            milliseconds: None,
            extensions: Map::new(),
        }
    }

    /// A step the model reasoned in.
    #[must_use]
    pub fn thought(reasoning: &str) -> Self {
        Step {
            reasoning: Some(reasoning.to_string()),
            ..Step::said(Source::Agent, "")
        }
    }

    /// A step that made a call.
    #[must_use]
    pub fn called(call: Call) -> Self {
        Step {
            call: Some(call),
            ..Step::said(Source::Agent, "")
        }
    }

    /// The model that produced this step, when the session header cannot
    /// name it.
    #[must_use]
    pub fn by(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    /// How long the step took.
    #[must_use]
    pub fn taking(mut self, milliseconds: u64) -> Self {
        self.milliseconds = Some(milliseconds);
        self
    }

    /// Put a turn's usage on this step.
    pub fn spent(&mut self, usage: Usage) {
        self.tokens = Some((usage.prompt, usage.completion));
    }

    /// Put one piece of host evidence on this step.
    #[must_use]
    pub fn noting(mut self, key: &str, value: Value) -> Self {
        self.extensions.insert(key.to_string(), value);
        self
    }

    /// The step as the document spells it, numbered from one.
    fn value(&self, ordinal: usize, model: &str) -> Value {
        let mut step = Map::new();
        step.insert("step_id".to_string(), json!(ordinal));
        step.insert("timestamp".to_string(), json!(iso(self.at)));
        step.insert("source".to_string(), json!(self.source.word()));
        step.insert("message".to_string(), json!(self.message));
        if self.source == Source::Agent {
            step.insert(
                "model_name".to_string(),
                json!(self.model.as_deref().unwrap_or(model)),
            );
        }
        if let Some(reasoning) = &self.reasoning {
            step.insert("reasoning_content".to_string(), json!(reasoning));
        }
        if let Some(call) = &self.call {
            let mut tool_call = json!({
                "tool_call_id": call.id,
                "function_name": call.name,
                "arguments": call.arguments,
            });
            if !call.extra.is_empty() {
                tool_call["extra"] = Value::Object(call.extra.clone());
            }
            step.insert("tool_calls".to_string(), json!([tool_call]));
            step.insert(
                "observation".to_string(),
                json!({
                    "results": [{
                        "source_call_id": call.id,
                        "content": call.output,
                        "status": call.outcome.word(),
                        "duration_ms": call.milliseconds,
                    }]
                }),
            );
        }
        if let Some((prompt, completion)) = self.tokens {
            step.insert(
                "metrics".to_string(),
                json!({ "prompt_tokens": prompt, "completion_tokens": completion }),
            );
        }
        let mut extra = self.extensions.clone();
        if let Some(milliseconds) = self.milliseconds {
            extra.insert("duration_ms".to_string(), json!(milliseconds));
        }
        if let Some(call) = &self.call
            && let Some(purpose) = &call.purpose
        {
            extra.insert("purpose".to_string(), json!(purpose));
        }
        if !extra.is_empty() {
            step.insert("extra".to_string(), Value::Object(extra));
        }
        Value::Object(step)
    }
}

/// What a document says about the session as a whole.
///
/// The fields a session knows when it opens — its identity, its door, where
/// it is running — are set once. The fields that are only true at the end —
/// how it ended and how long it took — are computed by [`crate::log::read`]
/// from the steps, so a session that is killed still reports them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    /// Unique per session, and the stem of the file the session writes to.
    pub id: String,
    /// The model the Generate door serves, as the session knows it at the
    /// start. A door that forwards to a worker does not know it then, and
    /// says so; the answer steps carry what answered.
    pub model: String,
    /// Which Generate door the session's turns went through.
    pub door: String,
    /// Where the session was running, as the agent knows it.
    pub repository: String,
    /// What the session was first asked to do.
    #[serde(default)]
    pub directive: String,
    /// How the session ended: `ended` or `interrupted`.
    #[serde(default)]
    pub state: String,
    /// Wall time from the first record to the last.
    #[serde(default)]
    pub seconds: u64,
    /// The version of the agent that wrote it.
    pub version: String,
}

impl Session {
    /// A session with the fields that are known when it opens, and the
    /// rest left for the reader to compute.
    #[must_use]
    pub fn opening(id: &str, model: &str, door: &str, repository: &str, version: &str) -> Self {
        Session {
            id: id.to_string(),
            model: model.to_string(),
            door: door.to_string(),
            repository: repository.to_string(),
            directive: String::new(),
            state: String::new(),
            seconds: 0,
            version: version.to_string(),
        }
    }
}

/// One dispatch of a decision call that asked more than once.
///
/// A reviewed or retried call spends several dispatches; each keeps its
/// own model, answers, usage, timing, and outcome so the record
/// attributes every attempt to the artifact that produced it rather
/// than folding the chain into the answer that was finally selected.
#[derive(Clone, Debug)]
pub struct Attempt {
    /// What the dispatch was in the chain: `primary`, `fallback:<cause>`,
    /// or `review`.
    pub role: String,
    /// The model the door reported answering with, or the identity the
    /// dispatch requested when it never answered.
    pub model: String,
    /// The typed answers, or null when the dispatch produced none. A
    /// reviewer that answered without a scored gate records its own
    /// answers — never the primary's confidence under another model's
    /// name.
    pub answers: Value,
    /// The usage the door reported — either side nullable when the door
    /// reports incompletely.
    pub input_tokens: Option<u64>,
    /// See `input_tokens`.
    pub output_tokens: Option<u64>,
    /// Wall time this dispatch took.
    pub milliseconds: u64,
    /// `answered`, or the failure cause the dispatch ended in.
    pub outcome: String,
}

/// One decision-model call, on its way to becoming a [`Call`].
///
/// A door answering `POST /v1/systemone` is the half of a session the Gym's
/// rows cannot supply: a row says what one door answered for one state, and
/// this says what the agent did next. Recording it as a first-class call
/// rather than as prose is why the format is worth having.
#[derive(Clone, Debug)]
pub struct Decision {
    /// Unique within the session.
    pub id: String,
    /// What the call is in the conversation: `classify` for the turn
    /// router, `shell_judge` for the round judge.
    pub name: String,
    /// Which door answered, by base URL.
    pub door: String,
    /// Which model the door answered with.
    pub model: String,
    /// The body that went out, `state` and `questions` and all.
    pub request: Value,
    /// The typed answers the call selected — the reviewed answer when a
    /// review answered, the original when it did not — or null when the
    /// call produced none.
    pub answers: Value,
    /// What the host did with the answers.
    pub route: Option<String>,
    /// Why the call produced no answers, when it produced none.
    pub error: Option<String>,
    /// Every dispatch the call spent, primary first. The `answers`
    /// field above is the selected attempt's; this is the whole
    /// chain's attribution.
    pub attempts: Vec<Attempt>,
    /// What a declared review did, when one ran: why it triggered, what
    /// it answered, and what became of the original — recorded even
    /// when it changed nothing.
    pub review: Option<Value>,
    /// Wall time the call took.
    pub milliseconds: u64,
}

impl Decision {
    /// The call the document records.
    #[must_use]
    pub fn call(self) -> Call {
        let state = self.request.get("state").cloned().unwrap_or(Value::Null);
        let questions = self
            .request
            .get("questions")
            .cloned()
            .unwrap_or(Value::Null);
        let mut extra = Map::new();
        extra.insert("schema".to_string(), json!(DECISION_CALL_SCHEMA));
        extra.insert("door".to_string(), json!(self.door));
        extra.insert("model".to_string(), json!(self.model));
        extra.insert("state_digest".to_string(), json!(digest(&state)));
        extra.insert("questions_digest".to_string(), json!(digest(&questions)));
        extra.insert("question_ids".to_string(), json!(question_ids(&questions)));
        extra.insert("answers".to_string(), self.answers.clone());
        if let Some(route) = &self.route {
            extra.insert("route".to_string(), json!(route));
        }
        if let Some(error) = &self.error {
            extra.insert("error".to_string(), json!(error));
        }
        if !self.attempts.is_empty() {
            extra.insert(
                "attempts".to_string(),
                json!(
                    self.attempts
                        .iter()
                        .map(|attempt| json!({
                            "role": attempt.role,
                            "model": attempt.model,
                            "answers": attempt.answers,
                            "input_tokens": attempt.input_tokens,
                            "output_tokens": attempt.output_tokens,
                            "milliseconds": attempt.milliseconds,
                            "outcome": attempt.outcome,
                        }))
                        .collect::<Vec<_>>()
                ),
            );
        }
        if let Some(review) = &self.review {
            extra.insert("review".to_string(), review.clone());
        }
        Call {
            id: self.id,
            name: self.name,
            arguments: self.request,
            output: match &self.error {
                Some(error) => error.clone(),
                None => serde_json::to_string(&self.answers).unwrap_or_default(),
            },
            outcome: match self.error {
                Some(_) => Outcome::Failed,
                None => Outcome::Completed,
            },
            milliseconds: self.milliseconds,
            purpose: None,
            extra,
        }
    }
}

/// The ids of a question set, in the order it names them.
fn question_ids(questions: &Value) -> Vec<String> {
    questions
        .as_object()
        .map(|set| set.keys().cloned().collect())
        .unwrap_or_default()
}

/// The whole session as one ATIF document.
#[must_use]
pub fn document(session: &Session, steps: &[Step]) -> Value {
    let values: Vec<Value> = steps
        .iter()
        .enumerate()
        .map(|(index, step)| step.value(index + 1, &session.model))
        .collect();
    let prompt: u64 = steps
        .iter()
        .filter_map(|step| step.tokens.map(|(prompt, _)| prompt))
        .sum();
    let completion: u64 = steps
        .iter()
        .filter_map(|step| step.tokens.map(|(_, completion)| completion))
        .sum();
    let calls = steps.iter().filter(|step| step.call.is_some()).count();
    let failed = tool_calls(steps)
        .filter(|call| call.outcome == Outcome::Failed)
        .count();
    json!({
        "schema_version": SCHEMA_VERSION,
        "session_id": session.id,
        "trajectory_id": session.id,
        "agent": {
            "name": AGENT_NAME,
            "version": session.version,
            "model_name": session.model,
            "extra": { "door": session.door },
        },
        "steps": values,
        "final_metrics": {
            "total_prompt_tokens": prompt,
            "total_completion_tokens": completion,
            "total_steps": steps.len(),
            "extra": {
                "door": session.door,
                "calls_total": calls,
                "tool_calls_total": tool_calls(steps).count(),
                "tool_calls_failed": failed,
                "decision_calls": decision_calls(steps),
                "reads_total": reads_total(steps),
                "search_calls_total": search_calls_total(steps),
                "largest_tool_result_chars": largest_tool_result_chars(steps),
                "median_inter_round_text_chars": median_inter_round_text_chars(steps),
                "wall_seconds": session.seconds,
                "waste": waste(steps),
            },
        },
        "extra": {
            "exporter": EXPORTER,
            "exported_at": iso(now_ms()),
            "repository": session.repository,
            "directive": session.directive,
            "state": session.state,
        },
    })
}

/// The calls that did work on the machine. A decision call is a question
/// put to a door, not a tool the agent ran, and counting it as one would
/// make every turn look like a tool round.
fn tool_calls(steps: &[Step]) -> impl Iterator<Item = &Call> {
    steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .filter(|call| !call.is_decision())
}

/// How many times each decision door was asked, and how many of those asks
/// it failed to answer. This is the count the Gym's rows cannot supply:
/// rows say what a door answered, and this says how often the agent asked.
fn decision_calls(steps: &[Step]) -> Value {
    let mut names: Vec<(String, u64, u64, u64)> = Vec::new();
    for call in steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .filter(|call| call.is_decision())
    {
        let entry = match names.iter_mut().find(|(name, ..)| *name == call.name) {
            Some(entry) => entry,
            None => {
                names.push((call.name.clone(), 0, 0, 0));
                names.last_mut().unwrap_or_else(|| unreachable!())
            }
        };
        entry.1 += 1;
        entry.2 += u64::from(call.outcome == Outcome::Failed);
        entry.3 += call.milliseconds;
    }
    let total: u64 = names.iter().map(|(_, asked, ..)| asked).sum();
    json!({
        "total": total,
        "by_name": names
            .iter()
            .map(|(name, asked, failed, milliseconds)| json!({
                "name": name,
                "asked": asked,
                "failed": failed,
                "duration_ms": milliseconds,
            }))
            .collect::<Vec<_>>(),
    })
}

/// Character count of the longest tool result recorded in the session.
///
/// Returns 0 when the session recorded no tool results. The number is worth
/// having because it is the one that decides whether a trace is big, and a
/// reader should be able to see that without reading the whole file.
fn largest_tool_result_chars(steps: &[Step]) -> usize {
    tool_calls(steps)
        .map(|call| call.output.chars().count())
        .max()
        .unwrap_or(0)
}

/// Character count of the median agent prose step between tool rounds.
///
/// Prose steps that carried calls do not count, and the final prose step —
/// the closing answer — is excluded. Returns 0 when no narration between
/// rounds occurred.
fn median_inter_round_text_chars(steps: &[Step]) -> usize {
    let mut counts: Vec<usize> = steps
        .iter()
        .filter(|step| step.source == Source::Agent && step.call.is_none())
        .map(|step| step.message.chars().count())
        .collect();
    if counts.len() <= 1 {
        return 0;
    }
    counts.pop();
    counts.sort_unstable();
    let len = counts.len();
    if len.is_multiple_of(2) {
        (counts[len / 2 - 1] + counts[len / 2]) / 2
    } else {
        counts[len / 2]
    }
}

/// The programs that read a file, and the programs that search for one.
///
/// This agent declares one tool, `shell`, and does its reading and its
/// searching by running these. Counting them is the only way a document can
/// say what a session spent its rounds on. The lists are enumerated fields
/// read off a recorded command, which is bounded deterministic parsing over
/// a record that already exists; nothing routes on them.
const READERS: [&str; 6] = ["cat", "sed", "head", "tail", "less", "bat"];
const SEARCHERS: [&str; 6] = ["grep", "rg", "ag", "ack", "fgrep", "egrep"];

/// Whether a shell command runs one of `programs`.
///
/// A command is a sequence of pipelines, and what a pipeline does is what
/// its first program does: `grep x | head -40` is a search, not a read,
/// because `head` is filtering the search rather than reading a file. A
/// sequence is read whole, so `cd src && grep -rn x .` searches.
fn shell_runs(command: &str, programs: &[&str]) -> bool {
    command
        .split([';', '\n'])
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .map(|pipeline| pipeline.split('|').next().unwrap_or(pipeline))
        .any(|part| {
            let mut words = part.split_whitespace().skip_while(|word| {
                // A chain often starts with a directory change or an
                // environment assignment; the program is what follows.
                *word == "cd" || *word == "sudo" || word.contains('=')
            });
            match words.next() {
                // `git grep` is a search, and `git` alone is not.
                Some("git") => words.next().is_some_and(|word| programs.contains(&word)),
                Some(word) => programs.contains(&word.rsplit('/').next().unwrap_or(word)),
                None => false,
            }
        })
}

/// The command a shell call ran, out of its arguments.
fn commanded(call: &Call) -> Option<&str> {
    (call.name == "shell")
        .then(|| call.arguments.get("command").and_then(Value::as_str))
        .flatten()
}

fn reads_total(steps: &[Step]) -> usize {
    tool_calls(steps)
        .filter(|call| commanded(call).is_some_and(|command| shell_runs(command, &READERS)))
        .count()
}

fn search_calls_total(steps: &[Step]) -> usize {
    tool_calls(steps)
        .filter(|call| commanded(call).is_some_and(|command| shell_runs(command, &SEARCHERS)))
        .count()
}

/// Work a session did more than once.
///
/// The document lists repeated calls and estimates their duration from the
/// group's average. Repetition does not establish that a call was
/// unnecessary: a command can run again to verify a change. Decision calls
/// are outside the table — the same questions go to the door every turn
/// over a state that has moved, so counting them as repeats would report
/// the loop working as waste.
fn waste(steps: &[Step]) -> Value {
    let mut groups: Vec<(String, u64, u64)> = Vec::new();
    for call in tool_calls(steps) {
        let what = intent(&call.name, &call.arguments);
        match groups.iter_mut().find(|(name, _, _)| *name == what) {
            Some((_, times, milliseconds)) => {
                *times += 1;
                *milliseconds += call.milliseconds;
            }
            None => groups.push((what, 1, call.milliseconds)),
        }
    }
    groups.retain(|(_, times, _)| *times > 1);
    groups.sort_by(|left, right| right.1.cmp(&left.1).then(right.2.cmp(&left.2)));
    let repeated: Vec<Value> = groups
        .iter()
        .map(|(what, times, milliseconds)| {
            // The first execution was the work; the rest is what it cost to
            // do it again, at the average of what the group took.
            let each = milliseconds / times;
            json!({
                "what": what,
                "executions": times,
                "duration_ms": milliseconds,
                "wasted_ms": each * (times - 1),
            })
        })
        .collect();
    let wasted: u64 = repeated
        .iter()
        .filter_map(|group| group.get("wasted_ms").and_then(Value::as_u64))
        .sum();
    let again: u64 = groups.iter().map(|(_, times, _)| times - 1).sum();
    json!({
        "repeated_calls": again,
        "wasted_ms": wasted,
        "repeated": repeated,
    })
}

/// The identity used to group repeated calls.
///
/// A shell call is the complete command and the directory it ran in: the
/// same text in two checkouts is two different pieces of work. Anything
/// else groups by its name.
#[must_use]
pub fn intent(name: &str, arguments: &Value) -> String {
    let field = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or("");
    match name {
        "shell" => format!("shell {}", json!([field("workdir"), field("command")])),
        other => other.to_string(),
    }
}

/// The digest of a value: object keys sorted at every depth, then SHA-256.
///
/// Sorting throughout is what makes the digest answer what was asked rather
/// than what order the fields happened to serialize in, and it is the rule
/// `gym`'s perturbation key already follows, so the two agree.
#[must_use]
pub fn digest(value: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical(value).as_bytes());
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// A value as text with every object's keys sorted, at every depth.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical(fields.get(*key).unwrap_or(&Value::Null))
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(canonical).collect();
            format!("[{}]", inner.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Milliseconds since the epoch.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// An ISO 8601 timestamp in UTC, to the millisecond. One clock formats
/// every timestamp a document carries, and the name of every file a session
/// writes to, so the two always agree.
#[must_use]
pub fn iso(at: u64) -> String {
    let seconds = at / 1_000;
    let milliseconds = at % 1_000;
    let (year, month, day) = date(seconds / 86_400);
    let day_seconds = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{milliseconds:03}Z",
        day_seconds / 3_600,
        (day_seconds % 3_600) / 60,
        day_seconds % 60
    )
}

/// A compact UTC stamp — `20260919T142233Z` — for a file name, where a
/// colon is not welcome.
#[must_use]
pub fn stamp(at: u64) -> String {
    let seconds = at / 1_000;
    let (year, month, day) = date(seconds / 86_400);
    let day_seconds = seconds % 86_400;
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        day_seconds / 3_600,
        (day_seconds % 3_600) / 60,
        day_seconds % 60
    )
}

/// The calendar date of a day count since 1970-01-01, by Howard Hinnant's
/// civil-from-days algorithm.
fn date(days: u64) -> (u64, u64, u64) {
    let shifted = days as i64 + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as u64, month as u64, day as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_session() -> Session {
        Session {
            id: "20260919T142233Z-0a1b2c3d".to_string(),
            model: "a-model".to_string(),
            door: "stub".to_string(),
            repository: "/Users/someone/work/openagents".to_string(),
            directive: "add a test".to_string(),
            state: "ended".to_string(),
            seconds: 42,
            version: "0.1.0".to_string(),
        }
    }

    fn a_shell_call(command: &str, output: &str) -> Call {
        Call {
            id: "call-1".to_string(),
            name: "shell".to_string(),
            arguments: json!({"command": command, "workdir": "/tmp"}),
            output: output.to_string(),
            outcome: Outcome::Completed,
            milliseconds: 12,
            purpose: Some("look".to_string()),
            extra: Map::new(),
        }
    }

    /// A document names the format, the agent, and the session, and numbers
    /// its steps from one.
    #[test]
    fn a_document_reports_the_session_it_came_from() {
        let steps = vec![
            Step::said(Source::System, "you are Coder"),
            Step::said(Source::User, "add a test"),
        ];
        let document = document(&a_session(), &steps);
        assert_eq!(document["schema_version"], SCHEMA_VERSION);
        assert_eq!(document["agent"]["name"], AGENT_NAME);
        assert_eq!(document["session_id"], a_session().id);
        assert_eq!(document["steps"][0]["step_id"], 1);
        assert_eq!(document["steps"][0]["source"], "system");
        assert_eq!(document["steps"][1]["step_id"], 2);
        assert_eq!(document["steps"][1]["message"], "add a test");
        assert_eq!(document["final_metrics"]["total_steps"], 2);
        assert_eq!(document["extra"]["exporter"], EXPORTER);
        assert_eq!(document["extra"]["state"], "ended");
    }

    /// A call and its result are one step, correlated by the call id, and
    /// the result says how long the call took and how it ended.
    #[test]
    fn a_call_and_its_result_are_one_step() {
        let steps = vec![Step::called(a_shell_call("ls crates", "atif\ncoder"))];
        let document = document(&a_session(), &steps);
        let step = &document["steps"][0];
        assert_eq!(step["tool_calls"][0]["tool_call_id"], "call-1");
        assert_eq!(step["tool_calls"][0]["function_name"], "shell");
        assert_eq!(
            step["observation"]["results"][0]["source_call_id"],
            "call-1"
        );
        assert_eq!(step["observation"]["results"][0]["content"], "atif\ncoder");
        assert_eq!(step["observation"]["results"][0]["status"], "completed");
        assert_eq!(step["observation"]["results"][0]["duration_ms"], 12);
        assert_eq!(step["extra"]["purpose"], "look");
    }

    /// A decision call records which door answered, what it was asked, what
    /// it said, and the digest of the state it read.
    #[test]
    fn a_decision_call_records_the_door_and_the_state_it_read() {
        let state = json!({"task": "list the crates", "transcript": []});
        let request = json!({
            "state": state,
            "model": "jev-latest",
            "questions": {"action": {"type": "choice"}, "risk": {"type": "score"}},
        });
        let call = Decision {
            id: "call-2".to_string(),
            name: "classify".to_string(),
            door: "https://api.typesafe.ai".to_string(),
            model: "jev-latest".to_string(),
            request,
            answers: json!({"action": {"choice": "respond", "confidence": 0.8}}),
            route: Some("respond".to_string()),
            error: None,
            attempts: Vec::new(),
            review: None,
            milliseconds: 240,
        }
        .call();
        assert!(call.is_decision());
        assert_eq!(call.extra["door"], "https://api.typesafe.ai");
        assert_eq!(call.extra["state_digest"], digest(&state));
        assert_eq!(call.extra["question_ids"], json!(["action", "risk"]));
        assert_eq!(call.extra["route"], "respond");
        assert_eq!(call.extra["answers"]["action"]["choice"], "respond");
        assert_eq!(call.outcome, Outcome::Completed);
        // The arguments hold the state, so the question and the answer read
        // back together without a second file.
        assert_eq!(call.arguments["state"]["task"], "list the crates");
    }

    /// A door that could not be reached is a failed call with the reason on
    /// it, not a missing step.
    #[test]
    fn a_failed_decision_call_keeps_its_reason() {
        let call = Decision {
            id: "call-3".to_string(),
            name: "classify".to_string(),
            door: "https://api.typesafe.ai".to_string(),
            model: "jev-latest".to_string(),
            request: json!({"state": {}, "questions": {}}),
            answers: Value::Null,
            route: None,
            error: Some("connection refused".to_string()),
            attempts: Vec::new(),
            review: None,
            milliseconds: 30,
        }
        .call();
        assert_eq!(call.outcome, Outcome::Failed);
        assert_eq!(call.output, "connection refused");
        assert_eq!(call.extra["error"], "connection refused");
    }

    /// A reviewed call records every dispatch it spent: the selected
    /// answer is the call's, and the chain keeps each attempt's own
    /// model, usage, timing, and outcome beside the review record that
    /// says what became of the original.
    #[test]
    fn a_reviewed_decision_records_its_attempts() {
        let call = Decision {
            id: "call-4".to_string(),
            name: "judge".to_string(),
            door: "https://api.typesafe.ai".to_string(),
            model: "reviewer".to_string(),
            request: json!({"state": {}, "model": "stub", "questions": {"q": {"type": "noul"}}}),
            answers: json!({"q": {"type": "noul", "noul": 0.95}}),
            route: Some("q 0.95".to_string()),
            error: None,
            attempts: vec![
                Attempt {
                    role: "primary".to_string(),
                    model: "stub".to_string(),
                    answers: json!({"q": {"type": "noul", "noul": 0.4}}),
                    input_tokens: Some(100),
                    output_tokens: Some(12),
                    milliseconds: 200,
                    outcome: "answered".to_string(),
                },
                Attempt {
                    role: "review".to_string(),
                    model: "reviewer".to_string(),
                    answers: json!({"q": {"type": "noul", "noul": 0.95}}),
                    input_tokens: None,
                    output_tokens: None,
                    milliseconds: 180,
                    outcome: "answered".to_string(),
                },
            ],
            review: Some(json!({
                "reason": "q answered 0.40, under 0.6",
                "reviewer": "reviewer",
                "original": {"model": "stub", "gate": 0.4},
                "reviewed": {"model": "reviewer", "gate": 0.95},
                "outcome": "changed",
            })),
            milliseconds: 380,
        }
        .call();
        let attempts = &call.extra["attempts"];
        assert_eq!(attempts[0]["role"], "primary");
        assert_eq!(attempts[0]["model"], "stub");
        assert_eq!(attempts[0]["answers"]["q"]["noul"], 0.4);
        assert_eq!(attempts[1]["role"], "review");
        assert_eq!(attempts[1]["model"], "reviewer");
        assert_eq!(attempts[1]["input_tokens"], Value::Null);
        // The selected answer is the reviewer's; the original stays
        // recorded in the chain, not overwritten by it.
        assert_eq!(call.extra["answers"]["q"]["noul"], 0.95);
        assert_eq!(call.extra["review"]["outcome"], "changed");
        assert_eq!(call.extra["review"]["original"]["gate"], 0.4);
    }

    /// Decision calls count as decisions and not as tool calls, so a turn
    /// that only asked the router does not read as a tool round.
    #[test]
    fn decisions_and_tool_calls_count_separately() {
        let decision = Decision {
            id: "call-1".to_string(),
            name: "classify".to_string(),
            door: "d".to_string(),
            model: "m".to_string(),
            request: json!({"state": {}, "questions": {"action": {}}}),
            answers: json!({}),
            route: Some("respond".to_string()),
            error: None,
            attempts: Vec::new(),
            review: None,
            milliseconds: 100,
        }
        .call();
        let steps = vec![
            Step::called(decision),
            Step::called(a_shell_call("grep -rn atif crates", "hit")),
        ];
        let extra = &document(&a_session(), &steps)["final_metrics"]["extra"];
        assert_eq!(extra["calls_total"], 2);
        assert_eq!(extra["tool_calls_total"], 1);
        assert_eq!(extra["decision_calls"]["total"], 1);
        assert_eq!(extra["decision_calls"]["by_name"][0]["name"], "classify");
        assert_eq!(extra["search_calls_total"], 1);
        assert_eq!(extra["reads_total"], 0);
        // The router asked the same questions twice would not be waste.
        assert_eq!(extra["waste"]["repeated_calls"], 0);
    }

    /// A pipeline is what its first program does, and a repeated command is
    /// counted once as work and once as the cost of doing it again.
    #[test]
    fn repeated_shell_commands_report_as_waste() {
        let mut second = a_shell_call("cargo test -p atif", "ok");
        second.id = "call-2".to_string();
        let steps = vec![
            Step::called(a_shell_call("cargo test -p atif", "ok")),
            Step::called(second),
        ];
        let waste = &document(&a_session(), &steps)["final_metrics"]["extra"]["waste"];
        assert_eq!(waste["repeated_calls"], 1);
        assert_eq!(waste["repeated"][0]["executions"], 2);
        assert_eq!(waste["repeated"][0]["wasted_ms"], 12);
    }

    #[test]
    fn a_pipeline_is_what_its_first_program_does() {
        assert!(shell_runs("grep -rn x . | head -40", &SEARCHERS));
        assert!(!shell_runs("grep -rn x . | head -40", &READERS));
        assert!(shell_runs("cd src && grep -rn x .", &SEARCHERS));
        assert!(shell_runs("git grep atif", &SEARCHERS));
        assert!(!shell_runs("git log -1", &SEARCHERS));
        assert!(shell_runs("sed -n '1,40p' Cargo.toml", &READERS));
    }

    /// Two states that differ digest differently, and two spellings of one
    /// state digest the same.
    #[test]
    fn a_digest_answers_what_was_asked_not_how_it_was_spelled() {
        let one = json!({"task": "a", "repo": ["x", "y"]});
        let other = json!({"repo": ["x", "y"], "task": "a"});
        assert_eq!(digest(&one), digest(&other));
        assert_ne!(digest(&one), digest(&json!({"task": "b", "repo": []})));
        assert_eq!(digest(&one).len(), 64);
    }

    /// One clock formats a document's timestamps and a session's file name.
    #[test]
    fn timestamps_format_in_utc() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_758_290_553_123), "2025-09-19T14:02:33.123Z");
        assert_eq!(stamp(1_758_290_553_123), "20250919T140233Z");
    }

    /// Usage lands on the step that spent it and totals across the session.
    #[test]
    fn usage_totals_across_the_session() {
        let mut first = Step::said(Source::Agent, "one");
        first.spent(Usage {
            prompt: 10,
            completion: 4,
        });
        let mut second = Step::said(Source::Agent, "two");
        second.spent(Usage {
            prompt: 20,
            completion: 6,
        });
        let document = document(&a_session(), &[first, second]);
        assert_eq!(document["steps"][0]["metrics"]["prompt_tokens"], 10);
        assert_eq!(document["final_metrics"]["total_prompt_tokens"], 30);
        assert_eq!(document["final_metrics"]["total_completion_tokens"], 10);
    }

    /// A step round-trips through JSON, which is how the log stores it.
    #[test]
    fn a_step_round_trips_through_json() {
        let step = Step::called(a_shell_call("pwd", "/tmp")).taking(5);
        let text = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&text).unwrap();
        assert_eq!(back.call.as_ref().unwrap().output, "/tmp");
        assert_eq!(back.milliseconds, Some(5));
        assert_eq!(back.source, Source::Agent);
    }
}
