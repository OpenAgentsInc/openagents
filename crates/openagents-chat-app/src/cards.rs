//! Typed reply actions and semantic card rows shared by chat adapters.
use crate::coder_tab::Availability;
use crate::eval_cards::{Button, CardView, Compare, Item, SheetView, Tone};
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
    openagents_chat::delegation::offered(meta, computer_lane)
}

/// A presentation target contains no credential or execution authority.
pub enum Target<'a> {
    NotConfigured,
    Connecting(&'a str),
    Offline(&'a str),
    Ready(&'a str),
}

pub fn reply_actions(
    meta: Option<&Meta>,
    used: &[String],
    computer_lane: bool,
    availability: &Availability<'_>,
    has_result: bool,
) -> ReplyActions {
    let target = match availability {
        Availability::NotConfigured => Target::NotConfigured,
        Availability::Connecting(host) => Target::Connecting(&host.label),
        Availability::Offline(host) => Target::Offline(&host.label),
        Availability::Ready(host) => Target::Ready(&host.label),
    };
    reply_actions_for(meta, used, computer_lane, &target, has_result)
}

pub fn reply_actions_for(
    meta: Option<&Meta>,
    used: &[String],
    computer_lane: bool,
    availability: &Target<'_>,
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
        Target::Ready(host) if judged => output.chips.push(Chip {
            key: "coder-run".into(),
            label: format!("Run Coder on {}", host),
            glyph: Glyph::Computer,
            action: Action::RunCoder,
        }),
        Target::NotConfigured if judged => output.chips.push(Chip {
            key: "coder-connect".into(),
            label: "Connect a computer".into(),
            glyph: Glyph::Add,
            action: Action::ConnectComputer,
        }),
        Target::Connecting(host) if judged => {
            output.notice = Some((
                "coder-connecting".into(),
                format!("Connecting to {}…", host),
            ))
        }
        Target::Offline(host) if judged => {
            output.notice = Some(("coder-offline".into(), format!("{} is offline.", host)))
        }
        _ => {}
    }
    // What the computer says will run it, in the shared words
    // (`Runner::text`), beside the offer and before anything runs.
    if judged
        && output.notice.is_none()
        && let Some(runner) = &meta.runner
    {
        output.notice = Some(("coder-runner".into(), runner.text()));
    }
    for (index, offer) in meta.offers.iter().enumerate() {
        let Offer::OpenScreen { screen } = offer else {
            continue;
        };
        if *screen == Screen::GymResult || (*screen == Screen::GymPublish && !has_result) {
            continue;
        }
        let connecting =
            *screen == Screen::Computers && matches!(availability, Target::NotConfigured);
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
    let fresh = |suggestion: &&crate::first_run::Suggestion| {
        !openagents_chat::basic_chats::suggestion_used(
            used,
            Some(suggestion.id),
            &[suggestion.label, suggestion.message],
        )
    };
    // The ones not used yet come first; used ones fill the rest, so a new
    // chat always shows suggestions (owner, 2026-10-01).
    let all = crate::first_run::SUGGESTIONS.iter();
    all.clone()
        .filter(fresh)
        .chain(all.filter(move |suggestion| !fresh(suggestion)))
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
        Screen::RoutesMap => ("Open the map", Glyph::Map),
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
            shortcut: None,
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
        text("compare", compare_text(compare), TextRole::Body, true);
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
        text(
            &format!("item-{index}"),
            item_text(item),
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
    // The one filled button spans the card, as on the phone; its outlined
    // buttons and chips are as wide as their words.
    if let Some(button) = &view.primary {
        rows.push(card_button(button, intent(&button.id)));
    }
    for button in view.secondary.iter().chain(&view.chips) {
        let mut node = card_button(button, intent(&button.id));
        node.style.intrinsic_width = Some(true);
        rows.push(node);
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

fn compare_text(compare: &Compare) -> String {
    format!(
        "{}: {}\n{}: {}",
        compare.without_label,
        compare.without.as_deref().unwrap_or("Not known yet"),
        compare.with_label,
        compare.with
    )
}

fn item_text(item: &Item) -> String {
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
    format!(
        "{marks} {}{}{}",
        item.text,
        item.detail
            .as_ref()
            .map_or(String::new(), |detail| format!("\n{detail}")),
        item.trailing
            .as_ref()
            .map_or(String::new(), |value| format!(" · {value}"))
    )
}

/// Mount the exact sheet value the phone draws over its chat (`SCR-05`,
/// `SCR-06`, `SCR-11`, `SCR-20`, `SCR-21`, the stop confirmation), as a
/// card under the reply: a surface without sheets shows it in place.
pub fn sheet<I>(view: &SheetView, mut intent: impl FnMut(&str) -> I) -> Node<I> {
    let prefix = format!("sheet-{}", view.id);
    let mut rows = vec![];
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
    text("title", view.title.clone(), TextRole::Heading, true);
    if let Some(headline) = &view.headline {
        text("headline", headline.clone(), TextRole::Body, true);
    }
    if let Some(big) = &view.big {
        text("big", big.clone(), TextRole::Heading, true);
    }
    if let Some(compare) = &view.compare {
        text("compare", compare_text(compare), TextRole::Body, true);
    }
    for (at, section) in view.sections.iter().enumerate() {
        if let Some(heading) = &section.heading {
            text(
                &format!("section-{at}-heading"),
                heading.clone(),
                TextRole::Status,
                true,
            );
        }
        for (index, line) in section.lines.iter().enumerate() {
            text(
                &format!("section-{at}-line-{index}"),
                line.text.clone(),
                if line.tone == Tone::Quiet {
                    TextRole::Status
                } else {
                    TextRole::Body
                },
                line.tone == Tone::Strong,
            );
        }
        for (index, item) in section.items.iter().enumerate() {
            text(
                &format!("section-{at}-item-{index}"),
                item_text(item),
                TextRole::Body,
                false,
            );
        }
    }
    if let Some(bar) = &view.bar {
        text(
            "bar",
            format!("{}: {} of {}", bar.label, bar.value, bar.max),
            TextRole::Status,
            false,
        );
    }
    if let Some(next) = &view.next {
        text("next", next.clone(), TextRole::Body, false);
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
    // As on the phone, the primary button spans the sheet; its other
    // choices and its close control are as wide as their words.
    if let Some(button) = &view.primary {
        rows.push(card_button(button, intent(&button.id)));
    }
    for button in view.secondary.iter().chain(&view.close) {
        let mut node = card_button(button, intent(&button.id));
        node.style.intrinsic_width = Some(true);
        rows.push(node);
    }
    Node {
        key: prefix,
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
            shortcut: None,
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
        // The Gym sheet on screen, from the same state the phone draws it
        // from, under the cards.
        let draft = match self.gym.sheet_talk() {
            Some(talk) if snapshot.chat.as_deref() == Some(talk) => Gym::draft_of(&snapshot.turns),
            _ => None,
        };
        if let Some(view) = self.gym.sheet_view(draft) {
            let actions = &mut self.actions;
            rows.push(sheet(&view, |id| {
                actions.insert(id.into(), Action::Gym { id: id.into() });
            }));
        }
        let meta = crate::projection::actionable(&snapshot.turns, busy, failed);
        let mut reply = reply_actions_for(
            meta,
            &snapshot.used,
            snapshot.computer && crate::projection::completed(&snapshot.turns, busy, failed),
            &snapshot
                .ready_computer
                .as_deref()
                .map_or(Target::NotConfigured, Target::Ready),
            self.gym.latest_result().is_some(),
        );
        if let Some(coder) = &snapshot.coder {
            reply
                .chips
                .retain(|chip| !matches!(chip.action, Action::RunCoder | Action::ConnectComputer));
            // A run on this computer shows its own events
            // (`crate::coder_run`); only a handoff to a host's policy says
            // where it went.
            reply.notice = (coder.host != crate::coder_run::LOCAL).then(|| {
                (
                    "coder-dispatched".into(),
                    format!(
                        "Task sent to Coder{}.",
                        coder
                            .project
                            .as_ref()
                            .map_or(String::new(), |project| format!(" in {project}")),
                    ),
                )
            });
        }
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

/// What a card button asks the surface around the chat to do.
#[derive(Debug)]
pub enum Effect {
    Requests(Vec<(u64, openagents_chat::service::Command)>),
    Navigate(Screen),
    Notice(String),
    Draft(String),
    /// A Gym run goes to Coder on the ready computer: start a Coder task
    /// with `prompt`, then report it with [`crate::gym::Gym::on_computer`]
    /// for `run`, as the phone does.
    GymCoder {
        run: String,
        prompt: String,
    },
    /// Send `text` to a Gym run's Coder task, or, with `stop`, stop it.
    GymCommand {
        host: String,
        task: String,
        text: String,
        stop: bool,
    },
    /// Open a Gym run's Coder chat.
    OpenCoder {
        host: String,
        task: String,
    },
    None,
}
impl crate::session::Session {
    pub fn card_action(&mut self, key: &str) -> Effect {
        let Some(action) = self.cards.action(key) else {
            return Effect::None;
        };
        let snapshot = self.state().cloned().unwrap_or_default();
        // The Gym's buttons and sheets do what they do on the phone
        // (`Gym::tap`), even while a reply streams.
        match &action {
            Action::Gym { id } => return self.gym_tap(id, &snapshot),
            Action::OpenScreen { screen }
                if matches!(
                    screen,
                    Screen::GymResult | Screen::GymPublish | Screen::GymTestSet
                ) =>
            {
                let draft = Gym::draft_of(&snapshot.turns).is_some();
                let open = self.selected.clone();
                self.cards.gym.open_screen(*screen, open.as_deref(), draft);
                return Effect::None;
            }
            _ => {}
        }
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
            Action::RunCoder => {
                return self
                    .run_coder()
                    .map_or(Effect::None, |request| Effect::Requests(vec![request]));
            }
            Action::ConnectComputer => {
                return Effect::Navigate(Screen::Computers);
            }
            Action::OpenScreen { screen } => return Effect::Navigate(screen),
            Action::RunCli { .. } => {
                return Effect::Notice("Connect a computer to run this command.".into());
            }
            Action::Gym { .. } => return Effect::None,
        };
        // Only the words go to the router; the draft's images stay, bound
        // to this message (`Session::submit`).
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

impl crate::session::Session {
    /// A Gym button: the phone's own [`Gym::tap`], its effect carried out
    /// with this session's requests, or handed to the surface.
    fn gym_tap(&mut self, id: &str, snapshot: &openagents_chat::service::Snapshot) -> Effect {
        use crate::gym::Effect as Gym;
        let open = self.selected.clone();
        let turns = &snapshot.turns;
        let Some(effect) = self.cards.gym.tap(
            id,
            |talk| {
                (open.as_deref() == Some(talk))
                    .then(|| crate::gym::Gym::draft_of(turns))
                    .flatten()
            },
            open.as_deref(),
            snapshot.ready_computer.as_deref(),
        ) else {
            return Effect::None;
        };
        let say = |session: &mut Self, text: String| {
            session
                .submit(uuid::Uuid::new_v4().simple().to_string(), text)
                .into_iter()
                .collect::<Vec<_>>()
        };
        match effect {
            Gym::None | Gym::Menu => Effect::None,
            Gym::Say { talk, text } => {
                self.select(&talk);
                Effect::Requests(say(self, text))
            }
            Gym::Fresh { text } => {
                let mut requests = vec![self.new_chat()];
                requests.extend(say(self, text));
                Effect::Requests(requests)
            }
            Gym::Compose { text } => Effect::Draft(text),
            Gym::Computer { run, prompt } => Effect::GymCoder { run, prompt },
            Gym::Command {
                host,
                task,
                text,
                stop,
            } => Effect::GymCommand {
                host,
                task,
                text,
                stop,
            },
            Gym::OpenCoder { host, task } => Effect::OpenCoder { host, task },
            Gym::ConnectComputer => Effect::Navigate(Screen::Computers),
            Gym::OpenChat { talk } => {
                if let Some(talk) = talk {
                    self.select(&talk);
                }
                Effect::None
            }
            Gym::VerseGym => Effect::Navigate(Screen::VerseGym),
        }
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

    /// The desktop and the phone both draw reply actions from here, so
    /// both show the computer's prediction in the same words.
    #[test]
    fn an_offer_to_run_coder_says_who_will_run_it() {
        use openagents_chat::coder_events::{Passed, PassedOver, Runner};
        let states = [
            (
                Runner::Runs {
                    provider: "codex".into(),
                    model: "gpt-6-luna".into(),
                    passed: vec![],
                    requested: None,
                },
                "Codex will do this.",
            ),
            (
                Runner::Runs {
                    provider: "claude".into(),
                    model: "claude-opus-5-5".into(),
                    passed: vec![Passed {
                        provider: "codex".into(),
                        why: PassedOver::NearLimit { used_percent: 92 },
                    }],
                    requested: None,
                },
                // A provider's window is never named (#10120).
                "Claude Code will do this.",
            ),
            (
                Runner::NotSignedIn { providers: vec![] },
                "No coding agent is signed in on this computer. Sign in to one, then ask again: \
                 Codex (run `codex login`); Claude Code (run `claude` and log in).",
            ),
        ];
        for (runner, words) in states {
            let meta = Meta {
                offers: vec![Offer::RunCoder],
                runner: Some(runner),
                ..Meta::default()
            };
            let actions =
                reply_actions_for(Some(&meta), &[], false, &Target::Ready("Studio Mac"), false);
            assert_eq!(actions.notice, Some(("coder-runner".into(), words.into())));
            assert!(
                actions
                    .chips
                    .iter()
                    .any(|chip| chip.action == Action::RunCoder)
            );
            // A reply that does not offer Coder says nothing about it.
            let quiet = Meta {
                offers: vec![],
                ..meta.clone()
            };
            assert!(
                reply_actions_for(
                    Some(&quiet),
                    &[],
                    false,
                    &Target::Ready("Studio Mac"),
                    false
                )
                .notice
                .is_none()
            );
        }
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
