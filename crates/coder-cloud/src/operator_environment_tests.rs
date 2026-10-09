//! ENV-06: new operator jobs pin the project's selected environment version.

use super::*;
use coder_environment::{
    self as env, Applied, BuildState, Command, Review,
    store::Store as EnvStore,
    transition::{BuildObservation as B, VerificationObservation as V},
};

fn d(c: char) -> String {
    c.to_string().repeat(64)
}
fn recipe(install: char) -> env::Recipe {
    env::Recipe {
        schema: env::RECIPE_SCHEMA.into(),
        base: env::ImagePin {
            provider: env::Provider::Boat,
            image_id: "oa-coder-runtime-20261008".into(),
            digest: d('a'),
        },
        runtime: env::ArtifactPin {
            revision: "rt-1".into(),
            digest: d('b'),
        },
        platform: env::Platform {
            os: "linux".into(),
            architecture: "x86_64".into(),
        },
        install: env::Script {
            cwd: ".".into(),
            digest: d(install),
        },
        start: Default::default(),
        inputs: Default::default(),
        credential_names: Default::default(),
        qualification: env::Qualification {
            profile: "rust-library".into(),
            plan_digest: d('e'),
        },
        limits: env::Limits {
            deadline_seconds: 3600,
            concurrent_machines: 2,
            total_machine_allocations: 4,
            output_bytes: 1 << 28,
        },
        capture: Default::default(),
    }
}
fn environments(f: &Fixture) -> EnvStore {
    EnvStore::under(f.owner.0.root.join("environments"))
}
fn run(job: &str) -> env::RunLink {
    env::RunLink {
        cloud_job: job.into(),
        task: None,
    }
}
#[track_caller]
fn apply(s: &EnvStore, c: Command) {
    // The store's lease waits out a lock briefly inherited by a forked child.
    let applied = s.apply("env-1", &c, 10).unwrap();
    assert!(matches!(applied, Applied::Changed(..)));
}
/// Build, verify, and promote recipe revision `n` as version `vN`.
fn promote(s: &EnvStore, n: u64) {
    if n == 1 {
        let e = env::Environment::new(
            "env-1",
            env::ProjectLink {
                workspace: "checkout".into(),
                project: "synthetic".into(),
            },
            env::SourcePin {
                repository: None,
                revision: "0".repeat(40),
                digest: d('f'),
            },
            recipe('1'),
            1,
        )
        .unwrap();
        s.create(&e).unwrap();
    } else {
        apply(
            s,
            Command::UpdateRecipe {
                expected_draft_revision: n - 1,
                recipe: recipe(char::from_digit(n as u32, 10).unwrap()),
            },
        );
    }
    let build = format!("build-{n}");
    let verify = format!("verify-{n}");
    apply(
        s,
        Command::StartBuild {
            request_id: format!("rb{n}"),
            expected_draft_revision: n,
        },
    );
    for o in [
        B::Linked {
            run: run(&format!("job-b{n}")),
        },
        B::Progress {
            state: BuildState::Installing,
        },
        B::ImageReady {
            image: env::ImageIdentity {
                provider: env::Provider::Boat,
                image_id: format!("oaenv-build-{n}"),
                snapshot_id: Some(format!("snap-{n}")),
                manifest_digest: d('c'),
            },
        },
    ] {
        apply(
            s,
            Command::ObserveBuild {
                build_id: build.clone(),
                observation: o,
            },
        );
    }
    apply(
        s,
        Command::StartVerification {
            request_id: format!("rv{n}"),
            build_id: build,
            plan_digest: d('e'),
        },
    );
    for o in [
        V::Linked {
            run: run(&format!("job-v{n}")),
        },
        V::Passed {
            evidence_digest: d('9'),
            evidence: env::evidence::EvidenceStatus::Complete,
        },
    ] {
        apply(
            s,
            Command::ObserveVerification {
                verification_id: verify.clone(),
                observation: o,
            },
        );
    }
    let current = s.read("env-1").unwrap();
    apply(
        s,
        Command::Promote {
            request_id: format!("rp{n}"),
            expected_selection_revision: current.selection.revision,
            review: Review {
                id: format!("review-{n}"),
                actor: "operator".into(),
                candidate: current.propose(&verify).unwrap(),
                granted_ms: 1,
                expires_ms: 1000,
            },
        },
    );
}
fn record(f: &Fixture, id: &str) -> Record {
    f.owner.store().unwrap().read(id).unwrap()
}

#[test]
fn new_jobs_pin_the_selected_version_and_existing_jobs_keep_theirs() {
    let f = fixture(Fake::default());
    let s = environments(&f);
    promote(&s, 1);
    let op = submit(&f);
    let first = accepted(f.owner.execute("first", &f.principal, &op).unwrap());
    wait(&f, "first", "completed");
    let pin = record(&f, "first").environment.unwrap();
    assert_eq!(
        (pin.version_id.as_str(), pin.image.image_id.as_str()),
        ("v1", "oaenv-build-1")
    );

    // A later promotion does not touch the admitted job; its lost reply
    // returns the original receipt without re-resolving the selection.
    promote(&s, 2);
    assert_eq!(
        accepted(f.owner.execute("first", &f.principal, &op).unwrap()),
        first
    );
    assert_eq!(record(&f, "first").environment.unwrap(), pin);

    accepted(f.owner.execute("second", &f.principal, &op).unwrap());
    wait(&f, "second", "completed");
    assert_eq!(record(&f, "second").environment.unwrap().version_id, "v2");

    // Rollback: the next job starts from v1 again; the v2 job keeps v2.
    apply(
        &s,
        Command::Select {
            request_id: "rollback".into(),
            expected_selection_revision: 2,
            version_id: "v1".into(),
        },
    );
    accepted(f.owner.execute("third", &f.principal, &op).unwrap());
    wait(&f, "third", "completed");
    assert_eq!(record(&f, "third").environment.unwrap().version_id, "v1");
    assert_eq!(record(&f, "second").environment.unwrap().version_id, "v2");
    assert_eq!(
        *f.backend.provisioned.lock().unwrap(),
        vec![
            Some("oaenv-build-1".to_string()),
            Some("oaenv-build-2".to_string()),
            Some("oaenv-build-1".to_string()),
        ]
    );
}

#[test]
fn a_startup_failure_on_the_saved_image_stays_a_failure() {
    let f = fixture(Fake::default());
    promote(&environments(&f), 1);
    f.backend.image_missing.store(true, Ordering::SeqCst);
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    let job = wait(&f, "first", "failed");
    assert!(job.error.unwrap().contains("version v1 cannot start"));
    // Nothing was provisioned, least of all from the base template.
    assert!(f.backend.provisioned.lock().unwrap().is_empty());
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 0);
}

#[test]
fn jobs_without_a_selection_keep_the_base_and_incompatible_profiles_refuse() {
    let f = fixture(Fake::default());
    accepted(f.owner.execute("first", &f.principal, &submit(&f)).unwrap());
    wait(&f, "first", "completed");
    assert!(record(&f, "first").environment.is_none());
    assert_eq!(*f.backend.provisioned.lock().unwrap(), vec![None]);

    let f = fixture_with(Fake::default(), |_, p| p.mode = Mode::Integrated);
    promote(&environments(&f), 1);
    assert_eq!(
        f.owner.execute("first", &f.principal, &submit(&f)),
        Err(Code::Unsupported)
    );
    // The refusal left no job behind.
    assert!(f.owner.store().unwrap().read("first").is_err());
}

// ENV-07: the project environment panel's native reads and effects.

use coder_access::environment as panel;

fn env_read(f: &Fixture, principal: &Principal, environment: Option<&str>) -> Result<panel::View> {
    match f.owner.execute(
        "panel-read",
        principal,
        &Operation::EnvironmentRead {
            query: panel::Query {
                workspace: "checkout".into(),
                project: "synthetic".into(),
                environment: environment.map(str::to_owned),
                before: None,
            },
        },
    )? {
        Outcome::EnvironmentRead { view } => Ok(*view),
        other => panic!("unexpected {other:?}"),
    }
}
fn env_effect(f: &Fixture, request: &str, op: Operation) -> Result<panel::Accepted> {
    match f.owner.execute(request, &f.principal, &op)? {
        Outcome::EnvironmentAccepted { accepted } => Ok(accepted),
        other => panic!("unexpected {other:?}"),
    }
}
/// Retain a sealed verifier record with one call's stdout, as ENV-05 would.
fn verifier_evidence(f: &Fixture, task: &str, stdout: &[u8]) -> env::evidence::Sealed {
    use env::evidence::{CallIdentity, CallResult, Recorder, Redactor, StreamName};
    let run = env::RunLink {
        cloud_job: "job-verify".into(),
        task: Some(task.into()),
    };
    let mut r = Recorder::create(
        f.owner.0.root.join(VERIFY_EVIDENCE).join(task),
        "verify",
        Some(run.clone()),
        Redactor::new(),
        1 << 20,
    )
    .unwrap();
    r.start_call(
        CallIdentity {
            id: "check-1".into(),
            parent: None,
            run,
            tool: "environment.verify.check".into(),
            request: None,
            operation: None,
        },
        &serde_json::json!({"check": "cargo test"}),
        5,
    )
    .unwrap();
    r.output("check-1", StreamName::Stdout, stdout).unwrap();
    r.close_stream("check-1", StreamName::Stdout).unwrap();
    r.close_stream("check-1", StreamName::Stderr).unwrap();
    r.result(
        "check-1",
        CallResult::Exited {
            code: Some(0),
            success: true,
        },
        6,
    )
    .unwrap();
    r.finish(7).unwrap()
}

#[test]
fn panel_reads_are_admitted_bounded_and_side_effect_free() {
    let f = fixture(Fake::default());
    // Empty: no environment and no store; the read creates nothing.
    let view = env_read(&f, &f.principal, None).unwrap();
    assert!(view.detail.is_none() && view.environments.is_empty());
    assert!(!f.owner.0.root.join("environments").exists());

    let s = environments(&f);
    promote(&s, 1);
    let before = fs::read(f.owner.0.root.join("environments/env-1.json")).unwrap();
    let names = || {
        let mut v: Vec<_> = fs::read_dir(f.owner.0.root.join("environments"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        v.sort();
        v
    };
    let entries = names();
    let view = env_read(&f, &f.principal, None).unwrap();
    assert_eq!(
        fs::read(f.owner.0.root.join("environments/env-1.json")).unwrap(),
        before
    );
    assert_eq!(names(), entries, "a read leaves no lock or marker");
    let d = view.detail.unwrap();
    assert_eq!(d.active.as_deref(), Some("v1"));
    assert_eq!(d.history.len(), 1);
    assert!(d.history[0].selected);
    assert_eq!(d.builds[0].state, "ready");
    assert_eq!(d.verifications[0].state, "passed");
    // v1 is already saved from verify-1, so nothing awaits review.
    assert!(d.verifications[0].candidate.is_none());
    // No setup owner is composed: setup is unavailable, not empty.
    assert!(d.setup.is_none());
    assert!(!d.verifications[0].evidence_readable);

    // Denied: a device the operator policy does not admit for the project.
    let other = Principal {
        device: "other-device".into(),
        ..f.principal.clone()
    };
    assert_eq!(env_read(&f, &other, None), Err(Code::Forbidden));
    // An unknown environment is denied, not disclosed.
    assert_eq!(
        env_read(&f, &f.principal, Some("env-unknown")),
        Err(Code::Forbidden)
    );
}

#[test]
fn reviewed_promote_select_and_rollback_recover_by_request_and_pin_new_jobs() {
    let f = fixture(Fake::default());
    let s = environments(&f);
    promote(&s, 1);
    // Revision 2 is built and verified but not saved yet.
    apply(
        &s,
        Command::UpdateRecipe {
            expected_draft_revision: 1,
            recipe: recipe('2'),
        },
    );
    apply(
        &s,
        Command::StartBuild {
            request_id: "rb2".into(),
            expected_draft_revision: 2,
        },
    );
    for o in [
        B::Linked { run: run("job-b2") },
        B::ImageReady {
            image: env::ImageIdentity {
                provider: env::Provider::Boat,
                image_id: "oaenv-build-2".into(),
                snapshot_id: Some("snap-2".into()),
                manifest_digest: d('c'),
            },
        },
    ] {
        apply(
            &s,
            Command::ObserveBuild {
                build_id: "build-2".into(),
                observation: o,
            },
        );
    }
    apply(
        &s,
        Command::StartVerification {
            request_id: "rv2".into(),
            build_id: "build-2".into(),
            plan_digest: d('e'),
        },
    );
    let sealed = verifier_evidence(&f, "vjob-2", b"test result: ok. 3 passed\n");
    for o in [
        V::Linked {
            run: env::RunLink {
                cloud_job: "job-v2".into(),
                task: Some("vjob-2".into()),
            },
        },
        sealed.passed(),
    ] {
        apply(
            &s,
            Command::ObserveVerification {
                verification_id: "verify-2".into(),
                observation: o,
            },
        );
    }
    let d2 = env_read(&f, &f.principal, None).unwrap().detail.unwrap();
    let v2 = &d2.verifications[0];
    assert_eq!(v2.id, "verify-2");
    assert!(v2.evidence_readable);
    let candidate = v2.candidate.clone().expect("a reviewable candidate");
    assert_eq!(
        candidate.digest,
        s.read("env-1")
            .unwrap()
            .propose("verify-2")
            .unwrap()
            .digest()
    );

    let promote_op = |digest: String, expected| Operation::EnvironmentPromote {
        intent: panel::Promote {
            workspace: "checkout".into(),
            project: "synthetic".into(),
            environment: "env-1".into(),
            verification: "verify-2".into(),
            candidate_digest: digest,
            expected_selection_revision: expected,
        },
    };
    // A review of anything but the displayed candidate is stale.
    assert_eq!(
        env_effect(&f, "promote-wrong", promote_op(d('0'), 1)),
        Err(Code::Stale)
    );
    // A stale selection fence is refused and saves nothing.
    assert_eq!(
        env_effect(&f, "promote-fence", promote_op(candidate.digest.clone(), 0)),
        Err(Code::Stale)
    );
    assert_eq!(s.read("env-1").unwrap().versions.len(), 1);
    let first = env_effect(&f, "promote-2", promote_op(candidate.digest.clone(), 1)).unwrap();
    assert_eq!(
        (
            first.action.as_str(),
            first.version.as_deref(),
            first.selection_revision
        ),
        ("promote", Some("v2"), Some(2))
    );
    // The original request replays its result after a lost reply; reusing
    // its ID for another operation conflicts. Nothing new is saved.
    assert_eq!(
        env_effect(&f, "promote-2", promote_op(candidate.digest.clone(), 1)).unwrap(),
        first
    );
    assert_eq!(
        env_effect(
            &f,
            "promote-2",
            Operation::EnvironmentSelect {
                intent: panel::Select {
                    workspace: "checkout".into(),
                    project: "synthetic".into(),
                    environment: "env-1".into(),
                    version: "v1".into(),
                    expected_selection_revision: 2,
                },
            }
        ),
        Err(Code::Conflict)
    );
    assert_eq!(s.read("env-1").unwrap().versions.len(), 2);

    // A new job pins v2, and its job view shows the pin.
    accepted(f.owner.execute("job-a", &f.principal, &submit(&f)).unwrap());
    let job = wait(&f, "job-a", "completed");
    let pin = job.environment.expect("the job view shows its version");
    assert_eq!((pin.version_id.as_str(), pin.number), ("v2", 2));

    // Reviewed rollback to v1; the v2 job keeps its pin.
    let select = |expected| Operation::EnvironmentSelect {
        intent: panel::Select {
            workspace: "checkout".into(),
            project: "synthetic".into(),
            environment: "env-1".into(),
            version: "v1".into(),
            expected_selection_revision: expected,
        },
    };
    assert_eq!(env_effect(&f, "select-stale", select(1)), Err(Code::Stale));
    let rolled = env_effect(&f, "select-1", select(2)).unwrap();
    assert_eq!(
        (rolled.state.as_str(), rolled.version.as_deref()),
        ("rolled_back", Some("v1"))
    );
    // The original select request replays its first result.
    assert_eq!(env_effect(&f, "select-1", select(2)).unwrap(), rolled);
    assert_eq!(read(&f, "job-a").environment.unwrap().version_id, "v2");
    let detail = env_read(&f, &f.principal, None).unwrap().detail.unwrap();
    assert_eq!(detail.active.as_deref(), Some("v1"));
    assert_eq!(detail.changes[0].kind, "rolled_back");
    // Steering needs a composed setup owner.
    assert_eq!(
        env_effect(
            &f,
            "steer-1",
            Operation::EnvironmentSteer {
                intent: panel::Steer {
                    workspace: "checkout".into(),
                    project: "synthetic".into(),
                    environment: "env-1".into(),
                    session: "setup-1".into(),
                    text: "Use the locked toolchain.".into(),
                },
            }
        ),
        Err(Code::Unavailable)
    );

    // Evidence: summary, then original bytes by page, then a forged cursor.
    let query = |call: Option<&str>, cursor| panel::EvidenceQuery {
        workspace: "checkout".into(),
        project: "synthetic".into(),
        environment: "env-1".into(),
        verification: "verify-2".into(),
        child: None,
        call: call.map(str::to_owned),
        stream: call.map(|_| panel::Stream::Stdout),
        cursor,
        limit: 8,
    };
    let page = |q: panel::EvidenceQuery| {
        f.owner
            .execute(
                "evidence",
                &f.principal,
                &Operation::EnvironmentEvidence { query: q },
            )
            .map(|o| match o {
                Outcome::EnvironmentEvidence { page } => *page,
                other => panic!("unexpected {other:?}"),
            })
    };
    let summary = page(query(None, None)).unwrap();
    assert!(summary.complete && summary.gaps.is_empty() && summary.chunk.is_none());
    assert_eq!(
        summary.sealed_digest.as_deref(),
        Some(sealed.digest.as_str())
    );
    assert_eq!(summary.calls[0].stdout_bytes, 26);
    let mut bytes = vec![];
    let mut cursor = None;
    loop {
        use base64::Engine;
        let chunk = page(query(Some("check-1"), cursor)).unwrap().chunk.unwrap();
        bytes.extend(
            base64::engine::general_purpose::STANDARD
                .decode(&chunk.data)
                .unwrap(),
        );
        match chunk.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(bytes, b"test result: ok. 3 passed\n");
    let mut forged = page(query(Some("check-1"), None))
        .unwrap()
        .chunk
        .unwrap()
        .next
        .unwrap();
    forged.chunk_digest = Some(d('7'));
    assert_eq!(page(query(Some("check-1"), Some(forged))), Err(Code::Stale));
}
