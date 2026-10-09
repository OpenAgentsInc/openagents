use crate::sanitize::Plan;
use crate::service::{BuildError, BuildRequest, Builder};
use crate::*;
use coder_environment::capture::{Capture as CapturePolicy, within};
use coder_environment::evidence::{EvidenceStatus, Redactor, load_manifest};
use coder_environment::{
    ArtifactPin, BuildState, Command as EnvCommand, Environment, ImagePin, Inputs as RecipeInputs,
    Limits, Platform, ProjectLink, Provider, Qualification, Recipe, Script, SourcePin, Start,
    digest,
};
use coder_working_computer::driver::Driver;
use coder_working_computer::provider::fake::{FakeProvider, FakeRun, Inject};
use coder_working_computer::provider::{ImageRecord, ImageState};
use coder_working_computer::{Phase as ComputerPhase, Principal, Purpose};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const GH_SECRET: &str = "ghp_builder_secret_value_0123456789";
const SCRIPT: &str = "echo \"installing as $GH_TOKEN\"; ./install-script";

fn d(c: char) -> String {
    c.to_string().repeat(64)
}
fn capture_policy() -> CapturePolicy {
    CapturePolicy {
        required: ["target/release/app".to_string()].into(),
        exclude: ["secrets.env".to_string()].into(),
        keep_explored: ["~/.cache/sccache".to_string()].into(),
    }
}
fn recipe(script: &str, credentials: &[&str]) -> Recipe {
    Recipe {
        schema: coder_environment::RECIPE_SCHEMA.into(),
        base: ImagePin {
            provider: Provider::Boat,
            image_id: "coder-base-2026-10-08".into(),
            digest: d('a'),
        },
        runtime: ArtifactPin {
            revision: "rt-1".into(),
            digest: d('b'),
        },
        platform: Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: Script {
            cwd: ".".into(),
            digest: digest(script.as_bytes()),
        },
        start: Start::default(),
        inputs: RecipeInputs::default(),
        credential_names: credentials.iter().map(|c| c.to_string()).collect(),
        qualification: Qualification {
            profile: "rust-library".into(),
            plan_digest: d('e'),
        },
        limits: Limits {
            deadline_seconds: 3600,
            concurrent_machines: 1,
            total_machine_allocations: 2,
            output_bytes: 1 << 24,
        },
        capture: capture_policy(),
    }
}
fn pin() -> SourcePin {
    SourcePin {
        repository: Some("example/repo".into()),
        revision: "a".repeat(40),
        digest: "b".repeat(64),
    }
}
fn environment(recipe: Recipe) -> Environment {
    Environment::new(
        "env-1",
        ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        pin(),
        recipe,
        0,
    )
    .unwrap()
}

/// Emulates the sanitization script over the fake machine's files (the
/// real script runs under `sh` in `tests/sanitize_script.rs`).
fn emulate(plan: &Plan, files: &mut BTreeMap<String, String>, faulty: bool) -> FakeRun {
    let mut out = String::new();
    let keys: Vec<String> = files.keys().cloned().collect();
    for key in keys {
        let under = |roots: &[String]| roots.iter().any(|r| within(&key, r));
        let login = under(&plan.login) && !(faulty && key.ends_with(".credentials.json"));
        let explored =
            under(&plan.explored) && !under(&plan.keep) && !within(&key, &plan.own_record);
        if login || under(&plan.mounts) || under(&plan.exclude) || explored {
            files.remove(&key);
            out.push_str(&format!("removed {key}\n"));
        } else if key.ends_with(".git/config") {
            let text = files[&key].clone();
            let clean: String = text
                .lines()
                .filter(|l| !l.contains("extraheader"))
                .map(|l| match (l.find("://"), l.find('@')) {
                    (Some(a), Some(b)) if b > a => format!("{}{}\n", &l[..a + 3], &l[b + 1..]),
                    _ => format!("{l}\n"),
                })
                .collect();
            if clean != text {
                files.insert(key.clone(), clean);
                out.push_str(&format!("scrubbed {key}\n"));
            }
        }
    }
    let mut bad = false;
    for key in files.keys() {
        if plan.login.iter().any(|r| within(key, r)) {
            out.push_str(&format!("residue {key}\n"));
            bad = true;
        }
    }
    for r in &plan.required {
        if !files.keys().any(|k| within(k, r)) {
            out.push_str(&format!("missing {r}\n"));
            bad = true;
        }
    }
    if bad {
        FakeRun::exit(3, &out, "")
    } else {
        out.push_str(&format!("sanitized {}\n", plan.digest()));
        FakeRun::exit(0, &out, "")
    }
}

struct Harness {
    _dir: tempfile::TempDir,
    builder: Builder<FakeProvider>,
    /// The sanitizer leaves a Claude login behind.
    faulty: Arc<AtomicBool>,
    /// The install keeps running until the test finishes it.
    slow: Arc<AtomicBool>,
    /// The source step finds a different commit.
    wrong: Arc<AtomicBool>,
}

fn harness_with(recipe: Recipe) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let environments = coder_environment::store::Store::under(root.join("environments"));
    environments.create(&environment(recipe.clone())).unwrap();
    let blobs = root.join("blobs");
    std::fs::create_dir_all(&blobs).unwrap();
    std::fs::write(blobs.join(digest(SCRIPT.as_bytes())), SCRIPT).unwrap();
    let provider = FakeProvider::new(
        BTreeMap::from([("GH_TOKEN".into(), GH_SECRET.into())]),
        true,
    );
    let plan = Plan::new(&recipe.capture, "/tmp/oa-commands/sanitize");
    let faulty = Arc::new(AtomicBool::new(false));
    let slow = Arc::new(AtomicBool::new(false));
    let wrong = Arc::new(AtomicBool::new(false));
    let (f, s, w) = (faulty.clone(), slow.clone(), wrong.clone());
    provider.on_command(Box::new(move |spec, env, files| match spec.id.as_str() {
        "install" => {
            // The base carries an engine login; the install leaves tokens,
            // explored state, and its build output behind.
            for (k, v) in [
                ("~/.claude/.credentials.json", "{\"accessToken\":\"sk-ant-oat01-x\"}"),
                ("~/.codex/auth.json", "{\"access_token\":\"codex-x\"}"),
                ("~/.config/gh/hosts.yml", "oauth_token: gho_x"),
                ("~/.bash_history", "export GH_TOKEN=..."),
                ("~/.cache/sccache/blob", "warm"),
                ("~/.cache/pip/wheel", "explored"),
                ("/run/secrets/token", "mounted"),
                ("/tmp/oa-commands/install/stdout", "install output"),
                ("secrets.env", "API=x"),
                (
                    ".git/config",
                    "[remote \"origin\"]\n\turl = https://x-access-token:ghp_x@github.com/o/r.git\n[http]\n\textraheader = AUTHORIZATION: basic x\n",
                ),
                ("target/release/app", "binary"),
            ] {
                files.insert(k.into(), v.into());
            }
            let token = env.get("GH_TOKEN").cloned().unwrap_or_default();
            let mut run = FakeRun::exit(0, &format!("installing as {token}\n"), "");
            if s.load(Ordering::SeqCst) {
                run.exit = None;
            }
            run
        }
        "sanitize" => emulate(&plan, files, f.load(Ordering::SeqCst)),
        // The real script runs under `sh` in coder-environment-setup.
        "source" => {
            let pin = pin();
            let report = coder_environment_setup::source::Report::verified_for(&pin, true);
            if w.load(Ordering::SeqCst) {
                let mut bad = report;
                bad.head = "c".repeat(40);
                bad.error = Some("head".into());
                return FakeRun::exit(3, &bad.render(), "source: head\n");
            }
            files.insert("file.txt".into(), "pinned".into());
            FakeRun::exit(0, &report.render(), "")
        }
        _ => FakeRun::exit(0, "", ""),
    }));
    let computers = coder_working_computer::store::Store::under(root.join("computers"));
    let builder = Builder::new(
        root.join("build"),
        blobs,
        store::Store::under(root.join("jobs")),
        environments,
        Driver::new(computers, provider),
        Box::new(|names: &BTreeSet<String>| {
            let mut r = Redactor::new();
            if names.contains("GH_TOKEN") {
                r.select(GH_SECRET).map_err(|e| e.to_string())?;
            }
            Ok(r)
        }),
    );
    Harness {
        _dir: dir,
        builder,
        faulty,
        slow,
        wrong,
    }
}
fn harness() -> Harness {
    harness_with(recipe(SCRIPT, &["GH_TOKEN"]))
}
fn request(id: &str, expected: u64) -> BuildRequest {
    BuildRequest {
        request_id: id.into(),
        environment: "env-1".into(),
        owner: Principal {
            workspace: "ws-1".into(),
            principal: "user-1".into(),
        },
        expected_draft_revision: expected,
        size: "small".into(),
    }
}
fn provider(h: &Harness) -> &FakeProvider {
    &h.builder.computers.provider
}
fn count(h: &Harness, op: &str) -> usize {
    provider(h).calls().iter().filter(|c| *c == op).count()
}
fn env_build(h: &Harness, id: &str) -> coder_environment::BuildAttempt {
    h.builder
        .environments
        .read("env-1")
        .unwrap()
        .build(id)
        .unwrap()
        .clone()
}

#[tokio::test]
async fn a_clean_build_captures_a_sanitized_immutable_image() {
    let h = harness();
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready, "{job:?}");

    // A fresh builder computer, never a setup or chat computer, never
    // restored from a checkpoint.
    let c = h.builder.computers.store.read(&job.computer).unwrap();
    assert_eq!(
        c.purpose,
        Purpose::EnvironmentBuild {
            environment: "env-1".into(),
            build: "build-1".into()
        }
    );
    assert!(c.base.is_none() && c.checkpoints.is_empty());
    assert_eq!(count(&h, "create"), 1);
    assert_eq!(count(&h, "restore"), 0);
    assert_eq!(count(&h, "checkpoint"), 0);

    // The exact recipe revision's script ran once with ephemeral Git auth.
    let install = job.install.as_ref().unwrap();
    assert_eq!(install.spec.command, SCRIPT);
    assert_eq!(install.spec.env["GIT_CONFIG_KEY_1"], "credential.helper");
    assert_eq!(job.inputs.recipe_revision, 1);
    let resource = c.creates[0].resource.clone().unwrap();
    assert_eq!(count(&h, "command_start"), 3);
    // Sanitization sees no credential at all.
    let sanitize = &job.sanitize.as_ref().unwrap().spec;
    assert!(sanitize.credential_names.is_empty() && sanitize.env.is_empty());

    // The image: no login files, no token-bearing Git config, no unkept
    // explored state or private mounts; required and kept paths present.
    let image = job.image.clone().unwrap();
    assert_eq!(image.image_id, job.inputs.image_name);
    assert!(image.image_id.starts_with("oaenv-build-1-"));
    let files = provider(&h).image_files(&image.image_id).unwrap();
    for gone in [
        "~/.claude/.credentials.json",
        "~/.codex/auth.json",
        "~/.config/gh/hosts.yml",
        "~/.bash_history",
        "~/.cache/pip/wheel",
        "/run/secrets/token",
        "/tmp/oa-commands/install/stdout",
        "secrets.env",
    ] {
        assert!(!files.contains_key(gone), "{gone} was captured");
    }
    assert_eq!(files["~/.cache/sccache/blob"], "warm");
    assert_eq!(files["target/release/app"], "binary");
    let git = &files[".git/config"];
    assert!(!git.contains('@') && !git.contains("extraheader"), "{git}");
    assert!(job.report.as_ref().unwrap().clean_for(&job.inputs.plan));

    // Exact identity recorded on the BuildAttempt.
    let b = env_build(&h, "build-1");
    assert_eq!(b.state, BuildState::Ready);
    assert_eq!(b.image.as_ref(), Some(&image));
    assert_eq!(b.run.as_ref().unwrap().cloud_job, job.computer);
    let snapshot = image.snapshot_id.clone().unwrap();
    let record = job.capture.as_ref().unwrap().record.clone().unwrap();
    assert_eq!(record.snapshot.as_deref(), Some(snapshot.as_str()));
    let manifest = ImageManifest {
        schema: MANIFEST_SCHEMA.into(),
        environment: "env-1".into(),
        build_id: "build-1".into(),
        recipe_revision: 1,
        recipe_digest: b.recipe_digest.clone(),
        source: b.source.clone(),
        base: job.inputs.base.clone(),
        runtime: job.inputs.runtime.clone(),
        platform: job.inputs.platform.clone(),
        plan_digest: job.inputs.plan.digest(),
        checkout: job.checkout.clone().unwrap(),
        report: job.report.clone().unwrap(),
        name: image.image_id.clone(),
        snapshot,
        builder: resource.clone(),
    };
    assert_eq!(image.manifest_digest, manifest.digest());
    assert_eq!(job.manifest(), Some(manifest.clone()));

    // Usage and cleanup are retained; the builder is deleted.
    assert_eq!(job.usage.evidence, vec![format!("usage:{resource}")]);
    assert!(job.usage.uncertain.is_none());
    assert!(
        matches!(job.cleanup, Cleanup::Complete { .. }),
        "{:?}",
        job.cleanup
    );
    let c = h.builder.computers.store.read(&job.computer).unwrap();
    assert_eq!(c.phase, ComputerPhase::Deleted);

    // Evidence is sealed complete and redacted of the Git token.
    let seg = &job.segments[0];
    let sealed = seg.sealed.clone().unwrap();
    assert_eq!(sealed.status, EvidenceStatus::CompleteWithRedactions);
    let dir = h.builder.evidence_dir(&job.id, &seg.id);
    let m = load_manifest(&dir, &sealed.digest).unwrap();
    assert!(
        m.calls
            .iter()
            .any(|c| c.identity.tool == "environment.build.capture")
    );
    let stdout = std::fs::read_to_string(dir.join("streams/install.stdout")).unwrap();
    assert!(!stdout.contains(GH_SECRET), "{stdout}");

    // Repeating the request returns the same job; nothing runs again.
    let again = h.builder.start(&request("req-1", 1), 9_000).await.unwrap();
    assert_eq!(again.id, job.id);
    assert_eq!(count(&h, "create"), 1);
    assert_eq!(count(&h, "capture_image"), 1);
}

#[tokio::test]
async fn a_base_pinned_to_another_provider_never_allocates() {
    let mut r = recipe(SCRIPT, &["GH_TOKEN"]);
    r.base.provider = Provider::Gce;
    let h = harness_with(r);
    let refused = h.builder.start(&request("req-1", 1), 1_000).await;
    assert!(
        matches!(refused, Err(BuildError::Refused(_))),
        "{refused:?}"
    );
    assert_eq!(count(&h, "create"), 0);
}

#[tokio::test]
async fn the_image_identity_names_the_builders_provider() {
    let h = harness();
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.image.unwrap().provider, Provider::Boat);
    let c = h.builder.computers.store.read(&job.computer).unwrap();
    assert_eq!(c.provider, Provider::Boat);
}

#[tokio::test]
async fn a_recipe_edit_marks_earlier_builds_stale() {
    let h = harness();
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready);
    assert!(!h.builder.view(&job.id).unwrap().stale);
    let mut edited = recipe(SCRIPT, &["GH_TOKEN"]);
    edited.limits.deadline_seconds = 1800;
    h.builder
        .environments
        .apply(
            "env-1",
            &EnvCommand::UpdateRecipe {
                expected_draft_revision: 1,
                recipe: edited,
            },
            2_000,
        )
        .unwrap();
    let view = h.builder.view(&job.id).unwrap();
    assert!(view.stale);
    let env = h.builder.environments.read("env-1").unwrap();
    assert_eq!(env.stale_builds(), vec!["build-1"]);
    // A stale build can no longer be verified.
    let refused = h.builder.environments.apply(
        "env-1",
        &EnvCommand::StartVerification {
            request_id: "verify-1".into(),
            build_id: "build-1".into(),
            plan_digest: d('e'),
        },
        3_000,
    );
    assert!(matches!(
        refused,
        Err(coder_environment::store::StoreError::Refused(
            coder_environment::Refusal::StaleBuild {
                build_recipe: 1,
                draft: 2
            }
        ))
    ));
    // A build request against the old draft is refused too.
    assert!(h.builder.start(&request("req-2", 1), 4_000).await.is_err());
}

#[tokio::test]
async fn an_edit_during_a_build_cancels_it_before_capture() {
    let h = harness();
    h.slow.store(true, Ordering::SeqCst);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Installing);
    assert_eq!(env_build(&h, "build-1").state, BuildState::Installing);
    let mut edited = recipe(SCRIPT, &["GH_TOKEN"]);
    edited.limits.deadline_seconds = 1800;
    h.builder
        .environments
        .apply(
            "env-1",
            &EnvCommand::UpdateRecipe {
                expected_draft_revision: 1,
                recipe: edited,
            },
            2_000,
        )
        .unwrap();
    let c = h.builder.computers.store.read(&job.computer).unwrap();
    let resource = c.resource().unwrap().to_owned();
    provider(&h).finish_command(&resource, "install", 0, "done\n");
    let job = h.builder.advance(&job.id, 3_000).await.unwrap();
    assert_eq!(job.phase, Phase::Cancelled, "{job:?}");
    assert!(job.reason.as_deref().unwrap().contains("stale"));
    assert_eq!(count(&h, "capture_image"), 0);
    assert_eq!(env_build(&h, "build-1").state, BuildState::Cancelled);
    assert!(matches!(job.cleanup, Cleanup::Complete { .. }));
}

#[tokio::test]
async fn a_lost_capture_reply_is_reconciled_by_name_not_retried() {
    let h = harness();
    provider(&h).inject("capture_image", Inject::LostReply);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Capturing);
    assert!(job.unresolved.is_some());
    let b = env_build(&h, "build-1");
    assert_eq!(b.state, BuildState::NeedsReconciliation);
    assert!(b.unresolved.is_some());
    // A different build of this environment waits for reconciliation.
    assert!(h.builder.start(&request("req-2", 1), 1_500).await.is_err());

    let job = h.builder.advance(&job.id, 2_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready, "{job:?}");
    assert_eq!(count(&h, "capture_image"), 1);
    assert_eq!(job.capture.as_ref().unwrap().issued, 1);
    let b = env_build(&h, "build-1");
    assert_eq!(b.state, BuildState::Ready);
    // The unknown outcome stays in the attempt's history.
    assert!(
        b.history
            .iter()
            .any(|s| s.state == BuildState::NeedsReconciliation)
    );
}

#[tokio::test]
async fn a_capture_that_never_landed_is_reissued_under_the_same_name() {
    let h = harness();
    provider(&h).inject("capture_image", Inject::Unknown);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert!(job.unresolved.is_some());
    let job = h.builder.advance(&job.id, 2_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready);
    let cap = job.capture.as_ref().unwrap();
    assert_eq!(cap.issued, 2);
    assert_eq!(cap.name, job.inputs.image_name);
    // Each capture was preceded by a read of the name.
    assert_eq!(count(&h, "read_image"), 2);
}

#[tokio::test]
async fn an_image_ready_needs_the_providers_typed_readiness() {
    let h = harness();
    provider(&h).state.lock().unwrap().images_pending = true;
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Capturing);
    assert!(job.image.is_none());
    assert_eq!(env_build(&h, "build-1").state, BuildState::SnapshotPending);
    let job = h.builder.advance(&job.id, 2_000).await.unwrap();
    assert_eq!(job.phase, Phase::Capturing);
    provider(&h).settle_images();
    let job = h.builder.advance(&job.id, 3_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready);
    assert_eq!(count(&h, "capture_image"), 1);
}

#[tokio::test]
async fn an_image_name_held_by_another_source_is_never_replaced() {
    let h = harness();
    let env = h.builder.environments.read("env-1").unwrap();
    let name = image_name("env-1", "build-1", &env.draft().digest);
    let foreign = ImageRecord {
        name: name.clone(),
        source: "box-elsewhere".into(),
        state: ImageState::Ready,
        snapshot: Some("snap-foreign".into()),
        size_bytes: None,
    };
    provider(&h).state.lock().unwrap().images.insert(
        name.clone(),
        (foreign.clone(), BTreeMap::from([("x".into(), "y".into())])),
    );
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Failed);
    assert!(job.reason.as_deref().unwrap().contains("never replaced"));
    assert_eq!(count(&h, "capture_image"), 0);
    let state = provider(&h).state.lock().unwrap().images[&name].0.clone();
    assert_eq!(state, foreign);
    assert_eq!(env_build(&h, "build-1").state, BuildState::Failed);
    assert!(env_build(&h, "build-1").image.is_none());
}

#[tokio::test]
async fn failed_sanitization_never_captures() {
    let h = harness();
    h.faulty.store(true, Ordering::SeqCst);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Failed);
    let report = job.report.clone().unwrap();
    assert_eq!(
        report.residue,
        vec!["~/.claude/.credentials.json".to_string()]
    );
    assert_eq!(count(&h, "capture_image"), 0);
    assert!(matches!(job.cleanup, Cleanup::Complete { .. }));
}

#[tokio::test]
async fn a_missing_required_path_fails_the_build() {
    let mut r = recipe(SCRIPT, &["GH_TOKEN"]);
    r.capture.required.insert("target/debug/app".into());
    let h = harness_with(r);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Failed);
    assert_eq!(
        job.report.unwrap().missing,
        vec!["target/debug/app".to_string()]
    );
    assert_eq!(count(&h, "capture_image"), 0);
}

#[tokio::test]
async fn a_lost_start_reply_never_runs_a_command_twice() {
    let h = harness();
    // The first identified command (the source step) loses its reply.
    provider(&h).inject("command_start", Inject::LostReply);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Materializing);
    assert!(job.unresolved.is_some());
    assert_eq!(
        env_build(&h, "build-1").state,
        BuildState::NeedsReconciliation
    );
    let job = h.builder.advance(&job.id, 2_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready);
    let c = h.builder.computers.store.read(&job.computer).unwrap();
    let resource = c.creates[0].resource.clone().unwrap();
    // The fake retains processes after deletion only until the machine is
    // gone; count starts instead.
    let _ = resource;
    assert_eq!(count(&h, "command_start"), 3);
    assert!(matches!(job.source.unwrap().run, Run::Exited { code: 0 }));
    let install = job.install.unwrap();
    assert!(matches!(install.run, Run::Exited { code: 0 }));
}

#[tokio::test]
async fn a_restart_mid_install_continues_in_a_new_evidence_segment() {
    let h = harness();
    h.slow.store(true, Ordering::SeqCst);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Installing);
    h.builder.restart();
    let c = h.builder.computers.store.read(&job.computer).unwrap();
    provider(&h).finish_command(c.resource().unwrap(), "install", 0, "more\n");
    let job = h.builder.advance(&job.id, 2_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready);
    assert_eq!(job.segments.len(), 2);
    assert!(job.segments[1].sealed.is_some());
    assert_eq!(job.install.as_ref().unwrap().call, "install-seg-2");
}

#[tokio::test]
async fn unknown_cleanup_is_retained_and_blocks_new_builders() {
    let h = harness();
    provider(&h).inject("delete", Inject::Unknown);
    // The meter keeps running after the stop (a provider fault).
    provider(&h).state.lock().unwrap().sticky_meter = true;
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Ready);
    assert!(job.cleanup.uncertain(), "{:?}", job.cleanup);
    assert!(job.usage.uncertain.is_some());
    let refused = h.builder.start(&request("req-2", 1), 2_000).await;
    assert!(matches!(refused, Err(BuildError::Refused(m)) if m.contains("cleanup")));

    provider(&h).state.lock().unwrap().sticky_meter = false;
    let job = h.builder.cleanup(&job.id, 3_000).await.unwrap();
    assert!(
        matches!(job.cleanup, Cleanup::Complete { .. }),
        "{:?}",
        job.cleanup
    );
    // The sealed image is untouched by cleanup.
    assert_eq!(job.phase, Phase::Ready);
    let next = h.builder.start(&request("req-2", 1), 4_000).await.unwrap();
    assert_eq!(next.inputs.build_id, "build-2");
    assert_ne!(next.inputs.image_name, job.inputs.image_name);
}

#[tokio::test]
async fn a_builder_never_carries_a_claude_sign_in() {
    let h = harness_with(recipe(SCRIPT, &["CLAUDE_CODE_OAUTH_TOKEN"]));
    let refused = h.builder.start(&request("req-1", 1), 1_000).await;
    assert!(matches!(refused, Err(BuildError::Refused(_))));
    assert_eq!(count(&h, "create"), 0);
    assert!(
        h.builder
            .environments
            .read("env-1")
            .unwrap()
            .builds
            .is_empty()
    );
}

#[test]
fn a_recipe_cannot_require_or_keep_a_login_file() {
    for bad in [
        CapturePolicy {
            required: ["~/.claude/.credentials.json".to_string()].into(),
            ..Default::default()
        },
        CapturePolicy {
            keep_explored: ["~/.codex/auth.json".to_string()].into(),
            ..Default::default()
        },
        CapturePolicy {
            required: ["/run/secrets/token".to_string()].into(),
            ..Default::default()
        },
        CapturePolicy {
            required: ["~/.cache/pip".to_string()].into(),
            ..Default::default()
        },
        CapturePolicy {
            required: ["../escape".to_string()].into(),
            ..Default::default()
        },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    assert!(capture_policy().validate().is_ok());
    // An empty policy leaves earlier recipe digests unchanged.
    let mut r = recipe(SCRIPT, &[]);
    r.capture = CapturePolicy::default();
    assert!(!serde_json::to_string(&r).unwrap().contains("capture"));
}

#[tokio::test]
async fn a_job_record_rejects_a_drifted_image_name() {
    let h = harness();
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    let lease = h.builder.jobs.lease(&job.id).unwrap();
    let drift = lease.update(5_000, |j| {
        j.capture.as_mut().unwrap().name = "oaenv-latest".into();
    });
    assert!(drift.is_err());
    let drift = lease.update(5_000, |j| {
        j.image.as_mut().unwrap().snapshot_id = Some("snap-other".into());
    });
    assert!(drift.is_err());
}

#[tokio::test]
async fn a_checkout_that_is_not_the_pin_fails_before_the_install() {
    let h = harness();
    h.wrong.store(true, Ordering::SeqCst);
    let job = h.builder.start(&request("req-1", 1), 1_000).await.unwrap();
    assert_eq!(job.phase, Phase::Failed, "{job:?}");
    assert!(job.reason.as_deref().unwrap().contains("pinned source"));
    let checkout = job.checkout.clone().unwrap();
    assert_eq!(checkout.error.as_deref(), Some("head"));
    assert!(job.install.is_none());
    assert_eq!(count(&h, "command_start"), 1);
    assert_eq!(count(&h, "capture_image"), 0);
    // The fetch used ephemeral auth only.
    let source = job.source.unwrap();
    assert_eq!(source.spec.env["GIT_CONFIG_KEY_1"], "credential.helper");
    assert!(source.spec.command.contains(&pin().revision));
    assert_eq!(env_build(&h, "build-1").state, BuildState::Failed);
}

#[test]
fn a_snapshot_exclusion_file_is_removed_so_installed_toolchains_are_captured() {
    let plan = Plan::new(&CapturePolicy::default(), "/tmp/oa-commands/sanitize");
    assert!(plan.exclude.iter().any(|p| p == "~/.boxignore"));
    let with_recipe = Plan::new(&capture_policy(), "/tmp/oa-commands/sanitize");
    assert!(with_recipe.exclude.iter().any(|p| p == "secrets.env"));
    assert!(with_recipe.exclude.iter().any(|p| p == "~/.boxignore"));

    // Run the real script against a scratch home and root.
    let dir = tempfile::tempdir().unwrap();
    let (home, root, work) = (
        dir.path().join("home"),
        dir.path().join("root"),
        dir.path().join("work"),
    );
    for d in [&home, &root, &work, &home.join(".cargo/bin")] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(home.join(".boxignore"), ".cache/\n.cargo/\n.rustup/\n").unwrap();
    std::fs::write(home.join(".cargo/bin/cargo"), "").unwrap();
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(crate::sanitize::script(&plan))
        .current_dir(&work)
        .env("HOME", &home)
        .env("OA_ROOT", &root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("removed ~/.boxignore"), "{stdout}");
    assert!(!home.join(".boxignore").exists());
    assert!(home.join(".cargo/bin/cargo").exists());
}
