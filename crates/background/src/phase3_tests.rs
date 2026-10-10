//! Phase 3: unknown-folder judgment with a stand-in Jev, escalation
//! briefings, each new built-in in the phase 1 style (temporary home,
//! injected clock and volume), and plugins' actions and proposals.
//! Nothing here reads or changes the real home or asks a live model.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::engine::{self, Answer, Clock, Event, Judge, Powers, Question, StepOutcome};
use crate::inuse::{Processes, Snapshot};
use crate::judged::{self, Status};
use crate::paths::Layout;
use crate::plan::{Env, TaskFact};
use crate::rule::{self, Action, Class, Escalate, GB, Rule, Watched};
use crate::run::{self, Cause, Outcome};
use crate::services::{Claim, CoderRun, Failure, Services, Usage};
use crate::store::{self, State};
use crate::volume::{Space, Volumes};

struct Fixed {
    free: u64,
    total: u64,
}

impl Volumes for Fixed {
    fn space(&self, path: &Path) -> std::io::Result<Space> {
        use std::os::unix::fs::MetadataExt;
        Ok(Space {
            device: std::fs::metadata(path)?.dev(),
            free: self.free,
            total: self.total,
        })
    }
}

struct Using(Vec<std::path::PathBuf>);

impl Processes for Using {
    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot::new(self.0.clone()))
    }
}

/// A stand-in Jev: a Noul at `noul`, a Choice whose `pick` reads `p`.
struct Stand {
    noul: f64,
    pick: &'static str,
    p: f64,
    asked: Mutex<Vec<String>>,
}

impl Stand {
    fn new(noul: f64, pick: &'static str, p: f64) -> Self {
        Self {
            noul,
            pick,
            p,
            asked: Mutex::new(Vec::new()),
        }
    }
}

impl Judge for Stand {
    fn ask(
        &self,
        state: &str,
        questions: &[(String, Question)],
    ) -> Result<BTreeMap<String, Answer>, String> {
        self.asked.lock().unwrap().push(state.to_owned());
        Ok(questions
            .iter()
            .map(|(id, question)| {
                let answer = match question {
                    Question::Noul(_) => Answer {
                        noul: Some(self.noul),
                        choice: Vec::new(),
                    },
                    Question::Choice { options, .. } => Answer {
                        noul: None,
                        choice: options
                            .iter()
                            .map(|(name, _)| {
                                let p = if name == self.pick {
                                    self.p
                                } else {
                                    (1.0 - self.p) / 5.0
                                };
                                (name.clone(), p)
                            })
                            .collect(),
                    },
                };
                (id.clone(), answer)
            })
            .collect())
    }
}

/// A stand-in host: records what it was asked, answers as configured.
#[derive(Default)]
struct Host {
    runs: Mutex<Vec<CoderRun>>,
    claims: Vec<Claim>,
    released: Mutex<Vec<u64>>,
    relay_up: Mutex<bool>,
    restarts: Mutex<Vec<Watched>>,
    failures: Vec<Failure>,
    issues: Mutex<Vec<(String, Option<String>)>>,
    plugins: Mutex<Vec<(String, String)>>,
}

impl Services for Host {
    fn start_coder_run(&self, run: &CoderRun) -> Result<String, String> {
        self.runs.lock().unwrap().push(run.clone());
        Ok("task-0123456789abcdef".into())
    }
    fn stale_claims(&self, idle_hours: u64, _now: u64) -> Result<Vec<Claim>, String> {
        assert_eq!(idle_hours, 6);
        Ok(self.claims.clone())
    }
    fn release_claim(&self, claim: &Claim) -> Result<(), String> {
        self.released.lock().unwrap().push(claim.number);
        Ok(())
    }
    fn probe(&self, target: Watched) -> Result<(), String> {
        match target {
            Watched::Relay if !*self.relay_up.lock().unwrap() => Err("refused".into()),
            _ => Ok(()),
        }
    }
    fn restart(&self, target: Watched) -> Result<String, String> {
        self.restarts.lock().unwrap().push(target);
        Ok("restarted the host.".into())
    }
    fn usage(&self, _since: u64) -> Result<Usage, String> {
        Ok(Usage {
            ended: 12,
            succeeded: 10,
            failed: 2,
            cost_microusd: Some(3_400_000),
            unpriced: 1,
        })
    }
    fn failures(&self, since: u64) -> Result<Vec<Failure>, String> {
        Ok(self
            .failures
            .iter()
            .filter(|f| f.at >= since)
            .cloned()
            .collect())
    }
    fn report_issue(
        &self,
        title: &str,
        _body: &str,
        existing: Option<&str>,
    ) -> Result<String, String> {
        self.issues
            .lock()
            .unwrap()
            .push((title.to_owned(), existing.map(str::to_owned)));
        Ok(existing.unwrap_or("#77").to_owned())
    }
    fn run_plugin(&self, plugin: &str, input: &str) -> Result<String, String> {
        self.plugins
            .lock()
            .unwrap()
            .push((plugin.to_owned(), input.to_owned()));
        Ok("Checked 3 folders.".into())
    }
}

static SERIAL: Mutex<()> = Mutex::new(());

struct Home {
    _serial: std::sync::MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
    layout: Layout,
}

fn home() -> Home {
    let serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path(), None).unwrap();
    std::fs::create_dir_all(&layout.openagents).unwrap();
    std::fs::create_dir_all(dir.path().join("work")).unwrap();
    Home {
        _serial: serial,
        _dir: dir,
        layout,
    }
}

fn no_tasks() -> Result<Vec<TaskFact>, String> {
    Ok(Vec::new())
}

fn env<'a>(home: &'a Home, volumes: &'a Fixed, processes: &'a dyn Processes) -> Env<'a> {
    Env {
        layout: &home.layout,
        facts: Some(&no_tasks),
        volumes,
        processes,
        now: crate::paths::now(),
        kache: None,
    }
}

fn low() -> Fixed {
    Fixed {
        free: 20 * GB,
        total: 1_000 * GB,
    }
}

fn idle() -> &'static Using {
    static IDLE: Using = Using(Vec::new());
    &IDLE
}

fn clock() -> Clock {
    Clock {
        now: crate::paths::now(),
        offset: 0,
    }
}

fn folder(path: &Path, bytes: usize) {
    std::fs::create_dir_all(path.join("http")).unwrap();
    std::fs::write(path.join("http/blob"), vec![1u8; bytes]).unwrap();
}

fn age(path: &Path, days: u64) {
    let when = SystemTime::now() - Duration::from_secs(days * 86_400);
    let mut stack = vec![path.to_owned()];
    while let Some(next) = stack.pop() {
        let meta = std::fs::symlink_metadata(&next).unwrap();
        if meta.is_dir() {
            for entry in std::fs::read_dir(&next).unwrap() {
                stack.push(entry.unwrap().path());
            }
        }
        let _ = std::fs::File::open(&next).and_then(|f| f.set_modified(when));
    }
}

fn powers<'a>(judge: Option<&'a dyn Judge>, services: Option<&'a dyn Services>) -> Powers<'a> {
    Powers { judge, services }
}

fn steps(env: &Env<'_>, rule: &Rule, p: Powers<'_>, dry: bool) -> Vec<engine::Step> {
    engine::steps_with(env, rule, &Event::default(), p, dry)
}

// Built-ins.

#[test]
fn every_built_in_is_valid_listed_and_off_but_host_defaults() {
    let h = home();
    for id in rule::BUILT_IN {
        let rule = rule::built_in(id).unwrap();
        rule.validate().unwrap();
        assert_eq!(rule.id, id);
        assert_eq!(rule.origin, rule::Origin::BuiltIn);
        if id == "checkout" {
            assert_eq!(rule.enabled, crate::builtins::on_coderos());
        } else if id == "worktrees" {
            assert_eq!(rule.enabled, crate::builtins::coder_host());
            assert_eq!(rule.classes.worktree_days, 7);
            assert_eq!(
                rule.triggers,
                vec![rule::Trigger::Daily { at: "03:30".into() }]
            );
        } else {
            assert!(!rule.enabled, "{id} ships off");
        }
    }
    let listed: Vec<String> = crate::view::list(&h.layout)
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(listed, rule::BUILT_IN.to_vec());
    // A conversation or plugin rule cannot take a built-in id.
    let mut taken = rule::built_in("usage").unwrap();
    taken.origin = rule::Origin::Conversation {
        thread: "t".into(),
        message: "m".into(),
    };
    assert!(taken.validate().is_err());
}

#[test]
fn stale_worktree_pruning_waits_seven_days_after_the_task_ends() {
    let h = home();
    let fresh = h.layout.worktrees().join("fresh");
    let old = h.layout.worktrees().join("old");
    std::fs::create_dir_all(&fresh).unwrap();
    std::fs::create_dir_all(&old).unwrap();
    age(&old, 9);
    let tasks = vec![
        TaskFact {
            id: "a".into(),
            worktree: fresh.clone(),
            ended: true,
            ..TaskFact::default()
        },
        TaskFact {
            id: "b".into(),
            worktree: old.clone(),
            ended: true,
            ..TaskFact::default()
        },
    ];
    let facts = move || Ok(tasks.clone());
    let volumes = Fixed {
        free: 900 * GB,
        total: 1_000 * GB,
    };
    let env = Env {
        facts: Some(&facts),
        ..env(&h, &volumes, idle())
    };
    let rule = rule::built_in("worktrees").unwrap();
    // Plenty of free space: it plans anyway.
    let plan = crate::plan::plan(&env, &rule, false);
    let kept: BTreeMap<_, _> = plan
        .kept
        .iter()
        .map(|k| (k.path.clone(), k.why.clone()))
        .collect();
    assert!(kept[&fresh].contains("used in the last 7 days"), "{kept:?}");
    // The old one passed the age gate; it is not a Git worktree here, so
    // the unsaved-work check keeps it.
    assert!(!kept[&old].contains("used in the last"), "{kept:?}");
}

#[test]
fn stale_claims_are_released_and_a_dry_run_only_says_so() {
    let h = home();
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let host = Host {
        claims: vec![Claim {
            repository: "acme/app".into(),
            number: 41,
            task: "t1".into(),
            why: "its task ended".into(),
        }],
        ..Host::default()
    };
    let rule = rule::built_in("claims").unwrap();
    let dry = steps(&env, &rule, powers(None, Some(&host)), true);
    assert_eq!(dry[0].outcome, StepOutcome::Would);
    assert!(host.released.lock().unwrap().is_empty());
    let done = steps(&env, &rule, powers(None, Some(&host)), false);
    assert_eq!(done[0].outcome, StepOutcome::Done);
    assert_eq!(done[0].detail, "Released #41: its task ended.");
    assert_eq!(*host.released.lock().unwrap(), vec![41]);
    // Without the host's services it says it cannot, and does nothing.
    let none = steps(&env, &rule, Powers::default(), false);
    assert_eq!(none[0].outcome, StepOutcome::Failed);
}

#[test]
fn the_health_watch_restarts_after_three_failures_in_a_row() {
    let h = home();
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let host = Host::default();
    let mut rule = rule::built_in("health").unwrap();
    rule.actions.truncate(1);
    for n in 1..=2 {
        let s = steps(&env, &rule, powers(None, Some(&host)), false);
        assert_eq!(s[0].outcome, StepOutcome::Skipped, "failure {n}");
    }
    let third = steps(&env, &rule, powers(None, Some(&host)), false);
    assert_eq!(third[0].outcome, StepOutcome::Done);
    assert!(
        third[0]
            .detail
            .starts_with("The relay did not answer 3 times")
    );
    assert_eq!(*host.restarts.lock().unwrap(), vec![Watched::Relay]);
    // The count starts over; an answer resets it too.
    steps(&env, &rule, powers(None, Some(&host)), false);
    *host.relay_up.lock().unwrap() = true;
    steps(&env, &rule, powers(None, Some(&host)), false);
    let state = State::load(&h.layout);
    assert!(state.rules["health"].failures.is_empty());
}

#[test]
fn a_repeated_flake_is_judged_then_reported_once_and_noted_after() {
    let h = home();
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let now = env.now;
    let fail = |task: &str, at: u64| Failure {
        test: "net::reconnects".into(),
        output: "timed out after 5s".into(),
        task: task.into(),
        at,
    };
    let rule = rule::built_in("flakes").unwrap();
    let host = Host {
        failures: vec![fail("t1", now - 60), fail("t2", now - 30)],
        ..Host::default()
    };
    // A judge that says these differ: nothing is reported.
    let differ = Stand::new(0.3, "x", 0.0);
    let s = steps(&env, &rule, powers(Some(&differ), Some(&host)), false);
    assert!(s.is_empty(), "{s:?}");
    assert!(host.issues.lock().unwrap().is_empty());
    // Start over with a judge that says they match.
    std::fs::remove_file(h.layout.flakes()).unwrap();
    let same = Stand::new(0.95, "x", 0.0);
    let s = steps(&env, &rule, powers(Some(&same), Some(&host)), false);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].detail, "net::reconnects looks flaky; reported in #77.");
    assert!(same.asked.lock().unwrap()[0].contains("timed out after 5s"));
    // A third failure is noted on the same issue.
    let host = Host {
        failures: vec![fail("t3", now)],
        ..Host::default()
    };
    let s = steps(&env, &rule, powers(Some(&same), Some(&host)), false);
    assert!(s[0].detail.contains("noted on #77"), "{s:?}");
    assert_eq!(
        host.issues.lock().unwrap()[0].1.as_deref(),
        Some("#77"),
        "comments on the known issue"
    );
}

#[test]
fn the_usage_summary_is_one_line_and_never_a_limit() {
    let h = home();
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let rule = rule::built_in("usage").unwrap();
    let host = Host::default();
    let s = steps(&env, &rule, powers(None, Some(&host)), false);
    assert_eq!(
        s[0].detail,
        "Today: 12 Coder runs ended (10 finished, 2 failed), $3.40 (1 unpriced)."
    );
    let lower = s[0].detail.to_lowercase();
    for word in ["limit", "quota", "budget", "cap"] {
        assert!(!lower.contains(word));
    }
    // Without services it still says what the background rules did.
    let s = steps(&env, &rule, Powers::default(), false);
    assert_eq!(s[0].detail, "Today: nothing ran.");
    assert_eq!(crate::builtins::dollars(1_234_567), "$1.23");
}

#[test]
fn rotation_compresses_then_removes_old_logs_and_skips_what_is_open() {
    let h = home();
    let traces = h.layout.openagents.join("traces");
    std::fs::create_dir_all(&traces).unwrap();
    let week = traces.join("week.atif.jsonl");
    let month = traces.join("month.log");
    let open = traces.join("open.log");
    let today = traces.join("today.log");
    for path in [&week, &month, &open, &today] {
        std::fs::write(path, "line\n".repeat(2000)).unwrap();
    }
    age(&week, 8);
    age(&month, 31);
    age(&open, 31);
    let elsewhere = h.layout.home.join("work/keep.log");
    std::fs::write(&elsewhere, "x").unwrap();
    std::os::unix::fs::symlink(&elsewhere, traces.join("link.log")).unwrap();
    age(&elsewhere, 60);
    let volumes = low();
    let using = Using(vec![open.clone()]);
    let env = env(&h, &volumes, &using);
    let rule = rule::built_in("rotate").unwrap();
    let dry = steps(&env, &rule, Powers::default(), true);
    assert_eq!(dry[0].outcome, StepOutcome::Would);
    assert!(week.exists() && month.exists());
    let done = steps(&env, &rule, Powers::default(), false);
    assert_eq!(done[0].outcome, StepOutcome::Done, "{done:?}");
    assert!(done[0].detail.starts_with("Compressed 1 and removed 1"));
    let gz = traces.join("week.atif.jsonl.gz");
    assert!(gz.exists() && !week.exists());
    assert!(!month.exists());
    assert!(open.exists(), "an open file stays");
    assert!(today.exists());
    assert!(elsewhere.exists(), "links are not followed");
    let kept = std::fs::metadata(&gz).unwrap().modified().unwrap();
    assert!(SystemTime::now().duration_since(kept).unwrap() > Duration::from_secs(7 * 86_400));
}

#[test]
fn the_nightly_qa_run_starts_coder_only_for_real() {
    let h = home();
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let rule = rule::built_in("qa").unwrap();
    let host = Host::default();
    let dry = steps(&env, &rule, powers(None, Some(&host)), true);
    assert_eq!(dry[0].outcome, StepOutcome::Would);
    assert!(host.runs.lock().unwrap().is_empty());
    let done = steps(&env, &rule, powers(None, Some(&host)), false);
    assert_eq!(
        done[0].detail,
        "Started Coder run task-0123456: Nightly simulated-user QA."
    );
    assert!(
        host.runs.lock().unwrap()[0]
            .prompt
            .contains("simulated-users")
    );
}

#[test]
fn a_scheduled_prompt_can_post_into_an_existing_chat() {
    let h = home();
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let mut rule = rule::built_in("qa").unwrap();
    rule.name = "Scheduled prompt: triage".into();
    rule.actions = vec![Action::StartCoderRun {
        prompt: "triage the new issues".into(),
        workspace: None,
        chat: Some("2026-10-10-abc_1".into()),
    }];
    assert!(rule.validate().is_ok());
    let host = Host::default();
    let dry = steps(&env, &rule, powers(None, Some(&host)), true);
    assert!(dry[0].detail.starts_with("would post into chat"), "{dry:?}");
    let done = steps(&env, &rule, powers(None, Some(&host)), false);
    assert_eq!(
        done[0].detail,
        "Posted into chat 2026-10-10-a: Scheduled prompt: triage."
    );
    let runs = host.runs.lock().unwrap().clone();
    assert_eq!(runs[0].chat.as_deref(), Some("2026-10-10-abc_1"));
    assert_eq!(runs[0].prompt, "triage the new issues");
    // A chat id is a Coder session id, nothing else.
    rule.actions = vec![Action::StartCoderRun {
        prompt: "p".into(),
        workspace: None,
        chat: Some("../etc".into()),
    }];
    assert!(rule.validate().is_err());
}

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
}

#[test]
fn keeping_a_checkout_on_main_says_once_when_it_is_on_another_branch() {
    let h = home();
    let repo = h.layout.home.join("openagents");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(&repo, &["checkout", "-q", "-b", "feature"]);
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let mut rule = rule::built_in("checkout").unwrap();
    rule.enabled = true;
    let first = engine::evaluate_with(
        &env,
        &rule,
        Cause::Interval,
        &Event::default(),
        clock(),
        Powers::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        first.notice.as_deref(),
        Some("~/openagents is on feature, not main; left as it is.")
    );
    let again = engine::evaluate_with(
        &env,
        &rule,
        Cause::Interval,
        &Event::default(),
        clock(),
        Powers::default(),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(again.notice, None, "said once, not every check");
}

// Unknown-folder judgment.

fn unknowns(h: &Home) -> (std::path::PathBuf, std::path::PathBuf) {
    let cache = h.layout.home.join(".cache/pip");
    folder(&cache, 300_000);
    let small = h.layout.home.join(".cache/tiny");
    folder(&small, 10);
    // A checkout, a denied folder, and a covered one are never asked about.
    let checkout = h.layout.home.join("work/project");
    folder(&checkout, 400_000);
    std::fs::create_dir_all(checkout.join(".git")).unwrap();
    folder(&h.layout.openagents.join("pylon"), 400_000);
    folder(&h.layout.targets().join("x-slot-0"), 400_000);
    let data = h.layout.home.join("work/datasets");
    folder(&data, 200_000);
    (cache, data)
}

#[test]
fn only_the_largest_unknown_folders_code_allows_are_judged() {
    let h = home();
    let (cache, data) = unknowns(&h);
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let found: Vec<_> = judged::survey_min(&env, &rule::disk(), 100_000)
        .into_iter()
        .map(|u| u.path)
        .collect();
    assert_eq!(found, vec![cache.clone(), data]);
    let state = judged::survey_min(&env, &rule::disk(), 100_000)[0].state(&h.layout.home);
    assert!(state.starts_with("Path: ~/.cache/pip\n"), "{state}");
    assert!(state.contains("Largest entries: http"));
    assert!(state.contains("Processes using it now: 0"));
}

#[test]
fn a_yes_proposes_and_never_deletes_and_the_bar_is_the_setting() {
    let h = home();
    let (cache, _) = unknowns(&h);
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let unknown = &judged::survey_min(&env, &rule::disk(), 100_000)[0];
    let home = &h.layout.home;
    assert!(judged::decide(unknown, home, 0.9, ("package_cache", 0.8)).proposed);
    assert!(!judged::decide(unknown, home, 0.89, ("package_cache", 0.8)).proposed);
    assert!(!judged::decide(unknown, home, 0.99, ("user_data", 0.8)).proposed);
    let busy = judged::Unknown {
        users: 1,
        ..unknown.clone()
    };
    assert!(!judged::decide(&busy, home, 0.99, ("app_cache", 0.9)).proposed);
    let jev = Stand::new(0.95, "package_cache", 0.9);
    let judgments = judged::consider_min(&env, &rule::disk(), &jev, 100_000).unwrap();
    assert_eq!(judgments.len(), 2);
    assert!(cache.exists(), "a judgment deletes nothing");
    let proposals = judged::Proposals::load(&h.layout);
    assert_eq!(proposals.waiting().len(), 2);
    assert_eq!(
        proposals.folders["~/.cache/pip"].setting,
        "background.cache_dir"
    );
    // Asked once: the next look does not ask again.
    let again = judged::consider_min(&env, &rule::disk(), &jev, 100_000).unwrap();
    assert!(again.is_empty());
}

#[test]
fn a_confirmed_cache_is_a_plain_rule_entry_trashed_undone_and_expired() {
    let h = home();
    let (cache, data) = unknowns(&h);
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let jev = Stand::new(0.95, "package_cache", 0.9);
    judged::consider_min(&env, &rule::disk(), &jev, 100_000).unwrap();
    judged::decline(&h.layout, "~/work/datasets").unwrap();
    let rule = judged::confirm(&h.layout, "~/.cache/pip", env.now).unwrap();
    assert_eq!(rule.classes.judged[0].path, "~/.cache/pip");
    assert!(
        rule.actions
            .iter()
            .any(|a| a.classes() == vec![Class::Judged])
    );
    let proposals = judged::Proposals::load(&h.layout);
    assert_eq!(
        proposals.folders["~/work/datasets"].status,
        Status::Declined
    );
    // The next run needs no model: the folder moves to the trash.
    let report = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    let record = report.record.clone().unwrap();
    let moved = record
        .actions
        .iter()
        .find(|a| a.class == Class::Judged)
        .unwrap();
    assert_eq!(moved.outcome, Outcome::Trashed);
    assert!(!cache.exists() && moved.trashed.as_ref().unwrap().exists());
    assert!(data.exists(), "declined stays");
    assert_eq!(
        record.freed_sum,
        record
            .actions
            .iter()
            .filter(|a| a.outcome == Outcome::Deleted)
            .map(|a| a.bytes)
            .sum::<u64>()
    );
    // Undo puts it back.
    let restored = run::undo(&h.layout, &record.run).unwrap();
    assert!(
        restored
            .iter()
            .any(|(path, result)| path == &cache && result.is_ok())
    );
    assert!(cache.exists());
    // Trashed again, then past its day the next run empties it.
    let report = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    let trashed = report
        .record
        .unwrap()
        .actions
        .iter()
        .find(|a| a.class == Class::Judged)
        .unwrap()
        .trashed
        .clone()
        .unwrap();
    age(trashed.parent().unwrap(), 2);
    let later = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    let expired = later.record.unwrap();
    assert!(
        expired
            .actions
            .iter()
            .any(|a| a.class == Class::Trash && a.outcome == Outcome::Deleted)
    );
    assert!(!trashed.exists());
}

#[test]
fn a_confirmed_folder_that_became_a_checkout_is_kept() {
    let h = home();
    let (cache, _) = unknowns(&h);
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let jev = Stand::new(0.95, "package_cache", 0.9);
    judged::consider_min(&env, &rule::disk(), &jev, 100_000).unwrap();
    let rule = judged::confirm(&h.layout, "~/.cache/pip", env.now).unwrap();
    std::fs::create_dir_all(cache.join(".git")).unwrap();
    let plan = crate::plan::plan(&env, &rule, true);
    assert!(
        plan.kept
            .iter()
            .any(|k| k.path == cache && k.why == "a Git checkout")
    );
}

// Escalation.

#[test]
fn a_rule_that_falls_short_escalates_once_a_day_with_a_code_built_briefing() {
    let h = home();
    let ended = h.layout.targets().join("old-slot-9");
    std::fs::create_dir_all(ended.join("debug")).unwrap();
    std::fs::write(ended.join("debug/blob"), vec![0u8; 50_000]).unwrap();
    let pylon = h.layout.openagents.join("pylon");
    folder(&pylon, 1000);
    let volumes = low();
    let env = env(&h, &volumes, idle());
    let mut rule = rule::disk();
    rule.enabled = true;
    rule.escalate = Some(Escalate {
        workspace: Some("~/work/openagents".into()),
    });
    rule.safety.report = vec!["~/.openagents/pylon".into()];
    let report = run::run(&env, &rule, Cause::Threshold, false, false).unwrap();
    assert!(crate::escalate::short(&report));
    let host = Host::default();
    let record = crate::escalate::after(&env, &rule, &report, None, Some(&host)).unwrap();
    assert!(record.escalated);
    let runs = host.runs.lock().unwrap().clone();
    assert_eq!(runs.len(), 1);
    let prompt = &runs[0].prompt;
    assert!(prompt.contains("propose changes"));
    assert!(prompt.contains("Do not delete"));
    assert!(
        prompt.contains("cleans below 200 GB, aims for 300 GB"),
        "{prompt}"
    );
    assert!(
        prompt.contains("~/.openagents/targets/old-slot-9"),
        "{prompt}"
    );
    assert!(
        prompt.contains("Measured, not a known cache:\n- ~/.openagents/pylon"),
        "{prompt}"
    );
    assert!(
        runs[0]
            .workspace
            .as_ref()
            .unwrap()
            .ends_with("work/openagents")
    );
    // Once a day.
    assert!(
        crate::escalate::after(&env, &rule, &report, None, Some(&host))
            .is_none_or(|r| !r.escalated)
    );
    assert_eq!(host.runs.lock().unwrap().len(), 1);
    // A rule that does not ask never escalates.
    let mut quiet = rule.clone();
    quiet.id = "disk".into();
    quiet.escalate = None;
    State::update(&h.layout, "disk", |s| s.last_escalation = None);
    assert!(crate::escalate::after(&env, &quiet, &report, None, Some(&host)).is_none());
}

// Plugins.

fn installed(id: &str) -> crate::plugins::Installed {
    crate::plugins::Installed {
        id: id.into(),
        slug: id.split(':').nth(1).unwrap_or(id).into(),
        name: "P".into(),
        summary: String::new(),
        version: "1".into(),
        dir: std::path::PathBuf::new(),
        background: Vec::new(),
        classes: Vec::new(),
        enabled: true,
    }
}

#[test]
fn a_plugin_runs_only_its_own_action_and_starts_coder_only_when_it_asks() {
    let mut rule = rule::disk();
    rule.id = "checker".into();
    rule.actions = vec![Action::RunPlugin {
        plugin: "k:checker".into(),
        input: "look".into(),
    }];
    assert!(crate::plugins::admit(rule.clone(), &installed("k:checker")).is_ok());
    let err = crate::plugins::admit(rule.clone(), &installed("k:other")).unwrap_err();
    assert!(err.contains("runs only its own plugin"), "{err}");
    rule.actions = vec![Action::StartCoderRun {
        prompt: "p".into(),
        workspace: None,
        chat: None,
    }];
    assert!(crate::plugins::admit(rule.clone(), &installed("k:checker")).is_err());
    rule.needs.coder = true;
    assert!(crate::plugins::admit(rule.clone(), &installed("k:checker")).is_ok());
    rule.actions = vec![Action::UsageSummary];
    assert!(crate::plugins::admit(rule, &installed("k:checker")).is_err());
}

#[test]
fn a_rule_published_as_a_plugin_installs_off_and_admits_as_it_was() {
    let h = home();
    let mut rule = rule::disk();
    rule.id = "keep-free".into();
    rule.name = "Keep 50 GB free".into();
    rule.origin = rule::Origin::Conversation {
        thread: "t".into(),
        message: "keep 50 GB free".into(),
    };
    rule.enabled = true;
    rule.classes.judged.push(rule::Judged {
        path: "~/.cache/pip".into(),
        kind: "package_cache".into(),
        confirmed: 1,
    });
    rule.actions.push(Action::DeleteCaches {
        classes: vec![Class::Judged],
    });
    store::save(&h.layout, &rule).unwrap();
    let out = h.layout.home.join("work/keep-free-plugin");
    crate::plugins::package(&h.layout, "keep-free", &out).unwrap();
    // Installed like any plugin: off, then on, then admitted.
    let dir = h
        .layout
        .extensions()
        .join(crate::plugins::LOCAL_KEY)
        .join("keep-free")
        .join("0.1.0");
    std::fs::create_dir_all(dir.join("background")).unwrap();
    std::fs::copy(out.join("package.json"), dir.join("package.json")).unwrap();
    std::fs::copy(
        out.join("background/keep-free.json"),
        dir.join("background/keep-free.json"),
    )
    .unwrap();
    // The conversation rule moves aside, as when installed elsewhere.
    store::remove(&h.layout, "keep-free").unwrap();
    crate::plugins::set_enabled(&h.layout, "keep-free", true).unwrap();
    let admitted = store::load(&h.layout, "keep-free").unwrap();
    assert!(
        !admitted.active(crate::paths::now()),
        "paused until a dry run"
    );
    assert!(
        admitted.classes.judged.is_empty(),
        "confirmed caches stay home"
    );
    assert!(admitted.needs.tasks);
    assert_eq!(admitted.goal, rule.goal);
    // A rule that updates a checkout cannot be a plugin.
    let mut update = rule::built_in("checkout").unwrap();
    update.enabled = true;
    store::save(&h.layout, &update).unwrap();
    assert!(crate::plugins::package(&h.layout, "checkout", &out).is_err());
}

#[test]
fn a_plugins_proposed_folders_wait_for_the_person() {
    let h = home();
    let dir = h
        .layout
        .extensions()
        .join(crate::plugins::LOCAL_KEY)
        .join("caches")
        .join("1.0.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("package.json"),
        serde_json::json!({
            "v": 1, "slug": "caches", "name": "Caches", "version": "1.0.0",
            "classes": [
                {"path": "~/.cache/yarn", "kind": "package_cache"},
                {"path": "~/Documents", "kind": "user_data"},
                {"path": "/etc", "kind": "app_cache"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    crate::plugins::set_enabled(&h.layout, "caches", true).unwrap();
    let proposals = judged::Proposals::load(&h.layout);
    let waiting: Vec<&str> = proposals
        .waiting()
        .iter()
        .map(|p| p.path.as_str())
        .collect();
    assert_eq!(waiting, vec!["~/.cache/yarn"]);
    assert_eq!(
        proposals.folders["~/.cache/yarn"].plugin.as_deref(),
        Some(format!("{}:caches", crate::plugins::LOCAL_KEY).as_str())
    );
}
