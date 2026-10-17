//! Storage admission and cleanup conservation against disposable PostgreSQL.
use super::*;
use oa_computers::{storage::allowance::CleanupProof, usage::Limits};

async fn database() -> Option<(Db, String)> {
    let admin = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL").ok()?;
    Some(Db::scratch(&admin).await.unwrap())
}

fn limits(bytes: i64, checkpoints: i64) -> Limits {
    Limits {
        private_storage_bytes: Some(bytes),
        private_checkpoint_count: Some(checkpoints),
        storage_bytes_per_boot: Some(100),
        checkpoints_per_boot: Some(2),
        ..Default::default()
    }
}

fn proof(e: &oa_computers::store::Entry) -> CleanupProof {
    CleanupProof {
        schema: "openagents.computers.storage-cleanup.v1".into(),
        owner: e.owner.clone(),
        computer: e.id.clone(),
        resources: e
            .record
            .boots
            .iter()
            .map(|b| b.resource.clone())
            .chain(e.record.creates.iter().filter_map(|c| c.resource.clone()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        create_operations: e
            .record
            .creates
            .iter()
            .map(|c| e.binding.as_ref().unwrap().operation(&c.operation))
            .collect(),
        observed_ms: oa_computers::reconcile::now_ms(),
        live_resources: 0,
        retained_objects: 0,
        unfinished_uploads: 0,
        unresolved_operations: 0,
        inventory_receipt_sha256: "a".repeat(64),
    }
}

#[tokio::test]
async fn storage_capacity_is_reserved_before_provider_calls_across_accounts() {
    let Some((db, _)) = database().await else {
        return;
    };
    let store = Store::new(db.clone());
    store.set_usage_limits(None, &limits(200, 4)).await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let a = Simulated::new(&root.path().join("a"), false);
    let b = Simulated::new(&root.path().join("b"), true);
    let driver = configured("b", &a, &b);
    let mut tasks = Vec::new();
    for i in 0..10 {
        let store = store.clone();
        let catalog = driver.catalog().clone();
        tasks.push(tokio::spawn(async move {
            store
                .create_placed(
                    &format!("account-{i}"),
                    &CreateIntent {
                        idempotency_key: Some("same-request".into()),
                        ..Default::default()
                    },
                    &catalog,
                )
                .await
        }));
    }
    let mut accepted = Vec::new();
    for task in tasks {
        match task.await.unwrap() {
            Ok((entry, true)) => accepted.push(entry),
            Err(oa_computers::store::CreateError::Limit) => {}
            other => panic!("unexpected admission: {other:?}"),
        }
    }
    assert_eq!(accepted.len(), 2);
    assert!(a.creates.lock().unwrap().is_empty());
    assert!(b.creates.lock().unwrap().is_empty());
    for e in accepted {
        let (_, fresh) = store
            .create_placed(
                &e.owner,
                &CreateIntent {
                    idempotency_key: Some("same-request".into()),
                    ..Default::default()
                },
                driver.catalog(),
            )
            .await
            .unwrap();
        assert!(!fresh);
        let summary = store.usage(&e.owner, 0, 200).await.unwrap();
        assert_eq!(summary["storage_allowance"]["held_bytes"], 100);
        assert_eq!(summary["storage_allowance"]["held_checkpoints"], 2);
        assert_eq!(summary["storage_allowance"]["held_allowances"], 1);
    }
    assert_eq!(
        db.client()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM computers.storage_reservations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        2
    );
}

#[tokio::test]
async fn deletion_restart_and_partial_inventory_cannot_release_storage() {
    let Some((db, dsn)) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async move {
            let store = Store::new(db.clone());
            store
                .set_usage_limits(Some("alice"), &limits(300, 6))
                .await
                .unwrap();
            let root = tempfile::tempdir().unwrap();
            let a = Simulated::new(&root.path().join("a"), false);
            let b = Simulated::new(&root.path().join("b"), true);
            let driver = configured("b", &a, &b);
            let (e, _) = store
                .create_placed("alice", &CreateIntent::default(), driver.catalog())
                .await
                .unwrap();
            let awake = phase(&db, &store, &driver, &e.id, "awake").await;
            assert_eq!(
                store.usage("alice", 0, 200).await.unwrap()["storage_allowance"]["held_allowances"],
                1
            );
            assert!(
                store
                    .resolve_storage_cleanup("alice", &proof(&awake))
                    .await
                    .is_err()
            );
            store
                .set_desired("alice", &e.id, Desired::Stopped)
                .await
                .unwrap();
            phase(&db, &store, &driver, &e.id, "stopped").await;
            store
                .set_desired("alice", &e.id, Desired::Running)
                .await
                .unwrap();
            store
                .set_desired("alice", &e.id, Desired::Running)
                .await
                .unwrap();
            phase(&db, &store, &driver, &e.id, "awake").await;
            let mut changed = limits(300, 6);
            changed.storage_bytes_per_boot = Some(1);
            changed.checkpoints_per_boot = Some(1);
            store
                .set_usage_limits(Some("alice"), &changed)
                .await
                .unwrap();
            assert_eq!(
                store.usage("alice", 0, 200).await.unwrap()["storage_allowance"]["held_bytes"],
                200
            );
            store
                .set_desired("alice", &e.id, Desired::Deleted)
                .await
                .unwrap();
            let deleted = phase(&db, &store, &driver, &e.id, "deleted").await;
            let restarted = Store::new(Db::connect(&dsn).await.unwrap());
            assert_eq!(
                restarted.usage("alice", 0, 200).await.unwrap()["storage_allowance"]["held_bytes"],
                200
            );
            let complete = proof(&deleted);
            for kind in 0..8 {
                let mut invalid = complete.clone();
                match kind {
                    0 => invalid.retained_objects = 1,
                    1 => invalid.unfinished_uploads = 1,
                    2 => invalid.unresolved_operations = 1,
                    3 => invalid.live_resources = 1,
                    4 => invalid.resources.clear(),
                    5 => invalid.create_operations.clear(),
                    6 => invalid.observed_ms = 0,
                    _ => invalid.owner = "mallory".into(),
                }
                assert!(
                    restarted
                        .resolve_storage_cleanup("alice", &invalid)
                        .await
                        .is_err(),
                    "case {kind}"
                );
            }
            assert!(
                restarted
                    .resolve_storage_cleanup("mallory", &complete)
                    .await
                    .is_err()
            );
            assert!(
                restarted
                    .resolve_storage_cleanup("alice", &complete)
                    .await
                    .unwrap()
            );
            assert!(
                !restarted
                    .resolve_storage_cleanup("alice", &complete)
                    .await
                    .unwrap()
            );
            let mut altered = complete.clone();
            altered.inventory_receipt_sha256 = "b".repeat(64);
            assert!(
                restarted
                    .resolve_storage_cleanup("alice", &altered)
                    .await
                    .is_err()
            );
            let summary = restarted.usage("alice", 0, 200).await.unwrap();
            assert_eq!(summary["storage_allowance"]["held_bytes"], 0);
            assert_eq!(summary["storage_allowance"]["released_allowances"], 2);
            assert!(summary["customer_charge"].is_null());
            assert!(
                db.client()
                    .await
                    .unwrap()
                    .execute("DELETE FROM computers.storage_resolutions", &[])
                    .await
                    .is_err()
            );
        })
        .await;
}

#[tokio::test]
async fn missing_bounds_and_overflow_cannot_appear_as_available_storage() {
    let Some((db, _)) = database().await else {
        return;
    };
    let store = Store::new(db.clone());
    let root = tempfile::tempdir().unwrap();
    let a = Simulated::new(&root.path().join("a"), false);
    let b = Simulated::new(&root.path().join("b"), true);
    let driver = configured("b", &a, &b);
    store
        .set_usage_limits(
            Some("alice"),
            &Limits {
                private_storage_bytes: Some(100),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .create_placed("alice", &CreateIntent::default(), driver.catalog())
            .await,
        Err(oa_computers::store::CreateError::Limit)
    ));
    store
        .set_usage_limits(Some("alice"), &Limits::default())
        .await
        .unwrap();
    store
        .create_placed("alice", &CreateIntent::default(), driver.catalog())
        .await
        .unwrap();
    assert!(
        store.usage("alice", 0, 200).await.unwrap()["storage_allowance"]["held_bytes"].is_null()
    );
    assert!(
        store
            .set_usage_limits(Some("alice"), &limits(100, 2))
            .await
            .is_err()
    );
    let mut large = limits(i64::MAX, 4);
    large.storage_bytes_per_boot = Some(i64::MAX);
    store.set_usage_limits(Some("bob"), &large).await.unwrap();
    store
        .create_placed("bob", &CreateIntent::default(), driver.catalog())
        .await
        .unwrap();
    assert!(matches!(
        store
            .create_placed("bob", &CreateIntent::default(), driver.catalog())
            .await,
        Err(oa_computers::store::CreateError::Limit)
    ));
    assert_eq!(
        store.usage("bob", 0, 200).await.unwrap()["storage_allowance"]["held_bytes"],
        i64::MAX
    );
}

#[tokio::test]
async fn account_cleanup_survives_restart_and_waits_for_private_storage() {
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
            let (e, _) = store
                .create_placed("alice", &CreateIntent::default(), driver.catalog())
                .await
                .unwrap();
            phase(&db, &store, &driver, &e.id, "awake").await;
            let request = serde_json::from_value(json!({
                "command":"private-public-command-canary","timeout_seconds":1,
                "idempotency_key":"cleanup-command","env":{"OA_PARAM_CANARY":"private-env-canary"}
            }))
            .unwrap();
            let (command, _) = store.add_command("alice", &e.id, &request).await.unwrap();
            store
                .add_turn(
                    "cleanup-turn",
                    &e.id,
                    "alice",
                    "private-cleanup-canary",
                    None,
                    1,
                    &[],
                )
                .await
                .unwrap();
            let (other, _) = store
                .create_placed("bob", &CreateIntent::default(), driver.catalog())
                .await
                .unwrap();
            db.client().await.unwrap().execute(
                "INSERT INTO computers.turns(id,computer,owner,state,command,timeout_seconds,finished_at)
                 SELECT 'history-'||n,$1,'alice','done','private-history-canary',1,now()
                 FROM generate_series(1,501) AS n", &[&e.id],
            ).await.unwrap();
            assert!(store.request_account_cleanup("alice").await.unwrap());
            assert!(!store.request_account_cleanup("alice").await.unwrap());
            let restarted = Store::new(Db::connect(&dsn).await.unwrap());
            assert!(
                restarted
                    .create_placed("alice", &CreateIntent::default(), driver.catalog())
                    .await
                    .is_err()
            );
            assert!(
                restarted
                    .add_turn("new-turn", &e.id, "alice", "true", None, 1, &[])
                    .await
                    .is_err()
            );
            restarted.advance_account_cleanup(1).await.unwrap();
            assert_eq!(
                restarted
                    .get("bob", &other.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .desired,
                Desired::Running
            );
            assert!(
                restarted
                    .set_desired("alice", &e.id, Desired::Running)
                    .await
                    .is_ok_and(|changed| !changed)
            );
            let deleted = phase(&db, &restarted, &driver, &e.id, "deleted").await;
            assert_eq!(
                restarted.account_cleanup_status("alice").await.unwrap()["complete"],
                false
            );
            assert!(
                restarted
                    .resolve_storage_cleanup("alice", &proof(&deleted))
                    .await
                    .unwrap()
            );
            db.client()
                .await
                .unwrap()
                .execute(
                    "UPDATE computers.account_cleanup SET next_attempt_at=now()",
                    &[],
                )
                .await
                .unwrap();
            restarted.advance_account_cleanup(1).await.unwrap();
            assert_eq!(restarted.account_cleanup_status("alice").await.unwrap()["complete"],false);
            assert_eq!(db.client().await.unwrap().query_one(
                "SELECT count(*) FROM computers.turns WHERE owner='alice' AND command<>''", &[]
            ).await.unwrap().get::<_,i64>(0),3);
            db.client().await.unwrap().execute(
                "UPDATE computers.account_cleanup SET next_attempt_at=now()", &[]
            ).await.unwrap();
            restarted.advance_account_cleanup(1).await.unwrap();
            assert_eq!(
                restarted.account_cleanup_status("alice").await.unwrap()["complete"],
                true
            );
            assert_eq!(
                db.client()
                    .await
                    .unwrap()
                    .query_one(
                        "SELECT command FROM computers.turns WHERE id='cleanup-turn'",
                        &[]
                    )
                    .await
                    .unwrap()
                    .get::<_, String>(0),
                ""
            );
            let scrubbed = restarted
                .command("alice", &e.id, &command)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(scrubbed.options, json!({}));
            assert!(scrubbed.stdout.is_empty() && scrubbed.stderr.is_empty());
            let (replayed, fresh) = restarted
                .add_command("alice", &e.id, &request)
                .await
                .unwrap();
            assert_eq!(replayed, command);
            assert!(!fresh);
            assert!(
                restarted
                    .create_placed("alice", &CreateIntent::default(), driver.catalog())
                    .await
                    .is_err()
            );
        })
        .await;
}

#[tokio::test]
async fn old_writers_leave_unknown_storage_and_cannot_commit_past_a_new_fence() {
    let Some((db, dsn)) = database().await else {
        return;
    };
    let store = Store::new(db.clone());
    let root = tempfile::tempdir().unwrap();
    let a = Simulated::new(&root.path().join("a"), false);
    let b = Simulated::new(&root.path().join("b"), true);
    let driver = configured("b", &a, &b);
    let (source, _) = store
        .create_placed("alice", &CreateIntent::default(), driver.catalog())
        .await
        .unwrap();
    let (mut old, connection) = tokio_postgres::connect(&dsn, tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    old.batch_execute("SET openagents.usage_writer='v2'")
        .await
        .unwrap();
    const INSERT: &str = "INSERT INTO computers.computers(id,owner,driver,size,desired,phase,revision,record,
        usage_revision,usage_intent_generation,intent_generation,binding)
        SELECT $1,$2,driver,size,desired,phase,revision,record,usage_revision,usage_intent_generation,intent_generation,NULL
        FROM computers.computers WHERE id=$3";
    assert_eq!(
        old.execute(INSERT, &[&"old-uncapped", &"legacy", &source.id])
            .await
            .unwrap(),
        1
    );
    let summary = store.usage("legacy", 0, 200).await.unwrap();
    assert_eq!(summary["storage_allowance"]["unknown_allowances"], 1);
    assert!(summary["storage_allowance"]["held_bytes"].is_null());

    // The old insert has passed its immediate trigger before policy activation.
    let tx = old.transaction().await.unwrap();
    tx.execute(INSERT, &[&"old-pending", &"late-account", &source.id])
        .await
        .unwrap();
    store
        .set_usage_limits(Some("late-account"), &limits(100, 2))
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    assert!(
        store
            .get("late-account", "old-pending")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        old.execute(INSERT, &[&"old-capped", &"late-account", &source.id])
            .await
            .is_err()
    );

    // Commands inserted before closure must also fail at commit, even though
    // the previous protocol is otherwise permitted on this uncapped account.
    let tx = old.transaction().await.unwrap();
    tx.execute(
        "INSERT INTO computers.turns(id,computer,owner,state,command,timeout_seconds)
        VALUES('late-turn',$1,'alice','queued','private-turn-canary',1)",
        &[&source.id],
    )
    .await
    .unwrap();
    store.request_account_cleanup("alice").await.unwrap();
    assert!(tx.commit().await.is_err());
    assert!(
        store
            .turn("alice", &source.id, "late-turn")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        old.execute(
            "UPDATE computers.computers SET next_attempt_at=now() WHERE id=$1",
            &[&source.id]
        )
        .await
        .is_err()
    );
}
