use super::*;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::local(dir.path().to_path_buf());
    (dir, store)
}

fn requester(account: &str) -> Requester {
    Requester {
        account: account.into(),
        workspace: "ws_1".into(),
        members_epoch: 1,
    }
}

fn ask(issue: u64) -> Ask {
    Ask {
        repo: "OpenAgentsInc/openagents".into(),
        issue,
        land: Land::Pr,
        engine: Engine::Briefed,
        source: "api".into(),
        title: None,
        github: None,
    }
}

/// Gives every requester a fake subscription token, or refuses one.
struct Fake {
    refuse: Option<&'static str>,
}

impl Credentials for Fake {
    fn release(&self, _: &Requester) -> Result<BTreeMap<String, String>, String> {
        match self.refuse {
            Some(why) => Err(why.into()),
            None => Ok(BTreeMap::from([(
                "CLAUDE_CODE_OAUTH_TOKEN".to_owned(),
                "sk-ant-oat01-test".to_owned(),
            )])),
        }
    }
}

const SIGNED_IN: Fake = Fake { refuse: None };

#[tokio::test]
async fn a_run_goes_from_the_request_to_a_host_and_back_with_its_result() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let run = submit(&store, &owner, requester("acct_alice"), ask(11257))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.state, RunState::Waiting);
    assert!(valid_id(&run.id));

    let taken = claim(&store, &SIGNED_IN, "oa-work-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(taken.run.id, run.id);
    assert_eq!(
        taken.env.get("CLAUDE_CODE_OAUTH_TOKEN").map(String::as_str),
        Some("sk-ant-oat01-test")
    );
    assert!(
        claim(&store, &SIGNED_IN, "oa-work-1")
            .await
            .unwrap()
            .is_none()
    );

    let heard = report(
        &store,
        "oa-work-1",
        &run.id,
        Report {
            lines: vec![Progress {
                secs: 6.2,
                phase: "briefing".into(),
                text: "briefing ready: 8 files".into(),
                ..Progress::default()
            }],
            title: Some("briefed-agent: the run summary says how verify went".into()),
            ..Report::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!heard.cancel);
    // Another host can't report on it.
    assert!(
        report(&store, "oa-work-2", &run.id, Report::default())
            .await
            .unwrap()
            .is_none()
    );

    let result = json!({
        "type": "result", "engine": "briefed", "escalated": null, "ok": true,
        "cost_usd": 0.4034, "secs": 74.4,
        "checks": [{"name": "cargo test -p briefed-agent", "ok": true, "passed": 20}],
        "diff": {"files": ["crates/briefed-agent/src/main.rs"], "added": 70, "removed": 4},
        "commit": "18d6ee1d2b0000000000000000000000000000ab",
        "landed": {"how": "pr", "url": "https://github.com/OpenAgentsInc/openagents/pull/1"},
        "briefing": {"files": ["crates/briefed-agent/src/main.rs"]},
    });
    report(
        &store,
        "oa-work-1",
        &run.id,
        Report {
            result: Some(result),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let done = load(&store, &owner, &run.id).await.unwrap().unwrap();
    assert_eq!(done.state, RunState::Done);
    let outcome = done.outcome.clone().unwrap();
    assert_eq!(outcome.engine, "briefed");
    assert_eq!(outcome.cost_usd, Some(0.4034));
    assert_eq!(
        outcome.pr.as_deref(),
        Some("https://github.com/OpenAgentsInc/openagents/pull/1")
    );
    assert_eq!(done.lines.len(), 1);
    assert_eq!(
        done.title.as_deref(),
        Some("briefed-agent: the run summary says how verify went")
    );
    // Off the queue: the account can be served again.
    let next = submit(&store, &owner, requester("acct_alice"), ask(11258))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claim(&store, &SIGNED_IN, "oa-work-1")
            .await
            .unwrap()
            .unwrap()
            .run
            .id,
        next.id
    );

    let shown = view(&done, 0);
    assert_eq!(shown["state"], "done");
    assert_eq!(shown["result"]["cost_usd"], 0.4034);
    let item = item(&done);
    assert_eq!(item.status, "done");
    assert_eq!(item.engine.as_deref(), Some("Briefed agent"));
}

/// A run's fleet row (#11250) through each stage the engine reports, from
/// a fixture stream of its progress lines: the line says the stage, the
/// cost and tokens follow each attempt, and the title starts `#N`.
#[tokio::test]
async fn a_runs_row_moves_through_the_stages_with_its_cost() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let mut asked = ask(11250);
    asked.land = Land::Queue;
    let run = submit(&store, &owner, requester("acct_alice"), asked)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(item(&run).line.as_deref(), Some("waiting for a computer"));
    claim(&store, &SIGNED_IN, "oa-work-1")
        .await
        .unwrap()
        .unwrap();
    let taken = load(&store, &owner, &run.id).await.unwrap().unwrap();
    assert_eq!(item(&taken).line.as_deref(), Some("starting"));
    let progress = |phase: &str, text: &str| Progress {
        phase: phase.into(),
        text: text.into(),
        ..Progress::default()
    };
    let cost = |usd: Option<f64>, tokens: u64| Progress {
        cost_usd: usd,
        tokens: Some(tokens),
        ..progress("cost", "so far")
    };
    let steps: Vec<(Vec<Progress>, &str, Option<f64>, Option<u64>)> = vec![
        (
            vec![
                progress("start", "working OpenAgentsInc/openagents#11250"),
                progress("issue", "#11250 Fleet rows for cloud runs"),
            ],
            "starting",
            None,
            None,
        ),
        (
            vec![progress("briefing", "finding the files this issue needs")],
            "finding files",
            None,
            None,
        ),
        (
            vec![progress("agent", "the briefed agent is working")],
            "coding",
            None,
            None,
        ),
        (
            vec![progress("verify", "verify: passed"), cost(Some(0.4), 1_000)],
            "coding",
            Some(0.4),
            Some(1_000),
        ),
        (
            vec![progress("checks", "running coder's tests")],
            "checks",
            Some(0.4),
            Some(1_000),
        ),
        (
            vec![
                progress("escalate", "the replayed checks failed; falling back"),
                progress("agent", "Claude Code is working"),
            ],
            "coding again with Claude Code",
            Some(0.4),
            Some(1_000),
        ),
        (
            vec![cost(None, 3_000)],
            "coding again with Claude Code",
            None,
            Some(3_000),
        ),
        (
            vec![progress("commit", "committed 18d6ee1d2b")],
            "landing",
            None,
            Some(3_000),
        ),
        (
            vec![Progress {
                entry: Some("20261011T080000Z-oa-work-1-ab12".into()),
                ..progress("land", "handed to the landing queue")
            }],
            "queued 20261011T080000Z-oa-work-1-ab12",
            None,
            Some(3_000),
        ),
    ];
    for (lines, stage_words, usd, tokens) in steps {
        report(
            &store,
            "oa-work-1",
            &run.id,
            Report {
                lines,
                title: Some("Fleet rows for cloud runs".into()),
                ..Report::default()
            },
        )
        .await
        .unwrap()
        .unwrap();
        let now = load(&store, &owner, &run.id).await.unwrap().unwrap();
        let row = item(&now);
        assert_eq!(row.line.as_deref(), Some(stage_words));
        assert_eq!(row.cost_usd, usd, "{stage_words}");
        assert_eq!(row.tokens, tokens, "{stage_words}");
        assert_eq!(row.title, "#11250 Fleet rows for cloud runs");
        assert_eq!(row.status, "working");
        assert_eq!(row.finished_unix, None);
    }
    report(
        &store,
        "oa-work-1",
        &run.id,
        Report {
            result: Some(json!({"engine": "bare", "ok": true, "cost_usd": 1.25,
                "commit": "18d6ee1d2b0000000000000000000000000000ab",
                "landed": {"how": "queue", "entry": {"id": "20261011T080000Z-oa-work-1-ab12"}}})),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let done = load(&store, &owner, &run.id).await.unwrap().unwrap();
    let row = item(&done);
    assert_eq!(row.status, "done");
    assert_eq!(
        row.line.as_deref(),
        Some("queued 20261011T080000Z-oa-work-1-ab12")
    );
    assert_eq!(row.cost_usd, Some(1.25));
    assert!(row.finished_unix.is_some());

    // A run that fails says why in one line.
    let failing = submit(&store, &owner, requester("acct_alice"), ask(7))
        .await
        .unwrap()
        .unwrap();
    claim(&store, &SIGNED_IN, "oa-work-1")
        .await
        .unwrap()
        .unwrap();
    report(
        &store,
        "oa-work-1",
        &failing.id,
        Report {
            result: Some(json!({"engine": "bare", "ok": false, "cost_usd": null,
                "error": "no change passed its checks"})),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let failed = load(&store, &owner, &failing.id).await.unwrap().unwrap();
    let row = item(&failed);
    assert_eq!(
        row.line.as_deref(),
        Some("failed: no change passed its checks")
    );
    assert_eq!(row.cost_usd, None, "unknown, never zero");
    assert_eq!(row.title, "#7 in OpenAgentsInc/openagents");
    let mut pr = done.clone();
    pr.outcome = Some(Outcome {
        pr: Some("https://github.com/OpenAgentsInc/openagents/pull/9".into()),
        ..Outcome::default()
    });
    assert_eq!(
        stage(&pr),
        "pull request https://github.com/OpenAgentsInc/openagents/pull/9"
    );
}

#[tokio::test]
async fn an_unknown_cost_stays_unknown() {
    let outcome = outcome(&json!({"engine": "bare", "ok": false, "cost_usd": null}));
    assert_eq!(outcome.cost_usd, None);
    assert_eq!(money(outcome.cost_usd), "unknown");
    assert_eq!(money(Some(0.5)), "$0.50");
}

#[tokio::test]
async fn one_accounts_runs_take_turns_and_others_go_first() {
    let (_dir, store) = store();
    let alice = account_owner("acct_alice");
    let bob = account_owner("acct_bob");
    let a1 = submit(&store, &alice, requester("acct_alice"), ask(1))
        .await
        .unwrap()
        .unwrap();
    let a2 = submit(&store, &alice, requester("acct_alice"), ask(2))
        .await
        .unwrap()
        .unwrap();
    let b1 = submit(&store, &bob, requester("acct_bob"), ask(3))
        .await
        .unwrap()
        .unwrap();
    let first = claim(&store, &SIGNED_IN, "h").await.unwrap().unwrap();
    assert_eq!(first.run.id, a1.id);
    // Alice has one running: Bob's goes next, then nobody's.
    let second = claim(&store, &SIGNED_IN, "h").await.unwrap().unwrap();
    assert_eq!(second.run.id, b1.id);
    assert!(claim(&store, &SIGNED_IN, "h").await.unwrap().is_none());
    report(
        &store,
        "h",
        &a1.id,
        Report {
            failed: Some("stopped".into()),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        claim(&store, &SIGNED_IN, "h")
            .await
            .unwrap()
            .unwrap()
            .run
            .id,
        a2.id
    );
}

#[tokio::test]
async fn a_run_without_a_saved_claude_sign_in_stops_and_says_so() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let run = submit(&store, &owner, requester("acct_alice"), ask(5))
        .await
        .unwrap()
        .unwrap();
    let none = Fake {
        refuse: Some("Save your own Claude login in Settings > Claude, then ask again."),
    };
    assert!(claim(&store, &none, "h").await.unwrap().is_none());
    let stopped = load(&store, &owner, &run.id).await.unwrap().unwrap();
    assert_eq!(stopped.state, RunState::Failed);
    assert!(stopped.why.unwrap().contains("Settings > Claude"));
}

#[tokio::test]
async fn cancelling_stops_a_waiting_run_at_once_and_a_running_one_at_its_next_report() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let waiting = submit(&store, &owner, requester("acct_alice"), ask(7))
        .await
        .unwrap()
        .unwrap();
    cancel(&store, &owner, &waiting.id).await.unwrap().unwrap();
    assert_eq!(
        load(&store, &owner, &waiting.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        RunState::Cancelled
    );
    let running = submit(&store, &owner, requester("acct_alice"), ask(8))
        .await
        .unwrap()
        .unwrap();
    claim(&store, &SIGNED_IN, "h").await.unwrap().unwrap();
    cancel(&store, &owner, &running.id).await.unwrap().unwrap();
    let heard = report(&store, "h", &running.id, Report::default())
        .await
        .unwrap()
        .unwrap();
    assert!(heard.cancel);
    // Another account can't see or stop it.
    let bob = account_owner("acct_bob");
    assert!(load(&store, &bob, &running.id).await.unwrap().is_none());
    assert!(cancel(&store, &bob, &running.id).await.unwrap().is_none());
}

#[tokio::test]
async fn bad_requests_are_refused() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let mut bad = ask(1);
    bad.repo = "not a repo".into();
    assert!(matches!(
        submit(&store, &owner, requester("acct_alice"), bad)
            .await
            .unwrap(),
        Err(Refused::Invalid(_))
    ));
    assert!(matches!(
        submit(&store, &owner, requester("acct_alice"), ask(0))
            .await
            .unwrap(),
        Err(Refused::Invalid(_))
    ));
    for n in 0..MAX_WAITING_EACH {
        submit(&store, &owner, requester("acct_alice"), ask(100 + n as u64))
            .await
            .unwrap()
            .unwrap();
    }
    assert_eq!(
        submit(&store, &owner, requester("acct_alice"), ask(999))
            .await
            .unwrap()
            .unwrap_err(),
        Refused::Busy
    );
}

#[test]
fn issue_addresses_and_repositories() {
    assert_eq!(
        issue_address("https://github.com/OpenAgentsInc/openagents/issues/11257"),
        Some(("OpenAgentsInc/openagents".into(), 11257))
    );
    assert_eq!(
        issue_address("github.com/a/b/issues/3#issuecomment-1"),
        Some(("a/b".into(), 3))
    );
    assert_eq!(
        issue_address("OpenAgentsInc/openagents#42"),
        Some(("OpenAgentsInc/openagents".into(), 42))
    );
    assert_eq!(issue_address("https://github.com/a/b/pull/3"), None);
    assert_eq!(issue_address("42"), None);
    assert!(valid_repo("OpenAgentsInc/openagents"));
    assert!(!valid_repo("OpenAgentsInc"));
    assert!(!valid_repo("a/b/c"));
    assert!(!valid_repo("../b"));
    assert_eq!(Land::parse("main"), Some(Land::Queue));
    assert_eq!(Land::parse("pr"), Some(Land::Pr));
    assert_eq!(Land::parse("push"), None);
    assert_eq!(Engine::parse(""), Some(Engine::Briefed));
    assert_eq!(Engine::parse("bare"), Some(Engine::Bare));
    assert!(owns("/v1/work"));
    assert!(owns("/v1/work/wrk0123"));
    assert!(owns("/v1/work-hosts/h/claim"));
    assert!(!owns("/v1/workspaces"));
}

#[test]
fn a_finished_runs_result_reads_plainly() {
    let mut run = WorkRun {
        schema: RUN_SCHEMA.into(),
        id: "wrk000000000000000000000000".into(),
        owner: "o".into(),
        requester: requester("acct_alice"),
        repo: "OpenAgentsInc/openagents".into(),
        issue: 11257,
        title: Some("A title".into()),
        land: Land::Pr,
        engine: Engine::Briefed,
        source: "web".into(),
        created_unix: 1,
        updated_unix: 1,
        state: RunState::Done,
        host: Some("h".into()),
        started_unix: Some(1),
        lines: Vec::new(),
        dropped: 0,
        outcome: Some(outcome(&json!({
            "engine": "bare", "ok": true, "cost_usd": null, "secs": 125.0,
            "escalated": "low briefing confidence",
            "checks": [{"name": "cargo test -p x", "ok": true}],
            "landed": {"how": "pr", "url": "https://github.com/a/b/pull/9"},
        }))),
        why: None,
        cancel: false,
        finished_unix: Some(2),
        github: None,
    };
    let html = html! { div { (result_markup(&run)) } }.into_string();
    assert!(
        html.contains("Claude Code · 2 min 5 s · unknown · checks 1/1"),
        "{html}"
    );
    assert!(html.contains("Handed to Claude Code: low briefing confidence"));
    crate::copy_guard::assert_plain("/work", &html);
    run.outcome = None;
    assert_eq!(result_markup(&run).into_string(), "");
}

#[tokio::test]
async fn a_finished_run_tells_its_chat_what_it_did() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let run = submit(&store, &owner, requester("acct_alice"), ask(11257))
        .await
        .unwrap()
        .unwrap();
    claim(&store, &SIGNED_IN, "h").await.unwrap().unwrap();
    report(
        &store,
        "h",
        &run.id,
        Report {
            result: Some(json!({"engine": "briefed", "ok": true, "cost_usd": 0.4, "secs": 74.0,
                "checks": [{"name": "cargo test -p a", "ok": true}],
                "landed": {"how": "pr", "url": "https://github.com/OpenAgentsInc/openagents/pull/7"}})),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let done = load(&store, &owner, &run.id).await.unwrap().unwrap();
    assert_eq!(
        said(&done),
        "Briefed agent finished OpenAgentsInc/openagents#11257 in 1 min 14 s for $0.40: checks 1/1 \
         passed, pull request https://github.com/OpenAgentsInc/openagents/pull/7."
    );
}

fn solver_ask(issue: u64, ticket: Option<&str>) -> Ask {
    Ask {
        repo: "AtlantisPleb/solver-box-smoke".into(),
        issue,
        land: Land::Pr,
        engine: Engine::Solver,
        source: "chat".into(),
        title: None,
        github: ticket.map(|t| Ticket::new(t.into())),
    }
}

#[tokio::test]
async fn solver_runs_go_only_to_hosts_that_name_them_with_the_ticket_once() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let solver = submit(
        &store,
        &owner,
        requester("acct_alice"),
        solver_ask(5, Some("ogb_a_b_c")),
    )
    .await
    .unwrap()
    .unwrap();
    // The ticket is kept, never answered.
    let stored = load(&store, &owner, &solver.id).await.unwrap().unwrap();
    assert!(stored.github.is_some());
    assert!(!view(&stored, 0).to_string().contains("ogb_"));
    assert_eq!(format!("{:?}", stored.github), "Some(Ticket(…))");
    let briefed = submit(&store, &owner, requester("acct_alice"), ask(7))
        .await
        .unwrap()
        .unwrap();

    // A Claude work host never gets a Solver run.
    let taken = claim(&store, &SIGNED_IN, "oa-work-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(taken.run.id, briefed.id);
    assert!(
        claim(&store, &SIGNED_IN, "oa-work-1")
            .await
            .unwrap()
            .is_none()
    );

    // The dispatcher takes it while the account's briefed run still goes
    // (no turns), without Claude credentials, and gets the ticket once.
    let taken = claim_for(&store, None, "oa-solver-1", Some(&[Engine::Solver]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(taken.run.id, solver.id);
    assert!(taken.env.is_empty());
    assert_eq!(taken.run.github, Some(Ticket::new("ogb_a_b_c".into())));
    let after = load(&store, &owner, &solver.id).await.unwrap().unwrap();
    assert_eq!(after.state, RunState::Running);
    assert!(after.github.is_none(), "the ticket left the record");
    assert!(
        claim_for(&store, None, "oa-solver-1", Some(&[Engine::Solver]))
            .await
            .unwrap()
            .is_none()
    );

    // Its progress and its result, with the box and the model apart.
    let lines = [
        "box", "briefing", "thinking", "tool", "checks", "pr", "cost",
    ]
    .iter()
    .zip([
        "Computer bx_10m5t58f98 is up",
        "The issue: Add a word counter",
        "I will add a function.",
        "Edit crates/textkit/src/lib.rs",
        "2 of 2 checks passed",
        "https://github.com/AtlantisPleb/solver-box-smoke/pull/6",
        "$0.04 in model calls",
    ])
    .map(|(phase, text)| Progress {
        secs: 1.0,
        phase: (*phase).into(),
        text: text.into(),
        ..Progress::default()
    })
    .collect();
    let heard = report(
        &store,
        "oa-solver-1",
        &solver.id,
        Report {
            lines,
            ..Report::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!heard.cancel);
    let going = load(&store, &owner, &solver.id).await.unwrap().unwrap();
    assert_eq!(stage(&going), "opening a pull request");
    report(
        &store,
        "oa-solver-1",
        &solver.id,
        Report {
            result: Some(json!({
                "engine": "solver", "ok": true, "outcome": "passed",
                "cost_usd": 0.0534, "model_usd": 0.0434, "secs": 169.0,
                "checks": [{"name": "check:textkit", "ok": true}, {"name": "test:textkit", "ok": true}],
                "diff": {"files": ["crates/textkit/src/lib.rs"], "added": 13, "removed": 0},
                "commit": "25bb7d9",
                "landed": {"url": "https://github.com/AtlantisPleb/solver-box-smoke/pull/6"},
                "box": {"sandbox": "bx_5ij1cb060a", "usd": 0.01, "secs": 108.0},
            })),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let done = load(&store, &owner, &solver.id).await.unwrap().unwrap();
    assert_eq!(done.state, RunState::Done);
    let outcome = done.outcome.as_ref().unwrap();
    assert_eq!(outcome.files, vec!["crates/textkit/src/lib.rs".to_owned()]);
    assert_eq!(outcome.box_usd, Some(0.01));
    assert_eq!(
        said(&done),
        "Solver in the cloud finished AtlantisPleb/solver-box-smoke#5 in 2 min 49 s for $0.05 \
         ($0.04 model, $0.01 computer): checks 2/2 passed, pull request \
         https://github.com/AtlantisPleb/solver-box-smoke/pull/6."
    );
    assert_eq!(
        stage(&done),
        "pull request https://github.com/AtlantisPleb/solver-box-smoke/pull/6"
    );
}

#[tokio::test]
async fn a_site_without_claude_sign_ins_hands_out_only_solver_runs() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    submit(&store, &owner, requester("acct_alice"), ask(7))
        .await
        .unwrap()
        .unwrap();
    // A host naming no engines, with no Claude store here: nothing.
    assert!(claim_for(&store, None, "h", None).await.unwrap().is_none());
    let solver = submit(&store, &owner, requester("acct_alice"), solver_ask(5, None))
        .await
        .unwrap()
        .unwrap();
    let taken = claim_for(&store, None, "h", Some(&[Engine::Solver, Engine::Briefed]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(taken.run.id, solver.id);
    assert_eq!(taken.run.github, None);
}

#[tokio::test]
async fn stopping_a_solver_run_reaches_its_host_and_drops_a_waiting_ticket() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let waiting = submit(
        &store,
        &owner,
        requester("acct_alice"),
        solver_ask(5, Some("ogb_a_b_c")),
    )
    .await
    .unwrap()
    .unwrap();
    cancel(&store, &owner, &waiting.id).await.unwrap().unwrap();
    let stopped = load(&store, &owner, &waiting.id).await.unwrap().unwrap();
    assert_eq!(stopped.state, RunState::Cancelled);
    assert!(stopped.github.is_none());

    let running = submit(&store, &owner, requester("acct_alice"), solver_ask(6, None))
        .await
        .unwrap()
        .unwrap();
    claim_for(&store, None, "s", Some(&[Engine::Solver]))
        .await
        .unwrap()
        .unwrap();
    cancel(&store, &owner, &running.id).await.unwrap().unwrap();
    let heard = report(&store, "s", &running.id, Report::default())
        .await
        .unwrap()
        .unwrap();
    assert!(heard.cancel, "the host is told to stop and delete its box");
    report(
        &store,
        "s",
        &running.id,
        Report {
            result: Some(json!({"engine": "solver", "ok": false, "secs": 81.0,
                "cost_usd": null, "checks": [], "box": {"usd": 0.004}})),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let ended = load(&store, &owner, &running.id).await.unwrap().unwrap();
    assert_eq!(ended.state, RunState::Cancelled);
    assert_eq!(
        said(&ended),
        "Solver in the cloud on AtlantisPleb/solver-box-smoke#6 stopped after 1 min 21 s, as \
         asked; its computer was deleted. It cost $0.00."
    );
}

#[test]
fn solver_progress_reads_in_plain_words() {
    let words = |phase: &str, text: &str| {
        solver_words(&Progress {
            phase: phase.into(),
            text: text.into(),
            ..Progress::default()
        })
    };
    assert_eq!(
        words("box", "Computer bx_10m5t58f98 is up").as_deref(),
        Some("The computer is up")
    );
    assert_eq!(
        words("box", "Deleted computer bx_10m5t58f98").as_deref(),
        Some("Deleted the computer")
    );
    assert_eq!(
        words("box", "Installing Docker on the computer").as_deref(),
        Some("Setting up the computer")
    );
    assert_eq!(words("stage", "You are the Briefer. Read …"), None);
    assert_eq!(
        words("tool", "Edit crates/textkit/src/lib.rs").as_deref(),
        Some("Editing crates/textkit/src/lib.rs")
    );
    assert_eq!(
        words("tool", "Bash cargo test -p textkit").as_deref(),
        Some("Running cargo test -p textkit")
    );
    assert_eq!(
        words("thinking", "I will add\na function.").as_deref(),
        Some("Thinking: I will add a function.")
    );
    assert_eq!(
        words(
            "thinking",
            "## Files to change\n- `a.rs`: add it\n```rust\nfn a() {}\n```"
        )
        .as_deref(),
        Some("Thinking: Files to change - a.rs: add it fn a() {}")
    );
    assert_eq!(
        words(
            "tool",
            "Run git commit -q -m textkit: add (#7) Made by Coder"
        )
        .as_deref(),
        Some("Committing the change")
    );
    assert_eq!(
        words("tool", "Run git switch -c coder/issue-7").as_deref(),
        Some("Making the branch coder/issue-7")
    );
    assert_eq!(words("tool", "Run git add -A"), None);
    assert_eq!(words("tool", "Summary"), None);
    assert_eq!(
        words(
            "briefing",
            "Context finder: candidates, then a learned scorer (#11210)"
        )
        .as_deref(),
        Some("Context finder: candidates, then a learned scorer")
    );
    assert_eq!(
        words(
            "briefing",
            "Briefing: plan and rules (#11211's generator), checked"
        )
        .as_deref(),
        Some("Briefing: plan and rules, checked")
    );
    assert_eq!(
        words("box", "Solver on issue #7 with google/gemini-4-argon").as_deref(),
        Some("Solver started on issue #7")
    );
    assert_eq!(words("box", "The solver exited 0"), None);
    assert_eq!(
        words(
            "notice",
            "Solver ended: passed. Working copy: /work/state/x"
        ),
        None
    );
    assert_eq!(
        words("pr", "https://github.com/a/b/pull/2").as_deref(),
        Some("Opened a pull request: https://github.com/a/b/pull/2")
    );
    let page: String = ["box", "briefing", "thinking", "checks", "cost"]
        .iter()
        .zip([
            "Asking for a computer",
            "The briefing: 3 files",
            "Reading the issue.",
            "2 of 2 checks passed",
            "$0.04 in model calls",
        ])
        .filter_map(|(phase, text)| words(phase, text))
        .collect::<Vec<_>>()
        .join("\n");
    crate::copy_guard::assert_plain("/chat", &page);
}

#[test]
fn claims_name_their_engines_and_answers_read_the_token() {
    assert_eq!(Engine::parse("solver"), Some(Engine::Solver));
    assert_eq!(Engine::Solver.word(), "solver");
    assert!(!Engine::Solver.uses_claude());
    assert_eq!(
        password("username=x-access-token\npassword=ghs_abc\npassword_expiry_utc=1\n").as_deref(),
        Some("ghs_abc")
    );
    assert_eq!(password("error=ticket_refused\n"), None);
}

#[tokio::test]
async fn a_solver_run_that_finds_the_issue_already_done_ends_done_and_says_so() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let run = submit(
        &store,
        &owner,
        requester("acct_alice"),
        Ask {
            repo: "OpenAgentsInc/openagents".into(),
            ..solver_ask(11579, None)
        },
    )
    .await
    .unwrap()
    .unwrap();
    claim_for(&store, None, "oa-solver-1", Some(&[Engine::Solver]))
        .await
        .unwrap()
        .unwrap();
    let notice = |text: &str| Progress {
        secs: 90.0,
        phase: "notice".into(),
        text: text.into(),
        ..Progress::default()
    };
    // The solver's "no working copy" line comes before its real reason:
    // the chat leaves it out.
    assert_eq!(
        solver_words(&notice("No pull request: the run has no working copy.")),
        None
    );
    report(
        &store,
        "oa-solver-1",
        &run.id,
        Report {
            result: Some(json!({
                "engine": "solver", "ok": false,
                "outcome": {"status": "already_done", "reason": "Main already has a commit for this issue: e3646e3d38 Drive answers that look at videos get 330 s, not the chat's 130 s (#11579)", "delivers": false},
                "cost_usd": null, "model_usd": null, "secs": 228.0, "checks": [],
                "box": {"sandbox": "bx_gt7mfj6isw", "usd": 0.01, "secs": 110.0},
            })),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let done = load(&store, &owner, &run.id).await.unwrap().unwrap();
    assert_eq!(done.state, RunState::Done, "nothing to do is not a failure");
    assert_eq!(done.why, None);
    assert_eq!(stage(&done), "already done on main: e3646e3d38");
    let text = said(&done);
    assert_eq!(
        text,
        "Solver in the cloud found OpenAgentsInc/openagents#11579 already done on main: \
         [e3646e3d38](https://github.com/OpenAgentsInc/openagents/commit/e3646e3d38) Drive answers \
         that look at videos get 330 s, not the chat's 130 s (#11579). Nothing was changed \
         (checked in 3 min 48 s for $0.01 of computer time)."
    );
    assert!(
        !text.contains("unknown") && !text.contains("checks"),
        "{text}"
    );
}

#[tokio::test]
async fn a_solver_run_with_no_change_and_no_checks_never_claims_checks_failed() {
    let (_dir, store) = store();
    let owner = account_owner("acct_alice");
    let ask = Ask {
        repo: "OpenAgentsInc/openagents".into(),
        ..solver_ask(3, None)
    };
    let run = submit(&store, &owner, requester("acct_alice"), ask)
        .await
        .unwrap()
        .unwrap();
    claim_for(&store, None, "oa-solver-1", Some(&[Engine::Solver]))
        .await
        .unwrap()
        .unwrap();
    report(
        &store,
        "oa-solver-1",
        &run.id,
        Report {
            result: Some(json!({
                "engine": "solver", "ok": false, "outcome": null,
                "cost_usd": null, "secs": 30.0, "checks": [],
            })),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let failed = load(&store, &owner, &run.id).await.unwrap().unwrap();
    assert_eq!(failed.state, RunState::Failed);
    assert_eq!(
        failed.why.as_deref(),
        Some("The run ended without a change.")
    );
    let text = said(&failed);
    assert_eq!(
        text,
        "Solver in the cloud stopped on OpenAgentsInc/openagents#3 in 30 s. The run ended without a change."
    );
}
