//! The Coder surface: start a chat with Coder on one of your computers and
//! follow it.
//!
//! A new chat is a NIP-HOST `task.create` on the chosen computer, the same
//! operation as the Computers surface's Order work. With the host's
//! auto-start policy on, the host runs Coder's engine on it in the chosen
//! workspace right away; otherwise the task waits in the host's inbox. The
//! list follows each task through the host's signed activity summaries
//! (phase and a one-line headline). Reading the engine's full transcript on
//! the phone is not implemented yet.

use coder_computers::{Computers, HostRecord, Snapshot};
use nostr::activity_summary::{ActivitySummary, Phase, SubjectKind};
use rust_native::input::InputRequest;
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};

/// The largest prompt, as NIP-HOST `task.create` allows.
const MAX_PROMPT_BYTES: usize = 16 * 1024;
const SHOWN_TASKS: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    NewChat,
    /// Use the next computer that can take work.
    NextComputer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Prompt,
}

pub struct CoderTab {
    instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
    selected: Option<String>,
    input: Option<InputRequest<Purpose>>,
    notice: Option<String>,
    tokens: u64,
    /// The first line of each chat this device started, by task ID: the
    /// host's headline is deliberately generic.
    titles: std::collections::BTreeMap<String, String>,
}

impl CoderTab {
    pub fn new(instance: String) -> Self {
        Self {
            instance,
            revision: 0,
            current: None,
            selected: None,
            input: None,
            notice: None,
            tokens: 0,
            titles: std::collections::BTreeMap::new(),
        }
    }

    pub fn input(&self) -> Option<&InputRequest<Purpose>> {
        self.input.as_ref()
    }

    /// The computers this device may order work on, in snapshot order.
    fn hosts(computers: &Computers) -> Vec<&HostRecord> {
        computers
            .snapshot()
            .hosts
            .iter()
            .filter(|host| computers.can_operate(&host.key))
            .collect()
    }

    fn chosen<'a>(&self, computers: &'a Computers) -> Option<&'a HostRecord> {
        let hosts = Self::hosts(computers);
        self.selected
            .as_ref()
            .and_then(|key| hosts.iter().find(|host| &host.key == key).copied())
            .or_else(|| hosts.first().copied())
    }

    /// The workspace a new chat uses: `openagents` when the computer lists
    /// it, else its first.
    fn workspace(host: &HostRecord) -> Option<String> {
        let listed = host.workspaces.as_ref()?;
        listed
            .iter()
            .find(|label| *label == "openagents")
            .or_else(|| listed.first())
            .cloned()
    }

    pub fn activate(&mut self, event: &Activation, computers: Option<&mut Computers>) {
        let Some(intent) = self
            .current
            .as_ref()
            .and_then(|view| view.activate(event).ok())
            .cloned()
        else {
            return;
        };
        let Some(computers) = computers else { return };
        match intent {
            Intent::NextComputer => {
                let hosts = Self::hosts(computers);
                let at = self
                    .chosen(computers)
                    .and_then(|chosen| hosts.iter().position(|h| h.key == chosen.key))
                    .unwrap_or(0);
                self.selected = hosts
                    .get((at + 1) % hosts.len().max(1))
                    .map(|host| host.key.clone());
            }
            Intent::NewChat => {
                let Some(host) = self.chosen(computers).map(|h| h.key.clone()) else {
                    return;
                };
                if computers
                    .snapshot()
                    .host(&host)
                    .and_then(Self::workspace)
                    .is_none()
                    && let Err(refusal) = computers.refresh_workspaces(&host)
                {
                    self.notice = Some(refusal.reason());
                    return;
                }
                let Some(record) = computers.snapshot().host(&host) else {
                    return;
                };
                let Some(workspace) = Self::workspace(record) else {
                    self.notice = Some(format!(
                        "{} lists no workspace for Coder yet.",
                        record.label
                    ));
                    return;
                };
                self.tokens += 1;
                self.notice = None;
                self.input = Some(InputRequest {
                    token: format!("coder-prompt-{}", self.tokens),
                    purpose: Purpose::Prompt,
                    label: "Message".into(),
                    prompt: format!(
                        "What should Coder do? It works on {} in {workspace}.",
                        record.label
                    ),
                    scan: false,
                    secret: false,
                    max_bytes: MAX_PROMPT_BYTES,
                });
            }
        }
    }

    pub fn cancel(&mut self, token: &str) {
        if self
            .input
            .as_ref()
            .is_some_and(|input| input.token == token)
        {
            self.input = None;
        }
    }

    pub fn submit(&mut self, token: &str, value: &str, computers: Option<&mut Computers>) {
        let Some(input) = self.input.take() else {
            return;
        };
        if input.accept(token, value).is_err() {
            self.input = Some(input);
            return;
        }
        let prompt = value.trim();
        let Some(computers) = computers else { return };
        if prompt.is_empty() {
            return;
        }
        let Some(record) = self.chosen(computers) else {
            return;
        };
        let (host, label) = (record.key.clone(), record.label.clone());
        let Some(workspace) = Self::workspace(record) else {
            return;
        };
        self.notice = Some(match computers.start_task(&host, &workspace, prompt) {
            Ok(task) => {
                let title: String = prompt
                    .lines()
                    .next()
                    .unwrap_or(prompt)
                    .chars()
                    .take(80)
                    .collect();
                self.titles.insert(task, title);
                format!("Sent to Coder on {label}.")
            }
            Err(refusal) => refusal.reason(),
        });
    }

    pub fn render(&mut self, computers: Option<&Computers>) -> Option<serde_json::Value> {
        self.revision += 1;
        let root = self.root(computers);
        let view = View::new(self.instance.clone(), self.revision, root)
            .validate()
            .ok()?;
        let value = serde_json::to_value(view.view()).ok();
        self.current = Some(view);
        value
    }

    fn root(&self, computers: Option<&Computers>) -> Node<Intent> {
        let mut children = vec![heading("coder-title", "Coder")];
        let chosen = computers.and_then(|c| self.chosen(c));
        let Some((computers, host)) = computers.zip(chosen) else {
            children.push(body(
                "coder-empty",
                "Add a computer under Computers, then start a chat with Coder on it.",
            ));
            return page(children);
        };
        let place = match Self::workspace(host) {
            Some(workspace) => format!("On {} · {workspace}", host.label),
            None => format!("On {}", host.label),
        };
        let mut row_children = vec![status("coder-computer", &place)];
        if Self::hosts(computers).len() > 1 {
            row_children.push(button("coder-next", "Change", Intent::NextComputer));
        }
        children.push(row("coder-place", row_children));
        children.push(button("coder-new", "New chat", Intent::NewChat));
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let rows = tasks(computers.snapshot(), &self.titles);
        if rows.is_empty() {
            children.push(status("coder-none", "No chats with Coder yet."));
        } else {
            children.push(Node {
                key: "coder-chats".into(),
                style: Style::default(),
                element: Element::List {
                    label: "Chats with Coder".into(),
                    children: rows,
                },
            });
        }
        page(children)
    }
}

/// One row per task, newest first, from the newest summary of each.
fn tasks(
    snapshot: &Snapshot,
    titles: &std::collections::BTreeMap<String, String>,
) -> Vec<Node<Intent>> {
    let mut newest: Vec<&ActivitySummary> = vec![];
    for summary in snapshot
        .activity
        .iter()
        .filter(|s| s.subject_kind == SubjectKind::Task)
    {
        match newest
            .iter_mut()
            .find(|known| known.host == summary.host && known.subject == summary.subject)
        {
            Some(known) if known.sequence < summary.sequence => *known = summary,
            Some(_) => {}
            None => newest.push(summary),
        }
    }
    newest.sort_by_key(|summary| std::cmp::Reverse(summary.updated_at));
    newest
        .into_iter()
        .take(SHOWN_TASKS)
        .map(|summary| {
            let key = format!("task-{}", &summary.subject[..16.min(summary.subject.len())]);
            let label = snapshot
                .host(&summary.host)
                .map_or("a computer", |host| host.label.as_str());
            stack(
                &key,
                vec![
                    text(
                        &format!("{key}-headline"),
                        titles.get(&summary.subject).unwrap_or(&summary.headline),
                        TextRole::Body,
                        WHITE,
                        true,
                    ),
                    status(
                        &format!("{key}-detail"),
                        &format!(
                            "{} · {label} · {}",
                            phase(summary.phase),
                            ago(snapshot.now, summary.updated_at)
                        ),
                    ),
                ],
            )
        })
        .collect()
}

fn phase(phase: Phase) -> &'static str {
    match phase {
        Phase::Queued => "Queued",
        Phase::Running => "Working",
        Phase::Waiting => "Waiting for you",
        Phase::Completed => "Done",
        Phase::Failed => "Failed",
        Phase::Cancelled => "Stopped",
        Phase::Unknown => "Unknown",
    }
}

fn ago(now: u64, then: u64) -> String {
    let seconds = now.saturating_sub(then);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

const WHITE: Color = Color::rgb(255, 255, 255);
const GRAY: Color = Color::rgb(153, 153, 153);

fn page(children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack("coder", children);
    node.style.gap = Some(Space::Sm);
    node.style.padding_top = Some(Space::Md);
    node.style.padding_end = Some(Space::Md);
    node.style.padding_start = Some(Space::Md);
    node
}

fn stack(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Xs),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn row(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Md),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    }
}

fn text(key: &str, value: &str, role: TextRole, foreground: Color, bold: bool) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(foreground),
            weight: bold.then_some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn heading(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Heading, WHITE, true)
}

fn body(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Body, WHITE, false)
}

fn status(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Status, GRAY, false)
}

fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(WHITE),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled: true,
            intent,
        },
    }
}
