//! Operator retention limits delete observed stopped computers, never unknown ones.

use coder_working_computer::{Phase, Trigger, driver, on_driver::OnDriver};
use computer_driver::{
    Driver, State,
    fake::{FakeDriver, Inject},
};
use oa_computers::{
    Db, Store,
    reconcile::{Reconciler, Settings},
    registry::CreateIntent,
    store::{Desired, NewComputer, PgLease},
    turns::Secrets,
    usage::{Limits, QUOTA_ERROR},
};
use std::{rc::Rc, time::Duration};

const HOLDER: &str = "retention-worker";
const LEASE: Duration = Duration::from_secs(30);

async fn claim(db: &Db, store: &Store, id: &str) -> oa_computers::store::Entry {
    db.client().await.unwrap().execute(
        "UPDATE computers.computers SET next_attempt_at=now()+interval '1 hour' WHERE id<>$1 AND NOT done", &[&id],
    ).await.unwrap();
    store.poke(id).await.unwrap();
    let entry = store.claim(HOLDER, LEASE).await.unwrap().unwrap();
    assert_eq!(entry.id, id);
    entry
}

async fn stopped(
    db: &Db,
    store: &Store,
    fake: &FakeDriver,
    owner: &str,
    age_seconds: u64,
    uncertain_stop: bool,
) -> (String, String) {
    let now = oa_computers::reconcile::now_ms() - age_seconds * 1000;
    let mut record =
        oa_computers::api::admit_create(&fake.capabilities(), owner, &CreateIntent::default())
            .unwrap();
    record.created_ms = now;
    record.updated_ms = now;
    let (entry, _) = store
        .create(&NewComputer {
            owner,
            chat: None,
            idempotency_key: None,
            driver: "fake",
            image: None,
            record: &record,
        })
        .await
        .unwrap();
    claim(db, store, &entry.id).await;
    let lease = PgLease {
        store: store.clone(),
        id: entry.id.clone(),
        holder: HOLDER.into(),
        lease: LEASE,
    };
    let provider = OnDriver::new(fake.clone(), None);
    let awake = driver::settle(&provider, &lease, Trigger::Wake, now)
        .await
        .unwrap();
    assert_eq!(awake.computer().phase, Phase::Awake);
    let resource = awake.computer().resource().unwrap().to_owned();
    store
        .set_desired(owner, &entry.id, Desired::Stopped)
        .await
        .unwrap();
    if uncertain_stop {
        fake.inject("suspend", Inject::LoseRequest);
        for _ in 0..3 {
            fake.inject("observe", Inject::LoseRequest);
        }
    }
    let stopped = driver::stop(&provider, &lease, now).await.unwrap();
    if uncertain_stop {
        assert!(matches!(stopped.computer().phase, Phase::Unknown { .. }));
    } else {
        assert_eq!(stopped.computer().phase, Phase::Stopped);
    }
    store
        .release(&entry.id, HOLDER, Duration::ZERO, 0, None, false)
        .await
        .unwrap();
    (entry.id, resource)
}

async fn visit(db: &Db, store: &Store, fake: &FakeDriver, id: &str, configured_retention: u64) {
    let entry = claim(db, store, id).await;
    let mut settings = Settings::new(HOLDER);
    settings.lease = LEASE;
    settings.keep_stopped = Duration::from_secs(configured_retention);
    settings.driver_poll = Duration::from_millis(1);
    settings.driver_deadline = Duration::from_secs(1);
    settings.revisit = Duration::from_millis(1);
    settings.backoff = Duration::from_millis(1);
    Rc::new(Reconciler {
        store: store.clone(),
        driver: fake.clone(),
        settings,
        turns: Secrets::default(),
    })
    .visit(entry)
    .await;
}

#[tokio::test]
async fn retention_uses_the_shortest_limit_and_keeps_uncertain_cleanup_held() {
    let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    tokio::task::LocalSet::new().run_until(async move {
        let store = Store::new(db.clone());
        let root = tempfile::tempdir().unwrap();
        let fake = FakeDriver::new(root.path()).boot_observations(0);
        store.set_usage_limits(None, &Limits {
            stopped_retention_seconds: Some(7200), ..Default::default()
        }).await.unwrap();
        for (owner, seconds) in [("account-limit", 1800), ("service-limit", 10800), ("configured-limit", i64::MAX), ("unknown-stop", 0)] {
            store.set_usage_limits(Some(owner), &Limits {
                stopped_retention_seconds: Some(seconds), ..Default::default()
            }).await.unwrap();
        }
        assert!(store.set_usage_limits(Some("account-limit"), &Limits {
            stopped_retention_seconds: Some(-1), ..Default::default()
        }).await.is_err());
        assert!(serde_json::from_str::<Limits>("{\"stopped_retention_seconds\":9223372036854775808}").is_err());

        let (account, _) = stopped(&db, &store, &fake, "account-limit", 3600, false).await;
        let (service, _) = stopped(&db, &store, &fake, "service-limit", 8000, false).await;
        let (configured, _) = stopped(&db, &store, &fake, "configured-limit", 3600, false).await;
        let (kept, kept_resource) = stopped(&db, &store, &fake, "no-account-limit", 3600, false).await;
        let (unknown, unknown_resource) = stopped(&db, &store, &fake, "unknown-stop", 3600, true).await;

        fake.inject("delete", Inject::LoseReply);
        visit(&db, &store, &fake, &account, 7 * 86400).await;
        let deleting = store.get("account-limit", &account).await.unwrap().unwrap();
        assert_eq!(deleting.desired, Desired::Deleted);
        assert_eq!(deleting.phase, "deleting");
        assert_eq!(store.set_usage_limits(Some("account-limit"), &Limits {
            computer_count: Some(0), ..Default::default()
        }).await.unwrap_err().0, QUOTA_ERROR);
        visit(&db, &store, &fake, &account, 7 * 86400).await;
        assert_eq!(store.get("account-limit", &account).await.unwrap().unwrap().phase, "deleted");
        store.set_usage_limits(Some("account-limit"), &Limits {
            computer_count: Some(0), ..Default::default()
        }).await.unwrap();
        let usage = store.usage("account-limit", 0, 200).await.unwrap();
        assert_eq!(usage["allocation_allowance"]["unresolved_allowances"], 1);
        assert!(usage["allocation_allowance"]["held_cpu_ms"].is_null());

        visit(&db, &store, &fake, &service, 7 * 86400).await;
        assert_eq!(store.get("service-limit", &service).await.unwrap().unwrap().phase, "deleted");
        store.set_usage_limits(None, &Limits::default()).await.unwrap();
        visit(&db, &store, &fake, &configured, 1800).await;
        assert_eq!(store.get("configured-limit", &configured).await.unwrap().unwrap().phase, "deleted");
        visit(&db, &store, &fake, &kept, 7 * 86400).await;
        assert_eq!(store.get("no-account-limit", &kept).await.unwrap().unwrap().phase, "stopped");
        assert!(fake.live().contains(&(kept_resource, State::Stopped)));

        fake.inject("observe", Inject::LoseRequest);
        visit(&db, &store, &fake, &unknown, 0).await;
        let uncertain = store.get("unknown-stop", &unknown).await.unwrap().unwrap();
        assert_eq!(uncertain.desired, Desired::Stopped);
        assert_eq!(uncertain.phase, "unknown");
        assert!(fake.live().contains(&(unknown_resource, State::Running)));
        assert_eq!(store.usage("unknown-stop", 0, 200).await.unwrap()["allocation_allowance"]["unresolved_allowances"], 1);
    }).await;
}

#[tokio::test]
async fn a_saved_retention_ceiling_survives_later_meter_reads_and_a_longer_default() {
    use coder_working_computer::{Command, driver::Held};
    use computer_driver::{Meter, Outcome};
    let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
        return;
    };
    let (db, _) = Db::scratch(&admin).await.unwrap();
    tokio::task::LocalSet::new()
        .run_until(async move {
            let store = Store::new(db.clone());
            let root = tempfile::tempdir().unwrap();
            let fake = FakeDriver::new(root.path()).boot_observations(0);
            fake.inject("meter", Inject::LoseRequest);
            let (id, _) = stopped(&db, &store, &fake, "saved-retention", 8 * 86400, false).await;
            let entry = claim(&db, &store, &id).await;
            assert_eq!(entry.stopped_retention_ms, Some(7 * 86400 * 1000));
            let lease = PgLease {
                store: store.clone(),
                id: id.clone(),
                holder: HOLDER.into(),
                lease: LEASE,
            };
            let updated = lease
                .apply(
                    &Command::ObserveMeter {
                        boot: entry.record.boot().unwrap().number,
                        outcome: Outcome::done(Meter {
                            running: false,
                            evidence: "later meter read".into(),
                            seconds: None,
                        }),
                    },
                    oa_computers::reconcile::now_ms(),
                )
                .await
                .unwrap();
            assert!(updated.updated_ms > entry.record.updated_ms);
            store
                .release(&id, HOLDER, Duration::ZERO, 0, None, false)
                .await
                .unwrap();
            assert!(
                db.client()
                    .await
                    .unwrap()
                    .execute(
                        "UPDATE computers.computers SET stopped_retention_ms=864000000 WHERE id=$1",
                        &[&id],
                    )
                    .await
                    .is_err()
            );
            visit(&db, &store, &fake, &id, 10 * 86400).await;
            let deleted = store.get("saved-retention", &id).await.unwrap().unwrap();
            assert_eq!(deleted.phase, "deleted");
            assert!(fake.live().is_empty());
        })
        .await;
}
