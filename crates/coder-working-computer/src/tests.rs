use crate::driver::{Driver, Settled};
use crate::provider::fake::{FakeProvider, Inject};
use crate::provider::{CheckpointEvidence, Outcome};
use crate::store::{Store, StoreError};
use crate::*;

const GH_SECRET: &str = "gh-secret-value-0123456789";
const USER_LOGIN: &str = "user-claude-oauth-session-abcdef";
const LOGIN_PATH: &str = ".claude/.credentials.json";

fn owner() -> Principal {
    Principal {
        workspace: "ws-1".into(),
        principal: "user-1".into(),
    }
}
fn spec(services: Vec<ServiceDecl>) -> Spec {
    Spec {
        id: "computer-1".into(),
        owner: owner(),
        chat: "chat-1".into(),
        project: coder_environment::ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        source: coder_environment::SourcePin {
            repository: Some("example/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        },
        base: Some(BaseLink {
            environment: "env-1".into(),
            version: "env-1-v1".into(),
        }),
        size: "small".into(),
        credential_names: ["GH_TOKEN".to_string()].into(),
        services,
        bounds: Bounds {
            idle_ms: 600_000,
            observed_extension_ms: 300_000,
            absolute_ms: 7_200_000,
        },
    }
}
fn web() -> ServiceDecl {
    ServiceDecl {
        name: "web".into(),
        command: "npm run dev".into(),
        cwd: ".".into(),
        health: Health::Http {
            port: 3000,
            path: "/health".into(),
        },
        ready_within_seconds: 30,
    }
}
fn setup(
    dir: &tempfile::TempDir,
    checkpoint_stops: bool,
    services: Vec<ServiceDecl>,
) -> Driver<FakeProvider> {
    let store = Store::under(dir.path());
    store
        .create(&Computer::new(spec(services), 0).unwrap())
        .unwrap();
    let provider = FakeProvider::new(
        [("GH_TOKEN".to_string(), GH_SECRET.to_string())].into(),
        checkpoint_stops,
    );
    Driver::new(store, provider)
}
fn generation(s: &Settled) -> u64 {
    match s {
        Settled::Dispatch { generation, .. } => *generation,
        other => panic!("expected a dispatch, got {other:?}"),
    }
}
fn read(d: &Driver<FakeProvider>) -> Computer {
    d.store.read("computer-1").unwrap()
}

#[tokio::test]
async fn turn_checkpoint_restore_reapplies_credentials_and_restarts_services() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, true, vec![web()]);

    let s = d.prompt("computer-1", 1_000).await.unwrap();
    assert_eq!(generation(&s), 1);
    let c = read(&d);
    let resource = c.resource().unwrap().to_owned();
    assert_eq!(c.boot().unwrap().origin, BootOrigin::Created);
    assert!(c.boot().unwrap().services["web"].is_done());
    d.provider.write(&resource, "src/lib.rs", "turn one");

    let s = d.turn_finished("computer-1", 1, 2_000).await.unwrap();
    let c = s.computer();
    assert_eq!(c.phase, Phase::Stopped);
    let k = c.latest_checkpoint().unwrap();
    assert_eq!(k.turn_generation, 1);
    assert!(k.may_hold_user_logins);
    let b = c.boot().unwrap();
    // Separate facts: checkpoint, shutdown, resource stop, meter.
    assert!(b.shutdown.as_ref().unwrap().is_done());
    assert!(b.resource_stop.as_ref().unwrap().is_done());
    assert!(b.meter_stop.as_ref().unwrap().is_done());
    assert!(c.deletion.is_none());
    assert!(c.creates[0].deletion.is_none());
    let machine = d.provider.machine(&resource).unwrap();
    assert!(!machine.running && machine.env.is_empty() && machine.services.is_empty());

    let before = d.provider.calls().len();
    let s = d.prompt("computer-1", 3_000).await.unwrap();
    assert_eq!(generation(&s), 2);
    let calls = d.provider.calls()[before..].to_vec();
    assert_eq!(calls, ["restore", "credentials", "service"]);
    let c = read(&d);
    assert_eq!(c.resource(), Some(resource.as_str()));
    assert_eq!(
        c.boot().unwrap().origin,
        BootOrigin::Restored {
            checkpoint: Some("computer-1-turn-1".into())
        }
    );
    let machine = d.provider.machine(&resource).unwrap();
    assert_eq!(machine.files["src/lib.rs"], "turn one");
    assert_eq!(machine.env["GH_TOKEN"], GH_SECRET);
    assert!(machine.services.contains("web"));
    assert_eq!(c.service_readiness()[0].1.map(Fact::is_done), Some(true));
}

#[tokio::test]
async fn a_queued_prompt_cannot_race_the_checkpoint_of_the_finished_turn() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    assert_eq!(generation(&d.prompt("computer-1", 1).await.unwrap()), 1);
    let resource = read(&d).resource().unwrap().to_owned();
    d.provider.write(&resource, "a.txt", "one");

    // Fence the turn without taking the checkpoint yet (as if the owner
    // restarted right after the turn finished).
    let lease = d.store.lease("computer-1").unwrap();
    lease
        .apply(&Command::FinishTurn { generation: 1 }, 2)
        .unwrap();
    let c = lease.read().unwrap();
    drop(lease);
    assert_eq!(
        decide(&c, Trigger::Prompt, 3),
        Decision::Checkpoint { generation: 1 }
    );
    assert_eq!(
        apply(&c, &Command::StartTurn { generation: 2 }, 3),
        Err(Refusal::Phase("checkpointing".into()))
    );
    assert_eq!(
        apply(
            &c,
            &Command::ObserveCheckpoint {
                generation: 2,
                outcome: Outcome::done(CheckpointEvidence {
                    snapshot: "x".into(),
                    stopped: None
                })
            },
            3
        ),
        Err(Refusal::StaleGeneration {
            fence: 1,
            observed: 2
        })
    );

    // The queued prompt takes turn 1's checkpoint first, then dispatches.
    let s = d.prompt("computer-1", 4).await.unwrap();
    assert_eq!(generation(&s), 2);
    d.provider.write(&resource, "a.txt", "two");
    let c = read(&d);
    let k = &c.checkpoints[0];
    assert_eq!(k.turn_generation, 1);
    let files = d
        .provider
        .snapshot_files(k.fact.evidence().unwrap())
        .unwrap();
    assert_eq!(files["a.txt"], "one");
    // A late duplicate completion of turn 1 cannot refence turn 2.
    assert!(matches!(
        apply(&c, &Command::FinishTurn { generation: 1 }, 5),
        Err(Refusal::StaleGeneration {
            fence: 2,
            observed: 1
        })
    ));
}

#[tokio::test]
async fn user_login_persists_only_in_their_own_checkpoint_and_no_credential_is_captured() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, true, vec![]);
    d.prompt("computer-1", 1).await.unwrap();
    let resource = read(&d).resource().unwrap().to_owned();
    // The user signs in inside their own computer's terminal.
    d.provider.write(&resource, LOGIN_PATH, USER_LOGIN);
    d.turn_finished("computer-1", 1, 2).await.unwrap();

    let c = read(&d);
    let k = c.latest_checkpoint().unwrap().clone();
    let snapshot = d
        .provider
        .snapshot_files(k.fact.evidence().unwrap())
        .unwrap();
    // It persists in this user's checkpoint ...
    assert_eq!(snapshot[LOGIN_PATH], USER_LOGIN);
    // ... and selected credentials, applied per boot, are not captured.
    assert!(!snapshot.values().any(|v| v.contains(GH_SECRET)));
    // The retained record holds neither the login nor credential values.
    let json = serde_json::to_string(&c).unwrap();
    assert!(!json.contains(USER_LOGIN) && !json.contains(GH_SECRET));
    assert!(json.contains("GH_TOKEN"));

    // Only this computer, for its owner, may use the checkpoint.
    let me = owner();
    let other = Principal {
        workspace: "ws-1".into(),
        principal: "user-2".into(),
    };
    assert_eq!(
        admit_checkpoint_use(
            &c,
            &k.id,
            &CheckpointUse::Restore {
                computer: "computer-1",
                principal: &me
            }
        ),
        Ok(())
    );
    for refused in [
        CheckpointUse::Restore {
            computer: "computer-2",
            principal: &me,
        },
        CheckpointUse::Restore {
            computer: "computer-1",
            principal: &other,
        },
        CheckpointUse::EnvironmentImage,
        CheckpointUse::OperatorCopy,
        CheckpointUse::ServiceRead,
    ] {
        assert_eq!(
            admit_checkpoint_use(&c, &k.id, &refused),
            Err(Refusal::CheckpointCustody),
            "{refused:?}"
        );
    }
    // No service read path reaches the login file.
    assert!(!service_may_read(LOGIN_PATH));
    assert!(!service_may_read("/home/user/.claude/.credentials.json"));
    assert!(service_may_read("src/main.rs"));
    // OpenAgents never accepts a Claude sign-in token as a credential.
    let mut s = spec(vec![]);
    s.credential_names.insert("CLAUDE_CODE_OAUTH_TOKEN".into());
    assert!(Computer::new(s, 0).is_err());
    // A tampered record that hands a checkpoint to someone else is corrupt.
    let mut tampered = c.clone();
    tampered.checkpoints[0].custody = Custody::UserPrivate {
        principal: other,
        computer: "computer-1".into(),
    };
    assert!(tampered.validate().is_err());

    // The login comes back on restore; credentials are applied afresh.
    d.prompt("computer-1", 3).await.unwrap();
    let m = d.provider.machine(&resource).unwrap();
    assert_eq!(m.files[LOGIN_PATH], USER_LOGIN);
    assert_eq!(m.env["GH_TOKEN"], GH_SECRET);
}

#[tokio::test]
async fn idle_and_absolute_bounds_extend_while_observed() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    d.prompt("computer-1", 0).await.unwrap();
    d.turn_finished("computer-1", 1, 1_000).await.unwrap();
    let c = read(&d);
    assert_eq!(c.phase, Phase::Awake);
    assert_eq!(c.boot().unwrap().idle_deadline_ms, 601_000);

    assert!(matches!(
        d.tick("computer-1", false, 600_000).await.unwrap(),
        Settled::Idle(_)
    ));
    // Observed near the deadline: extended by five minutes.
    d.tick("computer-1", true, 590_000).await.unwrap();
    assert_eq!(read(&d).boot().unwrap().idle_deadline_ms, 890_000);
    assert_eq!(read(&d).phase, Phase::Awake);
    let s = d.tick("computer-1", false, 890_000).await.unwrap();
    let b = s.computer().boot().unwrap();
    assert_eq!(s.computer().phase, Phase::Stopped);
    assert_eq!(b.stop_reason, Some(StopReason::Idle));
    assert!(b.shutdown.as_ref().unwrap().is_done());
    assert!(b.resource_stop.as_ref().unwrap().is_done());
    assert!(b.meter_stop.as_ref().unwrap().is_done());

    // Observation never passes the absolute bound, which also ends a turn.
    let s = d.prompt("computer-1", 1_000_000).await.unwrap();
    assert_eq!(generation(&s), 2);
    let absolute = read(&d).boot().unwrap().absolute_deadline_ms;
    assert_eq!(absolute, 8_200_000);
    // The turn's owner is alive throughout (#11059).
    d.heartbeat("computer-1", 2, absolute - 1).await.unwrap();
    d.tick("computer-1", true, absolute - 1).await.unwrap();
    assert_eq!(read(&d).boot().unwrap().idle_deadline_ms, absolute);
    let s = d.tick("computer-1", true, absolute).await.unwrap();
    assert_eq!(s.computer().phase, Phase::Stopped);
    assert_eq!(
        s.computer().boot().unwrap().stop_reason,
        Some(StopReason::Absolute)
    );
}

#[tokio::test]
async fn unknown_provider_stop_stays_unknown_until_proved() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    d.prompt("computer-1", 0).await.unwrap();
    d.turn_finished("computer-1", 1, 1).await.unwrap();
    d.provider.inject("stop", Inject::LostReply);
    d.provider.inject("inspect", Inject::Unknown);
    let s = d.stop("computer-1", 2).await.unwrap();
    assert!(matches!(s, Settled::Stuck(Decision::Inspect, _)), "{s:?}");
    let c = s.computer();
    assert!(matches!(c.phase, Phase::Unknown { .. }));
    assert!(
        c.boot()
            .unwrap()
            .resource_stop
            .as_ref()
            .unwrap()
            .is_unknown()
    );
    assert!(c.boot().unwrap().meter_stop.is_none());
    // A prompt neither restores nor replaces it; it reconciles first. Now the
    // provider answers, proving the stop that happened.
    let before = d.provider.calls().len();
    let s = d.prompt("computer-1", 3).await.unwrap();
    assert_eq!(d.provider.calls()[before], "inspect");
    assert_eq!(generation(&s), 2);
    let c = read(&d);
    assert!(c.boots[0].resource_stop.as_ref().unwrap().is_done());
    assert_eq!(c.creates.len(), 1);

    // Running after an unknown stop: definitely not stopped, so stop again.
    let mut c = read(&d);
    c.phase = Phase::Unknown {
        reason: "stop".into(),
    };
    c.boot_mut().unwrap().stop_reason = Some(StopReason::Owner);
    c.boot_mut().unwrap().resource_stop = Some(Fact::Unknown {
        reason: "lost".into(),
        at_ms: 4,
    });
    let ins = provider::Inspection {
        running: Some(false),
        stop: None,
        latest_snapshot: None,
    };
    assert_eq!(
        apply(
            &c,
            &Command::ObserveInspection {
                outcome: Outcome::done(ins)
            },
            5
        ),
        Ok(Applied::Unchanged),
        "not running without stop evidence stays unknown"
    );
}

#[tokio::test]
async fn lost_create_reply_reconciles_the_same_resource() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    d.provider.inject("create", Inject::LostReply);
    let s = d.prompt("computer-1", 0).await.unwrap();
    assert_eq!(generation(&s), 1);
    let c = read(&d);
    assert_eq!(c.creates.len(), 1);
    assert_eq!(c.resource(), Some("box-1"));
    assert_eq!(d.provider.state.lock().unwrap().machines.len(), 1);
    assert_eq!(
        d.provider.calls().iter().filter(|c| *c == "create").count(),
        2
    );
}

#[tokio::test]
async fn failed_fresh_boot_is_cleaned_up_and_a_later_prompt_creates_again() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    d.provider.inject("credentials", Inject::Failed);
    let s = d.prompt("computer-1", 0).await.unwrap();
    assert!(matches!(s, Settled::BootFailed(_)), "{s:?}");
    let c = s.computer();
    assert!(matches!(c.phase, Phase::Failed { .. }), "{s:?}");
    let b = c.boot().unwrap();
    assert_eq!(b.stop_reason, Some(StopReason::FailedBoot));
    assert!(b.resource_stop.as_ref().unwrap().is_done());
    assert!(b.meter_stop.as_ref().unwrap().is_done());
    assert!(c.creates[0].deletion.as_ref().unwrap().is_done());
    assert!(c.resource().is_none());
    assert!(d.provider.machine("box-1").is_none());

    let s = d.prompt("computer-1", 10).await.unwrap();
    assert_eq!(generation(&s), 1);
    let c = read(&d);
    assert_eq!(c.creates[1].operation, "computer-1-create-2");
    assert_ne!(c.resource(), Some("box-1"));
    assert_eq!(d.provider.state.lock().unwrap().machines.len(), 1);
}

#[tokio::test]
async fn failed_restore_boot_stops_but_keeps_the_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, true, vec![]);
    d.prompt("computer-1", 0).await.unwrap();
    d.turn_finished("computer-1", 1, 1).await.unwrap();
    d.provider.inject("credentials", Inject::Failed);
    let s = d.prompt("computer-1", 2).await.unwrap();
    assert!(matches!(s, Settled::BootFailed(_)), "{s:?}");
    assert_eq!(s.computer().phase, Phase::Stopped);
    // The next prompt restores the same checkpoint again.
    let s = d.prompt("computer-1", 3).await.unwrap();
    assert_eq!(generation(&s), 2);
    let c = read(&d);
    assert_eq!(c.boots.len(), 3);
    assert_eq!(c.boots[1].stop_reason, Some(StopReason::FailedBoot));
    assert!(c.boots[1].resource_stop.as_ref().unwrap().is_done());
    assert!(c.creates[0].deletion.is_none());
    assert!(c.latest_checkpoint().is_some());
}

#[tokio::test]
async fn service_readiness_is_reported_and_a_broken_service_does_not_block() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = web();
    api.name = "api".into();
    let d = setup(&dir, true, vec![web(), api]);
    d.provider
        .state
        .lock()
        .unwrap()
        .broken_services
        .insert("api".into());
    assert_eq!(generation(&d.prompt("computer-1", 0).await.unwrap()), 1);
    let c = read(&d);
    let ready: Vec<_> = c
        .service_readiness()
        .into_iter()
        .map(|(n, f)| (n, f.map(Fact::is_done)))
        .collect();
    assert_eq!(ready, [("web", Some(true)), ("api", Some(false))]);
}

#[tokio::test]
async fn sticky_meter_stays_unknown_while_stop_is_done() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    d.prompt("computer-1", 0).await.unwrap();
    d.turn_finished("computer-1", 1, 1).await.unwrap();
    d.provider.state.lock().unwrap().sticky_meter = true;
    let s = d.stop("computer-1", 2).await.unwrap();
    assert!(matches!(s, Settled::Stuck(Decision::CheckMeter { .. }, _)));
    let b = s.computer().boot().unwrap().clone();
    assert_eq!(s.computer().phase, Phase::Stopped);
    assert!(b.resource_stop.unwrap().is_done());
    assert!(b.meter_stop.unwrap().is_unknown());
}

#[tokio::test]
async fn delete_stops_meters_and_deletes_as_separate_facts() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    d.prompt("computer-1", 0).await.unwrap();
    let s = d.delete("computer-1", 1).await.unwrap();
    let c = s.computer();
    assert_eq!(c.phase, Phase::Deleted, "{s:?}");
    let b = c.boot().unwrap();
    assert!(b.shutdown.as_ref().unwrap().is_done());
    assert!(b.resource_stop.as_ref().unwrap().is_done());
    assert!(b.meter_stop.as_ref().unwrap().is_done());
    assert!(c.creates[0].deletion.as_ref().unwrap().is_done());
    assert!(c.deletion.as_ref().unwrap().is_done());
    assert!(d.provider.machine("box-1").is_none());
    assert!(matches!(
        d.prompt("computer-1", 2).await.unwrap(),
        Settled::Refused(..)
    ));
}

#[tokio::test]
async fn the_lease_serializes_owners_and_history_is_append_only() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, true, vec![]);
    let held = d.store.lease("computer-1").unwrap();
    assert!(matches!(
        d.prompt("computer-1", 0).await,
        Err(StoreError::Busy)
    ));
    drop(held);
    d.prompt("computer-1", 0).await.unwrap();
    d.turn_finished("computer-1", 1, 1).await.unwrap();
    let lease = d.store.lease("computer-1").unwrap();
    let c = lease.read().unwrap();
    let mut rewritten = c.clone();
    rewritten.revision += 1;
    rewritten.checkpoints[0].fact = Fact::Done {
        evidence: "other".into(),
        at_ms: 9,
    };
    assert_eq!(
        lease.commit(c.revision, &rewritten),
        Err(StoreError::Immutable)
    );
    assert!(matches!(
        lease.commit(c.revision + 7, &c),
        Err(StoreError::Fence { .. })
    ));
}

#[test]
fn decisions_are_pure_over_retained_state() {
    let c = Computer::new(spec(vec![]), 0).unwrap();
    assert_eq!(decide(&c, Trigger::Tick, 0), Decision::Skip);
    assert_eq!(
        decide(&c, Trigger::Prompt, 0),
        Decision::Create {
            operation: "computer-1-create-1".into()
        }
    );
    let mut bad = c.clone();
    bad.bounds.idle_ms = bad.bounds.absolute_ms + 1;
    assert!(bad.validate().is_err());
}

/// #11059: a turn whose owner stops beating ends as stale and the machine
/// stops (its files stay with it); a live owner's long turn runs on.
#[tokio::test]
async fn a_silent_turn_stops_and_a_beating_one_runs_on() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    let g = generation(&d.prompt("computer-1", 0).await.unwrap());
    assert_eq!(read(&d).turn.heartbeat_ms, 0, "a turn started at 0");

    // A live owner beats every 30 s; an hour of turn is never stale.
    let mut now = 0;
    while now < 3_600_000 {
        now += HEARTBEAT_EVERY_MS;
        d.heartbeat("computer-1", g, now).await.unwrap();
        let s = d.tick("computer-1", false, now + 1).await.unwrap();
        assert_eq!(s.computer().phase, Phase::Turn { generation: g }, "{now}");
    }
    // A heartbeat for another generation is refused.
    assert!(d.heartbeat("computer-1", g + 1, now).await.is_err());

    // Silent for less than the window: still running.
    let s = d
        .tick("computer-1", false, now + STALE_AFTER_MS - 1)
        .await
        .unwrap();
    assert_eq!(s.computer().phase, Phase::Turn { generation: g });
    // Recovered: a late beat keeps it.
    d.heartbeat("computer-1", g, now + STALE_AFTER_MS - 1)
        .await
        .unwrap();
    now += STALE_AFTER_MS - 1;
    // Silent past the window: stopped as stale through the stop path.
    let s = d
        .tick("computer-1", false, now + STALE_AFTER_MS)
        .await
        .unwrap();
    let c = s.computer();
    assert_eq!(c.phase, Phase::Stopped, "{c:?}");
    let b = c.boot().unwrap();
    assert_eq!(b.stop_reason, Some(StopReason::Stale));
    assert!(b.resource_stop.as_ref().is_some_and(Fact::is_done));
    assert!(!d.provider.machine(c.resource().unwrap()).unwrap().running);
    // Never announced as a finished turn, and the next prompt restores.
    assert_eq!(c.turn.completed, 0);
    let s = d
        .prompt("computer-1", now + STALE_AFTER_MS + 1)
        .await
        .unwrap();
    assert_eq!(generation(&s), g + 1);
}

/// A record from before heartbeats (no beat recorded) is never judged.
#[tokio::test]
async fn a_turn_with_no_recorded_heartbeat_is_not_judged() {
    let mut c = Computer::new(spec(vec![]), 0).unwrap();
    c.phase = Phase::Turn { generation: 1 };
    c.turn.heartbeat_ms = 0;
    assert!(!c.turn_silent(u64::MAX / 2));
    c.turn.heartbeat_ms = 10;
    assert!(c.turn_silent(10 + STALE_AFTER_MS));
    c.stale_ms = 0;
    assert!(!c.turn_silent(u64::MAX / 2));
}

/// Three failed boots in five minutes refuse a fourth with a plain
/// message until the window passes; then a prompt boots again.
#[tokio::test]
async fn repeated_boot_failures_open_and_close_the_breaker() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, false, vec![]);
    for at in [0, 60_000, 120_000] {
        d.provider.inject("create", Inject::Failed);
        let s = d.prompt("computer-1", at).await.unwrap();
        assert!(matches!(s.computer().phase, Phase::Failed { .. }), "{s:?}");
    }
    let creates = d.provider.calls().iter().filter(|c| *c == "create").count();
    let s = d.prompt("computer-1", 180_000).await.unwrap();
    let Settled::Refused(message, _) = &s else {
        panic!("expected the breaker, got {s:?}");
    };
    assert!(message.contains("failed to start 3 times"), "{message}");
    assert!(message.contains("in 2 minutes"), "{message}");
    assert!(oa_copy_free(message), "{message}");
    // Refused: no new machine was asked for.
    assert_eq!(
        d.provider.calls().iter().filter(|c| *c == "create").count(),
        creates
    );
    // The window passes (the first failure ages out): it boots again.
    let s = d.prompt("computer-1", 300_000).await.unwrap();
    assert_eq!(generation(&s), 1, "{s:?}");
    assert_eq!(read(&d).boot_failures.len(), 3);
}

/// A failed restore counts too, and the breaker refuses the next restore.
#[tokio::test]
async fn failed_restores_count_toward_the_breaker() {
    let dir = tempfile::tempdir().unwrap();
    let d = setup(&dir, true, vec![]);
    let g = generation(&d.prompt("computer-1", 0).await.unwrap());
    d.turn_finished("computer-1", g, 1).await.unwrap();
    for at in [10, 20, 30] {
        d.provider.inject("restore", Inject::Failed);
        let s = d.prompt("computer-1", at).await.unwrap();
        assert_eq!(s.computer().phase, Phase::Stopped, "{s:?}");
    }
    let s = d.prompt("computer-1", 40).await.unwrap();
    assert!(matches!(s, Settled::Refused(_, _)), "{s:?}");
    let s = d
        .prompt("computer-1", 10 + BREAKER_WINDOW_MS)
        .await
        .unwrap();
    assert_eq!(generation(&s), g + 1, "{s:?}");
}

fn oa_copy_free(text: &str) -> bool {
    !["circuit", "breaker", "provision", "reconcile"]
        .iter()
        .any(|w| text.to_lowercase().contains(w))
}
