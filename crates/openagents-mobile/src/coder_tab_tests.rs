//! The Coder tab against Coder's offline Computers fixture: seven hosts in
//! every status, with activity summaries, and no network.

use crate::chats::Chats;
use crate::coder_list::Store;
use crate::coder_tab::CoderTab;
use coder_computers::cache::Cache;
use coder_computers::synthetic::Synthetic;
use coder_computers::{Capabilities, Computers, ComputersService, Platform};
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
            "chats:test".into(),
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
    let before = first.render();
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
    let after = again.render();
    assert!(again.computers.snapshot().activity.is_empty());
    assert_eq!(rows(&after), shown);
    // A cached chat opens like a live one.
    let chat = again.tap(&first_task(&after));
    assert!(node(&chat, "coder-back").is_some());
}

/// Without a saved list, a first launch has no rows until a host sends
/// summaries.
#[test]
fn a_first_launch_with_no_summaries_has_no_rows() {
    let mut fixture = Fixture::new(Relaunched(Synthetic::fixture(Platform::Phone, now)));
    let view = fixture.render();
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
        let list = fixture.render();
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
