use super::*;
use serde_json::json;

#[test]
fn arguments_take_a_list_of_issues_and_the_persons_limits() {
    let request = arguments(&json!({"issues": [11346, "#11347", "acme/app#7"]})).unwrap();
    assert_eq!(
        request.issues,
        vec![(None, 11346), (None, 11347), (Some("acme/app".into()), 7)]
    );
    assert!(!request.no_pr);
    assert_eq!(request.concurrency, default_concurrency());

    let request = arguments(&json!({
        "issues": [11346],
        "no_pr": true,
        "concurrency": 3,
        "budget_usd": 12.5
    }))
    .unwrap();
    assert!(request.no_pr);
    assert_eq!(request.concurrency, 3);
    assert_eq!(request.budget_usd, Some(12.5));
}

#[test]
fn arguments_refuse_a_bad_list_or_an_unknown_field() {
    assert!(arguments(&json!({})).is_err());
    assert!(arguments(&json!({"issues": []})).is_err());
    assert!(arguments(&json!({"issues": ["nope"]})).is_err());
    assert!(arguments(&json!({"issues": [1], "extra": true})).is_err());
    assert!(arguments(&json!({"issues": [1], "concurrency": 0})).is_err());
    assert!(arguments(&json!({"issues": [1], "concurrency": 99})).is_err());
    assert!(arguments(&json!({"issues": [1], "budget_usd": -1})).is_err());
    // Duplicates collapse.
    let request = arguments(&json!({"issues": [5, "#5"]})).unwrap();
    assert_eq!(request.issues.len(), 1);
}

#[test]
fn the_task_line_names_every_issue_once() {
    let request = arguments(&json!({"issues": [11346, "#11347"], "no_pr": true})).unwrap();
    let task = task(&request);
    assert!(task.starts_with("Solve 2 issues at once"), "{task}");
    assert!(task.contains("Issues: #11346, #11347."), "{task}");
    assert!(task.contains("without opening pull requests"));
}

/// The recorded solver run's summary shape: what a row carries after a run.
#[test]
fn a_finished_solver_fills_its_row_and_posts_its_intent() {
    let dir = tempfile::tempdir().unwrap();
    let channel = Channel::open(dir.path(), "swarm").unwrap();
    let mut row = Row {
        member: Member::channel_name(11346),
        issue: 11346,
        stage: Stage::Running,
        outcome: None,
        pull_request: None,
        cost_usd: None,
        folder: None,
    };
    let summary = json!({
        "outcome": {"status": "passed", "reason": null},
        "solver": {"pull_request": "https://github.com/OpenAgentsInc/openagents/pull/1", "cost_usd": 0.42},
        "changed": ["crates/coder/src/lib.rs", "crates/coder/src/router.rs"],
        "folder": "/runs/11346",
    });
    record(&summary, &mut row, Some(&channel));
    assert_eq!(row.outcome.as_deref(), Some("passed"));
    assert_eq!(row.cost_usd, Some(0.42));
    assert_eq!(
        row.pull_request.as_deref(),
        Some("https://github.com/OpenAgentsInc/openagents/pull/1")
    );
    assert_eq!(row.folder, Some(PathBuf::from("/runs/11346")));
    // The intent reaches the channel, so another solver sees the overlap.
    let intents = channel.intents().unwrap();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].member, "solver-11346");
    assert_eq!(intents[0].crates, vec!["coder"]);
}

#[test]
fn two_solvers_touching_one_file_overlap_and_settle_an_order() {
    let dir = tempfile::tempdir().unwrap();
    let channel = Channel::open(dir.path(), "swarm").unwrap();
    // Solver A posts first, then B.
    channel
        .intent(Intent::new(
            "solver-1",
            vec!["crates/coder/src/lib.rs".into()],
            vec![],
        ))
        .unwrap();
    let overlaps = channel
        .intent(Intent::new(
            "solver-2",
            vec!["crates/coder/src/lib.rs".into()],
            vec![],
        ))
        .unwrap();
    assert_eq!(overlaps.len(), 1);
    assert_eq!(
        overlaps[0].members,
        ["solver-1".to_string(), "solver-2".to_string()]
    );
    let agreement = channel.settle_first("solver-1", "solver-2").unwrap();
    assert_eq!(agreement.detail, "solver-1");
    assert_eq!(agreement.decided_by, "swarm");

    let request = arguments(&json!({"issues": [1, 2]})).unwrap();
    let rows = vec![
        Row {
            member: "solver-1".into(),
            issue: 1,
            stage: Stage::Done,
            outcome: Some("passed".into()),
            pull_request: Some("https://github.com/x/pr/1".into()),
            cost_usd: Some(0.10),
            folder: None,
        },
        Row {
            member: "solver-2".into(),
            issue: 2,
            stage: Stage::Done,
            outcome: Some("failed".into()),
            pull_request: None,
            cost_usd: Some(0.05),
            folder: None,
        },
    ];
    let agreements = channel.agreements().unwrap();
    let summary = summary(&request, &rows, &agreements);
    assert_eq!(summary["overlaps_settled"], 1);
    assert_eq!(summary["merge_order"], json!(["solver-1"]));
    assert!((summary["cost"]["total_usd"].as_f64().unwrap() - 0.15).abs() < 1e-9);
    assert_eq!(summary["failures"][0]["member"], "solver-2");
    assert_eq!(summary["rows"][0]["outcome"], "passed");

    let reply = reply(&rows, Some(0.15));
    assert!(
        reply.starts_with("Solver swarm finished: 1 of 2 passed."),
        "{reply}"
    );
    assert!(reply.contains("Pull requests: https://github.com/x/pr/1."));
    assert!(reply.contains("Cost: $0.15."));
}

#[test]
fn the_budget_stops_new_solvers_once_the_finished_ones_have_spent_it() {
    let mut request = arguments(&json!({"issues": [1]})).unwrap();
    assert!(!over_budget(&request, 1000.0), "no limit set");
    request.budget_usd = Some(5.0);
    assert!(!over_budget(&request, 4.99));
    assert!(over_budget(&request, 5.0));
    assert!(over_budget(&request, 9.0));
}

#[test]
fn the_swarm_tool_is_listed_and_on_by_default() {
    let definition = crate::plugin_definition::DEFINITIONS
        .iter()
        .find(|definition| definition.id == PLUGIN)
        .expect("solver-swarm is a registered plugin");
    assert!(definition.default_enabled);
    let channel = crate::plugin_definition::DEFINITIONS
        .iter()
        .find(|definition| definition.id == crate::comms::PLUGIN)
        .expect("comms is a registered plugin");
    assert!(channel.default_enabled);
}

#[test]
fn the_concurrency_fits_the_machine() {
    let n = default_concurrency();
    assert!((1..=4).contains(&n), "{n}");
}

/// The integration test the channel doc names: three replayed solver runs,
/// two of which touch one file. They post their intents, the overlap is
/// detected once and names both, an order is agreed, and the record holds
/// it.
#[tokio::test]
async fn three_replayed_solvers_post_intents_detect_the_overlap_and_agree_an_order() {
    let recording =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/solver-run-11338.jsonl");
    let dir = tempfile::tempdir().unwrap();
    let channel = Channel::open(dir.path(), "session-a").unwrap();
    let request = arguments(&json!({"issues": [101, 102, 103]})).unwrap();
    let mut rows: Vec<Row> = request
        .issues
        .iter()
        .map(|(_, issue)| Row {
            member: Member::channel_name(*issue),
            issue: *issue,
            stage: Stage::Queued,
            outcome: None,
            pull_request: None,
            cost_usd: None,
            folder: None,
        })
        .collect();

    // Each replayed run is the same recorded one; the solver's mapper turns
    // it into the summary a real run would write, so the row carries the
    // outcome, pull request and cost, and the intent comes from the change.
    for (index, issue) in request.issues.iter().enumerate() {
        let member = Member::channel_name(issue.1);
        channel.join(&member).unwrap();
        let mut options = options(&request, issue, &std::env::temp_dir()).unwrap();
        options.inner.replay = Some(recording.clone());
        options.inner.speed = 50.0;
        let feed = crate::solver::start(options);
        let mut mapper = crate::solver_plugin::Mapper::default();
        loop {
            let (events, ended) = feed.poll();
            for event in events {
                mapper.apply(event, &mut |_| {});
            }
            if ended {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let summary = mapper.result().unwrap();
        // The recorded run changed `docs/verse/zones.md`, so the member's
        // intent touches that file.
        channel
            .intent(Intent::new(
                &member,
                vec!["docs/verse/zones.md".into()],
                vec![],
            ))
            .unwrap();
        record(&summary, &mut rows[index], Some(&channel));
        channel.finish(&member).unwrap();
    }

    // Every solver passed and named its pull request and cost.
    assert!(
        rows.iter()
            .all(|row| row.outcome.as_deref() == Some("passed"))
    );
    assert!(rows.iter().all(|row| row.cost_usd.is_some()));

    // Three intents on one file: each new one overlaps the earlier ones.
    let notices = channel.overlaps().unwrap();
    assert!(notices.len() >= 2, "each later intent overlaps the earlier");
    let members: Vec<String> = notices
        .iter()
        .flat_map(|notice| [notice.body["a"].as_str(), notice.body["b"].as_str()])
        .flatten()
        .map(str::to_owned)
        .collect();
    assert!(members.contains(&"solver-101".to_string()));
    assert!(members.contains(&"solver-102".to_string()));

    // The swarm settles the first overlap by the fixed rule and records it.
    let agreement = channel.settle_first("solver-101", "solver-102").unwrap();
    assert_eq!(agreement.outcome, crate::comms::Outcome::First);
    assert_eq!(
        agreement.detail, "solver-101",
        "the earlier intent goes first"
    );
    let recorded = channel.agreements().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].decided_by, "swarm");

    // The summary names the merge order and the overlaps settled.
    let summary = summary(&request, &rows, &recorded);
    assert_eq!(summary["merge_order"], json!(["solver-101"]));
    assert_eq!(summary["overlaps_settled"], 1);
}
