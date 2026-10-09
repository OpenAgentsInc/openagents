use crate::*;
use coder_environment::{
    ArtifactPin, Environment, ImagePin, Limits, Platform, ProjectLink, Provider, Qualification,
    Recipe, Script, SourcePin,
};
use coder_environment_build::Phase;
use coder_working_computer::Principal;
use coder_working_computer::provider::fake::{FakeProvider, FakeRun};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

const SCRIPT: &str = "./install.sh";

fn pin() -> SourcePin {
    SourcePin {
        repository: Some("example/repo".into()),
        revision: "a".repeat(40),
        digest: "b".repeat(64),
    }
}
fn recipe() -> Recipe {
    Recipe {
        schema: coder_environment::RECIPE_SCHEMA.into(),
        base: ImagePin {
            provider: Provider::Boat,
            image_id: "oa-coder-runtime-1".into(),
            digest: "c".repeat(64),
        },
        runtime: ArtifactPin {
            revision: "rt-1".into(),
            digest: "d".repeat(64),
        },
        platform: Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: Script {
            cwd: ".".into(),
            digest: digest(SCRIPT.as_bytes()),
        },
        start: Default::default(),
        inputs: Default::default(),
        credential_names: Default::default(),
        qualification: Qualification {
            profile: "rust-library".into(),
            plan_digest: "f".repeat(64),
        },
        limits: Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 4,
            output_bytes: 1 << 20,
        },
        capture: Default::default(),
    }
}

fn custody() -> Custody {
    Arc::new(|_: &BTreeSet<String>| Ok(Redactor::new()))
}
fn open(state: &Path, provider: &Arc<FakeProvider>) -> Owners<Arc<FakeProvider>> {
    Owners::open(
        state,
        Providers {
            setup: provider.clone(),
            build: provider.clone(),
            verify: provider.clone(),
        },
        custody(),
    )
    .unwrap()
}

#[test]
fn the_layout_is_the_operators_and_private() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("cloud-operator");
    let provider = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let owners = open(&state, &provider);
    let l = &owners.layout;
    // The panel reads verifier evidence exactly where the verifier writes
    // it, and admits jobs from the same environment records.
    assert_eq!(l.verify_evidence(), state.join(VERIFY_EVIDENCE));
    assert_eq!(
        owners.verifier.evidence_dir("v-1"),
        l.verify_evidence().join("v-1")
    );
    assert_eq!(l.environments(), state.join("environments"));
    #[cfg(unix)]
    for d in [
        l.environments(),
        l.sessions(),
        l.setup_blobs(),
        l.build_jobs(),
        l.verify_jobs(),
        l.verify_evidence(),
        l.artifacts(),
        l.computers(),
    ] {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&d).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", d.display());
    }
    // Reopening over the same state is the restart path.
    drop(owners);
    let again = open(&state, &provider);
    assert_eq!(again.layout, Layout::under(&state));
    // Artifacts are content-addressed and idempotent.
    let d = again.seal_artifact(b"plan").unwrap();
    assert_eq!(again.seal_artifact(b"plan").unwrap(), d);
    assert_eq!(
        fs::read(again.layout.artifacts().join(&d)).unwrap(),
        b"plan"
    );
}

#[test]
fn the_config_is_explicit_and_bounded() {
    let good = Config {
        schema: SCHEMA.into(),
        provider: ProviderKind::Boat,
        workdir: "/workspace/repo".into(),
        gce: None,
        template: None,
        credential_names: ["GH_TOKEN".to_string()].into(),
        tick_seconds: 15,
    };
    good.validate().unwrap();
    assert_eq!(good.cadence().tick, Duration::from_secs(15));
    let text = r#"{"schema":"openagents.environment.owners.v1","provider":"boat","workdir":"/w"}"#;
    let parsed: Config = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.tick_seconds, DEFAULT_TICK_SECONDS);
    assert!(parsed.credential_names.is_empty());
    for bad in [
        Config {
            schema: "other".into(),
            ..good.clone()
        },
        Config {
            workdir: "relative".into(),
            ..good.clone()
        },
        Config {
            tick_seconds: 0,
            ..good.clone()
        },
        Config {
            credential_names: ["CLAUDE_CODE_OAUTH_TOKEN".to_string()].into(),
            ..good.clone()
        },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    // The GCE adapter needs its own section, and only with its provider.
    let gce: Config = serde_json::from_str(
        r#"{"schema":"openagents.environment.owners.v1","provider":"gce","workdir":"/w"}"#,
    )
    .unwrap();
    assert!(gce.validate().is_err());
    let section = coder_working_computer::gce::GceConfig {
        project: "oa-test".into(),
        zone: "us-central1-a".into(),
        machine: "c3-standard-8".into(),
        disk_gb: 100,
        base: coder_working_computer::gce::GceImage {
            project: "oa-test".into(),
            name: "oa-coder-host-1".into(),
            id: "42".into(),
        },
    };
    let gce = Config {
        provider: ProviderKind::Gce,
        gce: Some(section.clone()),
        ..good.clone()
    };
    gce.validate().unwrap();
    let back: Config = serde_json::from_str(&serde_json::to_string(&gce).unwrap()).unwrap();
    assert_eq!(back, gce);
    for bad in [
        Config {
            gce: Some(section.clone()),
            ..good.clone()
        },
        Config {
            template: Some("oa-coder-runtime-1".into()),
            ..gce.clone()
        },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
}

#[tokio::test]
async fn a_restarted_owner_recovers_a_build_mid_install() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("cloud-operator");
    let provider = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let slow = Arc::new(AtomicBool::new(true));
    let plan = coder_environment_build::sanitize::Plan::new(
        &recipe().capture,
        "/tmp/oa-commands/sanitize",
    );
    let s = slow.clone();
    provider.on_command(Box::new(move |spec, _env, files| match spec.id.as_str() {
        "source" => {
            files.insert("src/main.rs".into(), "fn main() {}".into());
            let r = coder_environment_setup::source::Report::verified_for(&pin(), true);
            FakeRun::exit(0, &r.render(), "")
        }
        "install" => {
            files.insert("target/release/app".into(), "binary".into());
            let mut run = FakeRun::exit(0, "built\n", "");
            if s.load(Ordering::SeqCst) {
                run.exit = None;
            }
            run
        }
        "sanitize" => FakeRun::exit(0, &format!("sanitized {}\n", plan.digest()), ""),
        _ => FakeRun::exit(0, "", ""),
    }));
    let owners = open(&state, &provider);
    let env = Environment::new(
        "env-1",
        ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        pin(),
        recipe(),
        0,
    )
    .unwrap();
    owners.setup.environments.create(&env).unwrap();
    fs::write(
        owners.layout.setup_blobs().join(digest(SCRIPT.as_bytes())),
        SCRIPT,
    )
    .unwrap();
    let job = owners
        .build(
            &coder_environment_build::service::BuildRequest {
                request_id: "req-1".into(),
                environment: "env-1".into(),
                owner: Principal {
                    workspace: "ws-1".into(),
                    principal: "user-1".into(),
                },
                expected_draft_revision: 1,
                size: "small".into(),
            },
            1_000,
        )
        .await
        .unwrap();
    assert_eq!(job.phase, Phase::Installing);

    // The operator restarts while the install runs; the install finishes
    // while it is down.
    drop(owners);
    let owners = open(&state, &provider);
    let c = owners.builder.computers.store.read(&job.computer).unwrap();
    provider.finish_command(c.resource().unwrap(), "install", 0, "more\n");
    let r = owners.recover(2_000).await;
    assert_eq!(r.builds, vec![job.id.clone()]);
    assert!(r.errors.is_empty(), "{r:?}");
    let job = owners.builder.jobs.read(&job.id).unwrap();
    assert_eq!(job.phase, Phase::Ready, "{:?}", job.reason);
    assert!(matches!(
        job.cleanup,
        coder_environment_build::Cleanup::Complete { .. }
    ));
    // The restart is disclosed as a new evidence segment.
    assert_eq!(job.segments.len(), 2);
    // A settled build is not visited again.
    let r = owners.recover(3_000).await;
    assert!(r.builds.is_empty() && r.verifications.is_empty() && r.setup.is_empty());
}

#[path = "gce_tests.rs"]
mod gce_tests;
