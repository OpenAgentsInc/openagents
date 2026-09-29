//! The Coder tab against Coder's offline Computers fixture: seven hosts in
//! every status, with activity summaries, and no network.

use crate::chats::Chats;
use crate::coder_list::Store;
use crate::coder_tab::CoderTab;
use coder_computers::cache::Cache;
use coder_computers::synthetic::Synthetic;
use coder_computers::{Capabilities, Computers, ComputersService, Platform};
use rust_native::Activation;
use serde_json::{Value, json};

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
    fn new(service: impl ComputersService + Send + 'static) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        Self::in_dir(service, dir)
    }

    /// A fixture whose Coder list lives in `dir`, as the app's store does.
    fn in_dir(service: impl ComputersService + Send + 'static, dir: tempfile::TempDir) -> Self {
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
        );
        Self {
            coder: CoderTab::new("coder:test".into()).with_list(Store::open(
                Cache::open(&dir.path().join("coder-list"), &secret).ok(),
            )),
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

    /// The previous chats, behind the menu button.
    fn list(&mut self) -> Value {
        let view = self.render();
        if node(&view, "coder-menu").is_some() {
            self.tap("coder-menu")
        } else {
            view
        }
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

/// An open chat's header has the menu button for the previous chats and a
/// button for a new chat. The previous chats close back to the chat.
#[test]
fn an_open_chat_has_the_menu_and_a_new_chat() {
    let mut fixture = Fixture::hosts();
    let list = fixture.list();
    let task = first_task(&list);
    let chat = fixture.tap(&task);
    assert!(node(&chat, "coder-back").is_none());
    let menu = node(&chat, "coder-menu").expect("menu");
    let props = &menu["element"]["props"];
    assert_eq!(props["label"], "Previous chats");
    assert_eq!(props["icon"]["glyph"], "menu");
    assert_eq!(props["icon"]["circular"], true);
    let new = node(&chat, "coder-new").expect("new chat");
    assert_eq!(new["element"]["props"]["icon"]["glyph"], "compose");
    // The previous chats, and back to the chat that was open.
    let list = fixture.tap("coder-menu");
    assert!(keys(&list).iter().any(|key| key.starts_with("task-")));
    assert_eq!(kinds(&list, "composer"), 0);
    let back = node(&list, "coder-back").expect("close");
    assert_eq!(back["element"]["props"]["label"], "OpenAgents");
    fixture.tap("coder-back");
    assert!(fixture.coder.open_task().is_some());
    // A new chat closes it, ready to type.
    let landing = fixture.tap("coder-new");
    assert!(fixture.coder.open_task().is_none());
    assert_eq!(composer_of(&landing)["focus"], true);
    assert!(node(&landing, "coder-chats").is_none());
}

#[test]
fn an_open_chat_shows_no_title_above_its_messages() {
    let mut fixture = Fixture::hosts();
    let list = fixture.list();
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
    let rows = crate::coder_tab::tasks(
        &snapshot,
        &snapshot.activity,
        &crate::coder_list::List::default(),
        &saved,
    );
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

/// The tab opens on a new chat, ready to type, and nothing on it picks where
/// the chat goes: no Cloud or computer selector, even with a computer ready
/// and chats on it. Above the field sit no previous chats and no way to run
/// Coder or connect a computer; the previous chats are behind the menu.
#[test]
fn the_tab_opens_on_a_new_chat_ready_to_type() {
    let mut fixture = Fixture::hosts();
    let screen = fixture.render();
    assert_eq!(kinds(&screen, "composer"), 1);
    let composer = composer_of(&screen);
    assert_eq!(composer["enabled"], true);
    // The screen exists to write: it opens with the cursor in the field.
    assert_eq!(composer["focus"], true);
    // Even with a computer ready, a new chat goes to OpenAgents.
    assert_eq!(composer["placeholder"], "Message OpenAgents");
    let text = texts(&screen);
    assert!(text.contains(&"OpenAgents".to_owned()), "{text:?}");
    for key in keys(&screen) {
        assert!(
            !key.starts_with("coder-target")
                && !key.starts_with("coder-continue")
                && !key.starts_with("coder-repo")
                && !key.starts_with("coder-run")
                && !key.starts_with("coder-connect"),
            "{key} on the new chat: {:?}",
            keys(&screen)
        );
    }
    assert!(!text.contains(&"Cloud".to_owned()), "{text:?}");
    let glyphs = serde_json::to_string(&screen).unwrap();
    assert!(!glyphs.contains("\"history\""), "{glyphs}");
    assert!(node(&screen, "coder-chats").is_none());
    assert!(!keys(&screen).iter().any(|key| key.starts_with("task-")));
    let menu = node(&screen, "coder-menu").expect("menu");
    assert_eq!(menu["element"]["props"]["icon"]["glyph"], "menu");
    // The previous chats: a list with a New chat button and no field.
    let list = fixture.tap("coder-menu");
    assert_eq!(kinds(&list, "composer"), 0, "{:?}", keys(&list));
    assert!(keys(&list).iter().any(|key| key.starts_with("task-")));
    let new = node(&list, "coder-new").expect("New chat button");
    let props = &new["element"]["props"];
    assert_eq!(props["label"], "New chat");
    assert_eq!(props["icon"]["glyph"], "compose");
    assert_eq!(props["icon"]["circular"], true);
    let screen = fixture.tap("coder-new");
    assert_eq!(kinds(&screen, "composer"), 1);
}

/// A message typed on a new chat goes to OpenAgents even with a computer
/// ready: it starts a conversation, not a task on the computer.
#[test]
fn a_new_chat_goes_to_openagents_with_a_computer_ready() {
    let hand = Hand::default();
    let mut fixture = Fixture::hosts().answered_by(&hand);
    fixture.say("Tidy the scratch notes");
    assert!(fixture.coder.open_task().is_none());
    assert_eq!(
        hand.asked(),
        vec![vec!["Tidy the scratch notes".to_owned()]]
    );
}

/// The fixture's hosts right after a relaunch: every computer is added, and
/// no host has sent a summary yet.
struct Relaunched(Synthetic);

type Answer<T> = coder_computers::service::Result<T>;

impl ComputersService for Relaunched {
    fn snapshot(&mut self) -> Answer<coder_computers::Snapshot> {
        let mut snapshot = self.0.snapshot()?;
        snapshot.activity.clear();
        Ok(snapshot)
    }
    fn set_enabled(&mut self, host: &str, enabled: bool) -> Answer<()> {
        self.0.set_enabled(host, enabled)
    }
    fn retry_now(&mut self, host: &str) -> Answer<()> {
        self.0.retry_now(host)
    }
    fn forget(&mut self, host: &str) -> Answer<()> {
        self.0.forget(host)
    }
    fn redeem_invitation(&mut self, invitation: &str) -> Answer<String> {
        self.0.redeem_invitation(invitation)
    }
    fn approve_enrollment(
        &mut self,
        host: &str,
        enrollment: &str,
        code: &str,
        rights: &coder_host::access::Rights,
        grant_expires_at: u64,
    ) -> Answer<()> {
        self.0
            .approve_enrollment(host, enrollment, code, rights, grant_expires_at)
    }
    fn deny_enrollment(&mut self, host: &str, enrollment: &str) -> Answer<()> {
        self.0.deny_enrollment(host, enrollment)
    }
    fn connect_ssh(&mut self, destination: &str) -> Answer<()> {
        self.0.connect_ssh(destination)
    }
    fn run_without_local_host(&mut self) -> Answer<()> {
        self.0.run_without_local_host()
    }
    fn refresh_devices(&mut self, host: &str) -> Answer<()> {
        self.0.refresh_devices(host)
    }
    fn create_invitation(
        &mut self,
        host: &str,
        rights: &coder_host::access::Rights,
        grant_expires_at: u64,
    ) -> Answer<coder_computers::CreatedInvitation> {
        self.0.create_invitation(host, rights, grant_expires_at)
    }
    fn cancel_invitation(&mut self, host: &str, invitation: &str) -> Answer<()> {
        self.0.cancel_invitation(host, invitation)
    }
    fn revoke(&mut self, host: &str, device: &str) -> Answer<()> {
        self.0.revoke(host, device)
    }
    fn complete_first_run(&mut self) -> Answer<()> {
        self.0.complete_first_run()
    }
}

/// A relaunch shows the chats list as it was at once, before any computer
/// sends a summary, and live summaries replace it as they arrive.
#[test]
fn a_relaunch_shows_the_last_chats_list_at_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().to_path_buf();
    let mut first = Fixture::in_dir(Synthetic::fixture(Platform::Phone, now), dir);
    let before = first.list();
    let rows = |view: &Value| -> Vec<String> {
        nodes(view)
            .into_iter()
            .filter(|node| node["key"].as_str().is_some_and(|k| k.starts_with("task-")))
            .map(|node| {
                node["element"]["props"]["label"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    };
    let shown = rows(&before);
    assert_eq!(shown.len(), 3, "{shown:?}");
    // The app ends; its store stays.
    let Fixture { _dir: dir, .. } = first;
    assert_eq!(dir.path(), path);
    let mut again = Fixture::in_dir(Relaunched(Synthetic::fixture(Platform::Phone, now)), dir);
    let after = again.list();
    assert!(again.computers.snapshot().activity.is_empty());
    assert_eq!(rows(&after), shown);
    // A cached chat opens like a live one.
    let chat = again.tap(&first_task(&after));
    assert!(node(&chat, "coder-menu").is_some());
}

/// Without a saved list, a first launch has no rows until a host sends
/// summaries.
#[test]
fn a_first_launch_with_no_summaries_has_no_rows() {
    let mut fixture = Fixture::new(Relaunched(Synthetic::fixture(Platform::Phone, now)));
    let view = fixture.list();
    assert!(!keys(&view).iter().any(|key| key.starts_with("task-")));
}

/// What a [`Scripted`] service shows and records, shared with the test.
#[derive(Default)]
struct Script {
    /// The open task's newest summary: its phase and attention.
    phase: Option<(
        nostr::activity_summary::Phase,
        nostr::activity_summary::Attention,
    )>,
    /// The computer cannot be reached.
    offline: bool,
    /// Commands the computer answered: action and text.
    commands: Vec<(coder_host::CommandAction, String, bool)>,
    /// Queue edits, by action name.
    edits: Vec<String>,
    nudges: usize,
}

/// The fixture's hosts with the first task's summary set by the test, and
/// the task commands, queue edits, and nudges recorded.
struct Scripted {
    inner: Synthetic,
    script: std::sync::Arc<std::sync::Mutex<Script>>,
}

impl ComputersService for Scripted {
    fn snapshot(&mut self) -> Answer<coder_computers::Snapshot> {
        let mut snapshot = self.inner.snapshot()?;
        // One more task, on the computer that is online, newest of all.
        if let Some((phase, attention)) = self.script.lock().unwrap().phase
            && let Some(mut summary) = snapshot
                .activity
                .iter()
                .find(|s| s.subject_kind == nostr::activity_summary::SubjectKind::Task)
                .cloned()
        {
            summary.host = snapshot.hosts[0].key.clone();
            summary.subject = "f".repeat(64);
            summary.sequence = 7;
            summary.headline = "Scripted task".into();
            summary.updated_at = NOW + 1;
            summary.phase = phase;
            summary.attention = attention;
            snapshot.activity.push(summary);
        }
        Ok(snapshot)
    }
    fn command_task(&mut self, host: &str, command: &coder_host::TaskCommand) -> Answer<()> {
        let mut script = self.script.lock().unwrap();
        if script.offline {
            return Err(coder_host::access::Error::new(
                coder_host::Code::Transport,
                "not connected",
            ));
        }
        script
            .commands
            .push((command.action, command.text.clone(), command.emulate));
        drop(script);
        self.inner.command_task(host, command)
    }
    fn queue_task(
        &mut self,
        host: &str,
        task: &str,
        edit: &coder_host::QueueEdit,
    ) -> Answer<coder_host::TaskQueue> {
        let name = serde_json::to_value(edit).unwrap()["action"]
            .as_str()
            .unwrap()
            .to_owned();
        self.script.lock().unwrap().edits.push(name);
        self.inner.queue_task(host, task, edit)
    }
    fn nudge_host(&mut self, host: &str) -> Answer<()> {
        self.script.lock().unwrap().nudges += 1;
        self.inner.nudge_host(host)
    }
    fn set_enabled(&mut self, host: &str, enabled: bool) -> Answer<()> {
        self.inner.set_enabled(host, enabled)
    }
    fn retry_now(&mut self, host: &str) -> Answer<()> {
        self.inner.retry_now(host)
    }
    fn forget(&mut self, host: &str) -> Answer<()> {
        self.inner.forget(host)
    }
    fn redeem_invitation(&mut self, invitation: &str) -> Answer<String> {
        self.inner.redeem_invitation(invitation)
    }
    fn approve_enrollment(
        &mut self,
        host: &str,
        enrollment: &str,
        code: &str,
        rights: &coder_host::access::Rights,
        grant_expires_at: u64,
    ) -> Answer<()> {
        self.inner
            .approve_enrollment(host, enrollment, code, rights, grant_expires_at)
    }
    fn deny_enrollment(&mut self, host: &str, enrollment: &str) -> Answer<()> {
        self.inner.deny_enrollment(host, enrollment)
    }
    fn connect_ssh(&mut self, destination: &str) -> Answer<()> {
        self.inner.connect_ssh(destination)
    }
    fn run_without_local_host(&mut self) -> Answer<()> {
        self.inner.run_without_local_host()
    }
    fn refresh_devices(&mut self, host: &str) -> Answer<()> {
        self.inner.refresh_devices(host)
    }
    fn create_invitation(
        &mut self,
        host: &str,
        rights: &coder_host::access::Rights,
        grant_expires_at: u64,
    ) -> Answer<coder_computers::CreatedInvitation> {
        self.inner.create_invitation(host, rights, grant_expires_at)
    }
    fn cancel_invitation(&mut self, host: &str, invitation: &str) -> Answer<()> {
        self.inner.cancel_invitation(host, invitation)
    }
    fn revoke(&mut self, host: &str, device: &str) -> Answer<()> {
        self.inner.revoke(host, device)
    }
    fn complete_first_run(&mut self) -> Answer<()> {
        self.inner.complete_first_run()
    }
}

impl Fixture {
    /// The fixture with a scripted first task, opened.
    fn scripted(
        phase: nostr::activity_summary::Phase,
        attention: nostr::activity_summary::Attention,
    ) -> (Self, std::sync::Arc<std::sync::Mutex<Script>>, Value) {
        let script = std::sync::Arc::new(std::sync::Mutex::new(Script {
            phase: Some((phase, attention)),
            ..Script::default()
        }));
        let mut fixture = Self::new(Scripted {
            inner: Synthetic::fixture(Platform::Phone, now),
            script: script.clone(),
        });
        fixture.computers.refresh().expect("refresh");
        let list = fixture.list();
        let chat = fixture.tap(&first_task(&list));
        (fixture, script, chat)
    }

    /// Send `text` with the composer's token, or with one of its choices'.
    fn send(&mut self, view: &Value, choice: Option<&str>, text: &str) -> Value {
        let props = &node(view, "coder-composer").expect("composer")["element"]["props"];
        let token = match choice {
            None => props["token"].as_str().unwrap().to_owned(),
            Some(label) => props["choices"]
                .as_array()
                .expect("choices")
                .iter()
                .find(|choice| choice["label"] == label)
                .unwrap_or_else(|| panic!("no choice {label}: {props}"))["token"]
                .as_str()
                .unwrap()
                .to_owned(),
        };
        self.coder
            .submit(&token, text, Some(&mut self.computers), &mut self.chats);
        self.coder.flush(Some(&mut self.computers));
        self.render()
    }
}

fn choice_labels(view: &Value) -> Vec<String> {
    node(view, "coder-composer").expect("composer")["element"]["props"]["choices"]
        .as_array()
        .map(|choices| {
            choices
                .iter()
                .map(|choice| choice["label"].as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_long_press_offers_queue_or_stop_and_send_while_coder_works() {
    use coder_host::CommandAction;
    use nostr::activity_summary::{Attention, Phase};
    let (mut fixture, script, chat) = Fixture::scripted(Phase::Running, Attention::None);
    assert_eq!(
        choice_labels(&chat),
        ["Queue for next turn", "Stop and send"]
    );
    // The Rust-drawn steer switch is gone; Stop stays.
    assert!(node(&chat, "coder-steer").is_none());
    assert!(node(&chat, "coder-stop").is_some());
    let chat = fixture.send(&chat, None, "Then the changelog.");
    let chat = fixture.send(&chat, Some("Stop and send"), "Only the parser.");
    fixture.send(&chat, Some("Queue for next turn"), "And the docs.");
    assert_eq!(
        script.lock().unwrap().commands,
        [
            (CommandAction::Queue, "Then the changelog.".into(), false),
            (CommandAction::Steer, "Only the parser.".into(), true),
            (CommandAction::Queue, "And the docs.".into(), false),
        ]
    );
    // A turn that has not started takes new instructions instead.
    let (_, _, queued) = Fixture::scripted(Phase::Queued, Attention::None);
    assert_eq!(choice_labels(&queued), ["Queue for next turn", "Steer now"]);
    // A finished chat sends a follow-up and offers nothing else.
    let (_, _, done) = Fixture::scripted(Phase::Completed, Attention::Completed);
    assert!(choice_labels(&done).is_empty());
}

#[test]
fn a_question_is_answered_and_an_approval_has_its_own_controls() {
    use coder_host::CommandAction;
    use nostr::activity_summary::{Attention, Phase};
    let (mut fixture, script, chat) = Fixture::scripted(Phase::Waiting, Attention::Input);
    assert!(texts(&chat).contains(&"Coder is waiting for your answer.".to_owned()));
    assert!(node(&chat, "coder-stop").is_none());
    assert!(choice_labels(&chat).is_empty());
    let placeholder = &node(&chat, "coder-composer").unwrap()["element"]["props"]["placeholder"];
    assert_eq!(placeholder, "Answer Coder");
    fixture.send(&chat, None, "Two lines.");
    assert_eq!(
        script.lock().unwrap().commands,
        [(CommandAction::Answer, "Two lines.".into(), false)]
    );
    let (mut fixture, script, chat) = Fixture::scripted(Phase::Waiting, Attention::Approval);
    assert!(texts(&chat).contains(&"Coder is waiting for your approval.".to_owned()));
    fixture.tap("coder-approve");
    fixture.tap("coder-deny");
    assert_eq!(
        script.lock().unwrap().commands,
        [
            (CommandAction::Answer, "Approved.".into(), false),
            (CommandAction::Answer, "Denied.".into(), false),
        ]
    );
}

#[test]
fn the_queue_panel_edits_reorders_and_removes_under_the_lease() {
    use nostr::activity_summary::{Attention, Phase};
    let (mut fixture, script, chat) = Fixture::scripted(Phase::Running, Attention::None);
    let chat = fixture.send(&chat, None, "First queued.");
    let chat = fixture.send(&chat, None, "Second queued.");
    // The queue shows once the computer lists it.
    let chat = fixture.tap(
        &keys(&chat)
            .into_iter()
            .find(|key| key == "coder-edit-queue")
            .unwrap_or_else(|| panic!("no Edit queue in {:?}", texts(&chat))),
    );
    let rows: Vec<String> = keys(&chat)
        .into_iter()
        .filter(|key| key.starts_with("coder-queued-row-"))
        .collect();
    assert_eq!(rows.len(), 2, "{:?}", texts(&chat));
    assert!(texts(&chat).contains(&"First queued.".to_owned()));
    // Move the second up.
    let up = keys(&chat)
        .into_iter()
        .find(|key| key.starts_with("coder-queued-up-"))
        .expect("move up");
    let chat = fixture.tap(&up);
    let queued: Vec<String> = texts(&chat)
        .into_iter()
        .filter(|text| text.ends_with("queued."))
        .collect();
    assert_eq!(queued, ["Second queued.", "First queued."]);
    // Edit one: its text becomes the composer's draft, and a send edits it.
    let edit = keys(&chat)
        .into_iter()
        .find(|key| key.starts_with("coder-queued-edit-"))
        .expect("edit");
    let chat = fixture.tap(&edit);
    let props = &node(&chat, "coder-composer").unwrap()["element"]["props"];
    assert_eq!(props["draft"], "Second queued.");
    assert_eq!(props["placeholder"], "Edit your queued message");
    assert!(choice_labels(&chat).is_empty());
    let chat = fixture.send(&chat, None, "Second, edited.");
    assert!(texts(&chat).contains(&"Second, edited.".to_owned()));
    let remove = keys(&chat)
        .into_iter()
        .find(|key| key.starts_with("coder-queued-remove-"))
        .expect("remove");
    let chat = fixture.tap(&remove);
    assert_eq!(
        keys(&chat)
            .into_iter()
            .filter(|key| key.starts_with("coder-queued-row-"))
            .count(),
        1
    );
    let chat = fixture.tap("coder-queue-done");
    assert!(node(&chat, "coder-queue-title").is_none());
    let edits = script.lock().unwrap().edits.clone();
    assert_eq!(edits.first().map(String::as_str), Some("list"));
    for action in ["lease", "reorder", "edit", "remove", "release"] {
        assert!(edits.iter().any(|edit| edit == action), "{edits:?}");
    }
    // Each queued message was sent once, however often the panel moved.
    assert_eq!(script.lock().unwrap().commands.len(), 2);
}

#[test]
fn an_unreached_computer_is_nudged_and_the_command_waits() {
    use nostr::activity_summary::{Attention, Phase};
    let (mut fixture, script, chat) = Fixture::scripted(Phase::Completed, Attention::Completed);
    script.lock().unwrap().offline = true;
    let chat = fixture.send(&chat, None, "Follow up.");
    assert!(
        texts(&chat)
            .iter()
            .any(|text| text.starts_with("1 message waiting to reach")),
        "{:?}",
        texts(&chat)
    );
    assert_eq!(script.lock().unwrap().nudges, 1);
    assert!(script.lock().unwrap().commands.is_empty());
    // The command waits for its backoff or the computer's fresh presence
    // (the outbox's own test covers both); a second failure in the same
    // window nudges no more.
    script.lock().unwrap().offline = false;
    fixture.coder.flush(Some(&mut fixture.computers));
    assert!(script.lock().unwrap().commands.is_empty());
    assert_eq!(script.lock().unwrap().nudges, 1);
}

/// A message shows in its chat the moment it is sent, marked while it waits
/// for the computer or in the computer's queue, until the transcript shows
/// it; a refusal takes it away.
#[test]
fn a_sent_message_shows_in_the_chat_at_once() {
    use nostr::activity_summary::{Attention, Phase};
    let notes = |view: &Value| -> Vec<(String, String)> {
        nodes(view)
            .into_iter()
            .filter(|node| {
                node["key"]
                    .as_str()
                    .is_some_and(|key| key.starts_with("coder-sent-") && !key.ends_with("-md"))
            })
            .map(|node| {
                let props = &node["element"]["props"];
                let text =
                    props["children"][0]["element"]["props"]["blocks"][0]["spans"][0]["text"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned();
                (text, props["note"].as_str().unwrap_or_default().to_owned())
            })
            .collect()
    };
    // Sent while the computer is out of reach: it shows, waiting.
    let (mut fixture, script, chat) = Fixture::scripted(Phase::Completed, Attention::Completed);
    script.lock().unwrap().offline = true;
    let chat = fixture.send(&chat, None, "Follow up.");
    assert_eq!(
        notes(&chat),
        [("Follow up.".into(), "Sending".into())],
        "{:?}",
        keys(&chat)
    );
    // Once the computer answers, it shows as sent.
    script.lock().unwrap().offline = false;
    fixture.coder.flush(Some(&mut fixture.computers));
    let chat = fixture.render();
    assert!(notes(&chat).iter().all(|(text, _)| text == "Follow up."));
    // Queued while Coder works: it shows as queued.
    let (mut fixture, _, chat) = Fixture::scripted(Phase::Running, Attention::None);
    let chat = fixture.send(&chat, None, "And the docs.");
    assert_eq!(notes(&chat), [("And the docs.".into(), "Queued".into())]);
    // A stop is not a message.
    let chat = fixture.tap("coder-stop");
    assert_eq!(notes(&chat).len(), 1);
}

#[test]
fn a_pulled_chat_publishes_its_rows_for_the_layout_instead_of_listing_them() {
    let mut listed = Fixture::hosts();
    let task = first_task(&listed.list());
    let listed = listed.tap(&task);
    let rows: Vec<String> =
        node(&listed, "coder-transcript").expect("transcript")["element"]["props"]["children"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|row| row["key"].as_str().expect("key").to_owned())
            .collect();
    assert!(!rows.is_empty());

    let mut pulled = Fixture::hosts();
    pulled.coder = CoderTab::new("coder:pulled".into()).with_pulled_transcripts(true);
    pulled.list();
    let view = pulled.tap(&task);
    let props = &node(&view, "coder-transcript").expect("transcript")["element"]["props"];
    assert_eq!(props["children"], serde_json::json!([]));
    assert_eq!(props["source"], "coder:pulled:coder-transcript");
    let snapshot =
        rust_native::layout::source::get("coder:pulled:coder-transcript").expect("published");
    assert_eq!(snapshot.keys().collect::<Vec<_>>(), rows);

    // Out to a new chat, the chat's source is retired.
    pulled.tap("coder-new");
    assert!(rust_native::layout::source::get("coder:pulled:coder-transcript").is_none());
}

/// A basic Coder the test answers by hand: each question's reply waits in
/// `replies` for the test to stream into and end.
/// Each question the test's basic Coder was asked, with its reply.
type Asked = Vec<(
    Vec<String>,
    std::sync::Arc<std::sync::Mutex<crate::basic_coder::Reply>>,
)>;

#[derive(Clone, Default)]
struct Hand {
    replies: std::sync::Arc<std::sync::Mutex<Asked>>,
    /// The router context each question carried.
    contexts: std::sync::Arc<std::sync::Mutex<Vec<crate::router::Context>>>,
}

impl crate::basic_coder::Door for Hand {
    fn ask(
        &self,
        turns: Vec<crate::basic_coder::Turn>,
        context: crate::router::Context,
        reply: std::sync::Arc<std::sync::Mutex<crate::basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let texts = turns.into_iter().map(|turn| turn.text).collect();
        self.replies.lock().unwrap().push((texts, reply));
        self.contexts.lock().unwrap().push(context);
        Box::pin(async {})
    }
}

impl Hand {
    /// Stream `text` into the newest reply, ending it when `done`.
    fn say(&self, text: &str, done: bool) {
        let replies = self.replies.lock().unwrap();
        let (_, reply) = replies.last().expect("a question");
        let mut reply = reply.lock().unwrap();
        reply.text = text.into();
        reply.done = done;
    }

    /// The router's reading of the newest question: the judgment, offers,
    /// and result fields, as the reply would hold them from the wire.
    fn route(&self, payloads: &[serde_json::Value]) {
        let replies = self.replies.lock().unwrap();
        let (_, reply) = replies.last().expect("a question");
        let mut reply = reply.lock().unwrap();
        for payload in payloads {
            match payload["type"].as_str() {
                Some("judgment") => reply.meta.judged(payload),
                Some("offer") => reply.meta.offered(payload),
                Some("result") => reply.meta.resulted(payload),
                _ => {}
            }
        }
    }

    /// The worker's judgment of the newest question.
    fn judge(&self, lane: crate::basic_coder::Lane) {
        let replies = self.replies.lock().unwrap();
        let (_, reply) = replies.last().expect("a question");
        reply.lock().unwrap().lane = Some(lane);
    }

    fn asked(&self) -> Vec<Vec<String>> {
        self.replies
            .lock()
            .unwrap()
            .iter()
            .map(|(texts, _)| texts.clone())
            .collect()
    }
}

impl Fixture {
    /// The fixture with a basic Coder answered by `hand`.
    fn answered_by(mut self, hand: &Hand) -> Self {
        let basic = crate::basic_chats::BasicChats::new(
            Some(self._runtime.handle().clone()),
            Some(std::sync::Arc::new(hand.clone())),
            None,
        );
        self.coder = std::mem::replace(&mut self.coder, CoderTab::new("coder:test".into()))
            .with_basic(basic);
        self
    }

    /// Send `text` from the current view's composer.
    fn say(&mut self, text: &str) -> Value {
        let view = self.render();
        let composer = nodes(&view)
            .into_iter()
            .find(|node| node["element"]["kind"] == "composer")
            .expect("composer");
        let token = composer["element"]["props"]["token"].as_str().unwrap();
        self.coder
            .submit(token, text, Some(&mut self.computers), &mut self.chats);
        self._runtime.block_on(tokio::task::yield_now());
        self.render()
    }
}

/// Every host of the fixture removed: a phone with no computer.
struct NoComputers(Synthetic);

impl ComputersService for NoComputers {
    fn snapshot(&mut self) -> Answer<coder_computers::Snapshot> {
        let mut snapshot = self.0.snapshot()?;
        snapshot.hosts.clear();
        snapshot.activity.clear();
        Ok(snapshot)
    }
    fn set_enabled(&mut self, host: &str, enabled: bool) -> Answer<()> {
        self.0.set_enabled(host, enabled)
    }
    fn retry_now(&mut self, host: &str) -> Answer<()> {
        self.0.retry_now(host)
    }
    fn forget(&mut self, host: &str) -> Answer<()> {
        self.0.forget(host)
    }
    fn redeem_invitation(&mut self, invitation: &str) -> Answer<String> {
        self.0.redeem_invitation(invitation)
    }
    fn approve_enrollment(
        &mut self,
        host: &str,
        enrollment: &str,
        code: &str,
        rights: &coder_host::access::Rights,
        grant_expires_at: u64,
    ) -> Answer<()> {
        self.0
            .approve_enrollment(host, enrollment, code, rights, grant_expires_at)
    }
    fn deny_enrollment(&mut self, host: &str, enrollment: &str) -> Answer<()> {
        self.0.deny_enrollment(host, enrollment)
    }
    fn connect_ssh(&mut self, destination: &str) -> Answer<()> {
        self.0.connect_ssh(destination)
    }
    fn run_without_local_host(&mut self) -> Answer<()> {
        self.0.run_without_local_host()
    }
    fn refresh_devices(&mut self, host: &str) -> Answer<()> {
        self.0.refresh_devices(host)
    }
    fn create_invitation(
        &mut self,
        host: &str,
        rights: &coder_host::access::Rights,
        grant_expires_at: u64,
    ) -> Answer<coder_computers::CreatedInvitation> {
        self.0.create_invitation(host, rights, grant_expires_at)
    }
    fn cancel_invitation(&mut self, host: &str, invitation: &str) -> Answer<()> {
        self.0.cancel_invitation(host, invitation)
    }
    fn revoke(&mut self, host: &str, device: &str) -> Answer<()> {
        self.0.revoke(host, device)
    }
    fn complete_first_run(&mut self) -> Answer<()> {
        self.0.complete_first_run()
    }
}

fn composer_of(view: &Value) -> Value {
    nodes(view)
        .into_iter()
        .find(|node| node["element"]["kind"] == "composer")
        .expect("composer")["element"]["props"]
        .clone()
}

/// A fresh install with no computer chats at once: the first message
/// starts a conversation with the basic Coder, its reply streams in as
/// Markdown, and the chat offers to connect a computer for Coder.
#[test]
fn a_first_chat_needs_no_computer_and_streams_its_reply() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    let list = fixture.list();
    assert!(texts(&list).contains(&"Chats".to_owned()));
    assert!(texts(&list).contains(&"No chats yet.".to_owned()));
    // The tab opens on a new chat with OpenAgents: no selector, no welcome
    // line, and no way to connect a computer above the field (that comes
    // only as an offer under a reply that needs one).
    let screen = fixture.tap("coder-back");
    assert!(node(&screen, "coder-target").is_none());
    assert!(node(&screen, "coder-welcome").is_none());
    assert!(node(&screen, "coder-profile").is_none());
    assert!(node(&screen, "coder-continue-0").is_none());
    assert!(node(&screen, "coder-connect").is_none());
    // A first chat's starter questions are the only suggestions.
    assert!(node(&screen, "coder-first-0").is_some());
    assert_eq!(composer_of(&screen)["enabled"], true);
    assert_eq!(composer_of(&screen)["placeholder"], "Message OpenAgents");

    let chat = fixture.say("What is a **relay**?");
    assert_eq!(hand.asked(), vec![vec!["What is a **relay**?".to_owned()]]);
    // Before the first words: the working row, a spinner.
    assert!(node(&chat, "talk-working").is_some(), "{:?}", keys(&chat));
    assert_eq!(
        node(&chat, "talk-working").unwrap()["element"]["props"]["label"],
        "Working…"
    );
    assert_eq!(composer_of(&chat)["busy"], true);
    assert!(fixture.coder.streaming());
    assert!(fixture.coder.live(Some(&fixture.computers)));

    // Half-written Markdown shows styled while it streams.
    hand.say("A relay **stores", false);
    let chat = fixture.render();
    let streamed = node(&chat, "talk-m1-md").expect("streaming reply");
    let blocks = serde_json::to_string(&streamed["element"]["props"]["blocks"]).unwrap();
    assert!(blocks.contains("\"bold\":true"), "{blocks}");
    // The first part of a reply is not all of it: the working row stays
    // under it until the result.
    let rows = &node(&chat, "coder-transcript").unwrap()["element"]["props"]["children"];
    let order: Vec<&str> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["key"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["talk-m0", "talk-m1", "talk-working"]);

    hand.say("A relay **stores** and forwards events.", true);
    let chat = fixture.render();
    assert!(!fixture.coder.streaming());
    assert_eq!(composer_of(&chat)["busy"], false);
    assert!(node(&chat, "talk-m1").is_some());
    assert!(node(&chat, "talk-working").is_none());

    // No computer and no judgment that it needs one: nothing but the chat.
    assert!(node(&chat, "coder-connect").is_none());
    // The worker judged the message needs a computer: the chat offers to
    // connect one for Coder.
    fixture.say("Run the tests in my repo");
    hand.judge(crate::basic_coder::Lane::Computer);
    hand.say("That needs a computer.", true);
    let chat = fixture.render();
    let connect = node(&chat, "coder-connect").expect("connect a computer");
    assert_eq!(connect["element"]["props"]["label"], "Connect a computer");
    fixture.tap("coder-connect");
    assert_eq!(
        fixture.coder.take_go(),
        Some(crate::coder_tab::Go::Computers)
    );
    assert_eq!(fixture.coder.take_go(), None);

    // A follow-up carries the conversation.
    fixture.say("And a worker?");
    assert_eq!(hand.asked()[2].len(), 5);

    let list = fixture.list();
    let row = keys(&list)
        .into_iter()
        .find(|key| key.starts_with("talk-"))
        .expect("the conversation in the list");
    let label = node(&list, &row).unwrap()["element"]["props"]["label"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        label.starts_with("What is a **relay**?\nOpenAgents · "),
        "{label}"
    );
}

/// A failed reply says why and offers to try again; stopping keeps what
/// streamed.
#[test]
fn a_failed_reply_offers_to_try_again() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.say("hi");
    {
        let replies = hand.replies.lock().unwrap();
        replies[0].1.lock().unwrap().failure = Some(crate::basic_coder::Failure::Refused {
            code: "rate_limited".into(),
            message: "slow".into(),
            retry_after_ms: Some(9_000),
        });
    }
    let chat = fixture.render();
    assert!(
        texts(&chat)
            .contains(&"You're sending messages quickly. Try again in 9 seconds.".to_owned())
    );
    let chat = fixture.tap("talk-retry");
    assert_eq!(hand.asked().len(), 2);
    assert!(node(&chat, "talk-retry").is_none());
    hand.say("Hel", false);
    fixture.render();
    let chat = fixture.tap("coder-composer");
    // Stop kept the partial reply as the answer.
    assert!(!fixture.coder.streaming());
    assert!(node(&chat, "talk-m1").is_some(), "{:?}", keys(&chat));
}

/// From a conversation, Coder runs on the connected computer: a task there
/// that starts from the conversation, which opens, and which the
/// conversation remembers.
#[test]
fn run_coder_starts_a_task_with_the_conversation() {
    let hand = Hand::default();
    let mut fixture = Fixture::hosts().answered_by(&hand);
    // A computer is ready, but a reply that does not need it shows no way
    // to run Coder: no standing button above the field.
    fixture.say("What is a relay?");
    hand.say("A relay stores and forwards events.", true);
    let chat = fixture.render();
    assert!(node(&chat, "coder-run").is_none(), "{:?}", keys(&chat));
    assert!(node(&chat, "coder-agents").is_none(), "{:?}", keys(&chat));
    // Nor while a reply that will need it still streams.
    fixture.tap("coder-new");
    fixture.say("Run the tests in my repo");
    hand.say("That needs a computer.", false);
    let chat = fixture.render();
    assert!(node(&chat, "coder-run").is_none(), "{:?}", keys(&chat));
    // The worker's judgment placed the message on a computer: Run Coder is
    // a chip under that reply.
    hand.judge(crate::basic_coder::Lane::Computer);
    hand.say("That needs a computer.", true);
    let chat = fixture.render();
    let run = &node(&chat, "coder-run").expect("run coder")["element"]["props"];
    assert_eq!(run["label"], "Run Coder on Studio Mac");
    assert_eq!(run["icon"]["glyph"], "computer");
    assert_eq!(run["icon"]["pill"], true);
    let opened = fixture.tap("coder-run");
    let (host, task) = fixture.coder.open_task().expect("the task's chat opens");
    assert_eq!(
        fixture.computers.snapshot().host(&host).unwrap().label,
        "Studio Mac"
    );
    // The chat shows what was sent until the computer's transcript does:
    // one line naming the chat, never the conversation pasted back.
    let sent = serde_json::to_string(&node(&opened, "coder-transcript").unwrap()).unwrap();
    assert!(
        sent.contains("Continued from the OpenAgents app: Run the tests in my repo"),
        "{sent}"
    );
    assert!(!sent.contains("Continue this conversation"), "{sent}");
    assert!(!sent.contains("Coder: That needs a computer"), "{sent}");
    // In the previous chats, the task's row carries the conversation's
    // title, and the conversation says where Coder runs.
    let list = fixture.list();
    let labels: Vec<String> = texts(&list);
    assert!(
        labels
            .iter()
            .any(|label| label
                .starts_with("Run the tests in my repo\nCoder · running on Studio Mac")),
        "{labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|label| label.starts_with("Run the tests in my repo\nQueued · Studio Mac")),
        "{labels:?}"
    );
    // Back in the conversation, no Open Coder button stands above the
    // field; the task is in the previous chats.
    let talk = keys(&list)
        .into_iter()
        .find(|key| key.starts_with("talk-"))
        .unwrap();
    let chat = fixture.tap(&talk);
    assert!(node(&chat, "coder-spawned").is_none(), "{:?}", keys(&chat));
    let list = fixture.list();
    let row = keys(&list)
        .into_iter()
        .find(|key| key.starts_with("task-"))
        .expect("the task in the list");
    fixture.tap(&row);
    assert_eq!(fixture.coder.open_task(), Some((host, task)));
}

/// Every basic turn asks for the chat router with a context that says only
/// whether a computer is ready and which build this is: never a computer's
/// name or workspace.
#[test]
fn a_turn_asks_for_routing_with_a_bounded_context() {
    let hand = Hand::default();
    let mut fixture = Fixture::hosts().answered_by(&hand);
    fixture.coder = std::mem::replace(&mut fixture.coder, CoderTab::new("x".into()))
        .with_app_build(Some("1.0.0 (19)".into()));
    fixture.say("Who are you?");
    let contexts = hand.contexts.lock().unwrap().clone();
    assert_eq!(
        contexts,
        [crate::router::Context {
            computer_ready: true,
            app_build: Some("1.0.0 (19)".into()),
            ..crate::router::Context::default()
        }]
    );
    let wire = contexts[0].json().to_string();
    assert!(!wire.contains("Studio Mac"), "{wire}");

    let bare = Hand::default();
    let mut none =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&bare);
    none.say("Who are you?");
    assert!(!bare.contexts.lock().unwrap()[0].computer_ready);
}

/// The canned answer the router chose, as the worker sends it.
fn canned(hand: &Hand, text: &str) {
    hand.route(&[
        json!({"v": 2, "type": "judgment", "verdict": "respond", "line": text,
            "set": "chat-router-v1", "tier": "canned", "answer": "meta.model@1",
            "answer_p": 0.93, "route": "meta", "lane": "chat"}),
        json!({"v": 2, "type": "result", "text": text, "model": "bank:chat-answers-v1",
            "tier": "canned", "answer": "meta.model@1", "route": "meta",
            "bank": "chat-answers-v1@9f2c",
            "followups": [{"id": "meta.privacy", "label": "Is this chat private?"},
                {"id": "meta.coder", "label": "What is Coder?"}]}),
    ]);
    hand.say(text, true);
}

/// A prepared answer offers its follow-ups as chips that send their
/// words; it carries no note and no report control.
#[test]
fn a_prepared_answer_offers_followups() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.say("What model are you?");
    canned(&hand, "Our chat runs on Gemini 3.8 Flash.");
    let chat = fixture.render();
    assert!(node(&chat, "talk-m1").unwrap()["element"]["props"]["note"].is_null());
    assert!(node(&chat, "talk-m0").unwrap()["element"]["props"]["note"].is_null());
    assert!(node(&chat, "coder-wrong").is_none());
    let chip = &node(&chat, "coder-followup-1").expect("follow-up")["element"]["props"];
    assert_eq!(chip["label"], "What is Coder?");
    assert_eq!(chip["icon"]["glyph"], "ask");

    // A follow-up chip sends its words as the next message.
    fixture.tap("coder-followup-0");
    assert_eq!(
        hand.asked()[1],
        [
            "What model are you?",
            "Our chat runs on Gemini 3.8 Flash.",
            "Is this chat private?"
        ]
    );
    // The model's own reply offers no follow-ups.
    hand.say("It is encrypted.", true);
    let chat = fixture.render();
    assert!(node(&chat, "talk-m3").unwrap()["element"]["props"]["note"].is_null());
    assert!(node(&chat, "coder-followup-0").is_none());
    // Share this chat carries the whole conversation, with its judgments.
    let shared = fixture.coder.shared_chat().expect("shared");
    assert_eq!(shared.reason, playtest::report::ShareReason::Shared);
    assert_eq!(shared.turns.len(), 4);
    assert!(shared.turns[3].answer.is_none());
}

/// The router's offers become the phone's own controls: dispatching Coder
/// is the Run Coder chip or, with no computer, Connect a computer; a
/// screen offer opens that screen; a read-only command is a card that runs
/// only when tapped. A screen or command outside the phone's tables never
/// shows.
#[test]
fn offers_become_the_phones_own_controls() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.say("Fix the flaky test in my repo");
    hand.route(&[
        json!({"v": 2, "type": "offer", "offer": "run_coder", "target": "connected_computer",
            "label": "Run Coder"}),
        json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "account.computers",
            "label": "Connect a computer"}),
        json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "settings.erase",
            "label": "Erase"}),
    ]);
    hand.say("We'll dispatch Coder to fix the flaky test.", true);
    let chat = fixture.render();
    assert_eq!(
        node(&chat, "coder-connect").expect("connect")["element"]["props"]["label"],
        "Connect a computer"
    );
    // One way to connect, not two; no unknown screen.
    assert!(
        !keys(&chat)
            .iter()
            .any(|key| key.starts_with("coder-screen-")),
        "{:?}",
        keys(&chat)
    );
    fixture.tap("coder-connect");
    assert_eq!(
        fixture.coder.take_go(),
        Some(crate::coder_tab::Go::Computers)
    );

    fixture.say("How do I back up my wallet?");
    hand.route(&[
        json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "wallet",
        "label": "Send all funds"}),
    ]);
    hand.say("Your recovery words are in the Wallet tab.", true);
    let chat = fixture.render();
    let chip = &node(&chat, "coder-screen-0").expect("wallet chip")["element"]["props"];
    assert_eq!(chip["label"], "Open Wallet");
    assert_eq!(chip["icon"]["glyph"], "wallet");
    fixture.tap("coder-screen-0");
    assert_eq!(fixture.coder.take_go(), Some(crate::coder_tab::Go::Wallet));

    // A result card's See the board opens the Gym in the Verse, in the
    // phone's own words.
    fixture.say("Who else tested Code finder?");
    hand.route(&[
        json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "verse.gym",
        "label": "Walk to the Gym"}),
    ]);
    hand.say("Two trainers published results on that test set.", true);
    let chat = fixture.render();
    let chip = &node(&chat, "coder-screen-0").expect("board chip")["element"]["props"];
    assert_eq!(chip["label"], "See the board");
    fixture.tap("coder-screen-0");
    assert_eq!(
        fixture.coder.take_go(),
        Some(crate::coder_tab::Go::VerseGym)
    );
}

#[test]
fn a_read_only_command_runs_only_when_tapped() {
    let hand = Hand::default();
    let mut fixture = Fixture::hosts().answered_by(&hand);
    fixture.say("Which of my computers are online?");
    hand.route(&[
        json!({"v": 2, "type": "offer", "offer": "cli", "argv": ["computer", "list"],
            "effect": "read_only", "runs_on": "this_device", "confirm": true}),
        json!({"v": 2, "type": "offer", "offer": "cli", "argv": ["verse", "xp"],
            "effect": "read_only", "runs_on": "connected_computer", "confirm": true}),
        json!({"v": 2, "type": "offer", "offer": "cli", "argv": ["wallet", "pay", "lnbc1"],
            "effect": "read_only", "runs_on": "this_device", "confirm": true}),
    ]);
    hand.say("We can list them.", true);
    let chat = fixture.render();
    assert_eq!(
        node(&chat, "coder-cli-0-command").unwrap()["element"]["props"]["value"],
        "openagents computer list"
    );
    assert_eq!(
        node(&chat, "coder-cli-0-where").unwrap()["element"]["props"]["value"],
        "Reads only. Runs on this phone."
    );
    // The wallet payment never made it past the phone's table.
    assert!(node(&chat, "coder-cli-2").is_none());
    // Nothing ran yet.
    assert!(node(&chat, "coder-cli-0-out-0").is_none());
    let ran = fixture.tap("coder-cli-0-run");
    let out = node(&ran, "coder-cli-0-out-0").expect("output")["element"]["props"]["value"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(out.starts_with("Studio Mac"), "{out}");
    // With no way to reach computers, the card says so plainly.
    let ran = fixture.tap("coder-cli-1-run");
    assert_eq!(
        node(&ran, "coder-cli-1-why").unwrap()["element"]["props"]["value"],
        "This phone can't reach your computers right now."
    );
}

/// What a fake runner was asked: host and command.
type RemoteAsked = std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<String>)>>>;

/// Runs every command by answering `output` after `gate` opens, and
/// records what it was asked to run where.
struct Remote {
    output: &'static str,
    gate: std::sync::Arc<std::sync::Mutex<bool>>,
    asked: RemoteAsked,
}

impl crate::cli_run::RemoteCli for Remote {
    fn run(&self, host: &str, command: &[String]) -> Result<crate::cli_run::RemoteRun, String> {
        self.asked
            .lock()
            .unwrap()
            .push((host.to_owned(), command.to_vec()));
        while !*self.gate.lock().unwrap() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(crate::cli_run::RemoteRun {
            output: self.output.into(),
            exit: 0,
            timed_out: false,
        })
    }
}

/// A command that runs on the computer starts there after the tap, off
/// the UI thread: the card shows it running, then what it printed.
#[test]
fn a_computer_command_runs_there_after_the_tap() {
    let hand = Hand::default();
    let mut fixture = Fixture::hosts().answered_by(&hand);
    let gate = std::sync::Arc::new(std::sync::Mutex::new(false));
    let asked: RemoteAsked = std::sync::Arc::default();
    let remote = Remote {
        output: "{\"xp\": 120}",
        gate: gate.clone(),
        asked: asked.clone(),
    };
    fixture.coder = std::mem::replace(&mut fixture.coder, CoderTab::new("x".into()))
        .with_remote_cli(Some(std::sync::Arc::new(remote)));
    fixture.say("What level am I?");
    hand.route(&[
        json!({"v": 2, "type": "offer", "offer": "cli", "argv": ["verse", "xp"],
            "effect": "read_only", "runs_on": "connected_computer", "confirm": true}),
    ]);
    hand.say("We can check on your computer.", true);
    let chat = fixture.render();
    let place = node(&chat, "coder-cli-0-where").unwrap()["element"]["props"]["value"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(place.starts_with("Reads only. Runs on "), "{place}");
    assert!(
        asked.lock().unwrap().is_empty(),
        "nothing runs before the tap"
    );
    let running = fixture.tap("coder-cli-0-run");
    let label =
        node(&running, "coder-cli-0-running").expect("running")["element"]["props"]["value"]
            .as_str()
            .unwrap()
            .to_owned();
    assert!(label.starts_with("Running on "), "{label}");
    assert!(fixture.coder.live(Some(&fixture.computers)));
    // A second tap while it runs starts nothing more.
    let _ = fixture.tap("coder-cli-0-running");
    *gate.lock().unwrap() = true;
    let mut done = fixture.render();
    for _ in 0..200 {
        if node(&done, "coder-cli-0-out-0").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        done = fixture.render();
    }
    let lines: Vec<String> = (0..3)
        .filter_map(|at| {
            node(&done, &format!("coder-cli-0-out-{at}"))
                .map(|n| n["element"]["props"]["value"].as_str().unwrap().to_owned())
        })
        .collect();
    assert_eq!(lines, ["{", "  \"xp\": 120", "}"]);
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, ["openagents", "--json", "verse", "xp"]);
    assert!(!fixture.coder.live(Some(&fixture.computers)));
}
