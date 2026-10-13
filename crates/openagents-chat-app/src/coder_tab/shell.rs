//! The phone's shell (#11126): a top bar with the menu, a **Coder** /
//! **Verse** switch, and a new-chat button; a drawer with the main places
//! and recent chats. A host that draws the shell turns it on
//! ([`CoderTab::set_shell`]); the tab then leaves out its own header rows,
//! and the host draws the top bar and the drawer from
//! [`CoderTab::shell_view`], sending [`ShellAction`]s back.
//!
//! The app opens where the person left off (#11557): the chat on view
//! when they last used it, else every chat, else, for a person with no
//! chats yet, a welcome with one action, **Start a chat**. A new chat is
//! a clean composer, with no cards over it.
//!
//! **Coder** is the one conversation: one composer that asks OpenAgents,
//! whose network decides when a message needs Coder on a computer and
//! offers it under the reply. **Verse** is the Grid world, which the host
//! draws over the chat. A reply's **Enter the Grid** card switches to the
//! Verse.

use super::*;
use crate::coder_list::LastOpen;

/// Recent chats the drawer lists before **See all**.
const DRAWER_ROWS: usize = 8;
/// The most matches a drawer search lists.
const SEARCH_ROWS: usize = 50;
/// The longest drawer search, in bytes.
const MAX_QUERY: usize = 200;
/// The welcome's words, for a person with no chats yet.
pub const WELCOME_TITLE: &str = "Welcome to OpenAgents";
pub const WELCOME_LINE: &str =
    "Ask anything. When it takes code, Coder writes it on your computer.";
pub const WELCOME_START: &str = "Start a chat";
/// The surface resource of a reply's **Enter the Grid** card.
pub const VERSE_PORTAL: &str = "verse-portal";
/// The height the transcript keeps for that card, in points.
pub const VERSE_PORTAL_HEIGHT: u16 = 120;

/// The shell's state in the tab.
#[derive(Default)]
pub(super) struct Shell {
    /// The host draws the shell.
    pub(super) on: bool,
    /// The switch is on **Verse**: the host shows the Grid world.
    pub(super) verse: bool,
    /// The host's drawer is open: the packet carries its rows.
    drawer: bool,
    query: String,
    /// What each drawer row opens, by its index in the last view.
    intents: Vec<Intent>,
    /// The launch's first view chose where the app opens ([`CoderTab::reopen`]).
    launched: bool,
    /// The app opened with no chats at all: the new chat shows the welcome
    /// until the person starts a chat.
    pub(super) welcome: bool,
}

/// What the host's shell asks of the tab.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ShellAction {
    /// The **Coder** / **Verse** switch, or a portal into the Verse.
    Switch { verse: bool },
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
}

/// The shell as the host draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShellView {
    /// `coder` (the conversation) or `verse` (the Grid world).
    pub place: &'static str,
    /// `new` (a new chat, or the welcome), `chat` (a conversation), or
    /// `list` (every chat, which draws its own header).
    pub screen: &'static str,
    /// The drawer's rows, while it is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawer: Option<Drawer>,
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
            place: if self.shell.verse { "verse" } else { "coder" },
            screen,
            drawer,
        })
    }

    /// Where the app opens, once, at the launch's first view: the chat on
    /// view when the person last used the app, while it still exists; else
    /// every chat, when there are any; else the welcome. A debug script,
    /// the Gym's intro chat, or a host launch argument that already chose
    /// a screen keeps it.
    pub(super) fn reopen(&mut self, computers: Option<&Computers>, chats: &mut Chats) {
        if !self.shell.on || self.shell.launched {
            return;
        }
        self.shell.launched = true;
        if self.open.is_some()
            || self.talk.is_some()
            || self.threads.opened().is_some()
            || self.drawer
            || self.shell.verse
            || !self.script.is_empty()
        {
            return;
        }
        match self.list.list.last_open.clone() {
            Some(LastOpen::Talk { id }) if self.basic.get(&id).is_some() => {
                self.talk_turns = TALK_TURNS;
                self.talk = Some(id);
                return;
            }
            Some(LastOpen::Task { host, task })
                if self
                    .list
                    .list
                    .rows
                    .iter()
                    .any(|row| row.host == host && row.task == task) =>
            {
                self.open(host, task, chats);
                return;
            }
            _ => {}
        }
        let any = !self.list.list.rows.is_empty()
            || self
                .recent(computers, chats)
                .iter()
                .any(|row| row.group != crate::chat_list::Group::Archived);
        if any {
            self.drawer = true;
        } else {
            self.shell.welcome = true;
        }
    }

    /// Keep the chat on view for the next launch ([`CoderTab::reopen`]):
    /// none while every chat or a new chat shows.
    pub(super) fn remember_open(&mut self) {
        if !self.shell.on || !self.shell.launched {
            return;
        }
        let on_view = if self.drawer {
            None
        } else if let Some(id) = &self.talk {
            Some(LastOpen::Talk { id: id.clone() })
        } else {
            self.open.as_ref().map(|open| LastOpen::Task {
                host: open.host.clone(),
                task: open.task.clone(),
            })
        };
        if self.list.list.last_open != on_view {
            self.list.list.last_open = on_view;
            self.list.save();
        }
    }

    /// Carry out a shell action.
    pub fn shell(
        &mut self,
        action: ShellAction,
        computers: Option<&mut Computers>,
        chats: &mut Chats,
    ) {
        match action {
            ShellAction::Switch { verse } => {
                self.shell.verse = verse;
                self.shell.drawer = false;
                self.notice = None;
            }
            ShellAction::NewChat => {
                self.shell.drawer = false;
                self.shell.verse = false;
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
                self.shell.verse = false;
                self.shell.query.clear();
                self.run_intent(intent, computers, chats);
            }
            ShellAction::SeeAll => {
                self.shell.drawer = false;
                self.shell.verse = false;
                self.shell.query.clear();
                self.run_intent(Intent::Menu, computers, chats);
            }
        }
    }

    /// Whether the switch is on **Verse**.
    #[must_use]
    pub fn in_verse(&self) -> bool {
        self.shell.on && self.shell.verse
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

    /// A new chat under the shell: a clean composer under an empty
    /// conversation, its field's **Ask OpenAgents** the only prompt; or,
    /// for a person with no chats yet, the welcome.
    pub(super) fn shell_landing(&self) -> Node<Intent> {
        if self.shell.welcome {
            return self.shell_welcome();
        }
        let mut children = vec![];
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        // An empty conversation fills the screen above the field.
        children.push(node(
            "coder-new-transcript",
            Element::Transcript {
                label: "New chat".into(),
                children: vec![],
                earlier: None,
                source: None,
            },
        ));
        children.extend(self.attachments());
        children.push(self.composer_with(
            self.ask_words().to_owned(),
            true,
            false,
            &[],
            None,
            // A new chat opened on purpose is ready to type.
            true,
        ));
        page(children)
    }

    /// The first screen for a person with no chats: what OpenAgents is, in
    /// one line, and one action, **Start a chat**, which opens a new chat.
    fn shell_welcome(&self) -> Node<Intent> {
        let mut children = vec![];
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let title = heading("shell-welcome-title", WELCOME_TITLE);
        let mut line = body("shell-welcome-line", WELCOME_LINE);
        line.style.foreground = Some(gray());
        let start = pill(
            "shell-welcome-start",
            WELCOME_START,
            Glyph::Compose,
            Intent::NewChat,
        );
        let mut welcome = node(
            "shell-welcome",
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![title, line, start],
            },
        );
        welcome.style.gap = Some(Space::Md);
        welcome.style.padding_top = Some(Space::Lg);
        children.push(welcome);
        page(children)
    }
}

/// A reply's **Enter the Grid** card, when the router offered the Verse
/// under it: a surface the host draws, whose tap switches to the Verse.
pub(super) fn verse_portal() -> Node<Intent> {
    let mut card = node(
        "talk-verse-portal",
        Element::Surface {
            resource: VERSE_PORTAL.into(),
            label: "Enter the Grid".into(),
        },
    );
    card.style.min_height = Some(VERSE_PORTAL_HEIGHT);
    card
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
        } else {
            // No model's name: the phone names no vendor model.
            steps.push("Wrote the answer".into());
        }
        // The first model provider missed the turn and another model
        // answered it (#11132).
        if meta.is_some_and(|meta| meta.switched.is_some()) {
            steps.push("Another model answered it".into());
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

    fn chats() -> (tokio::runtime::Runtime, crate::chats::Chats) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([0x32; 32]).unwrap();
        let chats = crate::chats::Chats::new(runtime.handle().clone(), secret, Err("test".into()));
        (runtime, chats)
    }

    #[test]
    fn the_switch_is_coder_or_verse() {
        let (_runtime, mut chats) = chats();
        let mut tab = CoderTab::new("coder:shell".into()).with_shell(true);
        let view = tab.shell_view(None, &chats).unwrap();
        assert_eq!((view.place, view.screen), ("coder", "new"));
        let json = serde_json::to_value(&view).unwrap();
        assert!(json.get("mode").is_none(), "no Chat / Code mode");
        assert!(json.get("cards").is_none(), "no feature cards");
        assert!(json.get("carousel").is_none(), "no carousel");
        tab.shell(ShellAction::Switch { verse: true }, None, &mut chats);
        assert!(tab.in_verse());
        assert!(tab.talk.is_none());
        assert_eq!(tab.shell_view(None, &chats).unwrap().place, "verse");
        // Back to Coder with the switch, or a new chat.
        tab.shell(ShellAction::Switch { verse: false }, None, &mut chats);
        assert!(!tab.in_verse());
        tab.shell(ShellAction::Switch { verse: true }, None, &mut chats);
        tab.shell(ShellAction::NewChat, None, &mut chats);
        assert!(!tab.in_verse());
        let action: ShellAction =
            serde_json::from_str(r#"{"action":"switch","verse":true}"#).unwrap();
        assert_eq!(action, ShellAction::Switch { verse: true });
        assert!(serde_json::from_str::<ShellAction>(r#"{"action":"mode","code":true}"#).is_err());
        assert!(
            serde_json::from_str::<ShellAction>(r#"{"action":"try_card","id":"verse"}"#).is_err()
        );
    }

    #[test]
    fn a_reply_offering_the_verse_shows_a_portal_card() {
        let card = verse_portal();
        let Element::Surface { resource, label } = &card.element else {
            panic!("a surface");
        };
        assert_eq!(resource, VERSE_PORTAL);
        assert_eq!(label, "Enter the Grid");
        assert_eq!(card.style.min_height, Some(VERSE_PORTAL_HEIGHT));
    }

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
    fn the_shell_welcomes_a_new_person_then_asks_openagents() {
        let mut tab = CoderTab::new("coder".into());
        tab.set_shell(true);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([0x31; 32]).unwrap();
        let mut chats = Chats::new(runtime.handle().clone(), secret, Err("test".into()));
        // No chats yet: the welcome, with one action.
        let text = tab.render(None, &mut chats).expect("a view").to_string();
        assert!(text.contains(WELCOME_START), "{text}");
        assert!(!text.contains("coder-composer"), "{text}");
        assert!(!text.contains("home-cards"), "{text}");
        tab.shell(ShellAction::NewChat, None, &mut chats);
        let view = tab.render(None, &mut chats).expect("a view");
        let text = view.to_string();
        assert!(!text.contains("home-cards"), "{text}");
        assert!(!text.contains(WELCOME_START), "{text}");
        assert!(text.contains("Ask OpenAgents"), "{text}");
        assert!(!text.contains("coder-header"), "{text}");
        let shell = tab.shell_view(None, &chats).expect("the shell");
        assert_eq!((shell.place, shell.screen), ("coder", "new"));
        assert!(shell.drawer.is_none());
        // One conversation: no Coder mode words over the field.
        assert!(!text.contains("Work with Coder"), "{text}");

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
        let words = [
            "Ask OpenAgents",
            "Enter the Grid",
            "The Verse",
            "Coder",
            "Verse",
            WELCOME_TITLE,
            WELCOME_LINE,
            WELCOME_START,
        ];
        for text in words {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
