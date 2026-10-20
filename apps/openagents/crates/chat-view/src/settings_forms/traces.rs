//! Traces (#12329): the account's uploaded agent traces and one trace's
//! steps and agents, with Share and Delete, as the site's
//! `/settings/traces` pages have them. A host reads `GET /v1/traces`,
//! `GET /v1/traces/{id}`, and `GET /v1/traces/{id}/agents`, fills
//! [`State`], and sends each [`Action`] to the same API.

use rust_native::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Intent, Load, Paints, button, card, column, danger_button, header, row, section, text,
};

/// The most steps the viewer draws, as the site's page does.
pub const MAX_STEPS: usize = 400;
/// The most characters of one step's text.
const STEP_CHARS: usize = 600;

/// One saved trace, as `GET /v1/traces` lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub agent: String,
    pub model: String,
    pub steps: u64,
    pub uploaded_unix: u64,
    pub shared: bool,
    pub share_url: Option<String>,
}

impl Summary {
    pub fn from_value(value: &Value) -> Option<Self> {
        let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
        Some(Self {
            id: value["id"].as_str()?.to_owned(),
            title: text("title"),
            agent: text("agent"),
            model: text("model"),
            steps: value["steps"].as_u64().unwrap_or(0),
            uploaded_unix: value["uploaded_unix"].as_u64().unwrap_or(0),
            shared: value["shared"].as_bool().unwrap_or(false),
            share_url: value["share_url"].as_str().map(str::to_owned),
        })
    }

    /// `GET /v1/traces`'s answer.
    pub fn list(answer: &Value) -> Vec<Self> {
        answer["traces"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Self::from_value)
            .collect()
    }

    fn name(&self) -> &str {
        if self.title.trim().is_empty() {
            "Untitled trace"
        } else {
            &self.title
        }
    }

    fn detail(&self) -> String {
        let mut parts = Vec::new();
        if !self.agent.is_empty() {
            parts.push(self.agent.clone());
        }
        if !self.model.is_empty() {
            parts.push(self.model.clone());
        }
        parts.push(match self.steps {
            1 => "1 step".into(),
            n => format!("{n} steps"),
        });
        if self.shared {
            parts.push("Shared".into());
        }
        parts.join(" · ")
    }
}

/// One step of a trace, in plain words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepRow {
    /// Who: "You", "Agent", or "System".
    pub who: String,
    pub text: String,
    /// The tools it called, by name.
    pub tools: Vec<String>,
}

/// The steps of an ATIF document, at most [`MAX_STEPS`].
pub fn steps_of(document: &Value) -> Vec<StepRow> {
    document["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_STEPS)
        .map(|step| {
            let who = match step["source"].as_str().unwrap_or_default() {
                "user" => "You",
                "system" => "System",
                _ => "Agent",
            };
            let message = &step["message"];
            let text = message.as_str().map(str::to_owned).unwrap_or_else(|| {
                message
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|part| part["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            let tools = step["tool_calls"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|call| call["function_name"].as_str().map(str::to_owned))
                .collect();
            StepRow {
                who: who.into(),
                text: clip(&text),
                tools,
            }
        })
        .collect()
}

fn clip(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= STEP_CHARS {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(STEP_CHARS).collect();
    out.push('…');
    out
}

/// One agent under a trace (`GET /v1/traces/{id}/agents`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentRow {
    pub id: String,
    pub title: String,
    pub steps: u64,
    /// The trace's own conversation, not an agent it started.
    pub main: bool,
}

impl AgentRow {
    pub fn list(answer: &Value) -> Vec<Self> {
        answer["agents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| {
                Some(Self {
                    id: a["id"].as_str()?.to_owned(),
                    title: a["title"]
                        .as_str()
                        .filter(|t| !t.trim().is_empty())
                        .or(a["agent"].as_str())
                        .unwrap_or("Agent")
                        .to_owned(),
                    steps: a["steps"].as_u64().unwrap_or(0),
                    main: a["parent"].is_null(),
                })
            })
            .collect()
    }
}

/// The open trace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Open {
    pub id: String,
    /// The agent shown instead of the trace's own conversation.
    pub agent: Option<String>,
    pub steps: Load<Vec<StepRow>>,
    pub agents: Load<Vec<AgentRow>>,
}

/// What the page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub list: Load<Vec<Summary>>,
    pub open: Option<Open>,
    /// The trace waiting for Delete to be confirmed.
    pub confirm_delete: Option<String>,
    pub busy: bool,
}

impl State {
    pub fn summary(&self, id: &str) -> Option<&Summary> {
        self.list.ready()?.iter().find(|t| t.id == id)
    }
}

/// What a control on the page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Open a trace, or one of its agents.
    Open(String, Option<String>),
    /// Back to the list.
    List,
    Share(String, bool),
    Copy(String),
    AskDelete(Option<String>),
    Delete(String),
}

fn act(action: Action) -> Intent {
    Intent::Traces(action)
}

/// The view.
pub fn view(state: &State, paints: &Paints) -> Node<Intent> {
    let body = match &state.open {
        Some(open) => trace(state, open, paints),
        None => list(state, paints),
    };
    column("traces", body)
}

fn loading(key: &str, load: &Load<impl Sized>, empty: &str, paints: &Paints) -> Node<Intent> {
    let line = match load {
        Load::Failed(error) => error.as_str(),
        Load::Ready(_) => empty,
        _ => "Loading…",
    };
    card(
        key,
        vec![text(&format!("{key}-line"), line, paints.muted)],
        paints,
    )
}

fn list(state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let mut out = vec![header(
        "traces-title",
        "Traces",
        Some(
            "Agent traces you uploaded with `openagents traces upload`. Only you see them until you share one.",
        ),
        paints,
    )];
    match state.list.ready() {
        Some(traces) if !traces.is_empty() => out.push(card(
            "traces-list",
            traces
                .iter()
                .map(|t| {
                    row(
                        &format!("trace-{}", t.id),
                        t.name(),
                        Some(&t.detail()),
                        vec![button(
                            &format!("trace-open-{}", t.id),
                            "Open",
                            act(Action::Open(t.id.clone(), None)),
                        )],
                        paints,
                    )
                })
                .collect(),
            paints,
        )),
        _ => out.push(loading(
            "traces-empty",
            &state.list,
            "No traces yet. Upload one with `openagents traces upload`.",
            paints,
        )),
    }
    out
}

fn trace(state: &State, open: &Open, paints: &Paints) -> Vec<Node<Intent>> {
    let summary = state.summary(&open.id);
    let title = summary.map_or("Trace", Summary::name);
    let detail = summary.map(Summary::detail);
    let mut out = vec![header("trace-title", title, detail.as_deref(), paints)];
    let mut controls = vec![button("trace-back", "All traces", act(Action::List))];
    if let Some(summary) = summary {
        controls.push(button(
            "trace-share",
            if summary.shared {
                "Stop sharing"
            } else {
                "Share"
            },
            act(Action::Share(summary.id.clone(), !summary.shared)),
        ));
        if let Some(url) = summary.share_url.as_ref().filter(|_| summary.shared) {
            controls.push(button(
                "trace-copy",
                "Copy link",
                act(Action::Copy(url.clone())),
            ));
        }
        controls.push(
            if state.confirm_delete.as_deref() == Some(summary.id.as_str()) {
                danger_button(
                    "trace-delete-confirm",
                    "Delete for good",
                    act(Action::Delete(summary.id.clone())),
                    paints,
                )
            } else {
                danger_button(
                    "trace-delete",
                    "Delete",
                    act(Action::AskDelete(Some(summary.id.clone()))),
                    paints,
                )
            },
        );
    }
    let shared_line = summary
        .filter(|s| s.shared)
        .and_then(|s| s.share_url.clone())
        .map(|url| format!("Anyone with the link can read it: {url}"));
    out.push(card(
        "trace-actions",
        vec![row(
            "trace-actions-row",
            if summary.is_some_and(|s| s.shared) {
                "Shared"
            } else {
                "Private"
            },
            Some(
                shared_line
                    .as_deref()
                    .unwrap_or("Only you can see this trace."),
            ),
            controls,
            paints,
        )],
        paints,
    ));
    if let Some(agents) = open.agents.ready().filter(|a| a.len() > 1) {
        out.push(section(
            "trace-agents",
            "Agents",
            agents
                .iter()
                .map(|a| {
                    let shown = open.agent.as_deref() == Some(a.id.as_str())
                        || (open.agent.is_none() && a.main);
                    let steps = match a.steps {
                        1 => "1 step".to_string(),
                        n => format!("{n} steps"),
                    };
                    row(
                        &format!("trace-agent-{}", a.id),
                        &a.title,
                        Some(&steps),
                        if shown {
                            Vec::new()
                        } else {
                            vec![button(
                                &format!("trace-agent-open-{}", a.id),
                                "Show",
                                act(Action::Open(
                                    open.id.clone(),
                                    (!a.main).then(|| a.id.clone()),
                                )),
                            )]
                        },
                        paints,
                    )
                })
                .collect(),
            paints,
        ));
    }
    match open.steps.ready() {
        Some(steps) if !steps.is_empty() => out.push(section(
            "trace-steps",
            "Steps",
            steps
                .iter()
                .enumerate()
                .map(|(ix, step)| {
                    let mut detail = step.text.clone();
                    if !step.tools.is_empty() {
                        if !detail.is_empty() {
                            detail.push('\n');
                        }
                        detail.push_str(&format!("Used {}", step.tools.join(", ")));
                    }
                    row(
                        &format!("trace-step-{ix}"),
                        &step.who,
                        Some(&detail),
                        Vec::new(),
                        paints,
                    )
                })
                .collect(),
            paints,
        )),
        _ => out.push(loading(
            "trace-steps-empty",
            &open.steps,
            "This trace has no steps.",
            paints,
        )),
    }
    out
}
