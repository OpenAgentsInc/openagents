use lab_jobs::{Capabilities, Placement, authority::Authority};
use lab_kit::SpaceRef;
use lab_store::backend::FsBackend;
use std::sync::Arc;

fn capabilities() -> Capabilities {
    Capabilities {
        worker: "label".into(),
        placement: Placement::Enrolled,
        cpu_millis: 1000,
        memory_bytes: 128 * 1024 * 1024,
        gpu: false,
        tools: Default::default(),
        jobs: ["echo/shout".into()].into(),
        local_assets: Default::default(),
        max_seconds: 600,
    }
}

#[tokio::test]
async fn worker_key_is_scoped_expiring_revocable_and_stored_only_as_a_hash() {
    let tmp = tempfile::tempdir().unwrap();
    let backend = Arc::new(FsBackend::open(tmp.path()).unwrap());
    let authority = Authority::new(backend.clone());
    let issued = authority
        .enroll(
            "alice",
            SpaceRef::Personal {
                account: "alice".into(),
            },
            capabilities(),
            100,
            60_000,
        )
        .await
        .unwrap();
    assert_ne!(issued.worker.id, "label");
    assert_eq!(
        authority
            .authenticate(&issued.token, 101)
            .await
            .unwrap()
            .account,
        "alice"
    );
    assert!(!format!("{issued:?}").contains(&issued.token));
    let mut forged = issued.token.clone();
    forged.pop();
    forged.push('!');
    assert!(authority.authenticate(&forged, 102).await.is_err());
    assert!(authority.revoke("bob", &issued.worker.id).await.is_err());
    assert!(authority.authenticate(&issued.token, 60_100).await.is_err());
    let restored = Authority::new(backend);
    assert!(restored.authenticate(&issued.token, 103).await.is_ok());
    restored.revoke("alice", &issued.worker.id).await.unwrap();
    assert!(authority.authenticate(&issued.token, 104).await.is_err());
    for ns in std::fs::read_dir(tmp.path())
        .unwrap()
        .flatten()
        .filter(|p| p.path().is_dir())
    {
        let index = std::fs::read_to_string(ns.path().join("index.json")).unwrap();
        assert!(!index.contains(&issued.token));
        assert!(!index.contains(issued.token.split_once('.').unwrap().1));
    }
}

#[tokio::test]
async fn concurrent_pollers_reserve_one_slot_and_expired_unused_slots_recover() {
    let tmp = tempfile::tempdir().unwrap();
    let authority = Authority::new(Arc::new(FsBackend::open(tmp.path()).unwrap()));
    let issued = authority
        .enroll(
            "alice",
            SpaceRef::Personal {
                account: "alice".into(),
            },
            capabilities(),
            100,
            60_000,
        )
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        authority.reserve(&issued.worker, "first", 101),
        authority.reserve(&issued.worker, "second", 101)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(
        authority
            .workers(&issued.worker.space, 102)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        authority
            .reserve(&issued.worker, "next", 30_102)
            .await
            .is_ok()
    );
    authority
        .release(&issued.worker, "unrelated")
        .await
        .unwrap();
    assert!(
        authority
            .reserve(&issued.worker, "wrong", 30_103)
            .await
            .is_err()
    );
    authority.release(&issued.worker, "next").await.unwrap();
    assert!(
        authority
            .reserve(&issued.worker, "last", 30_104)
            .await
            .is_ok()
    );
    authority.revoke("alice", &issued.worker.id).await.unwrap();
    assert!(
        authority
            .reserve(&issued.worker, "revoked", 30_105)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_personal_worker_cannot_be_enrolled_for_another_account() {
    let tmp = tempfile::tempdir().unwrap();
    let authority = Authority::new(Arc::new(FsBackend::open(tmp.path()).unwrap()));
    assert!(
        authority
            .enroll(
                "alice",
                SpaceRef::Personal {
                    account: "bob".into()
                },
                capabilities(),
                100,
                60_000
            )
            .await
            .is_err()
    );
}
