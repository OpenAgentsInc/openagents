//! Agents (#12330): every agent the account's computers and cloud
//! environments report, with Stop and Message, and the landing queue, as
//! the site's `/settings/agents` page has them. A host reads `GET
//! /v1/agents` and `GET /v1/land`, fills [`State`], and sends Stop and
//! Message through `POST /v1/agents/actions`.

use rust_native::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Intent, Load, Paints, button, card, column, danger_button, header, key, row, section, text,
};

/// One agent, landing entry, or run a computer reports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Agent {
    pub computer: String,
    pub id: String,
    /// `agent` or `landing`.
    pub kind: String,
    pub title: String,
    pub status: String,
    pub line: Option<String>,
}

impl Agent {
    /// Whether it still runs, so Stop and Message apply.
    pub fn live(&self) -> bool {
        matches!(self.status.as_str(), "working" | "asking")
    }
}

/// One computer's agents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Computer {
    pub name: String,
    pub online: bool,
    pub agents: Vec<Agent>,
}

/// `GET /v1/agents`'s computers, with their agents and landing entries
/// (a chat's own turn shows in its chat, not here).
pub fn computers(answer: &Value) -> Vec<Computer> {
    answer["computers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|board| {
            let name = board["name"].as_str()?.to_owned();
            let agents: Vec<Agent> = board["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|item| matches!(item["kind"].as_str(), Some("agent" | "landing")))
                .filter_map(|item| {
                    Some(Agent {
                        computer: name.clone(),
                        id: item["id"].as_str()?.to_owned(),
                        kind: item["kind"].as_str().unwrap_or("agent").to_owned(),
                        title: item["title"].as_str().unwrap_or("Agent").to_owned(),
                        status: item["status"].as_str().unwrap_or("stopped").to_owned(),
                        line: item["line"]
                            .as_str()
                            .map(str::to_owned)
                            .filter(|l| !l.is_empty()),
                    })
                })
                .collect();
            (!agents.is_empty()).then(|| Computer {
                online: board["online"].as_bool().unwrap_or(false),
                name,
                agents,
            })
        })
        .collect()
}

/// One entry of the landing queue (`GET /v1/land`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Landing {
    pub id: String,
    pub summary: String,
    pub branch: String,
    /// `queued`, `landing`, `landed`, `bounced`, or `withdrawn`.
    pub state: String,
    pub issue: Option<u64>,
    pub reason: Option<String>,
}

/// The queue's waiting, landing, and failed entries, in that order.
pub fn landings(answer: &Value) -> Vec<Landing> {
    ["landing", "waiting", "failed"]
        .iter()
        .flat_map(|key| answer[*key].as_array().into_iter().flatten())
        .filter_map(|entry| {
            Some(Landing {
                id: entry["id"].as_str()?.to_owned(),
                summary: entry["summary"].as_str().unwrap_or_default().to_owned(),
                branch: entry["branch"].as_str().unwrap_or_default().to_owned(),
                state: entry["state"].as_str().unwrap_or("queued").to_owned(),
                issue: entry["issue"].as_u64(),
                reason: entry["reason"].as_str().map(str::to_owned),
            })
        })
        .collect()
}

/// What the page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub computers: Load<Vec<Computer>>,
    pub landing: Load<Vec<Landing>>,
    /// An action on its way, by item id.
    pub busy: Option<String>,
}

/// What a control on the page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Stop item `.1` on computer `.0`.
    Stop(String, String),
    /// Write to item `.1` on computer `.0`.
    Message(String, String),
    Refresh,
}

fn act(action: Action) -> Intent {
    Intent::Agents(action)
}

fn status_words(status: &str) -> &'static str {
    match status {
        "working" => "Working",
        "asking" => "Waiting for you",
        "done" => "Done",
        "failed" => "Failed",
        _ => "Stopped",
    }
}

fn landing_words(state: &str) -> &'static str {
    match state {
        "landing" => "Landing now",
        "landed" => "Landed",
        "bounced" => "Didn't land",
        "withdrawn" => "Withdrawn",
        _ => "Waiting to land",
    }
}

/// The view.
pub fn view(state: &State, paints: &Paints) -> Node<Intent> {
    let mut out = vec![header(
        "agents-title",
        "Agents",
        Some("What your computers and cloud environments are running, with Stop and Message."),
        paints,
    )];
    match state.computers.ready() {
        Some(computers) if !computers.is_empty() => {
            for computer in computers {
                let title = if computer.online {
                    computer.name.clone()
                } else {
                    format!("{} · offline", computer.name)
                };
                out.push(section(
                    &key("agents", &computer.name),
                    &title,
                    computer
                        .agents
                        .iter()
                        .map(|agent| {
                            let mut detail = status_words(&agent.status).to_string();
                            if let Some(line) = &agent.line {
                                detail = format!("{detail} · {line}");
                            }
                            let busy = state.busy.as_deref() == Some(agent.id.as_str());
                            let mut controls = Vec::new();
                            if agent.live() && agent.kind == "agent" && !busy {
                                controls.push(button(
                                    &key(
                                        "agent-message",
                                        &format!("{}:{}", agent.computer, agent.id),
                                    ),
                                    "Message",
                                    act(Action::Message(agent.computer.clone(), agent.id.clone())),
                                ));
                                controls.push(danger_button(
                                    &key("agent-stop", &format!("{}:{}", agent.computer, agent.id)),
                                    "Stop",
                                    act(Action::Stop(agent.computer.clone(), agent.id.clone())),
                                    paints,
                                ));
                            }
                            row(
                                &key("agent", &format!("{}:{}", agent.computer, agent.id)),
                                &agent.title,
                                Some(&detail),
                                controls,
                                paints,
                            )
                        })
                        .collect(),
                    paints,
                ));
            }
        }
        _ => out.push(card(
            "agents-none",
            vec![text(
                "agents-none-line",
                match &state.computers {
                    Load::Failed(error) => error.as_str(),
                    Load::Ready(_) => {
                        "Nothing is running now. Background agents your computers start show here."
                    }
                    _ => "Loading…",
                },
                paints.muted,
            )],
            paints,
        )),
    }
    match state.landing.ready() {
        Some(entries) if !entries.is_empty() => out.push(section(
            "agents-landing",
            "Landing queue",
            entries
                .iter()
                .map(|entry| {
                    let title = match entry.issue {
                        Some(issue) if !entry.summary.is_empty() => {
                            format!("#{issue} {}", entry.summary)
                        }
                        Some(issue) => format!("#{issue}"),
                        None if entry.summary.is_empty() => entry.branch.clone(),
                        None => entry.summary.clone(),
                    };
                    let mut detail = format!("{} · {}", landing_words(&entry.state), entry.branch);
                    if let Some(reason) = &entry.reason {
                        detail = format!("{detail} · {reason}");
                    }
                    row(
                        &key("landing", &entry.id),
                        &title,
                        Some(&detail),
                        Vec::new(),
                        paints,
                    )
                })
                .collect(),
            paints,
        )),
        Some(_) => out.push(section(
            "agents-landing",
            "Landing queue",
            vec![text(
                "agents-landing-none",
                "Nothing is waiting to land.",
                paints.muted,
            )],
            paints,
        )),
        // The queue is for the people who land changes; others don't see it.
        None => {}
    }
    out.push(button("agents-refresh", "Refresh", act(Action::Refresh)));
    column("agents", out)
}
