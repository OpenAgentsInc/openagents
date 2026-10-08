use crate::plan::*;
use crate::service::{Verifier, VerifyError, VerifyRequest};
use crate::*;
use coder_environment::evidence::{EvidenceStatus, Redactor, load_manifest};
use coder_environment::{
    ArtifactPin, Command as EnvCommand, Environment, ImagePin, Inputs as RecipeInputs, Limits,
    Platform, ProjectLink, Provider, Qualification, Recipe, Script, SourcePin, Start,
    VerificationState, digest,
};
use coder_environment_build::sanitize::Plan as SanitizePlan;
use coder_environment_build::service::{BuildRequest, Builder};
use coder_environment_setup::source::Report as SourceReport;
use coder_working_computer::driver::Driver;
use coder_working_computer::provider::fake::{FakeProvider, FakeRun, Inject};
use coder_working_computer::{Health, Phase as ComputerPhase, Principal, Purpose, ServiceDecl};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const INSTALL: &str = "./install.sh";
const LOCK: &str = "version = 3\n";
const UNIT: &str = "cargo test -p app --locked";
const BROWSER: &str = "openagents browser run --flow smoke";

fn pin() -> SourcePin {
    SourcePin {
        repository: Some("example/repo".into()),
        revision: "a".repeat(40),
        digest: "b".repeat(64),
    }
}
fn check(name: &str, kind: CheckKind, script: &str, assertions: Assertions) -> Check {
    Check {
        name: name.into(),
        kind,
        script: digest(script.as_bytes()),
        cwd: ".".into(),
        timeout_seconds: 600,
        assertions,
    }
}
fn service_plan() -> CheckPlan {
    CheckPlan {
        schema: PLAN_SCHEMA.into(),
        profile: "rust-service".into(),
        source: SourceStep::Contained,
        offline: true,
        startup: Startup::Services {
            services: vec![ServiceDecl {
                name: "web".into(),
                command: "target/release/app serve".into(),
                cwd: ".".into(),
                health: Health::Http {
                    port: 8080,
                    path: "/health".into(),
                },
                ready_within_seconds: 30,
            }],
            readiness: vec![check(
                "browser",
                CheckKind::Browser,
                BROWSER,
                Assertions::Marker { min_passed: 1 },
            )],
        },
        checks: vec![check(
            "unit",
            CheckKind::Behavior,
            UNIT,
            Assertions::CargoTest { min_passed: 1 },
        )],
        idempotence: Idempotence {
            inventory: vec!["target".into(), "~/.cargo".into()],
        },
    }
}
fn plan_bytes(plan: &CheckPlan) -> Vec<u8> {
    serde_json::to_vec_pretty(plan).unwrap()
}
fn recipe(plan: &CheckPlan) -> Recipe {
    Recipe {
        schema: coder_environment::RECIPE_SCHEMA.into(),
        base: ImagePin {
            provider: Provider::Boat,
            image_id: "coder-base-2026-10-08".into(),
            digest: "a".repeat(64),
        },
        runtime: ArtifactPin {
            revision: "rt-1".into(),
            digest: "b".repeat(64),
        },
        platform: Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: Script {
            cwd: ".".into(),
            digest: digest(INSTALL.as_bytes()),
        },
        start: Start::default(),
        inputs: RecipeInputs {
            toolchain: None,
            locks: BTreeMap::from([("Cargo.lock".to_string(), digest(LOCK.as_bytes()))]),
        },
        credential_names: BTreeSet::new(),
        qualification: Qualification {
            profile: plan.profile.clone(),
            plan_digest: digest(&plan_bytes(plan)),
        },
        limits: Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 3,
            output_bytes: 1 << 24,
        },
        capture: Default::default(),
    }
}

/// Knobs a test turns to make the candidate misbehave.
#[derive(Default)]
struct Faults {
    failing_check: AtomicBool,
    empty_check: AtomicBool,
    mutating_install: AtomicBool,
}

struct Harness {
    _dir: tempfile::TempDir,
    verifier: Verifier<FakeProvider>,
    blobs: std::path::PathBuf,
    faults: Arc<Faults>,
    /// The builder machine the image was captured from.
    builder_resource: String,
    plan: CheckPlan,
}

fn inventory(files: &BTreeMap<String, String>, paths: &[&str]) -> String {
    let mut out = String::new();
    for p in paths {
        let under: Vec<_> = files
            .iter()
            .filter(|(k, _)| k.as_str() == *p || k.starts_with(&format!("{p}/")))
            .collect();
        if under.is_empty() {
            out.push_str(&format!("oa-inventory {p} absent\n"));
        } else {
            let d = digest(&serde_json::to_vec(&under).unwrap());
            out.push_str(&format!("oa-inventory {p} {d}\n"));
        }
    }
    out.push_str(&format!("oa-inventory git:head {}\n", "a".repeat(40)));
    out.push_str("oa-inventory done\n");
    out
}

/// Builds a real image with the ENV-04 builder, then hands its provider to
/// a verifier.
async fn harness_with(plan: CheckPlan) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let r = recipe(&plan);
    let env = Environment::new(
        "env-1",
        ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        pin(),
        r.clone(),
        0,
    )
    .unwrap();
    let environments = coder_environment::store::Store::under(root.join("environments"));
    environments.create(&env).unwrap();
    let blobs = root.join("blobs");
    std::fs::create_dir_all(&blobs).unwrap();
    for bytes in [
        INSTALL.as_bytes().to_vec(),
        UNIT.as_bytes().to_vec(),
        BROWSER.as_bytes().to_vec(),
        plan_bytes(&plan),
    ] {
        std::fs::write(blobs.join(digest(&bytes)), bytes).unwrap();
    }
    let provider = FakeProvider::new(BTreeMap::new(), true);
    let sanitize = SanitizePlan::new(&r.capture, "/tmp/oa-commands/sanitize");
    provider.on_command(Box::new(move |spec, _env, files| match spec.id.as_str() {
        "source" => {
            files.insert("Cargo.lock".into(), LOCK.into());
            files.insert("src/main.rs".into(), "fn main() {}".into());
            FakeRun::exit(0, &SourceReport::verified_for(&pin(), true).render(), "")
        }
        "install" => {
            files.insert("target/release/app".into(), "binary".into());
            files.insert("~/.cargo/registry/dep".into(), "vendored".into());
            FakeRun::exit(0, "built\n", "")
        }
        "sanitize" => FakeRun::exit(0, &format!("sanitized {}\n", sanitize.digest()), ""),
        _ => FakeRun::exit(0, "", ""),
    }));
    let computers = coder_working_computer::store::Store::under(root.join("computers"));
    let builder = Builder::new(
        root.join("build"),
        blobs.clone(),
        coder_environment_build::store::Store::under(root.join("builds")),
        environments,
        Driver::new(computers, provider),
        Box::new(|_: &BTreeSet<String>| Ok(Redactor::new())),
    );
    let job = builder
        .start(
            &BuildRequest {
                request_id: "build-req".into(),
                environment: "env-1".into(),
                owner: owner(),
                expected_draft_revision: 1,
                size: "small".into(),
            },
            1_000,
        )
        .await
        .unwrap();
    assert_eq!(
        job.phase,
        coder_environment_build::Phase::Ready,
        "{:?}",
        job.reason
    );
    let builder_resource = job.manifest().unwrap().builder;
    let Driver { store, provider } = builder.computers;
    let faults = Arc::new(Faults::default());
    let f = faults.clone();
    provider.on_command(Box::new(move |spec, env, files| {
        let id = spec.id.as_str();
        let offline = env.get("CARGO_NET_OFFLINE").map(String::as_str) == Some("true");
        if id.ends_with("-source") {
            // The image already holds the checkout.
            if !files.contains_key("src/main.rs") {
                return FakeRun::exit(3, "oa-source error=no-checkout\n", "");
            }
            FakeRun::exit(0, &SourceReport::verified_for(&pin(), false).render(), "")
        } else if id.ends_with("-locks") {
            let ok = files.get("Cargo.lock").map(String::as_str) == Some(LOCK);
            let line = if ok { "ok" } else { "mismatch" };
            FakeRun::exit(
                if ok { 0 } else { 3 },
                &format!("oa-lock {line} Cargo.lock\noa-lock done\n"),
                "",
            )
        } else if id.ends_with("-check-unit") {
            if !offline || !files.contains_key("target/release/app") {
                return FakeRun::exit(101, "", "error: no network\n");
            }
            if f.failing_check.load(Ordering::SeqCst) {
                return FakeRun::exit(
                    101,
                    "test result: FAILED. 4 passed; 1 failed; 0 ignored;\n",
                    "",
                );
            }
            if f.empty_check.load(Ordering::SeqCst) {
                return FakeRun::exit(0, "test result: ok. 0 passed; 0 failed; 0 ignored;\n", "");
            }
            FakeRun::exit(
                0,
                "running 5 tests\ntest result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n",
                "",
            )
        } else if id.ends_with("-check-browser") {
            FakeRun::exit(0, "OA-CHECK passed=2 failed=0\n", "")
        } else if id.ends_with("-inventory-before") || id.ends_with("-inventory-after") {
            FakeRun::exit(0, &inventory(files, &["target", "~/.cargo"]), "")
        } else if id.ends_with("-install") {
            if f.mutating_install.load(Ordering::SeqCst) {
                files.insert("target/release/app".into(), "rebuilt".into());
            }
            FakeRun::exit(0, "nothing to do\n", "")
        } else {
            FakeRun::exit(0, "", "")
        }
    }));
    let verifier = Verifier::new(
        root.join("verify"),
        blobs.clone(),
        store::Store::under(root.join("verify-jobs")),
        coder_environment::store::Store::under(root.join("environments")),
        coder_environment_build::store::Store::under(root.join("builds")),
        Driver::new(store, provider),
    );
    Harness {
        _dir: dir,
        verifier,
        blobs,
        faults,
        builder_resource,
        plan,
    }
}
async fn harness() -> Harness {
    harness_with(service_plan()).await
}
fn owner() -> Principal {
    Principal {
        workspace: "ws-1".into(),
        principal: "user-1".into(),
    }
}
fn request(h: &Harness, id: &str) -> VerifyRequest {
    VerifyRequest {
        request_id: id.into(),
        environment: "env-1".into(),
        build_id: "build-1".into(),
        owner: owner(),
        plan_digest: digest(&plan_bytes(&h.plan)),
        size: "small".into(),
    }
}
fn provider(h: &Harness) -> &FakeProvider {
    &h.verifier.computers.provider
}
fn count(h: &Harness, op: &str) -> usize {
    provider(h).calls().iter().filter(|c| *c == op).count()
}
fn attempt(h: &Harness, job: &VerifyJob) -> coder_environment::VerificationAttempt {
    h.verifier.view(&job.id).unwrap().attempt
}
fn failure(job: &VerifyJob) -> String {
    match &job.verdict {
        Some(Verdict::Failed { reason }) => reason.clone(),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[tokio::test]
async fn the_exact_image_passes_on_fresh_machines_without_repair() {
    let h = harness().await;
    let creates_before = count(&h, "create");
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert_eq!(job.phase, Phase::Done, "{:?}", job.verdict);
    assert_eq!(job.verdict, Some(Verdict::Passed));

    // Two fresh machines booted from the sealed image: not the builder,
    // not each other, carrying no credentials, restored from nothing.
    let image = job.inputs.image.image_id.clone();
    for (m, role) in [
        (&job.baseline, VerifyRole::Baseline),
        (&job.fork, VerifyRole::Fork),
    ] {
        let c = h.verifier.computers.store.read(&m.computer).unwrap();
        assert_eq!(
            c.purpose,
            Purpose::EnvironmentVerify {
                environment: "env-1".into(),
                build: "build-1".into(),
                verification: job.inputs.verification_id.clone(),
                image: image.clone(),
                role,
            }
        );
        assert!(c.credential_names.is_empty() && c.checkpoints.is_empty());
        assert_eq!(c.phase, ComputerPhase::Deleted);
        assert!(matches!(m.cleanup, Cleanup::Complete { .. }));
        assert!(m.hydrated && m.usage_uncertain.is_none());
        assert_ne!(m.resource.as_deref(), Some(h.builder_resource.as_str()));
    }
    assert_ne!(job.baseline.resource, job.fork.resource);
    assert_eq!(count(&h, "create") - creates_before, 2);
    assert_eq!(count(&h, "restore"), 0);
    // Nothing is ever captured from a verifier.
    assert_eq!(count(&h, "capture_image"), 1);

    // The untouched baseline never installs; the fork reran the exact
    // install script once, offline.
    let install = job
        .steps
        .iter()
        .find(|s| s.action == Action::Install)
        .unwrap();
    assert_eq!(install.role, VerifyRole::Fork);
    let spec = install.spec.as_ref().unwrap();
    assert_eq!(spec.command, INSTALL);
    assert!(spec.credential_names.is_empty());
    assert_eq!(spec.env["CARGO_NET_OFFLINE"], "true");
    assert!(
        job.steps
            .iter()
            .filter(|s| s.role == VerifyRole::Baseline)
            .all(|s| s.action != Action::Install)
    );
    // Services and readiness ran on both machines; behavior on the
    // baseline.
    let ids: Vec<&str> = job.steps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "b01-source",
            "b02-locks",
            "b03-svc-web",
            "b04-check-browser",
            "b05-check-unit",
            "f01-inventory-before",
            "f02-install",
            "f03-svc-web",
            "f04-check-browser",
            "f05-inventory-after",
        ]
    );
    let unit = job.step("b05-check-unit").unwrap();
    assert_eq!(unit.tally.passed, 5);

    // The verdict and its complete evidence are on the attempt, and a
    // version can now be saved from it.
    let a = attempt(&h, &job);
    assert_eq!(a.state, VerificationState::Passed);
    let sealed = job.evidence.clone().unwrap();
    assert_eq!(a.evidence_digest.as_deref(), Some(sealed.digest.as_str()));
    assert_eq!(a.evidence_status, Some(EvidenceStatus::Complete));
    assert_eq!(
        a.run.as_ref().unwrap().task.as_deref(),
        Some(job.id.as_str())
    );
    let manifest = load_manifest(&h.verifier.evidence_dir(&job.id), &sealed.digest).unwrap();
    assert_eq!(manifest.status, EvidenceStatus::Complete);
    let children: Vec<_> = manifest
        .children
        .iter()
        .map(|c| (c.evidence_id.as_str(), c.status))
        .collect();
    assert_eq!(
        children,
        vec![
            ("baseline", EvidenceStatus::Complete),
            ("fork", EvidenceStatus::Complete)
        ]
    );
    let tools: Vec<&str> = manifest
        .calls
        .iter()
        .map(|c| c.identity.tool.as_str())
        .collect();
    assert_eq!(
        tools,
        vec![
            "environment.verify.identity",
            "environment.verify.machine",
            "environment.verify.machine",
            "environment.verify.cleanup",
        ]
    );
    let baseline = load_manifest(
        &h.verifier.evidence_dir(&job.id).join("children/baseline"),
        &manifest.children[0].manifest_digest,
    )
    .unwrap();
    assert_eq!(baseline.calls.len(), 6, "hydration plus five steps");
    let saved = h
        .verifier
        .environments
        .apply(
            "env-1",
            &EnvCommand::SaveVersion {
                request_id: "save-1".into(),
                verification_id: a.id.clone(),
                expected_draft_revision: 1,
            },
            20_000,
        )
        .unwrap();
    assert!(matches!(
        saved,
        coder_environment::Applied::Changed(_, coder_environment::Effect::VersionSaved { .. })
    ));

    // A repeated request is the same job.
    let again = h.verifier.start(&request(&h, "v-1"), 30_000).await.unwrap();
    assert_eq!(again, job);
}

#[tokio::test]
async fn a_failed_check_fails_on_the_baseline_and_never_forks() {
    let h = harness().await;
    h.faults.failing_check.store(true, Ordering::SeqCst);
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert_eq!(job.phase, Phase::Done);
    assert!(failure(&job).contains("b05-check-unit"), "{job:?}");
    // The fork never ran, and its machine was never allocated.
    let fork = h.verifier.computers.store.read(&job.fork.computer).unwrap();
    assert!(fork.creates.is_empty());
    assert!(
        job.steps
            .iter()
            .filter(|s| s.role == VerifyRole::Fork)
            .all(|s| s.run == Run::NotStarted)
    );
    assert!(matches!(job.baseline.cleanup, Cleanup::Complete { .. }));
    let a = attempt(&h, &job);
    assert_eq!(a.state, VerificationState::Failed);
    assert!(a.evidence_status.is_some());
    // A failed verification never saves.
    assert!(
        h.verifier
            .environments
            .apply(
                "env-1",
                &EnvCommand::SaveVersion {
                    request_id: "save-1".into(),
                    verification_id: a.id.clone(),
                    expected_draft_revision: 1,
                },
                20_000,
            )
            .is_err()
    );
}

#[tokio::test]
async fn an_empty_or_missing_check_fails() {
    // Exit 0 with no passing assertion is not a pass.
    let h = harness().await;
    h.faults.empty_check.store(true, Ordering::SeqCst);
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert!(failure(&job).contains("assertions"), "{job:?}");

    // A check whose protected script is gone fails; nothing repairs it.
    let h = harness().await;
    std::fs::remove_file(h.blobs.join(digest(UNIT.as_bytes()))).unwrap();
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    let why = failure(&job);
    assert!(why.contains("missing or altered"), "{why}");
    assert!(job.step("b05-check-unit").unwrap().spec.is_none());

    // A plan without a behavior check cannot pass.
    let mut empty = service_plan();
    empty.checks.clear();
    let h = harness_with(empty).await;
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert!(failure(&job).contains("no behavior check"), "{job:?}");
}

#[tokio::test]
async fn a_rerun_that_changes_the_image_fails_idempotence() {
    let h = harness().await;
    h.faults.mutating_install.store(true, Ordering::SeqCst);
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    let why = failure(&job);
    assert!(why.contains("changed") && why.contains("target"), "{why}");
    // The baseline passed untouched before the fork ran.
    assert!(
        job.steps
            .iter()
            .filter(|s| s.role == VerifyRole::Baseline)
            .all(|s| matches!(s.outcome, Some(StepOutcome::Passed { .. })))
    );
    // The candidate image itself is unchanged.
    let files = provider(&h)
        .image_files(&job.inputs.image.image_id)
        .unwrap();
    assert_eq!(files["target/release/app"], "binary");
    assert!(matches!(job.fork.cleanup, Cleanup::Complete { .. }));
}

#[tokio::test]
async fn restore_waits_for_hydration_and_a_changed_plan_invalidates_the_run() {
    let h = harness().await;
    provider(&h).state.lock().unwrap().hydration_pending = true;
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert_eq!(job.phase, Phase::Baseline);
    assert!(!job.baseline.hydrated);
    assert!(job.steps.iter().all(|s| s.run == Run::NotStarted));
    assert_eq!(attempt(&h, &job).state, VerificationState::Restoring);
    // Still copying: nothing runs.
    let job = h.verifier.advance(&job.id, 11_000).await.unwrap();
    assert_eq!(count(&h, "command_start"), 3, "only the build's commands");
    assert!(!job.baseline.hydrated);

    // A recipe revision (a new plan) lands mid-run.
    let mut next = recipe(&h.plan);
    next.qualification.plan_digest = "f".repeat(64);
    h.verifier
        .environments
        .apply(
            "env-1",
            &EnvCommand::UpdateRecipe {
                expected_draft_revision: 1,
                recipe: next,
            },
            11_500,
        )
        .unwrap();
    provider(&h).settle_hydration();
    let job = h.verifier.advance(&job.id, 12_000).await.unwrap();
    assert_eq!(job.phase, Phase::Done);
    assert!(
        matches!(&job.verdict, Some(Verdict::Cancelled { reason }) if reason.contains("plan")),
        "{:?}",
        job.verdict
    );
    assert_eq!(attempt(&h, &job).state, VerificationState::Cancelled);
    assert!(matches!(job.baseline.cleanup, Cleanup::Complete { .. }));
}

#[tokio::test]
async fn a_hydrated_restore_then_passes() {
    let h = harness().await;
    provider(&h).state.lock().unwrap().hydration_pending = true;
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert_eq!(job.phase, Phase::Baseline);
    provider(&h).settle_hydration();
    // The fork boots un-hydrated too; settle it on the next visit.
    let job = h.verifier.advance(&job.id, 11_000).await.unwrap();
    assert_eq!(job.phase, Phase::Fork);
    provider(&h).settle_hydration();
    let job = h.verifier.advance(&job.id, 12_000).await.unwrap();
    assert_eq!(job.verdict, Some(Verdict::Passed));
}

#[tokio::test]
async fn unknown_cleanup_holds_the_verdict_and_blocks_new_verifiers() {
    let h = harness().await;
    provider(&h).inject("delete", Inject::Unknown);
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert_eq!(job.phase, Phase::Cleanup);
    assert_eq!(job.verdict, Some(Verdict::Passed));
    assert!(job.cleanup_uncertain(), "{:?}", job.baseline.cleanup);
    assert!(job.evidence.is_none());
    assert_eq!(
        attempt(&h, &job).state,
        VerificationState::NeedsReconciliation
    );
    let refused = h.verifier.start(&request(&h, "v-2"), 11_000).await;
    assert!(
        matches!(refused, Err(VerifyError::Refused(m)) if m.contains("cleanup")),
        "{refused:?}"
    );
    // Reconciled: the delete is confirmed, then the verdict is recorded.
    let job = h.verifier.advance(&job.id, 12_000).await.unwrap();
    assert_eq!(job.phase, Phase::Done);
    assert!(!job.cleanup_uncertain());
    assert_eq!(attempt(&h, &job).state, VerificationState::Passed);
}

#[tokio::test]
async fn a_lost_start_reply_is_reconciled_by_identity() {
    let h = harness().await;
    provider(&h).inject("command_start", Inject::LostReply);
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert!(job.unresolved.is_some());
    assert_eq!(
        attempt(&h, &job).state,
        VerificationState::NeedsReconciliation
    );
    let resource = job.baseline.resource.clone().unwrap();
    let job = h.verifier.advance(&job.id, 11_000).await.unwrap();
    assert_eq!(job.verdict, Some(Verdict::Passed));
    // The source command ran exactly once (the fake keeps processes until
    // deletion, so count starts instead).
    let starts = count(&h, "command_start");
    assert_eq!(
        starts,
        3 + 10 - 2,
        "build commands plus each command step once"
    );
    let _ = resource;
}

#[tokio::test]
async fn a_restart_ends_incomplete_and_still_cleans_up() {
    let h = harness().await;
    provider(&h).state.lock().unwrap().hydration_pending = true;
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    h.verifier.restart();
    provider(&h).settle_hydration();
    let job = h.verifier.advance(&job.id, 11_000).await.unwrap();
    assert_eq!(job.phase, Phase::Done);
    assert!(job.evidence_lost && job.evidence.is_none());
    assert!(matches!(job.verdict, Some(Verdict::Incomplete { .. })));
    assert!(matches!(job.baseline.cleanup, Cleanup::Complete { .. }));
    assert_eq!(attempt(&h, &job).state, VerificationState::Incomplete);
}

#[tokio::test]
async fn a_service_that_never_becomes_ready_fails() {
    let h = harness().await;
    provider(&h)
        .state
        .lock()
        .unwrap()
        .broken_services
        .insert("web".into());
    let job = h.verifier.start(&request(&h, "v-1"), 10_000).await.unwrap();
    assert!(failure(&job).contains("b03-svc-web"), "{job:?}");
    assert!(job.step("b04-check-browser").unwrap().spec.is_none());
}

#[tokio::test]
async fn admission_refuses_a_missing_plan_a_wrong_plan_and_a_drifted_image() {
    let h = harness().await;
    let mut r = request(&h, "v-1");
    r.plan_digest = "e".repeat(64);
    assert!(matches!(
        h.verifier.start(&r, 10_000).await,
        Err(VerifyError::Refused(m)) if m.contains("plan")
    ));
    // A plan blob that exists but is not the recipe's frozen plan.
    let mut other = service_plan();
    other.offline = false;
    let bytes = plan_bytes(&other);
    std::fs::write(h.blobs.join(digest(&bytes)), &bytes).unwrap();
    r.plan_digest = digest(&bytes);
    assert!(matches!(
        h.verifier.start(&r, 10_000).await,
        Err(VerifyError::Environment(_))
    ));
    // The provider's name points at another snapshot.
    let image = {
        let env = h.verifier.environments.read("env-1").unwrap();
        env.build("build-1").unwrap().image.clone().unwrap()
    };
    provider(&h)
        .state
        .lock()
        .unwrap()
        .images
        .get_mut(&image.image_id)
        .unwrap()
        .0
        .snapshot = Some("other-snap".into());
    assert!(matches!(
        h.verifier.start(&request(&h, "v-2"), 10_000).await,
        Err(VerifyError::Refused(m)) if m.contains("sealed snapshot")
    ));
    assert!(
        h.verifier
            .environments
            .read("env-1")
            .unwrap()
            .verifications
            .is_empty()
    );
}

#[test]
fn tallies_require_real_assertions() {
    let cargo = Assertions::CargoTest { min_passed: 1 };
    let mut t = Tally::default();
    t.feed(cargo, b"test result: ok. 3 pas");
    t.feed(
        cargo,
        b"sed; 0 failed; 0 ignored\ntest result: ok. 2 passed; 0 failed;",
    );
    t.finish(cargo);
    assert_eq!((t.passed, t.failed, t.results), (5, 0, 2));
    assert!(t.verdict(cargo).is_ok());
    let mut empty = Tally::default();
    empty.feed(
        cargo,
        b"running 0 tests\ntest result: ok. 0 passed; 0 failed;\n",
    );
    assert!(empty.verdict(cargo).is_err());
    assert!(Tally::default().verdict(cargo).is_err());
    let marker = Assertions::Marker { min_passed: 2 };
    let mut m = Tally::default();
    m.feed(marker, b"OA-CHECK passed=1 failed=0\n");
    assert!(m.verdict(marker).is_err(), "below the minimum");
    m.feed(marker, b"OA-CHECK passed=1 failed=0\nOA-CHECK garbage\n");
    assert!(
        m.verdict(marker).is_err(),
        "a malformed marker never passes"
    );
}

#[test]
fn plans_are_validated() {
    let mut p = service_plan();
    assert!(p.validate().is_ok());
    p.checks[0].assertions = Assertions::CargoTest { min_passed: 0 };
    assert!(p.validate().is_err());
    let mut p = service_plan();
    p.idempotence.inventory = vec!["../escape".into()];
    assert!(p.validate().is_err());
    let mut p = service_plan();
    p.startup = Startup::NotApplicable {
        reason: "A library has no services.".into(),
    };
    assert!(p.validate().is_ok());
    let steps = steps(&p, false);
    assert!(
        steps
            .iter()
            .all(|s| !matches!(s.action, Action::Service { .. }))
    );
    assert_eq!(
        steps.last().unwrap().action,
        Action::Inventory { after: true }
    );
}
