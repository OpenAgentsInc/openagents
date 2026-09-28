//! The Coder tab against Coder's offline Computers fixture: seven hosts in
//! every status, with activity summaries, and no network.

use crate::chats::Chats;
use crate::coder_tab::CoderTab;
use coder_computers::synthetic::Synthetic;
use coder_computers::{Capabilities, Computers, Platform};
use rust_native::Activation;
use serde_json::Value;

/// The fixture's clock, fixed so ages are stable.
const NOW: u64 = 1_790_000_000;

fn now() -> u64 {
    NOW
}

struct Fixture {
    coder: CoderTab,
    computers: Computers,
    chats: Chats,
    _runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

impl Fixture {
    fn new(service: Synthetic) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let secret = secp256k1::SecretKey::from_byte_array([0x11; 32]).expect("key");
        let computers = Computers::new(
            Box::new(service),
            Capabilities {
                platform: Platform::Phone,
                camera: false,
            },
            "computers:test",
        )
        .expect("computers");
        let chats = Chats::new(
            runtime.handle().clone(),
            secret,
            Err("no store in tests".into()),
            "chats:test".into(),
        );
        Self {
            coder: CoderTab::new("coder:test".into()),
            computers,
            chats,
            _runtime: runtime,
            _dir: dir,
        }
    }

    fn hosts() -> Self {
        Self::new(Synthetic::fixture(Platform::Phone, now))
    }

    fn render(&mut self) -> Value {
        self.coder
            .render(Some(&self.computers), &mut self.chats)
            .expect("coder view")
    }

    /// Tap the node `key` in the current view.
    fn tap(&mut self, key: &str) -> Value {
        let view = self.render();
        assert!(node(&view, key).is_some(), "no {key} in {:?}", keys(&view));
        self.coder.activate(
            &Activation {
                instance: view["instance"].as_str().expect("instance").into(),
                revision: view["revision"].as_u64().expect("revision"),
                node: key.into(),
            },
            Some(&mut self.computers),
            &mut self.chats,
        );
        self.render()
    }
}

fn nodes(view: &Value) -> Vec<&Value> {
    let mut out = vec![];
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        out.push(node);
        if let Some(children) = node["element"]["props"]["children"].as_array() {
            pending.extend(children.iter().rev());
        }
    }
    out
}

fn node<'a>(view: &'a Value, key: &str) -> Option<&'a Value> {
    nodes(view).into_iter().find(|node| node["key"] == key)
}

fn keys(view: &Value) -> Vec<String> {
    nodes(view)
        .into_iter()
        .filter_map(|node| node["key"].as_str().map(str::to_owned))
        .collect()
}

/// The first task row's key.
fn first_task(view: &Value) -> String {
    keys(view)
        .into_iter()
        .find(|key| key.starts_with("task-"))
        .expect("a task row")
}

#[test]
fn an_open_chat_has_a_breadcrumb_back_to_the_list() {
    let mut fixture = Fixture::hosts();
    let list = fixture.render();
    let chat = fixture.tap(&first_task(&list));
    let back = node(&chat, "coder-back").expect("breadcrumb back");
    let props = &back["element"]["props"];
    assert_eq!(props["label"], "Coder");
    assert_eq!(props["icon"]["glyph"], "back");
    assert_eq!(props["icon"]["circular"], false);
    let list = fixture.tap("coder-back");
    assert!(node(&list, "coder-back").is_none());
    assert!(keys(&list).iter().any(|key| key.starts_with("task-")));
}

#[test]
fn an_open_chat_shows_no_title_above_its_messages() {
    let mut fixture = Fixture::hosts();
    let list = fixture.render();
    let chat = fixture.tap(&first_task(&list));
    assert!(node(&chat, "coder-chat-title").is_none());
    let headings: Vec<_> = nodes(&chat)
        .into_iter()
        .filter(|node| node["element"]["props"]["role"] == "heading")
        .collect();
    assert!(headings.is_empty(), "{headings:?}");
    // The breadcrumb bar comes first.
    let root = chat["root"]["element"]["props"]["children"][0]["key"].clone();
    assert_eq!(root, "coder-chat-header");
}
