//! Original keyed-create reconciliation against the simulated public Boat API.

use retail_cloud::{
    boat::{BoatAdapter, BoatConfig, INDEX_FILE, INTENTS_FILE},
    provision::{CreateSpec, Provider},
};
use retail_qualify::sim::FakeBoat;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn missing_index_reconciles_only_exact_retained_creation_inside_the_window() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let boat = FakeBoat::start("synthetic-retail-recovery-key").unwrap();
    let config = BoatConfig {
        base_url: boat.base().into(),
        org: None,
        state_dir: root.clone(),
        retry: None,
    };
    let spec = CreateSpec {
        provisioning: "rx_fixture#1".into(),
        account: "alice".into(),
        template: "oa-coder-main-2026-10-07".into(),
        size: "large".into(),
        no_env: true,
        lifetime_secs: 1800,
    };
    let adapter = BoatAdapter::new(
        boat::ApiKey::new("synthetic-retail-recovery-key").unwrap(),
        &config,
    )
    .unwrap();
    let first = adapter.create(&spec).unwrap();
    let now = retail_service::http::now();
    assert_eq!(
        fs::metadata(root.join(INTENTS_FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(root.join(INDEX_FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(adapter);
    fs::remove_file(root.join(INDEX_FILE)).unwrap();
    let adapter = BoatAdapter::new(
        boat::ApiKey::new("synthetic-retail-recovery-key").unwrap(),
        &config,
    )
    .unwrap();
    assert!(adapter.find(&spec.provisioning).unwrap().is_none());
    assert_eq!(boat.create_requests(), 1);
    let mut changed = spec.clone();
    changed.template = "oa-coder-main-2026-10-08".into();
    assert!(adapter.reconcile_creation(&changed, now).unwrap().is_none());
    assert!(
        adapter
            .reconcile_creation(&spec, now + 601)
            .unwrap()
            .is_none()
    );
    assert_eq!(boat.create_requests(), 1);
    assert_eq!(adapter.reconcile_creation(&spec, now).unwrap(), Some(first));
    assert_eq!(boat.create_requests(), 2);
    assert_eq!(boat.sandboxes_created(), 1);
}
