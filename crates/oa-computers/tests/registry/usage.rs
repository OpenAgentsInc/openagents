//! Allocation conservation and usage evidence against disposable PostgreSQL.
use super::*;
use oa_computers::usage::{Limits, QUOTA_ERROR};

const BOOT_CPU_MS: i64 = 2 * oa_computers::api::BOUNDS.absolute_ms as i64;

async fn database() -> Option<(Db, String)> {
    let admin = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL").ok()?;
    Some(Db::scratch(&admin).await.unwrap())
}

fn intent(key: &str) -> CreateIntent {
    CreateIntent {
        idempotency_key: Some(key.into()),
        ..Default::default()
    }
}

#[tokio::test]
async fn simultaneous_accounts_cannot_overbook_service_capacity() {
    let Some((db, dsn)) = database().await else {
        return;
    };
    let store = Store::new(db.clone());
    store
        .set_usage_limits(
            None,
            &Limits {
                computer_count: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let a = Simulated::new(&root.path().join("a"), false);
    let b = Simulated::new(&root.path().join("b"), true);
    let driver = configured("a", &a, &b);
    let mut jobs = Vec::new();
    for index in 0..12 {
        let store = store.clone();
        let catalog = driver.catalog().clone();
        jobs.push(tokio::spawn(async move {
            store
                .create_placed(&format!("account-{index}"), &intent("create"), &catalog)
                .await
        }));
    }
    let mut accepted = Vec::new();
    for job in jobs {
        match job.await.unwrap() {
            Ok((entry, true)) => accepted.push(entry),
            Err(oa_computers::store::CreateError::Limit) => {}
            other => panic!("unexpected admission: {other:?}"),
        }
    }
    assert_eq!(accepted.len(), 1);
    let e = &accepted[0];
    let replay = store
        .create_placed(&e.owner, &intent("create"), driver.catalog())
        .await
        .unwrap();
    assert!(!replay.1);
    assert_eq!(replay.0.id, e.id);
    assert_eq!(
        db.client()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM computers.usage_reservations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert!(a.creates.lock().unwrap().is_empty());
    assert!(b.creates.lock().unwrap().is_empty());
    // The previous service protocol cannot bypass reservation or evidence.
    let c = db.client().await.unwrap();
    assert_eq!(c.execute("UPDATE computers.computers SET revision=revision+1,record=jsonb_set(record,'{revision}',to_jsonb(revision+1)) WHERE id=$1", &[&e.id]).await.unwrap(),0);
    assert_eq!(c.execute("UPDATE computers.computers SET desired='deleted',intent_generation=intent_generation+1 WHERE id=$1", &[&e.id]).await.unwrap(),0);
    assert_eq!(c.execute("INSERT INTO computers.computers(id,owner,driver,size,desired,phase,revision,record) SELECT id||'-old',owner,driver,size,desired,phase,revision,record FROM computers.computers WHERE id=$1", &[&e.id]).await.unwrap(),0);
    let (old, connection) = tokio_postgres::connect(&dsn, tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    assert!(old.execute("INSERT INTO computers.turns(id,computer,owner,state,command,timeout_seconds) VALUES('legacy-turn',$1,$2,'queued','true',3600)", &[&e.id,&e.owner]).await.is_err());
    store
        .add_turn("compat-turn", &e.id, &e.owner, "true", None, 2, &[])
        .await
        .unwrap();
    assert!(
        old.execute(
            "UPDATE computers.turns SET state='running' WHERE id='compat-turn'",
            &[]
        )
        .await
        .is_err()
    );
    assert!(old.execute("INSERT INTO computers.commands(turn,owner,computer,request_key,digest,options,status) VALUES('compat-turn',$1,$2,'legacy-key','legacy-digest','{}','queued')", &[&e.owner,&e.id]).await.is_err());
    let request = serde_json::from_value(
        json!({"command":"true","idempotency_key":"compat-command","timeout_seconds":1}),
    )
    .unwrap();
    let (command, _) = store.add_command(&e.owner, &e.id, &request).await.unwrap();
    assert!(
        old.execute(
            "UPDATE computers.commands SET status='dispatching' WHERE turn=$1",
            &[&command]
        )
        .await
        .is_err()
    );
    assert_eq!(
        store
            .set_usage_limits(
                None,
                &Limits {
                    turn_seconds: Some(1),
                    ..Default::default()
                }
            )
            .await
            .unwrap_err()
            .0,
        QUOTA_ERROR
    );
}

#[tokio::test]
async fn missing_shapes_cannot_enter_bounded_allocations_or_appear_as_zero_usage() {
    let Some((db, _)) = database().await else {
        return;
    };
    let store = Store::new(db);
    store
        .set_usage_limits(
            Some("alice"),
            &Limits {
                running_vcpus: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(root.path());
    let record =
        oa_computers::api::admit_create(&fake.capabilities(), "alice", &intent("legacy")).unwrap();
    let request = oa_computers::store::NewComputer {
        owner: "alice",
        chat: None,
        idempotency_key: Some("legacy"),
        driver: "fake",
        image: None,
        record: &record,
    };
    assert!(matches!(
        store.create(&request).await,
        Err(oa_computers::store::CreateError::Limit)
    ));
    assert!(store.list("alice").await.unwrap().is_empty());
    assert_eq!(
        store.usage("alice", 0, 200).await.unwrap()["allocation_allowance"]["unresolved_allowances"],
        0
    );
    store
        .set_usage_limits(Some("alice"), &Limits::default())
        .await
        .unwrap();
    store.create(&request).await.unwrap();
    let usage = store.usage("alice", 0, 200).await.unwrap();
    assert!(usage["allocation_allowance"]["held_cpu_ms"].is_null());
    assert_eq!(usage["allocation_allowance"]["unknown_allowances"], 1);
}

#[tokio::test]
async fn stop_restart_wake_and_delete_preserve_unknown_usage_and_exact_allowances() {
    let Some((db, dsn)) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async move {
            let store = Store::new(db.clone());
            let root = tempfile::tempdir().unwrap();
            let a = Simulated::new(&root.path().join("a"), false);
            let b = Simulated::new(&root.path().join("b"), true);
            let driver = configured("b", &a, &b);
            store
                .set_usage_limits(
                    Some("alice"),
                    &Limits {
                        computer_count: Some(1),
                        running_vcpus: Some(2),
                        running_memory_mib: Some(2048),
                        boot_cpu_ms: Some(BOOT_CPU_MS),
                        turn_seconds: Some(5),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            let (made, _) = store
                .create_placed("alice", &intent("cycle"), driver.catalog())
                .await
                .unwrap();
            let id = &made.id;
            phase(&db, &store, &driver, id, "awake").await;
            assert!(
                store
                    .resolve_usage_allowance("alice", id, 1, BOOT_CPU_MS, 0, "test policy")
                    .await
                    .is_err()
            );
            assert!(
                store
                    .add_turn(
                        "wrong-owner",
                        id,
                        "mallory",
                        "private command",
                        None,
                        1,
                        &[]
                    )
                    .await
                    .is_err()
            );
            assert!(
                store
                    .add_turn("too-long", id, "alice", "private command", None, 6, &[])
                    .await
                    .is_err()
            );
            assert!(
                store.usage("mallory", 0, 200).await.unwrap()["events"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            store
                .set_desired("alice", id, Desired::Stopped)
                .await
                .unwrap();
            // An accepted stop does not free running capacity.
            assert_eq!(
                store
                    .set_usage_limits(
                        Some("alice"),
                        &Limits {
                            running_vcpus: Some(0),
                            ..Default::default()
                        }
                    )
                    .await
                    .unwrap_err()
                    .0,
                QUOTA_ERROR
            );
            phase(&db, &store, &driver, id, "stopped").await;
            let restarted = Store::new(Db::connect(&dsn).await.unwrap());
            let stopped = restarted.usage("alice", 0, 200).await.unwrap();
            assert_eq!(stopped["allocation_allowance"]["held_cpu_ms"], BOOT_CPU_MS);
            let meters: Vec<_> = stopped["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["kind"] == "meter")
                .collect();
            assert_eq!(meters.len(), 1);
            assert!(meters[0]["evidence"]["provider_seconds"].is_null());
            assert_eq!(meters[0]["evidence"]["measurement"], "unknown");
            assert_eq!(meters[0]["deployment"], "b");
            assert_eq!(meters[0]["shape"]["memory_mib"], 2048);
            assert!(stopped["customer_charge"].is_null());
            assert_eq!(
                restarted
                    .set_desired("alice", id, Desired::Running)
                    .await
                    .unwrap_err()
                    .0,
                QUOTA_ERROR
            );
            assert_eq!(
                restarted.get("alice", id).await.unwrap().unwrap().desired,
                Desired::Stopped
            );
            restarted
                .resolve_usage_allowance(
                    "alice",
                    id,
                    1,
                    BOOT_CPU_MS,
                    0,
                    "synthetic allocation policy",
                )
                .await
                .unwrap();
            restarted
                .resolve_usage_allowance(
                    "alice",
                    id,
                    1,
                    BOOT_CPU_MS,
                    0,
                    "synthetic allocation policy",
                )
                .await
                .unwrap();
            assert!(
                restarted
                    .resolve_usage_allowance("alice", id, 1, BOOT_CPU_MS, 1, "changed policy")
                    .await
                    .is_err()
            );
            restarted
                .set_desired("alice", id, Desired::Running)
                .await
                .unwrap();
            // A wake accepted before its boot starts cannot be settled early.
            assert!(
                restarted
                    .resolve_usage_allowance("alice", id, 2, BOOT_CPU_MS, 0, "early")
                    .await
                    .is_err()
            );
            phase(&db, &restarted, &driver, id, "awake").await;
            assert_eq!(b.creates.lock().unwrap().len(), 1);
            restarted
                .set_desired("alice", id, Desired::Deleted)
                .await
                .unwrap();
            phase(&db, &restarted, &driver, id, "deleted").await;
            let deleted = restarted.usage("alice", 0, 200).await.unwrap();
            assert_eq!(deleted["allocation_allowance"]["held_cpu_ms"], BOOT_CPU_MS);
            assert_eq!(
                deleted["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|e| e["kind"] == "resolution")
                    .count(),
                1
            );
            assert!(
                !serde_json::to_string(&deleted)
                    .unwrap()
                    .contains("private command")
            );
            let replay = restarted
                .create_placed("alice", &intent("cycle"), driver.catalog())
                .await
                .unwrap();
            assert!(!replay.1);
            assert_eq!(replay.0.desired, Desired::Deleted);
            restarted
                .resolve_usage_allowance(
                    "alice",
                    id,
                    2,
                    BOOT_CPU_MS,
                    1,
                    "synthetic allocation policy",
                )
                .await
                .unwrap();
            let usage = restarted.usage("alice", 0, 200).await.unwrap();
            assert_eq!(usage["allocation_allowance"]["held_cpu_ms"], 0);
            assert_eq!(usage["allocation_allowance"]["settled_cpu_ms"], 1);
            let db = restarted.db.client().await.unwrap();
            assert!(
                db.execute(
                    "DELETE FROM computers.usage_events WHERE computer=$1",
                    &[&id]
                )
                .await
                .is_err()
            );
        })
        .await;
}

#[tokio::test]
async fn a_lost_create_reply_retains_its_allowance_and_operation_after_restart() {
    let Some((db, dsn)) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async move {
            let root = tempfile::tempdir().unwrap();
            let a = Simulated::new(&root.path().join("a"), false);
            let b = Simulated::new(&root.path().join("b"), true);
            let driver = configured("a", &a, &b);
            let store = Store::new(db.clone());
            store
                .set_usage_limits(
                    Some("alice"),
                    &Limits {
                        boot_cpu_ms: Some(BOOT_CPU_MS),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            let (made, _) = store
                .create_placed("alice", &intent("lost"), driver.catalog())
                .await
                .unwrap();
            use coder_working_computer::{Command, driver::Held, provider::Provider};
            let lease_seconds = Duration::from_secs(10);
            let entry = store.claim("crashed-worker",lease_seconds).await.unwrap().unwrap();
            let lease = oa_computers::store::PgLease {
                store:store.clone(),id:made.id.clone(),holder:"crashed-worker".into(),lease:lease_seconds,
            };
            let operation = "crash-create".to_owned();
            let current = lease.apply(&Command::RequestCreate { operation:operation.clone() },
                oa_computers::reconcile::now_ms()).await.unwrap();
            let provider = coder_working_computer::on_driver::OnDriver::new(
                driver.resolve(&entry).unwrap(), entry.image.clone());
            a.fake.inject("create", Inject::LoseReply);
            let outcome = Provider::create(&provider,&current,&operation).await;
            assert!(matches!(outcome,Outcome::Unknown { .. }));
            lease.apply(&Command::ObserveCreate { operation,outcome },
                oa_computers::reconcile::now_ms()).await.unwrap();
            assert_eq!(store.get("alice",&made.id).await.unwrap().unwrap().phase,"unknown");
            // A dead worker cannot renew. Move the clock past its lease without
            // releasing any allowance, then start a new database client.
            db.client().await.unwrap().execute(
                "UPDATE computers.computers SET lease_until=now()-interval '1 second',next_attempt_at=now() WHERE id=$1",
                &[&made.id]).await.unwrap();
            let restarted = Store::new(Db::connect(&dsn).await.unwrap());
            assert!(matches!(
                restarted
                    .create_placed("alice", &intent("different"), driver.catalog())
                    .await,
                Err(oa_computers::store::CreateError::Limit)
            ));
            phase(&db, &restarted, &driver, &made.id, "awake").await;
            let calls = a.creates.lock().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].operation, calls[1].operation);
            drop(calls);
            assert!(lease.read().await.is_err());
            let usage = restarted.usage("alice", 0, 200).await.unwrap();
            assert_eq!(usage["allocation_allowance"]["held_cpu_ms"], BOOT_CPU_MS);
            assert_eq!(usage["allocation_allowance"]["unresolved_allowances"], 1);
            restarted
                .set_desired("alice", &made.id, Desired::Deleted)
                .await
                .unwrap();
            phase(&db, &restarted, &driver, &made.id, "deleted").await;
            let usage = restarted.usage("alice",0,200).await.unwrap();
            let events = usage["events"].as_array().unwrap();
            assert!(events.iter().any(|e| e["kind"] == "meter" && e["evidence"]["provider_seconds"].is_u64()));
        })
        .await;
}

#[tokio::test]
async fn usage_reads_stay_account_scoped_and_legacy_chat_wake_keeps_its_200() {
    let Some((db, _)) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async move {
            let store = Store::new(db.clone());
            let root = tempfile::tempdir().unwrap();
            let a = Simulated::new(&root.path().join("a"), false);
            let b = Simulated::new(&root.path().join("b"), true);
            let driver = configured("a", &a, &b);
            let mut request = intent("usage-read");
            request.chat = Some("usage-chat".into());
            let (made, _) = store
                .create_placed("alice", &request, driver.catalog())
                .await
                .unwrap();
            phase(&db, &store, &driver, &made.id, "awake").await;
            store
                .set_desired("alice", &made.id, Desired::Stopped)
                .await
                .unwrap();
            phase(&db, &store, &driver, &made.id, "stopped").await;
            let (url, server) = server(store.clone(), &driver).await;
            let (status, body) = post(
                &format!("{url}/v1/computers"),
                "alice",
                json!({"chat":"usage-chat"}),
            )
            .await;
            assert_eq!(status, 200);
            assert_eq!(body["id"], made.id);
            assert_eq!(
                store.get("alice", &made.id).await.unwrap().unwrap().desired,
                Desired::Running
            );
            let client = reqwest::Client::new();
            assert_eq!(
                client
                    .get(format!("{url}/v1/usage"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                401
            );
            let foreign: Value = client
                .get(format!("{url}/v1/usage?owner=alice"))
                .bearer_auth(TOKEN)
                .header("X-OpenAgents-Account", "mallory")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert!(foreign["events"].as_array().unwrap().is_empty());
            let first: Value = client
                .get(format!("{url}/v1/usage?limit=1"))
                .bearer_auth(TOKEN)
                .header("X-OpenAgents-Account", "alice")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(first["events"].as_array().unwrap().len(), 1);
            let cursor = first["after"].as_i64().unwrap();
            let next: Value = client
                .get(format!("{url}/v1/usage?after={cursor}"))
                .bearer_auth(TOKEN)
                .header("X-OpenAgents-Account", "alice")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert!(!next["events"].as_array().unwrap().is_empty());
            for event in next["events"].as_array().unwrap() {
                assert_eq!(event["owner"], "alice");
                assert!(event["sequence"].as_i64().unwrap() > cursor);
            }
            store
                .set_desired("alice", &made.id, Desired::Deleted)
                .await
                .unwrap();
            phase(&db, &store, &driver, &made.id, "deleted").await;
            server.abort();
        })
        .await;
}

#[tokio::test]
async fn the_free_allowance_bounds_enrolled_accounts_and_yields_to_their_own_policy() {
    use oa_computers::usage::{FREE_BOOT_MS, FreeAllowance};
    let Some((db, _dsn)) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async move {
            let store = Store::new(db.clone());
            let root = tempfile::tempdir().unwrap();
            let a = Simulated::new(&root.path().join("a"), false);
            let b = Simulated::new(&root.path().join("b"), true);
            let driver = configured("b", &a, &b);
            // Off: nobody is enrolled. (`phase` follows alice.)
            store.set_free_allowance(None).await.unwrap();
            assert!(!store.enroll_free("alice").await.unwrap());
            // One free boot's worth: two hours of two vCPUs.
            let one_boot = 2 * FREE_BOOT_MS as i64;
            store
                .set_free_allowance(Some(FreeAllowance {
                    computer_count: 1,
                    boot_cpu_ms: one_boot,
                }))
                .await
                .unwrap();
            assert!(store.enroll_free("alice").await.unwrap());
            assert!(
                store.enroll_free("alice").await.unwrap(),
                "enrolling twice is fine"
            );
            let free = |key: &str| CreateIntent {
                free_allowance: true,
                ..intent(key)
            };
            let (made, _) = store
                .create_placed("alice", &free("one"), driver.catalog())
                .await
                .unwrap();
            assert_eq!(made.record.bounds.absolute_ms, FREE_BOOT_MS);
            // One computer at a time.
            assert!(matches!(
                store
                    .create_placed("alice", &free("two"), driver.catalog())
                    .await,
                Err(oa_computers::store::CreateError::Limit)
            ));
            let usage = store.usage("alice", 0, 200).await.unwrap();
            assert_eq!(usage["free_allowance"]["cpu_ms"], one_boot);
            assert_eq!(usage["free_allowance"]["counted_cpu_ms"], one_boot);
            assert_eq!(usage["free_allowance"]["computer_count"], 1);
            let id = &made.id;
            phase(&db, &store, &driver, id, "awake").await;
            store
                .set_desired("alice", id, Desired::Stopped)
                .await
                .unwrap();
            phase(&db, &store, &driver, id, "stopped").await;
            // The first boot's maximum is still counted, so a second boot is refused.
            assert_eq!(
                store
                    .set_desired("alice", id, Desired::Running)
                    .await
                    .unwrap_err()
                    .0,
                QUOTA_ERROR
            );
            // An account's own policy replaces the free allowance.
            store
                .set_usage_limits(Some("alice"), &Limits::default())
                .await
                .unwrap();
            store
                .set_desired("alice", id, Desired::Running)
                .await
                .unwrap();
            assert!(store.usage("alice", 0, 200).await.unwrap()["free_allowance"].is_null());
            // Accounts never enrolled keep the ordinary limits.
            for key in ["i1", "i2"] {
                store
                    .create_placed("invited", &intent(key), driver.catalog())
                    .await
                    .unwrap();
            }
            assert!(store.usage("invited", 0, 200).await.unwrap()["free_allowance"].is_null());
            // Switched off: an enrolled account without a policy is refused.
            assert!(store.enroll_free("later").await.unwrap());
            store.set_free_allowance(None).await.unwrap();
            assert!(matches!(
                store
                    .create_placed("later", &free("one"), driver.catalog())
                    .await,
                Err(oa_computers::store::CreateError::Limit)
            ));
        })
        .await;
}

#[tokio::test]
async fn the_front_door_enrolls_free_accounts_only_when_the_allowance_is_on() {
    use oa_computers::usage::{FREE_BOOT_MS, FreeAllowance};
    let Some((db, _dsn)) = database().await else {
        return;
    };
    let store = Store::new(db.clone());
    let root = tempfile::tempdir().unwrap();
    let a = Simulated::new(&root.path().join("a"), false);
    let b = Simulated::new(&root.path().join("b"), true);
    let driver = configured("b", &a, &b);
    let (url, task) = server(store.clone(), &driver).await;
    let create = |key: &'static str, allowance: &'static str| {
        let url = url.clone();
        async move {
            let response = reqwest::Client::new()
                .post(format!("{url}/v1/computers"))
                .bearer_auth(TOKEN)
                .header("X-OpenAgents-Account", "free-http")
                .header("X-OpenAgents-Allowance", allowance)
                .json(&json!({"idempotency_key": key}))
                .send()
                .await
                .unwrap();
            (
                response.status().as_u16(),
                response.json::<Value>().await.unwrap(),
            )
        }
    };
    store.set_free_allowance(None).await.unwrap();
    assert_eq!(create("off", "free").await.0, 403);
    store
        .set_free_allowance(Some(FreeAllowance {
            computer_count: 1,
            boot_cpu_ms: 120_000_000,
        }))
        .await
        .unwrap();
    assert_eq!(create("bad", "paid").await.0, 400);
    let (status, made) = create("on", "free").await;
    assert_eq!(status, 202, "{made}");
    assert_eq!(made["lifecycle_policy"]["absolute_ms"], FREE_BOOT_MS);
    assert_eq!(
        store.usage("free-http", 0, 1).await.unwrap()["free_allowance"]["cpu_ms"],
        120_000_000
    );
    task.abort();
}
