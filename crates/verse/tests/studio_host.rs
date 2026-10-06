//! Everglade's Agent Studio against a scratch host (#10492): a host with
//! its access store, host root, control socket, and task store under a
//! temporary directory, and its studio over a scratch repository. Nothing
//! here reaches the person's own host, home, or relays.
//!
//! The studio connects as Everglade does, through the live source over the
//! host's control socket, and acts through the panels' controllers:
//!
//! - A goal typed at the console appears on the Task Wall as its lead's
//!   task, planned.
//! - An answer typed at the podium resumes the goal: its plan's tasks are
//!   released.
//! - A merge decision from the merge station at a review the worktree moved
//!   past is refused as `stale`, and the panel says so; a decision at the
//!   reloaded review is taken.
//!
//! No engine runs, so the test stands in for what an engine and the host's
//! auto-start sweep would do: it cancels the lead's task, runs the
//! coordinator's pass, and commits a change in a task's worktree. Every
//! task the test creates is archived when it ends, pass or fail.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder::task::studio::{self as coordinator, Role, Seat, Studio as Coordinator};
use coder::task::{Status, Store, studio_sim};
use coder_access::review::TaskReview;
use coder_access::studio::{DecisionKind, GoalStatus, TaskStatus, Verdict, View};
use coder_access::{Code, Operation, Outcome, RelayPolicy, Right};
use coder_host::config::{Config, Control, Iroh};
use secp256k1::SecretKey;
use verse::panels::studio::{Controller, Effect, status_text};
use verse::panels::{Intent, Key, Panel};
use verse::zones::everglade::studio::intents;
use verse::zones::everglade::studio::live::{ControlSocket, Live, Transport};
use verse::zones::everglade::studio::{PanelKind, Studio};

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
/// How long the studio may take to show what a step did.
const WAIT: Duration = Duration::from_secs(60);
/// The goal the console starts.
const GOAL: &str = "Greet with Hello, studio and document the greeting.";

/// A scratch host and everything it keeps.
struct Host {
    temp: tempfile::TempDir,
    runtime: tokio::runtime::Runtime,
    running: Option<coder_host::Running>,
    _relay: tokio::task::JoinHandle<()>,
    socket: PathBuf,
    store: PathBuf,
    checkout: PathBuf,
}

/// Seats a lead and two workers in the task store at `store`.
fn seat(store: &Path) {
    let mut studio = Coordinator::open(store).expect("the coordinator opens");
    let route = coordinator::parse_route("codex:gpt-6-luna").expect("a route");
    for (desk, (name, role)) in [
        ("lead", Role::Lead),
        ("ada", Role::Worker),
        ("grace", Role::Worker),
    ]
    .into_iter()
    .enumerate()
    {
        studio
            .set_seat(Seat {
                name: name.into(),
                role,
                route: route.clone(),
                look: "default".into(),
                desk: desk as u32,
            })
            .expect("a seat");
    }
}

fn host() -> Host {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("a runtime");
    let temp = tempfile::tempdir().expect("a scratch directory");
    let fixture = studio_sim::Fixture::create(&temp.path().join("fixture")).expect("a repository");
    let checkout = std::fs::canonicalize(&fixture.checkout).expect("the checkout");
    let store = temp.path().join("tasks");
    seat(&store);
    let socket = temp.path().join("c/control.sock");
    let workspaces = BTreeMap::from([("scratch".to_owned(), checkout.clone())]);
    let (running, relay) = runtime.block_on(async {
        let (relay, task, _) = relay::start().await;
        let access = temp.path().join("access");
        let owner = SecretKey::new(&mut secp256k1::rand::rng());
        coder_host::access::host::Host::new(&access, POLICY)
            .init(&coder_host::reach::pubkey(&owner))
            .expect("the access store");
        let mut config = Config::new(access, vec![relay], 3);
        config.policy = POLICY;
        config.iroh = Some(Iroh::loopback());
        config.control = Some(Control {
            path: socket.clone(),
            root: temp.path().join("host"),
            autostart: None,
            tasks: store.clone(),
            uid: coder_host::control::own_uid(),
        });
        config.workspaces = workspaces.clone();
        let tasks = Arc::new(coder::task::remote::Inbox::new(store.clone(), workspaces))
            as Arc<dyn coder_host::Tasks>;
        let running = coder_host::start(config, tasks).await.expect("the host");
        (running, task)
    });
    Host {
        temp,
        runtime,
        running: Some(running),
        _relay: relay,
        socket,
        store,
        checkout,
    }
}

impl Host {
    /// Cancels every task still open and archives every task in the store,
    /// through the host, as `task.cancel` and `task.archive` do.
    fn archive(&self) -> Result<usize, String> {
        let mut control = ControlSocket::new(self.socket.clone());
        let tasks = Store::open(&self.store)
            .and_then(|store| store.list())
            .map_err(|error| format!("{error:?}"))?;
        for task in &tasks {
            if matches!(
                task.status,
                Status::Queued | Status::Running | Status::CancelRequested
            ) {
                control
                    .call(
                        &intents::mint(),
                        &Operation::CancelTask {
                            task: task.task_id.clone(),
                            revision: task.revision,
                            reason: "The studio test ended.".into(),
                        },
                    )
                    .map_err(|error| format!("cancel {}: {error:?}", task.task_id))?;
            }
            control
                .call(
                    &intents::mint(),
                    &Operation::ArchiveTask {
                        task: task.task_id.clone(),
                    },
                )
                .map_err(|error| format!("archive {}: {error:?}", task.task_id))?;
        }
        Ok(tasks.len())
    }

    fn shutdown(&mut self) {
        if let Some(running) = self.running.take() {
            self.runtime.block_on(running.shutdown());
        }
    }

    /// The host's coordinator pass, which its auto-start sweep runs.
    fn reconcile(&self) {
        let mut tasks = Store::open(&self.store).expect("the store");
        let mut studio = Coordinator::open(&self.store).expect("the coordinator");
        studio
            .reconcile(&mut tasks, intents::now(), &|_: &str| None)
            .expect("the coordinator's pass");
    }
}

/// Polls `studio` until `test` holds, or fails naming `what`.
fn drive(studio: &mut Studio, what: &str, mut test: impl FnMut(&mut Studio) -> bool) {
    let deadline = Instant::now() + WAIT;
    loop {
        studio.poll(0.05, &[]);
        if test(studio) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the studio never showed {what}; the host last said {:?}",
            studio.status()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Waits for the host's answer to `ticket`.
fn answered(studio: &mut Studio, ticket: u64) -> Result<Outcome, coder_access::Error> {
    drive(studio, "the answer", |studio| {
        studio
            .status()
            .is_some_and(|answer| answer.ticket == ticket)
    });
    studio.status().expect("an answer").result.clone()
}

fn type_into(panel: &mut Panel, text: &str) {
    for ch in text.chars() {
        assert!(panel.key(Key::Char(ch)).is_none());
    }
}

fn fill(
    studio: &Studio,
    controller: &mut Controller,
    panel: &mut Panel,
    review: Option<&TaskReview>,
) {
    controller.fill(
        panel,
        studio.revision(),
        studio.view(),
        review,
        &studio.rights(),
        studio.status(),
    );
}

/// Runs `intent` on the panel and sends what it asks for, as Everglade's
/// app does. Returns each sent intent's ticket.
fn press(
    studio: &mut Studio,
    controller: &mut Controller,
    panel: &mut Panel,
    intent: Intent,
    review: Option<&TaskReview>,
) -> Vec<u64> {
    let effects = controller.intent(intent, panel, studio.view(), review);
    let mut tickets = Vec::new();
    for effect in effects {
        if let Effect::Send(action) = effect {
            tickets.push(
                studio
                    .send(action.operation(intents::now()))
                    .expect("the intent is sent"),
            );
        }
    }
    tickets
}

fn view(studio: &Studio) -> &View {
    studio.view().expect("the studio is loaded")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=seat",
            "-c",
            "user.email=seat@studio.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn run(host: &Host) {
    let mut studio = Studio::default();
    studio.set_source(Box::new(Live::control(host.socket.clone())));
    studio.set_active(true);
    drive(&mut studio, "the seats", |studio| {
        studio.view().is_some_and(|view| view.seats.len() == 3)
    });
    assert!(studio.rights().contains(&Right::Operate));

    // The console: pick the repository, then type the goal.
    let mut console = Controller::new(PanelKind::Console);
    let mut panel = Panel::new("Console");
    fill(&studio, &mut console, &mut panel, None);
    type_into(&mut panel, "/repo scratch");
    assert_eq!(panel.key(Key::Enter), Some(Intent::Submit));
    assert!(press(&mut studio, &mut console, &mut panel, Intent::Submit, None).is_empty());
    type_into(&mut panel, GOAL);
    let tickets = press(&mut studio, &mut console, &mut panel, Intent::Submit, None);
    assert_eq!(tickets.len(), 1);
    let receipt = answered(&mut studio, tickets[0]).expect("the goal is taken");
    assert!(matches!(receipt, Outcome::Dispatched { .. }), "{receipt:?}");
    // The Task Wall draws the view's tasks: the lead's task is planned.
    drive(&mut studio, "the goal's lead task", |studio| {
        studio.view().is_some_and(|view| {
            view.goals.iter().any(|goal| {
                goal.text == GOAL
                    && view
                        .tasks
                        .iter()
                        .any(|task| task.goal == goal.goal && task.entry == "lead")
            })
        })
    });
    let goal = view(&studio)
        .goals
        .iter()
        .find(|goal| goal.text == GOAL)
        .map(|goal| goal.goal.clone())
        .expect("the goal");
    let lead = view(&studio)
        .tasks
        .iter()
        .find(|task| task.goal == goal && task.entry == "lead")
        .cloned()
        .expect("the lead's task");
    assert_eq!(lead.status, TaskStatus::Queued);
    assert_eq!(lead.seat, "lead");

    // The lead's task ends without a plan, and the coordinator's pass
    // opens the goal's decision at the podium.
    let ticket = studio
        .send(Operation::CancelStudioTask {
            task: lead.task.clone(),
        })
        .expect("the cancel is sent");
    answered(&mut studio, ticket).expect("the cancel is taken");
    drive(&mut studio, "the lead's task cancelled", |studio| {
        studio.view().is_some_and(|view| {
            view.tasks
                .iter()
                .any(|task| task.task == lead.task && task.status == TaskStatus::Cancelled)
        })
    });
    host.reconcile();
    drive(&mut studio, "the goal's decision", |studio| {
        studio.view().is_some_and(|view| {
            view.decisions
                .iter()
                .any(|open| open.goal == goal && open.kind == DecisionKind::LeadFailed)
        })
    });

    // The podium: the answer is a plan, which resumes the goal.
    let mut podium = Controller::new(PanelKind::Decisions);
    let mut panel = Panel::new("Decisions");
    fill(&studio, &mut podium, &mut panel, None);
    type_into(&mut panel, &studio_sim::plan());
    let tickets = press(&mut studio, &mut podium, &mut panel, Intent::Submit, None);
    assert_eq!(tickets.len(), 1, "the answer is sent");
    let receipt = answered(&mut studio, tickets[0]).expect("the answer is taken");
    assert!(matches!(receipt, Outcome::Dispatched { .. }), "{receipt:?}");
    drive(&mut studio, "the goal resumed", |studio| {
        studio.view().is_some_and(|view| {
            let running = view
                .goals
                .iter()
                .any(|g| g.goal == goal && g.status == GoalStatus::Running);
            let released = ["greet", "docs"].iter().all(|entry| {
                view.tasks.iter().any(|task| {
                    task.goal == goal && task.entry == *entry && task.status == TaskStatus::Queued
                })
            });
            running && released && view.decisions.iter().all(|open| open.goal != goal)
        })
    });
    let greet = view(&studio)
        .tasks
        .iter()
        .find(|task| task.goal == goal && task.entry == "greet")
        .map(|task| task.task.clone())
        .expect("the greeting task");

    // A seat's change to review: a worktree with one commit, recorded as a
    // local run records it.
    let worktree = host.temp.path().join("worktrees/greet");
    std::fs::create_dir_all(worktree.parent().expect("a parent")).expect("the worktrees");
    git(
        &host.checkout,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            worktree.to_str().expect("a path"),
            "HEAD",
        ],
    );
    let base = git(&worktree, &["rev-parse", "HEAD"]);
    std::fs::write(worktree.join("greeting.txt"), "Hello, studio\n").expect("a change");
    git(
        &worktree,
        &["commit", "-q", "-am", "Greet with Hello, studio"],
    );
    let record = serde_json::json!({
        "schema": coder::task::local::RECORD_SCHEMA,
        "task": greet,
        "project": "scratch",
        "checkout": host.checkout.to_string_lossy(),
        "worktree": worktree.to_string_lossy(),
        "base": base,
        "turns": [],
    });
    let records = host.store.join("local");
    std::fs::create_dir_all(&records).expect("the records");
    std::fs::write(
        records.join(format!("{greet}.json")),
        serde_json::to_vec(&record).expect("a record"),
    )
    .expect("the record");

    // The merge station reads the review.
    let read = |studio: &mut Studio, past: Option<&str>| {
        let mut found = None;
        drive(studio, "the review", |studio| {
            found = studio
                .review(&greet)
                .filter(|review| past.is_none_or(|past| review.head != past));
            found.is_some()
        });
        found.expect("a review")
    };
    let review = read(&mut studio, None);
    assert_eq!(review.base, base);
    assert!(review.diff.contains("Hello, studio"));

    // The worktree moves on after the person read the review: a merge at
    // the review they read is refused as stale, and the panel says so.
    std::fs::write(worktree.join("greeting.txt"), "Hello, studio!\n").expect("a change");
    git(&worktree, &["commit", "-q", "-am", "Exclaim"]);
    // The station's actions: **Merge**, **Request changes**, **Reject**.
    let (merge, reject) = (0, 2);
    let mut station = Controller::new(PanelKind::Review);
    let mut panel = Panel::new("Diff review");
    fill(&studio, &mut station, &mut panel, Some(&review));
    let tickets = press(
        &mut studio,
        &mut station,
        &mut panel,
        Intent::Action(merge),
        Some(&review),
    );
    assert_eq!(tickets.len(), 1, "the merge is sent");
    let refused = answered(&mut studio, tickets[0]).expect_err("a stale merge is refused");
    assert_eq!(refused.code, Code::Stale);
    // Every studio panel's status row shows the refusal's code.
    let answer = studio.status().expect("the refusal").clone();
    assert!(status_text(&answer).contains("`stale`"));

    // The review reloads at the new revisions, and a decision there is
    // taken: **Reject**, confirmed, with the reviewer's note, records it
    // and closes the task, keeping its worktree for inspection. A **Merge** would
    // publish through the forge, which a scratch repository has none of.
    let fresh = read(&mut studio, Some(&review.head));
    assert_ne!(fresh.head_commit, review.head_commit);
    fill(&studio, &mut station, &mut panel, Some(&fresh));
    type_into(&mut panel, "Keep the greeting plain.");
    assert!(
        press(
            &mut studio,
            &mut station,
            &mut panel,
            Intent::Submit,
            Some(&fresh)
        )
        .is_empty(),
        "a note waits for the decision"
    );
    // **Reject** waits for a confirming press, as the panel shows it.
    assert!(
        press(
            &mut studio,
            &mut station,
            &mut panel,
            Intent::Action(reject),
            Some(&fresh)
        )
        .is_empty(),
        "the first press arms the rejection"
    );
    let tickets = press(
        &mut studio,
        &mut station,
        &mut panel,
        Intent::Action(reject),
        Some(&fresh),
    );
    assert_eq!(tickets.len(), 1, "the rejection is sent");
    match answered(&mut studio, tickets[0]).expect("the rejection is taken") {
        Outcome::Merged { merged } => {
            assert_eq!(merged.verdict, Verdict::Reject);
            assert_eq!(merged.head, fresh.head);
        }
        other => panic!("expected the merge record, got {other:?}"),
    }
    assert!(worktree.exists(), "a rejected task keeps its worktree");
    studio.set_active(false);
}

#[test]
fn the_console_podium_and_merge_station_act_on_a_scratch_host() {
    let mut host = host();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&host)));
    let archived = host.archive();
    host.shutdown();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(
        archived.expect("every task the test created is archived") >= 3,
        "the lead and two plan tasks were created"
    );
}
