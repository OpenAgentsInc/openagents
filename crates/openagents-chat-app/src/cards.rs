//! Typed reply actions and semantic card rows shared by chat adapters.
use crate::coder_tab::Availability;
use crate::eval_cards::{Button, CardView, Tone};
use crate::gym::{Gym, Here};
use openagents_chat::router::{Meta, Offer, Screen};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Retry,
    RunCoder,
    ConnectComputer,
    OpenScreen { screen: Screen },
    Followup { index: usize },
    Suggestion { id: String },
    RunCli { index: usize },
    Gym { id: String },
}

pub struct Chip {
    pub key: String,
    pub label: String,
    pub glyph: Glyph,
    pub action: Action,
}

pub struct ReplyActions {
    pub chips: Vec<Chip>,
    pub notice: Option<(String, String)>,
}

/// The worker's judgment is an observation; the target still needs authority.
pub fn wants_coder(meta: Option<&Meta>, computer_lane: bool) -> bool {
    computer_lane || meta.is_some_and(|meta| meta.offers.contains(&Offer::RunCoder))
}

pub fn reply_actions(
    meta: Option<&Meta>,
    used: &[String],
    computer_lane: bool,
    availability: &Availability<'_>,
    has_result: bool,
) -> ReplyActions {
    let mut output = ReplyActions {
        chips: vec![],
        notice: None,
    };
    let empty = Meta::default();
    let meta = meta.unwrap_or(&empty);
    let judged = wants_coder(Some(meta), computer_lane);
    match availability {
        Availability::Ready(host) if judged => output.chips.push(Chip {
            key: "coder-run".into(),
            label: format!("Run Coder on {}", host.label),
            glyph: Glyph::Computer,
            action: Action::RunCoder,
        }),
        Availability::NotConfigured if judged => output.chips.push(Chip {
            key: "coder-connect".into(),
            label: "Connect a computer".into(),
            glyph: Glyph::Add,
            action: Action::ConnectComputer,
        }),
        Availability::Connecting(host) if judged => {
            output.notice = Some((
                "coder-connecting".into(),
                format!("Connecting to {}…", host.label),
            ))
        }
        Availability::Offline(host) if judged => {
            output.notice = Some((
                "coder-offline".into(),
                format!("{} is offline.", host.label),
            ))
        }
        _ => {}
    }
    for (index, offer) in meta.offers.iter().enumerate() {
        let Offer::OpenScreen { screen } = offer else {
            continue;
        };
        if *screen == Screen::GymResult || (*screen == Screen::GymPublish && !has_result) {
            continue;
        }
        let connecting =
            *screen == Screen::Computers && matches!(availability, Availability::NotConfigured);
        if connecting && judged {
            continue;
        }
        let (label, glyph) = screen_chip(*screen, connecting);
        output.chips.push(Chip {
            key: format!("coder-screen-{index}"),
            label: label.into(),
            glyph,
            action: Action::OpenScreen { screen: *screen },
        });
    }
    for (index, followup) in crate::projection::followups(meta, used) {
        output.chips.push(Chip {
            key: format!("coder-followup-{index}"),
            label: clip(&followup.label, 60),
            glyph: Glyph::Ask,
            action: Action::Followup { index },
        });
    }
    output
}

pub fn suggestions(
    used: &[String],
) -> impl Iterator<Item = &'static crate::first_run::Suggestion> + '_ {
    crate::first_run::SUGGESTIONS
        .iter()
        .filter(|suggestion| {
            !openagents_chat::basic_chats::suggestion_used(
                used,
                Some(suggestion.id),
                &[suggestion.label, suggestion.message],
            )
        })
        .take(crate::first_run::SUGGESTIONS_SHOWN)
}

pub fn screen_chip(screen: Screen, connecting: bool) -> (&'static str, Glyph) {
    match screen {
        Screen::Wallet => ("Open Wallet", Glyph::Wallet),
        Screen::Computers if connecting => ("Connect a computer", Glyph::Add),
        Screen::Computers => ("Your computers", Glyph::Computer),
        Screen::Keys => ("Identity keys", Glyph::Key),
        Screen::Playtest => ("Playtest", Glyph::Flag),
        Screen::Report => ("Report a problem", Glyph::Flag),
        Screen::VerseGym => ("See the board", Glyph::Check),
        Screen::GymResult => ("See your result", Glyph::Check),
        Screen::GymPublish => ("Add to the Gym", Glyph::Add),
        Screen::GymTestSet => ("See the tests", Glyph::Ask),
    }
}

fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.into()
    } else {
        format!(
            "{}…",
            text.chars()
                .take(limit.saturating_sub(1))
                .collect::<String>()
        )
    }
}

pub fn chip<I>(chip: &Chip, intent: I) -> Node<I> {
    Node {
        key: chip.key.clone(),
        style: Style::default(),
        element: Element::Button {
            label: chip.label.clone(),
            enabled: true,
            icon: Some(Icon {
                glyph: chip.glyph,
                circular: false,
                pill: true,
            }),
            intent,
        },
    }
}

/// Mount the exact card values consumed by the phone's native hosts.
pub fn card<I>(view: &CardView, mut intent: impl FnMut(&str) -> I) -> Node<I> {
    let mut rows = vec![];
    let prefix = &view.id;
    let mut text = |suffix: &str, value: String, role: TextRole, strong: bool| {
        rows.push(Node {
            key: format!("{prefix}-{suffix}"),
            style: Style {
                weight: strong.then_some(TextWeight::Bold),
                ..Style::default()
            },
            element: Element::Text { value, role },
        });
    };
    if let Some(step) = &view.step {
        text("step", step.clone(), TextRole::Status, false);
    }
    text("title", view.title.clone(), TextRole::Heading, true);
    if let Some(badge) = &view.badge {
        text("badge", badge.clone(), TextRole::Status, true);
    }
    if let Some(compare) = &view.compare {
        text(
            "compare",
            format!(
                "{}: {}\n{}: {}",
                compare.without_label,
                compare.without.as_deref().unwrap_or("Not known yet"),
                compare.with_label,
                compare.with
            ),
            TextRole::Body,
            true,
        );
    }
    for (index, line) in view.lines.iter().enumerate() {
        text(
            &format!("line-{index}"),
            line.text.clone(),
            if line.tone == Tone::Quiet {
                TextRole::Status
            } else {
                TextRole::Body
            },
            line.tone == Tone::Strong,
        );
    }
    for (index, item) in view.items.iter().enumerate() {
        let marks = item
            .marks
            .iter()
            .map(|mark| match *mark {
                "check" => "✓",
                "cross" => "✕",
                "wait" => "…",
                "dot" => "•",
                _ => "",
            })
            .collect::<Vec<_>>()
            .join(" ");
        text(
            &format!("item-{index}"),
            format!(
                "{marks} {}{}{}",
                item.text,
                item.detail
                    .as_ref()
                    .map_or(String::new(), |detail| format!("\n{detail}")),
                item.trailing
                    .as_ref()
                    .map_or(String::new(), |value| format!(" · {value}"))
            ),
            TextRole::Body,
            false,
        );
    }
    for (index, progress) in view.progress.iter().enumerate() {
        text(
            &format!("progress-{index}"),
            format!(
                "{}: {} of {}",
                progress.label, progress.done, progress.total
            ),
            TextRole::Status,
            false,
        );
    }
    if let Some(source) = &view.source {
        text("source", source.clone(), TextRole::Status, false);
    }
    if view.busy {
        rows.push(Node {
            key: format!("{prefix}-working"),
            style: Style::default(),
            element: Element::Working {
                label: "Working…".into(),
            },
        });
    }
    for button in view
        .primary
        .iter()
        .chain(&view.secondary)
        .chain(&view.chips)
    {
        rows.push(card_button(button, intent(&button.id)));
    }
    Node {
        key: prefix.clone(),
        style: Style {
            background: Some(Color::rgb(26, 29, 34)),
            padding_top: Some(Space::Md),
            padding_bottom: Some(Space::Md),
            padding_start: Some(Space::Md),
            padding_end: Some(Space::Md),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children: rows,
        },
    }
}

fn card_button<I>(button: &Button, intent: I) -> Node<I> {
    Node {
        key: button.id.clone(),
        style: Style::default(),
        element: Element::Button {
            label: button.label.clone(),
            enabled: button.enabled,
            icon: None,
            intent,
        },
    }
}

/// Portable state for a client that receives host snapshots instead of holding credentials.
pub struct Cards {
    pub gym: Gym,
    pub actions: BTreeMap<String, Action>,
}
impl Default for Cards {
    fn default() -> Self {
        Self {
            gym: Gym::empty(),
            actions: BTreeMap::new(),
        }
    }
}
impl Cards {
    pub fn rows(&mut self, snapshot: &openagents_chat::service::Snapshot) -> Vec<Node<()>> {
        self.rows_with(snapshot, snapshot.busy, None)
    }
    pub fn rows_with(
        &mut self,
        snapshot: &openagents_chat::service::Snapshot,
        busy: bool,
        error: Option<&str>,
    ) -> Vec<Node<()>> {
        let failed =
            snapshot.failure.is_some() || snapshot.storage_error.is_some() || error.is_some();
        self.gym.begin();
        self.actions.clear();
        let mut rows = vec![];
        if let Some(id) = &snapshot.chat {
            for key in self.gym.cards_for(id, &snapshot.turns, &Here { busy }) {
                if let Some(view) = self.gym.cards().get(&key) {
                    let actions = &mut self.actions;
                    rows.push(card(view, |id| {
                        actions.insert(id.into(), Action::Gym { id: id.into() });
                    }));
                }
            }
        }
        let meta = crate::projection::actionable(&snapshot.turns, busy, failed);
        let reply = reply_actions(
            meta,
            &snapshot.used,
            snapshot.computer && crate::projection::completed(&snapshot.turns, busy, failed),
            &Availability::NotConfigured,
            self.gym.latest_result().is_some(),
        );
        if let Some((key, value)) = reply.notice {
            rows.push(Node {
                key,
                style: Style::default(),
                element: Element::Text {
                    value,
                    role: TextRole::Status,
                },
            });
        }
        for item in reply.chips {
            self.actions.insert(item.key.clone(), item.action.clone());
            rows.push(chip(&item, ()));
        }
        if let Some(meta) = meta {
            for (index, offer) in meta.offers.iter().enumerate() {
                if let Offer::Cli { argv, runs_on } = offer {
                    let key = format!("coder-cli-{index}");
                    let line = Node {
                        key: format!("{key}-command"),
                        style: Style::default(),
                        element: Element::Text {
                            value: Offer::command_line(argv),
                            role: TextRole::Code,
                        },
                    };
                    let run = Chip {
                        key: format!("{key}-run"),
                        label: "Run".into(),
                        glyph: Glyph::Terminal,
                        action: Action::RunCli { index },
                    };
                    self.actions.insert(run.key.clone(), run.action.clone());
                    rows.push(Node {
                        key,
                        style: Style::default(),
                        element: Element::Stack {
                            axis: Axis::Vertical,
                            children: vec![
                                line,
                                Node {
                                    key: format!("coder-cli-{index}-where"),
                                    style: Style::default(),
                                    element: Element::Text {
                                        value: match runs_on {
                                            openagents_chat::router::RunsOn::ThisDevice => {
                                                "Reads only. Runs on this computer."
                                            }
                                            _ => "Reads only. Runs on your computer.",
                                        }
                                        .into(),
                                        role: TextRole::Status,
                                    },
                                },
                                chip(&run, ()),
                            ],
                        },
                    });
                }
            }
        }
        if snapshot.failure.is_some()
            || snapshot.storage_error.is_some()
            || snapshot.turns.last().is_some_and(|turn| turn.stopped)
        {
            let retry = Chip {
                key: "talk-retry".into(),
                label: "Try again".into(),
                glyph: Glyph::Ask,
                action: Action::Retry,
            };
            self.actions.insert(retry.key.clone(), retry.action.clone());
            rows.push(chip(&retry, ()));
        }
        if snapshot.turns.is_empty() && !busy && !failed {
            for suggestion in suggestions(&snapshot.used) {
                let item = Chip {
                    key: format!("coder-suggest-{}", suggestion.id),
                    label: suggestion.label.into(),
                    glyph: Glyph::Ask,
                    action: Action::Suggestion {
                        id: suggestion.id.into(),
                    },
                };
                self.actions.insert(item.key.clone(), item.action.clone());
                rows.push(chip(&item, ()));
            }
        }
        rows
    }

    pub fn action(&self, id: &str) -> Option<Action> {
        self.actions.get(id).cloned()
    }
}

/// Card mounting precedes host execution and Gym integration in their own slices.
pub enum Effect {
    Requests(Vec<(u64, openagents_chat::service::Command)>),
    Navigate(Screen),
    Notice(String),
    Draft(String),
    None,
}
impl crate::session::Session {
    pub fn card_action(&mut self, key: &str) -> Effect {
        let Some(action) = self.cards.action(key) else {
            return Effect::None;
        };
        let snapshot = self.state().cloned().unwrap_or_default();
        if action == Action::Retry {
            return self
                .retry()
                .map_or(Effect::None, |request| Effect::Requests(vec![request]));
        }
        if self.busy() {
            return Effect::None;
        }
        let (text, suggestion_id) = match action {
            Action::Retry => return Effect::None,
            Action::Followup { index } => {
                let Some(followup) = crate::projection::actionable(
                    &snapshot.turns,
                    false,
                    snapshot.failure.is_some(),
                )
                .and_then(|meta| meta.followups.get(index)) else {
                    return Effect::None;
                };
                (followup.label.clone(), followup.answer.clone())
            }
            Action::Suggestion { id } => {
                let Some(suggestion) =
                    suggestions(&snapshot.used).find(|suggestion| suggestion.id == id)
                else {
                    return Effect::None;
                };
                (suggestion.message.into(), Some(suggestion.id.into()))
            }
            Action::ConnectComputer | Action::RunCoder => {
                return Effect::Navigate(Screen::Computers);
            }
            Action::OpenScreen { screen } => return Effect::Navigate(screen),
            Action::RunCli { .. } => {
                return Effect::Notice("Connect a computer to run this command.".into());
            }
            Action::Gym { id } => {
                let Some(action) = self.cards.gym.actions.get(&id).cloned() else {
                    return Effect::None;
                };
                match action {
                    crate::eval_cards::Action::VerseGym => {
                        return Effect::Navigate(Screen::VerseGym);
                    }
                    crate::eval_cards::Action::ConnectComputer => {
                        return Effect::Navigate(Screen::Computers);
                    }
                    crate::eval_cards::Action::ChangeIt { .. } => {
                        return Effect::Draft(crate::gym::CHANGE.into());
                    }
                    crate::eval_cards::Action::Say { text, fresh } => {
                        let mut requests = vec![];
                        if fresh {
                            requests.push(self.new_chat());
                        }
                        if let Some(send) =
                            self.submit(uuid::Uuid::new_v4().simple().to_string(), text)
                        {
                            requests.push(send);
                        }
                        return Effect::Requests(requests);
                    }
                    _ => return Effect::Notice("This Gym action needs a connected runner.".into()),
                }
            }
        };
        let mut requests = vec![];
        if let Some(id) = suggestion_id
            && let Some(chat) = &self.selected
        {
            requests.push(
                self.request(openagents_chat::service::Command::UseSuggestion {
                    chat: chat.clone(),
                    id,
                }),
            );
        }
        if let Some(send) = self.submit(uuid::Uuid::new_v4().simple().to_string(), text) {
            requests.push(send);
        }
        Effect::Requests(requests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use openagents_chat::basic_coder::Turn;
    use openagents_chat::service::{Command, Snapshot};
    use std::time::Instant;

    #[test]
    fn typed_reply_chips_deduplicate_connect_and_preserve_followup_identity() {
        let meta = Meta {
            offers: vec![
                Offer::RunCoder,
                Offer::OpenScreen {
                    screen: Screen::Computers,
                },
                Offer::OpenScreen {
                    screen: Screen::Wallet,
                },
            ],
            followups: vec![openagents_chat::router::Followup {
                label: "A question".into(),
                answer: Some("answer@2".into()),
            }],
            ..Meta::default()
        };
        let actions = reply_actions(Some(&meta), &[], true, &Availability::NotConfigured, false);
        assert_eq!(
            actions
                .chips
                .iter()
                .filter(|chip| chip.label == "Connect a computer")
                .count(),
            1
        );
        assert!(actions.chips.iter().any(|chip| chip.action
            == Action::OpenScreen {
                screen: Screen::Wallet
            }));
        assert!(
            actions
                .chips
                .iter()
                .any(|chip| chip.action == Action::Followup { index: 0 })
        );
        let used = vec!["id:answer".into()];
        assert!(
            !reply_actions(
                Some(&meta),
                &used,
                false,
                &Availability::NotConfigured,
                false
            )
            .chips
            .iter()
            .any(|chip| matches!(chip.action, Action::Followup { .. }))
        );
    }

    #[test]
    fn a_card_send_is_stable_and_only_current_minted_actions_are_admitted() {
        let mut session = Session::new(Instant::now());
        session.select("a");
        let snapshot = Snapshot {
            chat: Some("a".into()),
            ..Snapshot::default()
        };
        session.cards.rows(&snapshot);
        assert!(matches!(session.card_action("invented"), Effect::None));
        let Effect::Requests(requests) = session.card_action("coder-suggest-meta.who") else {
            panic!("send")
        };
        assert_eq!(requests.len(), 2);
        assert!(matches!(&requests[0].1, Command::UseSuggestion { id, .. } if id == "meta.who"));
        assert!(matches!(&requests[1].1, Command::Send { text, .. } if text == "Who are you?"));
        assert!(matches!(
            session.card_action("coder-suggest-meta.who"),
            Effect::None
        ));
        let (retry, repeated) = session.retry().unwrap();
        assert_ne!(retry, requests[1].0);
        assert_eq!(repeated, requests[1].1);
    }

    #[test]
    fn working_or_failed_replies_remove_suggestions_and_offered_actions() {
        let mut cards = Cards::default();
        let mut snapshot = Snapshot {
            chat: Some("a".into()),
            turns: vec![Turn::assistant(
                "Answer",
                Some(Meta {
                    offers: vec![Offer::RunCoder],
                    ..Meta::default()
                }),
            )],
            ..Snapshot::default()
        };
        cards.rows(&snapshot);
        assert!(cards.actions.contains_key("coder-connect"));
        snapshot.busy = true;
        cards.rows(&snapshot);
        assert!(!cards.actions.contains_key("coder-connect"));
        snapshot.busy = false;
        snapshot.failure = Some("Connection lost".into());
        cards.rows(&snapshot);
        assert!(!cards.actions.contains_key("coder-connect"));
    }
}
