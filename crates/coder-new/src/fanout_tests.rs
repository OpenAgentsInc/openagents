//! Fan-out (#11539) on a scripted backend: independent parts run at the
//! same time, dependent parts wait and start from their inputs' work, and
//! changes that clash at converge are reported, never forced.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use coder_sdk::delegation::{
    Authority, Check, Child, Delegator, Host, Lineage, Outcome as Ended, Report, Request, Target,
    changed_files, scripted::init_repo,
};
use futures::future::BoxFuture;

use super::*;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::var_os("OPENAGENTS_SCRATCH")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(format!(
            "fanout-{name}-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn run_git(dir: &Path, args: &[&str]) -> String {
    git(dir, args).unwrap_or_else(|error| panic!("git {args:?}: {error}"))
}

/// A host whose children follow their part's text: `write FILE TEXT`
/// writes TEXT to FILE in the child's worktree, after a pause; `fail`
/// fails. It counts how many run at once and the order they start and
/// end, and what each child's worktree held when it started.
#[derive(Clone)]
struct Scripted {
    repo: PathBuf,
    pause: Duration,
    now: Arc<AtomicUsize>,
    most: Arc<AtomicUsize>,
    log: Arc<Mutex<Vec<String>>>,
    seen: Arc<Mutex<Vec<(String, Vec<String>)>>>,
    /// The first `together` parts to start wait (up to [`RENDEZVOUS`]) until
    /// that many have started, so parts the fan-out really runs at once
    /// overlap however slow each one's `git worktree add` is (#11570). A
    /// fan-out that ran them one after another still shows one at a time:
    /// the first part's wait runs out before the second can start.
    together: usize,
    started: Arc<AtomicUsize>,
}

/// How long a part waits for the others to start before it goes on alone.
const RENDEZVOUS: Duration = Duration::from_secs(5);

impl Scripted {
    fn new(repo: &Path, pause: Duration) -> Self {
        Scripted {
            repo: repo.to_owned(),
            pause,
            now: Arc::default(),
            most: Arc::default(),
            log: Arc::default(),
            seen: Arc::default(),
            together: 1,
            started: Arc::default(),
        }
    }

    /// The first `parts` to start wait for each other ([`Scripted::together`]).
    fn together(mut self, parts: usize) -> Self {
        self.together = parts;
        self
    }

    fn log(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn act(&self, child: &Child) -> Report {
        let report = Report::for_child(child, Ended::Done);
        let line = child
            .request
            .task
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        let part = line
            .split_once(": ")
            .map_or("", |(_, part)| part)
            .to_owned();
        let number = line
            .strip_prefix("Part ")
            .and_then(|rest| rest.split(' ').next())
            .unwrap_or_default()
            .to_owned();
        let dir = child.run_dir.join("worktree");
        let branch = format!("delegate/{}", child.lineage.id);
        let start = child.start.clone().unwrap_or_else(|| "HEAD".into());
        let base = run_git(&self.repo, &["rev-parse", &start]);
        run_git(
            &self.repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &branch,
                &dir.to_string_lossy(),
                &base,
            ],
        );
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != ".git")
            .collect();
        files.sort();
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((number.clone(), files));
        let at_once = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(at_once, Ordering::SeqCst);
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(format!("start {number}"));
        let order = self.started.fetch_add(1, Ordering::SeqCst) + 1;
        if order <= self.together {
            let until = std::time::Instant::now() + RENDEZVOUS;
            while self.started.load(Ordering::SeqCst) < self.together
                && std::time::Instant::now() < until
            {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        std::thread::sleep(self.pause);
        let result = if part == "fail" {
            Err("the scripted part failed".to_owned())
        } else if let Some(rest) = part.strip_prefix("write ") {
            let (file, text) = rest.split_once(' ').unwrap_or((rest, ""));
            std::fs::write(dir.join(file), format!("{text}\n")).map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(format!("end {number}"));
        self.now.fetch_sub(1, Ordering::SeqCst);
        let files = changed_files(&dir, Some(&base));
        let mut report = report
            .summarized(&format!("did part {number}"))
            .with_checks(vec![Check {
                command: "true".into(),
                passed: true,
            }])
            .with_files(files);
        if let Err(error) = result {
            report = report.failed(&error);
        }
        if report.changed_files.is_empty() {
            let _ = git(
                &self.repo,
                &["worktree", "remove", "--force", &dir.to_string_lossy()],
            );
            let _ = git(&self.repo, &["branch", "-D", &branch]);
        } else {
            report.worktree = Some(dir);
            report.branch = Some(branch);
        }
        report
    }
}

impl Host for Scripted {
    fn target(&self, request: &Request) -> Result<Target, String> {
        Ok(Target {
            harness: if request.to == "auto" {
                "coder".into()
            } else {
                request.to.clone()
            },
            name: "Coder".into(),
            model: request.model.clone(),
        })
    }

    fn run(&self, child: Child) -> BoxFuture<'static, Report> {
        let host = self.clone();
        let (answer, report) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let _ = answer.send(host.act(&child));
        });
        Box::pin(async move { report.await.expect("the scripted child reports") })
    }

    fn start(&self, _child: Child) -> Result<Report, String> {
        Err("no background here".into())
    }
}

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    host: Scripted,
    delegator: Arc<Delegator>,
}

fn fixture(name: &str, pause: Duration) -> Fixture {
    let root = scratch(name);
    let repo = root.join("repo");
    init_repo(&repo).unwrap();
    std::fs::write(repo.join("README.md"), "one\ntwo\nthree\n").unwrap();
    run_git(&repo, &["add", "-A"]);
    run_git(&repo, &["commit", "-q", "-m", "readme"]);
    let host = Scripted::new(&repo, pause);
    let delegator = Arc::new(Delegator::new(
        Arc::new(host.clone()),
        Lineage::root("run-test", "coder"),
        Authority::default(),
        &root.join("runs"),
    ));
    Fixture {
        root,
        repo,
        host,
        delegator,
    }
}

fn run_for(repo: &Path, parts: &[&str], edges: &[(u32, u32)], cap: usize) -> Run {
    let n = u32::try_from(parts.len()).unwrap();
    let mut needs = Vec::new();
    for part in 2..=n {
        for on in 1..part {
            needs.push(Need {
                part,
                on,
                p: if edges.contains(&(part, on)) {
                    0.8
                } else {
                    0.2
                },
            });
        }
    }
    let mut execution = crate::App::default().agent_execution();
    execution.cwd = repo.to_owned();
    Run {
        plan: "plan-test".into(),
        around: String::new(),
        graph: Graph {
            parts: parts.iter().map(|part| (*part).to_owned()).collect(),
            needs,
        },
        cap,
        to: "auto".into(),
        model: None,
        execution,
        record: None,
        chat: None,
    }
}

fn go(fixture: &Fixture, run: &Run) -> (Summary, Vec<RuntimeEvent>) {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut events = Vec::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let summary = runtime.block_on(execute(
        run,
        fixture.delegator.clone(),
        None,
        &cancel,
        &mut |event| events.push(event),
    ));
    (summary, events)
}

fn states(summary: &Summary) -> Vec<State> {
    summary.parts.iter().map(|part| part.state).collect()
}

#[test]
fn independent_parts_run_at_the_same_time_and_come_back_together() {
    let mut fixture = fixture("parallel", Duration::from_millis(400));
    fixture.host = fixture.host.clone().together(2);
    let run = run_for(
        &fixture.repo,
        &["write a.txt alpha", "write b.txt beta"],
        &[],
        4,
    );
    assert!(run.graph.parallel());
    let (summary, events) = go(&fixture, &run);
    assert_eq!(fixture.host.most.load(Ordering::SeqCst), 2, "both at once");
    assert_eq!(summary.widest, 2);
    assert_eq!(states(&summary), [State::Merged, State::Merged]);
    // Both parts' changes are in the chat's checkout, uncommitted.
    assert_eq!(
        std::fs::read_to_string(fixture.repo.join("a.txt")).unwrap(),
        "alpha\n"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.repo.join("b.txt")).unwrap(),
        "beta\n"
    );
    assert_eq!(
        run_git(&fixture.repo, &["rev-list", "--count", "HEAD"]),
        "2"
    );
    // Their worktrees and branches are gone; the checks ran once.
    assert_eq!(
        run_git(&fixture.repo, &["branch", "--list", "delegate/*"]),
        ""
    );
    assert_eq!(
        summary.checks,
        [Checked {
            command: "true".into(),
            passed: true
        }]
    );
    // Each part opened and closed its row on the agent tray.
    let rows: Vec<(String, bool)> = events
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::Delegation { id, event, .. } => match event.as_ref() {
                RuntimeEvent::Tool { name, running, .. } if name == "delegate" => {
                    Some((id.clone(), *running))
                }
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(rows.iter().filter(|(_, running)| *running).count(), 2);
    assert_eq!(rows.iter().filter(|(_, running)| !*running).count(), 2);
    let text = report(&run.graph, &summary);
    assert!(
        text.starts_with("All 2 parts are done (2 ran at the same time):"),
        "{text}"
    );
    assert!(
        text.contains("1. write a.txt alpha: Coder (finished; its changes are here (a.txt))"),
        "{text}"
    );
    assert!(
        text.contains("Checks on the combined changes: 1 of 1 pass."),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&fixture.root);
}

#[test]
fn a_dependent_part_waits_and_starts_from_its_inputs_work() {
    let mut fixture = fixture("ordered", Duration::from_millis(150));
    fixture.host = fixture.host.clone().together(2);
    let run = run_for(
        &fixture.repo,
        &["write a.txt alpha", "write b.txt beta", "write c.txt gamma"],
        &[(3, 1)],
        4,
    );
    assert_eq!(run.graph.inputs(2), [0]);
    assert_eq!(run.graph.levels(), [0, 0, 1]);
    let (summary, _) = go(&fixture, &run);
    let log = fixture.host.log();
    let at = |what: &str| log.iter().position(|line| line == what).unwrap();
    assert!(at("end 1") < at("start 3"), "{log:?}");
    assert!(at("start 2") < at("end 1"), "{log:?}");
    // Part 3's worktree held part 1's file, not part 2's.
    let seen = fixture
        .host
        .seen
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let third = seen.iter().find(|(part, _)| part == "3").unwrap();
    assert_eq!(third.1, ["README.md", "a.txt"]);
    assert_eq!(
        states(&summary),
        [State::Merged, State::Merged, State::Merged]
    );
    // Part 1's change came back once, not twice.
    assert_eq!(
        std::fs::read_to_string(fixture.repo.join("a.txt")).unwrap(),
        "alpha\n"
    );
    assert!(fixture.repo.join("c.txt").exists());
    let _ = std::fs::remove_dir_all(&fixture.root);
}

#[test]
fn changes_that_clash_at_converge_are_reported_and_never_forced() {
    let fixture = fixture("conflict", Duration::from_millis(50));
    let run = run_for(
        &fixture.repo,
        &["write README.md first", "write README.md second"],
        &[],
        4,
    );
    let (summary, _) = go(&fixture, &run);
    assert_eq!(states(&summary), [State::Merged, State::Conflict]);
    assert_eq!(summary.parts[1].conflicts, ["README.md"]);
    // The first part's change stands; the second's stays on its branch.
    assert_eq!(
        std::fs::read_to_string(fixture.repo.join("README.md")).unwrap(),
        "first\n"
    );
    let branch = summary.parts[1]
        .report
        .as_ref()
        .and_then(|report| report.branch.clone())
        .unwrap();
    assert_eq!(
        run_git(&fixture.repo, &["show", &format!("{branch}:README.md")]),
        "second"
    );
    let text = report(&run.graph, &summary);
    assert!(
        text.contains(&format!(
            "2. write README.md second: Coder (finished, but its changes to README.md clash with changes already here, so they stay on branch {branch})"
        )),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&fixture.root);
}

#[test]
fn the_persons_own_edits_are_never_overwritten() {
    let fixture = fixture("own-edits", Duration::from_millis(10));
    std::fs::write(fixture.repo.join("README.md"), "mine\n").unwrap();
    let run = run_for(
        &fixture.repo,
        &["write README.md theirs", "write b.txt beta"],
        &[],
        4,
    );
    let (summary, _) = go(&fixture, &run);
    assert_eq!(states(&summary), [State::Conflict, State::Merged]);
    assert_eq!(
        std::fs::read_to_string(fixture.repo.join("README.md")).unwrap(),
        "mine\n"
    );
    let _ = std::fs::remove_dir_all(&fixture.root);
}

#[test]
fn a_part_whose_input_failed_never_starts_and_the_cap_holds() {
    let fixture = fixture("skipped", Duration::from_millis(50));
    let run = run_for(
        &fixture.repo,
        &["fail", "write b.txt beta", "write c.txt gamma"],
        &[(3, 1)],
        1,
    );
    let (summary, _) = go(&fixture, &run);
    assert_eq!(fixture.host.most.load(Ordering::SeqCst), 1, "cap of one");
    assert_eq!(
        states(&summary),
        [State::Failed, State::Merged, State::Skipped]
    );
    assert!(!fixture.host.log().contains(&"start 3".to_owned()));
    let text = report(&run.graph, &summary);
    assert!(
        text.contains("3. write c.txt gamma (Part 1 did not finish, so it never started.)"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&fixture.root);
}

#[test]
fn the_graph_needs_some_parts_side_by_side_to_fan_out() {
    let parts: Vec<String> = ["a", "b", "c"].map(str::to_owned).to_vec();
    let need = |part, on, p| Need { part, on, p };
    // A chain runs one part at a time, as before.
    let chain = vec![need(2, 1, 0.9), need(3, 1, 0.2), need(3, 2, 0.9)];
    assert!(Graph::fan(&parts, Some(chain)).is_none());
    assert!(Graph::fan(&parts, None).is_none());
    let graph = Graph::fan(
        &parts,
        Some(vec![need(2, 1, 0.1), need(3, 1, 0.9), need(3, 2, 0.1)]),
    )
    .unwrap();
    assert_eq!(graph.edges(), [(3, 1)]);
    assert_eq!(
        outline(&graph, 3),
        "This is 3 separate pieces of work. Parts 1 and 2 start at the same time, each in its own worktree; part 3 waits for 1. Up to 3 run at once, and their changes come back here when they finish:\n1. a\n2. b\n3. c"
    );
    assert_eq!(cap(Some(6)), 6);
    assert_eq!(cap(Some(500)), CAP_MAX);
    assert!((2..=4).contains(&default_cap()));
    let ids: Vec<String> = questions(3).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["part_2_needs_1", "part_3_needs_1", "part_3_needs_2"]);
}

#[test]
fn a_fanout_record_shows_the_graph_and_each_outcome() {
    let fixture = fixture("record", Duration::from_millis(10));
    let records = fixture.root.join("routing");
    let mut run = run_for(
        &fixture.repo,
        &["write a.txt alpha", "write b.txt beta"],
        &[],
        2,
    );
    run.record = Some(crate::routing_record::Recorder::new(&records));
    let (summary, _) = go(&fixture, &run);
    run.record
        .as_ref()
        .unwrap()
        .fanout(record_line(&run, &summary, 0));
    let lines = crate::routing_record::read(&records);
    let Some(crate::routing_record::Line::Fanout(line)) = lines.first() else {
        panic!("a fanout line, got {lines:?}");
    };
    assert_eq!(line.parts, 2);
    assert_eq!(line.question_set, QUESTION_SET);
    assert_eq!(line.needs.len(), 1);
    assert!(line.edges.is_empty());
    assert_eq!(
        line.outcomes
            .iter()
            .map(|part| part.state)
            .collect::<Vec<_>>(),
        [State::Merged, State::Merged]
    );
    assert_eq!(line.checks_passed, Some(true));
    let _ = std::fs::remove_dir_all(&fixture.root);
}

/// A list-shaped ask whose parts System One judged independent starts a
/// fan-out turn; one judged a chain keeps the one-at-a-time plan.
#[test]
fn a_multi_part_ask_with_independent_parts_fans_out() {
    let fixture = fixture("app", Duration::from_millis(10));
    crate::harness_plugin::sign_in_claude(true);
    crate::dispatch::set_test_judge(Some(Some(0.9)));
    set_test_needs(Some(vec![(3, 1)]));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.cwd = Some(fixture.repo.clone());
    app.draft.text = "1. Add a --verbose flag\n2. Fix the typo in README\n3. Test the flag".into();
    app.submit_live();
    app.poll_route();
    let fanned = matches!(
        app.request.as_ref().map(|request| &request.kind),
        Some(crate::live::Work::Fanout { run }) if run.graph.edges() == [(3, 1)]
    );
    let outline = match app.live.entries.last() {
        Some(crate::live::Entry::Assistant { text, .. }) => text.clone(),
        other => panic!("the outline, got {other:?}"),
    };
    // A chain runs one part at a time.
    set_test_needs(Some(vec![(2, 1), (3, 2)]));
    let mut chained = crate::App::default();
    chained.set_mode(crate::Mode::Live);
    chained.cwd = Some(fixture.repo.clone());
    chained.draft.text = "1. Add a flag\n2. Document the flag\n3. Test the flag".into();
    chained.submit_live();
    chained.poll_route();
    let planned = chained.plan.is_some();
    set_test_needs(None);
    crate::dispatch::set_test_judge(None);
    crate::harness_plugin::sign_in_claude(false);
    let _ = std::fs::remove_dir_all(&fixture.root);
    assert!(fanned, "a fan-out turn");
    assert!(app.live.busy);
    assert!(app.plan.is_none());
    assert!(
        outline.contains("Parts 1 and 2 start at the same time"),
        "{outline}"
    );
    assert!(planned, "a chain keeps the plan");
}

/// A chat pinned with Shift+Tab is never routed, but a list-shaped ask in
/// it still asks System One about its parts (#11544): independent parts fan
/// out, a chain runs as a plan, and every part runs on the pin.
#[test]
fn a_pinned_chat_still_splits_a_multi_part_ask_and_keeps_every_part_on_the_pin() {
    use crate::harness_pick::Pick;
    use crate::harness_plugin::Harness;
    let fixture = fixture("pinned", Duration::from_millis(10));
    // Auto would pick Claude Code; the pin is OpenCode.
    crate::harness_plugin::sign_in_claude(true);
    crate::opencode_chat::install_for_test(true);
    crate::dispatch::set_test_judge(Some(Some(0.9)));
    set_test_needs(Some(vec![(3, 1)]));
    let pinned = |text: &str| {
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.cwd = Some(fixture.repo.clone());
        app.next_pick = Some(Pick::Harness(Harness::OpenCode));
        app.draft.text = text.into();
        app.submit_live();
        let routed = app
            .pending_route
            .as_ref()
            .map(|pending| pending.receiver.is_some());
        app.poll_route();
        (app, routed)
    };
    let (app, routed) =
        pinned("1. Add a --verbose flag\n2. Fix the typo in README\n3. Test the flag");
    let fanned = match app.request.as_ref().map(|request| &request.kind) {
        Some(crate::live::Work::Fanout { run }) => Some((run.to.clone(), run.graph.edges())),
        _ => None,
    };
    // A chain runs one part at a time, each on the pin.
    set_test_needs(Some(vec![(2, 1), (3, 2)]));
    let (mut chained, _) = pinned("1. Add a flag\n2. Document the flag\n3. Test the flag");
    let first = (
        chained.plan.as_ref().map(|plan| plan.at),
        chained.live.runs_on,
    );
    chained.live.busy = false;
    chained.request = None;
    chained.advance_plan(None);
    let second = (
        chained.plan.as_ref().map(|plan| plan.at),
        chained.live.runs_on,
    );
    // Prose in a pinned chat is not held: it goes straight to the pin.
    let (prose, prose_routed) = pinned("Fix the bug in main.rs and add a test.");
    set_test_needs(None);
    crate::dispatch::set_test_judge(None);
    crate::opencode_chat::install_for_test(false);
    crate::harness_plugin::sign_in_claude(false);
    let _ = std::fs::remove_dir_all(&fixture.root);
    assert_eq!(routed, Some(false), "only the parts question, no route");
    assert_eq!(
        fanned,
        Some((crate::delegate_host::OPENCODE.to_owned(), vec![(3, 1)]))
    );
    assert!(app.live.pinned);
    assert_eq!(first, (Some(0), Some(Harness::OpenCode)));
    assert_eq!(second, (Some(1), Some(Harness::OpenCode)));
    assert!(chained.live.pinned);
    assert_eq!(prose_routed, None);
    assert_eq!(prose.live.runs_on, Some(Harness::OpenCode));
}

/// A pinned fan-out runs each part on the pin, and its record names it.
#[test]
fn a_pinned_fanout_runs_every_part_on_the_pin_and_records_it() {
    let fixture = fixture("pinned-record", Duration::from_millis(10));
    let mut run = run_for(
        &fixture.repo,
        &["write a.txt alpha", "write b.txt beta"],
        &[],
        2,
    );
    run.to = crate::delegate_host::OPENCODE.into();
    let (summary, _) = go(&fixture, &run);
    let line = record_line(&run, &summary, 0);
    let unpinned = record_line(&run_for(&fixture.repo, &["a", "b"], &[], 2), &summary, 0);
    let _ = std::fs::remove_dir_all(&fixture.root);
    assert_eq!(line.pin.as_deref(), Some(crate::delegate_host::OPENCODE));
    assert_eq!(
        line.outcomes
            .iter()
            .map(|part| (part.state, part.harness.clone()))
            .collect::<Vec<_>>(),
        [
            (State::Merged, Some("opencode".to_owned())),
            (State::Merged, Some("opencode".to_owned()))
        ]
    );
    assert_eq!(unpinned.pin, None);
}
