//! The phone's shell (#11126): a top bar with the menu, a **Coder** /
//! **Verse** switch, and a new-chat button; a new chat with the feature
//! cards; and a drawer with the main places and recent chats. A host that
//! draws the shell turns it on ([`CoderTab::set_shell`]); the tab then
//! leaves out its own header rows, and the host draws the top bar, the
//! drawer, and the cards from [`CoderTab::shell_view`], sending
//! [`ShellAction`]s back.
//!
//! **Coder** is the one conversation: one composer that asks OpenAgents,
//! whose network decides when a message needs Coder on a computer and
//! offers it under the reply. **Verse** is the Grid world, which the host
//! draws over the chat. The **Explore the Verse** card's **Try it** and a
//! reply's **Enter the Grid** card switch to the Verse.

use super::*;
use openagents_chat::home_cards::{self, HOME_CARDS};

/// Recent chats the drawer lists before **See all**.
const DRAWER_ROWS: usize = 8;
/// The most matches a drawer search lists.
const SEARCH_ROWS: usize = 50;
/// The longest drawer search, in bytes.
const MAX_QUERY: usize = 200;
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
    /// The feature cards were on view at the last view.
    cards_shown: bool,
    /// The card the carousel opens on this time; the list keeps it for
    /// the next time (`List::last_card`).
    start: usize,
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
    /// A feature card's **Try it**.
    TryCard { id: String },
}

/// The shell as the host draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShellView {
    /// `coder` (the conversation) or `verse` (the Grid world).
    pub place: &'static str,
    /// `new` (a new chat), `chat` (a conversation), or `list` (every chat,
    /// which draws its own header).
    pub screen: &'static str,
    /// The feature cards a new chat shows.
    pub cards: Vec<Card>,
    /// The card the carousel opens on and how it moves by itself.
    pub carousel: crate::carousel::Timing,
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
        // Each time the cards come on view they open on another card.
        let shown = screen == "new" && !self.shell.verse;
        if shown && !self.shell.cards_shown {
            self.shell.start = crate::carousel::pick_start(
                HOME_CARDS.len(),
                self.list.list.last_card,
                crate::carousel::roll(),
            );
            self.list.list.last_card = Some(self.shell.start);
            self.list.save();
        }
        self.shell.cards_shown = shown;
        Some(ShellView {
            place: if self.shell.verse { "verse" } else { "coder" },
            screen,
            cards: HOME_CARDS
                .iter()
                .map(|card| Card {
                    id: card.id,
                    title: card.title,
                    line: card.line,
                })
                .collect(),
            carousel: crate::carousel::Timing {
                start: self.shell.start,
                dwell_ms: crate::carousel::DWELL_MS,
                resume_ms: crate::carousel::RESUME_MS,
            },
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
            ShellAction::TryCard { id } => {
                let Some(card) = home_cards::find(&id) else {
                    return;
                };
                if card.opens_verse {
                    self.shell.verse = true;
                } else {
                    self.shell.verse = false;
                    self.start_talk(card.message, computers.as_deref());
                }
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

    /// A new chat under the shell: the feature cards over a composer ready
    /// to type.
    pub(super) fn shell_landing(&self) -> Node<Intent> {
        let mut children = vec![];
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
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
        children.extend(self.attachments());
        children.push(self.composer_with(
            self.ask_words().to_owned(),
            true,
            false,
            &[],
            None,
            true,
        ));
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
    fn the_switch_is_coder_or_verse_and_the_verse_card_opens_the_grid() {
        let (_runtime, mut chats) = chats();
        let mut tab = CoderTab::new("coder:shell".into()).with_shell(true);
        let view = tab.shell_view(None, &chats).unwrap();
        assert_eq!((view.place, view.screen), ("coder", "new"));
        let json = serde_json::to_value(&view).unwrap();
        assert!(json.get("mode").is_none(), "no Chat / Code mode");
        assert_eq!(json["carousel"]["dwell_ms"], crate::carousel::DWELL_MS);
        // Explore the Verse's Try it goes straight to the Grid, sending
        // nothing.
        tab.shell(
            ShellAction::TryCard { id: "verse".into() },
            None,
            &mut chats,
        );
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
    }

    #[test]
    fn the_cards_open_on_another_card_each_time_they_come_on_view() {
        let (_runtime, mut chats) = chats();
        let mut tab = CoderTab::new("coder:cards".into()).with_shell(true);
        let mut last = tab.shell_view(None, &chats).unwrap().carousel.start;
        for _ in 0..12 {
            // Still on view: the same card.
            assert_eq!(tab.shell_view(None, &chats).unwrap().carousel.start, last);
            tab.shell(ShellAction::Switch { verse: true }, None, &mut chats);
            tab.shell_view(None, &chats);
            tab.shell(ShellAction::Switch { verse: false }, None, &mut chats);
            let start = tab.shell_view(None, &chats).unwrap().carousel.start;
            assert_ne!(start, last);
            assert!(start < HOME_CARDS.len());
            last = start;
        }
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
        assert_eq!((shell.place, shell.screen), ("coder", "new"));
        assert_eq!(shell.cards.len(), 4);
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
        let mut words: Vec<&str> = vec![
            "Ask OpenAgents",
            "Enter the Grid",
            "The Verse",
            "Coder",
            "Verse",
        ];
        for card in HOME_CARDS {
            words.extend([card.title, card.line]);
        }
        for text in words {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
