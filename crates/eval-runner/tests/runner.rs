//! The hosted runner end to end over an in-memory relay: a signed request
//! admitted, run with the fake agent through a fake door, streamed,
//! sealed to the trainer, and published on the trainer's publish request
//! with the trainer's signed request inline; refusals for an unsigned or
//! rebound request, a tool outside the catalog, a draft that asks for
//! `exec`, an oversize request, and a trainer over an operator's emergency
//! brake (a check is never counted); no daily count by default, so a
//! trainer's fourth run of a day runs; every job in the usage log; a
//! cancel that stops a live run; and no door key in any event or file.

mod support;

use std::path::Path;
use std::time::Duration;

use eval_runner::config::Limits;
use nostr::cj_conversation::{SubjectSource, SuiteSource};
use nostr::eval_ext::{self, EventPointer, hosted};
use nostr::execution;
use serde_json::{Value, json};
use support::{FIXTURE, Phone, door, files_holding, memory_runner, result_of};

async fn until<T>(within: Duration, mut find: impl FnMut() -> Option<T>) -> Option<T> {
    let started = std::time::Instant::now();
    while started.elapsed() < within {
        if let Some(found) = find() {
            return Some(found);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

fn pointer(value: &Value) -> EventPointer {
    EventPointer {
        id: value["id"].as_str().unwrap().into(),
        pubkey: value["pubkey"].as_str().unwrap().into(),
        kind: u16::try_from(value["kind"].as_u64().unwrap()).unwrap(),
    }
}

/// The shipped bounds (`None`: no daily count) or an emergency brake.
fn limits(runs_per_trainer: Option<u32>) -> Limits {
    Limits {
        runs_per_trainer,
        turns_per_day: runs_per_trainer.map(|_| 500),
        jobs: 2,
        concurrency: 2,
    }
}

fn draft(tool_uses: &str, prompt_run: &str, cases: usize) -> Value {
    let cases: Vec<Value> = (0..cases)
        .map(|n| {
            json!({
                "id": format!("overview-{n}"),
                "kind": "should-fire",
                "prompt": format!("+++\nv = \"openagents.eval-case.v1\"\n{prompt_run}+++\n\nGive me an overview of the repository.\n"),
                "graders": [{"name": "said", "text": "+++\ntype = \"regex\"\ntarget = \"last_message\"\n+++\n\nRust\n"}],
            })
        })
        .collect();
    json!({
        "v": nostr::cj_conversation::DRAFT_SCHEMA,
        "tool": {"name": "Map brief", "summary": "Maps, then briefs.", "catalog": null,
                 "skill": "When asked about a repository, name its main language.", "uses": [tool_uses]},
        "cases": cases,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hosted_run_is_streamed_sealed_and_published_only_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let door = door();
    let (runner, memory, _) = memory_runner(dir.path(), &door, limits(None));
    let runner_key = runner.pubkey().to_string();
    let tool = runner.catalog().tools[0].clone();
    let release = runner
        .release_extension_suite(Path::new(FIXTURE), "repo-map-brief-tests")
        .await
        .unwrap();
    let events = || memory.events.lock().unwrap().clone();

    let phone = Phone::new();
    let input = hosted::run_input(
        &SuiteSource::Published(pointer(&release)),
        &SubjectSource::Definition(Box::new(tool.definition.clone())),
        None,
        2,
        None,
    )
    .unwrap();
    let (request, body) = phone.request(&runner_key, &input);
    runner.handle(request.clone()).await;
    let result = until(Duration::from_secs(180), || {
        result_of(&phone.answers(&runner_key, &request, &body, &events()))
    })
    .await
    .expect("the run finished");
    assert_eq!(result["outcome"], "completed", "{result:#}");
    let answers = phone.answers(&runner_key, &request, &body, &events());
    assert!(
        answers
            .iter()
            .any(|(kind, p)| *kind == execution::FEEDBACK_KIND && p["type"] == "accepted")
    );
    let progress: Vec<hosted::Progress> = answers
        .iter()
        .filter(|(kind, p)| *kind == execution::FEEDBACK_KIND && p["type"] == "progress")
        .map(|(_, p)| hosted::parse_progress(p).unwrap())
        .collect();
    assert!(progress.iter().all(|p| p.planned == 8), "{progress:?}");
    assert_eq!(progress.last().map(|p| p.completed), Some(8));
    let output = hosted::parse_run_output(&result["output"]).unwrap();
    assert_eq!(output.headline.total, 2);

    // The report is sealed to the trainer, and nothing is public yet.
    let sealed = events()
        .into_iter()
        .find(|e| e.id == output.sealed.id)
        .expect("the sealed report");
    let opened = nostr::private_artifact::open(&sealed, &phone.secret).unwrap();
    assert_eq!(opened.artifact().digest, output.report.digest);
    assert!(events().iter().all(|e| e.kind != nostr::kb::EVIDENCE_KIND));

    // A retransmission of the same request gets the same answer, no rerun.
    let (again, _) = phone.seal(&runner_key, &body);
    runner.handle(again.clone()).await;
    let repeated = until(Duration::from_secs(10), || {
        result_of(&phone.answers(&runner_key, &again, &body, &events()))
    })
    .await
    .expect("the recorded result");
    assert_eq!(repeated["output"], result["output"]);

    // Someone else can't publish this trainer's report.
    let stranger = Phone::new();
    let (theirs, their_body) =
        stranger.request(&runner_key, &hosted::publish_input(&output.report));
    runner.handle(theirs.clone()).await;
    let refused = until(Duration::from_secs(10), || {
        result_of(&stranger.answers(&runner_key, &theirs, &their_body, &events()))
    })
    .await
    .unwrap();
    assert_eq!(refused["code"], "not_admitted");

    // The trainer's publish request publishes the 3189, naming them.
    let (publish, publish_body) =
        phone.request(&runner_key, &hosted::publish_input(&output.report));
    runner.handle(publish.clone()).await;
    let published = until(Duration::from_secs(30), || {
        result_of(&phone.answers(&runner_key, &publish, &publish_body, &events()))
    })
    .await
    .unwrap();
    assert_eq!(published["outcome"], "completed", "{published:#}");
    let out = hosted::parse_publish_output(&published["output"]).unwrap();
    assert_eq!(out.suite_release.id, release["id"].as_str().unwrap());
    let result_event = events()
        .into_iter()
        .find(|e| e.id == out.result.id)
        .expect("the 3189");
    let publication = eval_ext::parse_publication(&result_event).unwrap();
    assert_eq!(publication.evaluator, runner_key);
    assert_eq!(publication.trainer(), phone.pubkey());
    assert_eq!(
        publication.request.as_ref().map(|r| r.id.as_str()),
        Some(request.id.as_str())
    );
    assert_eq!(
        eval_ext::verified_trainer(&publication, &[]).unwrap(),
        phone.pubkey()
    );
    assert_eq!(publication.report_ref.digest, output.report.digest);

    // A second publish reuses the first.
    let (second, second_body) = phone.request(&runner_key, &hosted::publish_input(&output.report));
    runner.handle(second.clone()).await;
    let reused = until(Duration::from_secs(30), || {
        result_of(&phone.answers(&runner_key, &second, &second_body, &events()))
    })
    .await
    .unwrap();
    assert_eq!(reused["output"], published["output"]);
    assert_eq!(
        events()
            .iter()
            .filter(|e| e.kind == nostr::kb::EVIDENCE_KIND)
            .count(),
        1
    );

    // The usage log has one line per job, retransmissions aside: the run,
    // the stranger's refused publish, and the trainer's two publishes.
    let logged = eval_runner::usage::read(&dir.path().join("state/usage"), None).unwrap();
    assert_eq!(logged.unreadable, 0);
    let jobs: Vec<(&str, &str, Option<&str>)> = logged
        .records
        .iter()
        .map(|r| (r.action.as_str(), r.outcome.as_str(), r.code.as_deref()))
        .collect();
    assert_eq!(
        jobs,
        [
            ("run", "completed", None),
            ("publish", "refused", Some("not_admitted")),
            ("publish", "completed", None),
            ("publish", "completed", None),
        ]
    );
    let run = &logged.records[0];
    assert_eq!(run.key, phone.pubkey());
    assert_eq!(run.request, request.id);
    assert_eq!(run.subject.as_deref(), Some(tool.definition.id.as_str()));
    assert_eq!(run.suite.as_deref(), release["id"].as_str());
    assert_eq!(
        (run.tests, run.runs, run.turns),
        (Some(2), Some(2), Some(8))
    );
    assert_eq!(run.subject_passed, Some(output.headline.subject_passed));
    assert!(run.verdict.is_some() && run.bytes_in > 0);
    assert_eq!(
        logged.records[2].result.as_deref(),
        Some(out.result.id.as_str())
    );

    // Every run had read and sandbox write, and nothing more.
    let runs: Vec<Value> = support::files_named(&dir.path().join("state/jobs"), "run.json")
        .iter()
        .map(|path| serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap())
        .collect();
    assert!(!runs.is_empty());
    for run in &runs {
        assert_eq!(run["grants"], json!(["read", "write"]), "{run:#}");
    }

    // No door key in any event, decrypted answer, or state file.
    for event in events() {
        assert!(!event.content.contains(&door.key));
    }
    for (_, payload) in phone.answers(&runner_key, &request, &body, &events()) {
        assert!(!payload.to_string().contains(&door.key));
    }
    assert_eq!(
        files_holding(dir.path(), &door.key),
        Vec::<std::path::PathBuf>::new()
    );
}

/// No usage limit (owner decision, 2026-10-01, #10121): with the shipped
/// bounds one trainer's fourth run of a UTC day runs like the first, and
/// each is a line in the usage log.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_trainers_fourth_run_of_the_day_runs() {
    let dir = tempfile::tempdir().unwrap();
    let door = door();
    let (runner, memory, _) = memory_runner(dir.path(), &door, Limits::default());
    let runner_key = runner.pubkey().to_string();
    let tool = runner.catalog().tools[0].clone();
    let release = runner
        .release_extension_suite(Path::new(FIXTURE), "repo-map-brief-tests")
        .await
        .unwrap();
    let phone = Phone::new();
    let input = hosted::run_input(
        &SuiteSource::Published(pointer(&release)),
        &SubjectSource::Definition(Box::new(tool.definition.clone())),
        None,
        1,
        None,
    )
    .unwrap();
    for _ in 0..4 {
        let (request, body) = phone.request(&runner_key, &input);
        runner.handle(request.clone()).await;
        let done = until(Duration::from_secs(120), || {
            let events = memory.events.lock().unwrap().clone();
            result_of(&phone.answers(&runner_key, &request, &body, &events))
        })
        .await
        .expect("the run finished");
        assert_eq!(done["outcome"], "completed", "{done:#}");
    }
    let logged = eval_runner::usage::read(&dir.path().join("state/usage"), None).unwrap();
    let rows = eval_runner::usage::stats(&logged.records, eval_runner::usage::By::Key);
    assert_eq!(rows[0].group, phone.pubkey());
    assert_eq!(
        (rows[0].jobs, rows[0].completed, rows[0].refused),
        (4, 4, 0)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bad_requests_are_dropped_or_refused_and_the_quota_holds() {
    let dir = tempfile::tempdir().unwrap();
    let door = door();
    let (runner, memory, _) = memory_runner(dir.path(), &door, limits(Some(1)));
    let runner_key = runner.pubkey().to_string();
    let tool = runner.catalog().tools[0].clone();
    let release = runner
        .release_extension_suite(Path::new(FIXTURE), "repo-map-brief-tests")
        .await
        .unwrap();
    let events = || memory.events.lock().unwrap().clone();
    let published_input = |runs: u64, check: Option<&str>| {
        hosted::run_input(
            &SuiteSource::Published(pointer(&release)),
            &SubjectSource::Definition(Box::new(tool.definition.clone())),
            None,
            runs,
            check,
        )
        .unwrap()
    };
    let phone = Phone::new();
    let ask = |input: &Value| phone.request(&runner_key, input);
    let refusal = |request: &nostr::domain::Event, body: &Value| {
        let events = memory.events.lock().unwrap().clone();
        result_of(&phone.answers(&runner_key, request, body, &events))
    };
    let answered_before = events().len();

    // Unsigned: a broken signature is dropped, with no answer.
    let (mut unsigned, _) = ask(&published_input(1, None));
    unsigned.sig = "00".repeat(64);
    runner.handle(unsigned).await;
    // Rebound: a request sent to another worker isn't this runner's.
    let other = support::secret();
    let other_key = other
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string();
    let (rebound, _) = phone.request(&other_key, &published_input(1, None));
    runner.handle(rebound).await;
    // Tampered: a request whose tags were changed after signing.
    let (mut tampered, _) = phone.request(&other_key, &published_input(1, None));
    tampered.tags = vec![nostr::domain::Tag::new(vec![
        "p".into(),
        runner_key.clone(),
    ])];
    runner.handle(tampered).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        events().len(),
        answered_before,
        "nothing answers an unproven sender"
    );

    // A tool outside the catalog.
    let mut stranger = tool.definition.clone();
    stranger.artifact.digest = format!("sha256:{}", "ab".repeat(32));
    let input = hosted::run_input(
        &SuiteSource::Published(pointer(&release)),
        &SubjectSource::Definition(Box::new(stranger)),
        None,
        1,
        None,
    )
    .unwrap();
    let (request, body) = ask(&input);
    runner.handle(request.clone()).await;
    assert_eq!(refusal(&request, &body).unwrap()["code"], "not_admitted");

    // A draft that asks to run commands.
    let exec = draft(
        &tool.aliases[0].id,
        "[run]\nallowed_operations = [\"read\", \"exec\"]\n",
        1,
    );
    let input = hosted::run_input(
        &SuiteSource::Draft,
        &SubjectSource::Draft,
        Some(&exec),
        1,
        None,
    )
    .unwrap();
    let (request, body) = ask(&input);
    runner.handle(request.clone()).await;
    assert_eq!(refusal(&request, &body).unwrap()["code"], "not_admitted");

    // Oversize: four runs per arm, and nine tests.
    let mut four = published_input(1, None);
    four["runs"] = json!(4);
    let body = hosted::request_body(
        &runner_key,
        "oversize-runs",
        &published_input(1, None),
        eval_runner::unix_now(),
    )
    .unwrap();
    let mut body = body;
    body["input"] = four;
    let (request, body) = phone.seal(&runner_key, &body);
    runner.handle(request.clone()).await;
    assert_eq!(refusal(&request, &body).unwrap()["code"], "too_large");
    let nine = draft(&tool.aliases[0].id, "", 9);
    let mut body = hosted::request_body(
        &runner_key,
        "oversize-cases",
        &published_input(1, None),
        eval_runner::unix_now(),
    )
    .unwrap();
    body["input"] = json!({"v": hosted::SCHEMA, "action": "run", "suite": "draft", "subject": "draft",
                            "draft": nine, "runs": 1, "baseline": true, "check": null});
    let (request, body) = phone.seal(&runner_key, &body);
    runner.handle(request.clone()).await;
    assert_eq!(refusal(&request, &body).unwrap()["code"], "too_large");

    // An operator's emergency brake: one run a day here. The first runs;
    // the second is refused, and the refusal names no count.
    let (first, first_body) = ask(&published_input(1, None));
    runner.handle(first.clone()).await;
    let done = until(Duration::from_secs(120), || refusal(&first, &first_body))
        .await
        .unwrap();
    assert_eq!(done["outcome"], "completed", "{done:#}");
    let (second, second_body) = ask(&published_input(1, None));
    runner.handle(second.clone()).await;
    let braked = refusal(&second, &second_body).unwrap();
    assert_eq!(braked["code"], "over_quota");
    assert_eq!(
        braked["error"]["message"]
            .as_str()
            .or(braked["message"].as_str()),
        Some(eval_runner::quota::BRAKED),
        "{braked:#}"
    );

    // Publish the first, then another trainer's check of it runs even
    // though that trainer has spent the day's runs: checks don't count.
    let report = hosted::parse_run_output(&done["output"]).unwrap().report;
    let (publish, publish_body) = ask(&hosted::publish_input(&report));
    runner.handle(publish.clone()).await;
    let published = until(Duration::from_secs(30), || refusal(&publish, &publish_body))
        .await
        .unwrap();
    let original = hosted::parse_publish_output(&published["output"])
        .unwrap()
        .result;
    let checker = Phone::new();
    let spend = |input: &Value| checker.request(&runner_key, input);
    let (own, own_body) = spend(&published_input(1, None));
    runner.handle(own.clone()).await;
    let own_done = until(Duration::from_secs(120), || {
        result_of(&checker.answers(&runner_key, &own, &own_body, &events()))
    })
    .await
    .unwrap();
    assert_eq!(own_done["outcome"], "completed");
    let (check, check_body) = spend(&published_input(1, Some(&original.id)));
    runner.handle(check.clone()).await;
    let checked = until(Duration::from_secs(120), || {
        result_of(&checker.answers(&runner_key, &check, &check_body, &events()))
    })
    .await
    .unwrap();
    assert_eq!(checked["outcome"], "completed", "{checked:#}");
    let check_report = hosted::parse_run_output(&checked["output"]).unwrap().report;
    let (publish_check, publish_check_body) =
        checker.request(&runner_key, &hosted::publish_input(&check_report));
    runner.handle(publish_check.clone()).await;
    let check_published = until(Duration::from_secs(30), || {
        result_of(&checker.answers(&runner_key, &publish_check, &publish_check_body, &events()))
    })
    .await
    .unwrap();
    let check_event_id = hosted::parse_publish_output(&check_published["output"])
        .unwrap()
        .result
        .id;
    let all = events();
    let original_event = all.iter().find(|e| e.id == original.id).unwrap();
    let check_event = all.iter().find(|e| e.id == check_event_id).unwrap();
    // Two hosted runs by one runner are a check when different trainers
    // asked; the verdicts decide confirm or dispute.
    assert_ne!(
        eval_ext::confirms(original_event, check_event).unwrap(),
        eval_ext::Linkage::NotACheck
    );
    // A check of a result that isn't on the relay is refused.
    let (bogus, bogus_body) = spend(&published_input(1, Some(&"cd".repeat(32))));
    runner.handle(bogus.clone()).await;
    assert_eq!(
        result_of(&checker.answers(&runner_key, &bogus, &bogus_body, &events())).unwrap()["code"],
        "not_admitted"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cancel_stops_a_live_run_and_an_admission_switch_closes_the_door() {
    let dir = tempfile::tempdir().unwrap();
    let door = door();
    let (runner, memory, _) = memory_runner(dir.path(), &door, limits(None));
    let runner_key = runner.pubkey().to_string();
    let tool = runner.catalog().tools[0].clone();
    let events = || memory.events.lock().unwrap().clone();
    let phone = Phone::new();
    let slow = draft(
        &tool.aliases[0].id,
        "runs = 1\n[run]\nenv = { OA_EVAL_FAKE = \"sleep\" }\n",
        1,
    );
    let input = hosted::run_input(
        &SuiteSource::Draft,
        &SubjectSource::Draft,
        Some(&slow),
        1,
        None,
    )
    .unwrap();
    let (request, body) = phone.request(&runner_key, &input);
    runner.handle(request.clone()).await;
    until(Duration::from_secs(30), || {
        phone
            .answers(&runner_key, &request, &body, &events())
            .into_iter()
            .find(|(_, p)| p["type"] == "progress" && p["status"] == "running")
    })
    .await
    .expect("the run started");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let cancel = phone.cancel(&runner_key, &request, &body);
    runner.handle(cancel).await;
    let result = until(Duration::from_secs(60), || {
        result_of(&phone.answers(&runner_key, &request, &body, &events()))
    })
    .await
    .expect("the run stopped");
    assert_eq!(result["outcome"], "cancelled", "{result:#}");

    // With the switch closed, a new run is refused before anything runs.
    std::fs::write(dir.path().join("state/closed"), "").unwrap();
    let (closed, closed_body) = phone.request(&runner_key, &input);
    runner.handle(closed.clone()).await;
    let refused = until(Duration::from_secs(10), || {
        result_of(&phone.answers(&runner_key, &closed, &closed_body, &events()))
    })
    .await
    .unwrap();
    assert_eq!(refused["code"], "not_admitted");
}

/// A run the runner was stopped during has an unknown outcome: after a
/// restart, a retransmission of its request is answered `unknown`, and it
/// never runs again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_run_interrupted_by_a_restart_answers_unknown_and_never_reruns() {
    let dir = tempfile::tempdir().unwrap();
    let door = door();
    let (runner, memory, key) = memory_runner(dir.path(), &door, limits(None));
    let runner_key = runner.pubkey().to_string();
    let tool = runner.catalog().tools[0].clone();
    let release = runner
        .release_extension_suite(Path::new(FIXTURE), "repo-map-brief-tests")
        .await
        .unwrap();
    drop(runner);
    let phone = Phone::new();
    let input = hosted::run_input(
        &SuiteSource::Published(pointer(&release)),
        &SubjectSource::Definition(Box::new(tool.definition.clone())),
        None,
        1,
        None,
    )
    .unwrap();
    let (request, body) = phone.request(&runner_key, &input);
    // The ledger as the stopped runner left it: claimed, accepted, and
    // dispatched, with no outcome.
    let store = eval_runner::store::Store::open(&dir.path().join("state")).unwrap();
    let mut service = store.service(&runner_key, 16, u64::MAX);
    let opened = execution::open_request(
        &request,
        &runner_key,
        &key,
        eval_runner::unix_now(),
        execution::Window::DEFAULT,
    )
    .unwrap();
    let execution::Admission::Reserved { claim, root } = service
        .prepare(&opened, eval_runner::unix_now(), &"ab".repeat(32))
        .unwrap()
    else {
        panic!("a new claim")
    };
    service.acknowledge(&claim.key, &root).unwrap();
    service.intend(&claim.key).unwrap();
    store.save_service(&service).unwrap();

    let identity = coder::relay::Identity::from_text(&support::hex(&key), "key").unwrap();
    let restarted = eval_runner::runner::Runner::new(
        support::config(dir.path(), &door, limits(None)),
        identity,
        memory.clone(),
        memory.clone(),
    )
    .unwrap();
    let (again, _) = phone.seal(&runner_key, &body);
    restarted.handle(again.clone()).await;
    let events = || memory.events.lock().unwrap().clone();
    let answer = until(Duration::from_secs(10), || {
        result_of(&phone.answers(&runner_key, &again, &body, &events()))
    })
    .await
    .expect("the recorded outcome");
    assert_eq!(answer["outcome"], "unknown", "{answer:#}");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        support::files_named(&dir.path().join("state/jobs"), "run.json").is_empty(),
        "nothing ran again"
    );
}
