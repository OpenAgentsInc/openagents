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

#[test]
fn times_read_rfc3339_and_bare_dates() {
    use crate::coder_tab::unix_seconds;
    assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(unix_seconds("2023-11-14T22:13:20Z"), Some(1_700_000_000));
    assert_eq!(
        unix_seconds("2023-11-14T22:13:20.123456Z"),
        Some(1_700_000_000)
    );
    assert_eq!(
        unix_seconds("2023-11-15T00:13:20+02:00"),
        Some(1_700_000_000)
    );
    assert_eq!(unix_seconds("2026-01-01"), Some(1_767_225_600));
    for bad in [
        "",
        "new",
        "2026-13-01",
        "2026-01-01T25:00:00Z",
        "2026-01-01T00:00:00Q",
    ] {
        assert_eq!(unix_seconds(bad), None, "{bad}");
    }
}

/// Every row's time is its chat's last message, not when the host last
/// published the summary, which a host restart resets for every task.
#[test]
fn each_chat_shows_the_time_of_its_last_message() {
    use std::collections::BTreeMap;
    let fixture = Fixture::hosts();
    let mut snapshot = fixture.computers.snapshot().clone();
    // A host restart republishes every summary at the same moment.
    for summary in &mut snapshot.activity {
        summary.updated_at = NOW - 60;
    }
    let mut tasks: Vec<_> = snapshot
        .activity
        .iter()
        .filter(|s| s.subject_kind == nostr::activity_summary::SubjectKind::Task)
        .map(|s| (s.host.clone(), s.subject.clone()))
        .collect();
    tasks.dedup();
    assert!(tasks.len() >= 3);
    let chat = |updated: &str| coder_history::Chat {
        id: "chat".into(),
        harness: coder_history::Harness::Coder,
        native_id: None,
        title: "A chat".into(),
        title_truncated: false,
        updated_at: Some(updated.into()),
        archived: false,
        subagent: false,
        source_id: Some("source".into()),
        status: coder_history::SourceStatus::Available,
    };
    let (first, second) = (tasks[0].1.clone(), tasks[1].1.clone());
    let saved = move |_: &str, task: &str| {
        if task == first {
            // Three hours before the fixture's clock.
            Some(chat("2026-09-21T11:13:20Z"))
        } else if task == second {
            Some(chat("2026-09-21T13:53:20Z"))
        } else {
            None
        }
    };
    assert_eq!(
        crate::coder_tab::unix_seconds("2026-09-21T14:13:20Z"),
        Some(NOW)
    );
    let sent = BTreeMap::new();
    let rows = crate::coder_tab::tasks(&snapshot, &BTreeMap::new(), &sent, &saved);
    let labels: Vec<String> = rows
        .iter()
        .map(|row| match &row.element {
            rust_native::Element::Button { label, .. } => label.clone(),
            _ => String::new(),
        })
        .collect();
    // Newest message first, each with its own time; a chat the computer
    // has not listed shows no time rather than the summary's.
    assert!(labels[0].contains("20 min ago"), "{labels:?}");
    assert!(labels[1].contains("3 h ago"), "{labels:?}");
    assert!(
        labels[2..].iter().all(|label| !label.contains("ago")),
        "{labels:?}"
    );
}

/// A computer that is still connecting, as right after launch, is not the
/// same as none added: the chats list stays and says it is connecting.
#[test]
fn a_connecting_computer_is_not_a_missing_one() {
    use crate::coder_tab::{Availability, availability};
    use coder_computers::synthetic::key;
    let fixture = Fixture::hosts();
    let full = fixture.computers.snapshot().clone();
    let only = |tags: &[u8]| {
        let mut snapshot = full.clone();
        let keys: Vec<String> = tags.iter().map(|tag| key(*tag)).collect();
        snapshot.hosts.retain(|host| keys.contains(&host.key));
        snapshot
    };
    let label = |availability: Availability<'_>| match availability {
        Availability::NotConfigured => "not configured".to_owned(),
        Availability::Connecting(host) => format!("connecting {}", host.label),
        Availability::Offline(host) => format!("offline {}", host.label),
        Availability::Ready(host) => format!("ready {}", host.label),
    };
    assert_eq!(label(availability(&full, None)), "ready Studio Mac");
    // Build server is connecting; Home NAS retries after a failure.
    assert_eq!(
        label(availability(&only(&[0xa2]), None)),
        "connecting Build server"
    );
    assert_eq!(
        label(availability(&only(&[0xa3]), None)),
        "connecting Home NAS"
    );
    // Switched off, or out of date: added, but offline.
    assert_eq!(
        label(availability(&only(&[0xa7]), None)),
        "offline Travel mini"
    );
    assert_eq!(
        label(availability(&only(&[0xa4]), None)),
        "offline Old laptop"
    );
    // Not enrolled or revoked: nothing this device may run work on.
    assert_eq!(
        label(availability(&only(&[0xa5, 0xa6]), None)),
        "not configured"
    );
    assert_eq!(label(availability(&only(&[]), None)), "not configured");
}

fn texts(view: &Value) -> Vec<String> {
    nodes(view)
        .into_iter()
        .filter_map(|node| {
            let props = &node["element"]["props"];
            props["value"]
                .as_str()
                .or(props["label"].as_str())
                .map(str::to_owned)
        })
        .collect()
}

fn kinds(view: &Value, kind: &str) -> usize {
    nodes(view)
        .into_iter()
        .filter(|node| node["element"]["kind"] == kind)
        .count()
}

/// The chats list has a circular New chat button and no text field; the
/// New chat screen has the field.
#[test]
fn new_chat_is_its_own_screen() {
    let mut fixture = Fixture::hosts();
    let list = fixture.render();
    assert_eq!(kinds(&list, "composer"), 0, "{:?}", keys(&list));
    let new = node(&list, "coder-new").expect("New chat button");
    let props = &new["element"]["props"];
    assert_eq!(props["label"], "New chat");
    assert_eq!(props["icon"]["glyph"], "compose");
    assert_eq!(props["icon"]["circular"], true);
    let screen = fixture.tap("coder-new");
    assert_eq!(kinds(&screen, "composer"), 1);
    assert!(texts(&screen).contains(&"On Studio Mac · openagents".to_owned()));
    let composer = nodes(&screen)
        .into_iter()
        .find(|node| node["element"]["kind"] == "composer")
        .expect("composer");
    assert_eq!(composer["element"]["props"]["enabled"], true);
    assert!(node(&screen, "coder-chats").is_none());
    let list = fixture.tap("coder-back");
    assert!(node(&list, "coder-new").is_some());
    assert_eq!(kinds(&list, "composer"), 0);
}
