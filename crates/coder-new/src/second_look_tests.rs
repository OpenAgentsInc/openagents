use std::sync::atomic::AtomicUsize;

use coder_sdk::{Budget, HookBackend, Hooks, Permission};
use futures::future::BoxFuture;
use jev::decisions::{ChoiceProbability, DecisionAnswer, Served};

use super::*;

/// The decider: `check` at `p`, or a failure.
struct Fixed(Result<f64, &'static str>);

impl Decider for Fixed {
    fn decide(
        &self,
        _request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
        let answer = self.0;
        Box::pin(async move { answer.map(answered).map_err(str::to_owned) })
    }
}

fn answered(p: f64) -> DecisionsResponse {
    DecisionsResponse {
        id: "dec_1".into(),
        object: "decision".into(),
        model: "clef-flash".into(),
        answers: vec![DecisionAnswer::Choice {
            name: QUESTION.into(),
            choice: if p >= 0.5 { "check" } else { "skip" }.into(),
            probabilities: vec![
                ChoiceProbability {
                    value: "check".into(),
                    probability: p,
                },
                ChoiceProbability {
                    value: "skip".into(),
                    probability: 1.0 - p,
                },
            ],
            confidence: p.max(1.0 - p),
        }],
        usage: jev::decisions::DecisionUsage::default(),
        metadata: None,
        openagents: Served {
            door: Some("clef".into()),
            latency_ms: 5,
            receipt: None,
        },
    }
}

fn found(problem: &str) -> Verdict {
    Verdict::SendBack {
        findings: vec![Finding {
            file: Some("src/lib.rs".into()),
            line: Some(3),
            problem: problem.into(),
        }],
    }
}

/// A scripted reviewer: answers each review with the next verdict, and
/// keeps what it was shown.
struct Script {
    verdicts: Mutex<Vec<Verdict>>,
    seen: Mutex<Vec<Review>>,
}

fn script(verdicts: Vec<Verdict>) -> (Arc<Script>, Reviewer) {
    let script = Arc::new(Script {
        verdicts: Mutex::new(verdicts),
        seen: Mutex::default(),
    });
    let shared = script.clone();
    let reviewer: Reviewer = Arc::new(move |review: &Review| {
        lock(&shared.seen).push(review.clone());
        let mut verdicts = lock(&shared.verdicts);
        let verdict = if verdicts.is_empty() {
            Verdict::Pass
        } else {
            verdicts.remove(0)
        };
        Ok(Reviewed {
            verdict,
            model: "free-model".into(),
            door: "openagents-gateway".into(),
            latency_ms: 7,
        })
    });
    (script, reviewer)
}

struct Fixture {
    repo: tempfile::TempDir,
    hooks: Hooks,
    lines: Arc<Mutex<Vec<Decision>>>,
    outcomes: Arc<Mutex<Vec<Value>>>,
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
    assert!(status.status.success(), "git {args:?}");
}

fn fixture(decider: Fixed, reviewer: Reviewer, shadow: bool) -> Fixture {
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q"]);
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();
    git(repo.path(), &["add", "-A"]);
    git(repo.path(), &["commit", "-qm", "start"]);
    let lines: Arc<Mutex<Vec<Decision>>> = Arc::default();
    let outcomes: Arc<Mutex<Vec<Value>>> = Arc::default();
    let (kept, out) = (lines.clone(), outcomes.clone());
    let mut registry = Registry::new();
    for hook in hooks(
        repo.path().to_owned(),
        Arc::new(decider),
        reviewer,
        Arc::new(move |line: &Decision| lock(&kept).push(line.clone())),
        Arc::new(move |line: &Value| lock(&out).push(line.clone())),
        shadow,
    ) {
        registry.register(hook).unwrap();
    }
    let hooks = Hooks::new(registry, Budget::default(), HookBackend::Scripted).unwrap();
    Fixture {
        repo,
        hooks,
        lines,
        outcomes,
    }
}

impl Fixture {
    async fn prompt(&self, text: &str) {
        self.hooks
            .fire(Call::new(Event::UserPromptSubmit, json!({"prompt": text})))
            .await;
    }

    fn write(&self, body: &str) {
        std::fs::write(self.repo.path().join("src/lib.rs"), body).unwrap();
    }

    async fn ran(&self, command: &str, exit: i64) {
        self.hooks
            .fire(Call::tool(
                Event::PostToolUse,
                "Bash",
                json!({"command": command, "tool_response": {"stdout": "test result: ok", "exit_code": exit}}),
            ))
            .await;
    }

    async fn stop(&self) -> Stop {
        self.hooks
            .fire(Call::new(
                Event::Stop,
                json!({"last_assistant_message": "WORKER-CONVERSATION: I am done, trust me.", "stop_hook_active": false}),
            ))
            .await
            .stop
    }

    fn actions(&self) -> Vec<(String, String)> {
        lock(&self.lines)
            .iter()
            .map(|line| (line.question.clone(), line.action.clone()))
            .collect()
    }
}

const BUGGY: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a.wrapping_add(b)\n}\n";

#[tokio::test]
async fn a_bigger_change_is_sent_back_once_with_findings_then_passes() {
    let (seen, reviewer) = script(vec![found("subtracts instead of adding"), Verdict::Pass]);
    let f = fixture(Fixed(Ok(0.8)), reviewer, false);
    f.prompt("Make add() add its two arguments.").await;
    f.write(BUGGY);
    f.ran("cargo test", 0).await;
    let Stop::Block { reason } = f.stop().await else {
        panic!("should send back");
    };
    assert!(
        reason.contains("src/lib.rs:3: subtracts instead of adding"),
        "{reason}"
    );
    // The reviewer saw the request, the diff and the check results, and
    // nothing of the agent's conversation.
    let review = lock(&seen.seen)[0].clone();
    assert_eq!(review.request, "Make add() add its two arguments.");
    assert!(review.diff.contains("-    a + b") && review.diff.contains("+    a - b"));
    assert_eq!(review.checks.len(), 1);
    assert!(
        review.checks[0].starts_with("$ cargo test -> exit 0"),
        "{:?}",
        review.checks
    );
    let shown = serde_json::to_string(&review).unwrap() + &prompt(&review);
    assert!(!shown.contains("WORKER-CONVERSATION"));
    // The agent fixes it; the second look passes and the stop goes through.
    f.write(FIXED);
    assert_eq!(f.stop().await, Stop::Continue);
    assert_eq!(lock(&seen.seen).len(), 2);
    assert_eq!(
        f.actions(),
        [
            ("verdict".into(), "send_back".into()),
            (QUESTION.into(), "send_back".into()),
            ("verdict".into(), "pass".into()),
            (QUESTION_SET.into(), "recheck_pass".into()),
        ]
    );
    // Every line carries the turn's run id; the outcome joins the verdict.
    let lines = lock(&f.lines);
    let run = lines[0].run_id.clone().unwrap();
    assert!(
        lines
            .iter()
            .all(|line| line.run_id.as_deref() == Some(run.as_str()))
    );
    let outcomes = lock(&f.outcomes);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0]["run_id"], json!(run));
    assert_eq!(outcomes[0]["passed"], json!(true));
    assert_eq!(outcomes[0]["labels"][0]["decision_ts"], json!(lines[0].ts));
    assert_eq!(outcomes[0]["labels"][0]["correct"], json!(true));
    // `hook-log report` joins the verdict to it: the keep rate.
    let parsed: Vec<hook_log::report::Outcome> =
        hook_log::report::parse_lines(&outcomes[0].to_string());
    let joined = hook_log::report::join(lines.clone(), &parsed);
    assert_eq!(joined[0].correct, Some(true));
    assert_eq!(joined[0].passed, Some(true));
    // The decision line has the probability and the written threshold.
    assert_eq!(lines[1].p, Some(0.8));
    assert!((lines[1].threshold.unwrap() - 0.2).abs() < 1e-9);
}

#[tokio::test]
async fn it_is_bounded_one_round_back_then_a_report_then_done() {
    let (seen, reviewer) = script(vec![
        found("subtracts instead of adding"),
        found("still subtracts"),
    ]);
    let f = fixture(Fixed(Ok(0.9)), reviewer, false);
    f.prompt("Make add() add.").await;
    f.write(BUGGY);
    assert!(matches!(f.stop().await, Stop::Block { .. }));
    f.write("pub fn add(a: i32, b: i32) -> i32 {\n    b - a\n}\n");
    let Stop::Block { reason } = f.stop().await else {
        panic!("the remaining findings go back as a report");
    };
    assert!(reason.contains("Make no more changes") && reason.contains("still subtracts"));
    // Done: the next stop goes through unreviewed.
    assert_eq!(f.stop().await, Stop::Continue);
    assert_eq!(lock(&seen.seen).len(), 2);
    assert_eq!(lock(&f.outcomes)[0]["passed"], json!(false));
}

#[tokio::test]
async fn an_unchanged_send_back_is_reported_without_another_review() {
    let (seen, reviewer) = script(vec![found("subtracts instead of adding")]);
    let f = fixture(Fixed(Ok(0.9)), reviewer, false);
    f.prompt("Make add() add.").await;
    f.write(BUGGY);
    assert!(matches!(f.stop().await, Stop::Block { .. }));
    let Stop::Block { reason } = f.stop().await else {
        panic!("report");
    };
    assert!(reason.contains("subtracts instead of adding"));
    assert_eq!(lock(&seen.seen).len(), 1);
    let outcomes = lock(&f.outcomes);
    assert_eq!(outcomes[0]["labels"][0]["correct"], json!(false));
}

#[tokio::test]
async fn no_change_asks_nothing_and_a_small_one_below_the_line_is_not_reviewed() {
    let (seen, reviewer) = script(vec![]);
    let f = fixture(Fixed(Ok(0.9)), reviewer, false);
    f.prompt("What does add() do?").await;
    assert_eq!(f.stop().await, Stop::Continue);
    assert!(f.actions().is_empty(), "nothing to judge");

    let (seen_small, reviewer) = script(vec![found("never asked")]);
    let small = fixture(Fixed(Ok(0.1)), reviewer, false);
    small.prompt("Fix the typo.").await;
    small.write(FIXED);
    assert_eq!(small.stop().await, Stop::Continue);
    assert!(lock(&seen_small.seen).is_empty());
    assert_eq!(small.actions(), [(QUESTION.into(), "skip".into())]);
    assert!(lock(&seen.seen).is_empty());
}

#[tokio::test]
async fn a_failed_decision_or_review_lets_the_stop_through() {
    let (seen, reviewer) = script(vec![]);
    let f = fixture(Fixed(Err("unreachable")), reviewer, false);
    f.prompt("Make add() add.").await;
    f.write(BUGGY);
    assert_eq!(f.stop().await, Stop::Continue);
    assert!(lock(&seen.seen).is_empty());
    assert_eq!(f.actions(), [(QUESTION_SET.into(), "fallback".into())]);

    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let failing: Reviewer = Arc::new(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        Err("no model".into())
    });
    let f = fixture(Fixed(Ok(0.9)), failing, false);
    f.prompt("Make add() add.").await;
    f.write(BUGGY);
    assert_eq!(f.stop().await, Stop::Continue);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.actions(),
        [
            ("verdict".into(), "failed".into()),
            (QUESTION.into(), "review_failed".into())
        ]
    );
}

#[tokio::test]
async fn in_shadow_it_records_and_never_sends_back() {
    let (_seen, reviewer) = script(vec![found("subtracts")]);
    let f = fixture(Fixed(Ok(0.9)), reviewer, true);
    f.prompt("Make add() add.").await;
    f.write(BUGGY);
    assert_eq!(f.stop().await, Stop::Continue);
    assert_eq!(
        f.actions(),
        [
            ("verdict".into(), "send_back".into()),
            (QUESTION.into(), "send_back".into())
        ]
    );
}

#[tokio::test]
async fn the_coder_loop_task_starts_the_turn() {
    let (seen, reviewer) = script(vec![Verdict::Pass]);
    let f = fixture(Fixed(Ok(0.9)), reviewer, false);
    f.hooks
        .fire(Call::new(
            Event::SessionStart,
            json!({"task": "Make add() add.", "environment": "x"}),
        ))
        .await;
    f.write(BUGGY);
    // The loop's Stop carries no message at all.
    let stopped = f
        .hooks
        .fire(Call::new(
            Event::Stop,
            json!({"step": 3, "stop_hook_active": false}),
        ))
        .await;
    assert_eq!(stopped.stop, Stop::Continue);
    assert_eq!(stopped.permission, Permission::NoOpinion);
    assert_eq!(lock(&seen.seen)[0].request, "Make add() add.");
}

#[test]
fn verdicts_parse_typed_and_vague_send_backs_are_unsure() {
    assert_eq!(
        Verdict::parse(r#"{"verdict":"pass","findings":[]}"#),
        Ok(Verdict::Pass)
    );
    let text = "Here you go:\n```json\n{\"verdict\": \"send_back\", \"findings\": [{\"file\": \"a.rs\", \"line\": 2, \"problem\": \"off by one\"}]}\n```";
    assert_eq!(
        Verdict::parse(text),
        Ok(Verdict::SendBack {
            findings: vec![Finding {
                file: Some("a.rs".into()),
                line: Some(2),
                problem: "off by one".into()
            }]
        })
    );
    assert!(matches!(
        Verdict::parse(r#"{"verdict":"send_back","findings":[]}"#),
        Ok(Verdict::Unsure { .. })
    ));
    assert!(Verdict::parse("looks fine").is_err());
    assert!(Verdict::parse(r#"{"verdict":"maybe"}"#).is_err());
}

#[test]
fn the_switch_registers_the_hooks_on_and_in_shadow_only() {
    let dir = tempfile::tempdir().unwrap();
    let ids = |mode| {
        let mut registry = Registry::new();
        let (_, reviewer) = script(vec![]);
        register_with(
            &mut registry,
            dir.path(),
            mode,
            Arc::new(Fixed(Ok(0.5))),
            reviewer,
        );
        let mut ids = registry
            .hooks()
            .map(|hook| hook.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    assert!(ids(crate::sdk_hooks::Mode::Off).is_empty());
    let on = ids(crate::sdk_hooks::Mode::On);
    assert_eq!(
        on,
        [
            "second-look",
            "second-look-start",
            "second-look-steps",
            "second-look-turn"
        ]
    );
    assert_eq!(ids(crate::sdk_hooks::Mode::Shadow), on);
    // Off by default, and not in the chats' hooks while off.
    assert!(!enabled());
    let ids: Vec<String> = crate::sdk_hooks::coder_loop(dir.path(), &Arc::default())
        .registry()
        .hooks()
        .map(|hook| hook.id.clone())
        .collect();
    assert!(
        !ids.iter().any(|id| id.starts_with("second-look")),
        "{ids:?}"
    );
}

#[test]
fn the_snapshot_sees_untracked_files_and_leaves_the_index_alone() {
    let (_, reviewer) = script(vec![]);
    let f = fixture(Fixed(Ok(0.5)), reviewer, false);
    let before = snapshot(f.repo.path()).unwrap();
    std::fs::write(f.repo.path().join("new.txt"), "hello\n").unwrap();
    let after = snapshot(f.repo.path()).unwrap();
    assert_ne!(before, after);
    let (diff, counts) = changes(f.repo.path(), &before, &after);
    assert!(diff.contains("+hello"));
    assert_eq!(counts, [(1, 0, "new.txt".to_owned())]);
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(f.repo.path())
        .args(["status", "--short"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "?? new.txt\n");
}
