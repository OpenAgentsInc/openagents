//! A chat asks one of the person's own saved agents, such as Foreman
//! (#11934).
//!
//! When the signed-in person has saved agents (`/v1/agent-definitions`), a
//! message goes first to [`AgentsDoor`]: Gemini on Vertex AI ("Google
//! first") with one typed function, `ask_agent`, whose `agent` is an enum
//! of their agents' names, and `general_chat`. The model's typed function
//! choice is the route; no words of the message are matched here. Asking
//! creates an ordinary assignment (`/v1/agent-assignments`) on a computer
//! of theirs that is online with that agent's runtime, waits for the
//! agent's answer, and returns it as the agent's own reply. The agent runs
//! on that computer exactly as an assignment from the phone does: the same
//! saved instructions, access, and conversation, which the person can open
//! in the app.
//!
//! This is how `openagents chat send "Ask Foreman …" --project NAME` reaches
//! an agent from the frozen 1.0.0 CLI: `--project` sends through
//! `/v1/threads`, whose answer runs these doors.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use openagents_chat::basic_coder::{self, Door, Failure, Reply, Role as TurnRole, Turn};
use openagents_chat::router::Context;
use openagents_sdk::agents::AgentDefinition;
use openagents_sdk::agents::assignments::{AssignmentSpec, AssignmentState, CreateAssignment};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::chat_store::Store;
use crate::chat_vision::{GEMINI, Gemini, gemini_text};
use crate::connections::chat::{HANDOFF, gemini_call};

/// How long a chat waits for an agent's answer. An agent reads GitHub and
/// its tools, so it takes minutes, not seconds.
pub(crate) const DEADLINE: Duration = Duration::from_secs(600);
/// How long the door itself waits, a little inside [`DEADLINE`] so the
/// reply says the agent is still working instead of timing out.
const WAIT: Duration = Duration::from_secs(570);
const POLL: Duration = Duration::from_secs(3);
/// The tool's name.
const ASK: &str = "ask_agent";
/// The most agents offered to the model.
const MAX_AGENTS: usize = 32;

/// A door that asks one of the person's saved agents, or hands the message
/// to `fallback`.
pub(crate) struct AgentsDoor {
    pub gemini: Gemini,
    pub store: Store,
    pub owner: String,
    /// The chat turn this answers, so a retried turn reuses its assignment.
    pub request_id: String,
    pub agents: Vec<AgentDefinition>,
    /// The chat's project repository (`OWNER/NAME`), to prefer the computer
    /// project of the same name.
    pub repository: Option<String>,
    pub fallback: Option<Box<dyn Door>>,
}

/// The agents door for `owner`'s turn, when they have saved agents and this
/// server has Gemini.
pub(crate) async fn door(
    store: &Store,
    owner: &str,
    request_id: &str,
    repository: Option<String>,
) -> Option<AgentsDoor> {
    if !crate::chat_store::is_account_owner(owner) {
        return None;
    }
    let agents: Vec<_> = crate::custom_agents::active(store, owner)
        .await
        .ok()?
        .into_iter()
        .take(MAX_AGENTS)
        .collect();
    if agents.is_empty() {
        return None;
    }
    let gemini = crate::chat_vision::doors_gemini()?;
    Some(AgentsDoor {
        gemini,
        store: store.clone(),
        owner: owner.to_owned(),
        request_id: request_id.to_owned(),
        agents,
        repository,
        fallback: None,
    })
}

/// What the model is told.
pub(crate) fn guidance(agents: &[AgentDefinition]) -> String {
    let list: Vec<String> = agents
        .iter()
        .map(|a| {
            if a.spec.description.trim().is_empty() {
                format!("- {}", a.spec.name)
            } else {
                format!("- {}: {}", a.spec.name, a.spec.description.trim())
            }
        })
        .collect();
    format!(
        "\n\nThe person has saved these agents of their own, which run on their computers:\n{}\n\
When the person asks one of these agents something or asks for it by name (\"ask Foreman …\", \"Foreman, what …\"), \
or asks for exactly the work one of them describes, call {ASK} with that agent and the question in the person's words, \
including anything from the conversation the agent needs. Ask only one agent, once. \
For anything else, call {HANDOFF} instead of answering.",
        list.join("\n")
    )
}

/// The function declarations Gemini gets.
pub(crate) fn tools(agents: &[AgentDefinition]) -> Vec<Value> {
    let names: Vec<&str> = agents.iter().map(|a| a.spec.name.as_str()).collect();
    vec![
        json!({
            "type": "function",
            "name": ASK,
            "description": "Ask one of the person's saved agents a question or give it a task, and get its answer.",
            "parameters": {
                "type": "object",
                "properties": {
                    "agent": {"type": "string", "enum": names, "description": "The agent's name."},
                    "question": {"type": "string", "description": "What to ask the agent, complete on its own."}
                },
                "required": ["agent", "question"]
            }
        }),
        json!({
            "type": "function",
            "name": HANDOFF,
            "description": "Hand the message to OpenAgents' general chat, for anything that isn't for one of the person's agents.",
            "parameters": {"type": "object", "properties": {}},
        }),
    ]
}

fn request(instructions: &str, turns: &[Turn], agents: &[AgentDefinition]) -> Value {
    let input: Vec<Value> = turns
        .iter()
        .map(|turn| {
            let role = match turn.role {
                TurnRole::User => "user",
                TurnRole::Assistant => "assistant",
            };
            json!({"type": "message", "role": role, "content": turn.text})
        })
        .collect();
    json!({
        "model": GEMINI,
        "instructions": instructions,
        "input": input,
        "tools": tools(agents),
        "stream": false,
        "store": false,
    })
}

/// The assignment request for this turn and agent: the same turn retried
/// finds the same assignment instead of starting the agent twice.
pub(crate) fn assignment_id(turn: &str, agent: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"openagents.web.chat.ask-agent.v1\0");
    hash.update(turn.as_bytes());
    hash.update(b"\0");
    hash.update(agent.as_bytes());
    format!("{:x}", hash.finalize())
}

/// How asking ended, as the reply shows it.
async fn ask(door: &AgentsDoor, agent: &AgentDefinition, question: &str) -> String {
    let name = &agent.spec.name;
    let hosts = match crate::agent_lab::online_hosts(&door.store, &door.owner).await {
        Ok(h) => h,
        Err(_) => {
            return format!(
                "We couldn't check your computers just now, so {name} wasn't asked. Try again in a minute."
            );
        }
    };
    let prefer = door
        .repository
        .as_deref()
        .and_then(|r| r.rsplit('/').next());
    let Ok((host, project)) =
        openagents_sdk::agents::place::place(&hosts, agent, None, None, prefer)
    else {
        return format!(
            "{name} runs on your computer, and none is online with {} and a project. \
Open the OpenAgents desktop app on that computer, then ask again.",
            match agent.spec.runtime.as_str() {
                "claude-code" => "Claude Code",
                "codex" => "Codex",
                _ => "its runtime",
            }
        );
    };
    let body = CreateAssignment {
        request_id: assignment_id(&door.request_id, &agent.id),
        chat: None,
        spec: AssignmentSpec {
            agent: agent.id.clone(),
            agent_revision: agent.revision,
            host: host.id.clone(),
            project: Some(project),
            task: question.to_owned(),
        },
    };
    if body.spec.validate().is_err() {
        return format!("That question is too long for {name}. Ask it in fewer words.");
    }
    let created = crate::agent_assignments::mutate(
        &door.store,
        &door.owner,
        crate::agent_assignments::Change::Create {
            body,
            definition: agent.clone(),
        },
    )
    .await;
    let id = match created {
        Ok(Ok(a)) => a.id,
        _ => {
            return format!(
                "We couldn't give {name} the question just now. Try again in a minute."
            );
        }
    };
    let started = tokio::time::Instant::now();
    loop {
        tokio::time::sleep(POLL).await;
        let current = crate::agent_assignments::load(&door.store, &door.owner)
            .await
            .ok()
            .and_then(|(b, _)| b.assignments.get(&id).cloned());
        if let Some(a) = current {
            match a.state {
                AssignmentState::Completed => {
                    let answer = a.result.unwrap_or_default();
                    return format!("**{name}** on {}:\n\n{}", host.name, answer.trim());
                }
                AssignmentState::Failed | AssignmentState::Stopped | AssignmentState::Unknown => {
                    return format!(
                        "{name} didn't finish on {}. Open its conversation in the OpenAgents app to see what happened.",
                        host.name
                    );
                }
                AssignmentState::Queued | AssignmentState::Running => {}
            }
        }
        if started.elapsed() >= WAIT {
            return format!(
                "{name} is still working on {}. Its answer will be in its conversation in the OpenAgents app.",
                host.name
            );
        }
    }
}

/// How the model routed the message.
enum Route {
    Ask {
        agent: AgentDefinition,
        question: String,
    },
    Handoff,
}

async fn route(door: &AgentsDoor, instructions: &str, turns: &[Turn]) -> Result<Route, String> {
    let body = door
        .gemini
        .body(&request(instructions, turns, &door.agents))?;
    let reply = gemini_call(&door.gemini, &body).await?;
    let call = reply["candidates"][0]["content"]["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|part| part["functionCall"].as_object().cloned());
    let Some(call) = call else {
        // An answer without a tool isn't this door's to give.
        let _ = gemini_text(&reply);
        return Ok(Route::Handoff);
    };
    if call.get("name").and_then(Value::as_str) != Some(ASK) {
        return Ok(Route::Handoff);
    }
    let args = call.get("args").cloned().unwrap_or_default();
    let name = args["agent"].as_str().unwrap_or_default();
    let question = args["question"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_owned();
    match door.agents.iter().find(|a| a.spec.name == name) {
        Some(agent) if !question.is_empty() => Ok(Route::Ask {
            agent: agent.clone(),
            question,
        }),
        _ => Ok(Route::Handoff),
    }
}

impl Door for AgentsDoor {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let instructions = format!(
            "{}{}",
            basic_coder::instructions(&context),
            guidance(&self.agents)
        );
        let fallback = self
            .fallback
            .as_ref()
            .map(|door| door.ask(turns.clone(), context.clone(), reply.clone()));
        let door = AgentsDoor {
            gemini: self.gemini.clone(),
            store: self.store.clone(),
            owner: self.owner.clone(),
            request_id: self.request_id.clone(),
            agents: self.agents.clone(),
            repository: self.repository.clone(),
            fallback: None,
        };
        Box::pin(async move {
            match route(&door, &instructions, &turns).await {
                Ok(Route::Ask { agent, question }) => {
                    let started = std::time::Instant::now();
                    let text = ask(&door, &agent, &question).await;
                    // Timing only: never the question or the answer.
                    eprintln!(
                        "openagents-web: chat agents: asked a saved agent in {} ms",
                        started.elapsed().as_millis()
                    );
                    let mut reply = basic_coder::lock(&reply);
                    reply.text = text;
                    reply.model = Some(door.gemini.row.id.clone());
                    reply.meta.tier = Some("model".into());
                    reply.meta.route = Some("agents".into());
                    reply.done = true;
                }
                other => {
                    if let Err(why) = &other {
                        eprintln!("openagents-web: chat agents: {why}; the hosted chat answers");
                    }
                    match fallback {
                        Some(job) => job.await,
                        None => {
                            basic_coder::lock(&reply).failure = Some(Failure::Transport(
                                other.err().unwrap_or_else(|| "no general chat".into()),
                            ));
                        }
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_sdk::agents::{Access, AgentSpec};

    fn agent(runtime: &str) -> AgentDefinition {
        AgentDefinition {
            id: "a".repeat(64),
            revision: 1,
            spec: AgentSpec {
                name: "Foreman".into(),
                description: "Keeps the whole picture.".into(),
                instructions: "Look.".into(),
                runtime: runtime.into(),
                model: None,
                access: Access::HostDefault,
                fallbacks: vec![],
                autonomy: None,
            },
            deleted: false,
        }
    }
    #[test]
    fn a_retried_turn_reuses_its_assignment() {
        let id = assignment_id("turn-1", &"a".repeat(64));
        assert_eq!(id, assignment_id("turn-1", &"a".repeat(64)));
        assert_ne!(id, assignment_id("turn-2", &"a".repeat(64)));
        assert!(openagents_sdk::agents::valid_id(&id));
    }

    #[test]
    fn the_model_chooses_among_the_persons_agents_by_name() {
        let tools = tools(&[agent("claude-code")]);
        assert_eq!(
            tools[0]["parameters"]["properties"]["agent"]["enum"],
            json!(["Foreman"])
        );
        assert_eq!(tools[1]["name"], HANDOFF);
        assert!(guidance(&[agent("claude-code")]).contains("- Foreman: Keeps the whole picture."));
    }
}
