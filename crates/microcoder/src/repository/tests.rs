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
                prompt: "Write result.txt containing output.".into(),
                workspace: Workspace {
                    path: checkout.canonicalize().unwrap().display().to_string(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: NAME.into(),
                    model: Some(model.into()),
                },
            },
        },
    };
    let mut inbox = Store::open(&store).unwrap();
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
            max_steps: 4,
            acceptance: false,
            route: "never".into(),
            knowledge: "off".into(),
            dollar_limit_micros: None,
            expected_controller_digest: None,
            container: None,
            fallbacks: Vec::new(),
            access: coder::task::adapter::Access::Boundary,
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
    assert_eq!(view.cost_status, "unknown");
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
    assert_eq!(task.unwrap().execution, task::Execution::Stopped);
    let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
    assert!(trace.contains("interrupted model request may still consume tokens"));
    assert!(!trace.contains("Supervised command started"));
    assert!(trace.contains("\"usd\":null"));
}

#[tokio::test]
async fn evidence_cap_stops_future_effects_but_keeps_final_disposition() {
    let (_root, store, grant) = fixture();
    let host = Host::admit(&store, &grant).await.unwrap();
    assert!(
        host.append(&Step::said(Source::Agent, &"x".repeat(8 * 1024 * 1024)))
            .is_err()
    );
    assert!(host.effect("generation", json!({})).is_err());
    let task = host.finish("evidence_limit", false, json!({})).unwrap();
    assert!(task.run.unwrap().result.unwrap().output_incomplete);
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
    script: RefCell<VecDeque<Result<NextAction, Refusal>>>,
    model: &'static str,
    usd: f64,
    calls: Cell<usize>,
    refusal: RefCell<Option<Refusal>>,
}
fn lane(model: &'static str, usd: f64, script: Vec<Result<NextAction, Refusal>>) -> ScriptedLane {
    ScriptedLane {
        script: RefCell::new(script.into()),
        model,
        usd,
        calls: Cell::new(0),
        refusal: RefCell::new(None),
    }
}
impl Generate for ScriptedLane {
    async fn generate(&self, _system: &str, _prompt: &str) -> Generated {
        self.calls.set(self.calls.get() + 1);
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
    assert_eq!(result.execution, task::Execution::Finished);
    assert_eq!(
        std::fs::read(root.path().join("checkout/result.txt")).unwrap(),
        b"output"
    );
    // The refusal is durable capacity state with its reset.
    let book = capacity::Book::load(&store);
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
    capacity::record(&store, Refusal::codex(429, LIMIT, during_limit()).unwrap()).unwrap();
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
    /// Codex and Claude Code replies.
    struct Scripted(Mutex<VecDeque<(Script, Script)>>);

    impl Launch for Scripted {
        fn launch(&self, _: &Engine, grant: &Path, store: &Path) -> Result<Launched, String> {
            let bytes = std::fs::read(grant).map_err(|e| e.to_string())?;
            let (codex, claude) = self.0.lock().unwrap().pop_front().ok_or("no script")?;
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
                        &JudgeFixture,
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
        let script = VecDeque::from([
            // Turn 1: Codex refuses for its usage limit, Claude Code writes
            // the test and asks.
            (
                vec![Err(Refusal::codex(429, LIMIT, during_limit()).unwrap())],
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
            .with_launcher(Box::new(Scripted(Mutex::new(script))));
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
        assert_eq!(started.fallbacks, ["claude:claude-opus-5-5"]);
        let switched = turn_one
            .iter()
            .find_map(|line| match &line.event {
                CoderEvent::ProviderSwitched(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(switched.to.as_deref(), Some("claude:claude-opus-5-5"));
        assert_eq!(switched.resets_at, Some(RESET));
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
        assert!(
            started
                .reason
                .starts_with("Codex reached its usage limit until ")
                && started.reason.ends_with("; using Claude Code."),
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
                vec![Ok(asking(
                    "May I delete slugs.py?",
                    crate::models::Ask::Approval,
                ))],
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
            .with_launcher(Box::new(Scripted(Mutex::new(script))));
        let approval = local.start(&top, "tidy", "tidy up", None).unwrap();
        let (lines, state) = drain(&local, &approval.task);
        assert_eq!(
            (state, *names(&lines).last().unwrap()),
            (State::Waiting, "approval")
        );
        let mut endings = lines;

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
            .with_launcher(Box::new(Scripted(Mutex::new(VecDeque::new()))));
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
}
