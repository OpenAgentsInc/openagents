use base64::Engine;
use lab_jobs::{
    Capabilities, Placement,
    authority::Authority,
    coordinator::Coordinator,
    protocol::{Attempt, Finish, Output},
};
use lab_kit::{ArtifactRole, JobStatus, LabKind, SpaceRef, Want, echo::Echo};
use lab_store::{ArtifactInput, CheckedDraft, LabStore, Store, backend::FsBackend};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

struct Fixture {
    _tmp: tempfile::TempDir,
    store: Store,
    coordinator: Coordinator,
    kinds: BTreeMap<String, lab_kit::KindManifest>,
    worker: lab_jobs::authority::Worker,
    job: String,
}
impl Fixture {
    async fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::open(tmp.path()).unwrap());
        let space = SpaceRef::Personal {
            account: "alice".into(),
        };
        let store = Store::new(backend.clone(), &space).unwrap();
        let version = store
            .create(
                "d",
                "Echo",
                "alice",
                CheckedDraft::initial::<Echo>(
                    "echo.v1",
                    json!({"text":"hello"}),
                    "Echo",
                    &json!(null),
                )
                .unwrap(),
                1,
            )
            .await
            .unwrap();
        let manifest = Echo::manifest();
        let job = store
            .create_job(
                Echo::jobs_for(&version, &Want::Card).remove(0),
                &manifest,
                2,
            )
            .await
            .unwrap();
        let authority = Authority::new(backend);
        let issued = authority
            .enroll(
                "alice",
                space,
                Capabilities {
                    worker: "test".into(),
                    placement: Placement::Server,
                    cpu_millis: 1000,
                    memory_bytes: 128 * 1024 * 1024,
                    gpu: false,
                    tools: Default::default(),
                    jobs: ["echo/shout".into()].into(),
                    local_assets: Default::default(),
                    max_seconds: 600,
                },
                100,
                60_000,
            )
            .await
            .unwrap();
        Self {
            _tmp: tmp,
            store,
            coordinator: Coordinator { authority },
            kinds: [("echo".into(), manifest)].into(),
            worker: issued.worker,
            job: job.id,
        }
    }
    async fn claim_start(&self) -> lab_jobs::protocol::Work {
        let work = self
            .coordinator
            .poll(&self.worker, &self.store, &self.kinds, 101)
            .await
            .unwrap()
            .unwrap();
        self.coordinator
            .start(
                &self.worker,
                &self.store,
                &self.kinds,
                &Attempt {
                    job: work.assignment.lease.job.id,
                    attempt: work.assignment.lease.attempt,
                },
                102,
            )
            .await
            .unwrap()
    }
    fn finish(&self, attempt: u64) -> Finish {
        Finish {
            job: self.job.clone(),
            attempt,
            status: JobStatus::Done,
            outputs: vec![Output {
                role: ArtifactRole::Export,
                media_type: "text/plain".into(),
                content_base64: base64::engine::general_purpose::STANDARD.encode("HELLO"),
            }],
        }
    }
}

#[tokio::test]
async fn scoped_dispatch_finish_retry_and_usage_survive_restart() {
    let f = Fixture::new().await;
    let work = f.claim_start().await;
    let req = f.finish(work.assignment.lease.attempt);
    let done = f
        .coordinator
        .finish(&f.worker, &f.store, &f.kinds, &req, 103)
        .await
        .unwrap();
    assert_eq!(done.status, JobStatus::Done);
    assert_eq!(
        f.store
            .artifact_bytes(&done.artifacts[0], None)
            .await
            .unwrap(),
        b"HELLO"
    );
    let restarted = Coordinator {
        authority: f.coordinator.authority.clone(),
    };
    assert_eq!(
        restarted
            .finish(&f.worker, &f.store, &f.kinds, &req, 40_000)
            .await
            .unwrap(),
        done
    );
    let usage = restarted
        .authority
        .usage(&f.worker.space, &f.job)
        .await
        .unwrap();
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].charge, None);
    assert_eq!(usage[0].observed_wall_ms, Some(1));
    let mut changed = req;
    changed.outputs[0].content_base64 = "Zm9yZ2Vk".into();
    assert!(
        restarted
            .finish(&f.worker, &f.store, &f.kinds, &changed, 40_001)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_crash_after_result_publication_recovers_the_same_finish_even_after_lease_expiry() {
    let f = Fixture::new().await;
    let work = f.claim_start().await;
    let artifact = f
        .store
        .put_job_artifact(
            &work.assignment.lease,
            ArtifactInput {
                version: work.version.reference(),
                role: ArtifactRole::Export,
                media_type: "text/plain".into(),
                class: lab_kit::AssetClass::Public,
                no_training: false,
            },
            b"HELLO".to_vec(),
            1024,
            103,
        )
        .await
        .unwrap();
    f.store
        .finish(
            &work.assignment.lease,
            JobStatus::Done,
            None,
            vec![artifact.id],
            104,
        )
        .await
        .unwrap();
    // The account-service key and store survive; the coordinator missed its own receipt write.
    let req = f.finish(work.assignment.lease.attempt);
    let recovered = f
        .coordinator
        .finish(&f.worker, &f.store, &f.kinds, &req, 40_000)
        .await
        .unwrap();
    assert_eq!(recovered.status, JobStatus::Done);
    assert_eq!(
        f.coordinator
            .authority
            .usage(&f.worker.space, &f.job)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn cancelling_started_work_or_losing_its_lease_never_requeues_effects() {
    let f = Fixture::new().await;
    let work = f.claim_start().await;
    f.store.cancel(&f.job, 103).await.unwrap();
    let attempt = Attempt {
        job: f.job.clone(),
        attempt: work.assignment.lease.attempt,
    };
    assert!(
        f.coordinator
            .heartbeat(&f.worker, &f.store, &f.kinds, &attempt, 104)
            .await
            .is_err()
    );
    assert!(
        f.coordinator
            .finish(
                &f.worker,
                &f.store,
                &f.kinds,
                &f.finish(attempt.attempt),
                105
            )
            .await
            .is_err()
    );
    assert!(
        f.coordinator
            .poll(&f.worker, &f.store, &f.kinds, 40_000)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        f.store.job(&f.job).await.unwrap().status,
        JobStatus::Uncertain
    );
}

#[tokio::test]
async fn an_unstarted_claim_can_expire_without_starting_another_command() {
    let f = Fixture::new().await;
    let old = f
        .coordinator
        .poll(&f.worker, &f.store, &f.kinds, 101)
        .await
        .unwrap()
        .unwrap();
    let new = f
        .coordinator
        .poll(&f.worker, &f.store, &f.kinds, 40_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        new.assignment.lease.attempt,
        old.assignment.lease.attempt + 1
    );
    assert!(!new.assignment.lease.started);
    assert!(
        f.coordinator
            .start(
                &f.worker,
                &f.store,
                &f.kinds,
                &Attempt {
                    job: f.job.clone(),
                    attempt: old.assignment.lease.attempt
                },
                40_001
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_worker_cannot_poll_another_authorized_stores_space() {
    let f = Fixture::new().await;
    let other = tempfile::tempdir().unwrap();
    let store = Store::new(
        Arc::new(FsBackend::open(other.path()).unwrap()),
        &SpaceRef::Personal {
            account: "bob".into(),
        },
    )
    .unwrap();
    assert!(
        f.coordinator
            .poll(&f.worker, &store, &f.kinds, 101)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cancelled_job_cannot_attach_a_late_output_name() {
    let f = Fixture::new().await;
    let work = f
        .coordinator
        .poll(&f.worker, &f.store, &f.kinds, 101)
        .await
        .unwrap()
        .unwrap();
    let attempt = Attempt {
        job: f.job.clone(),
        attempt: work.assignment.lease.attempt,
    };
    let work = f
        .coordinator
        .start(&f.worker, &f.store, &f.kinds, &attempt, 102)
        .await
        .unwrap();
    let artifact = f
        .store
        .put_job_artifact(
            &work.assignment.lease,
            ArtifactInput {
                version: work.version.reference(),
                role: ArtifactRole::Build,
                media_type: "text/plain".into(),
                class: lab_kit::AssetClass::Public,
                no_training: false,
            },
            b"output".to_vec(),
            100,
            103,
        )
        .await
        .unwrap();
    f.store.cancel(&f.job, 104).await.unwrap();
    assert!(matches!(
        f.store
            .name_job_artifact(
                &work.assignment.lease,
                &work.version.reference(),
                "output.txt",
                &artifact.id,
                105
            )
            .await,
        Err(lab_store::Error::LeaseLost)
    ));
    assert!(matches!(
        f.store
            .named_artifact(&work.version.reference(), "output.txt")
            .await,
        Err(lab_store::Error::NotFound)
    ));
}
