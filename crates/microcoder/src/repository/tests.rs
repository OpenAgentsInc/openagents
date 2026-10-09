use super::*;
use crate::models::{Basis, NextAction};
use coder::task::adapter::Route as GrantRoute;
use coder::task::adapter::{CONFIG_SCHEMA, Configuration, NAME};
use coder::task::capacity::{self, Provider, Refusal};
use coder::task::{Action, Command, RequestedConfiguration, Store, TaskIntent, Workspace};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

/// The canonical system shell the owner admits: `/bin/bash`, or `/bin/sh`
/// where there is no `/bin/bash`, as on NixOS.
fn system_shell() -> std::path::PathBuf {
    ["/bin/bash", "/bin/sh"]
        .iter()
        .find_map(|path| Path::new(path).canonicalize().ok())
        .expect("a system shell")
}

pub(super) fn fixture() -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>) {
    fixture_with("fixture-model", |_| {})
}

/// A fixture task that requests `model`, with its grant's configuration
/// changed by `change`.
pub(super) fn fixture_with(
    model: &str,
    change: impl FnOnce(&mut Configuration),
) -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>) {
    fixture_images(model, change, &[], "Write result.txt containing output.")
}

/// [`fixture_with`], with `images` attached to the task as a device's
/// upload binds them: kept in the store's task media, named by the intent.
pub(super) fn fixture_images(
    model: &str,
    change: impl FnOnce(&mut Configuration),
    images: &[coder::task::media::wire::Upload],
    prompt: &str,
) -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>) {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let checkout = root.path().join("checkout");
    std::fs::create_dir(&repo).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "Fixture",
        ],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
    }
    assert!(
        std::process::Command::new("git")
            .args(["worktree", "add", "--detach", "-q"])
            .arg(&checkout)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    let store = root.path().join("tasks");
    let command = Command {
        schema: task::COMMAND_SCHEMA.into(),
        command_id: "submit-fixture".into(),
        task_id: "fixture".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: TaskIntent {
                title: "Repository fixture".into(),
                prompt: prompt.into(),
                workspace: Workspace {
                    path: checkout.canonicalize().unwrap().display().to_string(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: NAME.into(),
                    model: Some(model.into()),
                },
                images: images.iter().map(|image| image.reference.clone()).collect(),
            },
        },
    };
    let mut inbox = Store::open(&store).unwrap();
    // As a device sends them: chunk by chunk to the host's uploads, then
    // bound to the task its `task.create` names.
    let device = "d".repeat(64);
    for image in images {
        for chunk in image.chunks(0) {
            coder::task::media::put(&store, &device, &chunk).unwrap();
        }
        coder::task::media::adopt(&store, &device, "fixture", &image.reference).unwrap();
    }
    inbox.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
    let task = inbox.show("fixture").unwrap();
    let grant = task::owner::Grant {
        schema: task::owner::GRANT_SCHEMA.into(),
        task_id: task.task_id,
        intent_digest: task.intent_digest,
        expected_revision: 1,
        expected_source_snapshot: None,
        program: system_shell(),
        arguments: Vec::new(),
        write_workspace: true,
        wall_seconds: 8,
        stream_bytes: 4096,
        memory_bytes: 256 * 1024 * 1024,
        requirements: None,
        adapter_configuration: Some(Configuration {
            schema: CONFIG_SCHEMA.into(),
            provider: "synthetic".into(),
            model: "fixture-model".into(),
            effort: Some("medium".into()),
            generation_endpoint: "in-process".into(),
            decision_endpoint: "in-process".into(),
            decision_model: "fixture-judge".into(),
            max_steps: Some(4),
            acceptance: false,
            route: "never".into(),
            knowledge: "off".into(),
            dollar_limit_micros: None,
            expected_controller_digest: None,
            container: None,
            fallbacks: Vec::new(),
            access: coder::task::adapter::Access::Boundary,
            studio_seat: None,
        }),
    };
    let mut grant = grant;
    if let Some(configuration) = grant.adapter_configuration.as_mut() {
        change(configuration);
    }
    (root, store, serde_json::to_vec(&grant).unwrap())
}

struct Generator {
    actions: RefCell<VecDeque<NextAction>>,
    model: &'static str,
    calls: Cell<usize>,
}
fn generator(command: &str) -> Generator {
    Generator {
        actions: RefCell::new(VecDeque::from([
            NextAction {
                rationale: "Implement the requested output.".into(),
                commands: vec![command.into()],
                view: vec!["result.txt".into()],
                freeze_tests: false,
                expand: Vec::new(),
                finished: false,
                reply: String::new(),
                ask: crate::models::Ask::None,
            },
            NextAction {
                rationale: "The requested file exists.".into(),
                commands: Vec::new(),
                view: Vec::new(),
                freeze_tests: false,
                expand: Vec::new(),
                finished: true,
                reply: String::new(),
                ask: crate::models::Ask::None,
            },
        ])),
        model: "fixture-model",
        calls: Cell::new(0),
    }
}
impl Generate for Generator {
    async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
        self.calls.set(self.calls.get() + 1);
        Generated {
            action: self
                .actions
                .borrow_mut()
                .pop_front()
                .ok_or("no more fixture actions".into()),
            model: self.model.into(),
            prompt_tokens: 10,
            completion_tokens: 5,
            usd: Some(0.0),
            known_usd: 0.0,
            cost_unknown: None,
            usd_upper: Some(0.0),
            cost_basis: Basis::ListPrice,
            milliseconds: 1,
        }
    }
}
struct JudgeFixture;
impl Judge for JudgeFixture {
    async fn judge(&self, _set: &QuestionSet, _state: &Value) -> Judgment {
        Judgment::free()
    }
}

#[tokio::test]
async fn existing_loop_uses_common_owner_boundary_atif_and_retained_artifacts() {
    let (root, store, grant) = fixture();
    let generator = generator("printf output > result.txt; printf 'full command output'");
    let host = Host::admit(&store, &grant).await.unwrap();
    let result = run(host, &generator, &JudgeFixture).await.unwrap();
    assert_eq!(result.execution, task::Execution::Finished);
    assert_eq!(result.checks, task::Checks::NotRun);
    assert_eq!(generator.calls.get(), 2);
    assert_eq!(
        task::artifact::read(&store, "fixture", Path::new("result.txt")).unwrap(),
        b"output"
    );
    let view = task::view::read(&store, "fixture", None, 200).unwrap();
    assert_eq!(view.evidence.state, "sealed");
    // The fixture's calls are priced: the run records its cost (#10161).
    assert_eq!(view.cost_status, "priced");
    let result = view.task.run.as_ref().unwrap().result.clone().unwrap();
    assert_eq!(
        view.cost_usd,
        result.cost_microusd.map(|micro| micro as f64 / 1_000_000.0)
    );
    assert!(result.engine_microusd.is_some() && result.jev_microusd.is_some());
    let text = serde_json::to_string(&view).unwrap();
    assert!(
        text.contains("full command output")
            && text.contains("generation")
            && text.contains("decision")
    );
    assert!(text.contains("fixture-model") && text.contains("medium"));
    assert_eq!(
        std::fs::read(root.path().join("checkout/result.txt")).unwrap(),
        b"output"
    );
    if let Some(destination) = std::env::var_os("MICROCODER_REPOSITORY_ACCEPTANCE_DIR") {
        let destination = std::path::PathBuf::from(destination);
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("grant.json"), &grant).unwrap();
        std::fs::write(
            destination.join("task.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        std::fs::write(
            destination.join("view.json"),
            serde_json::to_vec_pretty(&view).unwrap(),
        )
        .unwrap();
        std::fs::copy(
            store.join("fixture.1.atif.jsonl"),
            destination.join("trace.atif.jsonl"),
        )
        .unwrap();
        std::fs::write(
            destination.join("result.txt"),
            task::artifact::read(&store, "fixture", Path::new("result.txt")).unwrap(),
        )
        .unwrap();
    }
    assert!(Host::admit(&store, &grant).await.is_err());
}

/// #10237: a disk that fills while a run keeps its evidence no longer ends
/// the engine before it records a result, which left the run to be ended
/// as "owner process ended". A full disk that clears is waited out; one
/// that stays full ends the run as `disk_full`, its result recorded.
#[tokio::test]
async fn a_full_disk_is_waited_out_or_recorded_never_left_to_the_settler() {
    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &grant).await.unwrap();
    host.fill_disk(3, Duration::from_secs(60));
    host.append(&atif::Step::said(
        atif::Source::System,
        "kept after the wait",
    ))
    .unwrap();
    host.fill_disk(usize::MAX, Duration::ZERO);
    assert!(
        host.append(&atif::Step::said(atif::Source::System, "lost"))
            .is_err()
    );
    let task = host.finish("model_finished", true, json!({})).unwrap();
    let run = task.run.as_ref().unwrap();
    let result = run.result.as_ref().unwrap();
    assert_eq!(result.ending, coder::task::adapter::DISK_FULL);
    assert_ne!(result.ending, coder::task::owner::OWNER_ENDED);
    assert_eq!(result.exit_code, Some(1));
    assert!(result.output_incomplete && result.group_clear);
    assert_eq!(task.status, task::Status::Finished);
    assert_eq!(task.execution, task::Execution::Failed);
    // Nothing is left for the settler to end.
    let mut opened = Store::open(&store).unwrap();
    assert!(opened.settle("fixture").unwrap().is_none());
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("kept after the wait") && !trace.contains("\"lost\""));
}

/// #10993: a long run's transcript reaching its size bound once ended the
/// run as a fault, and the fault read as "Stopped, as you asked". Past the
/// bound, bulky records are left out and the run goes on to finish.
#[tokio::test]
async fn a_full_transcript_leaves_records_out_and_the_run_goes_on() {
    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &grant).await.unwrap();
    host.fill_trace();
    let bulky = "b".repeat(200 * 1024);
    host.append(&atif::Step::said(atif::Source::System, &bulky))
        .unwrap();
    host.append(&atif::Step::said(atif::Source::System, "small, still kept"))
        .unwrap();
    assert!(!host.cancelled() && host.fault().is_none() && !host.stop_asked());
    let task = host.finish("model_finished", true, json!({})).unwrap();
    let result = task.run.as_ref().unwrap().result.clone().unwrap();
    assert_eq!(result.ending, "model_finished");
    assert!(!result.stop_requested && result.output_incomplete);
    assert_eq!(task.execution, task::Execution::Finished);
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("evidence_capped") && trace.contains("small, still kept"));
    assert!(!trace.contains(&bulky));
}

/// #10993: a fault of the host's own ends the turn as `host_fault`, a
/// failure that names the fault, never as a stop the person asked for.
#[tokio::test]
async fn a_host_fault_is_a_named_failure_not_a_stop_the_person_asked_for() {
    use openagents_chat::coder_events::{self, CoderEvent, Mapper};
    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &grant).await.unwrap();
    host.fail("task evidence could not be retained: fixture fault");
    assert!(host.cancelled() && !host.stop_asked());
    let task = host
        .finish("cancelled_or_host_refusal", false, json!({}))
        .unwrap();
    let run = task.run.as_ref().unwrap();
    let result = run.result.as_ref().unwrap();
    assert_eq!(result.ending, coder::task::adapter::HOST_FAULT);
    assert!(!result.stop_requested);
    assert_eq!(task.execution, task::Execution::Failed);
    let steps = atif::log::read(&store.join(&run.admission.trace_file))
        .unwrap()
        .document()["steps"]
        .as_array()
        .cloned()
        .unwrap();
    let mut mapper = Mapper::new(1, None);
    for step in &steps {
        mapper.step(step);
    }
    match mapper.end(&result.ending, Vec::new(), "", "", None) {
        CoderEvent::Failure(failed) => {
            assert!(
                failed.message.contains("fixture fault"),
                "{}",
                failed.message
            );
            assert!(failed.message.contains("Nobody stopped the task"));
            assert_ne!(failed.message, coder_events::STOPPED_AS_ASKED);
        }
        other => panic!("expected a failure, got {}", other.name()),
    }
}

/// Why a run has no Jev, as a step's judgment says it.
const NO_JEV: &str = "Jev is unreachable: the fixture has no service";

/// What a step's judgment is on a run with no Jev.
struct NoKeyJudge;
impl Judge for NoKeyJudge {
    async fn judge(&self, _set: &QuestionSet, _state: &Value) -> Judgment {
        Judgment {
            error: Some(NO_JEV.into()),
            usd: Some(0.0),
            usd_upper: Some(0.0),
            ..Judgment::default()
        }
    }
}

#[tokio::test]
async fn a_run_without_a_jev_key_still_runs_its_commands_and_finishes() {
    let (root, store, grant) = fixture();
    let generator = generator("printf output > result.txt");
    let host = Host::admit(&store, &grant).await.unwrap();
    let result = run(host, &generator, &NoKeyJudge).await.unwrap();
    assert_eq!(result.execution, task::Execution::Finished);
    assert_eq!(generator.calls.get(), 2);
    assert_eq!(
        std::fs::read(root.path().join("checkout/result.txt")).unwrap(),
        b"output"
    );
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains(NO_JEV));
}

#[tokio::test]
async fn unsafe_reads_and_outside_writes_do_not_escape_the_repository() {
    use std::os::unix::fs::symlink;
    let (root, store, grant) = fixture();
    // Outside the fixture's temporary directory: on Linux the boundary gives
    // commands a private /tmp, where the write would succeed unseen.
    let elsewhere = tempfile::tempdir_in("/var/tmp").unwrap();
    let outside = elsewhere.path().join("outside");
    std::fs::write(&outside, "outside marker").unwrap();
    symlink(&outside, root.path().join("checkout/link")).unwrap();
    let host = Host::admit(&store, &grant).await.unwrap();
    assert!(host.read("../outside", 1024).is_err());
    assert!(host.read("link", 1024).is_err());
    let script = format!("printf changed > '{}'", outside.display());
    let observation = host.command(&script, Duration::from_secs(2)).await.unwrap();
    assert_ne!(observation.exit, Some(0));
    assert_eq!(std::fs::read(&outside).unwrap(), b"outside marker");
    host.finish("fixture_complete", false, json!({})).unwrap();
}

async fn cancel_after_dispatch(store: &Path) {
    let started = std::time::Instant::now();
    loop {
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap_or_default();
        if trace.contains("command started") {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(4));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut inbox = Store::open(store).unwrap();
    let task = inbox.show("fixture").unwrap();
    inbox
        .apply(
            &serde_json::to_vec(&Command {
                schema: task::COMMAND_SCHEMA.into(),
                command_id: "cancel-fixture".into(),
                task_id: "fixture".into(),
                expected_revision: Some(task.revision),
                action: Action::Cancel {
                    reason: "fixture cancellation".into(),
                },
            })
            .unwrap(),
        )
        .unwrap();
}

/// A cancelled turn as every surface shows it: the event stream the CLI
/// and apps follow ([`openagents_chat::coder_events`]) and the thread's
/// history records (`coder_history`). Exactly one `stopped` ends it; no
/// failure and no failed model call come from the cancel.
fn assert_one_clean_stop(store: &Path, task: &task::Task) {
    use openagents_chat::coder_events::{CoderEvent, Mapper};
    let trace = store.join(&task.run.as_ref().unwrap().admission.trace_file);
    let raw = std::fs::read_to_string(&trace).unwrap();
    assert!(!raw.contains("cannot make this transition"), "{raw}");
    let steps = atif::log::read(&trace).unwrap().document()["steps"]
        .as_array()
        .cloned()
        .unwrap();
    let mut mapper = Mapper::new(1, None);
    let mut events: Vec<CoderEvent> = steps.iter().flat_map(|step| mapper.step(step)).collect();
    let ending = &task.run.as_ref().unwrap().result.as_ref().unwrap().ending;
    assert_eq!(ending, "cancelled_or_host_refusal");
    events.push(mapper.end(ending, Vec::new(), "", "", None));
    let names: Vec<&str> = events.iter().map(CoderEvent::name).collect();
    assert_eq!(
        names.iter().filter(|name| **name == "stopped").count(),
        1,
        "{names:?}"
    );
    assert!(!names.contains(&"failure"), "{names:?}");
    // The history reads the trajectory's log records, a line each.
    let history: Vec<String> = raw
        .lines()
        .filter_map(|line| coder_history::readable_record_full(line.as_bytes()))
        .map(|readable| readable.text)
        .collect();
    assert!(
        history
            .iter()
            .all(|text| !text.contains("The model call failed")),
        "{history:?}"
    );
    let stops: Vec<&String> = history
        .iter()
        .filter(|text| text.starts_with("Coder stopped"))
        .collect();
    assert_eq!(
        stops,
        ["Coder stopped: the task was stopped, or its host refused to go on."],
        "{history:?}"
    );
    // The loop's own record of the end says it was stopped from outside.
    let ended: Vec<&Value> = steps
        .iter()
        .filter_map(|step| step.pointer("/extra/microcoder/event"))
        .filter(|event| event["event"] == "ended")
        .collect();
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0]["outcome"]["ending"]["reason"], "stopped");
    let failed = steps
        .iter()
        .filter_map(|step| step.pointer("/extra/microcoder/event"))
        .filter(|event| event["event"] == "generated")
        .filter(|event| event["generated"]["action"].get("Err").is_some())
        .count();
    assert_eq!(failed, 0, "no failed model call is recorded for the cancel");
}

#[tokio::test]
async fn cancellation_reaps_the_current_command_and_dispatches_no_followup() {
    let (_root, store, grant) = fixture();
    let generator = generator("sleep 30; printf output > result.txt");
    let host = Host::admit(&store, &grant).await.unwrap();
    let (result, ()) = tokio::join!(
        run(host, &generator, &JudgeFixture),
        cancel_after_dispatch(&store)
    );
    let result = result.unwrap();
    assert_eq!(result.execution, task::Execution::Stopped);
    assert_one_clean_stop(&store, &result);
    assert!(result.run.unwrap().result.unwrap().group_clear);
    assert_eq!(generator.calls.get(), 1);
    assert!(task::artifact::read(&store, "fixture", Path::new("result.txt")).is_err());
}

#[tokio::test]
async fn mismatched_model_cannot_dispatch_its_action() {
    let (root, store, grant) = fixture();
    let mut generator = generator("printf wrong > result.txt");
    generator.model = "another-model";
    let result = run(
        Host::admit(&store, &grant).await.unwrap(),
        &generator,
        &JudgeFixture,
    )
    .await
    .unwrap();
    assert_ne!(result.execution, task::Execution::Finished);
    assert!(!root.path().join("checkout/result.txt").exists());
    assert_eq!(generator.calls.get(), 1);
}

#[test]
fn unsupported_configuration_is_refused_without_fallback() {
    let (_root, _store, bytes) = fixture();
    let mut grant = task::owner::Grant::parse(&bytes).unwrap();
    let configuration = grant.adapter_configuration.as_mut().unwrap();
    configuration.acceptance = true;
    assert!(configuration.validate().is_err());
    configuration.acceptance = false;
    configuration.dollar_limit_micros = Some(1_000_000);
    assert!(configuration.validate().is_err());
}

#[test]
fn claude_is_an_admitted_provider_and_other_names_are_not() {
    let (_root, _store, bytes) = fixture();
    let mut grant = task::owner::Grant::parse(&bytes).unwrap();
    let configuration = grant.adapter_configuration.as_mut().unwrap();
    configuration.provider = "claude".into();
    configuration.model = "claude-opus-5-5".into();
    configuration.generation_endpoint = crate::claude::ENDPOINT.into();
    configuration.decision_endpoint = "https://decision.example.invalid".into();
    assert!(configuration.validate().is_ok());
    assert_eq!(
        configuration.capabilities()["cost_reporting"],
        "provider-reported-list-price"
    );
    configuration.provider = "openrouter".into();
    assert!(configuration.validate().is_err());
    configuration.provider = "claude".into();
    configuration.generation_endpoint = "http://api.anthropic.com".into();
    assert!(configuration.validate().is_err());
}

#[tokio::test]
async fn claude_execution_refuses_another_endpoint_before_admission() {
    let (_root, store, bytes) = fixture();
    let mut grant = task::owner::Grant::parse(&bytes).unwrap();
    let configuration = grant.adapter_configuration.as_mut().unwrap();
    configuration.provider = "claude".into();
    configuration.model = "claude-opus-5-5".into();
    configuration.generation_endpoint = "https://other.example.invalid".into();
    configuration.decision_endpoint = "https://decision.example.invalid".into();
    let client = jev::Client::new(
        jev::Config::default()
            .api_key("unused-fixture-key")
            .base_url("https://decision.example.invalid")
            .default_model("fixture-judge"),
    )
    .unwrap();
    let judge = crate::models::JevJudge { client };
    let error = execute(&store, &serde_json::to_vec(&grant).unwrap(), Ok(judge))
        .await
        .unwrap_err();
    assert!(error.message.contains(crate::claude::ENDPOINT), "{error}");
    assert_eq!(error.cause, Some(StartCause::Configuration));
    assert_eq!(error.diagnostic()["cause"], "configuration");
    assert!(std::fs::read_dir(&store).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".atif.jsonl")
    }));
}

struct PendingGenerator {
    started: Cell<bool>,
}
impl Generate for PendingGenerator {
    async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
        self.started.set(true);
        std::future::pending().await
    }
}

#[tokio::test]
async fn interrupted_model_request_keeps_unknown_cost_and_starts_no_command() {
    let (_root, store, grant) = fixture();
    let generator = PendingGenerator {
        started: Cell::new(false),
    };
    let host = Host::admit(&store, &grant).await.unwrap();
    let cancel = async {
        while !generator.started.get() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let mut inbox = Store::open(&store).unwrap();
        let task = inbox.show("fixture").unwrap();
        inbox
            .apply(
                &serde_json::to_vec(&Command {
                    schema: task::COMMAND_SCHEMA.into(),
                    command_id: "cancel-model".into(),
                    task_id: "fixture".into(),
                    expected_revision: Some(task.revision),
                    action: Action::Cancel {
                        reason: "stop pending model".into(),
                    },
                })
                .unwrap(),
            )
            .unwrap();
    };
    let (task, ()) = tokio::join!(run(host, &generator, &JudgeFixture), cancel);
    let task = task.unwrap();
    assert_eq!(task.execution, task::Execution::Stopped);
    assert_one_clean_stop(&store, &task);
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("interrupted model request may still consume tokens"));
    assert!(!trace.contains("Supervised command started"));
    assert!(trace.contains("\"usd\":null"));
}

#[tokio::test]
async fn evidence_cap_leaves_records_out_but_never_stops_the_run() {
    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &grant).await.unwrap();
    // An oversized record is left out; the run goes on (#10993).
    host.append(&Step::said(Source::Agent, &"x".repeat(8 * 1024 * 1024)))
        .unwrap();
    assert!(host.effect("generation", json!({})).is_ok());
    assert!(!host.cancelled());
    let task = host.finish("evidence_limit", false, json!({})).unwrap();
    let result = task.run.unwrap().result.unwrap();
    assert!(result.output_incomplete && !result.stop_requested);
    let view = task::view::read(&store, "fixture", None, 200).unwrap();
    assert_eq!(view.evidence.state, "sealed");
    assert!(
        std::fs::metadata(store.join("fixture.1.atif.jsonl"))
            .unwrap()
            .len()
            < 64 * 1024 * 1024
    );
}

struct MissingUsageGenerator {
    prior_unknown: bool,
}
impl Generate for MissingUsageGenerator {
    async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
        Generated {
            action: Ok(NextAction {
                finished: true,
                reply: String::new(),
                ask: crate::models::Ask::None,
                rationale: "Synthetic completion.".into(),
                commands: Vec::new(),
                view: Vec::new(),
                freeze_tests: false,
                expand: Vec::new(),
            }),
            model: "fixture-model".into(),
            prompt_tokens: 0,
            completion_tokens: 0,
            usd: (!self.prior_unknown).then_some(0.0),
            known_usd: if self.prior_unknown { 0.002 } else { 0.0 },
            cost_unknown: self
                .prior_unknown
                .then(|| "earlier attempt charge unknown".into()),
            usd_upper: (!self.prior_unknown).then_some(0.0),
            cost_basis: Basis::ListPrice,
            milliseconds: 1,
        }
    }
}

#[tokio::test]
async fn missing_codex_usage_is_unknown_and_preserves_earlier_lower_bound() {
    for (provider, prior_unknown) in [("codex", false), ("codex", true), ("synthetic", false)] {
        let (_root, store, bytes) = fixture();
        let mut grant = task::owner::Grant::parse(&bytes).unwrap();
        let configuration = grant.adapter_configuration.as_mut().unwrap();
        configuration.provider = provider.into();
        if provider == "codex" {
            configuration.generation_endpoint = codex_transport::codex::BASE_URL.into();
            configuration.decision_endpoint = "https://decision.example.invalid".into();
        }
        let host = Host::admit(&store, &serde_json::to_vec(&grant).unwrap())
            .await
            .unwrap();
        let route = host.configuration().primary();
        let generated = RecordedGenerator {
            host: &host,
            inner: &MissingUsageGenerator { prior_unknown },
            route: &route,
        }
        .generate("fixture", "fixture")
        .await;
        assert_eq!(generated.known_usd, if prior_unknown { 0.002 } else { 0.0 });
        if prior_unknown {
            assert!(
                generated
                    .cost_unknown
                    .as_ref()
                    .unwrap()
                    .contains("earlier attempt")
            );
        }
        if provider == "codex" {
            assert_eq!(generated.usd, None);
            assert!(
                generated
                    .cost_unknown
                    .unwrap()
                    .contains("no usable token usage")
            );
        } else {
            assert_eq!(generated.usd, Some(0.0));
            assert_eq!(generated.cost_unknown, None);
        }
        host.finish("fixture_complete", false, json!({})).unwrap();
    }
}

struct ContextGenerator {
    prompt: RefCell<String>,
}
impl Generate for ContextGenerator {
    async fn generate(&self, _system: &str, prompt: &str) -> Generated {
        self.prompt.replace(prompt.into());
        Generated {
            action: Ok(NextAction {
                rationale: "Read the pinned reference.".into(),
                commands: Vec::new(),
                view: Vec::new(),
                freeze_tests: false,
                expand: Vec::new(),
                finished: true,
                reply: String::new(),
                ask: crate::models::Ask::None,
            }),
            model: "fixture-model".into(),
            prompt_tokens: 0,
            completion_tokens: 0,
            usd: Some(0.0),
            known_usd: 0.0,
            cost_unknown: None,
            usd_upper: Some(0.0),
            cost_basis: Basis::ListPrice,
            milliseconds: 0,
        }
    }
}

#[tokio::test]
async fn frozen_context_is_explicit_and_exact_bytes_reach_the_existing_loop() {
    use coder::task::checks;
    let (root, store, bytes) = fixture();
    let document = "---\nid: fixture.reference\nversion: 1\nkind: method\ntitle: Reference\nsummary: A synthetic reference.\ntags: [fixture]\napplies_when: A fixture needs context.\nstatus: candidate\nauthor: Fixture\nprovenance:\n  written_from: [reference]\n  cites: [Fixture specification]\nevidence: []\n---\n\nExact frozen reference bytes.\n";
    let entry = knowledge::Entry::parse(document).unwrap();
    std::fs::write(root.path().join("checkout/reference.md"), document).unwrap();
    let program = root.path().join("suite.sh");
    let suite = b"#!/bin/sh\nexit 0\n";
    std::fs::write(&program, suite).unwrap();
    let program = program.canonicalize().unwrap();
    let manifest = root.path().join("suite.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&coder::capability::executor_document(
            "task-check-test",
            program.to_str().unwrap(),
            vec![program.display().to_string(), "--version".into()],
            json!({"name":"Fixture","invoke":[program],"isolation":["directory"]}),
        ))
        .unwrap(),
    )
    .unwrap();
    let capability =
        coder::capability::Entry::load(&manifest, coder::capability::Source::Operator).unwrap();
    let mut grant = task::owner::Grant::parse(&bytes).unwrap();
    grant.requirements = Some(checks::Requirements {
        schema: checks::REQUIREMENTS_SCHEMA.into(),
        version: 1,
        requirements: vec![checks::Requirement {
            id: "output".into(),
            statement: "Independent content check.".into(),
            checks: vec!["content".into()],
        }],
        plan: json!({"schema":coder::verification::SCHEMA,"input_digest":checks::CANDIDATE,"seconds":5,"allow_unrestricted_reads":true,"allow_network":true,
            "checks":[{"id":"content","manifest":manifest,"manifest_digest":capability.digest,"arguments":[checks::CANDIDATE],"seconds":3,"output_bytes":4096,
            "acceptance":{"kind":"suite","suite_digest":nostr::contracts::digest_bytes(suite),"input_digest":checks::CANDIDATE}}]}),
        instruction_targets: Vec::new(),
        source_exclusions: Vec::new(),
        task_sources: vec!["fixture-target".into()],
        check_lineage: vec![checks::CheckLineage {
            check: "content".into(),
            sources: vec!["independent-reference".into()],
        }],
        knowledge: vec![checks::KnowledgeInput {
            id: entry.id,
            version: entry.version,
            digest: entry.digest,
            path: "reference.md".into(),
            sources: vec!["reference".into()],
        }],
    });
    assert!(
        Host::admit(&store, &serde_json::to_vec(&grant).unwrap())
            .await
            .is_err()
    );
    grant.adapter_configuration.as_mut().unwrap().knowledge = "frozen-context".into();
    let host = Host::admit(&store, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    assert_eq!(host.context().knowledge[0].text, document);
    let generator = ContextGenerator {
        prompt: RefCell::new(String::new()),
    };
    run(host, &generator, &JudgeFixture).await.unwrap();
    let prompt = generator.prompt.borrow();
    assert!(prompt.contains("Exact frozen reference bytes."));
    assert!(prompt.contains("fixture.reference") && prompt.contains("independent-reference"));
}

fn docker_grant(bytes: &[u8]) -> Vec<u8> {
    let mut grant = task::owner::Grant::parse(bytes).unwrap();
    let program = std::path::PathBuf::from(
        std::env::var_os("MICROCODER_REPOSITORY_DOCKER_PROGRAM").expect("explicit Docker program"),
    )
    .canonicalize()
    .unwrap();
    grant.wall_seconds = 60;
    grant.adapter_configuration.as_mut().unwrap().container =
        Some(task::adapter::container::Profile {
            schema: "openagents.microcoder.container.v1".into(),
            docker_digest: nostr::contracts::digest_bytes(&std::fs::read(&program).unwrap()),
            docker_program: program,
            socket: std::path::PathBuf::from(
                std::env::var_os("MICROCODER_REPOSITORY_DOCKER_SOCKET")
                    .expect("explicit Docker socket"),
            )
            .canonicalize()
            .unwrap(),
            image: std::env::var("MICROCODER_REPOSITORY_DOCKER_IMAGE").expect("explicit image ID"),
            uid: std::env::var("MICROCODER_REPOSITORY_DOCKER_UID")
                .unwrap()
                .parse()
                .unwrap(),
            gid: std::env::var("MICROCODER_REPOSITORY_DOCKER_GID")
                .unwrap()
                .parse()
                .unwrap(),
        });
    serde_json::to_vec(&grant).unwrap()
}

#[tokio::test]
#[ignore = "requires an explicitly admitted local Docker socket, executable, image, and user"]
async fn docker_repository_loop_retains_outputs_and_reconciles_whole_containers() {
    let (root, store, grant) = fixture();
    let grant = docker_grant(&grant);
    let outside = root.path().join("outside");
    std::fs::write(&outside, "unchanged").unwrap();
    let generator = generator(
        "set -e; test -z \"$TYPESAFE_API_KEY\"; test -z \"$OPENAI_API_KEY\"; test -z \"$HOME_SECRET\"; test ! -e /var/run/docker.sock; if printf denied > /outside-write; then exit 10; fi; if printf changed > .git; then exit 11; fi; printf output > result.txt; printf 'container output'",
    );
    let host = Host::admit(&store, &grant).await.unwrap();
    let result = run(host, &generator, &JudgeFixture).await.unwrap();
    assert_eq!(result.execution, task::Execution::Finished);
    assert!(
        result
            .run
            .as_ref()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .group_clear
    );
    assert_eq!(
        task::artifact::read(&store, "fixture", Path::new("result.txt")).unwrap(),
        b"output"
    );
    assert_eq!(std::fs::read(outside).unwrap(), b"unchanged");
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("container output") && trace.contains("\"container_removed\":true"));
    if let Some(destination) = std::env::var_os("MICROCODER_REPOSITORY_CONTAINER_PROOF_DIR") {
        let destination = std::path::PathBuf::from(destination);
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("grant.json"), grant).unwrap();
        std::fs::write(
            destination.join("task.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        std::fs::write(destination.join("trace.atif.jsonl"), trace).unwrap();
        std::fs::write(
            destination.join("view.json"),
            serde_json::to_vec_pretty(&task::view::read(&store, "fixture", None, 200).unwrap())
                .unwrap(),
        )
        .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires an explicitly admitted local Docker socket, executable, image, and user"]
async fn docker_cancellation_stops_the_whole_container_before_returning() {
    let (root, store, grant) = fixture();
    let grant = docker_grant(&grant);
    let generator = generator("(sleep 4; printf escaped > late.txt) & sleep 30");
    let host = Host::admit(&store, &grant).await.unwrap();
    let (result, ()) = tokio::join!(
        run(host, &generator, &JudgeFixture),
        cancel_after_dispatch(&store)
    );
    let result = result.unwrap();
    assert_eq!(result.execution, task::Execution::Stopped);
    assert!(result.run.unwrap().result.unwrap().group_clear);
    assert_eq!(generator.calls.get(), 1);
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(!root.path().join("checkout/late.txt").exists());
}

#[test]
fn landed_local_admission_keeps_its_original_serialized_identity() {
    let bytes = include_bytes!(
        "../../../../docs/coder/verification/2026-09-26-task-owner/microcoder-repository/grant.json"
    );
    let grant = task::owner::Grant::parse(bytes).unwrap();
    assert!(
        grant
            .adapter_configuration
            .as_ref()
            .unwrap()
            .container
            .is_none()
    );
    let original: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(serde_json::to_value(&grant).unwrap(), original);
    let original: Value = serde_json::from_slice(include_bytes!(
        "../../../../docs/coder/verification/2026-09-26-task-owner/microcoder-repository/task.json"
    ))
    .unwrap();
    let retained: task::Task = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(&retained).unwrap(), original);
}

/// The Codex refusal observed on 2026-09-28, and a clock inside its window.
const LIMIT: &str = r#"{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"pro","resets_at":1791050823,"eligible_promo":null,"limit_window_minutes":10080,"resets_in_seconds":478613}}"#;
const RESET: u64 = 1_791_050_823;
fn during_limit() -> u64 {
    1_790_572_210
}

/// A route whose replies are scripted: an action, or a capacity refusal.
struct ScriptedLane {
    /// The system text of each call.
    systems: RefCell<Vec<String>>,
    script: RefCell<VecDeque<Result<NextAction, Refusal>>>,
    model: &'static str,
    usd: f64,
    calls: Cell<usize>,
    refusal: RefCell<Option<Refusal>>,
}
fn lane(model: &'static str, usd: f64, script: Vec<Result<NextAction, Refusal>>) -> ScriptedLane {
    ScriptedLane {
        systems: RefCell::new(Vec::new()),
        script: RefCell::new(script.into()),
        model,
        usd,
        calls: Cell::new(0),
        refusal: RefCell::new(None),
    }
}
impl Generate for ScriptedLane {
    async fn generate(&self, system: &str, _prompt: &str) -> Generated {
        self.calls.set(self.calls.get() + 1);
        self.systems.borrow_mut().push(system.to_owned());
        let next = self
            .script
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(Refusal::codex(429, LIMIT, during_limit()).unwrap()));
        let (action, usd) = match next {
            Ok(action) => (Ok(action), self.usd),
            Err(refusal) => {
                *self.refusal.borrow_mut() = Some(refusal);
                (Err(format!("the provider returned HTTP 429: {LIMIT}")), 0.0)
            }
        };
        Generated {
            action,
            model: self.model.into(),
            prompt_tokens: 10,
            completion_tokens: 5,
            usd: Some(usd),
            known_usd: usd,
            cost_unknown: None,
            usd_upper: Some(usd),
            cost_basis: Basis::ListPrice,
            milliseconds: 1,
        }
    }
}
impl Lane for &ScriptedLane {
    fn refusal(&self) -> Option<Refusal> {
        self.refusal.borrow_mut().take()
    }
}
impl Generate for &ScriptedLane {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        (*self).generate(system, prompt).await
    }
}

fn route(provider: &str, model: &str) -> GrantRoute {
    GrantRoute {
        provider: provider.into(),
        model: model.into(),
        effort: None,
        generation_endpoint: Provider::from_config(provider).unwrap().endpoint().into(),
    }
}

fn write(command: &str) -> NextAction {
    NextAction {
        rationale: "Write the output.".into(),
        commands: vec![command.into()],
        view: Vec::new(),
        freeze_tests: false,
        expand: Vec::new(),
        finished: false,
        reply: String::new(),
        ask: crate::models::Ask::None,
    }
}

fn done() -> NextAction {
    NextAction {
        rationale: "Done.".into(),
        commands: Vec::new(),
        view: Vec::new(),
        freeze_tests: false,
        expand: Vec::new(),
        finished: true,
        reply: String::new(),
        ask: crate::models::Ask::None,
    }
}

#[tokio::test]
async fn a_capacity_refusal_fails_over_to_the_next_admitted_route_and_is_recorded() {
    let (root, store, grant) = fixture();
    let codex = lane(
        "gpt-6-luna",
        0.0,
        vec![Err(Refusal::codex(429, LIMIT, during_limit()).unwrap())],
    );
    let claude = lane(
        "claude-opus-5-5",
        0.25,
        vec![Ok(write("printf output > result.txt")), Ok(done())],
    );
    let host = Host::admit(&store, &grant).await.unwrap();
    let result = run_routes(
        host,
        store.clone(),
        vec![
            (route("codex", "gpt-6-luna"), &codex),
            (route("claude", "claude-opus-5-5"), &claude),
        ],
        &JudgeFixture,
        during_limit,
    )
    .await
    .unwrap();
    // One refused request, not three retries ending in bad replies.
    assert_eq!(codex.calls.get(), 1);
    assert_eq!(claude.calls.get(), 2);
    // Each step is told which engine it runs on, after a failover too
    // (#10084).
    assert!(
        codex.systems.borrow()[0].ends_with(
            "you are running as Codex (model gpt-6-luna), the coding engine OpenAgents chose."
        ),
        "{:?}",
        codex.systems.borrow()
    );
    for system in claude.systems.borrow().iter() {
        assert!(
            system.contains("you are running as Claude Code (model claude-opus-5-5)"),
            "{system}"
        );
    }
    assert_eq!(result.execution, task::Execution::Finished);
    assert_eq!(
        std::fs::read(root.path().join("checkout/result.txt")).unwrap(),
        b"output"
    );
    // The refusal is durable capacity state with its reset.
    let book = capacity::Book::load_with(&store, |_| None);
    let refusal = book.blocking(Provider::Codex, during_limit()).unwrap();
    assert_eq!(refusal.until, RESET);
    // The transcript records why the route changed, and the step's cost
    // counts both attempts: the refused one and the one that answered.
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("\"route_switch\""));
    assert!(trace.contains("usage_limit"));
    let generated: Vec<Value> = trace
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| line.pointer("/step/extensions/microcoder/event").cloned())
        .filter(|event| event["event"] == "generated")
        .collect();
    assert_eq!(generated.len(), 2, "{trace}");
    assert_eq!(generated[0]["generated"]["model"], "claude-opus-5-5");
    assert_eq!(generated[0]["generated"]["prompt_tokens"], 20);
    assert_eq!(generated[0]["generated"]["usd"], 0.25);
}

#[tokio::test]
async fn with_every_route_exhausted_the_run_ends_as_no_capacity_with_the_earliest_reset() {
    let (_root, store, grant) = fixture();
    let claude_refusal = Refusal::claude(true, Some(429), None, during_limit()).unwrap();
    let codex = lane(
        "gpt-6-luna",
        0.0,
        vec![Err(Refusal::codex(429, LIMIT, during_limit()).unwrap())],
    );
    let claude = lane("claude-opus-5-5", 0.0, vec![Err(claude_refusal.clone())]);
    let host = Host::admit(&store, &grant).await.unwrap();
    let result = run_routes(
        host,
        store.clone(),
        vec![
            (route("codex", "gpt-6-luna"), &codex),
            (route("claude", "claude-opus-5-5"), &claude),
        ],
        &JudgeFixture,
        during_limit,
    )
    .await
    .unwrap();
    assert_eq!((codex.calls.get(), claude.calls.get()), (1, 1));
    let run = result.run.unwrap();
    assert_eq!(run.result.unwrap().ending, capacity::NO_CAPACITY_ENDING);
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("\"route_switch\"") && trace.contains("\"route_exhausted\""));
    // Claude's unreported reset holds for the default; it is the earliest.
    let earliest = claude_refusal.until;
    assert!(earliest < RESET);
    assert!(trace.contains(&format!(
        "\"reason\":\"no_capacity\",\"detail\":{{\"resets_at\":{earliest}}}"
    )));
}

#[tokio::test]
async fn a_run_starts_on_the_first_route_with_capacity() {
    let (_root, store, grant) = fixture();
    capacity::record_with(
        &store,
        Refusal::codex(429, LIMIT, during_limit()).unwrap(),
        |_| None,
    )
    .unwrap();
    let codex = lane("gpt-6-luna", 0.0, vec![Ok(done())]);
    let claude = lane("claude-opus-5-5", 0.0, vec![Ok(done())]);
    let host = Host::admit(&store, &grant).await.unwrap();
    let result = run_routes(
        host,
        store.clone(),
        vec![
            (route("codex", "gpt-6-luna"), &codex),
            (route("claude", "claude-opus-5-5"), &claude),
        ],
        &JudgeFixture,
        during_limit,
    )
    .await
    .unwrap();
    assert_eq!((codex.calls.get(), claude.calls.get()), (0, 1));
    assert_eq!(result.execution, task::Execution::Finished);
    // The transcript says why the run did not start on the first route.
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    let start = trace
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|line| line.pointer("/step/extensions/route_capacity").cloned())
        .unwrap();
    assert_eq!(start["starts_on"]["provider"], "claude");
    assert_eq!(start["routes"][0]["refusal"]["kind"], "usage_limit");
    assert_eq!(start["routes"][0]["refusal"]["until"], RESET);
    assert!(start["routes"][1]["refusal"].is_null());
}

#[tokio::test]
async fn admission_accepts_the_task_model_on_any_admitted_route() {
    // The task records the policy's first model; the grant may start on a
    // fallback, and must still admit the recorded model.
    let (_root, store, bytes) = fixture();
    let mut grant = task::owner::Grant::parse(&bytes).unwrap();
    grant.adapter_configuration.as_mut().unwrap().model = "fixture-fallback".into();
    let (_other_root, other_store, other_bytes) = fixture();
    let mut refused = task::owner::Grant::parse(&other_bytes).unwrap();
    refused.adapter_configuration.as_mut().unwrap().model = "fixture-fallback".into();
    grant.adapter_configuration.as_mut().unwrap().fallbacks = vec![GrantRoute {
        provider: "synthetic".into(),
        model: "fixture-model".into(),
        effort: None,
        generation_endpoint: "in-process".into(),
    }];
    assert!(
        Host::admit(&other_store, &serde_json::to_vec(&refused).unwrap())
            .await
            .is_err_and(|error| error.to_string().contains("model differ"))
    );
    let host = Host::admit(&store, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    assert_eq!(host.configuration().routes().len(), 2);
    drop(host);
    // A repeated route or a real provider beside synthetic fixtures refuses.
    let configuration = refused.adapter_configuration.as_mut().unwrap();
    configuration.fallbacks = vec![configuration.primary()];
    assert!(configuration.validate().is_err());
    configuration.fallbacks = vec![route("claude", "claude-opus-5-5")];
    assert!(configuration.validate().is_err());
}

#[test]
fn a_grant_without_fallbacks_keeps_its_bytes() {
    let (_root, _store, bytes) = fixture();
    let grant = task::owner::Grant::parse(&bytes).unwrap();
    let text = serde_json::to_string(&grant).unwrap();
    assert!(!text.contains("fallbacks"));
    assert_eq!(crate::claude::ENDPOINT, Provider::Claude.endpoint());
}

#[tokio::test]
async fn a_follow_up_runs_as_the_next_turn_and_carries_the_earlier_one() {
    let (_root, store, grant) = fixture();
    let first = generator("printf output > result.txt");
    let host = Host::admit(&store, &grant).await.unwrap();
    let ended = run(host, &first, &JudgeFixture).await.unwrap();
    assert_eq!(ended.execution, task::Execution::Finished);
    let follow_up = Command {
        schema: task::COMMAND_SCHEMA.into(),
        command_id: "follow-up-fixture".into(),
        task_id: "fixture".into(),
        expected_revision: Some(ended.revision),
        action: Action::Continue {
            prompt: "Now also write done.txt.".into(),
        },
    };
    let bytes = serde_json::to_vec(&follow_up).unwrap();
    let receipt = Store::open(&store).unwrap().apply(&bytes).unwrap();
    assert_eq!(receipt.status, task::Status::Queued);
    // The earlier grant names an earlier revision and cannot run the turn.
    assert!(Host::admit(&store, &grant).await.is_err());
    let mut next: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
    next.expected_revision = receipt.revision;
    let next = serde_json::to_vec(&next).unwrap();
    let host = Host::admit(&store, &next).await.unwrap();
    assert_eq!(host.prompt(), "Now also write done.txt.");
    assert_eq!(host.earlier().len(), 1);
    assert_eq!(
        host.earlier()[0].reply.as_deref(),
        Some("The requested file exists.")
    );
    let prompt = host.engine_prompt();
    assert!(prompt.contains("Write result.txt containing output."));
    assert!(prompt.contains("The requested file exists."));
    assert!(prompt.ends_with("The user's new message:\nNow also write done.txt."));
    let context = ContextGenerator {
        prompt: RefCell::new(String::new()),
    };
    let task = run(host, &context, &JudgeFixture).await.unwrap();
    assert!(context.prompt.borrow().contains("Now also write done.txt."));
    assert!(
        context
            .prompt
            .borrow()
            .contains("Write result.txt containing output.")
    );
    assert_eq!(task.turn(), 2);
    assert_eq!(task.earlier.len(), 1);
    let run = task.run.as_ref().unwrap();
    assert_eq!(run.admission.trace_file, "fixture.2.atif.jsonl");
    assert_eq!(run.effect_id.as_deref(), Some("fixture:2:command"));
    // Both turns' traces stay; the second carries the first as marked steps.
    assert!(store.join("fixture.1.atif.jsonl").exists());
    let second = std::fs::read_to_string(store.join("fixture.2.atif.jsonl")).unwrap();
    assert!(second.contains("carried_from"));
    // The store replays both turns from its journal.
    assert_eq!(Store::open(&store).unwrap().show("fixture").unwrap(), task);
    // An exact retry of the follow-up returns its original receipt.
    assert_eq!(Store::open(&store).unwrap().apply(&bytes).unwrap(), receipt);
}

#[tokio::test]
async fn an_emulated_steer_stops_the_running_turn_and_continues_with_the_message() {
    use coder::task::commands::{self, Kind, Outcome, Request, Sender, State};
    let (_root, store, grant) = fixture();
    let generator = generator("sleep 30; printf output > result.txt");
    let host = Host::admit(&store, &grant).await.unwrap();
    let sender = Sender {
        device: "phone".into(),
        grant: None,
        epoch: None,
    };
    let always = |_: &Sender| true;
    let now = coder::task::autostart::unix_now();
    let steer = |emulate: bool, id: &str| Request {
        command: id.repeat(64),
        task: "fixture".into(),
        kind: Kind::Steer,
        based_on: 2,
        text: "Write steered.txt instead.".into(),
        emulate,
        issued_at: now,
    };
    let steered = async {
        let started = std::time::Instant::now();
        loop {
            let trace =
                std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap_or_default();
            if trace.contains("command started") {
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(4));
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        // Native steering of a running Microcoder turn is refused.
        let (native, _) = commands::record(
            &store,
            &sender,
            &steer(false, "a"),
            &crate::STEERING,
            &always,
            now,
        )
        .unwrap();
        assert!(matches!(
            native.state,
            State::Done(Outcome::Rejected { .. })
        ));
        // The chosen emulation stops the turn and holds the message.
        let (emulated, _) = commands::record(
            &store,
            &sender,
            &steer(true, "b"),
            &crate::STEERING,
            &always,
            now,
        )
        .unwrap();
        assert_eq!(emulated.state, State::Held { priority: true });
    };
    let (result, ()) = tokio::join!(run(host, &generator, &JudgeFixture), steered);
    let ended = result.unwrap();
    assert_eq!(ended.execution, task::Execution::Stopped);
    // When the turn has ended, the held steer becomes the next turn.
    let continued = commands::process(&store, "fixture", &crate::STEERING, &always, now).unwrap();
    assert_eq!(continued.len(), 1);
    let task = Store::open(&store).unwrap().show("fixture").unwrap();
    assert_eq!(task.status, task::Status::Queued);
    assert_eq!(task.effective_prompt(), "Write steered.txt instead.");
    assert_eq!(task.earlier.len(), 1);
}

/// The owner's report: asked "who are you", the chat showed the loop's
/// rationale and a `Finished.` marker instead of an answer. The engine's
/// reply is what a later turn carries as Coder's answer; the rationale is
/// not.
#[tokio::test]
async fn a_finished_reply_is_the_answer_and_the_rationale_is_not() {
    let (_root, store, grant) = fixture();
    let rationale = "This is a question about my identity, so no commands are needed; \
                     I'm answering it directly and marking the task complete.";
    let answer = "I'm Coder, the OpenAgents coding agent, running on this computer.";
    let generator = Generator {
        actions: RefCell::new(VecDeque::from([NextAction {
            rationale: rationale.into(),
            commands: Vec::new(),
            view: Vec::new(),
            freeze_tests: false,
            expand: Vec::new(),
            finished: true,
            reply: answer.into(),
            ask: crate::models::Ask::None,
        }])),
        model: "fixture-model",
        calls: Cell::new(0),
    };
    let host = Host::admit(&store, &grant).await.unwrap();
    let ended = run(host, &generator, &JudgeFixture).await.unwrap();
    assert_eq!(ended.execution, task::Execution::Finished);
    let follow_up = Command {
        schema: task::COMMAND_SCHEMA.into(),
        command_id: "follow-up-who".into(),
        task_id: "fixture".into(),
        expected_revision: Some(ended.revision),
        action: Action::Continue {
            prompt: "And what can you do?".into(),
        },
    };
    let receipt = Store::open(&store)
        .unwrap()
        .apply(&serde_json::to_vec(&follow_up).unwrap())
        .unwrap();
    let mut next: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
    next.expected_revision = receipt.revision;
    let host = Host::admit(&store, &serde_json::to_vec(&next).unwrap())
        .await
        .unwrap();
    assert_eq!(host.earlier()[0].reply.as_deref(), Some(answer));
    let prompt = host.engine_prompt();
    assert!(prompt.contains(answer));
    assert!(!prompt.contains(rationale) && !prompt.contains("Finished."));
    host.finish("fixture_complete", false, json!({})).unwrap();
}

/// A grant with `access: full` for the owner's own host.
fn full_access(grant: &[u8]) -> Vec<u8> {
    let mut grant: task::owner::Grant = serde_json::from_slice(grant).unwrap();
    grant.adapter_configuration.as_mut().unwrap().access = coder::task::adapter::Access::Full;
    serde_json::to_vec(&grant).unwrap()
}

/// A full-access run on a host whose every build slot is taken still
/// starts, building in its worktree (#10301): refusing it read as "another
/// process holds the task store lock" when more runs than slots started.
#[tokio::test]
async fn full_access_starts_when_every_build_slot_is_taken() {
    let (_root, store, grant) = fixture();
    let workspace = Store::open(&store)
        .unwrap()
        .show("fixture")
        .unwrap()
        .intent
        .workspace
        .path;
    let common = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(&workspace)
        .output()
        .unwrap();
    let common = std::path::PathBuf::from(String::from_utf8_lossy(&common.stdout).trim());
    let store_dir = store.canonicalize().unwrap();
    let mut held = Vec::new();
    while let Ok(lease) = coder::task::targets::Lease::acquire(&store_dir, &common) {
        held.push(lease);
    }
    assert!(!held.is_empty());
    let host = Host::admit(&store, &full_access(&grant)).await.unwrap();
    let built = host
        .command(
            "printf %s \"${CARGO_TARGET_DIR:-worktree}\"",
            Duration::from_secs(20),
        )
        .await
        .unwrap();
    assert_eq!(built.exit, Some(0), "{}", built.output);
    assert!(
        held.iter()
            .all(|lease| !built.output.contains(&*lease.path.to_string_lossy())),
        "{}",
        built.output
    );
    host.finish("fixture_complete", false, json!({})).unwrap();
}

/// The `fix-git` case (#10247): a full-access command, and a whole coding
/// agent approving its own tools, `cd` into the checkout the worktree was
/// made from and commit or merge there. Every write there fails and the
/// checkout's branch and files are as they were, while a commit in the
/// worktree itself works.
#[tokio::test]
async fn full_access_never_writes_the_checkout_the_worktree_came_from() {
    let (root, store, grant) = fixture();
    let source = root.path().join("repo").canonicalize().unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&source)
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    let master = git(&["rev-parse", "HEAD"]);
    let host = Host::admit(&store, &full_access(&grant)).await.unwrap();
    let guard = host.source_guard().expect("a linked worktree is guarded");
    assert!(guard.protected().any(|path| path == source));
    let identity = "export GIT_AUTHOR_NAME=E GIT_AUTHOR_EMAIL=e@example.invalid \
                    GIT_COMMITTER_NAME=E GIT_COMMITTER_EMAIL=e@example.invalid";
    let ours = host
        .command(
            &format!("{identity}; printf fixed > fixed.txt && git add fixed.txt && git commit -qm fix && git rev-parse HEAD"),
            Duration::from_secs(20),
        )
        .await
        .unwrap();
    assert_eq!(ours.exit, Some(0), "{}", ours.output);
    let commit = ours.output.lines().next().unwrap().trim().to_string();
    let escape = format!(
        "{identity}; cd '{}' && git merge -q --ff-only {commit} || git commit -q --allow-empty -m escape \
         || printf escape > escape.txt",
        source.display()
    );
    let escaped = host
        .command(&escape, Duration::from_secs(20))
        .await
        .unwrap();
    assert_ne!(escaped.exit, Some(0), "{}", escaped.output);
    // A whole coding agent (Grok Build, OpenCode, Devin) runs the same way.
    let (program, arguments) = host.private_argv(system_shell(), vec!["-c".into(), escape.clone()]);
    let agent = std::process::Command::new(program)
        .args(arguments)
        .current_dir(host.workspace())
        .envs(host.guard_environment())
        .output()
        .unwrap();
    assert!(!agent.status.success());
    assert_eq!(git(&["rev-parse", "HEAD"]), master);
    assert_eq!(git(&["status", "--porcelain"]), "");
    assert!(!source.join("escape.txt").exists());
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("\"source_guard\""), "{trace}");
    host.finish("fixture_complete", false, json!({})).unwrap();
}

/// Full access: no sandbox, the owner's login environment and real HOME,
/// and process listing; the boundary grant keeps its scratch HOME and its
/// write boundary.
#[tokio::test]
async fn full_access_runs_as_the_owner_with_no_sandbox() {
    let home = coder::task::adapter::login::account().home;
    let elsewhere = tempfile::tempdir_in("/var/tmp").unwrap();
    let outside = elsewhere.path().join("outside");
    let script = format!(
        "printf 'home=%s\\nuser=%s\\n' \"$HOME\" \"$USER\"; ps -p $$ -o pid= >/dev/null && echo ps=ok; \
         printf written > '{}' && echo write=ok",
        outside.display()
    );

    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &full_access(&grant)).await.unwrap();
    let observation = match host.command(&script, Duration::from_secs(20)).await {
        Ok(observation) => observation,
        Err(error) => panic!(
            "{error:?}: {}",
            std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap()
        ),
    };
    assert_eq!(observation.exit, Some(0), "{}", observation.output);
    assert!(
        observation
            .output
            .contains(&format!("home={}\n", home.display())),
        "{}",
        observation.output
    );
    assert!(
        !observation.output.contains("user=\n"),
        "{}",
        observation.output
    );
    assert!(
        observation.output.contains("ps=ok"),
        "{}",
        observation.output
    );
    assert!(
        observation.output.contains("write=ok"),
        "{}",
        observation.output
    );
    assert_eq!(std::fs::read(&outside).unwrap(), b"written");
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("\"access\":\"full\"") && trace.contains("host_network"));
    // The login shell is read beside admission: the admission record says
    // so, and what was read is recorded once, before the first command.
    let lines: Vec<&str> = trace.lines().collect();
    let read_at = lines
        .iter()
        .position(|line| line.contains("login environment, read for the run's commands"))
        .expect("the login environment is recorded");
    let command_at = lines
        .iter()
        .position(|line| line.contains("\"kind\":\"command\""))
        .unwrap();
    assert!(read_at < command_at);
    assert!(trace.contains("\"recorded\":\"before the first command\""));
    assert_eq!(
        trace
            .matches("login environment, read for the run's commands")
            .count(),
        1
    );
    // The workspace digests are kept for the next owner.
    assert!(store.join(coder::task::adapter::SNAPSHOT_DIGESTS).is_file());
    host.finish("fixture_complete", false, json!({})).unwrap();

    std::fs::remove_file(&outside).unwrap();
    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &grant).await.unwrap();
    let observation = host
        .command(&script, Duration::from_secs(20))
        .await
        .unwrap();
    assert!(
        !observation
            .output
            .contains(&format!("home={}\n", home.display())),
        "{}",
        observation.output
    );
    assert!(!outside.exists());
    host.finish("fixture_complete", false, json!({})).unwrap();
}

fn toolchains_access(grant: &[u8]) -> Vec<u8> {
    let mut grant: task::owner::Grant = serde_json::from_slice(grant).unwrap();
    grant.adapter_configuration.as_mut().unwrap().access = coder::task::adapter::Access::Toolchains;
    serde_json::to_vec(&grant).unwrap()
}

/// This computer's tools (a local run, #10045): the tools that run here
/// run inside the boundary, `HOME` is still the scratch, a write outside
/// the workspace is still denied, and the transcript records the allow
/// list the boundary used.
#[tokio::test]
async fn a_toolchain_run_uses_this_computers_tools_and_still_writes_only_its_workspace() {
    let home = coder::task::adapter::login::account().home;
    let elsewhere = tempfile::tempdir_in("/var/tmp").unwrap();
    let outside = elsewhere.path().join("outside");
    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &toolchains_access(&grant))
        .await
        .unwrap();
    let script = format!(
        "printf 'home=%s\\n' \"$HOME\"; git --version && echo git=ok; \
         printf inside > inside.txt && echo inside=ok; \
         if printf written > '{}' 2>/dev/null; then echo outside=written; else echo outside=denied; fi",
        outside.display()
    );
    let observation = host
        .command(&script, Duration::from_secs(20))
        .await
        .unwrap();
    let output = &observation.output;
    assert!(output.contains("outside=denied"), "{output}");
    assert!(output.contains("inside=ok"), "{output}");
    assert!(
        !output.contains(&format!("home={}\n", home.display())),
        "{output}"
    );
    // Where Git runs outside the boundary, it runs inside it (on macOS,
    // `/usr/bin/git` is an `xcrun` shim into Xcode or the Command Line
    // Tools).
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
    {
        assert!(output.contains("git=ok"), "{output}");
    }
    assert!(!outside.exists());
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("\"access\":\"toolchains\""), "{trace}");
    assert!(trace.contains("workspace_system_and_toolchains"));
    assert!(trace.contains("\"toolchains\":{\"reads\":["));
    assert!(trace.contains("\"source\":\"toolchains\""));
    host.finish("fixture_complete", false, json!({})).unwrap();
}

/// Full access applies to local commands only; a container keeps its own
/// boundary.
#[test]
fn full_access_is_refused_for_container_commands() {
    let (_root, _store, grant) = fixture();
    let grant: task::owner::Grant = serde_json::from_slice(&full_access(&grant)).unwrap();
    let mut configuration = grant.adapter_configuration.unwrap();
    assert!(configuration.validate().is_ok());
    configuration.container = Some(coder::task::adapter::container::Profile {
        schema: "openagents.microcoder.container.v1".into(),
        docker_program: "/usr/local/bin/docker".into(),
        docker_digest: format!("sha256:{}", "0".repeat(64)),
        socket: "/var/run/docker.sock".into(),
        image: format!("sha256:{}", "1".repeat(64)),
        uid: 501,
        gid: 20,
    });
    let refused = configuration.validate().unwrap_err().to_string();
    assert!(refused.contains("full access"), "{refused}");
}

/// A workspace must be a checkout's own top level: an empty directory
/// inside another repository, as the owner's host had, is refused before
/// anything runs.
#[tokio::test]
async fn an_empty_directory_inside_a_repository_is_not_a_workspace() {
    let (root, store, _grant) = fixture();
    let empty = root.path().join("repo/empty");
    std::fs::create_dir(&empty).unwrap();
    let command = Command {
        schema: task::COMMAND_SCHEMA.into(),
        command_id: "submit-empty".into(),
        task_id: "empty".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: TaskIntent {
                title: "Empty".into(),
                prompt: "who are you".into(),
                workspace: Workspace {
                    path: empty.canonicalize().unwrap().display().to_string(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: NAME.into(),
                    model: Some("fixture-model".into()),
                },
                images: Vec::new(),
            },
        },
    };
    let task = {
        let mut inbox = Store::open(&store).unwrap();
        inbox.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
        inbox.show("empty").unwrap()
    };
    let (_, _, grant) = fixture();
    let mut grant: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
    grant.task_id = task.task_id;
    grant.intent_digest = task.intent_digest;
    let refused = Host::admit(&store, &serde_json::to_vec(&grant).unwrap())
        .await
        .err()
        .map(|error| error.to_string());
    assert!(
        refused
            .as_deref()
            .is_some_and(|error| error.contains("top level")),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_question_ends_the_turn_waiting_and_the_first_answer_continues_it() {
    use coder::task::commands::{self, Kind, Outcome, Rejection, Request, Sender, State};
    use coder::task::interaction;
    let (_root, store, grant) = fixture();
    let asking = Generator {
        actions: RefCell::new(VecDeque::from([NextAction {
            rationale: "Two layouts fit; the user must choose.".into(),
            commands: Vec::new(),
            view: Vec::new(),
            freeze_tests: false,
            expand: Vec::new(),
            finished: false,
            reply: "Should result.txt hold one line or two?".into(),
            ask: crate::models::Ask::Question,
        }])),
        model: "fixture-model",
        calls: Cell::new(0),
    };
    let host = Host::admit(&store, &grant).await.unwrap();
    let waiting = run(host, &asking, &JudgeFixture).await.unwrap();
    // The turn ended as meant, asking; nothing ran.
    assert_eq!(waiting.execution, task::Execution::Finished);
    assert_eq!(asking.calls.get(), 1);
    let result = waiting.run.as_ref().unwrap().result.as_ref().unwrap();
    assert_eq!(result.ending, interaction::QUESTION_ENDING);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(
        interaction::pending(&waiting),
        Some(interaction::Kind::Question)
    );
    let always = |_: &Sender| true;
    let now = coder::task::autostart::unix_now();
    let answer = |device: &str, id: &str, based_on: u64| {
        (
            Sender {
                device: device.into(),
                grant: None,
                epoch: None,
            },
            Request {
                command: id.repeat(64),
                task: "fixture".into(),
                kind: Kind::Answer,
                based_on,
                text: "One line.".into(),
                emulate: false,
                issued_at: now,
            },
        )
    };
    // An answer to an earlier turn's question is stale.
    let (sender, stale) = answer("tablet", "a", 0);
    let (recorded, _) =
        commands::record(&store, &sender, &stale, &crate::STEERING, &always, now).unwrap();
    assert_eq!(
        recorded.state,
        State::Done(Outcome::Rejected {
            reason: Rejection::Stale
        })
    );
    // The first answer continues the task with it.
    let (sender, first) = answer("phone", "b", waiting.revision);
    let (recorded, continued) =
        commands::record(&store, &sender, &first, &crate::STEERING, &always, now).unwrap();
    assert!(matches!(
        recorded.state,
        State::Done(Outcome::Applied { .. })
    ));
    assert_eq!(continued.len(), 1);
    let task = recorded.task.unwrap();
    assert_eq!(task.status, task::Status::Queued);
    assert_eq!(interaction::pending(&task), None);
    // A competing answer finds the question already answered.
    let (sender, second) = answer("tablet", "c", waiting.revision);
    let (recorded, _) =
        commands::record(&store, &sender, &second, &crate::STEERING, &always, now).unwrap();
    assert!(matches!(
        recorded.state,
        State::Done(Outcome::Rejected { .. })
    ));
    // The next turn sees its question and the answer.
    let mut next: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
    next.expected_revision = task.revision;
    let host = Host::admit(&store, &serde_json::to_vec(&next).unwrap())
        .await
        .unwrap();
    let prompt = host.engine_prompt();
    assert!(prompt.contains("Should result.txt hold one line or two?"));
    assert!(prompt.ends_with("The user's new message:\nOne line."));
}

#[tokio::test]
async fn a_store_another_process_holds_is_not_a_stop() {
    let (_root, store, grant) = fixture();
    // Holding the store spends the lock wait from the turn's wall time; the
    // fixture's eight seconds would leave too little for the run under a
    // loaded test machine, where it would end as stopped by its deadline.
    let mut grant = task::owner::Grant::parse(&grant).unwrap();
    grant.wall_seconds = 60;
    let grant = serde_json::to_vec(&grant).unwrap();
    let host = Host::admit(&store, &grant).await.unwrap();
    assert!(!host.cancelled());
    // Hold the store past the lock wait, as a slow disk sync can.
    let held = Store::open(&store).unwrap();
    assert!(!host.cancelled(), "a busy store is not a stop request");
    drop(held);
    assert!(!host.cancelled());
    let task = run(
        host,
        &generator("printf output > result.txt"),
        &JudgeFixture,
    )
    .await
    .unwrap();
    assert_eq!(task.execution, task::Execution::Finished);
}

/// A chat's local run (`coder::task::local`) with scripted provider output:
/// the shared start writes the grant, this launcher runs the owner at once
/// with scripted routes instead of starting `microcoder`, and the shared
/// follower turns the recorded turns into the event stream the CLI, the
/// desktop, and the phone show.
mod local_run {
    use super::*;
    use coder::task::autostart::{Engine, Launch, Launched};
    use coder::task::local::{Local, State};
    use openagents_chat::coder_events::{CoderEvent, Line};
    use std::sync::Mutex;

    type Script = Vec<Result<NextAction, Refusal>>;

    /// Runs each launched turn in this process with the next scripted
    /// Codex and Claude Code replies, and Jev answering `progress` and
    /// `repeating` with the given probabilities (or nothing).
    struct Scripted(Mutex<VecDeque<(Script, Script)>>, Option<(f64, f64)>);

    /// Jev answering every step the same, or nothing.
    #[derive(Clone, Copy)]
    struct Answering(Option<(f64, f64)>);

    impl Judge for Answering {
        async fn judge(&self, _set: &QuestionSet, _state: &Value) -> Judgment {
            match self.0 {
                None => Judgment::free(),
                Some((progress, repeating)) => Judgment {
                    answers: vec![
                        ("done".into(), 0.1),
                        ("progress".into(), progress),
                        ("repeating".into(), repeating),
                    ],
                    ..Judgment::free()
                },
            }
        }
    }

    impl Launch for Scripted {
        fn launch(&self, _: &Engine, grant: &Path, store: &Path) -> Result<Launched, String> {
            let bytes = std::fs::read(grant).map_err(|e| e.to_string())?;
            let (codex, claude) = self.0.lock().unwrap().pop_front().ok_or("no script")?;
            let judge = Answering(self.1);
            let store = store.to_path_buf();
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async {
                    let codex = lane("gpt-6-luna", 0.0, codex);
                    let claude = lane("claude-opus-5-5", 0.1, claude);
                    let host = Host::admit(&store, &bytes).await.unwrap();
                    run_routes(
                        host,
                        store.clone(),
                        vec![
                            (route("codex", "gpt-6-luna"), &codex),
                            (route("claude", "claude-opus-5-5"), &claude),
                        ],
                        &judge,
                        during_limit,
                    )
                    .await
                    .unwrap();
                });
            })
            .join()
            .map_err(|_| "the scripted owner panicked".to_owned())?;
            Ok(Launched {
                owner_process: std::process::id(),
                grant_digest: String::new(),
            })
        }
    }

    fn signed_in(_: Provider) -> capacity::Connection {
        capacity::Connection::Connected
    }

    fn checkout(root: &Path) -> std::path::PathBuf {
        let top = root.join("slugs");
        std::fs::create_dir_all(&top).unwrap();
        std::fs::write(
            top.join("slugs.py"),
            "def slugify(text):\n    return text\n",
        )
        .unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "slugs.py"],
            vec![
                "-c",
                "user.name=F",
                "-c",
                "user.email=f@example.invalid",
                "commit",
                "-qm",
                "one",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&top)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        top
    }

    fn asking(reply: &str, ask: crate::models::Ask) -> NextAction {
        NextAction {
            reply: reply.into(),
            ask,
            finished: false,
            ..done()
        }
    }

    fn finished(reply: &str) -> NextAction {
        NextAction {
            reply: reply.into(),
            ..done()
        }
    }

    /// Every event until the task ends or asks.
    fn drain(local: &Local, task: &str) -> (Vec<Line>, State) {
        let mut follow = local.follow(task, Some(&"a".repeat(32)), Some("answer".into()));
        let mut lines = Vec::new();
        for _ in 0..200 {
            let (more, state) = follow.poll().unwrap();
            lines.extend(more);
            if state != State::Running {
                return (lines, state);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("the task never ended: {lines:?}");
    }

    fn names(lines: &[Line]) -> Vec<&'static str> {
        lines.iter().map(|line| line.event.name()).collect()
    }

    /// The scripted stream as the desktop's transcript test reads it
    /// (`crates/openagents-chat/fixtures/coder-events/NAME.ndjson`), with
    /// this run's temporary directory named `/tmp/scratch`. It is written
    /// under `UPDATE_FIXTURES=1`; otherwise the fixture must hold the same
    /// events, in order, as this run.
    /// The first task of the kept fixture `name`, as it was recorded.
    fn recorded_first_task(name: &str) -> Vec<Line> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../openagents-chat/fixtures/coder-events")
            .join(format!("{name}.ndjson"));
        let kept: Vec<Line> = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("no {}", path.display()))
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let first = kept[0].task.clone();
        kept.into_iter()
            .take_while(|line| line.task == first)
            .collect()
    }

    fn fixture(name: &str, root: &Path, lines: &[Line]) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../openagents-chat/fixtures/coder-events")
            .join(format!("{name}.ndjson"));
        let mut roots = vec![root.display().to_string()];
        if let Ok(real) = root.canonicalize() {
            roots.insert(0, real.display().to_string());
        }
        let mut text = String::new();
        for line in lines {
            let mut json = serde_json::to_string(line).unwrap();
            for root in &roots {
                json = json.replace(root.as_str(), "/tmp/scratch");
            }
            text.push_str(&json);
            text.push('\n');
        }
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            return;
        }
        let kept = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("no {}; run with UPDATE_FIXTURES=1", path.display()));
        let kept: Vec<Line> = kept
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            names(&kept),
            names(lines),
            "{} is out of date; run with UPDATE_FIXTURES=1",
            path.display()
        );
    }

    #[test]
    fn a_chat_run_streams_every_event_and_replays_identically() {
        let root = tempfile::tempdir().unwrap();
        let top = checkout(root.path());
        let store = root.path().join("tasks");
        // Local selects the second turn with the wall clock. Keep this
        // refusal active throughout both turns rather than using a past reset.
        let now = coder::task::autostart::unix_now();
        let reset = now + 3_600;
        let mut limit: Value = serde_json::from_str(LIMIT).unwrap();
        limit["error"]["resets_at"] = json!(reset);
        let refusal = Refusal::codex(429, &limit.to_string(), now).unwrap();
        let script = VecDeque::from([
            // Turn 1: Codex refuses for its usage limit, Claude Code writes
            // the test and asks.
            (
                vec![Err(refusal)],
                vec![
                    Ok(write("printf 'import unittest\\n' > test_slugs.py")),
                    Ok(asking(
                        "Should the test cover empty input too?",
                        crate::models::Ask::Question,
                    )),
                ],
            ),
            // Turn 2, after the answer: Codex is still refused in the
            // book, so Claude Code starts, and finishes.
            (
                vec![],
                vec![
                    Ok(write("printf 'x = 1\\n' >> test_slugs.py")),
                    Ok(finished("I added test_slugs.py.")),
                ],
            ),
        ]);
        let local = Local::new(store.clone())
            .with_probe(signed_in)
            .with_controller(std::env::current_exe().unwrap())
            // Not this computer's OpenCode configuration: the order must
            // not depend on what is installed where the test runs.
            .with_opencode_model(|| None)
            .with_launcher(Box::new(Scripted(Mutex::new(script), None)));
        let record = local
            .start(
                &top,
                "add a unit test for slugify",
                "add a unit test for slugify",
                Some(&"a".repeat(32)),
            )
            .unwrap();
        let (turn_one, state) = drain(&local, &record.task);
        assert_eq!(state, State::Waiting);
        let seen = names(&turn_one);
        for name in [
            "coder_started",
            "step",
            "progress",
            "provider_switched",
            "output",
            "question",
        ] {
            assert!(seen.contains(&name), "{name} missing from {seen:?}");
        }
        let CoderEvent::CoderStarted(started) = &turn_one[0].event else {
            panic!("{:?}", turn_one[0])
        };
        assert_eq!(started.provider, "codex");
        assert_eq!(started.reason, "Codex is signed in and has capacity.");
        // Grok Build follows Claude Code by default (#10091), and every
        // other agent is on unless turned off (#10184); OpenCode, with no
        // configured model here, is left out.
        assert_eq!(
            started.fallbacks,
            ["claude:claude-opus-5-5", "grok:default", "devin:default"]
        );
        let switched = turn_one
            .iter()
            .find_map(|line| match &line.event {
                CoderEvent::ProviderSwitched(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(switched.to.as_deref(), Some("claude:claude-opus-5-5"));
        assert_eq!(switched.resets_at, Some(reset));
        let CoderEvent::Question(asked) = &turn_one.last().unwrap().event else {
            panic!()
        };
        assert_eq!(asked.text, "Should the test cover empty input too?");
        assert_eq!(asked.answer.as_deref(), Some("answer"));
        // Sequence numbers count from one without gaps.
        assert!(
            turn_one
                .iter()
                .enumerate()
                .all(|(i, l)| l.seq == i as u64 + 1)
        );

        // The answer starts turn 2 on Claude Code, and says why.
        local.answer(&record.task, "Yes, cover it.").unwrap();
        let (whole, state) = drain(&local, &record.task);
        assert_eq!(state, State::Ended);
        assert_eq!(
            &whole[..turn_one.len()],
            &turn_one[..],
            "turn 1 replays identically"
        );
        let second: Vec<&Line> = whole[turn_one.len()..].iter().collect();
        let CoderEvent::CoderStarted(started) = &second[0].event else {
            panic!("{:?}", second[0])
        };
        assert_eq!((started.turn, started.provider.as_str()), (2, "claude"));
        // It says which engine runs and never names a limit (#10120).
        assert!(
            started.reason.contains("Claude Code") && !started.reason.contains("limit"),
            "{}",
            started.reason
        );
        let CoderEvent::Result(result) = &whole.last().unwrap().event else {
            panic!("{:?}", whole.last())
        };
        assert_eq!(result.summary, "I added test_slugs.py.");
        assert_eq!(result.files_changed.len(), 1);
        assert_eq!(result.files_changed[0].path, "test_slugs.py");
        assert_eq!((result.insertions, result.deletions), (2, 0));
        // The change is in Coder's worktree, never the checkout.
        assert!(Path::new(&result.worktree).join("test_slugs.py").exists());
        assert!(!top.join("test_slugs.py").exists());

        // `follow` of the finished task replays every event identically.
        let (again, _) = drain(&local, &record.task);
        assert_eq!(again, whole);
        // A follower that already emitted the ending emits nothing more:
        // the desktop keeps polling an ended task for a later turn.
        let mut follow = local.follow(&record.task, Some(&"a".repeat(32)), Some("answer".into()));
        assert_eq!(follow.poll().unwrap(), (whole.clone(), State::Ended));
        assert_eq!(follow.poll().unwrap(), (vec![], State::Ended));
        fixture("question-then-result", root.path(), &whole);
    }

    #[test]
    fn every_other_ending_is_its_own_event() {
        let root = tempfile::tempdir().unwrap();
        let top = checkout(root.path());
        let script = VecDeque::from([
            (
                vec![
                    Ok(asking(
                        "May I delete slugs.py?",
                        crate::models::Ask::Approval,
                    )),
                    Ok(finished("I deleted slugs.py.")),
                ],
                vec![],
            ),
            (
                vec![Err(Refusal::codex(429, LIMIT, during_limit()).unwrap())],
                vec![Err(
                    Refusal::claude(true, Some(429), None, during_limit()).unwrap()
                )],
            ),
        ]);
        let local = Local::new(root.path().join("tasks"))
            .with_probe(signed_in)
            .with_controller(std::env::current_exe().unwrap())
            .with_launcher(Box::new(Scripted(Mutex::new(script), None)));
        // Every step is approved in advance (#10104): a step that asks for
        // approval is granted it, and the turn ends with its result, never
        // waiting on the person.
        let approved = local.start(&top, "tidy", "tidy up", None).unwrap();
        let (lines, state) = drain(&local, &approved.task);
        assert_eq!(
            (state, *names(&lines).last().unwrap()),
            (State::Ended, "result")
        );
        assert!(
            !lines.iter().any(|line| matches!(
                line.event,
                CoderEvent::Approval(_) | CoderEvent::Question(_)
            )),
            "{lines:?}"
        );
        // The fixture keeps a task recorded before #10104 that waits on an
        // approval, as an older task store still holds; it still reads and
        // renders.
        let mut endings = recorded_first_task("other-endings");

        let exhausted = local.start(&top, "tidy", "tidy up", None).unwrap();
        let (lines, state) = drain(&local, &exhausted.task);
        assert_eq!(state, State::Ended);
        let CoderEvent::Failure(failure) = &lines.last().unwrap().event else {
            panic!("{lines:?}")
        };
        assert_eq!(
            failure.ending.as_deref(),
            Some(capacity::NO_CAPACITY_ENDING)
        );
        endings.extend(lines);

        // A turn stopped before it started ends as stopped.
        let idle = Local::new(root.path().join("tasks-idle"))
            .with_probe(signed_in)
            .with_controller(std::env::current_exe().unwrap())
            .with_launcher(Box::new(Scripted(Mutex::new(VecDeque::new()), None)));
        assert!(
            idle.start(&top, "tidy", "tidy up", None).is_err(),
            "no script, no start"
        );
        let task = std::fs::read_dir(root.path().join("tasks-idle/local"))
            .unwrap()
            .flatten()
            .find_map(|e| {
                e.file_name()
                    .to_string_lossy()
                    .strip_suffix(".json")
                    .map(str::to_owned)
            })
            .unwrap();
        idle.stop(&task).unwrap();
        let (lines, state) = drain(&idle, &task);
        assert_eq!(
            (state, *names(&lines).last().unwrap()),
            (State::Ended, "stopped")
        );
        endings.extend(lines);
        fixture("other-endings", root.path(), &endings);
    }

    /// Starts the owner of each launched turn on its own thread and returns
    /// at once, as a real launch does, so the turn can be stopped while it
    /// runs. The thread says how many model calls the turn made.
    struct Detached {
        script: Mutex<Option<Script>>,
        owner: Mutex<Option<std::thread::JoinHandle<usize>>>,
    }

    impl Launch for Detached {
        fn launch(&self, _: &Engine, grant: &Path, store: &Path) -> Result<Launched, String> {
            let bytes = std::fs::read(grant).map_err(|e| e.to_string())?;
            let script = self.script.lock().unwrap().take().ok_or("no script")?;
            let store = store.to_path_buf();
            let owner = std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async {
                    let codex = lane("gpt-6-luna", 0.0, script);
                    let claude = lane("claude-opus-5-5", 0.1, Vec::new());
                    let host = Host::admit(&store, &bytes).await.unwrap();
                    run_routes(
                        host,
                        store.clone(),
                        vec![
                            (route("codex", "gpt-6-luna"), &codex),
                            (route("claude", "claude-opus-5-5"), &claude),
                        ],
                        &JudgeFixture,
                        during_limit,
                    )
                    .await
                    .unwrap();
                    codex.calls.get() + claude.calls.get()
                })
            });
            *self.owner.lock().unwrap() = Some(owner);
            Ok(Launched {
                owner_process: std::process::id(),
                grant_digest: String::new(),
            })
        }
    }

    /// #10050: `openagents chat stop` (or a phone's Stop Coder too) while a
    /// command runs. The turn makes no model call after the stop and ends
    /// with exactly one `stopped`, with no failure or failed model call.
    #[test]
    fn a_turn_stopped_while_its_command_runs_ends_once_as_stopped() {
        let root = tempfile::tempdir().unwrap();
        let top = checkout(root.path());
        let store = root.path().join("tasks");
        let launcher = std::sync::Arc::new(Detached {
            script: Mutex::new(Some(vec![
                Ok(write("sleep 30")),
                Ok(write("printf 'x' > after.txt")),
                Ok(finished("Done.")),
            ])),
            owner: Mutex::new(None),
        });
        struct Shared(std::sync::Arc<Detached>);
        impl Launch for Shared {
            fn launch(&self, e: &Engine, g: &Path, s: &Path) -> Result<Launched, String> {
                self.0.launch(e, g, s)
            }
        }
        let local = Local::new(store.clone())
            .with_probe(signed_in)
            .with_controller(std::env::current_exe().unwrap())
            .with_launcher(Box::new(Shared(launcher.clone())));
        let record = local
            .start(&top, "slow", "run the slow thing", None)
            .unwrap();
        let trace = store.join(format!("{}.1.atif.jsonl", record.task));
        let started = std::time::Instant::now();
        while !std::fs::read_to_string(&trace)
            .unwrap_or_default()
            .contains("Supervised command started")
        {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "no command started"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        local.stop(&record.task).unwrap();
        let (lines, state) = drain(&local, &record.task);
        let calls = launcher
            .owner
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(calls, 1, "no model call after the stop");
        assert_eq!(state, State::Ended);
        let seen = names(&lines);
        assert_eq!(
            seen.iter().filter(|n| **n == "stopped").count(),
            1,
            "{seen:?}"
        );
        assert_eq!(seen.last(), Some(&"stopped"));
        assert!(!seen.contains(&"failure"), "{seen:?}");
        for line in &lines {
            let text = serde_json::to_string(line).unwrap();
            assert!(!text.contains("cannot make this transition"), "{text}");
        }
        let task = Store::open(&store).unwrap().show(&record.task).unwrap();
        assert_one_clean_stop(&store, &task);
        assert!(!Path::new(&record.worktree).join("after.txt").exists());
    }

    mod issue_flow {
        use super::*;
        use coder::task::issue_run::{
            CLAIM_MARK, Checked, Checks, Comment, Issue, Policy, RELEASE_MARK, Reference, Runner,
            Tracker,
        };
        use std::sync::Arc;

        /// GitHub as the flow sees it: one issue, and every comment and
        /// close it made.
        #[derive(Default)]
        struct FakeGitHub {
            comments: Mutex<Vec<String>>,
            closed: Mutex<Vec<u64>>,
        }

        impl coder::claim::Hub for FakeGitHub {
            fn comment(&self, _: &str, _: u64, body: &str) -> Result<(), String> {
                self.comments.lock().unwrap().push(body.into());
                Ok(())
            }
            fn comments(&self, _: &str, _: u64) -> Result<Vec<Comment>, String> {
                Ok(Vec::new())
            }
            fn labeled(&self, _: &str, _: &str) -> Result<Vec<u64>, String> {
                Ok(vec![])
            }
        }

        impl Tracker for FakeGitHub {
            fn repository(&self, _: &Path) -> Result<String, String> {
                Ok("acme/slugs".into())
            }
            fn issue(&self, _: &str, number: u64) -> Result<Issue, String> {
                Ok(Issue {
                    number,
                    title: "Add a slug helper".into(),
                    body: "Add helper.py with one helper.".into(),
                    url: format!("https://github.com/acme/slugs/issues/{number}"),
                    open: true,
                    comments: vec![Comment {
                        body: "Please keep it small.".into(),
                        at: 1,
                    }],
                })
            }
            fn close(&self, _: &str, number: u64) -> Result<(), String> {
                self.closed.lock().unwrap().push(number);
                Ok(())
            }
            fn pull_request(
                &self,
                _: &Path,
                _: &str,
                _: &str,
                _: &str,
                _: &str,
                _: &str,
            ) -> Result<String, String> {
                Err("not in this test".into())
            }
        }

        /// The repository's checks, answering in turn; green once the
        /// answers run out.
        struct Answers(Mutex<VecDeque<Vec<String>>>);

        impl Checks for Answers {
            fn check(&self, worktree: &Path, _: &Policy) -> Checked {
                assert!(worktree.join("helper.py").exists(), "the change is checked");
                Checked {
                    problems: self.0.lock().unwrap().pop_front().unwrap_or_default(),
                    ran: vec!["The fake checks ran.".into()],
                }
            }
        }

        fn git(dir: &Path, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .args(["-c", "user.name=F", "-c", "user.email=f@example.invalid"])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }

        /// A checkout whose `origin` is a bare repository with `main`,
        /// with `policy` committed as the repository's issue-flow policy.
        fn published(root: &Path, policy: &str) -> (std::path::PathBuf, std::path::PathBuf) {
            let top = checkout(root);
            std::fs::create_dir_all(top.join(".openagents")).unwrap();
            std::fs::write(top.join(".openagents/coder-issues.json"), policy).unwrap();
            git(&top, &["add", "-A"]);
            git(&top, &["commit", "-qm", "policy"]);
            git(&top, &["config", "user.name", "F"]);
            git(&top, &["config", "user.email", "f@example.invalid"]);
            let origin = root.join("origin.git");
            // `-b main` so a clone checks out `main` whatever the
            // computer's `init.defaultBranch` is.
            git(
                root,
                &[
                    "init",
                    "-q",
                    "--bare",
                    "-b",
                    "main",
                    origin.to_str().unwrap(),
                ],
            );
            git(&top, &["remote", "add", "origin", origin.to_str().unwrap()]);
            git(&top, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
            (top, origin)
        }

        fn runner(
            root: &Path,
            script: VecDeque<(Script, Script)>,
            checks: Answers,
            github: Arc<FakeGitHub>,
        ) -> Runner {
            judged_runner(root, script, checks, github, None)
        }

        /// [`runner`] with Jev answering `progress` and `repeating` on
        /// every step.
        fn judged_runner(
            root: &Path,
            script: VecDeque<(Script, Script)>,
            checks: Answers,
            github: Arc<FakeGitHub>,
            judged: Option<(f64, f64)>,
        ) -> Runner {
            let local = Local::new(root.join("tasks"))
                .with_probe(signed_in)
                .with_controller(std::env::current_exe().unwrap())
                .with_launcher(Box::new(Scripted(Mutex::new(script), judged)));
            Runner {
                local: Arc::new(local),
                tracker: github,
                checks: Arc::new(checks),
                land: None,
                skip_claimed: false,
                now: coder::task::autostart::unix_now,
                artifacts: None,
            }
        }

        /// #10049: a chat hands an issue to Coder; it claims it, works it
        /// in a worktree of origin/main, fixes what the checks find,
        /// rebases onto a main that moved, checks again, pushes main,
        /// comments the evidence, and closes the issue. The stream shows
        /// every step and ends in a result that links the issue, and it
        /// replays identically.
        #[test]
        fn an_issue_lands_on_main_with_evidence_and_closes() {
            let root = tempfile::tempdir().unwrap();
            let (top, origin) = published(
                root.path(),
                r#"{"land": "main", "fix_rounds": 2, "max_steps": 30}"#,
            );
            let script = VecDeque::from([
                (
                    vec![
                        Ok(write("printf 'x\\n' > helper.py")),
                        Ok(finished("I added helper.py.")),
                    ],
                    vec![],
                ),
                (
                    vec![
                        Ok(write("printf 'y\\n' >> helper.py")),
                        Ok(finished("I fixed the lint.")),
                    ],
                    vec![],
                ),
            ]);
            let github = Arc::new(FakeGitHub::default());
            let checks = Answers(Mutex::new(VecDeque::from([vec![
                "lint: helper.py needs a second line".to_owned(),
            ]])));
            let runner = runner(root.path(), script, checks, github.clone());
            let reference = Reference {
                repository: None,
                number: 7,
            };
            let thread = "a".repeat(32);
            let started = runner.begin(&top, &reference, Some(&thread)).unwrap();
            let task = started.record.task.clone();
            let base = git(&top, &["rev-parse", "origin/main"]);
            assert_eq!(
                started.record.base, base,
                "the worktree starts at origin/main"
            );

            // Someone else lands on main meanwhile.
            let other = root.path().join("other");
            git(
                root.path(),
                &[
                    "clone",
                    "-q",
                    origin.to_str().unwrap(),
                    other.to_str().unwrap(),
                ],
            );
            std::fs::write(other.join("notes.txt"), "later\n").unwrap();
            git(&other, &["add", "-A"]);
            git(&other, &["commit", "-qm", "someone else"]);
            git(&other, &["push", "-q", "origin", "HEAD:main"]);
            let theirs = git(&other, &["rev-parse", "HEAD"]);

            let flow = started.finish();
            assert!(flow.finished);
            assert_eq!(flow.link.outcome, "landed", "{flow:#?}");
            assert!(flow.link.closed);
            let landed = git(
                &other,
                &["ls-remote", origin.to_str().unwrap(), "refs/heads/main"],
            );
            let landed = landed.split_whitespace().next().unwrap().to_owned();
            assert_eq!(flow.link.commits, std::slice::from_ref(&landed));
            git(&other, &["fetch", "-q", "origin"]);
            assert_eq!(
                git(&other, &["rev-parse", &format!("{landed}~1")]),
                theirs,
                "rebased onto main"
            );
            assert_eq!(
                git(&other, &["show", &format!("{landed}:helper.py")]),
                "x\ny"
            );
            assert_eq!(
                git(&other, &["log", "-1", "--format=%s", &landed]),
                "Add a slug helper"
            );
            // The person's checkout is untouched.
            assert!(!top.join("helper.py").exists());

            let comments = github.comments.lock().unwrap().clone();
            assert_eq!(comments.len(), 2, "{comments:#?}");
            assert!(comments[0].starts_with("Claimed: ") && comments[0].contains(CLAIM_MARK));
            assert!(
                comments[1].contains("landed this on `main`"),
                "{}",
                comments[1]
            );
            assert!(
                comments[1].contains("helper.py") && comments[1].contains("The fake checks ran.")
            );
            assert_eq!(*github.closed.lock().unwrap(), [7]);

            let local = runner.local.clone();
            let (lines, state) = drain(&local, &task);
            assert_eq!(state, State::Ended);
            let notes: Vec<String> = lines
                .iter()
                .filter_map(|line| match &line.event {
                    CoderEvent::Step(step)
                        if step.kind == openagents_chat::coder_events::StepKind::Note =>
                    {
                        Some(step.text.clone())
                    }
                    _ => None,
                })
                .collect();
            assert!(
                notes[0].starts_with("Issue #7: Add a slug helper"),
                "{notes:#?}"
            );
            assert!(
                notes.iter().any(|n| n.starts_with("Claimed #7")),
                "{notes:#?}"
            );
            assert!(
                notes.iter().any(|n| n.contains("fix turn 1 of 2")),
                "{notes:#?}"
            );
            assert!(
                notes.iter().any(|n| n.contains("rebases the change")),
                "{notes:#?}"
            );
            // The claim comes before the first turn starts.
            let first_start = lines
                .iter()
                .position(|l| l.event.name() == "coder_started")
                .unwrap();
            assert!(matches!(&lines[0].event, CoderEvent::Step(_)) && first_start > 0);
            // Two turns: the work, then the fix; the first ends as a
            // result without the issue, the last carries it.
            let results: Vec<_> = lines
                .iter()
                .filter_map(|line| match &line.event {
                    CoderEvent::Result(result) => Some(result.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(results.len(), 2);
            assert!(results[0].issue.is_none());
            let last = results.last().unwrap();
            let issue = last.issue.as_ref().unwrap();
            assert_eq!(
                (issue.number, issue.outcome.as_str(), issue.closed),
                (7, "landed", true)
            );
            assert_eq!(issue.url, "https://github.com/acme/slugs/issues/7");
            assert_eq!(last.files_changed.len(), 1, "{:?}", last.files_changed);
            assert_eq!(last.files_changed[0].path, "helper.py");
            assert_eq!((last.insertions, last.deletions), (2, 0));
            assert!(last.summary.contains("closed #7"), "{}", last.summary);
            assert!(matches!(lines.last().unwrap().event, CoderEvent::Result(_)));
            // A later follower replays the same stream.
            let (again, _) = drain(&local, &task);
            assert_eq!(again, lines);
        }

        /// A change whose checks stay red is never pushed: the issue gets
        /// an honest comment with the failing output, stays open, and the
        /// claim is released.
        #[test]
        fn red_checks_push_nothing_and_say_why_on_the_issue() {
            let root = tempfile::tempdir().unwrap();
            let (top, origin) = published(root.path(), r#"{"land": "main", "fix_rounds": 0}"#);
            let before = git(
                &top,
                &["ls-remote", origin.to_str().unwrap(), "refs/heads/main"],
            );
            let script = VecDeque::from([(
                vec![
                    Ok(write("printf 'x\\n' > helper.py")),
                    Ok(finished("I added helper.py.")),
                ],
                vec![],
            )]);
            let github = Arc::new(FakeGitHub::default());
            let checks = Answers(Mutex::new(VecDeque::from([vec![
                "the slugs tests fail: assertion failed".to_owned(),
            ]])));
            let runner = runner(root.path(), script, checks, github.clone());
            let reference = Reference {
                repository: Some("acme/slugs".into()),
                number: 8,
            };
            let started = runner.begin(&top, &reference, None).unwrap();
            let task = started.record.task.clone();
            let flow = started.finish();
            assert_eq!(flow.link.outcome, "failed");
            assert!(!flow.link.closed && flow.link.commits.is_empty());
            assert_eq!(
                git(
                    &top,
                    &["ls-remote", origin.to_str().unwrap(), "refs/heads/main"]
                ),
                before,
                "main did not move"
            );
            let comments = github.comments.lock().unwrap().clone();
            // Failed runs also post the short claim release from #10144.
            assert_eq!(comments.len(), 3, "{comments:#?}");
            assert_eq!(
                comments[2],
                format!("Coder released its claim; nothing landed. {RELEASE_MARK}")
            );
            assert!(
                comments[1].contains("did not land a change"),
                "{}",
                comments[1]
            );
            assert!(comments[1].contains("assertion failed"));
            assert!(comments[1].contains(RELEASE_MARK));
            assert!(github.closed.lock().unwrap().is_empty());
            let (lines, state) = drain(&runner.local, &task);
            assert_eq!(state, State::Ended);
            let CoderEvent::Failure(failure) = &lines.last().unwrap().event else {
                panic!("{:?}", lines.last())
            };
            let issue = failure.issue.as_ref().unwrap();
            assert_eq!((issue.number, issue.outcome.as_str()), (8, "failed"));
            // The red change is kept on a stranded branch, never on main
            // (#10993).
            let branch = format!("coder/stranded-{}", &task[..8]);
            assert!(
                failure.message.contains(&format!("kept on `{branch}`")),
                "{}",
                failure.message
            );
            assert!(comments[1].contains(&branch), "{}", comments[1]);

            // A different repository's issue is refused plainly.
            let elsewhere = Reference {
                repository: Some("other/repo".into()),
                number: 1,
            };
            let Err(refused) = runner.begin(&top, &elsewhere, None) else {
                panic!("started")
            };
            assert!(
                refused.to_string().contains("another repository"),
                "{refused}"
            );
        }

        /// Each step writes one more line: steady work that never says
        /// it finished.
        fn working(steps: usize, from: usize) -> Script {
            (from..from + steps)
                .map(|n| Ok(write(&format!("printf '{n}\\n' >> helper.py"))))
                .collect()
        }

        fn main_of(top: &Path, origin: &Path) -> String {
            git(
                top,
                &["ls-remote", origin.to_str().unwrap(), "refs/heads/main"],
            )
        }

        fn turns(runner: &Runner, task: &str) -> usize {
            coder::task::local::record(runner.local.store(), task)
                .unwrap()
                .turns
                .len()
        }

        /// #10103: a turn has no step limit. One that keeps making
        /// progress goes past every limit Coder used to set (24 steps for a
        /// chat run, 40 and 80 at the delegate door, 100 in this flow's
        /// policy), in one turn, then finishes, passes the checks, and
        /// lands. An older policy's `max_steps` and `continue_turns` are
        /// read and ignored.
        #[test]
        fn a_run_past_the_old_step_limits_keeps_going_and_lands() {
            let root = tempfile::tempdir().unwrap();
            let (top, origin) = published(
                root.path(),
                r#"{"land": "main", "max_steps": 3, "continue_turns": 2}"#,
            );
            let mut steps = working(105, 1);
            steps.push(Ok(finished("I finished helper.py.")));
            let script = VecDeque::from([(steps, vec![])]);
            let github = Arc::new(FakeGitHub::default());
            let runner = judged_runner(
                root.path(),
                script,
                Answers(Mutex::new(VecDeque::new())),
                github.clone(),
                Some((0.8, 0.1)),
            );
            let reference = Reference {
                repository: None,
                number: 11,
            };
            let started = runner.begin(&top, &reference, None).unwrap();
            let task = started.record.task.clone();
            let flow = started.finish();
            assert_eq!(flow.link.outcome, "landed", "{flow:#?}");
            assert!(flow.link.closed);
            assert_eq!(turns(&runner, &task), 1, "one turn, never continued");
            let landed = main_of(&top, &origin);
            let landed = landed.split_whitespace().next().unwrap();
            let file = git(
                &top,
                &[
                    "--git-dir",
                    origin.to_str().unwrap(),
                    "show",
                    &format!("{landed}:helper.py"),
                ],
            );
            assert_eq!(file.lines().count(), 105, "every step ran");
            for limit in [24, 40, 80, 100] {
                assert!(file.lines().any(|line| line == limit.to_string()));
            }
            let comments = github.comments.lock().unwrap().clone();
            assert!(!comments[1].contains("continuation"), "{}", comments[1]);
            let (lines, state) = drain(&runner.local, &task);
            assert_eq!(state, State::Ended);
            let CoderEvent::Result(last) = &lines.last().unwrap().event else {
                panic!("{:?}", lines.last())
            };
            assert_eq!(last.issue.as_ref().unwrap().outcome, "landed");
            // The running line never names a budget.
            for line in &lines {
                if let CoderEvent::Progress(progress) = &line.event {
                    let shown = serde_json::to_string(progress).unwrap();
                    assert!(!shown.contains("max_steps"), "{shown}");
                }
            }
        }

        /// A run the loop judges repeating a failed approach without
        /// progress is ended by the stuck guard alone, fails without being
        /// continued, pushes nothing, and says why and how far it got.
        #[test]
        fn a_repeating_run_is_ended_by_the_stuck_guard() {
            let root = tempfile::tempdir().unwrap();
            let (top, origin) = published(root.path(), r#"{"land": "main"}"#);
            let before = main_of(&top, &origin);
            let script = VecDeque::from([(working(40, 1), vec![])]);
            let github = Arc::new(FakeGitHub::default());
            let runner = judged_runner(
                root.path(),
                script,
                Answers(Mutex::new(VecDeque::new())),
                github.clone(),
                Some((0.2, 0.9)),
            );
            let reference = Reference {
                repository: None,
                number: 12,
            };
            let started = runner.begin(&top, &reference, None).unwrap();
            let task = started.record.task.clone();
            let worktree = std::path::PathBuf::from(&started.record.worktree);
            let flow = started.finish();
            assert_eq!(flow.link.outcome, "failed", "{flow:#?}");
            assert_eq!(turns(&runner, &task), 1, "no continuation");
            assert_eq!(main_of(&top, &origin), before, "nothing pushed");
            // Steps 2 to 9 were judged stuck; step 9 never ran. The work
            // is kept on its stranded branch (#10993), and the worktree,
            // with nothing unsaved, is retired.
            let _ = worktree;
            let branch = format!("coder/stranded-{}", &task[..8]);
            assert_eq!(
                git(&origin, &["show", &format!("{branch}:helper.py")])
                    .lines()
                    .count(),
                crate::run::STUCK_STEPS
            );
            let comments = github.comments.lock().unwrap().clone();
            // Failed runs also post the short claim release from #10144.
            assert_eq!(comments.len(), 3, "{comments:#?}");
            assert_eq!(
                comments[2],
                format!("Coder released its claim; nothing landed. {RELEASE_MARK}")
            );
            let failure = &comments[1];
            assert!(failure.contains("stuck"), "{failure}");
            assert!(failure.contains("without progress"), "{failure}");
            assert!(
                failure.contains("**How far it got**") && failure.contains("helper.py"),
                "{failure}"
            );
            assert!(failure.contains(RELEASE_MARK));
            assert!(github.closed.lock().unwrap().is_empty());
        }
    }
}

fn image_uploads() -> Vec<coder::task::media::wire::Upload> {
    let png = {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend((0..70_000u32).map(|i| (i * 31 % 256) as u8));
        bytes
    };
    let jpeg = {
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xe0];
        bytes.extend((0..5_000u32).map(|i| (i * 17 % 256) as u8));
        bytes
    };
    vec![
        coder::task::media::wire::Upload::new("layout.png", std::sync::Arc::new(png)).unwrap(),
        coder::task::media::wire::Upload::new("photo.jpg", std::sync::Arc::new(jpeg)).unwrap(),
    ]
}

/// A fake Codex transport the test keeps a handle to after the run takes it.
struct Recording(std::rc::Rc<codex_transport::fake::FakeTransport>);

impl codex_transport::Transport for Recording {
    async fn respond(
        &self,
        request: &codex_transport::Request,
    ) -> Result<codex_transport::Reply, codex_transport::TransportError> {
        self.0.respond(request).await
    }
}

/// The decoded bytes of each `input_image` data URL in a Codex request.
fn codex_images(request: &codex_transport::Request) -> Vec<(String, Vec<u8>)> {
    use base64::Engine as _;
    request
        .input
        .iter()
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .filter(|part| part["type"] == "input_image")
        .map(|part| {
            let url = part["image_url"].as_str().unwrap();
            let (head, data) = url.split_once(";base64,").unwrap();
            (
                head.trim_start_matches("data:").to_owned(),
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn attached_images_reach_the_codex_engine_as_their_exact_bytes() {
    let uploads = image_uploads();
    let (_root, store, grant) =
        fixture_images("fixture-model", |_| {}, &uploads, "Fix the layout.");
    let host = Host::admit(&store, &grant).await.unwrap();
    let loaded = host.images().unwrap();
    let images: Vec<crate::images::InputImage> = loaded
        .into_iter()
        .map(|(reference, bytes)| crate::images::InputImage {
            media_type: reference.media_type,
            bytes: std::sync::Arc::new(bytes),
        })
        .collect();
    let fake = codex_transport::fake::FakeTransport::default();
    let action = serde_json::to_string(&done()).unwrap();
    fake.then(codex_transport::Reply {
        id: Some("image-fixture".into()),
        model: "fixture-model".into(),
        items: vec![json!({"type":"message","content":[{"type":"output_text","text":action}]})],
        usage: codex_transport::TokenUsage::default(),
    });
    let fake = std::rc::Rc::new(fake);
    let stages: Vec<Stage<Recording>> = vec![Stage::Loop(vec![(
        route("codex", "fixture-model"),
        Client::Codex(Recording(fake.clone())),
    )])];
    let task = run_stages(
        host,
        store.clone(),
        stages,
        Err("off in this fixture".into()),
        "fixture-session",
        &images,
    )
    .await
    .unwrap();
    assert_eq!(task.execution, task::Execution::Finished);
    let requests = fake.requests();
    assert_eq!(requests.len(), 1);
    let received = codex_images(&requests[0]);
    let expected: Vec<(String, Vec<u8>)> = uploads
        .iter()
        .map(|upload| (upload.reference.media_type.clone(), upload.bytes.to_vec()))
        .collect();
    assert_eq!(received, expected);
    for ((_, bytes), upload) in received.iter().zip(&uploads) {
        assert_eq!(
            coder::task::media::wire::digest(bytes),
            upload.reference.digest
        );
    }
    // The transcript keeps each image's digest and size, not its bytes.
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    for upload in &uploads {
        assert!(trace.contains(&upload.reference.digest), "{trace}");
    }
    assert!(!trace.contains(";base64,"));
}

#[tokio::test]
async fn a_task_whose_image_bytes_changed_is_refused_before_any_model_call() {
    let uploads = image_uploads();
    let (_root, store, grant) =
        fixture_images("fixture-model", |_| {}, &uploads[..1], "Fix the layout.");
    let path = coder::task::media::path(&store, "fixture", &uploads[0].reference).unwrap();
    let mut changed = uploads[0].bytes.to_vec();
    changed[20] ^= 1;
    std::fs::write(&path, changed).unwrap();
    let host = Host::admit(&store, &grant).await.unwrap();
    assert!(host.images().is_err());
    let task = refuse_images(host, "Coder couldn't read the attached images.".into()).unwrap();
    assert_ne!(task.execution, task::Execution::Finished);
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("images_refused"));
    assert!(!trace.contains("codex_request"));
}

#[test]
fn whole_agent_routes_are_left_out_of_a_task_with_images() {
    let agent = |engine| Stage::<Recording>::Agent(engine, route("codex", "m"), "/bin/true".into());
    let mut unavailable = Vec::new();
    let only_agents = image_stages(
        vec![agent(AgentEngine::Devin), agent(AgentEngine::Grok)],
        &mut unavailable,
    );
    assert!(only_agents.is_empty());
    assert_eq!(unavailable.len(), 2);
    assert_eq!(unavailable[0]["unavailable"], "Devin can't take images.");
    let fake = std::rc::Rc::new(codex_transport::fake::FakeTransport::default());
    let mut unavailable = Vec::new();
    let kept = image_stages(
        vec![
            agent(AgentEngine::OpenCode),
            Stage::Loop(vec![(route("codex", "m"), Client::Codex(Recording(fake)))]),
        ],
        &mut unavailable,
    );
    assert!(matches!(kept.as_slice(), [Stage::Loop(_)]));
    assert_eq!(unavailable[0]["unavailable"], "OpenCode can't take images.");
}

/// Live: the owner's Codex login reads a screenshot attached to a scratch
/// task and acts on it. Set `OPENAGENTS_LIVE_IMAGE` to a PNG or JPEG and
/// `OPENAGENTS_LIVE_IMAGE_WANT` to the word the model must write to
/// `answer.txt`. The task store, workspace, and repository are scratch; the
/// login is only read.
#[tokio::test]
#[ignore = "live: needs the owner's Codex login and OPENAGENTS_LIVE_IMAGE"]
async fn live_codex_acts_on_an_attached_screenshot() {
    let path = std::env::var("OPENAGENTS_LIVE_IMAGE").expect("OPENAGENTS_LIVE_IMAGE");
    let want = std::env::var("OPENAGENTS_LIVE_IMAGE_WANT").expect("OPENAGENTS_LIVE_IMAGE_WANT");
    let model = std::env::var("OPENAGENTS_LIVE_MODEL").unwrap_or_else(|_| "gpt-6-luna".into());
    let bytes = std::fs::read(path).unwrap();
    let upload =
        coder::task::media::wire::Upload::new("screenshot.png", std::sync::Arc::new(bytes))
            .unwrap();
    let (root, store, grant) = fixture_images(
        &model,
        |c| c.model = model.clone(),
        std::slice::from_ref(&upload),
        "Look at the attached image. Write the name of the one color that fills it, as one lowercase word, to answer.txt in the repository, then finish.",
    );
    // A real model needs more than the fixture's eight seconds.
    let mut grant: Value = serde_json::from_slice(&grant).unwrap();
    grant["wall_seconds"] = json!(300);
    let grant = serde_json::to_vec(&grant).unwrap();
    let host = Host::admit(&store, &grant).await.unwrap();
    let images: Vec<crate::images::InputImage> = host
        .images()
        .unwrap()
        .into_iter()
        .map(|(reference, bytes)| crate::images::InputImage {
            media_type: reference.media_type,
            bytes: std::sync::Arc::new(bytes),
        })
        .collect();
    let login = codex_transport::codex::Login::default_path().unwrap();
    let transport = codex_transport::codex::CodexTransport::new(login, "live-image").unwrap();
    let mut primary = route("codex", &model);
    primary.effort = Some("medium".into());
    let stages: Vec<Stage<codex_transport::codex::CodexTransport>> =
        vec![Stage::Loop(vec![(primary, Client::Codex(transport))])];
    let task = run_stages(
        host,
        store.clone(),
        stages,
        Err("off".into()),
        "live-image",
        &images,
    )
    .await
    .unwrap();
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains(&upload.reference.digest), "{trace}");
    assert!(!trace.contains(";base64,"));
    let answer = std::fs::read_to_string(root.path().join("checkout/answer.txt"))
        .unwrap_or_else(|_| panic!("no answer.txt; ending {:?}", task.execution));
    println!("answer.txt: {answer:?}");
    assert!(
        answer.to_lowercase().contains(&want.to_lowercase()),
        "{answer}"
    );
}
