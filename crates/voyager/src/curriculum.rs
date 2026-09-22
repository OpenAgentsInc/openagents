//! The curriculum: who decides what the agent attempts next.
//!
//! The paper's automatic curriculum proposes progressively harder
//! tasks from the agent's state and its completed and failed history.
//! Here it is a trait with two honest implementations and a declared
//! order between them:
//!
//! - [`Suite`] — the manifest's own task list. A world author knows
//!   the intended progression, and a declared list is also what an
//!   ablation needs: `Curriculum::Suite` is the paper's
//!   manual-curriculum arm.
//! - [`Generate`] — the paper's automatic arm: an Open Responses door
//!   proposes the next task as a JSON document carrying its goal and
//!   its program, with a warm-up schedule that scales the state it
//!   sees by how much the agent has finished.
//!
//! A `Curriculum` runs the declared tasks first, then the door — the
//! warm-up is the suite — and stops at `max_tasks` or when the door
//! answers `done`. A task's program is inline `script`, a `skill`
//! name in the store, or nothing yet — in which case the episode's
//! own generate step writes it and the refinement loop repairs it.

use std::collections::VecDeque;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::critic::Spec;
use crate::error::{Error, Result};
use crate::state::AgentState;

/// One task the curriculum hands the episode.
#[derive(Clone, Debug, Deserialize)]
pub struct Task {
    /// Its name in the record, such as `gather wood`.
    pub id: String,
    /// What it is trying to do, in the words the critic and the
    /// refinement prompt read.
    pub goal: String,
    /// The Lua program to run — the action agent's output when a
    /// world declares its tasks outright.
    #[serde(default)]
    pub script: Option<String>,
    /// A banked skill to run instead of a script, by `name` or
    /// `name@version`.
    #[serde(default)]
    pub skill: Option<String>,
    /// How the critic checks it.
    #[serde(default)]
    pub verify: Spec,
    /// Bank the program as a skill when it passes.
    #[serde(default)]
    pub bank: bool,
}

/// What the curriculum knows: the agent's state and its record so
/// far. `failed` pairs a task id with why it failed — the paper's
/// failed-task history is what keeps the door from proposing the
/// same impossible thing twice.
pub struct Context<'a> {
    /// The current agent state.
    pub state: &'a AgentState,
    /// Task ids that succeeded, in order.
    pub completed: &'a [String],
    /// Task ids that failed with their endings, in order.
    pub failed: &'a [(String, String)],
}

/// The curriculum a world runs: the declared list first, then the
/// door. `max_tasks` bounds the whole episode's task count either
/// way — the paper's iteration budget.
pub struct Curriculum {
    tasks: VecDeque<Task>,
    generate: Option<Generate>,
    max_tasks: usize,
    issued: usize,
}

impl Curriculum {
    /// Builds a curriculum from the manifest's section.
    ///
    /// # Errors
    ///
    /// A `generate` section must build its client — the door's URL
    /// must parse and a named credential must resolve.
    pub fn from_manifest(section: &CurriculumSection) -> Result<Self> {
        let generate = section
            .generate
            .as_ref()
            .map(Generate::from_manifest)
            .transpose()?;
        Ok(Curriculum {
            tasks: section.tasks.iter().cloned().collect(),
            generate,
            max_tasks: section.max_tasks as usize,
            issued: 0,
        })
    }

    /// Whether the curriculum can still propose — tasks remain, or a
    /// door stands behind them.
    #[must_use]
    pub fn live(&self) -> bool {
        !self.tasks.is_empty() || self.generate.is_some()
    }

    /// The built-in starter curriculum — what a world with no
    /// `curriculum` section runs. These are the episode's original
    /// survey/explore/gather legs expressed in the action language,
    /// so a manifest-free world and a declared one run the same loop.
    #[must_use]
    pub fn fallback() -> Self {
        let tasks = vec![
            Task {
                id: "survey the spawn area".to_string(),
                goal: "Read the world around spawn and report what is there.".to_string(),
                script: Some("local s = state() say(\"I see \" .. #s.nearby_blocks .. \" kinds of blocks\")".to_string()),
                skill: None,
                verify: Spec::Ran,
                bank: false,
            },
            Task {
                id: "explore north".to_string(),
                goal: "Walk north and see what the ground reveals.".to_string(),
                script: Some("explore(\"north\", 64)".to_string()),
                skill: None,
                verify: Spec::Moved { min_blocks: 8.0 },
                bank: false,
            },
            Task {
                id: "gather wood".to_string(),
                goal: "Dig log blocks until the inventory holds them.".to_string(),
                script: Some(concat!(
                    "local s = state() ",
                    "local logs = {} ",
                    "for _, block in ipairs(s.nearby_blocks) do ",
                    "if string.find(block, \"_log\", 1, true) then table.insert(logs, block) end ",
                    "end ",
                    "if #logs > 0 then mine(logs, 3) end",
                ).to_string()),
                skill: None,
                verify: Spec::Inventory {
                    item: "_log".to_string(),
                    at_least: 1,
                },
                bank: true,
            },
        ];
        Curriculum {
            max_tasks: tasks.len(),
            tasks: tasks.into(),
            generate: None,
            issued: 0,
        }
    }

    /// The next task, or `None` when the curriculum is done: the
    /// declared list first, then the door's proposals until the
    /// budget ends or the door says `done`.
    ///
    /// # Errors
    ///
    /// A `Generate` call that fails or answers unparseably is an
    /// error the episode records — the loop does not guess at a task
    /// the door never proposed.
    pub fn next(&mut self, context: &Context<'_>) -> Result<Option<Task>> {
        if self.issued >= self.max_tasks {
            return Ok(None);
        }
        if let Some(task) = self.tasks.pop_front() {
            self.issued += 1;
            return Ok(Some(task));
        }
        let Some(generate) = &mut self.generate else {
            return Ok(None);
        };
        match generate.propose(context)? {
            Proposed::Task(task) => {
                self.issued += 1;
                Ok(Some(task))
            }
            Proposed::Done => Ok(None),
        }
    }
}

/// The manifest's `curriculum` section.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CurriculumSection {
    /// The declared task list — the paper's manual arm and the
    /// warm-up schedule for the automatic one.
    #[serde(default)]
    pub tasks: Vec<Task>,
    /// The automatic arm: absent means the curriculum ends with the
    /// declared list.
    #[serde(default)]
    pub generate: Option<GenerateSection>,
    /// The action agent: the Open Responses door that writes a
    /// program when a task carries none, and rewrites one when the
    /// interpreter faults. Absent means a task without a `script`
    /// or a `skill` fails honestly rather than improvising.
    #[serde(default)]
    pub act: Option<GenerateSection>,
    /// The decision door for skill retrieval and `noul`
    /// verification — a `POST /v1/systemone` endpoint. Absent means
    /// the store lists and `noul` specs report unanswerable.
    #[serde(default)]
    pub decisions: Option<DecisionSection>,
    /// The most tasks one episode may attempt.
    #[serde(default = "default_max_tasks")]
    pub max_tasks: u32,
}

fn default_max_tasks() -> u32 {
    20
}

/// The manifest's `curriculum.decisions` section: which decision
/// door answers skill retrievals and `noul` verifications.
#[derive(Clone, Debug, Deserialize)]
pub struct DecisionSection {
    /// `POST {url}/v1/systemone` — a loopback address is a local
    /// `kev-serve`, anything else the live TypeSafe API.
    pub url: String,
    /// The model requests name, such as `kev-latest`.
    #[serde(default)]
    pub model: Option<String>,
}

/// The manifest's `curriculum.generate` section: which Open
/// Responses door proposes tasks, and how much state it sees. The
/// `act` section shares the shape — a door is a door; the section
/// name says which half of the loop it serves.
#[derive(Clone, Debug, Deserialize)]
pub struct GenerateSection {
    /// The door's base URL — `POST {url}/v1/responses`.
    pub url: String,
    /// The model the request names.
    #[serde(default = "default_generate_model")]
    pub model: String,
    /// The env var the bearer key comes from — never a manifest
    /// value. `VOYAGER_DOOR_KEY` when absent.
    #[serde(default)]
    pub key_env: Option<String>,
    /// Completed-task count at which the prompt stops simplifying —
    /// the paper's warm-up schedule.
    #[serde(default = "default_warmup")]
    pub warmup: u32,
}

fn default_generate_model() -> String {
    "coder".to_string()
}
fn default_warmup() -> u32 {
    5
}

/// What the door answered: a task, or `done` — the paper's
/// "automatic curriculum" may decide the world is finished.
enum Proposed {
    Task(Task),
    Done,
}

/// One Open Responses door, shared by the curriculum's `generate`
/// arm and the episode's `act` arm: `POST {url}/v1/responses`,
/// unstreamed, the answer text out of the output items.
pub(crate) struct Responses {
    http: reqwest::blocking::Client,
    url: String,
    model: String,
    key: Option<String>,
}

/// The automatic arm: a `Responses` call per proposal, with the
/// warm-up schedule scaling the prompt.
struct Generate {
    responses: Responses,
    warmup: u32,
}

impl Responses {
    /// Builds the client from a door section — the key resolves from
    /// the named env var, or `VOYAGER_DOOR_KEY`, or none (a local
    /// door that asks for none, the `kev-serve` shape).
    pub(crate) fn from_manifest(section: &GenerateSection) -> Result<Self> {
        let variable = section
            .key_env
            .clone()
            .unwrap_or_else(|| "VOYAGER_DOOR_KEY".to_string());
        let key = std::env::var(&variable).ok().filter(|key| !key.is_empty());
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|error| Error::episode(format!("responses door: {error}")))?;
        Ok(Responses {
            http,
            url: section.url.trim_end_matches('/').to_string(),
            model: section.model.clone(),
            key,
        })
    }

    /// `POST {url}/v1/responses`, unstreamed: the answer text out of
    /// the output items.
    pub(crate) fn ask(&self, instructions: &str, input: &str) -> Result<String> {
        let body = json!({
            "model": self.model,
            "instructions": instructions,
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": input}],
            }],
            "stream": false,
            "store": false,
            "tools": [],
            "tool_choice": "none",
        });
        let mut request = self.http.post(format!("{}/v1/responses", self.url));
        if let Some(key) = &self.key {
            request = request.bearer_auth(key);
        }
        let response = request
            .json(&body)
            .send()
            .map_err(|error| Error::episode(format!("curriculum door: {error}")))?;
        let status = response.status();
        let body: Value = response
            .json()
            .map_err(|error| Error::episode(format!("curriculum door body: {error}")))?;
        if !status.is_success() {
            let message = body["error"]["message"]
                .as_str()
                .unwrap_or("the door refused")
                .to_string();
            return Err(Error::episode(format!(
                "curriculum door {status}: {message}"
            )));
        }
        // The answer is the output items' text, joined — the same
        // walk the coder reader does, without the stream.
        let mut text = String::new();
        if let Some(output) = body.get("output").and_then(Value::as_array) {
            for item in output {
                for part in item["content"].as_array().into_iter().flatten() {
                    if part["type"].as_str() == Some("output_text")
                        && let Some(line) = part["text"].as_str()
                    {
                        text.push_str(line);
                    }
                }
            }
        }
        if let Some(text_field) = body.get("output_text").and_then(Value::as_str) {
            text.push_str(text_field);
        }
        if text.trim().is_empty() {
            return Err(Error::episode("responses door answered no text"));
        }
        Ok(text)
    }
}

impl Generate {
    /// The automatic arm over a `Responses` client.
    fn from_manifest(section: &GenerateSection) -> Result<Self> {
        Ok(Generate {
            responses: Responses::from_manifest(section)?,
            warmup: section.warmup,
        })
    }

    /// One proposal: the state and the record go out, a JSON task
    /// comes back. The reply's own `done` ends the curriculum.
    fn propose(&self, context: &Context<'_>) -> Result<Proposed> {
        let warming = (context.completed.len() as u32) < self.warmup;
        let state = json!({
            "position": context.state.position,
            "health": context.state.health,
            "food": context.state.food,
            "inventory": context.state.inventory,
            "nearby_blocks": context.state.nearby_blocks,
            "nearby_entities": context.state.nearby_entities,
            "completed": context.completed,
            "failed": context.failed.iter().map(|(id, why)| {
                json!({"task": id, "why": why})
            }).collect::<Vec<_>>(),
        });
        let instructions = if warming {
            concat!(
                "You are the curriculum of an open-ended Minecraft agent. ",
                "The agent is new to this world. Propose ONE next task it can plausibly ",
                "finish from its current state — early, foundational work before ",
                "ambition: survey, walk, gather, place. Reply with a JSON object only: ",
                "{\"id\": short name, \"goal\": one sentence, \"script\": a program in ",
                "the action language, \"verify\": {\"kind\": \"moved\"|\"inventory\"|",
                "\"block_at\"|\"noul\"|\"ran\", ...}, \"bank\": true|false}. ",
                "Or {\"done\": true} if the world offers nothing more."
            )
        } else {
            concat!(
                "You are the curriculum of an open-ended Minecraft agent. Propose ONE ",
                "next task that is a real step harder than what it has finished — new ",
                "materials, new places, composition over repetition — and still inside ",
                "what its state allows. Reply with a JSON object only: ",
                "{\"id\": short name, \"goal\": one sentence, \"script\": a program in ",
                "the action language, \"verify\": {\"kind\": \"moved\"|\"inventory\"|",
                "\"block_at\"|\"noul\"|\"ran\", ...}, \"bank\": true|false}. ",
                "Or {\"done\": true} if the world offers nothing more."
            )
        };
        let text = self.responses.ask(instructions, &state.to_string())?;
        parse_proposal(&text)
    }
}

/// The door's reply as a task: the first JSON object in the text,
/// parsed leniently — fields the reply omits take their defaults,
/// and a `done` flag ends the curriculum.
fn parse_proposal(text: &str) -> Result<Proposed> {
    let start = text.find('{');
    let end = text.rfind('}');
    let (Some(start), Some(end)) = (start, end) else {
        return Err(Error::episode(format!(
            "curriculum answer held no JSON: {}",
            &text[..text.len().min(120)]
        )));
    };
    let value: Value = serde_json::from_str(&text[start..=end])
        .map_err(|error| Error::episode(format!("curriculum answer did not parse: {error}")))?;
    if value["done"].as_bool().unwrap_or(false) {
        return Ok(Proposed::Done);
    }
    let goal = value["goal"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    if goal.is_empty() {
        return Err(Error::episode(
            "curriculum answer named no goal".to_string(),
        ));
    }
    let verify: Spec = serde_json::from_value(value["verify"].clone()).unwrap_or(Spec::Noul);
    Ok(Proposed::Task(Task {
        id: value["id"]
            .as_str()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or(&goal)
            .chars()
            .take(60)
            .collect(),
        goal,
        script: value["script"].as_str().map(str::to_string),
        skill: value["skill"].as_str().map(str::to_string),
        verify,
        bank: value["bank"].as_bool().unwrap_or(false),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(state: &'a AgentState) -> Context<'a> {
        Context {
            state,
            completed: &[],
            failed: &[],
        }
    }

    #[test]
    fn a_suite_issues_in_order_then_stops() {
        let section: CurriculumSection = serde_json::from_value(json!({
            "tasks": [
                {"id": "survey", "goal": "look around", "script": "state();",
                 "verify": {"kind": "ran"}},
                {"id": "walk", "goal": "move", "script": "walk(10, 0);",
                 "verify": {"kind": "moved", "min_blocks": 8.0}},
            ],
        }))
        .unwrap();
        let state = AgentState::default();
        let mut curriculum = Curriculum::from_manifest(&section).unwrap();
        let first = curriculum.next(&ctx(&state)).unwrap().unwrap();
        let second = curriculum.next(&ctx(&state)).unwrap().unwrap();
        assert_eq!(first.id, "survey");
        assert_eq!(second.id, "walk");
        assert!(curriculum.next(&ctx(&state)).unwrap().is_none());
        assert!(!curriculum.live());
    }

    #[test]
    fn the_task_budget_caps_the_list() {
        let section: CurriculumSection = serde_json::from_value(json!({
            "max_tasks": 1,
            "tasks": [
                {"id": "a", "goal": "one", "verify": {"kind": "ran"}},
                {"id": "b", "goal": "two", "verify": {"kind": "ran"}},
            ],
        }))
        .unwrap();
        let state = AgentState::default();
        let mut curriculum = Curriculum::from_manifest(&section).unwrap();
        assert!(curriculum.next(&ctx(&state)).unwrap().is_some());
        assert!(curriculum.next(&ctx(&state)).unwrap().is_none());
    }

    #[test]
    fn a_proposal_parses_out_of_wrapped_text() {
        let text = "Here is the next task.\n{\"id\": \"gather\", \"goal\": \"hold logs\", \"script\": \"mine([\\\"oak_log\\\"], 2);\", \"verify\": {\"kind\": \"inventory\", \"item\": \"_log\", \"at_least\": 2}, \"bank\": true}\nDone.";
        let Proposed::Task(task) = parse_proposal(text).unwrap() else {
            panic!("expected a task");
        };
        assert_eq!(task.id, "gather");
        assert!(task.script.is_some());
        assert!(task.bank);
    }

    #[test]
    fn a_done_answer_ends_the_curriculum() {
        let Proposed::Done = parse_proposal("{\"done\": true}").unwrap() else {
            panic!("expected done");
        };
    }

    #[test]
    fn an_empty_goal_is_an_error_not_a_task() {
        assert!(parse_proposal("{\"id\": \"x\"}").is_err());
        assert!(parse_proposal("no json here").is_err());
    }
}
