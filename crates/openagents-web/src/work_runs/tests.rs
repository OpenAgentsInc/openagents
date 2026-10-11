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
        refuse: Some("Save your own Claude sign-in in Settings > Claude, then ask again."),
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
