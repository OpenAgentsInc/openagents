use super::hands::{CODE_TOOLS, READ_TOOLS, Verdict, blocked, total_cost, with_analysis};
use super::{Hand, Options, Turn, issue_reference};
use crate::issue_run::agent::Work;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

#[test]
fn the_cli_reads_the_solver_options_and_opens_a_pr_by_default() {
    let options = Options::parse(&args(&[
        "11336",
        "--rounds",
        "3",
        "--coder-model",
        "claude-opus-5-5",
        "--briefer-model",
        "haiku",
        "--tester-model",
        "sonnet",
        "--plain",
        "--json",
        "--github",
        "o/r",
    ]))
    .unwrap();
    assert_eq!(options.inner.issue, 11336);
    assert_eq!(options.rounds, 3);
    assert_eq!(options.inner.model, "claude-opus-5-5");
    assert_eq!(options.briefer_model, "haiku");
    assert_eq!(options.tester_model, "sonnet");
    assert!(options.inner.open_pr);
    assert!(options.inner.plain);
    assert!(options.json);
    assert_eq!(options.inner.github, "o/r");
    assert!(options.inner.state.ends_with("solver-runs"));

    let quiet = Options::parse(&args(&["#7", "--no-pr"])).unwrap();
    assert!(!quiet.inner.open_pr);
    assert_eq!(quiet.inner.issue, 7);
    assert_eq!(quiet.rounds, 2);
}

#[test]
fn the_cli_refuses_issue_run_only_options_and_asks_for_an_issue() {
    assert!(Options::parse(&args(&["7", "--open-pr"])).is_err());
    assert!(Options::parse(&args(&["7", "--model", "x"])).is_err());
    let why = Options::parse(&args(&["--plain"])).unwrap_err();
    assert!(why.contains("coder solver"), "{why}");
    assert!(!why.contains("issue-run"), "{why}");
}

#[test]
fn the_conversation_reads_an_issue_reference() {
    assert_eq!(issue_reference("42"), Some((None, 42)));
    assert_eq!(issue_reference(" #42 "), Some((None, 42)));
    assert_eq!(
        issue_reference("OpenAgentsInc/openagents#42"),
        Some((Some("OpenAgentsInc/openagents".into()), 42))
    );
    assert_eq!(issue_reference("fix the login bug"), None);
    assert_eq!(issue_reference("a/b/c#1"), None);
    assert_eq!(issue_reference("#"), None);
}

#[test]
fn each_hand_holds_only_its_tools() {
    assert_eq!(READ_TOOLS, ["Read", "Grep", "Glob"]);
    assert_eq!(CODE_TOOLS, ["Read", "Edit", "Write", "Grep", "Glob"]);
    assert!(!CODE_TOOLS.contains(&"Bash"));
    assert!(!READ_TOOLS.iter().any(|t| matches!(*t, "Edit" | "Write")));
}

#[test]
fn the_tester_verdict_is_read_from_its_last_line_and_never_defaults_to_pass() {
    assert_eq!(Verdict::parse("All good.\nVERDICT: pass"), Verdict::Pass);
    assert_eq!(
        Verdict::parse("VERDICT: coder: fix src/a.rs, test x fails"),
        Verdict::Coder("fix src/a.rs, test x fails".into())
    );
    assert_eq!(
        Verdict::parse("VERDICT: Briefer: the handler lives elsewhere"),
        Verdict::Briefer("the handler lives elsewhere".into())
    );
    assert!(matches!(
        Verdict::parse("looks fine to me"),
        Verdict::Coder(_)
    ));
    assert!(matches!(
        Verdict::parse("VERDICT: maybe"),
        Verdict::Coder(_)
    ));
    // The last verdict line wins.
    assert!(matches!(
        Verdict::parse("VERDICT: pass\nwait\nVERDICT: coder: no"),
        Verdict::Coder(_)
    ));
}

fn turn(hand: Hand, reply: Option<&str>, cost: Option<f64>, error: Option<&str>) -> Turn {
    Turn {
        hand,
        round: 0,
        work: Work {
            cost_usd: cost,
            error: error.map(str::to_owned),
            reply: reply.map(str::to_owned),
            ..Work::default()
        },
        verdict: reply.map(str::to_owned),
    }
}

#[test]
fn the_intaker_blocks_only_on_a_blocked_line_or_a_failed_session() {
    let ready = turn(Hand::Intaker, Some("Title.\nINTAKE: ready"), None, None);
    assert_eq!(blocked(&ready), None);
    let dirty = turn(
        Hand::Intaker,
        Some("INTAKE: blocked: gh is missing"),
        None,
        None,
    );
    assert_eq!(
        blocked(&dirty).as_deref(),
        Some("The Intaker found a blocker: gh is missing")
    );
    let silent = turn(Hand::Intaker, Some("hello"), None, None);
    assert_eq!(blocked(&silent), None);
    let failed = turn(Hand::Intaker, None, None, Some("Claude Code did not start"));
    assert!(blocked(&failed).unwrap().contains("did not start"));
}

#[test]
fn the_analysis_replaces_an_earlier_round_and_costs_add_up() {
    let once = with_analysis("system", "first");
    let twice = with_analysis(&once, "second");
    assert_eq!(twice, "system\n\n## Briefer's analysis\n\nsecond");
    assert_eq!(total_cost(&[]), None);
    let turns = [
        turn(Hand::Coder, None, Some(0.5), None),
        turn(Hand::Tester, None, None, None),
        turn(Hand::Briefer, None, Some(0.25), None),
    ];
    assert_eq!(total_cost(&turns), Some(0.75));
}

#[test]
fn solver_asks_no_decision_model_and_refuses_a_decider() {
    let options = Options::parse(&["7".to_owned()]).unwrap();
    assert_eq!(options.tester_model, "haiku");
    assert_eq!(options.inner.decider, crate::issue_run::Decider::Off);
    let refused = Options::parse(&["7".into(), "--decider".into(), "jev".into()]).unwrap_err();
    assert!(refused.contains("Unknown option --decider"), "{refused}");
}

#[test]
fn the_coder_may_compile_but_leaves_tests_to_the_tester() {
    use crate::issue_run::decide::{Briefing, Check};
    let check = |id: &str| Check {
        id: id.into(),
        argv: vec!["cargo".into()],
        filter: false,
        what: String::new(),
        required: false,
    };
    let briefing = Briefing {
        checks: vec![
            check("check:x402"),
            check("test:x402"),
            check("tests:scripts"),
        ],
        ..Default::default()
    };
    let ids: Vec<String> = super::hands::compile_checks(&briefing)
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids, ["check:x402"]);
}

#[test]
fn the_intaker_names_the_issue_on_its_last_issue_line() {
    use super::chosen;
    assert_eq!(chosen("Here are two.\nWhich one?"), None);
    assert_eq!(
        chosen("Going with this.\nISSUE: OpenAgentsInc/openagents#11279"),
        Some((Some("OpenAgentsInc/openagents".into()), 11279))
    );
    assert_eq!(chosen("ISSUE: #42"), Some((None, 42)));
    assert_eq!(chosen("ISSUE: not a number"), None);
}

#[test]
fn the_intaker_gh_is_read_only() {
    use super::gh_read_only;
    let ok = |w: &[&str]| gh_read_only(&w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
    assert!(ok(&["issue", "list", "--label", "bug", "--state", "open"]));
    assert!(ok(&["issue", "view", "11279", "--json", "title,body"]));
    assert!(ok(&["search", "issues", "daily cap"]));
    assert!(ok(&["pr", "view", "11337"]));
    assert!(ok(&[
        "project",
        "item-list",
        "22",
        "--owner",
        "OpenAgentsInc",
        "--format",
        "json"
    ]));
    assert!(!ok(&["project", "item-edit", "--id", "x"]));
    assert!(!ok(&["project", "item-add", "22"]));
    assert!(!ok(&["issue", "close", "11279"]));
    assert!(!ok(&["issue", "comment", "1", "-b", "x"]));
    assert!(!ok(&["issue", "edit", "1"]));
    assert!(!ok(&["api", "repos/x/y/issues"]));
    assert!(!ok(&["issue", "view", "1", "--web"]));
    assert!(!ok(&[]));
}

/// One real Intaker turn on this computer's Claude login (run by hand:
/// `cargo test -p coder-new --lib live_intaker -- --ignored --nocapture`).
#[tokio::test]
#[ignore = "needs a Claude Code login and gh"]
async fn live_intaker_finds_and_names_an_issue() {
    let messages = vec![openrouter::Message::user(
        "Pick an open issue from https://github.com/orgs/OpenAgentsInc/projects/22 that isn't Done. Just pick it.",
    )];
    let prompt = super::intake_task(&messages, super::DEFAULT_GITHUB);
    let mut tools = Vec::new();
    let mut emit = |event: crate::bundled_runtime::RuntimeEvent| {
        if let crate::bundled_runtime::RuntimeEvent::Tool {
            name,
            running: false,
            ..
        } = event
        {
            tools.push(name);
        }
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let reply = crate::microclaude::run_with(
        &prompt,
        std::path::Path::new("."),
        &["Read", "Grep", "Glob"],
        Some(super::intake_chat_server(super::DEFAULT_GITHUB)),
        &cancel,
        &mut emit,
    )
    .await
    .unwrap();
    println!("tools: {tools:?}\nreply:\n{}", reply.text);
    assert!(super::chosen(&reply.text).is_some());
    assert!(tools.iter().any(|t| t == "mcp__intake__gh"));
}

#[test]
fn intaker_lists_reach_only_recent_issues() {
    use super::{civil_date, intake_task, recent_only};
    let words = |w: &[&str]| w.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
    assert_eq!(civil_date(0), "1970-01-01");
    assert_eq!(civil_date(20_737), "2026-10-11");
    assert_eq!(
        recent_only(words(&["issue", "list", "--state", "closed"]), "2026-09-20"),
        words(&[
            "issue",
            "list",
            "--state",
            "closed",
            "--search",
            "created:>=2026-09-20"
        ])
    );
    assert_eq!(
        recent_only(
            words(&["issue", "list", "--search", "reason:not-planned"]),
            "2026-09-20"
        ),
        words(&[
            "issue",
            "list",
            "--search",
            "reason:not-planned created:>=2026-09-20"
        ])
    );
    assert_eq!(
        recent_only(words(&["search", "issues", "bunny"]), "2026-09-20"),
        words(&["search", "issues", "bunny", "--created=>=2026-09-20"])
    );
    // A dated query and a single issue are left alone.
    let dated = words(&["issue", "list", "--search", "created:>=2026-01-01"]);
    assert_eq!(recent_only(dated.clone(), "2026-09-20"), dated);
    let view = words(&["issue", "view", "8205"]);
    assert_eq!(recent_only(view.clone(), "2026-09-20"), view);
    assert!(intake_task(&[], "OpenAgentsInc/openagents").contains("Pick only recent issues"));
}

#[test]
fn a_later_round_sees_its_own_change_and_the_real_errors() {
    use super::hands::{failing_output, so_far_text};
    let shown = so_far_text("--- a/docs/zones.md\n+++ b/docs/zones.md\n-old\n+new\n");
    assert!(shown.contains("not in the base"));
    assert!(shown.contains("+new"));
    assert!(so_far_text("").contains("no change"));
    let outputs = vec![
        ("check:a".to_owned(), true, "fine".to_owned()),
        (
            "check:b".to_owned(),
            false,
            "Compiling b\nerror[E0425]: cannot find value `x`\n  --> src/lib.rs:3:5\nerror: could not compile `b`".to_owned(),
        ),
    ];
    let text = failing_output(&outputs);
    assert!(text.contains("`check:b` failed"));
    assert!(text.contains("E0425"));
    assert!(!text.contains("check:a"));
}

#[test]
fn the_harness_runs_required_and_wasm_checks() {
    use crate::issue_run::{agent::harness_runs, decide::Check};
    let check = |id: &str, required: bool| Check {
        id: id.into(),
        argv: Vec::new(),
        filter: false,
        what: String::new(),
        required,
    };
    assert!(harness_runs(&check("check:a", false)));
    assert!(harness_runs(&check("wasm:a", true)));
    assert!(harness_runs(&check("links:docs", true)));
    assert!(!harness_runs(&check("test:a", false)));
    assert!(!harness_runs(&check("fmt:a", false)));
}

#[test]
fn a_small_change_starts_on_the_small_coder_and_a_hand_back_uses_the_full_one() {
    use super::{coder_model, hands::Size};
    assert_eq!(Size::parse("## Files\n...\nSIZE: small"), Size::Small);
    assert_eq!(Size::parse("SIZE: `Large`"), Size::Large);
    assert_eq!(Size::parse("no size line"), Size::Large);
    let options = Options::parse(&args(&["11338"])).unwrap();
    assert_eq!(coder_model(&options, Size::Small, 0), "sonnet");
    assert_eq!(coder_model(&options, Size::Small, 1), options.inner.model);
    assert_eq!(coder_model(&options, Size::Large, 0), options.inner.model);
    let fixed = Options::parse(&args(&["11338", "--coder-model", "claude-opus-5-5"])).unwrap();
    assert_eq!(coder_model(&fixed, Size::Small, 0), "claude-opus-5-5");
}

#[test]
fn the_run_cost_counts_every_hand() {
    use super::with_hands_cost;
    let mut summary = serde_json::json!({
        "agent_usd": 0.07,
        "cost": {"components": {"agent": {"usd": 0.07}, "decisions": {"usd": 0.0}}, "total_usd": 0.07},
    });
    let turns = [
        turn(Hand::Intaker, None, Some(0.001), None),
        turn(Hand::Briefer, None, Some(0.008), None),
        turn(Hand::Coder, None, Some(0.07), None),
        turn(Hand::Tester, None, Some(0.03), None),
    ];
    with_hands_cost(&mut summary, &turns);
    let total = summary["cost"]["total_usd"].as_f64().unwrap();
    assert!((total - 0.109).abs() < 1e-9, "{total}");
    assert!(
        (summary["cost"]["components"]["hands"]["tester"]["usd"]
            .as_f64()
            .unwrap()
            - 0.03)
            .abs()
            < 1e-9
    );
}

#[test]
fn failing_tests_are_read_from_cargo_output_once_each() {
    let text = "running 3 tests\ntest a::one ... ok\ntest a::two ... FAILED\ntest b::three ... FAILED\n\nfailures:\n\n---- a::two stdout ----\ntest a::two ... FAILED\n";
    assert_eq!(
        crate::issue_run::agent::failing_tests(text),
        vec!["a::two".to_owned(), "b::three".to_owned()]
    );
    assert!(crate::issue_run::agent::failing_tests("test result: ok. 3 passed").is_empty());
}

#[test]
fn a_linker_or_disk_fault_is_the_computers_not_the_changes() {
    let outputs = vec![
        ("check:a".to_owned(), true, "ok".to_owned()),
        (
            "test:b".to_owned(),
            false,
            "Compiling b\nerror: linking with `cc` failed: exit status: 1\n  = note: ..."
                .to_owned(),
        ),
    ];
    assert_eq!(
        super::hands::environment_fault(&outputs).as_deref(),
        Some("`test:b`: error: linking with `cc` failed: exit status: 1.")
    );
    let real = vec![(
        "test:b".to_owned(),
        false,
        "test a::two ... FAILED\nerror: test failed".to_owned(),
    )];
    assert_eq!(super::hands::environment_fault(&real), None);
}

#[test]
fn a_failed_check_keeps_the_names_of_the_failed_tests() {
    let text = "running 2 tests\ntest a::one ... ok\ntest a::two ... FAILED\n\nfailures:\n\n---- a::two stdout ----\npanicked at x\n\nfailures:\n    a::two\n\ntest result: FAILED. 1 passed; 1 failed\n";
    let shown = crate::issue_run::agent::trimmed(text, false);
    assert!(shown.contains("test a::two ... FAILED"), "{shown}");
    assert_eq!(
        crate::issue_run::agent::failing_tests(&shown),
        vec!["a::two".to_owned()]
    );
}

#[test]
fn an_excused_check_reads_as_passed_in_the_coder_widget() {
    let text = "cargo test -p a: passed\ntest a::two ... FAILED\n\nPASSED anyway: every failed test (a::two) already fails at the base commit abc, before this change.";
    let output = crate::issue_run::agent::coder_output(
        "Run",
        &serde_json::Value::Null,
        &serde_json::json!({}),
        text,
        None,
        false,
    );
    assert_eq!(output["result"], "passed", "{output}");
    assert!(output.get("error").is_none());
}

#[test]
fn a_touched_crates_tests_are_required_even_when_the_briefing_listed_them() {
    use crate::issue_run::decide::Check;
    let check = |id: &str, required: bool| Check {
        id: id.into(),
        argv: Vec::new(),
        filter: true,
        what: String::new(),
        required,
    };
    let mut checks = vec![check("check:a", false), check("test:a", false)];
    let mut required = vec!["check:a".to_owned()];
    super::merge_change_checks(
        &mut checks,
        &mut required,
        vec![
            check("check:a", false),
            check("test:a", true),
            check("test:b", true),
        ],
    );
    assert_eq!(required, ["check:a", "test:a", "test:b"]);
    assert!(checks.iter().find(|c| c.id == "test:a").unwrap().required);
    assert!(checks.iter().any(|c| c.id == "test:b" && c.required));
    assert_eq!(checks.len(), 3);
}

#[tokio::test]
async fn a_finished_runs_baseline_checkout_is_removed() {
    use crate::issue_run::agent::Baseline;
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let git = |args: &[&str], at: &std::path::Path| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(at)
                .output()
                .unwrap()
                .status
                .success()
        );
    };
    git(&["init", "-q"], &repo);
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    git(&["add", "."], &repo);
    git(
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "a",
        ],
        &repo,
    );
    let worktree = dir.path().join("run").join("baseline");
    git(
        &[
            "worktree",
            "add",
            "--detach",
            "-q",
            worktree.to_str().unwrap(),
            "HEAD",
        ],
        &repo,
    );
    let baseline = Baseline {
        repo: repo.clone(),
        commit: "HEAD".into(),
        worktree: worktree.clone(),
    };
    baseline.remove().await;
    assert!(!worktree.exists());
    // Removing it again, or one never made, does nothing.
    baseline.remove().await;
}

#[test]
fn a_sweep_removes_only_old_build_outputs() {
    use crate::issue_run::agent::sweep;
    let dir = tempfile::tempdir().unwrap();
    let deps = dir.path().join("debug/deps");
    let fingerprint = dir.path().join("debug/.fingerprint/x-1");
    std::fs::create_dir_all(&deps).unwrap();
    std::fs::create_dir_all(&fingerprint).unwrap();
    std::fs::write(deps.join("libx-1.rlib"), "x").unwrap();
    std::fs::write(fingerprint.join("lib-x"), "x").unwrap();
    std::fs::write(dir.path().join("debug/coder-new"), "bin").unwrap();
    // Nothing is old enough yet.
    assert_eq!(sweep(dir.path(), std::time::Duration::from_secs(3600)), 0);
    assert!(deps.join("libx-1.rlib").exists());
    // Everything in the build folders is older than no time at all.
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(sweep(dir.path(), std::time::Duration::from_millis(1)), 2);
    assert!(!deps.join("libx-1.rlib").exists());
    assert!(!fingerprint.exists(), "an emptied folder goes too");
    assert!(
        dir.path().join("debug/coder-new").exists(),
        "the binary stays"
    );
}
