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
