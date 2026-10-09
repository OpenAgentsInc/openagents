//! The phone's shell (#11126): a top bar with the menu, a **Chat** /
//! **Code** switch, and a new-chat button; a new chat with feature cards
//! (Chat) or Coder suggestions from the person's computers (Code); and a
//! drawer with the main places and recent chats. A host that draws the
//! shell turns it on ([`CoderTab::set_shell`]); the tab then leaves out its
//! own header rows, and the host draws the top bar, the drawer, and the
//! cards from [`CoderTab::shell_view`], sending [`ShellAction`]s back.
//!
//! Chat mode's new chat sends to OpenAgents as before. Code mode's new chat
//! starts Coder on the person's ready computer, in the project they picked
//! (or the one used last), the same task start as **Run Coder**.

use super::*;
use openagents_chat::home_cards::{self, HOME_CARDS};

/// Recent chats the drawer lists before **See all**.
const DRAWER_ROWS: usize = 8;
/// The most matches a drawer search lists.
const SEARCH_ROWS: usize = 50;
/// The most suggestions above Code mode's composer.
const CODE_ROWS: usize = 5;
/// The longest drawer search, in bytes.
const MAX_QUERY: usize = 200;

/// The shell's state in the tab.
#[derive(Default)]
pub(super) struct Shell {
    /// The host draws the shell.
    pub(super) on: bool,
    /// Code mode: a new chat starts Coder on a computer.
    pub(super) code: bool,
    /// The host's drawer is open: the packet carries its rows.
    drawer: bool,
    query: String,
    /// What each drawer row opens, by its index in the last view.
    intents: Vec<Intent>,
}

/// What the host's shell asks of the tab.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ShellAction {
    /// The **Chat** / **Code** switch.
    Mode { code: bool },
    /// The new-chat button or the drawer's **Chat** pill.
    NewChat,
    /// The drawer opened or closed.
    Drawer { open: bool },
    /// The drawer's search field changed.
    Search { query: String },
    /// A recent chat in the drawer, by its index in the last view.
    Open { index: usize },
    /// **See all**: every chat.
    SeeAll,
    /// A feature card's **Try it**.
    TryCard { id: String },
}

/// The shell as the host draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShellView {
    /// `chat` or `code`.
    pub mode: &'static str,
    /// `new` (a new chat: the switch shows), `chat` (a conversation), or
    /// `list` (every chat, which draws its own header).
    pub screen: &'static str,
    /// The feature cards a new chat in Chat mode shows.
    pub cards: Vec<Card>,
    /// The drawer's rows, while it is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawer: Option<Drawer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Card {
    pub id: &'static str,
    pub title: &'static str,
    pub line: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Drawer {
    pub query: String,
    pub recent: Vec<RecentChat>,
    /// More chats than the drawer lists: it shows **See all**.
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecentChat {
    pub index: usize,
    pub title: String,
    pub detail: String,
}

impl CoderTab {
    /// The host draws the shell ([`ShellView`]).
    pub fn set_shell(&mut self, on: bool) {
        self.shell.on = on;
    }

    #[must_use]
    pub fn with_shell(mut self, on: bool) -> Self {
        self.set_shell(on);
        self
    }

    /// The link cards' shared state: the app reads the pages they ask for.
    #[must_use]
    pub fn link_previews(&self) -> crate::links::LinkPreviews {
        self.links.clone()
    }

    /// The composer's words for a message to OpenAgents.
    pub(super) fn ask_words(&self) -> &'static str {
        if self.shell.on {
            "Ask OpenAgents"
        } else {
            "Message OpenAgents"
        }
    }

    /// The shell's part of the packet; `None` unless the host draws it.
    pub fn shell_view(
        &mut self,
        computers: Option<&Computers>,
        chats: &Chats,
    ) -> Option<ShellView> {
        if !self.shell.on {
            return None;
        }
        let screen = if self.drawer {
            "list"
        } else if self.open.is_some() || self.talk.is_some() || self.threads.opened().is_some() {
            "chat"
        } else {
            "new"
        };
        let drawer = self
            .shell
            .drawer
            .then(|| self.drawer_rows(computers, chats));
        Some(ShellView {
            mode: if self.shell.code { "code" } else { "chat" },
            screen,
            cards: HOME_CARDS
                .iter()
                .map(|card| Card {
                    id: card.id,
                    title: card.title,
                    line: card.line,
                })
                .collect(),
            drawer,
        })
    }

    /// Carry out a shell action.
    pub fn shell(
        &mut self,
        action: ShellAction,
        computers: Option<&mut Computers>,
        chats: &mut Chats,
    ) {
        match action {
            ShellAction::Mode { code } => {
                self.shell.code = code;
                self.notice = None;
            }
            ShellAction::NewChat => {
                self.shell.drawer = false;
                self.run_intent(Intent::NewChat, computers, chats);
            }
            ShellAction::Drawer { open } => {
                self.shell.drawer = open;
                if !open {
                    self.shell.query.clear();
                }
            }
            ShellAction::Search { mut query } => {
                while query.len() > MAX_QUERY {
                    query.pop();
                }
                self.shell.query = query;
            }
            ShellAction::Open { index } => {
                let Some(intent) = self.shell.intents.get(index).cloned() else {
                    return;
                };
                self.shell.drawer = false;
                self.shell.query.clear();
                self.run_intent(intent, computers, chats);
            }
            ShellAction::SeeAll => {
                self.shell.drawer = false;
                self.shell.query.clear();
                self.run_intent(Intent::Menu, computers, chats);
            }
            ShellAction::TryCard { id } => {
                let Some(card) = home_cards::find(&id) else {
                    return;
                };
                self.shell.code = false;
                self.start_talk(card.message, computers.as_deref());
            }
        }
    }

    /// The drawer's recent chats, newest first, as the full list orders
    /// them, less archived ones; a search lists the matching titles.
    fn drawer_rows(&mut self, computers: Option<&Computers>, chats: &Chats) -> Drawer {
        let query = self.shell.query.trim().to_lowercase();
        let mut recent = vec![];
        let mut intents = vec![];
        let mut total = 0;
        let limit = if query.is_empty() {
            DRAWER_ROWS
        } else {
            SEARCH_ROWS
        };
        for row in self.recent(computers, chats) {
            if row.group == crate::chat_list::Group::Archived {
                continue;
            }
            let Some((label, intent)) = first_button(&row.row) else {
                continue;
            };
            let mut lines = label.lines();
            let title = lines.next().unwrap_or_default().to_owned();
            let detail = lines.next().unwrap_or_default().to_owned();
            if !query.is_empty() && !title.to_lowercase().contains(&query) {
                continue;
            }
            total += 1;
            if recent.len() < limit {
                recent.push(RecentChat {
                    index: intents.len(),
                    title,
                    detail,
                });
                intents.push(intent);
            }
        }
        self.shell.intents = intents;
        Drawer {
            query: self.shell.query.clone(),
            more: query.is_empty() && total > recent.len(),
            recent,
        }
    }

    /// A new chat under the shell: the feature cards (Chat), or what Coder
    /// could work on (Code), over a composer ready to type.
    pub(super) fn shell_landing(&self, computers: Option<&Computers>) -> Node<Intent> {
        let mut children = vec![];
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let placeholder = if self.shell.code {
            children.push(node(
                "coder-new-transcript",
                Element::Transcript {
                    label: "New chat".into(),
                    children: vec![],
                    earlier: None,
                    source: None,
                },
            ));
            let (rows, target) = self.code_rows(computers);
            children.push(Node {
                key: "shell-code-rows".into(),
                style: Style {
                    gap: Some(Space::Md),
                    padding_start: Some(Space::Md),
                    padding_end: Some(Space::Sm),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children: rows,
                },
            });
            if let Some(target) = target {
                let mut line = status("shell-code-target", &target);
                line.style.padding_start = Some(Space::Md);
                children.push(line);
            }
            "Work with Coder"
        } else {
            // The host draws the cards and fills the screen with them.
            let mut cards = node(
                "shell-cards",
                Element::Surface {
                    resource: "home-cards".into(),
                    label: "What's new".into(),
                },
            );
            cards.style.fill_height = Some(true);
            children.push(cards);
            self.ask_words()
        };
        children.extend(self.attachments());
        children.push(self.composer_with(placeholder.to_owned(), true, false, &[], None, true));
        page(children)
    }

    /// Code mode's suggestions, and the line naming where Coder runs: the
    /// ready computer's recent Coder chats and its projects, or **Connect a
    /// computer** without one.
    fn code_rows(&self, computers: Option<&Computers>) -> (Vec<Node<Intent>>, Option<String>) {
        let availability = self.availability(computers);
        let (computers, host) = match (computers, &availability) {
            (Some(computers), Availability::Ready(host)) => (computers, *host),
            (_, Availability::NotConfigured) | (None, _) => {
                return (
                    vec![icon_button(
                        "shell-code-connect",
                        "Connect a computer",
                        Glyph::Add,
                        false,
                        Intent::ConnectComputer,
                    )],
                    Some("Coder works in your projects on your own computer.".into()),
                );
            }
            (Some(_), _) => {
                return (Self::unavailable(&availability).into_iter().collect(), None);
            }
        };
        let mut rows = vec![];
        let saved = |_: &str, _: &str| None;
        for (_, _, title, row) in task_rows(
            computers.snapshot(),
            &self.activity(computers),
            &self.list.list,
            &saved,
        )
        .into_iter()
        .take(3)
        {
            if let Element::Button { intent, .. } = row.element {
                rows.push(icon_button(
                    &format!("shell-{}", row.key),
                    &title,
                    Glyph::History,
                    false,
                    intent,
                ));
            }
        }
        let chosen = self.workspace(host);
        for workspace in host.workspaces.iter().flatten() {
            if rows.len() >= CODE_ROWS {
                break;
            }
            let here = chosen.as_deref() == Some(workspace.as_str());
            rows.push(icon_button(
                &format!("shell-project-{workspace}"),
                &format!("Work in {workspace}"),
                if here { Glyph::Check } else { Glyph::Folder },
                false,
                Intent::CodeProject {
                    host: host.key.clone(),
                    workspace: workspace.clone(),
                },
            ));
        }
        let engine = engines_of(host)
            .into_iter()
            .find(|engine| engine.state == crate::router::EngineState::Ready)
            .and_then(|engine| engine_name(&engine.engine));
        let place = match &chosen {
            Some(workspace) => format!("{workspace} on {}", host.label),
            None => host.label.clone(),
        };
        let target = match engine {
            Some(engine) => format!("{engine} · {place}"),
            None => place,
        };
        (rows, Some(target))
    }

    /// Code mode: new Coder work goes to `workspace` on `host`.
    pub(super) fn choose_project(&mut self, host: String, workspace: &str) {
        let now = unix_now();
        self.list.list.used.insert(used_key(&host, workspace), now);
        self.list.save();
        self.preferred = Some(host);
        self.notice = None;
    }

    /// Code mode's send: a Coder task on the ready computer with `prompt`,
    /// opened as its chat.
    pub(super) fn start_code(
        &mut self,
        prompt: &str,
        computers: Option<&mut Computers>,
        chats: &mut Chats,
    ) {
        let Some(computers) = computers else {
            self.go = Some(Go::Connect);
            return;
        };
        let host = match self.availability(Some(computers)) {
            Availability::Ready(host) => host.key.clone(),
            Availability::Connecting(host) => {
                self.notice = Some(format!("Connecting to {}…", host.label));
                return;
            }
            Availability::Offline(host) => {
                self.notice = Some(format!("{} is offline.", host.label));
                return;
            }
            Availability::NotConfigured => {
                self.go = Some(Go::Connect);
                return;
            }
        };
        if computers
            .snapshot()
            .host(&host)
            .and_then(|record| self.workspace(record))
            .is_none()
            && let Err(refusal) = computers.refresh_workspaces(&host)
        {
            self.notice = Some(refusal.reason());
            return;
        }
        let Some(record) = computers.snapshot().host(&host) else {
            return;
        };
        let label = record.label.clone();
        let Some(workspace) = self.workspace(record) else {
            self.notice = Some(format!("{label} lists no project for Coder yet."));
            return;
        };
        match computers.start_task_requesting(&host, &workspace, prompt, &[], None) {
            Ok(task) => {
                let now = computers.snapshot().now;
                self.list
                    .list
                    .titles
                    .insert(task.clone(), first_line(prompt));
                self.list.list.sent.insert(task.clone(), now);
                self.list.list.used.insert(used_key(&host, &workspace), now);
                self.list.save();
                self.notice = None;
                self.composers += 1;
                self.open(host, task.clone(), chats);
                self.echo(&task, prompt, None, false, now);
            }
            Err(refusal) => self.notice = Some(refusal.reason()),
        }
    }
}

/// A chat's title from its first message: its first line, shortened.
fn first_line(prompt: &str) -> String {
    let line = prompt.lines().next().unwrap_or_default().trim();
    let mut title: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        title.push('…');
    }
    title
}

/// An engine's name from its wire word.
fn engine_name(word: &str) -> Option<&'static str> {
    nostr::cj_conversation::Engine::ALL
        .into_iter()
        .find(|engine| {
            engine.word() == word || (word == "claude" && engine.word() == "claude_code")
        })
        .map(nostr::cj_conversation::Engine::name)
}

/// The first button in a list row (a chat card's row), its label and intent.
fn first_button(node: &Node<Intent>) -> Option<(String, Intent)> {
    match &node.element {
        Element::Button { label, intent, .. } => Some((label.clone(), intent.clone())),
        Element::Stack { children, .. } => children.first().and_then(first_button),
        _ => None,
    }
}

/// Leave out the tab's own header rows: the host's top bar replaces them.
pub(super) fn strip_header(root: &mut Node<Intent>) {
    if let Element::Stack { children, .. } = &mut root.element {
        children.retain(|child| child.key != "coder-header" && child.key != "coder-chat-header");
    }
}

/// Before each reply that took time, a **Worked for** row that opens what
/// was done: how long from the message to the reply, then the steps the
/// reply's own record shows.
pub(super) fn worked(
    rows: &mut Vec<Node<Intent>>,
    turns: &[openagents_chat::basic_coder::Turn],
    start: usize,
) {
    use openagents_chat::basic_coder::Role;
    let mut at = rows.len();
    while at > 0 {
        at -= 1;
        let Some(index) = rows[at]
            .key
            .strip_prefix("talk-m")
            .and_then(|rest| rest.parse::<usize>().ok())
        else {
            continue;
        };
        let Some(turn) = turns.get(index).filter(|turn| turn.role == Role::Assistant) else {
            continue;
        };
        let asked = turns[start.min(index)..index]
            .iter()
            .rev()
            .find(|turn| turn.role == Role::User)
            .and_then(|turn| turn.at);
        let (Some(asked), Some(answered)) = (asked, turn.at) else {
            continue;
        };
        let seconds = answered.saturating_sub(asked);
        if seconds == 0 {
            continue;
        }
        let mut steps = vec!["Read your message".to_owned()];
        let meta = turn.meta.as_ref();
        if meta.is_some_and(|meta| meta.answer.is_some()) {
            steps.push("Found a ready answer".into());
        } else if let Some(model) = &turn.model {
            steps.push(format!("Wrote the answer with {model}"));
        } else {
            steps.push("Wrote the answer".into());
        }
        if meta.is_some_and(|meta| !meta.offers.is_empty()) {
            steps.push("Suggested a next step".into());
        }
        let children = steps
            .iter()
            .enumerate()
            .map(|(step, words)| status(&format!("talk-w{index}-{step}"), words))
            .collect();
        rows.insert(
            at,
            node(
                &format!("talk-w{index}"),
                Element::Tool {
                    name: format!("Worked for {}", duration(seconds)),
                    detail: String::new(),
                    state: rust_native::ToolState::Done,
                    children,
                },
            ),
        );
    }
}

/// After each reply that names web links, a card for each (#11126): the
/// page's title, its site, and its picture once read ([`crate::links`]).
/// The host draws the card in the box the transcript reserves for it.
pub(super) fn link_cards(
    rows: &mut Vec<Node<Intent>>,
    turns: &[openagents_chat::basic_coder::Turn],
    previews: &crate::links::LinkPreviews,
) {
    use crate::links;
    use openagents_chat::basic_coder::Role;
    let mut at = rows.len();
    while at > 0 {
        at -= 1;
        let Some(index) = rows[at]
            .key
            .strip_prefix("talk-m")
            .and_then(|rest| rest.parse::<usize>().ok())
        else {
            continue;
        };
        let Some(turn) = turns.get(index).filter(|turn| turn.role == Role::Assistant) else {
            continue;
        };
        for (n, url) in links::answer_links(&turn.text).iter().enumerate().rev() {
            let card = previews.card(url);
            let label = if card.title == card.site {
                card.site.clone()
            } else {
                format!("{}, {}", card.title, card.site)
            };
            let mut row = node(
                &format!("talk-l{index}-{n}"),
                Element::Surface {
                    resource: links::resource(url),
                    label,
                },
            );
            row.style.min_height = Some(if card.image {
                links::IMAGE_CARD_HEIGHT
            } else {
                links::PLAIN_CARD_HEIGHT
            });
            rows.insert(at + 1, row);
        }
    }
}

/// `7s`, `1m 5s`, `2m`.
fn duration(seconds: u64) -> String {
    match (seconds / 60, seconds % 60) {
        (0, s) => format!("{s}s"),
        (m, 0) => format!("{m}m"),
        (m, s) => format!("{m}m {s}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_chat::basic_coder::Turn;

    #[test]
    fn worked_rows_go_before_replies_that_took_time() {
        let mut asked = Turn::user("hi");
        asked.at = Some(100);
        let mut answered = Turn::user("hello");
        answered.role = openagents_chat::basic_coder::Role::Assistant;
        answered.at = Some(107);
        let mut instant = answered.clone();
        instant.at = Some(107);
        let mut again = asked.clone();
        again.at = Some(107);
        let turns = vec![asked, answered, again, instant];
        let mut rows: Vec<Node<Intent>> = (0..4)
            .map(|index| status(&format!("talk-m{index}"), "x"))
            .collect();
        worked(&mut rows, &turns, 0);
        let keys: Vec<_> = rows.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(
            keys,
            ["talk-m0", "talk-w1", "talk-m1", "talk-m2", "talk-m3"]
        );
        let Element::Tool { name, children, .. } = &rows[1].element else {
            panic!("a tool row");
        };
        assert_eq!(name, "Worked for 7s");
        assert_eq!(children.len(), 2);
        assert_eq!(duration(65), "1m 5s");
        assert_eq!(duration(120), "2m");
    }

    #[test]
    fn replies_with_links_get_cards_after_them() {
        use openagents_chat::basic_coder::Role;
        let asked = Turn::user("where is the roadmap? https://ignored.example.com");
        let mut answered = Turn::user(
            "It's at [the roadmap](https://openagents.com/roadmap) and https://github.com/OpenAgentsInc.",
        );
        answered.role = Role::Assistant;
        let turns = vec![asked, answered];
        let mut rows: Vec<Node<Intent>> = (0..2)
            .map(|index| status(&format!("talk-m{index}"), "x"))
            .collect();
        let previews = crate::links::LinkPreviews::default();
        link_cards(&mut rows, &turns, &previews);
        let keys: Vec<_> = rows.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(keys, ["talk-m0", "talk-m1", "talk-l1-0", "talk-l1-1"]);
        let Element::Surface { resource, label } = &rows[2].element else {
            panic!("a surface");
        };
        assert_eq!(
            resource,
            &crate::links::resource("https://openagents.com/roadmap")
        );
        assert_eq!(label, "openagents.com");
        assert_eq!(
            rows[2].style.min_height,
            Some(crate::links::PLAIN_CARD_HEIGHT)
        );
        assert_eq!(previews.take_wanted().len(), 2);
        // A page with a picture makes a taller card.
        previews.finish(
            "https://openagents.com/roadmap",
            Some(crate::links::Preview {
                title: Some("Roadmap".into()),
                site: Some("OpenAgents".into()),
                image: Some(std::sync::Arc::new(vec![0])),
            }),
        );
        let mut again: Vec<Node<Intent>> = (0..2)
            .map(|index| status(&format!("talk-m{index}"), "x"))
            .collect();
        link_cards(&mut again, &turns, &previews);
        assert_eq!(
            again[2].style.min_height,
            Some(crate::links::IMAGE_CARD_HEIGHT)
        );
        let Element::Surface { label, .. } = &again[2].element else {
            panic!("a surface");
        };
        assert_eq!(label, "Roadmap, OpenAgents");
    }

    #[test]
    fn the_shell_draws_cards_and_asks_openagents() {
        let mut tab = CoderTab::new("coder".into());
        tab.set_shell(true);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([0x31; 32]).unwrap();
        let mut chats = Chats::new(runtime.handle().clone(), secret, Err("test".into()));
        let view = tab.render(None, &mut chats).expect("a view");
        let text = view.to_string();
        assert!(text.contains("home-cards"), "{text}");
        assert!(text.contains("Ask OpenAgents"), "{text}");
        assert!(!text.contains("coder-header"), "{text}");
        let shell = tab.shell_view(None, &chats).expect("the shell");
        assert_eq!((shell.mode, shell.screen), ("chat", "new"));
        assert_eq!(shell.cards.len(), 4);
        assert!(shell.drawer.is_none());

        // Code mode without a computer offers to connect one.
        tab.shell(ShellAction::Mode { code: true }, None, &mut chats);
        let text = tab.render(None, &mut chats).expect("a view").to_string();
        assert!(text.contains("Work with Coder"), "{text}");
        assert!(text.contains("Connect a computer"), "{text}");

        // The drawer lists rows only while it is open.
        tab.shell(ShellAction::Drawer { open: true }, None, &mut chats);
        let drawer = tab.shell_view(None, &chats).and_then(|v| v.drawer);
        assert_eq!(drawer.map(|d| d.recent.len()), Some(0));

        // See all shows every chat, with its own header.
        tab.shell(ShellAction::SeeAll, None, &mut chats);
        let text = tab.render(None, &mut chats).expect("a view").to_string();
        assert!(text.contains("coder-header"), "{text}");
        assert_eq!(tab.shell_view(None, &chats).map(|v| v.screen), Some("list"));
    }

    #[test]
    fn shell_words_pass_the_copy_guard() {
        let mut words: Vec<&str> = vec![
            "Work with Coder",
            "Ask OpenAgents",
            "Coder works in your projects on your own computer.",
        ];
        for card in HOME_CARDS {
            words.extend([card.title, card.line]);
        }
        for text in words {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
