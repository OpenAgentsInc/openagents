//! Storage facts stay with their account and the worker that observed them.

use coder_working_computer::{
    BootOrigin, Computer, Fact, Phase, Trigger,
    driver::{self, Held},
    on_driver::OnDriver,
    store::StoreError,
};
use computer_driver::{
    Artifact, ArtifactKind, CreateRequest, Driver, Outcome, Shape, StorageByteKind, StorageCleanup,
    StorageCoverage, StorageInventory, StorageKind, StorageObject, StorageObjectState,
    StorageObservation, StorageSource, fake::FakeDriver,
};
use oa_computers::{
    Db, Store,
    reconcile::{Reconciler, Settings, now_ms},
    registry::{Catalog, CreateIntent, Drivers, Placement, Registry, Target},
    storage::{DeletedStorageJob, DeletedStorageObserver},
    store::{Desired, Entry, PgLease},
    turns::Secrets,
};
use serde_json::Value;
use std::{rc::Rc, time::Duration};

const HOLDER: &str = "storage-worker";
const LEASE: Duration = Duration::from_secs(60);

struct Rig {
    db: Db,
    dsn: String,
    store: Store,
    registry: Registry<FakeDriver>,
    fake: FakeDriver,
    _root: tempfile::TempDir,
}

impl Rig {
    async fn new() -> Option<Self> {
        let admin = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL").ok()?;
        if admin.is_empty() {
            return None;
        }
        let (db, dsn) = Db::scratch(&admin).await.unwrap();
        let store = Store::new(db.clone());
        let root = tempfile::tempdir().unwrap();
        let fake = FakeDriver::new(root.path()).boot_observations(0);
        let mut caps = fake.capabilities();
        caps.shapes.insert(
            "small".into(),
            Shape {
                vcpus: 2,
                memory_mib: 2048,
                min_disk_gib: None,
            },
        );
        caps.default_artifact = Some(Artifact {
            kind: ArtifactKind::Test,
            reference: "test:empty".into(),
            provider_identity: None,
            disk_gib: None,
        });
        let target = Target::new(
            "storage-fixture",
            "https://storage-fixture.invalid",
            caps,
            ArtifactKind::Test,
            Placement::OurCloud,
        )
        .unwrap();
        let registry = Registry::new(
            Catalog::new("storage-fixture", "storage-test-policy", [target]).unwrap(),
            [("storage-fixture".into(), fake.clone())].into(),
        )
        .unwrap();
        Some(Self {
            db,
            dsn,
            store,
            registry,
            fake,
            _root: root,
        })
    }

    async fn claim(&self, id: &str, holder: &str) -> Entry {
        self.db
            .client()
            .await
            .unwrap()
            .execute(
                "UPDATE computers.computers SET next_attempt_at=now()+interval '1 hour'
             WHERE id<>$1 AND NOT done",
                &[&id],
            )
            .await
            .unwrap();
        self.store.poke(id).await.unwrap();
        let entry = self.store.claim(holder, LEASE).await.unwrap().unwrap();
        assert_eq!(entry.id, id);
        entry
    }

    async fn boot(&self, owner: &str) -> (PgLease, Entry) {
        let (entry, created) = self
            .store
            .create_placed(
                owner,
                &CreateIntent {
                    idempotency_key: Some("storage-computer".into()),
                    ..Default::default()
                },
                self.registry.catalog(),
            )
            .await
            .unwrap();
        assert!(created);
        self.claim(&entry.id, HOLDER).await;
        let lease = PgLease {
            store: self.store.clone(),
            id: entry.id.clone(),
            holder: HOLDER.into(),
            lease: LEASE,
        };
        let mut provider = OnDriver::new(self.registry.resolve(&entry).unwrap(), None);
        provider.poll = Duration::from_millis(1);
        provider.deadline = Duration::from_secs(5);
        let settled = driver::settle(&provider, &lease, Trigger::Wake, now_ms())
            .await
            .unwrap();
        assert_eq!(settled.computer().phase, Phase::Awake);
        let entry = self.store.get(owner, &entry.id).await.unwrap().unwrap();
        assert!(entry.binding.is_some());
        (lease, entry)
    }

    async fn events(&self, owner: &str) -> Vec<Value> {
        storage_events(&self.store, owner).await
    }
}

async fn storage_events(store: &Store, owner: &str) -> Vec<Value> {
    store.usage(owner, 0, 200).await.unwrap()["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["evidence"]["source"] == "provider_storage")
        .cloned()
        .collect()
}

fn observation(computer: &Computer, suffix: &str) -> Outcome<StorageObservation> {
    Outcome::done(StorageObservation {
        computer: computer.resource().unwrap().into(),
        objects: vec![
            StorageObject {
                id: format!("snapshot-{suffix}-unknown"),
                kind: StorageKind::Snapshot,
                state: StorageObjectState::Deleting,
                bytes: None,
                byte_kind: StorageByteKind::ProviderArchive,
                source: StorageSource::GceImage,
            },
            StorageObject {
                id: format!("snapshot-{suffix}-zero"),
                kind: StorageKind::Snapshot,
                state: StorageObjectState::Present,
                bytes: Some(0),
                byte_kind: StorageByteKind::ProviderArchive,
                source: StorageSource::GceImage,
            },
        ],
        inventories: vec![StorageInventory {
            kind: StorageKind::Snapshot,
            coverage: StorageCoverage::Partial,
            source: StorageSource::GceImage,
        }],
        cleanup: StorageCleanup::Pending,
    })
}

async fn record(lease: &PgLease, entry: &Entry, outcome: &Outcome<StorageObservation>) {
    lease
        .observe_storage(
            &entry.record,
            entry
                .binding
                .as_ref()
                .map(|binding| binding.fingerprint.as_str()),
            entry.record.resource().unwrap(),
            outcome,
            now_ms(),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn storage_observations_survive_reconnect_and_remain_account_scoped() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (alice_lease, alice) = rig.boot("alice-storage").await;
            let (bob_lease, bob) = rig.boot("bob-storage").await;
            let alice_outcome = observation(&alice.record, "alice");
            record(&alice_lease, &alice, &alice_outcome).await;
            // A different receive time does not duplicate an identical fact
            // observed at the same lifecycle revision.
            alice_lease
                .observe_storage(
                    &alice.record,
                    alice
                        .binding
                        .as_ref()
                        .map(|binding| binding.fingerprint.as_str()),
                    alice.record.resource().unwrap(),
                    &alice_outcome,
                    now_ms() + 1000,
                )
                .await
                .unwrap();
            record(&bob_lease, &bob, &observation(&bob.record, "bob")).await;

            let reopened = Store::new(Db::connect(&rig.dsn).await.unwrap());
            let alice_events = storage_events(&reopened, "alice-storage").await;
            let bob_events = storage_events(&reopened, "bob-storage").await;
            assert_eq!(alice_events.len(), 1);
            assert_eq!(bob_events.len(), 1);
            assert_eq!(alice_events[0]["computer"], alice.id);
            assert_eq!(alice_events[0]["owner"], "alice-storage");
            assert_eq!(alice_events[0]["deployment"], "storage-fixture");
            assert_eq!(alice_events[0]["revision"], alice.record.revision);
            assert_eq!(
                alice_events[0]["evidence"]["deployment_fingerprint"],
                alice.binding.as_ref().unwrap().fingerprint
            );
            let saved = &alice_events[0]["evidence"]["storage_observation"];
            assert_eq!(saved["computer"], alice.record.resource().unwrap());
            assert!(saved["objects"][0]["bytes"].is_null());
            assert_eq!(saved["objects"][0]["state"], "deleting");
            assert_eq!(saved["objects"][1]["bytes"], 0);
            assert_eq!(saved["objects"][1]["byte_kind"], "provider_archive");
            assert_eq!(saved["inventories"][0]["coverage"], "partial");
            assert_eq!(saved["cleanup"], "pending");
            assert!(alice_events[0]["evidence"]["customer_charge"].is_null());
            assert!(
                !serde_json::to_string(&alice_events)
                    .unwrap()
                    .contains("snapshot-bob")
            );
            assert!(
                !serde_json::to_string(&bob_events)
                    .unwrap()
                    .contains("snapshot-alice")
            );
            assert!(
                storage_events(&reopened, "unrelated-storage")
                    .await
                    .is_empty()
            );
        })
        .await;
}

#[tokio::test]
async fn invalid_storage_and_uncertain_outcomes_never_retain_foreign_ids_or_errors() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (lease, entry) = rig.boot("private-storage").await;
            let mut invalid = observation(&entry.record, "private-object")
                .value()
                .unwrap();
            invalid.objects[0].id = "https://private.invalid/path?token=synthetic-secret".into();
            record(&lease, &entry, &Outcome::done(invalid)).await;
            let mut foreign = observation(&entry.record, "foreign-private-object")
                .value()
                .unwrap();
            foreign.computer = "foreign-resource".into();
            record(&lease, &entry, &Outcome::done(foreign)).await;
            for outcome in [
                Outcome::unknown("synthetic-secret /private/path foreign-private-object"),
                Outcome::failed("synthetic-secret /private/path foreign-private-object"),
            ] {
                record(&lease, &entry, &outcome).await;
            }
            assert!(matches!(
                lease
                    .observe_storage(
                        &entry.record,
                        entry
                            .binding
                            .as_ref()
                            .map(|binding| binding.fingerprint.as_str()),
                        "foreign-resource",
                        &observation(&entry.record, "foreign-private-object"),
                        now_ms(),
                    )
                    .await,
                Err(StoreError::Corrupt(_))
            ));
            let events = rig.events("private-storage").await;
            // Invalid bodies and uncertain error reasons each share one
            // redacted event key at this revision.
            assert_eq!(events.len(), 2);
            assert!(
                events
                    .iter()
                    .all(|event| event["evidence"]["storage_observation"].is_null())
            );
            assert!(
                events
                    .iter()
                    .any(|event| event["evidence"]["status"] == "invalid")
            );
            assert!(
                events
                    .iter()
                    .any(|event| event["evidence"]["status"] == "unknown")
            );
            let retained = rig
                .store
                .usage("private-storage", 0, 200)
                .await
                .unwrap()
                .to_string();
            for private in [
                "synthetic-secret",
                "/private/path",
                "private-object",
                "foreign-resource",
                "private.invalid",
            ] {
                assert!(!retained.contains(private), "retained {private}");
            }
        })
        .await;
}

#[tokio::test]
async fn expired_lost_stale_or_wrong_deployment_observations_are_fenced() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (lease, entry) = rig.boot("fenced-storage").await;
            let outcome = observation(&entry.record, "before-fence");
            let fingerprint = entry.binding.as_ref().unwrap().fingerprint.as_str();
            let wrong = "f".repeat(64);
            assert_ne!(fingerprint, wrong);
            assert!(matches!(
                lease.observe_storage(&entry.record, Some(&wrong), entry.record.resource().unwrap(), &outcome, now_ms()).await,
                Err(StoreError::Busy)
            ));
            let mut changed = entry.record.clone();
            changed.updated_ms += 1;
            assert!(matches!(
                lease.observe_storage(&changed, Some(fingerprint), changed.resource().unwrap(), &outcome, now_ms()).await,
                Err(StoreError::Busy)
            ));
            rig.db.client().await.unwrap().execute(
                "UPDATE computers.computers SET lease_until=now()-interval '1 second' WHERE id=$1", &[&entry.id],
            ).await.unwrap();
            assert!(matches!(
                lease.observe_storage(&entry.record, Some(fingerprint), entry.record.resource().unwrap(), &outcome, now_ms()).await,
                Err(StoreError::Busy)
            ));
            rig.store.renew(&entry.id, HOLDER, LEASE).await.unwrap();
            let provider = OnDriver::new(rig.registry.resolve(&entry).unwrap(), None);
            let next = driver::settle(&provider, &lease, Trigger::Prompt, now_ms()).await.unwrap();
            assert!(next.computer().revision > entry.record.revision);
            assert!(matches!(
                lease.observe_storage(&entry.record, Some(fingerprint), entry.record.resource().unwrap(), &outcome, now_ms()).await,
                Err(StoreError::Busy)
            ));
            let current = lease.read().await.unwrap();
            rig.store.release(&entry.id, HOLDER, Duration::ZERO, 0, None, false).await.unwrap();
            let replacement = rig.claim(&entry.id, "replacement-worker").await;
            assert_eq!(replacement.record, current);
            assert!(matches!(
                lease.observe_storage(&current, Some(fingerprint), current.resource().unwrap(), &observation(&current, "lost-lease"), now_ms()).await,
                Err(StoreError::Busy)
            ));
            assert!(rig.events("fenced-storage").await.is_empty());
            let replacement_lease = PgLease {
                store: rig.store.clone(), id: entry.id.clone(), holder: "replacement-worker".into(), lease: LEASE,
            };
            record(&replacement_lease, &replacement, &observation(&current, "replacement")).await;
            assert_eq!(rig.events("fenced-storage").await.len(), 1);
        })
        .await;
}

#[tokio::test]
async fn historical_resource_cannot_be_recorded_as_the_active_boot() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (lease, entry) = rig.boot("replacement-storage").await;
            let historical_resource = entry.record.resource().unwrap();
            let active_resource = "replacement-resource";
            let at_ms = now_ms();

            // Persist a valid recovery fixture with a deleted old allocation
            // and its replacement, while keeping the old boot in history.
            let mut current = entry.record.clone();
            current.creates.last_mut().unwrap().deletion = Some(Fact::Done {
                evidence: "old-resource-deleted".into(),
                at_ms,
            });
            let mut allocation = current.creates.last().unwrap().clone();
            allocation.operation = format!("{}-replacement", allocation.operation);
            allocation.resource = Some(active_resource.into());
            allocation.fact = Fact::Done {
                evidence: active_resource.into(),
                at_ms,
            };
            allocation.deletion = None;
            let mut boot = current.boots.last().unwrap().clone();
            boot.number += 1;
            boot.resource = active_resource.into();
            boot.origin = BootOrigin::Created;
            boot.started_ms = at_ms;
            current.creates.push(allocation);
            current.boots.push(boot);
            current.revision += 1;
            current.updated_ms = at_ms;
            lease.commit(&entry.record, &current).await.unwrap();
            assert_eq!(lease.read().await.unwrap(), current);
            assert_eq!(current.resource(), Some(active_resource));
            assert!(
                current
                    .boots
                    .iter()
                    .any(|boot| boot.resource == historical_resource)
            );

            let fingerprint = entry.binding.as_ref().unwrap().fingerprint.as_str();
            assert!(matches!(
                lease
                    .observe_storage(
                        &current,
                        Some(fingerprint),
                        historical_resource,
                        &observation(&entry.record, "historical"),
                        at_ms,
                    )
                    .await,
                Err(StoreError::Corrupt(_))
            ));
            assert!(rig.events("replacement-storage").await.is_empty());

            let active = rig
                .store
                .get("replacement-storage", &entry.id)
                .await
                .unwrap()
                .unwrap();
            record(&lease, &active, &observation(&current, "active")).await;
            let events = rig.events("replacement-storage").await;
            assert_eq!(events.len(), 1);
            assert_eq!(events[0]["resource"], active_resource);
            assert_eq!(events[0]["boot"], current.boot().unwrap().number);
            assert_eq!(
                events[0]["evidence"]["storage_observation"]["computer"],
                active_resource
            );
            assert!(!events[0].to_string().contains("snapshot-historical"));
        })
        .await;
}

#[tokio::test]
async fn reconciler_records_unknown_storage_without_inventing_zero_or_cleanup() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (_, entry) = rig.boot("reconciled-storage").await;
            rig.store
                .release(&entry.id, HOLDER, Duration::ZERO, 0, None, false)
                .await
                .unwrap();
            let entry = rig.claim(&entry.id, "reconciler-storage-worker").await;
            let mut settings = Settings::new("reconciler-storage-worker");
            settings.lease = LEASE;
            settings.revisit = Duration::ZERO;
            settings.backoff = Duration::ZERO;
            settings.driver_poll = Duration::from_millis(1);
            settings.driver_deadline = Duration::from_secs(5);
            tokio::time::timeout(
                Duration::from_secs(10),
                Rc::new(Reconciler {
                    store: rig.store.clone(),
                    driver: rig.registry.clone(),
                    settings,
                    turns: Secrets::default(),
                })
                .visit(entry),
            )
            .await
            .unwrap();
            let events = rig.events("reconciled-storage").await;
            assert_eq!(events.len(), 1);
            assert_eq!(events[0]["evidence"]["status"], "unknown");
            assert!(events[0]["evidence"]["storage_observation"].is_null());
            assert!(events[0]["evidence"]["customer_charge"].is_null());
            assert!(!events[0]["resource"].is_null());
        })
        .await;
}

async fn deleted_entry(rig: &Rig, owner: &str) -> (Entry, String) {
    let (lease, original) = rig.boot(owner).await;
    let resource = original.record.resource().unwrap().to_owned();
    rig.store
        .set_desired(owner, &original.id, Desired::Deleted)
        .await
        .unwrap();
    let e = rig.store.get(owner, &original.id).await.unwrap().unwrap();
    let mut provider = OnDriver::new(rig.registry.resolve(&e).unwrap(), None);
    provider.poll = Duration::from_millis(1);
    provider.deadline = Duration::from_secs(5);
    let result = driver::delete(&provider, &lease, now_ms()).await.unwrap();
    assert_eq!(result.computer().phase, Phase::Deleted);
    assert_eq!(result.computer().resource(), None);
    rig.store
        .release(&e.id, HOLDER, Duration::ZERO, 0, None, true)
        .await
        .unwrap();
    (
        rig.store.get(owner, &e.id).await.unwrap().unwrap(),
        resource,
    )
}

async fn lifecycle_row(rig: &Rig, id: &str) -> Value {
    rig.db
        .client()
        .await
        .unwrap()
        .query_one(
            "SELECT to_jsonb(c) FROM computers.computers c WHERE id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0)
}

async fn due(rig: &Rig, job: &DeletedStorageJob) {
    rig.db.client().await.unwrap().execute(
        "UPDATE computers.deleted_storage_jobs SET next_attempt_at=now()-interval '1 second',lease_until=now()-interval '1 second' WHERE id=$1", &[&job.id],
    ).await.unwrap();
}

fn empty_storage(resource: &str) -> Outcome<StorageObservation> {
    Outcome::done(StorageObservation {
        computer: resource.into(),
        objects: vec![],
        inventories: vec![StorageInventory {
            kind: StorageKind::Snapshot,
            coverage: StorageCoverage::Partial,
            source: StorageSource::GceImage,
        }],
        cleanup: StorageCleanup::Unknown,
    })
}

#[tokio::test]
async fn deleted_storage_scan_wraps_and_finds_done_and_prior_live_allocations() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (alice_lease, alice) = rig.boot("deleted-storage-alice").await;
            // The cursor has already passed this row before its deletion appears.
            assert_eq!(rig.store.seed_deleted_storage(1).await.unwrap(), 0);
            let old = alice.record.resource().unwrap().to_owned();
            let mut replacement = alice.record.clone();
            let at_ms = now_ms();
            replacement.creates.last_mut().unwrap().deletion = Some(Fact::Done {
                evidence: "deleted-old".into(),
                at_ms,
            });
            let mut create = replacement.creates.last().unwrap().clone();
            create.operation.push_str("-replacement");
            let replacement_resource = rig
                .registry
                .resolve(&alice)
                .unwrap()
                .create(&CreateRequest {
                    operation: create.operation.clone(),
                    size: Some(alice.size.clone()),
                    image: None,
                    ttl_seconds: None,
                    labels: Default::default(),
                })
                .await
                .value()
                .unwrap();
            create.resource = Some(replacement_resource.clone());
            create.fact = Fact::Done {
                evidence: replacement_resource.clone(),
                at_ms,
            };
            create.deletion = None;
            replacement.creates.push(create);
            let mut boot = replacement.boots.last().unwrap().clone();
            boot.number += 1;
            boot.resource = replacement_resource;
            boot.origin = BootOrigin::Created;
            boot.started_ms = at_ms;
            replacement.boots.push(boot);
            replacement.revision += 1;
            replacement.updated_ms = at_ms;
            alice_lease
                .commit(&alice.record, &replacement)
                .await
                .unwrap();
            let (bob, bob_resource) = deleted_entry(&rig, "deleted-storage-bob").await;
            let alice_before = lifecycle_row(&rig, &alice.id).await;
            let bob_before = lifecycle_row(&rig, &bob.id).await;
            let mut inserted = 0;
            for _ in 0..8 {
                inserted += rig.store.seed_deleted_storage(1).await.unwrap();
            }
            assert_eq!(inserted, 2);
            let rows = rig.db.client().await.unwrap().query(
            "SELECT computer,resource FROM computers.deleted_storage_jobs ORDER BY computer", &[],
        ).await.unwrap();
            assert_eq!(rows.len(), 2);
            assert!(
                rows.iter()
                    .any(|r| r.get::<_, String>(0) == alice.id && r.get::<_, String>(1) == old)
            );
            assert!(
                rows.iter()
                    .any(|r| r.get::<_, String>(0) == bob.id
                        && r.get::<_, String>(1) == bob_resource)
            );
            assert_eq!(lifecycle_row(&rig, &alice.id).await, alice_before);
            assert_eq!(lifecycle_row(&rig, &bob.id).await, bob_before);
            // The whole computer can finish after the historical job was seeded.
            rig.store
                .set_desired(&alice.owner, &alice.id, Desired::Deleted)
                .await
                .unwrap();
            let active = rig
                .store
                .get(&alice.owner, &alice.id)
                .await
                .unwrap()
                .unwrap();
            let mut provider = OnDriver::new(rig.registry.resolve(&active).unwrap(), None);
            provider.poll = Duration::from_millis(1);
            provider.deadline = Duration::from_secs(5);
            let later = driver::delete(&provider, &alice_lease, now_ms())
                .await
                .unwrap();
            assert_eq!(later.computer().phase, Phase::Deleted);
            rig.store
                .release(&alice.id, HOLDER, Duration::ZERO, 0, None, true)
                .await
                .unwrap();
            let alice_before = lifecycle_row(&rig, &alice.id).await;
            let observer = DeletedStorageObserver {
                store: rig.store.clone(),
                driver: rig.registry.clone(),
                holder: "separate-observer".into(),
            };
            let calls = rig.fake.calls();
            while let Some(job) = rig
                .store
                .claim_deleted_storage("separate-observer", LEASE)
                .await
                .unwrap()
            {
                observer.visit(&job).await;
            }
            assert_eq!(rig.fake.calls(), calls);
            assert_eq!(lifecycle_row(&rig, &alice.id).await, alice_before);
            assert_eq!(lifecycle_row(&rig, &bob.id).await, bob_before);
            let alice_events = rig.events("deleted-storage-alice").await;
            let bob_events = rig.events("deleted-storage-bob").await;
            assert_eq!(alice_events.len(), 1);
            assert_eq!(bob_events.len(), 1);
            assert_eq!(alice_events[0]["resource"], old);
            assert_eq!(
                alice_events[0]["operation"],
                alice.record.creates[0].operation
            );
            assert_eq!(
                alice_events[0]["evidence"]["provider_operation"],
                alice
                    .binding
                    .as_ref()
                    .unwrap()
                    .operation(&alice.record.creates[0].operation)
            );
            assert_eq!(bob_events[0]["resource"], bob_resource);
            assert!(alice_events[0]["boot"].is_null());
            assert_eq!(alice_events[0]["revision"], later.computer().revision);
            assert_eq!(alice_events[0]["phase"], "deleted");
            assert_eq!(alice_events[0]["desired"], "deleted");
            assert_eq!(
                alice_events[0]["evidence"]["lifecycle_ms"],
                later.computer().updated_ms
            );
            assert_eq!(alice_events[0]["evidence"]["status"], "unknown");
            assert!(alice_events[0]["evidence"]["storage_observation"].is_null());
            assert!(rig.events("another-account").await.is_empty());
        })
        .await;
}

#[tokio::test]
async fn deleted_storage_retries_survive_reconnect_without_reopening_or_releasing_capacity() {
    tokio::task::LocalSet::new().run_until(async {
        let Some(rig) = Rig::new().await else { return };
        let (entry,resource)=deleted_entry(&rig,"deleted-storage-retry").await;
        let before=lifecycle_row(&rig,&entry.id).await;
        let allowance=rig.store.usage(&entry.owner,0,200).await.unwrap()["allocation_allowance"].clone();
        assert_eq!(rig.store.seed_deleted_storage(100).await.unwrap(),1);
        let reopened=Store::new(Db::connect(&rig.dsn).await.unwrap());
        let first=reopened.claim_deleted_storage("reader",LEASE).await.unwrap().unwrap();
        let outcome=empty_storage(&resource);
        // A provider clock cannot move the compound event into the future.
        let provider_at=now_ms()+86_400_000;
        reopened.observe_deleted_storage(&first,&outcome,provider_at,Duration::from_secs(60)).await.unwrap();
        let recorded=rig.events(&entry.owner).await;
        assert_eq!(recorded[0]["evidence"]["provider_observed_ms"],provider_at);
        assert!(recorded[0]["observed_ms"].as_u64().unwrap()<provider_at);
        assert!(reopened.claim_deleted_storage("reader",LEASE).await.unwrap().is_none());
        due(&rig,&first).await;
        let replay=reopened.claim_deleted_storage("reader",LEASE).await.unwrap().unwrap();
        assert!(replay.generation>first.generation);
        reopened.observe_deleted_storage(&replay,&outcome,now_ms()+1,Duration::from_secs(60)).await.unwrap();
        assert_eq!(rig.events(&entry.owner).await.len(),1);
        for outcome in [
            Outcome::unknown("synthetic-secret /private/path"),
            Outcome::failed("synthetic-secret /private/path"),
            empty_storage("foreign-resource"),
        ] {
            due(&rig,&replay).await;
            let job=reopened.claim_deleted_storage("reader",LEASE).await.unwrap().unwrap();
            reopened.observe_deleted_storage(&job,&outcome,now_ms(),Duration::from_secs(60)).await.unwrap();
        }
        let events=rig.events(&entry.owner).await;
        assert_eq!(events.len(),3);
        assert!(!serde_json::to_string(&events).unwrap().contains("synthetic-secret"));
        assert!(!serde_json::to_string(&events).unwrap().contains("foreign-resource"));
        assert!(events.iter().all(|event|event["resource"]==resource && event["boot"].is_null() && event["evidence"]["customer_charge"].is_null()));
        assert_eq!(lifecycle_row(&rig,&entry.id).await,before);
        assert_eq!(reopened.usage(&entry.owner,0,200).await.unwrap()["allocation_allowance"],allowance);
        assert!(rig.store.claim("lifecycle-worker",LEASE).await.unwrap().is_none());
        let state=rig.db.client().await.unwrap().query_one(
            "SELECT lease_holder,next_attempt_at>clock_timestamp(),attempts FROM computers.deleted_storage_jobs WHERE id=$1", &[&first.id],
        ).await.unwrap();
        assert!(state.get::<_,Option<String>>(0).is_none());
        assert!(state.get::<_,bool>(1));
        assert_eq!(state.get::<_,i32>(2),3);
    }).await;
}

#[tokio::test]
async fn deleted_storage_fences_same_holder_reuse_and_forged_assignment() {
    tokio::task::LocalSet::new().run_until(async {
        let Some(rig) = Rig::new().await else { return };
        let (entry,resource)=deleted_entry(&rig,"deleted-storage-fenced").await;
        rig.store.seed_deleted_storage(100).await.unwrap();
        let old=rig.store.claim_deleted_storage("same-holder",LEASE).await.unwrap().unwrap();
        assert!(rig.store.claim_deleted_storage("second-worker",LEASE).await.unwrap().is_none());
        due(&rig,&old).await;
        let current=rig.store.claim_deleted_storage("same-holder",LEASE).await.unwrap().unwrap();
        let outcome=empty_storage(&resource);
        assert!(rig.store.load_deleted_storage(&old).await.is_err());
        assert!(rig.store.observe_deleted_storage(&old,&outcome,now_ms(),Duration::from_secs(1)).await.is_err());
        assert!(rig.store.retry_deleted_storage(&old,Duration::from_secs(1)).await.is_err());
        for forged in [
            DeletedStorageJob { resource:"foreign-resource".into(), ..current.clone() },
            DeletedStorageJob { computer:"another-computer".into(), ..current.clone() },
            DeletedStorageJob { binding_fingerprint:"f".repeat(64), ..current.clone() },
        ] {
            assert!(rig.store.load_deleted_storage(&forged).await.is_err());
            assert!(rig.store.observe_deleted_storage(&forged,&outcome,now_ms(),Duration::from_secs(1)).await.is_err());
        }
        assert!(rig.db.client().await.unwrap().execute(
            "UPDATE computers.deleted_storage_jobs SET context=jsonb_set(context,'{owner}','\"foreign-account\"') WHERE id=$1", &[&current.id],
        ).await.is_err());
        assert!(rig.events(&entry.owner).await.is_empty());
        rig.store.observe_deleted_storage(&current,&outcome,now_ms(),Duration::from_secs(1)).await.unwrap();
        assert_eq!(rig.events(&entry.owner).await.len(),1);
    }).await;
}

#[tokio::test]
async fn deleted_storage_expiry_is_checked_after_waiting_for_the_usage_lock() {
    tokio::task::LocalSet::new().run_until(async {
        let Some(rig) = Rig::new().await else { return };
        let (entry,resource)=deleted_entry(&rig,"deleted-storage-expired").await;
        rig.store.seed_deleted_storage(100).await.unwrap();
        let job=rig.store.claim_deleted_storage("waiting-reader",LEASE).await.unwrap().unwrap();
        let (mut blocker,connection)=tokio_postgres::connect(&rig.dsn,tokio_postgres::NoTls).await.unwrap();
        tokio::spawn(async move { let _=connection.await; });
        let tx=blocker.transaction().await.unwrap();
        tx.query_one("SELECT pg_advisory_xact_lock(11815)",&[]).await.unwrap();
        let store=rig.store.clone();
        let pending=job.clone();
        let writer=tokio::task::spawn_local(async move {
            store.observe_deleted_storage(&pending,&empty_storage(&resource),now_ms(),Duration::from_secs(1)).await
        });
        tokio::time::timeout(Duration::from_secs(3),async {
            loop {
                let waiting:bool=rig.db.client().await.unwrap().query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND position('pg_advisory_xact_lock(11815)' in query)>0 AND pid<>pg_backend_pid())",&[],
                ).await.unwrap().get(0);
                if waiting { break }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.unwrap();
        due(&rig,&job).await;
        tx.commit().await.unwrap();
        assert!(writer.await.unwrap().is_err());
        assert!(rig.events(&entry.owner).await.is_empty());
        let next=rig.store.claim_deleted_storage("waiting-reader",LEASE).await.unwrap().unwrap();
        assert!(next.generation>job.generation);
    }).await;
}
