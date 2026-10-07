//! #10748: the live Boat binding against the loopback fake Boat API: keyed
//! creates survive a lost reply and a restart, deletion is acknowledged
//! only when Boat completes it, and a wrong key reaches nothing.

use retail_cloud::boat::{BoatAdapter, BoatConfig};
use retail_cloud::provision::{CreateSpec, Provider, ProviderError, ResourceState};
use retail_qualify::sim::FakeBoat;

const KEY: &str = "sim-boat-binding-key";

fn adapter(base: &str, key: &str, state: &std::path::Path) -> BoatAdapter {
    BoatAdapter::new(
        boat::ApiKey::new(key).unwrap(),
        &BoatConfig {
            base_url: base.into(),
            org: None,
            state_dir: state.into(),
            retry: Some(boat::RetryPolicy {
                max_retries: 2,
                base_delay: std::time::Duration::from_millis(1),
                max_delay: std::time::Duration::from_millis(5),
            }),
        },
    )
    .unwrap()
}

fn spec() -> CreateSpec {
    CreateSpec {
        provisioning: "rx_test#1".into(),
        account: "acct".into(),
        template: "oa-coder-main-20261006".into(),
        size: "large".into(),
        no_env: true,
        lifetime_secs: 4_800,
    }
}

#[test]
fn keyed_creates_restarts_and_deletion_reconcile_to_one_sandbox() {
    let fake = FakeBoat::start(KEY).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let boat = adapter(fake.base(), KEY, tmp.path());
    // The reply is lost: the effect is unknown, and the index cannot name
    // the sandbox yet.
    fake.lose_next_create_reply();
    assert!(matches!(
        boat.create(&spec()),
        Err(ProviderError::Unknown(_))
    ));
    assert_eq!(fake.sandboxes_created(), 1);
    assert_eq!(boat.find("rx_test#1").unwrap(), None);
    // Provisioning creates again with the same key and gets the same sandbox.
    let created = boat.create(&spec()).unwrap();
    assert_eq!(boat.create(&spec()).unwrap().id, created.id);
    assert_eq!(fake.sandboxes_created(), 1);
    assert_eq!(fake.create_requests(), 3);

    // A restarted adapter finds it by its provisioning identity.
    drop(boat);
    let boat = adapter(fake.base(), KEY, tmp.path());
    let found = boat.find("rx_test#1").unwrap().unwrap();
    assert_eq!(
        (found.id.as_str(), found.account.as_str()),
        (created.id.as_str(), "acct")
    );
    assert_eq!(boat.find("rx_other#1").unwrap(), None);

    assert_eq!(boat.state(&created.id).unwrap(), ResourceState::Starting);
    assert!(matches!(
        boat.state(&created.id).unwrap(),
        ResourceState::Ready { .. }
    ));
    assert_eq!(boat.usage_seconds(&created.id).unwrap(), Some(90));
    boat.delete(&created.id).unwrap();
    assert_eq!(boat.state(&created.id).unwrap(), ResourceState::Deleted);
    assert_eq!(fake.active(), 0);
    assert!(boat.undeleted().is_empty());
    boat.delete(&created.id).unwrap();
}

#[test]
fn a_wrong_key_reaches_nothing() {
    let fake = FakeBoat::start(KEY).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let boat = adapter(fake.base(), "operator-key", tmp.path());
    assert!(matches!(
        boat.create(&spec()),
        Err(ProviderError::Unknown(_))
    ));
    assert_eq!(fake.sandboxes_created(), 0);
    assert!(fake.unauthorized() >= 1);
}
