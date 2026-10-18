//! The upgrade against a fixture host (#12056): a fake checkout and build,
//! the real hand-over files, the real app-side watcher, swap, and reopen,
//! over a scripted owner, with real processes standing in for the keepers
//! that hold running chats.

use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use openagents_host_upgrade::app::{self, Action, Watcher};
use openagents_host_upgrade::layout::{BUNDLE_NAME, COMMIT_FILE, Layout};
use openagents_host_upgrade::quiet::Activity;
use openagents_host_upgrade::record::{self, LogEntry, Outcome, Pending};
use openagents_host_upgrade::{Host, Request, upgrade};

const OLD: &str = "0000000000aaaaaaaaaa";
const NEW: &str = "1111111111bbbbbbbbbb";

/// One second of the scripted owner: are they typing, on a call?
type Script = dyn Fn(u64) -> Activity;

struct FixtureApp {
    running: bool,
    /// Processes holding running chats, like keepers: they must outlive
    /// the app's restart.
    keepers: Vec<(String, Child)>,
    selected_chat: String,
    notices: Vec<(u64, String)>,
    cancels: Vec<u64>,
    restarted_at: Option<u64>,
}

struct FixtureHost {
    data_dir: PathBuf,
    bundle: PathBuf,
    work: PathBuf,
    main: String,
    checkout: String,
    build: Result<(), String>,
    verify: Result<(), String>,
    app: FixtureApp,
    script: Box<Script>,
    calls: Vec<String>,
    clock_ms: u64,
}

impl FixtureHost {
    fn new(dir: &Path, script: Box<Script>) -> FixtureHost {
        let bundle = dir.join("macos-dev").join(BUNDLE_NAME);
        make_bundle(&bundle, OLD);
        let keepers = ["foreman-9fa93200", "agent-chat-858dc0d9"]
            .into_iter()
            .map(|chat| {
                let child = std::process::Command::new("/bin/sleep")
                    .arg("120")
                    .spawn()
                    .unwrap();
                (chat.to_owned(), child)
            })
            .collect();
        FixtureHost {
            data_dir: dir.join("data"),
            work: dir.join("macos-dev").join(".host-upgrade"),
            bundle,
            main: NEW.into(),
            checkout: OLD.into(),
            build: Ok(()),
            verify: Ok(()),
            app: FixtureApp {
                running: true,
                keepers,
                selected_chat: "foreman-9fa93200".into(),
                notices: Vec::new(),
                cancels: Vec::new(),
                restarted_at: None,
            },
            script,
            calls: Vec::new(),
            clock_ms: 1_760_000_000_000,
        }
    }

    fn state_dir(&self) -> PathBuf {
        openagents_host_upgrade::layout::state_dir(&self.data_dir)
    }

    fn keepers_alive(&mut self) -> bool {
        self.app
            .keepers
            .iter_mut()
            .all(|(_, child)| matches!(child.try_wait(), Ok(None)))
    }

    /// The app's side, second by second: the watcher, the swap, the quit,
    /// and the reopen.
    fn run_app(&mut self) {
        let start = Instant::now();
        let mut watcher = Watcher::new(&self.data_dir, self.bundle.clone());
        for second in 0..3600 {
            let activity = (self.script)(second);
            // Activity times are seconds on the same clock.
            let activity = Activity {
                last_input: activity
                    .last_input
                    .map(|at| start + at.duration_since(epoch())),
                voice_live: activity.voice_live,
            };
            match watcher.poll(start + Duration::from_secs(second), &activity) {
                Action::None => {}
                Action::Notice(text) => self.app.notices.push((second, text)),
                Action::Cancel(_) => self.app.cancels.push(second),
                Action::Restart(pending) => {
                    assert!(
                        !activity.voice_live,
                        "never restarts during a call (second {second})"
                    );
                    let link = format!("openagents://open/chat/{}", self.app.selected_chat);
                    let running: Vec<String> = self
                        .app
                        .keepers
                        .iter()
                        .map(|(chat, _)| chat.clone())
                        .collect();
                    app::begin_restart(
                        watcher.state_dir(),
                        &pending,
                        Some(link),
                        &running,
                        self.clock_ms,
                    )
                    .unwrap();
                    // The app quits; keepers are not its to stop.
                    self.app.running = false;
                    self.app.selected_chat.clear();
                    self.app.restarted_at = Some(second);
                    // The new app starts and reports.
                    let (_, chat) = app::reopened(&self.data_dir, &self.bundle, self.clock_ms)
                        .expect("the new app reports the upgrade");
                    self.app.running = true;
                    self.app.selected_chat = chat
                        .unwrap()
                        .trim_start_matches("openagents://open/chat/")
                        .to_owned();
                    return;
                }
            }
        }
    }
}

impl Drop for FixtureHost {
    fn drop(&mut self) {
        for (_, child) in &mut self.app.keepers {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Host for FixtureHost {
    fn installed_commit(&mut self) -> Option<String> {
        Layout::bundle_commit(&self.bundle)
    }
    fn checkout_commit(&mut self) -> Option<String> {
        Some(self.checkout.clone())
    }
    fn fetch_main(&mut self) -> Result<String, String> {
        self.calls.push("fetch".into());
        Ok(self.main.clone())
    }
    fn checkout(&mut self, commit: &str) -> Result<(), String> {
        self.calls.push(format!("checkout {}", &commit[..4]));
        self.checkout = commit.to_owned();
        Ok(())
    }
    fn build(&mut self, id: &str) -> Result<PathBuf, String> {
        self.calls.push("build".into());
        self.build.clone()?;
        let staged = self.work.join(id).join(BUNDLE_NAME);
        make_bundle(&staged, &self.checkout);
        Ok(staged)
    }
    fn verify(&mut self, _: &Path, _: &str) -> Result<(), String> {
        self.calls.push("verify".into());
        self.verify.clone()
    }
    fn bundle_paths(&mut self) -> (PathBuf, PathBuf) {
        (
            self.bundle.clone(),
            self.work.join("previous").join(BUNDLE_NAME),
        )
    }
    fn app_running(&mut self) -> bool {
        self.app.running
    }
    fn hand_over(&mut self, pending: &Pending) -> Result<(), String> {
        self.calls.push("hand over".into());
        record::write_pending(&self.state_dir(), pending).map_err(|e| e.to_string())?;
        self.run_app();
        Ok(())
    }
    fn install_now(&mut self, _: &Pending) -> Result<(), String> {
        self.calls.push("install now".into());
        Err("the fixture app is always running".into())
    }
    fn wait_for_outcome(&mut self, pending: &Pending) -> Option<LogEntry> {
        record::outcome_of(&self.state_dir(), &pending.id)
    }
    fn log(&mut self, entry: &LogEntry) {
        record::log(&self.state_dir(), entry).unwrap();
    }
    fn now_ms(&mut self) -> u64 {
        self.clock_ms
    }
}

fn epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// Owner activity: last keystroke at `second` (on the script's clock).
fn typed_at(second: u64) -> Option<Instant> {
    Some(epoch() + Duration::from_secs(second))
}

fn make_bundle(bundle: &Path, commit: &str) {
    let resources = bundle.join("Contents").join("Resources");
    std::fs::create_dir_all(&resources).unwrap();
    std::fs::create_dir_all(bundle.join("Contents").join("MacOS")).unwrap();
    std::fs::write(resources.join(COMMIT_FILE), format!("{commit}\n")).unwrap();
    std::fs::write(Layout::binary(bundle), "#!/bin/sh\n").unwrap();
}

fn request(id: &str) -> Request {
    Request {
        id: id.into(),
        requested_by: "9fa93200-c1e1-65b8-9e06-94652a8904a3".into(),
        reason: "pick up #12056".into(),
        force: false,
    }
}

#[test]
fn a_failed_build_leaves_the_old_app_running() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = FixtureHost::new(dir.path(), Box::new(|_| Activity::default()));
    host.build = Err("the build failed: error[E0425]: cannot find value `x`".into());

    let entry = upgrade(&mut host, &request("u1"));

    assert_eq!(entry.outcome, Outcome::BuildFailed);
    assert!(entry.detail.contains("error[E0425]"), "{}", entry.detail);
    assert!(entry.detail.contains("wasn't touched"), "{}", entry.detail);
    assert_eq!(
        (entry.from.as_deref(), entry.to.as_deref()),
        (Some(OLD), Some(NEW))
    );
    // Nothing was handed over, swapped, or restarted.
    assert_eq!(
        host.calls,
        ["fetch", "checkout 1111", "build", "checkout 0000"]
    );
    assert_eq!(host.checkout, OLD, "the checkout is put back");
    assert!(host.app.running && host.app.restarted_at.is_none());
    assert_eq!(Layout::bundle_commit(&host.bundle).as_deref(), Some(OLD));
    assert!(record::read_pending(&host.state_dir()).is_none());
    assert!(host.keepers_alive());
    // The log has it, with the Chicago time.
    let log = record::read_log(&host.state_dir());
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], entry);
    assert!(log[0].at.ends_with("CDT") || log[0].at.ends_with("CST") || log[0].at.ends_with("UTC"));
}

#[test]
fn a_failed_check_leaves_the_old_app_running() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = FixtureHost::new(dir.path(), Box::new(|_| Activity::default()));
    host.verify = Err("the new app's engine didn't answer within 120 s".into());

    let entry = upgrade(&mut host, &request("u2"));

    assert_eq!(entry.outcome, Outcome::VerifyFailed);
    assert_eq!(host.checkout, OLD);
    assert!(!host.calls.contains(&"hand over".to_owned()));
    assert_eq!(Layout::bundle_commit(&host.bundle).as_deref(), Some(OLD));
    assert!(host.app.running && host.keepers_alive());
}

#[test]
fn the_restart_waits_for_a_quiet_moment_and_chats_keep_running() {
    let dir = tempfile::tempdir().unwrap();
    // The owner types until 0:40, comes back at 1:15 (while the first notice
    // is up), then takes a voice call from 1:20 to 6:40.
    let script = |second: u64| Activity {
        last_input: match second {
            0..=40 => typed_at(second - second % 5),
            41..=74 => typed_at(40),
            _ => typed_at(75),
        },
        voice_live: (80..400).contains(&second),
    };
    let mut host = FixtureHost::new(dir.path(), Box::new(script));
    let before = host.app.selected_chat.clone();

    let entry = upgrade(&mut host, &request("u3"));

    // Quiet at 1:10 (30 s after typing) brings the first notice; typing at
    // 1:15 takes it down; the call holds everything; quiet again at 6:40
    // brings a second notice and the restart ten seconds later.
    let notices: Vec<u64> = host.app.notices.iter().map(|(at, _)| *at).collect();
    assert_eq!(notices, [70, 400]);
    assert_eq!(host.app.cancels, [75]);
    assert_eq!(host.app.restarted_at, Some(410));
    assert!(
        host.app.notices[0]
            .1
            .contains("restarts in 10 seconds to install an upgrade (1111111111)"),
        "{}",
        host.app.notices[0].1
    );

    // Installed: the new bundle is in, the old one kept for a roll back.
    assert_eq!(entry.outcome, Outcome::Installed);
    assert_eq!(
        (entry.from.as_deref(), entry.to.as_deref()),
        (Some(OLD), Some(NEW))
    );
    assert_eq!(Layout::bundle_commit(&host.bundle).as_deref(), Some(NEW));
    let (_, previous) = host.bundle_paths();
    assert_eq!(Layout::bundle_commit(&previous).as_deref(), Some(OLD));
    assert_eq!(
        app::reopened_line(&entry),
        "Reopened after an upgrade from 0000000000 to 1111111111. Your chats kept running."
    );

    // The chats' processes outlived the app, the app reopened on the same
    // chat, and their queues run on.
    assert!(
        host.keepers_alive(),
        "the running chats survive the restart"
    );
    assert!(host.app.running);
    assert_eq!(host.app.selected_chat, before);
    assert_eq!(
        record::take_resume_queues(&host.state_dir()),
        ["foreman-9fa93200", "agent-chat-858dc0d9"]
    );

    // Logged once, by the reopened app; nothing is left pending.
    let log = record::read_log(&host.state_dir());
    assert_eq!(log, [entry]);
    assert!(record::read_pending(&host.state_dir()).is_none());
}

#[test]
fn main_already_running_is_not_rebuilt() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = FixtureHost::new(dir.path(), Box::new(|_| Activity::default()));
    host.main = OLD.into();

    let entry = upgrade(&mut host, &request("u4"));

    assert_eq!(entry.outcome, Outcome::UpToDate);
    assert_eq!(host.calls, ["fetch"]);

    // Forced, it rebuilds and installs the same commit.
    let mut forced = request("u5");
    forced.force = true;
    let entry = upgrade(&mut host, &forced);
    assert_eq!(entry.outcome, Outcome::Installed);
    assert_eq!(host.app.restarted_at, Some(10));
}
