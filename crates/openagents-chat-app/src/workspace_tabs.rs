//! Conversation-owned read-only files. Owners supply bytes; views never open paths.

use route_contract::{Digest, digest_of, workspace as wire};
use rust_native::{Axis, Element, Node, TextRole};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};

pub const MAX_TABS: usize = 8;
pub const MAX_TOTAL_TABS: usize = 64;
pub const MAX_CONVERSATIONS: usize = 32;
pub const MAX_CACHED_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_READS: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    pub reference: wire::Reference,
    pub path: String,
    pub line: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Key {
    pub conversation: String,
    pub host: String,
    pub task: String,
    pub checkout: String,
    pub target: String,
    pub path: String,
}
impl Key {
    pub fn id(&self) -> Digest {
        digest_of(self)
    }
    fn belongs_to(&self, reference: &wire::Reference) -> bool {
        self.host == reference.host
            && self.task == reference.task
            && self.checkout == reference.authority.checkout
            && self.target == reference.authority.target
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub instance: String,
    pub conversation: String,
    pub document: Digest,
    pub generation: u64,
    pub request: wire::Request,
}
impl Read {
    pub fn valid(&self) -> bool {
        self.instance.len() == 32
            && self.instance.bytes().all(|b| b.is_ascii_hexdigit())
            && !self.conversation.is_empty()
            && self.conversation.len() <= 128
            && !self.conversation.chars().any(char::is_control)
            && self.generation > 0
            && self.request.reference.host == "local"
            && self.request.reference.valid()
            && self.request.operation.valid()
            && matches!(&self.request.operation, wire::Operation::Read {path} if self.document == Key {
                conversation:self.conversation.clone(),host:self.request.reference.host.clone(),
                task:self.request.reference.task.clone(),checkout:self.request.reference.authority.checkout.clone(),
                target:self.request.reference.authority.target.clone(),path:path.clone(),
            }.id())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Control {
    Select(Digest),
    Close(Digest),
    Refresh,
    Hide,
    Previous,
    Next,
    Browser,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub conversation: String,
    pub view: Digest,
    pub control: Control,
}

pub struct Tab {
    pub key: Key,
    pub id: Digest,
    pub reference: wire::Reference,
    pub content: Option<Arc<String>>,
    pub content_id: Option<Digest>,
    pub truncated: bool,
    pub stale: bool,
    pub error: Option<String>,
    pub loading: bool,
    pub scroll: f32,
    pub jump: Option<usize>,
    pub generation: u64,
    touched: u64,
}
#[derive(Default)]
struct Conversation {
    tabs: Vec<Tab>,
    selected: Option<Digest>,
    visible: bool,
    revision: u64,
    reference: Option<wire::Reference>,
    document_focus: bool,
    browser: bool,
    error: Option<String>,
}
pub struct Workspaces {
    instance: String,
    conversations: BTreeMap<String, Conversation>,
    inflight: BTreeMap<u64, Read>,
    serial: u64,
    active: Option<String>,
}
impl Default for Workspaces {
    fn default() -> Self {
        Self {
            instance: uuid::Uuid::new_v4().simple().to_string(),
            conversations: Default::default(),
            inflight: Default::default(),
            serial: 0,
            active: None,
        }
    }
}
impl Workspaces {
    fn stamp(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }
    pub fn activate(&mut self, chat: Option<&str>) {
        self.active = chat.map(str::to_owned);
    }
    pub fn focus(&mut self, chat: &str, focused: bool) {
        if let Some(c) = self.conversations.get_mut(chat) {
            c.document_focus = focused;
        }
    }
    pub fn focused(&self, chat: &str) -> bool {
        self.conversations
            .get(chat)
            .is_some_and(|c| c.document_focus && c.visible && !c.browser && c.selected.is_some())
    }
    pub fn browser_visible(&self, chat: &str) -> bool {
        self.conversations
            .get(chat)
            .is_some_and(|c| c.visible && c.browser)
    }
    pub fn visible(&self, chat: &str) -> bool {
        self.conversations.get(chat).is_some_and(|c| c.visible)
    }
    pub fn show(&mut self, chat: &str) -> bool {
        if chat.is_empty() || chat.len() > 128 || chat.chars().any(char::is_control) {
            return false;
        }
        if !self.conversations.contains_key(chat) && self.conversations.len() >= MAX_CONVERSATIONS {
            let empty = self.conversations.iter().find_map(|(id, c)| {
                (c.tabs.is_empty() && Some(id) != self.active.as_ref()).then(|| id.clone())
            });
            if let Some(id) = empty {
                self.conversations.remove(&id);
            } else {
                return false;
            }
        }
        let stamp = self.stamp();
        let c = self.conversations.entry(chat.into()).or_default();
        c.visible = true;
        c.browser = true;
        c.revision = stamp;
        true
    }
    pub fn tabs(&self, chat: &str) -> &[Tab] {
        self.conversations.get(chat).map_or(&[], |c| &c.tabs)
    }
    pub fn selected(&self, chat: &str) -> Option<&Tab> {
        let c = self.conversations.get(chat)?;
        c.tabs
            .iter()
            .find(|tab| Some(&tab.id) == c.selected.as_ref())
    }
    pub fn selected_mut(&mut self, chat: &str) -> Option<&mut Tab> {
        let c = self.conversations.get_mut(chat)?;
        c.tabs
            .iter_mut()
            .find(|tab| Some(&tab.id) == c.selected.as_ref())
    }
    pub fn bind(&mut self, chat: &str, reference: &wire::Reference) {
        if !reference.valid() || reference.host != "local" {
            return;
        }
        let stamp = self.stamp();
        let Some(c) = self.conversations.get_mut(chat) else {
            return;
        };
        if c.reference.as_ref() == Some(reference) {
            return;
        }
        c.reference = Some(reference.clone());
        c.revision = stamp;
        for tab in &mut c.tabs {
            if tab.key.belongs_to(reference) && tab.reference != *reference {
                tab.reference = reference.clone();
                tab.generation = stamp;
                tab.loading = false;
                tab.stale = true;
            } else if !tab.key.belongs_to(reference) {
                tab.generation = stamp;
                tab.loading = false;
                tab.stale = true;
            }
        }
    }
    pub fn invalidate(&mut self, chat: &str) {
        let stamp = self.stamp();
        if let Some(c) = self.conversations.get_mut(chat) {
            c.reference = None;
            c.revision = stamp;
            for tab in &mut c.tabs {
                tab.generation = stamp;
                tab.loading = false;
                tab.stale = true;
            }
        }
    }
    pub fn open(&mut self, chat: &str, open: Open) -> Option<Digest> {
        let key = Key {
            conversation: chat.into(),
            host: open.reference.host.clone(),
            task: open.reference.task.clone(),
            checkout: open.reference.authority.checkout.clone(),
            target: open.reference.authority.target.clone(),
            path: open.path.clone(),
        };
        let probe = Read {
            instance: self.instance.clone(),
            conversation: chat.into(),
            document: key.id(),
            generation: 1,
            request: wire::Request {
                reference: open.reference.clone(),
                operation: wire::Operation::Read { path: open.path },
            },
        };
        if !probe.valid() || !self.show(chat) {
            return None;
        }
        self.bind(chat, &open.reference);
        let total: usize = self.conversations.values().map(|c| c.tabs.len()).sum();
        let stamp = self.stamp();
        let c = self.conversations.get_mut(chat)?;
        let id = key.id();
        c.browser = false;
        if let Some(tab) = c.tabs.iter_mut().find(|tab| tab.id == id) {
            tab.touched = stamp;
            if let Some(line) = open.line {
                tab.jump = Some(line);
            }
            if tab.content.is_none() && tab.error.is_none() {
                tab.loading = true;
                tab.generation = stamp;
            }
            c.selected = Some(id.clone());
            c.revision = stamp;
            return Some(id);
        }
        if c.tabs.len() >= MAX_TABS || total >= MAX_TOTAL_TABS {
            c.error = Some("Close a file tab before opening another file.".into());
            return None;
        }
        c.error = None;
        c.selected = Some(id.clone());
        c.revision = stamp;
        c.tabs.push(Tab {
            key,
            id: id.clone(),
            reference: open.reference,
            content: None,
            content_id: None,
            truncated: false,
            stale: false,
            error: None,
            loading: true,
            scroll: 0.0,
            jump: Some(open.line.unwrap_or(1)),
            generation: stamp,
            touched: stamp,
        });
        Some(id)
    }
    fn identity(&self, chat: &str) -> Digest {
        digest_of(&(chat, self.conversations.get(chat).map(|c| c.revision)))
    }
    pub fn target(&self, chat: &str, control: Control) -> Target {
        Target {
            conversation: chat.into(),
            view: self.identity(chat),
            control,
        }
    }
    pub fn apply(&mut self, target: &Target) -> bool {
        if target.view != self.identity(&target.conversation) {
            return false;
        }
        let stamp = self.stamp();
        let Some(c) = self.conversations.get_mut(&target.conversation) else {
            return false;
        };
        match &target.control {
            Control::Hide => c.visible = false,
            Control::Browser => c.browser = !c.browser,
            Control::Close(id) => {
                let Some(index) = c.tabs.iter().position(|tab| &tab.id == id) else {
                    return false;
                };
                c.tabs.remove(index);
                if c.selected.as_ref() == Some(id) {
                    c.selected = c
                        .tabs
                        .get(index.min(c.tabs.len().saturating_sub(1)))
                        .map(|tab| tab.id.clone());
                }
                c.error = None;
            }
            Control::Select(id) => {
                if !c.tabs.iter().any(|tab| &tab.id == id) {
                    return false;
                }
                c.selected = Some(id.clone());
                c.browser = false;
            }
            Control::Previous | Control::Next if !c.tabs.is_empty() => {
                let at = c
                    .tabs
                    .iter()
                    .position(|tab| Some(&tab.id) == c.selected.as_ref())
                    .unwrap_or(0);
                let next = if target.control == Control::Previous {
                    (at + c.tabs.len() - 1) % c.tabs.len()
                } else {
                    (at + 1) % c.tabs.len()
                };
                c.selected = Some(c.tabs[next].id.clone());
                c.browser = false;
            }
            Control::Refresh => {
                let Some(tab) = c
                    .tabs
                    .iter_mut()
                    .find(|tab| Some(&tab.id) == c.selected.as_ref())
                else {
                    return false;
                };
                if !c.reference.as_ref().is_some_and(|r| tab.key.belongs_to(r)) {
                    tab.error = Some("This file belongs to an earlier folder. Browse files again to open its current version.".into());
                } else {
                    tab.reference = c.reference.clone().unwrap();
                    tab.generation = stamp;
                    tab.loading = true;
                    tab.error = None;
                }
            }
            _ => return false,
        }
        if let Some(tab) = c
            .tabs
            .iter_mut()
            .find(|tab| Some(&tab.id) == c.selected.as_ref())
        {
            tab.touched = stamp;
            if matches!(
                target.control,
                Control::Select(_) | Control::Previous | Control::Next
            ) && tab.content.is_none()
                && tab.error.is_none()
            {
                tab.generation = stamp;
                tab.loading = true;
            }
        }
        c.revision = stamp;
        true
    }
    pub fn next_read(&mut self, chat: &str) -> Option<Read> {
        if self.inflight.len() >= MAX_READS || !self.visible(chat) {
            return None;
        }
        let c = self.conversations.get(chat)?;
        let tab = c
            .tabs
            .iter()
            .filter(|tab| {
                tab.loading && !self.inflight.values().any(|read| read.document == tab.id)
            })
            .min_by_key(|tab| usize::from(Some(&tab.id) != c.selected.as_ref()))?;
        let read = Read {
            instance: self.instance.clone(),
            conversation: chat.into(),
            document: tab.id.clone(),
            generation: tab.generation,
            request: wire::Request {
                reference: tab.reference.clone(),
                operation: wire::Operation::Read {
                    path: tab.key.path.clone(),
                },
            },
        };
        self.inflight.insert(read.generation, read.clone());
        Some(read)
    }
    pub fn received(&mut self, read: &Read, result: Result<wire::Output, String>) -> bool {
        if self.inflight.get(&read.generation) != Some(read) {
            return false;
        }
        self.inflight.remove(&read.generation);
        let stamp = self.stamp();
        let Some(c) = self.conversations.get_mut(&read.conversation) else {
            return false;
        };
        let Some(tab) = c.tabs.iter_mut().find(|t| {
            t.id == read.document
                && t.generation == read.generation
                && t.reference == read.request.reference
        }) else {
            return false;
        };
        tab.loading = false;
        tab.touched = stamp;
        c.revision = stamp;
        match result {
            Ok(output) if output.valid_for(&read.request) => {
                tab.content = output.text.map(Arc::new);
                tab.content_id = output.content;
                tab.truncated = output.truncated;
                tab.stale = false;
                tab.error = None;
            }
            Ok(_) => {
                tab.stale = true;
                tab.error =
                    Some("This file couldn't be confirmed. Refresh it to try again.".into());
            }
            Err(error) => {
                tab.stale = true;
                tab.error = Some(bounded_error(&error));
            }
        }
        self.trim_cache(&read.document);
        true
    }
    pub fn revision(&self, chat: &str) -> u64 {
        self.conversations.get(chat).map_or(0, |c| c.revision)
    }
    pub fn cached_bytes(&self) -> usize {
        self.conversations
            .values()
            .flat_map(|c| &c.tabs)
            .filter_map(|t| t.content.as_ref())
            .map(|s| s.len())
            .sum()
    }
    fn trim_cache(&mut self, keep: &Digest) {
        let mut candidates = Vec::new();
        for (chat, c) in &self.conversations {
            for tab in &c.tabs {
                if &tab.id != keep
                    && !(Some(chat) == self.active.as_ref() && c.selected.as_ref() == Some(&tab.id))
                {
                    candidates.push((tab.touched, chat.clone(), tab.id.clone()));
                }
            }
        }
        candidates.sort_by_key(|x| x.0);
        for (_, chat, id) in candidates {
            if self.cached_bytes() <= MAX_CACHED_BYTES {
                break;
            }
            if let Some(tab) = self
                .conversations
                .get_mut(&chat)
                .and_then(|c| c.tabs.iter_mut().find(|t| t.id == id))
            {
                tab.content = None;
            }
        }
    }
    pub fn forget(&mut self, chat: &str) {
        self.conversations.remove(chat);
    }
    pub fn view<I>(&self, chat: &str, intent: impl Fn(Target) -> I) -> Option<Node<I>> {
        let c = self.conversations.get(chat)?;
        if !c.visible {
            return None;
        }
        let button = |key: &str, label: String, control: Control, enabled| Node {
            key: format!("file-tabs-{key}"),
            style: Default::default(),
            element: Element::Button {
                label,
                enabled,
                intent: intent(self.target(chat, control)),
                shortcut: None,
                icon: None,
            },
        };
        let text = |key: &str, value: String| Node {
            key: format!("file-tabs-{key}"),
            style: Default::default(),
            element: Element::Text {
                value,
                role: TextRole::Status,
            },
        };
        let mut tabs = vec![];
        let selected = c
            .tabs
            .iter()
            .position(|t| Some(&t.id) == c.selected.as_ref())
            .unwrap_or(0);
        let start = selected
            .saturating_sub(1)
            .min(c.tabs.len().saturating_sub(3));
        for (index, tab) in c.tabs.iter().enumerate().skip(start).take(3) {
            let name = tab.key.path.rsplit('/').next().unwrap_or(&tab.key.path);
            let name = short(name, 24);
            tabs.push(button(
                &format!("select-{index}"),
                format!(
                    "{}{name}",
                    if c.selected.as_ref() == Some(&tab.id) {
                        "• "
                    } else {
                        ""
                    }
                ),
                Control::Select(tab.id.clone()),
                true,
            ));
        }
        let mut children = vec![Node {
            key: "file-tabs-list".into(),
            style: Default::default(),
            element: Element::Stack {
                axis: Axis::Wrap,
                children: tabs,
            },
        }];
        let mut navigation = vec![button(
            "browse",
            if c.browser {
                "Show file"
            } else {
                "Browse files"
            }
            .into(),
            Control::Browser,
            !c.browser || c.selected.is_some(),
        )];
        if let Some(id) = &c.selected {
            navigation.push(button(
                "close",
                "Close file".into(),
                Control::Close(id.clone()),
                true,
            ));
            if c.tabs.len() > 1 {
                navigation.push(button(
                    "previous",
                    "Previous file".into(),
                    Control::Previous,
                    true,
                ));
                navigation.push(button("next", "Next file".into(), Control::Next, true));
            }
        }
        children.push(Node {
            key: "file-tabs-navigation".into(),
            style: Default::default(),
            element: Element::Stack {
                axis: Axis::Wrap,
                children: navigation,
            },
        });
        if let Some(error) = &c.error {
            children.push(text("limit", error.clone()));
        }
        if let Some(tab) = self.selected(chat) {
            children.push(text("path", short(&tab.key.path, 96)));
            children.push(text(
                "folder",
                format!("Folder: {}", short(&tab.key.checkout, 96)),
            ));
            let mut status = String::from("Read only");
            if tab.loading {
                status.push_str(" · Loading…");
            } else if tab.stale {
                status.push_str(" · Refresh to check for changes");
            }
            if tab.truncated {
                status.push_str(" · First 64 KiB shown");
            }
            children.push(text("status", status));
            if let Some(error) = &tab.error {
                children.push(text("error", error.clone()));
            }
            if tab.content.as_ref().is_some_and(|s| s.is_empty()) {
                children.push(text("empty", "This file is empty.".into()));
            }
            children.push(button(
                "refresh",
                "Refresh file".into(),
                Control::Refresh,
                !tab.loading,
            ));
        } else {
            children.push(text("empty", "Choose a file to open it here.".into()));
        }
        children.push(button("hide", "Hide files".into(), Control::Hide, true));
        Some(Node {
            key: "file-tabs".into(),
            style: Default::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        })
    }
}
fn short(value: &str, max: usize) -> String {
    let mut text: String = value.chars().take(max).collect();
    if text.len() < value.len() {
        text.push('…');
    }
    text
}
fn bounded_error(error: &str) -> String {
    let mut text: String = error
        .chars()
        .filter(|c| !c.is_control())
        .take(1024)
        .collect();
    if text.is_empty() {
        text = "This file couldn't be loaded. Refresh it to try again.".into();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(turn: usize) -> wire::Reference {
        let mut reference = wire::Reference {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            host: "local".into(),
            task: "task".into(),
            turn,
            authority: route_contract::pending::Authority {
                intent: "intent".into(),
                grant: "grant".into(),
                checkout: "/project".into(),
                target: "inode".into(),
            },
        };
        reference.seal();
        reference
    }
    fn open(state: &mut Workspaces, chat: &str, path: &str) -> Digest {
        state
            .open(
                chat,
                Open {
                    reference: reference(1),
                    path: path.into(),
                    line: None,
                },
            )
            .unwrap()
    }
    fn output(read: &Read, text: &str) -> wire::Output {
        wire::Output {
            reference: read.request.reference.id.clone(),
            operation: read.request.operation.id(),
            rows: vec![],
            text: Some(text.into()),
            content: Some(Digest::of_bytes(text.as_bytes())),
            truncated: false,
        }
    }
    fn loaded(state: &mut Workspaces, chat: &str, path: &str, text: &str) -> Digest {
        let id = open(state, chat, path);
        let read = state.next_read(chat).unwrap();
        assert!(state.received(&read, Ok(output(&read, text))));
        id
    }
    #[test]
    fn tabs_deduplicate_without_replacing_the_browser_or_another_conversations_selection() {
        let mut state = Workspaces::default();
        let first = loaded(&mut state, "chat", "src/a.rs", "first");
        state.selected_mut("chat").unwrap().scroll = 240.0;
        let second = loaded(&mut state, "chat", "src/b.rs", "second");
        loaded(&mut state, "other", "src/a.rs", "other conversation");
        assert_eq!(open(&mut state, "chat", "src/a.rs"), first);
        assert_eq!(state.tabs("chat").len(), 2);
        assert!(state.next_read("chat").is_none());
        assert_eq!(state.selected("chat").unwrap().scroll, 240.0);
        assert_eq!(
            state
                .selected("other")
                .unwrap()
                .content
                .as_deref()
                .map(String::as_str),
            Some("other conversation")
        );
        let old = state.target("chat", Control::Close(first.clone()));
        assert!(state.apply(&state.target("chat", Control::Next)));
        assert_eq!(state.selected("chat").unwrap().id, second);
        assert!(!state.apply(&old));
        assert!(state.show("chat"));
        assert!(state.browser_visible("chat"));
        assert!(state.apply(&state.target("chat", Control::Select(first.clone()))));
        assert!(!state.browser_visible("chat"));
        assert!(state.apply(&state.target("chat", Control::Close(second))));
        assert_eq!(state.selected("chat").unwrap().id, first);
    }
    #[test]
    fn closing_and_reopening_a_file_rejects_the_old_read_even_with_identical_owner_and_path() {
        let mut state = Workspaces::default();
        let id = open(&mut state, "chat", "file.txt");
        let old = state.next_read("chat").unwrap();
        assert!(state.apply(&state.target("chat", Control::Close(id.clone()))));
        assert_eq!(open(&mut state, "chat", "file.txt"), id);
        assert!(state.next_read("chat").is_none());
        assert!(!state.received(&old, Ok(output(&old, "old"))));
        let new = state.next_read("chat").unwrap();
        assert_ne!(new.generation, old.generation);
        assert!(state.received(&new, Ok(output(&new, "current"))));
        assert!(!state.received(&old, Ok(output(&old, "late"))));
        assert_eq!(
            state
                .selected("chat")
                .unwrap()
                .content
                .as_deref()
                .map(String::as_str),
            Some("current")
        );
    }
    #[test]
    fn failed_or_forged_refresh_keeps_the_last_readable_text_and_scroll() {
        let mut state = Workspaces::default();
        loaded(&mut state, "chat", "file.txt", "keep this");
        state.selected_mut("chat").unwrap().scroll = 50.0;
        state.apply(&state.target("chat", Control::Refresh));
        let read = state.next_read("chat").unwrap();
        let mut forged = output(&read, "wrong");
        forged.content = Some(digest_of(&"wrong digest"));
        assert!(state.received(&read, Ok(forged)));
        assert_eq!(
            state
                .selected("chat")
                .unwrap()
                .content
                .as_deref()
                .map(String::as_str),
            Some("keep this")
        );
        assert!(state.selected("chat").unwrap().stale);
        state.apply(&state.target("chat", Control::Refresh));
        let read = state.next_read("chat").unwrap();
        state.received(&read, Err("This file isn't text.".into()));
        assert_eq!(state.selected("chat").unwrap().scroll, 50.0);
        assert_eq!(
            state.selected("chat").unwrap().error.as_deref(),
            Some("This file isn't text.")
        );
    }
    #[test]
    fn a_new_checkout_cannot_reuse_a_tab_or_accept_its_response() {
        let mut state = Workspaces::default();
        let id = open(&mut state, "chat", "file.txt");
        let old = state.next_read("chat").unwrap();
        let mut changed = reference(2);
        changed.authority.target = "new-inode".into();
        changed.seal();
        state.bind("chat", &changed);
        assert!(!state.received(&old, Ok(output(&old, "wrong checkout"))));
        state.apply(&state.target("chat", Control::Refresh));
        assert!(state.next_read("chat").is_none());
        let new = state
            .open(
                "chat",
                Open {
                    reference: changed,
                    path: "file.txt".into(),
                    line: None,
                },
            )
            .unwrap();
        assert_ne!(id, new);
        let read = state.next_read("chat").unwrap();
        assert!(read.valid());
        state.received(&read, Ok(output(&read, "new checkout")));
        assert_eq!(state.tabs("chat").len(), 2);
    }
    #[test]
    fn new_authority_preserves_read_only_content_but_invalidates_pending_reads() {
        let mut state = Workspaces::default();
        loaded(&mut state, "chat", "file.txt", "earlier");
        state.apply(&state.target("chat", Control::Refresh));
        let old = state.next_read("chat").unwrap();
        state.bind("chat", &reference(2));
        assert!(!state.received(&old, Ok(output(&old, "late"))));
        assert!(state.selected("chat").unwrap().stale);
        state.apply(&state.target("chat", Control::Refresh));
        let new = state.next_read("chat").unwrap();
        assert_eq!(new.request.reference.turn, 2);
        assert!(state.received(&new, Ok(output(&new, "later"))));
    }
    #[test]
    fn reads_are_bounded_and_hidden_conversations_do_not_start_more_work() {
        let mut state = Workspaces::default();
        for chat in ["a", "b", "c"] {
            open(&mut state, chat, "file.txt");
        }
        let a = state.next_read("a").unwrap();
        let b = state.next_read("b").unwrap();
        assert!(state.next_read("c").is_none());
        state.apply(&state.target("c", Control::Hide));
        state.received(&a, Ok(output(&a, "a")));
        assert!(state.next_read("c").is_none());
        state.show("c");
        assert!(state.next_read("c").is_some());
        let mut wrong = b.clone();
        wrong.conversation = "a".into();
        assert!(!state.received(&wrong, Ok(output(&b, "wrong"))));
        assert!(state.received(&b, Ok(output(&b, "b"))));
    }
    #[test]
    fn file_count_and_retained_bytes_have_separate_bounds() {
        let mut state = Workspaces::default();
        let text = "x".repeat(wire::MAX_TEXT);
        for chat in 0..5 {
            for file in 0..MAX_TABS {
                loaded(
                    &mut state,
                    &format!("chat-{chat}"),
                    &format!("file-{file}.txt"),
                    &text,
                );
            }
        }
        assert_eq!(state.tabs("chat-0").len(), MAX_TABS);
        assert!(state.cached_bytes() <= MAX_CACHED_BYTES);
        assert!(state.tabs("chat-0").iter().any(|tab| tab.content.is_none()));
        assert!(
            state
                .open(
                    "chat-0",
                    Open {
                        reference: reference(1),
                        path: "extra".into(),
                        line: None
                    }
                )
                .is_none()
        );
        assert_eq!(state.tabs("chat-0").len(), MAX_TABS);
        state.apply(&state.target(
            "chat-0",
            Control::Select(state.tabs("chat-0")[0].id.clone()),
        ));
        assert!(state.next_read("chat-0").is_some());
    }
    #[test]
    fn global_limits_recover_when_files_are_closed() {
        let mut state = Workspaces::default();
        for chat in 0..MAX_CONVERSATIONS {
            for file in 0..2 {
                open(&mut state, &format!("chat-{chat}"), &format!("{file}.txt"));
            }
        }
        assert_eq!(
            state
                .conversations
                .values()
                .map(|c| c.tabs.len())
                .sum::<usize>(),
            MAX_TOTAL_TABS
        );
        assert!(!state.show("another"));
        assert!(
            state
                .open(
                    "chat-0",
                    Open {
                        reference: reference(1),
                        path: "extra.txt".into(),
                        line: None
                    }
                )
                .is_none()
        );
        for id in state
            .tabs("chat-0")
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>()
        {
            assert!(state.apply(&state.target("chat-0", Control::Close(id))));
        }
        assert!(
            state
                .open(
                    "another",
                    Open {
                        reference: reference(1),
                        path: "new.txt".into(),
                        line: None
                    }
                )
                .is_some()
        );
        assert_eq!(state.conversations.len(), MAX_CONVERSATIONS);
    }
    #[test]
    fn an_old_window_reply_cannot_finish_a_new_windows_identical_file_read() {
        let mut old = Workspaces::default();
        open(&mut old, "chat", "file.txt");
        let old_read = old.next_read("chat").unwrap();
        let mut current = Workspaces::default();
        open(&mut current, "chat", "file.txt");
        let read = current.next_read("chat").unwrap();
        assert_eq!(old_read.generation, read.generation);
        assert_ne!(old_read.instance, read.instance);
        assert!(!current.received(&old_read, Ok(output(&old_read, "old account"))));
        assert!(current.received(&read, Ok(output(&read, "current account"))));
    }
    #[test]
    fn invalid_paths_and_remote_owners_never_become_reads() {
        let mut state = Workspaces::default();
        for path in ["../secret", "/absolute", ".git/config", "a\\b", ""] {
            assert!(
                state
                    .open(
                        "chat",
                        Open {
                            reference: reference(1),
                            path: path.into(),
                            line: None
                        }
                    )
                    .is_none()
            );
        }
        let mut remote = reference(1);
        remote.host = "remote".into();
        remote.seal();
        assert!(
            state
                .open(
                    "chat",
                    Open {
                        reference: remote,
                        path: "file.txt".into(),
                        line: None
                    }
                )
                .is_none()
        );
        assert!(state.next_read("chat").is_none());
    }
    #[test]
    fn the_view_is_bounded_plain_copy_and_carries_only_current_tab_controls() {
        let mut state = Workspaces::default();
        loaded(&mut state, "chat", "src/a.rs", "fn main() {}\n");
        let node = state.view("chat", |target| target).unwrap();
        assert!(
            rust_native::View::new("files", 1, node.clone())
                .validate()
                .is_ok()
        );
        fn check<I>(node: &Node<I>) {
            match &node.element {
                Element::Text { value, .. } | Element::Button { label: value, .. } => {
                    assert!(oa_copy::violations(value, &[]).is_empty(), "{value}")
                }
                Element::Stack { children, .. } => {
                    for child in children {
                        check(child)
                    }
                }
                _ => {}
            }
        }
        check(&node);
    }
}
