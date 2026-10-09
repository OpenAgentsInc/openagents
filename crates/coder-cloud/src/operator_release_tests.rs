//! BYO-05: the user's released Claude credential serves one turn of their
//! own job and is never persisted. Fake keys and synthetic owners only.

use super::*;

const FAKE_BYO_KEY: &str = "sk-ant-api03-fake-byo05-released-key-for-tests";

fn release_op(f: &Fixture, job: &str, owner: &str) -> Operation {
    let p = &f.policy.profiles["fixture"];
    Operation::CloudRelease {
        intent: dto::Release {
            workspace: "checkout".into(),
            project: "synthetic".into(),
            profile: "fixture".into(),
            profile_revision: Operator::profile_revision(p).unwrap(),
            source_digest: p.source_digest.clone(),
            job: job.into(),
            owner: owner.into(),
            name: crate::claude::API_KEY.into(),
            value: FAKE_BYO_KEY.into(),
        },
    }
}

fn byo_fixture(backend: Fake) -> Fixture {
    fixture_with(backend, |_, p| p.executor = crate::claude::ENGINE.into())
}

fn release(f: &Fixture, job: &str, owner: &str) -> std::result::Result<dto::Released, Code> {
    match f
        .owner
        .execute(job, &f.principal, &release_op(f, job, owner))?
    {
        Outcome::CloudReleased { released } => Ok(released),
        _ => panic!("Expected a release receipt."),
    }
}

/// Every byte the operator state holds.
fn state_bytes(root: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(state_bytes(&path));
        } else {
            out.extend(fs::read(&path).unwrap_or_default());
        }
    }
    out
}

fn idle(f: &Fixture, job: &str) {
    for _ in 0..500 {
        if f.owner.store().unwrap().lease(job).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_released_credential_serves_one_turn_of_its_own_job_and_is_never_persisted() {
    let f = byo_fixture(Fake::default());
    let alice = crate::release::owner_digest("alice", "alice-personal", 3);
    let released = release(&f, "byo-first", &alice).unwrap();
    assert_eq!(released.name, crate::claude::API_KEY);
    assert!(!format!("{:?}", release_op(&f, "byo-first", &alice)).contains(FAKE_BYO_KEY));
    accepted(
        f.owner
            .execute("byo-first", &f.principal, &submit(&f))
            .unwrap(),
    );
    let job = wait(&f, "byo-first", "completed");
    // The turn's process environment held the user's key; the job's
    // evidence names only its type.
    assert_eq!(
        f.backend.seen.lock().unwrap().as_slice(),
        [Some(FAKE_BYO_KEY.to_owned())]
    );
    let record = f.owner.store().unwrap().read("byo-first").unwrap();
    assert_eq!(
        record.binding["claude"]["evidence"]["credential"],
        "anthropic_api_key"
    );
    assert!(!crate::release::armed("byo-first"));
    assert!(job.credential_names.is_empty());

    // Another owner, or a later membership epoch, cannot release into it.
    let later = crate::release::owner_digest("alice", "alice-personal", 4);
    assert_eq!(release(&f, "byo-first", &later), Err(Code::Forbidden));
    // A device the operator policy does not assign cannot release at all.
    let stranger = Principal {
        device: "e".repeat(64),
        grant: Some("a".repeat(64)),
        epoch: Some(1),
    };
    assert_eq!(
        f.owner
            .execute("x", &stranger, &release_op(&f, "byo-first", &alice))
            .map(|_| ()),
        Err(Code::Forbidden)
    );

    // Removed: the next turn gets no release and runs on the plan login.
    idle(&f, "byo-first");
    accepted(
        f.owner
            .execute(
                "byo-continue",
                &f.principal,
                &Operation::CloudContinue {
                    intent: dto::Continue {
                        scope: read(&f, "byo-first").scope,
                        prompt: "Next turn after removal".into(),
                    },
                },
            )
            .unwrap(),
    );
    wait(&f, "byo-first", "completed");
    assert_eq!(f.backend.seen.lock().unwrap()[1], None);
    let record = f.owner.store().unwrap().read("byo-first").unwrap();
    assert_eq!(
        record.binding["claude"]["evidence"]["credential"],
        "claude_plan_login"
    );
    assert!(
        record
            .events
            .iter()
            .any(|e| e["event"] == "engine" && e["credential"] == "claude_plan_login")
    );

    // Nothing the operator keeps (records, admissions, journals, archives)
    // holds the credential.
    let bytes = state_bytes(&f.owner.0.root);
    assert!(
        !bytes
            .windows(FAKE_BYO_KEY.len())
            .any(|w| w == FAKE_BYO_KEY.as_bytes())
    );
}

#[test]
fn parallel_claude_turns_are_admitted_only_on_the_users_released_key() {
    let f = byo_fixture(Fake {
        running: true,
        ..Fake::default()
    });
    let alice = crate::release::owner_digest("alice", "alice-personal", 1);
    for job in ["byo-par-1", "byo-par-2"] {
        release(&f, job, &alice).unwrap();
        accepted(f.owner.execute(job, &f.principal, &submit(&f)).unwrap());
    }
    // One plan turn runs beside the keyed ones; a second waits.
    accepted(
        f.owner
            .execute("byo-plan-1", &f.principal, &submit(&f))
            .unwrap(),
    );
    assert_eq!(
        f.owner
            .execute("byo-plan-2", &f.principal, &submit(&f))
            .map(|_| ()),
        Err(Code::Conflict)
    );
    // A release for one job never serves another.
    release(&f, "byo-par-unused", &alice).unwrap();
    assert_eq!(
        f.owner
            .execute("byo-plan-3", &f.principal, &submit(&f))
            .map(|_| ()),
        Err(Code::Conflict)
    );
}

#[test]
fn only_claude_profiles_without_their_own_key_take_a_release() {
    let alice = crate::release::owner_digest("alice", "alice-personal", 1);
    let codex = fixture(Fake::default());
    assert_eq!(release(&codex, "byo-codex", &alice), Err(Code::Unsupported));
    let keyed = fixture_with(Fake::default(), |root, p| {
        p.executor = crate::claude::ENGINE.into();
        directory(&root.join("keys")).unwrap();
        let key = root.join("keys/synthetic-anthropic-key");
        write(&key, b"synthetic-own-api-key").unwrap();
        p.credentials.insert(crate::claude::API_KEY.into(), key);
    });
    assert_eq!(release(&keyed, "byo-keyed", &alice), Err(Code::Unsupported));
    // A claude.ai login is never accepted as a release.
    let f = byo_fixture(Fake::default());
    let Operation::CloudRelease { mut intent } = release_op(&f, "byo-login", &alice) else {
        unreachable!()
    };
    intent.value = format!("sk-ant-oat01-{}", "q7".repeat(40));
    assert!(
        f.owner
            .execute(
                "byo-login",
                &f.principal,
                &Operation::CloudRelease { intent }
            )
            .is_err()
    );
}

#[test]
fn a_release_lapses_and_never_crosses_devices() {
    let (class, credentials) =
        crate::release::credentials(crate::claude::API_KEY, FAKE_BYO_KEY).unwrap();
    crate::release::offer("byo-lapse", "dev-a", "o", class, credentials.clone(), 100);
    assert!(crate::release::claim("byo-lapse", "dev-a", 100 + dto::RELEASE_SECONDS).is_none());
    crate::release::offer("byo-cross", "dev-a", "o", class, credentials, 100);
    assert!(crate::release::claim("byo-cross", "dev-b", 101).is_none());
    // Dropped, not left for the right device either.
    assert!(crate::release::claim("byo-cross", "dev-a", 101).is_none());
}

#[test]
fn a_paused_job_on_a_released_key_resumes_only_with_a_fresh_release() {
    // The first turn ends on a usage limit whose reset already passed.
    let f = byo_fixture(Fake {
        limit: Some(1),
        ..Fake::default()
    });
    let alice = crate::release::owner_digest("alice", "alice-personal", 1);
    release(&f, "byo-pause", &alice).unwrap();
    accepted(
        f.owner
            .execute("byo-pause", &f.principal, &submit(&f))
            .unwrap(),
    );
    wait_until(&f, "byo-pause", |job| job.state == "paused");
    // Due at once, but the turn's release is spent: it stays paused.
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(read(&f, "byo-pause").state, "paused");
    assert_eq!(f.backend.dispatches.load(Ordering::SeqCst), 1);
    // A follow with a fresh release resumes it on the same key.
    release(&f, "byo-pause", &alice).unwrap();
    accepted(
        f.owner
            .execute(
                "byo-pause-follow",
                &f.principal,
                &Operation::CloudFollow {
                    intent: dto::Follow {
                        scope: read(&f, "byo-pause").scope,
                    },
                },
            )
            .unwrap(),
    );
    wait_long(&f, "byo-pause", "completed");
    assert_eq!(
        f.backend.seen.lock().unwrap().as_slice(),
        [Some(FAKE_BYO_KEY.to_owned()), Some(FAKE_BYO_KEY.to_owned())]
    );
}
