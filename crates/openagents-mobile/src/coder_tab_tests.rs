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

    /// The app's tick: a coding reply that just arrived may start Coder
    /// at once (#10101), then the view.
    fn render(&mut self) -> Value {
        self.coder
            .start_offered(Some(&mut self.computers), &mut self.chats);
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
    /// The task's head as the computer reads it now, if it reviews one.
    head: Option<char>,
    /// The heads publications named, in order.
    published: Vec<String>,
}

fn scripted_review(head: char) -> coder_host::access::review::TaskReview {
    let diff = "diff --git a/src/slug.rs b/src/slug.rs\n--- a/src/slug.rs\n+++ b/src/slug.rs\n@@ -1 +1,2 @@\n-fn slug() {}\n+fn slug() {}\n+fn more() {}\ndiff --git a/notes.md b/notes.md\n--- a/notes.md\n+++ b/notes.md\n@@ -1 +1 @@\n-old\n+new\n";
    coder_host::access::review::TaskReview {
        task: "f".repeat(64),
        base: "1".repeat(40),
        head_commit: "1".repeat(40),
        head: head.to_string().repeat(40),
        files: vec![
            coder_host::access::review::FileCount {
                path: "notes.md".into(),
                status: coder_host::access::review::FileStatus::Modified,
                added: Some(1),
                removed: Some(1),
            },
            coder_host::access::review::FileCount {
                path: "src/slug.rs".into(),
                status: coder_host::access::review::FileStatus::Modified,
                added: Some(2),
                removed: Some(1),
            },
        ],
        files_total: 2,
        added: 3,
        removed: 2,
        uncounted: 0,
        diff: diff.into(),
        completeness: coder_host::access::review::Completeness::Complete,
        publication: None,
    }
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
    fn review_task(
        &mut self,
        _host: &str,
        task: &str,
    ) -> Answer<coder_host::access::review::TaskReview> {
        match self.script.lock().unwrap().head {
            Some(head) if task == "f".repeat(64) => Ok(scripted_review(head)),
            _ => Err(coder_host::access::Error::new(
                coder_host::Code::Unsupported,
                "no change",
            )),
        }
    }
    /// As the host's task owner: a head the worktree moved past is
    /// refused; the current one publishes as a draft pull request.
    fn publish_task(
        &mut self,
        _host: &str,
        task: &str,
        base: &str,
        head_commit: &str,
        head: &str,
    ) -> Answer<coder_host::access::review::Publication> {
        let mut script = self.script.lock().unwrap();
        script.published.push(head.to_owned());
        let current = script.head.map(|head| head.to_string().repeat(40));
        let fresh = current.as_deref() == Some(head);
        Ok(coder_host::access::review::Publication {
            operation: "9".repeat(64),
            task: task.into(),
            base: base.into(),
            head_commit: head_commit.into(),
            head: head.into(),
            landing: coder_host::access::review::Landing::DraftPullRequest,
            state: if fresh {
                coder_host::access::review::PublishState::Published
            } else {
                coder_host::access::review::PublishState::Refused
            },
            branch: fresh.then(|| "coder/review-ffffffff-99999999".into()),
            commit: fresh.then(|| "4".repeat(40)),
            url: fresh.then(|| "https://github.com/example/scratch/pull/7".into()),
            note: if fresh {
                "Pushed 4444444444 and opened a draft pull request.".into()
            } else {
                "Nothing was published: the change moved since it was reviewed. Refresh and \
                 review it again."
                    .into()
            },
        })
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
    // A chat with rows: a running task's shows Coder working.
    let (task, rows) = keys(&Fixture::hosts().list())
        .into_iter()
        .filter(|key| key.starts_with("task-") && key.matches('-').count() == 1)
        .find_map(|task| {
            let mut listed = Fixture::hosts();
            listed.list();
            let listed = listed.tap(&task);
            let rows: Vec<String> = node(&listed, "coder-transcript").expect("transcript")
                ["element"]["props"]["children"]
                .as_array()
                .expect("rows")
                .iter()
                .map(|row| row["key"].as_str().expect("key").to_owned())
                .collect();
            (!rows.is_empty()).then_some((task, rows))
        })
        .expect("a chat with rows");

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
        let mut reply = reply.lock().unwrap();
        reply.lane = Some(lane);
        // The worker's judgment names a route with its lane; a computer
        // reading that offers Coder is a dispatch (#10079).
        if lane == crate::basic_coder::Lane::Computer {
            reply.meta.route = Some(openagents_chat::delegation::DISPATCH_ROUTE.into());
        }
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

    /// The fixture with a basic Coder answered by `hand`, its chats and
    /// used suggestions kept in `dir` as the app's encrypted store keeps
    /// them, so a second fixture on the same `dir` is a relaunch.
    fn answered_and_kept(mut self, hand: &Hand, dir: &std::path::Path) -> Self {
        let secret = secp256k1::SecretKey::from_byte_array([0x11; 32]).expect("key");
        let basic = crate::basic_chats::BasicChats::new(
            Some(self._runtime.handle().clone()),
            Some(std::sync::Arc::new(hand.clone())),
            Cache::open(dir, &secret).ok(),
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
    // Suggested questions are the only chips above the field.
    assert!(node(&screen, "coder-suggest-meta.who").is_some());
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
    assert_eq!(fixture.coder.take_go(), Some(crate::coder_tab::Go::Connect));
    assert_eq!(fixture.coder.take_go(), None);

    // A follow-up carries the conversation.
    fixture.say("And a worker?");
    assert_eq!(hand.asked()[2].len(), 5);

    let list = fixture.list();
    // The card wraps the row with its menu; the row itself opens the chat.
    let row = keys(&list)
        .into_iter()
        .find(|key| key.starts_with("talk-") && !key.ends_with("-card"))
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
    // An old worker's rate refusal never reads as a limit (#10120).
    assert!(texts(&chat).contains(&"Couldn't reach OpenAgents; try again.".to_owned()));
    assert!(
        !texts(&chat)
            .iter()
            .any(|text| text.to_lowercase().contains("limit") || text.contains("Try again in"))
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
    // the message that asked for the work (#10073), once, with where it
    // came from as its note (#10076), never the conversation pasted back.
    let sent = serde_json::to_string(&node(&opened, "coder-transcript").unwrap()).unwrap();
    assert!(sent.contains("Run the tests in my repo"), "{sent}");
    assert!(
        sent.contains("\"note\":\"Continued from the OpenAgents app\""),
        "{sent}"
    );
    assert!(
        !sent.contains("Continued from the OpenAgents app: "),
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

/// Every basic turn asks for the chat router with a bounded context: whether
/// a computer is ready, which build this is, and, with a paired computer,
/// its label, so the chat never asks the person to connect one (#10077);
/// never a workspace path or a key. With none, the context names no
/// computer, and the chat still offers to connect one.
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
            computer: Some(crate::router::Computer::Paired {
                name: "Studio Mac".into(),
                engines: vec![],
            }),
            ..crate::router::Context::default()
        }]
    );
    let wire = contexts[0].json();
    assert_eq!(
        wire["computer"],
        serde_json::json!({"place": "paired", "name": "Studio Mac"})
    );
    assert!(wire.get("project").is_none(), "{wire}");

    let bare = Hand::default();
    let mut none =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&bare);
    none.say("Who are you?");
    let context = bare.contexts.lock().unwrap()[0].clone();
    assert!(!context.computer_ready);
    assert_eq!(context.computer, None);
    assert!(context.json().get("computer").is_none());
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
    hand.say("Working on fixing the flaky test.", true);
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
    assert_eq!(fixture.coder.take_go(), Some(crate::coder_tab::Go::Connect));

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
    assert!(node(&chat, "coder-cli-out").is_none());
    let ran = fixture.tap("coder-cli-0-run");
    let out = cli_output(&ran);
    assert!(out.starts_with("Studio Mac"), "{out}");
    // The output is a row of the conversation, under the command it came
    // from, so a long result scrolls instead of covering the composer.
    assert_eq!(
        node(&ran, "coder-cli-out").unwrap()["element"]["props"]["note"],
        "openagents computer list"
    );
    let transcript = node(&ran, "coder-transcript").unwrap();
    assert!(
        transcript["element"]["props"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["key"] == "coder-cli-out")
    );
    assert!(node(&ran, "coder-cli-0-run").is_some(), "Run again stays");
    // With no way to reach computers, the card says so plainly.
    let ran = fixture.tap("coder-cli-1-run");
    assert_eq!(
        node(&ran, "coder-cli-1-why").unwrap()["element"]["props"]["value"],
        "This phone can't reach your computers right now."
    );
}

/// The code block a finished command left in the conversation.
fn cli_output(view: &serde_json::Value) -> String {
    node(view, "coder-cli-out-md").expect("output")["element"]["props"]["blocks"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned()
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
        if node(&done, "coder-cli-out").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        done = fixture.render();
    }
    assert_eq!(cli_output(&done), "{\n  \"xp\": 120\n}");
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, ["openagents", "--json", "verse", "xp"]);
    assert!(!fixture.coder.live(Some(&fixture.computers)));
}

/// The suggestion chips above a new chat's field, as `(key, label)`.
fn suggestions(view: &Value) -> Vec<(String, String)> {
    nodes(view)
        .into_iter()
        .filter_map(|node| {
            let key = node["key"].as_str()?;
            key.starts_with("coder-suggest-").then(|| {
                (
                    key.to_owned(),
                    node["element"]["props"]["label"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
        })
        .collect()
}

/// Every new chat shows suggested questions, not only a first chat ever:
/// with chats already kept and the Gym never opted into, a new chat still
/// offers "Who are you?" and the rest of the first four.
#[test]
fn a_fresh_new_chat_always_shows_suggestions() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    let screen = fixture.render();
    assert_eq!(
        suggestions(&screen),
        [
            ("coder-suggest-meta.who", "Who are you?"),
            ("coder-suggest-meta.capabilities", "What can you do?"),
            ("coder-suggest-gym.news", "What's new in the Gym?"),
            ("coder-suggest-meta.tools", "What tools do you have?"),
        ]
        .map(|(key, label)| (key.to_owned(), label.to_owned()))
    );
    // A chat exists now; a new chat still has its suggestions.
    fixture.say("Tell us a joke");
    hand.say("Why did the relay cross the road?", true);
    let screen = fixture.tap("coder-new");
    assert_eq!(suggestions(&screen).len(), 4, "{:?}", keys(&screen));
    assert!(node(&screen, "coder-suggest-meta.who").is_some());
}

/// A tapped suggestion sends its words and never shows again, on the next
/// new chat and after a relaunch; the next unused one takes its place.
#[test]
fn a_tapped_suggestion_never_shows_again_even_after_a_relaunch() {
    let dir = tempfile::tempdir().expect("temp dir");
    let kept = dir.path().join("basic");
    let hand = Hand::default();
    let mut fixture = Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now)))
        .answered_and_kept(&hand, &kept);
    fixture.tap("coder-suggest-meta.who");
    assert_eq!(hand.asked(), vec![vec!["Who are you?".to_owned()]]);
    hand.say("We are OpenAgents.", true);
    let screen = fixture.tap("coder-new");
    let shown = suggestions(&screen);
    assert!(
        node(&screen, "coder-suggest-meta.who").is_none(),
        "{shown:?}"
    );
    assert_eq!(shown.len(), 4);
    assert_eq!(shown[3].1, "What models does this use?");
    // "What tools do you have?" sends its question, and neither shows again.
    fixture.tap("coder-suggest-meta.tools");
    assert_eq!(hand.asked()[1], ["What tools do you have?"]);
    hand.say("Coder can hand a task to Claude Code.", true);
    drop(fixture);

    let hand = Hand::default();
    let mut fixture = Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now)))
        .answered_and_kept(&hand, &kept);
    let screen = fixture.render();
    let shown = suggestions(&screen);
    assert!(
        node(&screen, "coder-suggest-meta.who").is_none(),
        "{shown:?}"
    );
    assert!(
        node(&screen, "coder-suggest-meta.tools").is_none(),
        "{shown:?}"
    );
    assert_eq!(
        shown
            .iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>(),
        [
            "What can you do?",
            "What's new in the Gym?",
            "What models does this use?",
            "How do I earn XP?"
        ]
    );
}

/// Typing a suggestion's words counts as using it, whatever the case,
/// spacing, or punctuation.
#[test]
fn a_typed_question_hides_the_same_suggestion() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.say("  what can you DO ");
    hand.say("In this chat we can answer questions.", true);
    let screen = fixture.tap("coder-new");
    assert!(
        node(&screen, "coder-suggest-meta.capabilities").is_none(),
        "{:?}",
        suggestions(&screen)
    );
    // "whats new in the gym" is the words of "What's new in the Gym?".
    fixture.say("whats new in the gym");
    hand.say("Nothing new yet.", true);
    let screen = fixture.tap("coder-new");
    assert!(node(&screen, "coder-suggest-gym.news").is_none());
    assert!(node(&screen, "coder-suggest-meta.who").is_some());
}

/// A follow-up chip once used never shows again: tapped, typed, or its
/// prepared answer already shown.
#[test]
fn a_used_followup_chip_never_shows_again() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.say("What model are you?");
    canned(&hand, "Our chat runs on Gemini 3.8 Flash.");
    // Tap "Is this chat private?" (meta.privacy).
    fixture.tap("coder-followup-0");
    hand.say("It is encrypted.", true);
    // Another answer offering the same follow-ups shows only the unused
    // one, at its own index.
    fixture.say("Which model again?");
    canned(&hand, "Our chat runs on Gemini 3.8 Flash.");
    let chat = fixture.render();
    assert!(
        node(&chat, "coder-followup-0").is_none(),
        "{:?}",
        keys(&chat)
    );
    assert_eq!(
        node(&chat, "coder-followup-1").expect("unused")["element"]["props"]["label"],
        "What is Coder?"
    );
    // Typed words count too.
    fixture.say("what is coder?");
    canned(&hand, "Our chat runs on Gemini 3.8 Flash.");
    let chat = fixture.render();
    assert!(node(&chat, "coder-followup-0").is_none());
    assert!(node(&chat, "coder-followup-1").is_none());
    // The prepared answer shown (meta.model) is used: its suggestion is
    // gone from a new chat.
    let screen = fixture.tap("coder-new");
    let shown = suggestions(&screen);
    assert!(
        !shown
            .iter()
            .any(|(_, label)| label == "What models does this use?"),
        "{shown:?}"
    );
}

/// Unused suggestions come first; once they run out, used ones fill in, so
/// a new chat always shows four.
#[test]
fn a_new_chat_always_shows_four_suggestions() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    let mut tapped = Vec::new();
    for round in 0..crate::first_run::SUGGESTIONS.len() {
        let screen = fixture.render();
        let shown = suggestions(&screen);
        assert_eq!(shown.len(), 4, "round {round}: {shown:?}");
        assert!(!tapped.contains(&shown[0].0), "round {round}: {shown:?}");
        tapped.push(shown[0].0.clone());
        fixture.tap(&shown[0].0);
        hand.say("An answer.", true);
        fixture.tap("coder-new");
    }
    let screen = fixture.render();
    assert_eq!(suggestions(&screen).len(), 4, "{:?}", suggestions(&screen));
    assert!(node(&screen, "coder-suggestions").is_some());
    assert_eq!(kinds(&screen, "composer"), 1);
}

/// Every suggestion reads plainly: a short label with no banned word, and
/// an ID unique in the list.
#[test]
fn the_suggestions_are_plain_and_unique() {
    let list = crate::first_run::SUGGESTIONS;
    assert_eq!(list.len(), 10);
    let mut ids = std::collections::BTreeSet::new();
    for suggestion in list {
        assert!(ids.insert(suggestion.id), "{} twice", suggestion.id);
        assert!(
            suggestion.label.chars().count() <= 30,
            "{}",
            suggestion.label
        );
        assert_eq!(
            crate::eval_cards::jargon(suggestion.label),
            None,
            "{}",
            suggestion.label
        );
        assert!(!suggestion.message.is_empty());
    }
}

/// A task whose computer could not start it says why under its phase,
/// instead of waiting as Queued forever.
#[test]
fn a_task_the_computer_could_not_start_says_why() {
    let fixture = Fixture::hosts();
    let mut snapshot = fixture.computers.snapshot().clone();
    let summary = snapshot
        .activity
        .iter_mut()
        .find(|s| s.subject_kind == nostr::activity_summary::SubjectKind::Task)
        .expect("a task summary");
    summary.phase = nostr::activity_summary::Phase::Cancelled;
    summary.attention = nostr::activity_summary::Attention::None;
    summary.headline = coder_host::Note::NotStarted {
        cause: coder_host::StartCause::Claude,
    }
    .headline();
    summary.updated_at = NOW + 1;
    summary.sequence += 100;
    let (host, task) = (summary.host.clone(), summary.subject.clone());
    let label = snapshot
        .host(&host)
        .map(|h| h.label.clone())
        .expect("a host");
    let saved = move |_: &str, subject: &str| {
        (subject == task).then(|| coder_history::Chat {
            id: "chat".into(),
            harness: coder_history::Harness::Coder,
            native_id: None,
            title: "Fix the flaky test".into(),
            title_truncated: false,
            updated_at: None,
            archived: false,
            subagent: false,
            source_id: Some("source".into()),
            status: coder_history::SourceStatus::Available,
        })
    };
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
    let expected = format!(
        "Fix the flaky test\nStopped · {label}\nCouldn't start: Claude Code isn't set up on this computer"
    );
    assert!(labels.contains(&expected), "{labels:?}");
}

/// A computer's own threads, as NIP-HOST `thread.*` answers them: Studio
/// Mac keeps one thread, and a follow-up is answered on the next read, or
/// with `slow`, streams until it is stopped.
#[derive(Default)]
struct HostThreadsFake {
    host: String,
    sent: std::sync::Mutex<Vec<(String, String)>>,
    slow: bool,
    /// A computer from before `thread.stop`.
    old: bool,
    /// Every stop asked, by send ID.
    stops: std::sync::Mutex<Vec<Option<String>>>,
    coder: Option<coder_host::access::thread::ThreadCoder>,
    outside: Option<coder_host::access::thread::ThreadOutside>,
    /// The computer is off: nothing reaches it.
    offline: std::sync::atomic::AtomicBool,
}

impl HostThreadsFake {
    fn reached(&self) -> Result<(), openagents_chat_app::host_threads::Refusal> {
        if self.offline.load(std::sync::atomic::Ordering::SeqCst) {
            Err(openagents_chat_app::host_threads::Refusal::Failed)
        } else {
            Ok(())
        }
    }
}

impl openagents_chat_app::host_threads::Link for HostThreadsFake {
    fn list(
        &self,
        host: &str,
    ) -> Result<
        Vec<coder_host::access::thread::ThreadRow>,
        openagents_chat_app::host_threads::Refusal,
    > {
        if host != self.host {
            return Err(openagents_chat_app::host_threads::Refusal::NotServed);
        }
        self.reached()?;
        Ok(vec![coder_host::access::thread::ThreadRow {
            thread: "4a".repeat(16),
            title: "Write a haiku about rain".into(),
            started: NOW - 600,
            updated: NOW - 60,
            pinned: false,
            coder: None,
        }])
    }

    fn read(
        &self,
        _host: &str,
        thread: &str,
        _before: Option<u64>,
    ) -> Result<coder_host::access::thread::ThreadPage, openagents_chat_app::host_threads::Refusal>
    {
        use coder_host::access::thread::{ThreadRole, ThreadTurn};
        self.reached()?;
        let turn = |role, text: &str, request: Option<String>| ThreadTurn {
            role,
            text: text.into(),
            at: Some(NOW - 60),
            stopped: false,
            model: None,
            request,
            extras: Default::default(),
        };
        let mut turns = vec![
            turn(ThreadRole::User, "Write a haiku about rain", None),
            turn(ThreadRole::Assistant, "Rain on the roof.", None),
        ];
        let (mut busy, stops) = (false, self.stops.lock().unwrap().clone());
        for (request, text) in self.sent.lock().unwrap().iter() {
            turns.push(turn(ThreadRole::User, text, Some(request.clone())));
            if !self.slow {
                turns.push(turn(ThreadRole::Assistant, "Snow on the pines.", None));
            } else if stops.contains(&Some(request.clone())) {
                let mut stopped = turn(ThreadRole::Assistant, "Snow on", None);
                stopped.stopped = true;
                turns.push(stopped);
            } else {
                busy = true;
            }
        }
        Ok(coder_host::access::thread::ThreadPage {
            thread: thread.into(),
            title: "Write a haiku about rain".into(),
            start: 0,
            total: turns.len() as u64,
            turns,
            busy,
            partial: if busy {
                "Snow on".into()
            } else {
                String::new()
            },
            failure: None,
            coder: self.coder.clone(),
            outside: self.outside.clone(),
        })
    }

    fn send(
        &self,
        _host: &str,
        _thread: &str,
        request: &str,
        text: &str,
    ) -> Result<(), openagents_chat_app::host_threads::Refusal> {
        self.reached()?;
        // Once per send ID, as the host appends it.
        let mut sent = self.sent.lock().unwrap();
        if !sent.iter().any(|(held, _)| held == request) {
            sent.push((request.to_owned(), text.to_owned()));
        }
        Ok(())
    }

    fn stop(
        &self,
        _host: &str,
        _thread: &str,
        request: Option<&str>,
    ) -> Result<(), openagents_chat_app::host_threads::Refusal> {
        if self.old {
            return Err(openagents_chat_app::host_threads::Refusal::NotServed);
        }
        self.reached()?;
        self.stops.lock().unwrap().push(request.map(str::to_owned));
        Ok(())
    }

    fn run(
        &self,
        _host: &str,
        _thread: &str,
    ) -> Result<String, openagents_chat_app::host_threads::Refusal> {
        Err(openagents_chat_app::host_threads::Refusal::NotServed)
    }
}

#[test]
fn a_computers_own_threads_list_beside_the_phones_and_continue_through_it() {
    let mut fixture = Fixture::hosts();
    let studio = fixture
        .computers
        .snapshot()
        .hosts
        .iter()
        .find(|host| host.label == "Studio Mac")
        .expect("Studio Mac")
        .key
        .clone();
    let fake = std::sync::Arc::new(HostThreadsFake {
        host: studio,
        ..HostThreadsFake::default()
    });
    let coder = std::mem::replace(&mut fixture.coder, CoderTab::new("coder:none".into()));
    fixture.coder = coder.with_threads(
        openagents_chat_app::host_threads::HostThreads::default(),
        Some(fake.clone()),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let row = loop {
        let list = fixture.list();
        if let Some(row) = keys(&list)
            .into_iter()
            .find(|key| key.starts_with("thread-"))
        {
            let label = node(&list, &row).unwrap()["element"]["props"]["label"]
                .as_str()
                .unwrap()
                .to_owned();
            assert!(
                label.starts_with("Write a haiku about rain\nStudio Mac · "),
                "{label}"
            );
            break row;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no computer thread listed"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    fixture.tap(&row);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let chat = loop {
        let chat = fixture.render();
        if node(&chat, "thread-m1").is_some() {
            break chat;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the thread never opened"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(
        node(&chat, "coder-chat-place").unwrap()["element"]["props"]["value"],
        "Studio Mac"
    );
    let composer = &node(&chat, "coder-composer").unwrap()["element"]["props"];
    assert_eq!(composer["enabled"], true);
    assert_eq!(composer["placeholder"], "Message OpenAgents on Studio Mac");
    fixture.send(&chat, None, "And in the snow?");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let chat = fixture.render();
        if node(&chat, "thread-m3").is_some() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the follow-up never showed"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let sent = fake.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, "And in the snow?");
    assert_eq!(sent[0].0.len(), 32);
    // The phone's own chats are untouched, and New chat leaves the thread.
    let fresh = fixture.tap("coder-new");
    assert!(
        node(&fresh, "coder-suggestions").is_some()
            || node(&fresh, "coder-new-transcript").is_some()
    );
}

#[test]
fn a_computers_threads_survive_a_relaunch_with_it_off_and_a_queued_follow_up_goes_once() {
    use std::sync::atomic::Ordering;
    let mut fixture = Fixture::hosts();
    let studio = fixture
        .computers
        .snapshot()
        .hosts
        .iter()
        .find(|host| host.label == "Studio Mac")
        .expect("Studio Mac")
        .key
        .clone();
    let store = tempfile::tempdir().expect("temp dir");
    let secret = secp256k1::SecretKey::from_byte_array([0x22; 32]).expect("key");
    let launch = |fixture: &mut Fixture, fake: &std::sync::Arc<HostThreadsFake>, name: &str| {
        fixture.coder = CoderTab::new(format!("coder:{name}")).with_threads(
            openagents_chat_app::host_threads::HostThreads::default()
                .with_cache(Cache::open(store.path(), &secret).ok()),
            Some(fake.clone()),
        );
    };
    let until = |fixture: &mut Fixture, what: &str, done: &dyn Fn(&Value) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let view = fixture.render();
            if done(&view) {
                return view;
            }
            assert!(std::time::Instant::now() < deadline, "{what}");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    let thread_row = |list: &Value| {
        keys(list)
            .into_iter()
            .find(|key| key.starts_with("thread-"))
    };
    let kept = || {
        std::fs::read_dir(store.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("ht-"))
        })
    };
    // The computer is on: its thread lists and is read, and kept.
    let on = std::sync::Arc::new(HostThreadsFake {
        host: studio.clone(),
        ..HostThreadsFake::default()
    });
    launch(&mut fixture, &on, "first");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let row = loop {
        let list = fixture.list();
        if let Some(row) = thread_row(&list) {
            break row;
        }
        assert!(std::time::Instant::now() < deadline, "no thread listed");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    fixture.tap(&row);
    until(&mut fixture, "the thread never opened", &|view| {
        node(view, "thread-m1").is_some()
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !kept() {
        assert!(
            std::time::Instant::now() < deadline,
            "the thread was never kept"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    fixture.tap("coder-new");

    // Relaunch with the computer off: the thread lists at once, marked
    // with when it was read, and opens from what the phone kept.
    let off = std::sync::Arc::new(HostThreadsFake {
        host: studio.clone(),
        offline: true.into(),
        ..HostThreadsFake::default()
    });
    launch(&mut fixture, &off, "offline");
    let list = fixture.list();
    let row = thread_row(&list).expect("the kept thread lists at once");
    let label = node(&list, &row).unwrap()["element"]["props"]["label"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        label.starts_with("Write a haiku about rain\nStudio Mac · ") && label.contains("Last read"),
        "{label}"
    );
    let chat = fixture.tap(&row);
    let chat = if node(&chat, "thread-m1").is_some() {
        chat
    } else {
        fixture.render()
    };
    assert!(node(&chat, "thread-m0").is_some() && node(&chat, "thread-m1").is_some());
    assert!(node(&chat, "thread-loading").is_none());
    assert!(
        node(&chat, "thread-kept").unwrap()["element"]["props"]["value"]
            .as_str()
            .unwrap()
            .starts_with("Saved on this phone · last read ")
    );
    // A follow-up while it is off waits on the phone.
    fixture.send(&chat, None, "And in the snow?");
    let queued = until(&mut fixture, "the queued follow-up never showed", &|view| {
        node(view, "thread-m2").is_some()
    });
    assert!(
        queued["root"]
            .to_string()
            .contains("Waiting for Studio Mac…"),
        "a queued follow-up waits rather than works"
    );
    assert!(off.sent.lock().unwrap().is_empty());
    fixture.tap("coder-new");

    // Relaunch with the computer back: the follow-up goes once, under the
    // send ID it was given offline, without the thread being opened.
    let back = std::sync::Arc::new(HostThreadsFake {
        host: studio,
        ..HostThreadsFake::default()
    });
    launch(&mut fixture, &back, "back");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while back.sent.lock().unwrap().is_empty() {
        fixture.render();
        assert!(std::time::Instant::now() < deadline, "never delivered");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let list = fixture.list();
    let row = thread_row(&list).expect("listed");
    fixture.tap(&row);
    until(&mut fixture, "the reply never showed", &|view| {
        node(view, "thread-m3").is_some() && node(view, "thread-kept").is_none()
    });
    let sent = back.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, "And in the snow?");
    assert_eq!(sent[0].0.len(), 32);
    assert!(off.offline.load(Ordering::SeqCst) && off.sent.lock().unwrap().is_empty());
}

/// Open Studio Mac's thread with `fake`, send a follow-up whose reply
/// streams, and return the view while it streams.
fn stream_a_computers_reply(fixture: &mut Fixture, fake: std::sync::Arc<HostThreadsFake>) -> Value {
    let coder = std::mem::replace(&mut fixture.coder, CoderTab::new("coder:none".into()));
    fixture.coder = coder.with_threads(
        openagents_chat_app::host_threads::HostThreads::default(),
        Some(fake),
    );
    let until = |fixture: &mut Fixture, what: &str, done: &dyn Fn(&Value) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let view = fixture.render();
            if done(&view) {
                return view;
            }
            assert!(std::time::Instant::now() < deadline, "{what}");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    let row = loop {
        let list = fixture.list();
        if let Some(row) = keys(&list)
            .into_iter()
            .find(|key| key.starts_with("thread-"))
        {
            break row;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    fixture.tap(&row);
    let chat = until(fixture, "the thread never opened", &|view| {
        node(view, "thread-m1").is_some()
    });
    fixture.send(&chat, None, "Tell me slowly about snow");
    until(fixture, "the reply never streamed", &|view| {
        node(view, "coder-composer").unwrap()["element"]["props"]["busy"] == true
            && keys(view).iter().any(|key| key == "thread-m3")
    })
}

#[test]
fn a_computers_streaming_reply_stops_from_the_phone_and_offers_to_stop_its_coder() {
    use nostr::activity_summary::{Attention, Phase};
    // A running Coder task, which the thread started.
    let (mut fixture, script, _) = Fixture::scripted(Phase::Running, Attention::None);
    let (host, task) = fixture.coder.open_task().expect("an open task");
    let studio = fixture
        .computers
        .snapshot()
        .hosts
        .iter()
        .find(|host| host.label == "Studio Mac")
        .expect("Studio Mac")
        .key
        .clone();
    let fake = std::sync::Arc::new(HostThreadsFake {
        host: studio,
        slow: true,
        coder: Some(coder_host::access::thread::ThreadCoder {
            host,
            task,
            project: None,
            at: None,
        }),
        ..HostThreadsFake::default()
    });
    let chat = stream_a_computers_reply(&mut fixture, fake.clone());
    // The computer answered the phone's no-op stop, so the stop control
    // shows while the reply streams, and Coder is not offered yet.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut chat = chat;
    while node(&chat, "coder-composer").unwrap()["element"]["props"]["stop"].is_null() {
        assert!(std::time::Instant::now() < deadline, "no stop control");
        std::thread::sleep(std::time::Duration::from_millis(20));
        chat = fixture.render();
    }
    assert!(node(&chat, "thread-coder-stop").is_none());
    assert_eq!(fake.stops.lock().unwrap().len(), 1, "only the probe");
    // The tap names the view the phone held; a newer one replaced it while
    // the reply streamed. The stop still goes, but no other stale tap.
    let stale = chat;
    fixture.render();
    let late = |node: &str| Activation {
        instance: stale["instance"].as_str().unwrap().into(),
        revision: stale["revision"].as_u64().unwrap(),
        node: node.into(),
    };
    fixture.coder.activate(
        &late("coder-new"),
        Some(&mut fixture.computers),
        &mut fixture.chats,
    );
    assert!(
        node(&fixture.render(), "coder-chat-place").is_some(),
        "still the thread"
    );
    fixture.coder.activate(
        &late("coder-composer"),
        Some(&mut fixture.computers),
        &mut fixture.chats,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let chat = loop {
        let chat = fixture.render();
        if node(&chat, "thread-coder-stop").is_some()
            && chat["root"]
                .to_string()
                .contains("Stopped showing this reply")
        {
            break chat;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the reply never showed stopped: {:?}",
            keys(&chat)
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    // The stop named the phone's own follow-up, and the partial stays.
    let stops = fake.stops.lock().unwrap().clone();
    let sent = fake.sent.lock().unwrap()[0].0.clone();
    assert_eq!(stops.last().unwrap().as_deref(), Some(sent.as_str()));
    let composer = &node(&chat, "coder-composer").unwrap()["element"]["props"];
    assert_eq!(composer["busy"], false);
    assert!(composer["stop"].is_null());
    // Stopping the reply stopped nothing on Coder; the offer does, through
    // the task's own stop.
    assert!(script.lock().unwrap().commands.is_empty());
    // Tapped on a view a newer render replaced, as an open thread renders
    // every few hundred milliseconds while its Coder task runs (#10043).
    fixture.render();
    fixture.coder.activate(
        &Activation {
            instance: chat["instance"].as_str().unwrap().into(),
            revision: chat["revision"].as_u64().unwrap(),
            node: "thread-coder-stop".into(),
        },
        Some(&mut fixture.computers),
        &mut fixture.chats,
    );
    let commands = script.lock().unwrap().commands.clone();
    assert_eq!(commands.len(), 1, "{commands:?}");
    assert_eq!(commands[0].0, coder_host::CommandAction::Interrupt);
}

/// #10043: a Coder run `openagents chat` started on the computer, in a task
/// store its host does not serve, is said plainly, with no Open Coder or
/// Stop Coder too control that could not reach it.
#[test]
fn a_local_run_outside_the_computers_host_is_said_plainly_with_no_dead_controls() {
    let mut fixture = Fixture::hosts();
    let studio = fixture
        .computers
        .snapshot()
        .hosts
        .iter()
        .find(|host| host.label == "Studio Mac")
        .expect("Studio Mac")
        .key
        .clone();
    let fake = std::sync::Arc::new(HostThreadsFake {
        host: studio,
        slow: true,
        outside: Some(coder_host::access::thread::ThreadOutside {
            task: "a7".repeat(32),
            project: Some("proj".into()),
            at: None,
        }),
        ..HostThreadsFake::default()
    });
    let chat = stream_a_computers_reply(&mut fixture, fake);
    let words = node(&chat, "thread-coder-outside").expect("the outside run is said");
    let text = words.to_string();
    assert!(
        text.contains("Coder task for proj ran on Studio Mac outside the OpenAgents app"),
        "{text}"
    );
    assert!(text.contains("can't open or stop it"), "{text}");
    for control in ["thread-coder-open", "thread-coder-stop"] {
        assert!(node(&chat, control).is_none(), "{control}");
    }
}

#[test]
fn an_older_computer_shows_no_stop_control_while_its_reply_streams() {
    let mut fixture = Fixture::hosts();
    let studio = fixture
        .computers
        .snapshot()
        .hosts
        .iter()
        .find(|host| host.label == "Studio Mac")
        .expect("Studio Mac")
        .key
        .clone();
    let fake = std::sync::Arc::new(HostThreadsFake {
        host: studio,
        slow: true,
        old: true,
        ..HostThreadsFake::default()
    });
    let _ = stream_a_computers_reply(&mut fixture, fake);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let chat = fixture.render();
    let composer = &node(&chat, "coder-composer").unwrap()["element"]["props"];
    assert_eq!(composer["busy"], true);
    assert!(composer["stop"].is_null(), "no stop control: {composer}");
    let _ = chat;
}

/// A saved chat's card in the previous chats carries the shared chat menu:
/// pin, then unpin, archive, then restore, each carried out on the saved
/// conversation and offered again from the new state.
#[test]
fn saved_chat_cards_offer_the_shared_chat_menu() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.list();
    fixture.tap("coder-back");
    fixture.say("Keep this chat");
    hand.say("Kept.", true);
    fixture.render();
    let list = fixture.list();
    let card = keys(&list)
        .into_iter()
        .find(|key| key.starts_with("talk-") && key.ends_with("-card"))
        .expect("a chat card");
    let row = card.trim_end_matches("-card").to_owned();
    let card_node = node(&list, &card).unwrap();
    assert_eq!(card_node["style"]["menu"], "context");
    let items = |view: &Value| -> Vec<String> {
        node(view, &format!("{row}-card")).unwrap()["element"]["props"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .skip(1)
            .map(|item| {
                item["element"]["props"]["label"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    };
    assert_eq!(items(&list), ["Pin chat", "Archive chat"]);
    // The card itself still opens the chat.
    assert_eq!(
        node(&list, &card).unwrap()["element"]["props"]["children"][0]["key"],
        row.as_str()
    );
    let list = fixture.tap(&format!("{row}-pin"));
    assert_eq!(items(&list), ["Unpin chat", "Archive chat"]);
    let list = fixture.tap(&format!("{row}-archive"));
    assert_eq!(items(&list), ["Unpin chat", "Restore chat"]);
    let list = fixture.tap(&format!("{row}-restore"));
    assert_eq!(items(&list), ["Unpin chat", "Archive chat"]);
}

/// With phone attachments turned back on (they are off since #10093): the
/// attach control asks the host for a photo; an attached image shows as an
/// `image:` surface card with its alternative text, a text-only send keeps
/// the words and the image, and removing the image lets it send.
#[test]
fn attached_images_show_as_shared_image_nodes_and_are_not_dropped() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.coder.set_attachments(true);
    fixture.list();
    let screen = fixture.tap("coder-back");
    assert!(node(&screen, "coder-attach").is_some());
    fixture.tap("coder-attach");
    assert_eq!(
        fixture.coder.take_go(),
        Some(crate::coder_tab::Go::PickImage)
    );
    let png = openagents_chat_app::attachments::Image::pixels(3, 2, vec![200; 24]).unwrap();
    fixture
        .coder
        .attach_image("Photo.png", png.bytes.as_ref().clone());
    let screen = fixture.render();
    let surface = nodes(&screen)
        .into_iter()
        .find(|node| node["element"]["kind"] == "surface")
        .expect("an image surface")
        .clone();
    let resource = surface["element"]["props"]["resource"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(resource.starts_with("image:"));
    assert_eq!(surface["element"]["props"]["label"], "Photo.png · 3 × 2");
    assert_eq!(
        fixture.coder.image(&resource).unwrap().bytes.as_slice(),
        png.bytes.as_slice()
    );
    // A bad image is refused with a notice and adds nothing.
    fixture
        .coder
        .attach_image("bad.png", b"not an image".to_vec());
    let screen = fixture.render();
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text.contains("PNG or JPEG"))
    );
    // A send sends only the words; the image stays in the draft, bound to
    // that message, and a reply that is not coding keeps it there.
    let token = composer_of(&screen)["token"].as_str().unwrap().to_owned();
    fixture
        .coder
        .submit(&token, "Look at this", None, &mut fixture.chats);
    let screen = fixture.render();
    assert_eq!(hand.asked(), vec![vec!["Look at this".to_owned()]]);
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text == openagents_chat_app::attachments::HELD_FOR_CODER)
    );
    assert!(fixture.coder.image(&resource).is_some());
    hand.say("It's a photo of a cat.", true);
    let screen = fixture.render();
    assert!(
        texts(&screen)
            .iter()
            .any(|text| text == openagents_chat_app::attachments::ONLY_TO_CODER)
    );
    assert!(fixture.coder.image(&resource).is_some());
    let id = resource.trim_start_matches("image:");
    fixture.tap(&format!("image-remove-{id}"));
    assert!(fixture.coder.image(&resource).is_none());
}

/// A finished chat on the phone shows the change at its exact revisions
/// and offers Publish under `operate`. A publication of a head the
/// computer moved past is refused, the card then reads the change again
/// and says it is stale, and the refreshed head publishes once and links
/// its pull request (#10067, #10068).
#[test]
fn a_phone_reviews_a_change_refreshes_a_stale_view_and_publishes_once() {
    use nostr::activity_summary::{Attention, Phase};
    let (mut fixture, script, _) = Fixture::scripted(Phase::Completed, Attention::Completed);
    script.lock().unwrap().head = Some('c');
    fixture.coder.flush(Some(&mut fixture.computers));
    let card = fixture.render();
    let shown = texts(&card);
    assert!(shown.iter().any(|t| t == "2 files, +3, −2"), "{shown:?}");
    assert!(
        shown
            .iter()
            .any(|t| t == "Base 1111111111 · head cccccccccc"),
        "{shown:?}"
    );
    assert!(node(&card, "coder-changes-publish").is_some());
    // The diff shows line by line.
    let open = fixture.tap("coder-changes-open");
    assert!(
        node(&open, "coder-changes-line-0").is_some(),
        "{:?}",
        keys(&open)
    );
    assert!(texts(&open).iter().any(|t| t == "+fn more() {}"));
    // The worktree moves on the computer before the person publishes.
    script.lock().unwrap().head = Some('d');
    let refused = fixture.tap("coder-changes-publish");
    assert_eq!(script.lock().unwrap().published, vec!["c".repeat(40)]);
    assert!(
        node(&refused, "coder-changes-note-publication").is_some(),
        "{:?}",
        keys(&refused)
    );
    // The refusal reads the change again: the view is stale, Publish is
    // held back, and Refresh shows the new head.
    fixture.coder.flush(Some(&mut fixture.computers));
    let stale = fixture.render();
    assert!(
        node(&stale, "coder-changes-note-stale").is_some(),
        "{:?}",
        keys(&stale)
    );
    assert!(node(&stale, "coder-changes-publish").is_none());
    let fresh = fixture.tap("coder-changes-refresh");
    assert!(
        texts(&fresh)
            .iter()
            .any(|t| t == "Base 1111111111 · head dddddddddd")
    );
    let published = fixture.tap("coder-changes-publish");
    assert_eq!(
        script.lock().unwrap().published,
        vec!["c".repeat(40), "d".repeat(40)]
    );
    assert!(
        node(&published, "coder-changes-link").is_some(),
        "{:?}",
        keys(&published)
    );
    assert!(
        node(&published, "coder-changes-publish").is_none(),
        "published once"
    );
    // Reading again keeps the link and offers nothing more to publish.
    fixture.coder.flush(Some(&mut fixture.computers));
    assert!(node(&fixture.render(), "coder-changes-publish").is_none());
    assert_eq!(script.lock().unwrap().published.len(), 2);
}

/// A computer that reviews no change for a task shows no card.
#[test]
fn a_computer_that_reviews_nothing_shows_no_change_card() {
    use nostr::activity_summary::{Attention, Phase};
    let (mut fixture, _script, _) = Fixture::scripted(Phase::Completed, Attention::Completed);
    fixture.coder.flush(Some(&mut fixture.computers));
    assert!(node(&fixture.render(), "coder-changes").is_none());
}

/// What [`Imaging`] does with a start, and what reached the computer.
#[derive(Default)]
struct Delivery {
    /// The computer refuses the task.
    refuse: bool,
    /// The computer takes the task but its answer never arrives.
    lose_answer: bool,
    /// Image chunks sent.
    chunks: usize,
    /// Each accepted task's images, with the bytes the computer holds.
    created: Vec<Vec<(coder_host::access::media::ImageRef, Vec<u8>)>>,
    /// The capabilities each computer's presence advertises; no presence
    /// when unset.
    advertise: Option<Vec<&'static str>>,
    /// The engine each accepted task asked for (#10081).
    engines: Vec<Option<nostr::cj_conversation::Engine>>,
    /// Keep only the hosts with these tags, when set.
    only: Option<Vec<u8>>,
    /// The clock the snapshot carries, when set.
    now: Option<u64>,
    /// The phase every accepted task's newest summary says, when set,
    /// with the generic headline.
    phase: Option<nostr::activity_summary::Phase>,
    /// Each accepted task, by host and task ID.
    tasks: Vec<(String, String)>,
}

/// The fixture's hosts, keeping image chunks as a host does, with a start
/// the test can refuse or whose answer it can lose.
struct Imaging {
    inner: Synthetic,
    delivery: std::sync::Arc<std::sync::Mutex<Delivery>>,
}

impl ComputersService for Imaging {
    fn snapshot(&mut self) -> Answer<coder_computers::Snapshot> {
        use nostr::activity_summary::{ActivitySummary, Attention, SubjectKind, generic_headline};
        let mut snapshot = self.inner.snapshot()?;
        {
            let delivery = self.delivery.lock().unwrap();
            if let Some(now) = delivery.now {
                snapshot.now = now;
            }
            if let Some(phase) = delivery.phase {
                // A relaunched fixture never made the task: its summary
                // comes as the host republishes it.
                for (host, task) in &delivery.tasks {
                    if !snapshot.activity.iter().any(|s| s.subject == *task) {
                        snapshot.activity.push(ActivitySummary {
                            host: host.clone(),
                            subject_kind: SubjectKind::Task,
                            subject: task.clone(),
                            sequence: 1,
                            phase,
                            headline: String::new(),
                            attention: Attention::None,
                            updated_at: snapshot.now,
                        });
                    }
                }
                for summary in &mut snapshot.activity {
                    if delivery
                        .tasks
                        .iter()
                        .any(|(_, task)| *task == summary.subject)
                    {
                        summary.phase = phase;
                        summary.headline = generic_headline(SubjectKind::Task, phase).into();
                    }
                }
            }
        }
        if let Some(tags) = self.delivery.lock().unwrap().only.clone() {
            let keys: Vec<String> = tags
                .iter()
                .map(|tag| coder_computers::synthetic::key(*tag))
                .collect();
            snapshot.hosts.retain(|host| keys.contains(&host.key));
        }
        if let Some(capabilities) = self.delivery.lock().unwrap().advertise.clone() {
            for host in &mut snapshot.hosts {
                host.presence = Some(coder_host::reach::presence::Received {
                    presence: coder_host::reach::presence::Presence {
                        v: coder_host::reach::presence::SCHEMA.into(),
                        requires: vec![],
                        host: host.key.clone(),
                        owner: host.key.clone(),
                        generation: 1,
                        protocol: coder_host::PROTOCOL_VERSION,
                        compatibility: coder_host::reach::presence::VersionRange {
                            min: coder_host::PROTOCOL_VERSION,
                            max: coder_host::PROTOCOL_VERSION,
                        },
                        capabilities: capabilities.iter().map(|c| (*c).to_owned()).collect(),
                        observed_at: snapshot.now,
                        telemetry: None,
                        meta: None,
                    },
                    received_at: snapshot.now,
                });
            }
        }
        Ok(snapshot)
    }
    fn refresh_workspaces(&mut self, host: &str) -> Answer<()> {
        self.inner.refresh_workspaces(host)
    }
    fn put_artifact(
        &mut self,
        host: &str,
        put: &coder_host::access::media::ArtifactPut,
    ) -> Answer<coder_host::access::media::ArtifactState> {
        self.delivery.lock().unwrap().chunks += 1;
        self.inner.put_artifact(host, put)
    }
    fn create_task(&mut self, host: &str, task: &coder_host::TaskCreate) -> Answer<String> {
        let mut delivery = self.delivery.lock().unwrap();
        if delivery.refuse {
            return Err(coder_host::access::Error::new(
                coder_host::Code::Unsupported,
                "this computer keeps no images",
            ));
        }
        let id = self.inner.create_task(host, task)?;
        if delivery.lose_answer {
            return Err(coder_host::access::Error::new(
                coder_host::Code::Transport,
                "the answer was lost",
            ));
        }
        delivery
            .created
            .push(self.inner.task_images.get(&id).cloned().unwrap_or_default());
        delivery.engines.push(task.engine);
        delivery.tasks.push((host.to_owned(), id.clone()));
        Ok(id)
    }
    fn nudge_host(&mut self, host: &str) -> Answer<()> {
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

fn image_surfaces(view: &Value) -> usize {
    nodes(view)
        .into_iter()
        .filter(|node| {
            node["element"]["kind"] == "surface"
                && node["element"]["props"]["resource"]
                    .as_str()
                    .is_some_and(|resource| resource.starts_with("image:"))
        })
        .count()
}

/// Run Coder carries the conversation's attached screenshot to the
/// computer as its exact bytes. A computer that refuses the task, or whose
/// answer is lost, leaves the draft's images where they were; the hosted
/// conversation still refuses them.
/// The engine the reply's typed `run_coder` offer named reaches the computer
/// in the phone's own `task.create` (#10081), so a run asked of Claude Code
/// is asked of Claude Code there; never read from the words. A computer
/// whose presence does not say it reads the field gets the request it always
/// got, and runs its default.
#[test]
fn run_coder_asks_the_computer_for_the_engine_the_offer_named() {
    use nostr::cj_conversation::Engine;
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery {
        advertise: Some(vec![
            "task-create",
            coder_host::access::protocol::TASK_ENGINE,
        ]),
        ..Delivery::default()
    }));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    let offer = |hand: &Hand, engine: Option<&str>| {
        let mut offer = json!({"v": 2, "type": "offer", "offer": "run_coder",
            "target": "connected_computer", "label": "Run Coder"});
        if let Some(engine) = engine {
            offer["engine"] = json!(engine);
        }
        hand.route(&[offer]);
        hand.say("Working on this.", true);
    };
    fixture.say("Do a test delegation to claude");
    offer(&hand, Some("claude_code"));
    fixture.tap("coder-run");
    assert!(fixture.coder.open_task().is_some());
    assert_eq!(delivery.lock().unwrap().engines, [Some(Engine::ClaudeCode)]);

    // A reply that named no engine asks for none. Meanwhile the computer
    // is replaced by one that predates the field; the phone reads that
    // from its presence with the start's refresh.
    fixture.tap("coder-new");
    fixture.say("Fix the flaky test in my repo");
    offer(&hand, None);
    delivery.lock().unwrap().advertise = Some(vec!["task-create"]);
    fixture.tap("coder-run");
    assert_eq!(
        delivery.lock().unwrap().engines,
        [Some(Engine::ClaudeCode), None]
    );

    // A computer that predates the field is never sent it.
    fixture.tap("coder-new");
    fixture.say("Do a test delegation to devin");
    offer(&hand, Some("devin"));
    fixture.tap("coder-run");
    assert_eq!(
        delivery.lock().unwrap().engines,
        [Some(Engine::ClaudeCode), None, None]
    );
}

/// The phone's Coder start at once (#10101): a coding reply to a message
/// sent here, on a ready computer this phone may operate whose presence
/// says its owner starts Coder at once, makes exactly one `task.create`
/// with no tap, asking for the offer's engine with the message that asked;
/// the reply then shows the start with Stop instead of **Run Coder**, and
/// stays that way however often the tab redraws.
#[test]
fn a_coding_reply_starts_coder_at_once_on_a_computer_that_allows_it() {
    use coder_host::access::protocol::{CODER_START_AT_ONCE, TASK_ENGINE};
    use nostr::cj_conversation::Engine;
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery {
        advertise: Some(vec!["task-create", TASK_ENGINE, CODER_START_AT_ONCE]),
        ..Delivery::default()
    }));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.say("Can you summarize the code that implements that or doesn't?");
    hand.route(&[json!({"v": 2, "type": "offer", "offer": "run_coder",
        "target": "connected_computer", "label": "Run Coder", "engine": "claude_code"})]);
    hand.say("Looking through the code.", true);
    let view = fixture.render();
    assert_eq!(delivery.lock().unwrap().engines, [Some(Engine::ClaudeCode)]);
    assert!(node(&view, "coder-run").is_none(), "{:?}", keys(&view));
    assert!(node(&view, "coder-start").is_some(), "{:?}", keys(&view));
    assert!(
        node(&view, "coder-start-stop").is_some(),
        "{:?}",
        keys(&view)
    );
    assert!(
        texts(&view)
            .iter()
            .any(|t| t == "Coder started on Studio Mac"),
        "{:?}",
        texts(&view)
    );
    // The conversation stays; its task is the one started.
    assert!(fixture.coder.open_task().is_none());
    for _ in 0..5 {
        fixture.render();
    }
    assert_eq!(delivery.lock().unwrap().engines.len(), 1);
    // **Open Coder** opens the task it started, on the ready computer.
    fixture.tap("coder-start-open");
    let (host, _) = fixture
        .coder
        .open_task()
        .expect("Open Coder opens the task");
    assert_eq!(host, coder_computers::synthetic::key(0xa1));
    assert_eq!(delivery.lock().unwrap().engines.len(), 1);
}

/// A computer whose owner asks first (`coder.start: ask_first`), or one
/// that predates the capability, advertises no
/// `coder-start-at-once`: the coding reply keeps **Run Coder**, and
/// nothing starts until the tap (#10101).
#[test]
fn a_computer_that_asks_first_keeps_run_coder() {
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery {
        advertise: Some(vec![
            "task-create",
            coder_host::access::protocol::TASK_ENGINE,
        ]),
        ..Delivery::default()
    }));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.say("Fix the flaky test in my repo");
    hand.judge(crate::basic_coder::Lane::Computer);
    hand.say("Coder can take this on.", true);
    let view = fixture.render();
    assert!(delivery.lock().unwrap().engines.is_empty());
    assert!(node(&view, "coder-run").is_some(), "{:?}", keys(&view));
    assert!(node(&view, "coder-start").is_none());
    // The tap starts it once, and the Coder chat opens as before.
    fixture.tap("coder-run");
    assert_eq!(delivery.lock().unwrap().engines.len(), 1);
    assert!(fixture.coder.open_task().is_some());
    for _ in 0..3 {
        fixture.render();
    }
    assert_eq!(delivery.lock().unwrap().engines.len(), 1);
}

/// The paired computer's coding agents, from its presence, ride every
/// chat turn's context (#10119): each engine and its state in the
/// computer's order, so "what coding agents are connected?" is answered
/// from them; a flag this build does not know is left out, and a computer
/// whose presence names none sends the context it always did.
#[test]
fn a_turn_names_the_paired_computers_coding_agents() {
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery {
        advertise: Some(vec![
            "task-create",
            "engine-ready-codex",
            "engine-ready-claude",
            "engine-limited-grok",
            "engine-someday-devin",
            "engine-not_enabled-opencode",
        ]),
        ..Delivery::default()
    }));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.say("what coding agents are connected?");
    let context = hand.contexts.lock().unwrap()[0].clone();
    assert_eq!(
        context.json()["computer"],
        json!({"place": "paired", "name": "Studio Mac", "engines": [
            {"engine": "codex", "state": "ready"},
            {"engine": "claude", "state": "ready"},
            {"engine": "grok", "state": "limited"},
            {"engine": "opencode", "state": "not_enabled"},
        ]})
    );

    let older = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: std::sync::Arc::new(std::sync::Mutex::new(Delivery {
            advertise: Some(vec!["task-create"]),
            ..Delivery::default()
        })),
    })
    .answered_by(&older);
    fixture.say("what coding agents are connected?");
    assert_eq!(
        older.contexts.lock().unwrap()[0].json()["computer"],
        json!({"place": "paired", "name": "Studio Mac"})
    );
}

/// The phone's start card, after a coding reply started Coder at once on
/// a computer whose start `delivery` records.
fn started_at_once(fixture: &mut Fixture, hand: &Hand) -> Value {
    fixture.say("Archive the iOS app and upload it to TestFlight");
    hand.route(&[json!({"v": 2, "type": "offer", "offer": "run_coder",
        "target": "connected_computer", "label": "Run Coder"})]);
    hand.say("Archiving and uploading it.", true);
    fixture.render()
}

fn at_once() -> std::sync::Arc<std::sync::Mutex<Delivery>> {
    use coder_host::access::protocol::{CODER_START_AT_ONCE, TASK_ENGINE};
    std::sync::Arc::new(std::sync::Mutex::new(Delivery {
        advertise: Some(vec!["task-create", TASK_ENGINE, CODER_START_AT_ONCE]),
        phase: Some(nostr::activity_summary::Phase::Running),
        ..Delivery::default()
    }))
}

/// The first conversation row of the chats list.
fn conversation_row(list: &Value) -> String {
    keys(list)
        .into_iter()
        .find(|key| key.starts_with("talk-") && key.matches('-').count() == 1)
        .unwrap_or_else(|| panic!("no conversation in {:?}", keys(list)))
}

/// The start card's note.
fn start_note(view: &Value) -> String {
    node(view, "coder-start-note")
        .and_then(|note| note["element"]["props"]["value"].as_str())
        .unwrap_or_else(|| panic!("no start note in {:?}", keys(view)))
        .to_owned()
}

/// A run started from the phone can take many minutes, such as an iOS
/// archive: while it runs, its card says how long Coder has worked
/// (#10118).
#[test]
fn a_running_start_says_how_long_coder_has_worked() {
    let delivery = at_once();
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    let view = started_at_once(&mut fixture, &hand);
    assert_eq!(start_note(&view), "Working");
    for (later, shown) in [
        (12 * 60 + 30, "Working · 12 min"),
        (3_600 + 5 * 60, "Working · 1 h 5 min"),
    ] {
        delivery.lock().unwrap().now = Some(NOW + later);
        fixture.computers.refresh().expect("refresh");
        assert_eq!(start_note(&fixture.render()), shown);
    }
}

/// The conversation that started a run still shows its card after a
/// relaunch, once the run is done, with the outcome the phone read
/// (#10118).
#[test]
fn the_chat_that_started_coder_keeps_its_card_after_a_relaunch() {
    use nostr::activity_summary::Phase;
    let dir = tempfile::tempdir().expect("temp dir");
    let kept = dir.path().join("basic");
    let delivery = at_once();
    let hand = Hand::default();
    let mut first = Fixture::in_dir(
        Imaging {
            inner: Synthetic::fixture(Platform::Phone, now),
            delivery: delivery.clone(),
        },
        dir,
    )
    .answered_and_kept(&hand, &kept);
    started_at_once(&mut first, &hand);
    delivery.lock().unwrap().phase = Some(Phase::Completed);
    first.computers.refresh().expect("refresh");
    let view = first.render();
    assert!(
        texts(&view)
            .iter()
            .any(|t| t == "Coder on Studio Mac: Done"),
        "{:?}",
        texts(&view)
    );
    // The app ends; its stores stay.
    let Fixture { _dir: dir, .. } = first;
    let mut again = Fixture::in_dir(
        Imaging {
            inner: Synthetic::fixture(Platform::Phone, now),
            delivery: delivery.clone(),
        },
        dir,
    )
    .answered_and_kept(&Hand::default(), &kept);
    let list = again.list();
    let talk = conversation_row(&list);
    let view = again.tap(&talk);
    assert!(node(&view, "coder-start").is_some(), "{:?}", keys(&view));
    assert!(
        texts(&view)
            .iter()
            .any(|t| t == "Coder on Studio Mac: Done"),
        "{:?}",
        texts(&view)
    );
    assert!(node(&view, "coder-start-stop").is_none());
}

/// A finished run's card says what Coder did in one line, as the phone
/// read it from the task's chat, and still does after a relaunch
/// (#10118).
#[test]
fn a_finished_start_shows_its_outcome() {
    use nostr::activity_summary::Phase;
    let dir = tempfile::tempdir().expect("temp dir");
    let kept = dir.path().join("basic");
    let list = dir.path().join("coder-list");
    let delivery = at_once();
    let hand = Hand::default();
    let mut first = Fixture::in_dir(
        Imaging {
            inner: Synthetic::fixture(Platform::Phone, now),
            delivery: delivery.clone(),
        },
        dir,
    )
    .answered_and_kept(&hand, &kept);
    started_at_once(&mut first, &hand);
    delivery.lock().unwrap().phase = Some(Phase::Completed);
    first.computers.refresh().expect("refresh");
    first.render();
    let Fixture { _dir: dir, .. } = first;
    // The outcome the phone read from the task's chat, as it keeps it.
    let secret = secp256k1::SecretKey::from_byte_array([0x11; 32]).expect("key");
    let mut store = Store::open(Cache::open(&list, &secret).ok());
    let started = store
        .list
        .started
        .values_mut()
        .next()
        .expect("the start is kept");
    started.outcome = Some("Uploaded build 42 to TestFlight.".into());
    store.save();
    drop(store);
    let mut again = Fixture::in_dir(
        Imaging {
            inner: Synthetic::fixture(Platform::Phone, now),
            delivery: delivery.clone(),
        },
        dir,
    )
    .answered_and_kept(&Hand::default(), &kept);
    let list = again.list();
    let talk = conversation_row(&list);
    let view = again.tap(&talk);
    assert_eq!(start_note(&view), "Uploaded build 42 to TestFlight.");
}

/// A finished run whose chat this phone cannot read, as on a computer it
/// holds no chat pairing for: the card says Done once, not "Done · Done",
/// and Open Coder says the messages can't be read instead of a spinner
/// that never ends (#10118).
#[test]
fn a_finished_start_on_an_unlinked_computer_says_so() {
    use nostr::activity_summary::Phase;
    let delivery = at_once();
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    started_at_once(&mut fixture, &hand);
    delivery.lock().unwrap().phase = Some(Phase::Completed);
    fixture.computers.refresh().expect("refresh");
    let view = fixture.render();
    assert!(
        texts(&view)
            .iter()
            .any(|t| t == "Coder on Studio Mac: Done"),
        "{:?}",
        texts(&view)
    );
    assert!(
        node(&view, "coder-start-note").is_none(),
        "{:?}",
        texts(&view)
    );
    let view = fixture.tap("coder-start-open");
    assert!(
        node(&view, "coder-transcript-working").is_none(),
        "{:?}",
        keys(&view)
    );
    let said = node(&view, "coder-unlinked")
        .and_then(|note| note["element"]["props"]["value"].as_str())
        .unwrap_or_else(|| panic!("no unlinked note in {:?}", keys(&view)));
    assert_eq!(
        said,
        "This phone can't read Coder's messages on Studio Mac yet."
    );
}

/// An offline computer, even one whose last presence said it starts Coder
/// at once, starts nothing: the reply says it is offline, as before
/// (#10101).
#[test]
fn an_offline_computer_leaves_the_offer_unchanged() {
    use coder_host::access::protocol::CODER_START_AT_ONCE;
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery {
        advertise: Some(vec!["task-create", CODER_START_AT_ONCE]),
        only: Some(vec![0xa4]),
        ..Delivery::default()
    }));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.say("Fix the flaky test in my repo");
    hand.judge(crate::basic_coder::Lane::Computer);
    hand.say("Coder can take this on.", true);
    let view = fixture.render();
    assert!(delivery.lock().unwrap().engines.is_empty());
    assert!(node(&view, "coder-start").is_none());
    assert!(
        texts(&view).iter().any(|t| t == "Old laptop is offline."),
        "{:?}",
        texts(&view)
    );
}

/// A reply that answered the question (a route other than the dispatch,
/// with no `run_coder` offer), even on the computer lane, starts nothing,
/// and a coding reply read back from an earlier session never starts
/// Coder: only a reply to a message sent now does (#10101, #10079).
#[test]
fn an_answered_reply_starts_nothing() {
    use coder_host::access::protocol::CODER_START_AT_ONCE;
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery {
        advertise: Some(vec!["task-create", CODER_START_AT_ONCE]),
        ..Delivery::default()
    }));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.say("What's my working directory?");
    {
        let replies = hand.replies.lock().unwrap();
        let (_, reply) = replies.last().unwrap();
        let mut reply = reply.lock().unwrap();
        reply.lane = Some(crate::basic_coder::Lane::Computer);
        reply.meta.route = Some("meta".into());
    }
    hand.say("Your working directory is ~/code/openagents.", true);
    let view = fixture.render();
    fixture.render();
    assert!(delivery.lock().unwrap().engines.is_empty());
    assert!(node(&view, "coder-run").is_none(), "{:?}", keys(&view));
    assert!(node(&view, "coder-start").is_none());
}

#[test]
fn run_coder_delivers_the_drafts_images_and_keeps_them_when_refused_or_lost() {
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery::default()));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.say("Fix the layout of my settings page");
    hand.judge(crate::basic_coder::Lane::Computer);
    hand.say("That needs a computer.", true);
    let pixels: Vec<u8> = (0..64u32 * 48 * 4).map(|i| (i % 251) as u8).collect();
    let image = openagents_chat_app::attachments::Image::pixels(64, 48, pixels).unwrap();
    let bytes = image.bytes.as_ref().clone();
    fixture.coder.set_attachments(true);
    fixture.coder.attach_image("Settings.png", bytes.clone());
    let chat = fixture.render();
    assert_eq!(image_surfaces(&chat), 1);
    // A computer that refuses: no task, and the draft keeps its image.
    delivery.lock().unwrap().refuse = true;
    let chat = fixture.tap("coder-run");
    assert!(fixture.coder.open_task().is_none());
    assert_eq!(image_surfaces(&chat), 1);
    assert!(
        texts(&chat)
            .iter()
            .any(|t| t.contains("couldn't accept this")),
        "{:?}",
        texts(&chat)
    );

    // An answer that never arrives: the draft keeps its image too.
    {
        let mut delivery = delivery.lock().unwrap();
        delivery.refuse = false;
        delivery.lose_answer = true;
    }
    let chat = fixture.tap("coder-run");
    assert!(fixture.coder.open_task().is_none());
    assert_eq!(image_surfaces(&chat), 1);

    // Accepted: the computer holds the exact bytes, and the draft lets go.
    delivery.lock().unwrap().lose_answer = false;
    fixture.tap("coder-run");
    assert!(fixture.coder.open_task().is_some());
    let delivery = delivery.lock().unwrap();
    assert!(delivery.chunks >= 1);
    let [images] = delivery.created.as_slice() else {
        panic!("one task with images")
    };
    let [(reference, held)] = images.as_slice() else {
        panic!("one image")
    };
    assert_eq!(held, &bytes);
    assert_eq!(reference.media_type, "image/png");
    assert_eq!(reference.digest, coder_host::access::media::digest(&bytes));
    // The lost answer's task is still on the computer: a retry after a lost
    // answer starts a second task until the client keeps its submissions
    // (G05). The draft let its images go only on the accepted start.
    assert_eq!(delivery.created.len(), 1);
}

/// One send with words and a screenshot (#10070): only the words reach
/// the router; the screenshot stays on the phone, bound to the message, and
/// when the reply offers Coder, Run Coder carries its exact bytes to the
/// computer.
#[test]
fn one_send_with_words_and_a_screenshot_runs_coder_with_its_bytes() {
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery::default()));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.list();
    fixture.tap("coder-back");
    let pixels: Vec<u8> = (0..48u32 * 32 * 4).map(|i| (i % 241) as u8).collect();
    let image = openagents_chat_app::attachments::Image::pixels(48, 32, pixels).unwrap();
    let bytes = image.bytes.as_ref().clone();
    fixture.coder.set_attachments(true);
    fixture.coder.attach_image("Layout.png", bytes.clone());
    let chat = fixture.say("fix this layout bug");
    assert_eq!(hand.asked(), vec![vec!["fix this layout bug".to_owned()]]);
    assert_eq!(
        image_surfaces(&chat),
        1,
        "the screenshot stays on the phone"
    );
    assert!(delivery.lock().unwrap().chunks == 0);
    hand.judge(crate::basic_coder::Lane::Computer);
    hand.say("That needs a computer.", true);
    let chat = fixture.render();
    assert!(
        !texts(&chat)
            .iter()
            .any(|t| t == openagents_chat_app::attachments::ONLY_TO_CODER)
    );
    assert_eq!(image_surfaces(&chat), 1);
    fixture.tap("coder-run");
    assert!(fixture.coder.open_task().is_some());
    let delivery = delivery.lock().unwrap();
    let [images] = delivery.created.as_slice() else {
        panic!("one task with images")
    };
    let [(reference, held)] = images.as_slice() else {
        panic!("one image")
    };
    assert_eq!(held, &bytes);
    assert_eq!(reference.digest, coder_host::access::media::digest(&bytes));
}

/// A computer's own thread and a Coder task's chat carry words only: a
/// send there with images keeps the draft and says why.
#[test]
fn a_words_only_route_keeps_the_images_and_says_why() {
    let mut drafts = openagents_chat_app::attachments::Drafts::default();
    drafts
        .add(
            "task:a:b",
            openagents_chat_app::attachments::Image::pixels(1, 1, vec![0; 4]).unwrap(),
        )
        .unwrap();
    assert_eq!(
        drafts.text_only_refusal("task:a:b"),
        Some(openagents_chat_app::attachments::TEXT_ONLY_ROUTE)
    );
    assert!(!openagents_chat_app::attachments::TEXT_ONLY_ROUTE.contains("Hosted chat"));
}

/// An image that isn't PNG or JPEG, or is too large, is refused before
/// anything is sent.
#[test]
fn unsupported_and_oversized_images_are_refused_before_send() {
    let mut drafts = openagents_chat_app::attachments::Drafts::default();
    let mut image = openagents_chat_app::attachments::Image::pixels(1, 1, vec![0; 4]).unwrap();
    image.bytes = std::sync::Arc::new(b"GIF89a not a png".to_vec());
    drafts.add("talk:a", image.clone()).unwrap();
    assert!(
        drafts
            .uploads("talk:a")
            .unwrap_err()
            .contains("PNG or JPEG")
    );
    let mut drafts = openagents_chat_app::attachments::Drafts::default();
    let mut big = b"\x89PNG\r\n\x1a\n".to_vec();
    big.resize(openagents_chat_app::attachments::MAX_IMAGE_BYTES + 1, 0);
    image.bytes = std::sync::Arc::new(big);
    drafts.add("talk:b", image).unwrap();
    assert!(drafts.uploads("talk:b").unwrap_err().contains("8 MiB"));
    assert_eq!(drafts.get("talk:b").len(), 1);
}

/// The phone is text only (#10093): the chat, new or open, has no attach
/// control, Attach is never asked of the host, and an image the host hands
/// it anyway (a paste, a drop, a share) is dropped with no notice.
#[test]
fn the_phone_chat_is_text_only_with_no_attach_control() {
    const { assert!(!crate::coder_tab::ATTACHMENTS_ENABLED) };
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    assert!(!fixture.coder.attachments_enabled());
    fixture.list();
    let screen = fixture.tap("coder-back");
    for key in ["coder-attach", "coder-attachments"] {
        assert!(node(&screen, key).is_none(), "{key} on a new chat");
    }
    assert!(composer_of(&screen)["token"].is_string());
    let png = openagents_chat_app::attachments::Image::pixels(3, 2, vec![200; 24]).unwrap();
    fixture
        .coder
        .attach_image("Photo.png", png.bytes.as_ref().clone());
    fixture
        .coder
        .attach_image("bad.png", b"not an image".to_vec());
    let screen = fixture.render();
    assert_eq!(image_surfaces(&screen), 0);
    assert!(fixture.coder.take_go().is_none());
    assert!(
        !texts(&screen)
            .iter()
            .any(|text| text.contains("PNG or JPEG")),
        "{:?}",
        texts(&screen)
    );
    // An open conversation has none either.
    let screen = fixture.say("Hello");
    for key in ["coder-attach", "coder-attachments"] {
        assert!(node(&screen, key).is_none(), "{key} in a conversation");
    }
    assert_eq!(hand.asked(), vec![vec!["Hello".to_owned()]]);
}

/// A draft that holds images when attachments are off, as one restored
/// from before #10093, shows none and sends its words only: no images go
/// with the message or with Run Coder, and no line about them shows.
#[test]
fn a_restored_draft_with_images_sends_its_words_only() {
    let delivery = std::sync::Arc::new(std::sync::Mutex::new(Delivery::default()));
    let hand = Hand::default();
    let mut fixture = Fixture::new(Imaging {
        inner: Synthetic::fixture(Platform::Phone, now),
        delivery: delivery.clone(),
    })
    .answered_by(&hand);
    fixture.list();
    fixture.tap("coder-back");
    fixture.coder.set_attachments(true);
    let image = openagents_chat_app::attachments::Image::pixels(8, 8, vec![90; 256]).unwrap();
    fixture
        .coder
        .attach_image("Layout.png", image.bytes.as_ref().clone());
    assert_eq!(image_surfaces(&fixture.render()), 1);
    fixture.coder.set_attachments(false);
    let chat = fixture.render();
    assert_eq!(image_surfaces(&chat), 0);
    assert!(node(&chat, "coder-attach").is_none());
    let chat = fixture.say("fix this layout bug");
    assert_eq!(hand.asked(), vec![vec!["fix this layout bug".to_owned()]]);
    assert_eq!(image_surfaces(&chat), 0);
    let lines = [
        openagents_chat_app::attachments::HELD_FOR_CODER,
        openagents_chat_app::attachments::ONLY_TO_CODER,
        openagents_chat_app::attachments::TEXT_ONLY_ROUTE,
    ];
    assert!(
        !texts(&chat)
            .iter()
            .any(|text| lines.contains(&text.as_str()))
    );
    hand.judge(crate::basic_coder::Lane::Computer);
    hand.say("That needs a computer.", true);
    fixture.tap("coder-run");
    assert!(fixture.coder.open_task().is_some());
    let delivery = delivery.lock().unwrap();
    assert_eq!(delivery.chunks, 0);
    let [images] = delivery.created.as_slice() else {
        panic!("one task")
    };
    assert!(images.is_empty(), "the task carries no images");
}

#[test]
fn a_task_in_an_unknown_state_can_be_stopped_from_the_phone() {
    use coder_host::CommandAction;
    use nostr::activity_summary::{Attention, Phase};
    // The computer ends a stuck task whose process is gone when it is
    // stopped (#10124), so the phone offers Stop for it.
    let (mut fixture, script, chat) = Fixture::scripted(Phase::Unknown, Attention::None);
    assert!(node(&chat, "coder-stop").is_some(), "{:?}", keys(&chat));
    fixture.tap("coder-stop");
    assert!(
        script
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|(action, _, _)| *action == CommandAction::Interrupt),
        "{:?}",
        script.lock().unwrap().commands
    );
}

/// The release gate (`docs/mobile/1.0-audit.md`): with the Gym hidden, a
/// new chat offers no Gym or XP question, a reply's Gym and game screens
/// and Gym follow-ups show no chip, Train Coder and Profile do nothing, and
/// the tab never leaves the chat for the Gym's intro or menu.
#[test]
fn a_hidden_gym_shows_no_entry_point_in_chat() {
    let hand = Hand::default();
    let mut fixture =
        Fixture::new(NoComputers(Synthetic::fixture(Platform::Phone, now))).answered_by(&hand);
    fixture.coder.hide_gym();
    let screen = fixture.render();
    let shown = suggestions(&screen);
    assert_eq!(shown.len(), 4, "{shown:?}");
    assert!(
        shown
            .iter()
            .all(|(key, _)| !key.contains("gym.") && !key.contains("eval.")),
        "{shown:?}"
    );

    // Train Coder and Profile from Account do nothing.
    fixture.coder.train_coder();
    fixture.coder.show_profile();
    fixture.render();
    let gym = fixture.coder.gym_view();
    assert_eq!(gym.screen, "chat");
    assert!(gym.first_run.is_none() && gym.sheet.is_none());

    // A reply offering the Gym's board, Playtest, and Wallet, with a Gym
    // follow-up: only Wallet and the other follow-up show.
    fixture.say("What can I do here?");
    hand.route(&[
        json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "verse.gym",
            "label": "Walk to the Gym"}),
        json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "account.playtest",
            "label": "Playtest"}),
        json!({"v": 2, "type": "offer", "offer": "open_screen", "screen": "wallet",
            "label": "Wallet"}),
        json!({"v": 2, "type": "result", "text": "Chat, pay, and run Coder.",
            "model": "bank:chat-answers-v1", "tier": "canned", "answer": "meta.capabilities@1",
            "route": "meta", "bank": "chat-answers-v1@9f2c",
            "followups": [{"id": "gym.test", "label": "Test a plugin"},
                {"id": "meta.coder", "label": "What is Coder?"}]}),
    ]);
    hand.say("Chat, pay, and run Coder.", true);
    let chat = fixture.render();
    let labels: Vec<String> = keys(&chat)
        .iter()
        .filter(|key| key.starts_with("coder-screen-") || key.starts_with("coder-followup-"))
        .map(|key| {
            node(&chat, key).unwrap()["element"]["props"]["label"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        labels,
        ["Open Wallet", "What is Coder?"],
        "{:?}",
        keys(&chat)
    );
}
