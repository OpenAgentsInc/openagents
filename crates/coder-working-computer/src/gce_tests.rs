//! The GCE adapter over the in-memory GCE ([`fake`]). Scripts run under
//! local `sh` in scratch directories; nothing calls GCE.

use super::fake::{FakeCompute, paths};
use super::*;
use crate::provider::{CommandProgress, Provider};
use crate::{Bounds, Custody, Fact, Principal, Spec, VerifyRole};

const SECRET: &str = "ghp_gce_builder_secret_0123456789";
const PROJECT: &str = "oa-test-project";

fn base() -> GceImage {
    GceImage {
        project: "oa-test-images".into(),
        name: "oa-coder-host-20261001".into(),
        id: "1234567890123".into(),
    }
}
fn config() -> GceConfig {
    GceConfig {
        project: PROJECT.into(),
        zone: "us-central1-a".into(),
        machine: "c3-standard-8".into(),
        disk_gb: 50,
        base: base(),
    }
}
struct H {
    _dir: tempfile::TempDir,
    p: GceProvider<FakeCompute>,
}
impl H {
    fn fake(&self) -> &FakeCompute {
        &self.p.compute
    }
}
fn harness() -> H {
    let dir = tempfile::tempdir().unwrap();
    let compute = FakeCompute::new(dir.path().to_path_buf(), PROJECT, &base());
    let credentials = Credentials::from_names(&["GH_TOKEN".into(), "NPM_TOKEN".into()], |n| {
        Some(match n {
            "GH_TOKEN" => SECRET.into(),
            _ => "npm-secret-value-0000".into(),
        })
    })
    .unwrap();
    H {
        _dir: dir,
        p: GceProvider {
            compute,
            config: config(),
            credentials,
            paths: paths(),
            ready_attempts: 1,
            ready_pause: Duration::ZERO,
        },
    }
}
fn spec(id: &str, credentials: &[&str]) -> Spec {
    Spec {
        id: id.into(),
        owner: Principal {
            workspace: "ws-1".into(),
            principal: "user-1".into(),
        },
        chat: "job-1".into(),
        project: coder_environment::ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        source: coder_environment::SourcePin {
            repository: Some("example/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        },
        base: None,
        size: "c3-standard-8".into(),
        credential_names: credentials.iter().map(|s| s.to_string()).collect(),
        services: vec![],
        bounds: Bounds {
            idle_ms: 3_600_000,
            observed_extension_ms: 0,
            absolute_ms: 3_600_000,
        },
    }
}
fn builder() -> Computer {
    Computer::for_build(
        spec("builder-1", &["GH_TOKEN", "NPM_TOKEN"]),
        "env-1",
        "build-1",
        0,
    )
    .unwrap()
}
fn verifier(image: &str) -> Computer {
    Computer::for_verify(
        spec("verifier-1", &[]),
        Purpose::EnvironmentVerify {
            environment: "env-1".into(),
            build: "build-1".into(),
            verification: "verify-1".into(),
            image: image.into(),
            role: VerifyRole::Baseline,
        },
        0,
    )
    .unwrap()
}
fn done<T: std::fmt::Debug>(o: Outcome<T>) -> T {
    match o {
        Outcome::Done { value } => value,
        other => panic!("expected done, got {other:?}"),
    }
}
fn command(id: &str, text: &str, credentials: &[&str]) -> CommandSpec {
    CommandSpec {
        id: id.into(),
        command: text.into(),
        cwd: ".".into(),
        credential_names: credentials.iter().map(|s| s.to_string()).collect(),
        env: BTreeMap::new(),
        timeout_seconds: 60,
        digest: digest(text.as_bytes()),
    }
}
async fn finish(p: &GceProvider<FakeCompute>, c: &Computer, r: &str, id: &str) -> CommandRead {
    for _ in 0..200 {
        let read = done(
            p.read_command(c, r, id, CommandCursor::default(), 1 << 20)
                .await,
        );
        if matches!(read.progress, CommandProgress::Exited { .. }) {
            return read;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the command did not finish");
}

#[test]
fn instance_arguments_pin_the_image_and_never_join_the_pool() {
    let request = CreateInstance {
        name: instance_name("op-1"),
        image: base(),
        labels: BTreeMap::from([(MANAGED_KEY.into(), MANAGED_VALUE.into())]),
        max_run_seconds: 3600,
    };
    let args = create_instance_args(&config(), &request, "ssh-ed25519 AAAA test");
    let joined = args.join(" ");
    assert!(joined.contains("--image oa-coder-host-20261001 --image-project oa-test-images"));
    assert!(!joined.contains("--image-family"));
    assert!(joined.contains("--no-service-account") && joined.contains("--no-scopes"));
    assert!(joined.contains("--labels=openagents-managed=coder-environment"));
    assert!(!joined.contains(POOL_VALUE) && !joined.contains("openagents-pool"));
    assert!(joined.contains("--no-address"));
    let image = create_image_args(
        &config(),
        &CreateImage {
            name: "oaenv-build-1-abc".into(),
            source_disk: "oaenv-x".into(),
            description: "oaenv-build-1-abc".into(),
            labels: BTreeMap::new(),
        },
    );
    assert!(image.contains(&"--source-disk=oaenv-x".to_string()));
    assert!(gce_name(&instance_name("anything")));
    assert_eq!(image_resource("oaenv-build-1-abc"), "oaenv-build-1-abc");
    assert!(gce_name(&image_resource("Build_1.UPPER")));
    assert!(config().validate().is_ok());
    let mut bad = config();
    bad.base.id = "latest".into();
    assert!(bad.validate().is_err());
    assert!(definite_refusal("ERROR: QUOTA exceeded"));
    assert!(!definite_refusal("Connection reset by peer"));
}

#[tokio::test]
async fn admits_only_the_pinned_base() {
    let h = harness();
    assert_eq!(h.p.kind(), ProviderKind::Gce);
    assert!(h.p.admits_base(&base().pin()).is_ok());
    let mut other = base();
    other.id = "999".into();
    assert!(h.p.admits_base(&other.pin()).is_err());
    let mut boat = base().pin();
    boat.provider = ProviderKind::Boat;
    assert!(h.p.admits_base(&boat).is_err());
}

#[tokio::test]
async fn a_chat_computer_is_refused() {
    let h = harness();
    let mut c = builder();
    c.purpose = Purpose::Chat;
    assert!(matches!(h.p.create(&c, "op").await, Outcome::Failed { .. }));
    assert!(h.fake().calls().iter().all(|c| c != "create_instance"));
}

#[tokio::test(flavor = "multi_thread")]
async fn builder_runs_identified_commands_and_seals_an_image_without_credentials() {
    let h = harness();
    let c = builder();
    let r = done(h.p.create(&c, "create-builder-1").await);
    let inst = h.fake().instance(&r).unwrap();
    assert_eq!(
        inst.disk.source_image_id.as_deref(),
        Some(base().id.as_str())
    );
    assert_eq!(inst.view.labels["openagents-managed"], "coder-environment");
    assert_eq!(inst.view.labels["oa-env-purpose"], "build");
    // A lost create reply finds the same instance.
    assert_eq!(done(h.p.create(&c, "create-builder-1").await), r);
    assert_eq!(h.fake().state.lock().unwrap().creates.len(), 1);

    done(h.p.apply_credentials(&c, &r).await);
    let install = command(
        "install",
        "echo \"gh=${#GH_TOKEN} npm=${NPM_TOKEN:-none}\"; echo built > out.txt; echo warn >&2",
        &["GH_TOKEN"],
    );
    done(h.p.start_command(&c, &r, &install).await);
    let read = finish(&h.p, &c, &r, "install").await;
    assert_eq!(read.progress, CommandProgress::Exited { code: 0 });
    assert_eq!(read.digest.as_deref(), Some(install.digest.as_str()));
    let out = String::from_utf8(read.stdout).unwrap();
    assert!(out.contains(&format!("gh={}", SECRET.len())), "{out}");
    // A credential the command did not name is removed.
    assert!(out.contains("npm=none"), "{out}");
    assert_eq!(read.stderr, b"warn\n");
    // At most once: a repeated start does not run it again.
    done(h.p.start_command(&c, &r, &install).await);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let again = finish(&h.p, &c, &r, "install").await;
    assert_eq!(
        String::from_utf8(again.stdout)
            .unwrap()
            .matches("gh=")
            .count(),
        1
    );
    // An unknown identity reads as absent.
    let absent = done(
        h.p.read_command(&c, &r, "never", CommandCursor::default(), 64)
            .await,
    );
    assert_eq!(absent.progress, CommandProgress::Absent);

    // Capture needs a stopped builder.
    assert!(matches!(
        h.p.capture_image(&c, &r, "oaenv-build-1-abc").await,
        Outcome::Failed { .. }
    ));
    let stop = done(h.p.stop(&c, &r).await);
    assert!(stop.starts_with("stop:gce-disk:"));
    let image = done(h.p.capture_image(&c, &r, "oaenv-build-1-abc").await);
    assert_eq!(image.state, ImageState::Ready);
    assert_eq!(image.source, r);
    let id = image.snapshot.clone().unwrap();
    assert!(id.bytes().all(|b| b.is_ascii_digit()));
    // Capturing again returns the same image; another builder may not
    // take the name.
    assert_eq!(
        done(h.p.capture_image(&c, &r, "oaenv-build-1-abc").await),
        image
    );
    let other = Computer::for_build(spec("builder-2", &[]), "env-1", "build-2", 0).unwrap();
    let r2 = done(h.p.create(&other, "create-builder-2").await);
    done(h.p.stop(&other, &r2).await);
    assert!(matches!(
        h.p.capture_image(&other, &r2, "oaenv-build-1-abc").await,
        Outcome::Failed { .. }
    ));
    // The image holds the build output and no credential file.
    let files = h.fake().state.lock().unwrap().images
        [&(PROJECT.to_string(), "oaenv-build-1-abc".to_string())]
        .1
        .clone()
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(files.join("work/out.txt")).unwrap(),
        "built\n"
    );
    assert!(!files.join("shm").exists());
    let mut stack = vec![files.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                assert!(
                    !text.contains(SECRET),
                    "{} holds the credential",
                    p.display()
                );
            }
        }
    }
    // A setup computer's disk never becomes an image.
    let setup = Computer::for_setup(spec("setup-1", &[]), "env-1", 0).unwrap();
    assert!(matches!(
        h.p.capture_image(&setup, &r, "oaenv-other").await,
        Outcome::Failed { .. }
    ));

    // A fresh verifier boots exactly that image.
    let v = verifier("oaenv-build-1-abc");
    let vr = done(h.p.create(&v, "create-verifier-1").await);
    assert_ne!(vr, r);
    let vi = h.fake().instance(&vr).unwrap();
    assert_eq!(vi.disk.source_image_id.as_deref(), Some(id.as_str()));
    assert_eq!(vi.view.labels["oa-env-purpose"], "verify");
    assert!(done(h.p.hydration(&v, &vr).await));
    assert_eq!(
        std::fs::read_to_string(vi.dir.join("work/out.txt")).unwrap(),
        "built\n"
    );
    // The verifier carries no credentials and inherits no records.
    done(h.p.apply_credentials(&v, &vr).await);
    assert!(!vi.dir.join("shm").exists());
    let check = command("check", "cat out.txt; echo ${GH_TOKEN:-none}", &[]);
    done(h.p.start_command(&v, &vr, &check).await);
    let read = finish(&h.p, &v, &vr, "check").await;
    assert_eq!(String::from_utf8(read.stdout).unwrap(), "built\nnone\n");

    // An unanswering guest is not ready; a replaced image is refused.
    h.fake().set_unreachable(true);
    assert!(!done(h.p.hydration(&v, &vr).await));
    h.fake().set_unreachable(false);
    h.fake().recreate_image(PROJECT, "oaenv-build-1-abc");
    assert!(matches!(
        h.p.hydration(&v, &vr).await,
        Outcome::Failed { .. }
    ));

    // Delete: gone, confirmed, and repeatable.
    let deleted = done(h.p.delete(&v, &vr).await);
    assert!(deleted.starts_with("deleted:"));
    assert_eq!(done(h.p.delete(&v, &vr).await), "already gone");
    assert_eq!(done(h.p.inspect(&v, &vr).await).running, None);
    assert!(!done(h.p.meter(&v, &vr).await).running);
}

#[tokio::test]
async fn a_drifted_base_or_missing_image_never_boots() {
    let h = harness();
    h.fake().recreate_image(&base().project, &base().name);
    assert!(matches!(
        h.p.create(&builder(), "op-1").await,
        Outcome::Failed { .. }
    ));
    let h = harness();
    assert!(matches!(
        h.p.create(&verifier("oaenv-missing"), "op-2").await,
        Outcome::Failed { .. }
    ));
    assert!(h.fake().state.lock().unwrap().creates.is_empty());
}

#[tokio::test]
async fn lost_replies_reconcile_by_identity() {
    let h = harness();
    let c = builder();
    h.fake().lose_reply("create_instance");
    assert!(matches!(
        h.p.create(&c, "op").await,
        Outcome::Unknown { .. }
    ));
    let r = done(h.p.create(&c, "op").await);
    assert_eq!(h.fake().state.lock().unwrap().creates.len(), 1);
    // An indefinite transport error is unknown, a refusal is failed.
    h.fake().fail_next("stop_instance", false);
    assert!(matches!(h.p.stop(&c, &r).await, Outcome::Unknown { .. }));
    h.fake().fail_next("stop_instance", true);
    assert!(matches!(h.p.stop(&c, &r).await, Outcome::Failed { .. }));
    done(h.p.stop(&c, &r).await);
    h.fake().lose_reply("create_image");
    assert!(matches!(
        h.p.capture_image(&c, &r, "oaenv-img").await,
        Outcome::Unknown { .. }
    ));
    // The image exists; the next capture reads it instead of creating.
    let image = done(h.p.capture_image(&c, &r, "oaenv-img").await);
    assert_eq!(image.state, ImageState::Ready);
    assert_eq!(
        h.fake()
            .calls()
            .iter()
            .filter(|c| *c == "create_image")
            .count(),
        1
    );
}

#[tokio::test]
async fn a_checkpoint_restores_only_the_disk_it_recorded() {
    let h = harness();
    let c = Computer::for_setup(spec("setup-1", &[]), "env-1", 0).unwrap();
    let r = done(h.p.create(&c, "setup-op").await);
    let evidence = done(h.p.checkpoint(&c, &r, 1).await);
    assert!(evidence.stopped.is_some());
    let inspected = done(h.p.inspect(&c, &r).await);
    assert_eq!(inspected.running, Some(false));
    assert_eq!(
        inspected.latest_snapshot.as_deref(),
        Some(evidence.snapshot.as_str())
    );
    let k = Checkpoint {
        id: "k1".into(),
        turn_generation: 1,
        boot: 1,
        resource: r.clone(),
        fact: Fact::Done {
            evidence: evidence.snapshot.clone(),
            at_ms: 1,
        },
        custody: Custody::UserPrivate {
            principal: c.owner.clone(),
            computer: c.id.clone(),
        },
        may_hold_user_logins: false,
    };
    done(h.p.restore(&c, &r, Some(&k)).await);
    assert!(done(h.p.meter(&c, &r).await).running);
    // Booted and stopped again outside the checkpoint: refused.
    done(h.p.stop(&c, &r).await);
    assert!(matches!(
        h.p.restore(&c, &r, Some(&k)).await,
        Outcome::Failed { .. }
    ));
}

#[tokio::test]
async fn reconcile_lists_usage_and_sweeps_only_owned_orphans() {
    let h = harness();
    let kept = done(h.p.create(&builder(), "kept").await);
    let orphan = done(
        h.p.create(
            &Computer::for_setup(spec("setup-9", &[]), "env-1", 0).unwrap(),
            "orphan",
        )
        .await,
    );
    h.fake().add_foreign_instance("oa-pool-p1-abcd", POOL_VALUE);
    done(h.p.stop(&builder(), &kept).await);
    let image = done(h.p.capture_image(&builder(), &kept, "oaenv-kept").await);
    let retained = Retained {
        instances: [kept.clone()].into(),
        images: BTreeSet::new(),
    };
    let report = h.p.reconcile(|| Ok(retained)).await.unwrap();
    assert_eq!(report.instances.len(), 2, "the pool host is not listed");
    assert_eq!(report.orphans(), vec![orphan.as_str()]);
    assert_eq!(report.unreferenced_images(), vec!["oaenv-kept"]);
    assert_eq!(report.disk_gb, 100);
    assert_eq!(report.image_bytes, 1_000_000);
    let swept = h.p.sweep(&report).await;
    assert_eq!(swept.len(), 1);
    assert!(matches!(swept[0].1, Outcome::Done { .. }));
    assert!(h.fake().instance(&orphan).is_none());
    assert!(h.fake().instance(&kept).is_some());
    assert!(h.fake().instance("oa-pool-p1-abcd").is_some());
    // The pool host is never stopped or deleted through this adapter.
    assert!(matches!(
        h.p.delete(&builder(), "oa-pool-p1-abcd").await,
        Outcome::Failed { .. }
    ));
    // Retire only the expected image.
    assert!(matches!(
        h.p.retire_image("oaenv-kept", "1").await,
        Outcome::Failed { .. }
    ));
    let id = image.snapshot.unwrap();
    assert!(done(h.p.retire_image("oaenv-kept", &id).await).starts_with("deleted:"));
    assert!(done(h.p.retire_image("oaenv-kept", &id).await).starts_with("absent:"));
}
