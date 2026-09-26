use super::*;
use crate::models::NextAction;
use coder::task::adapter::{CONFIG_SCHEMA, Configuration, NAME};
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
                    model: Some("fixture-model".into()),
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
        }),
    };
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
            },
            NextAction {
                rationale: "The requested file exists.".into(),
                commands: Vec::new(),
                view: Vec::new(),
                freeze_tests: false,
                expand: Vec::new(),
                finished: true,
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
            configuration.generation_endpoint = microluna::codex::BASE_URL.into();
            configuration.decision_endpoint = "https://decision.example.invalid".into();
        }
        let host = Host::admit(&store, &serde_json::to_vec(&grant).unwrap())
            .await
            .unwrap();
        let generated = RecordedGenerator {
            host: &host,
            inner: &MissingUsageGenerator { prior_unknown },
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
            }),
            model: "fixture-model".into(),
            prompt_tokens: 0,
            completion_tokens: 0,
            usd: Some(0.0),
            known_usd: 0.0,
            cost_unknown: None,
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
