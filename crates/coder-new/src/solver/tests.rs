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
