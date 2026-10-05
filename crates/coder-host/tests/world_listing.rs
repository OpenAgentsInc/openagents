//! A world host names its instance in the owner directory (#10552): only
//! with the owner key, only for a host the owner already listed, and once.
#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

use coder_host::access::RelayPolicy;
use coder_host::client::{WorldListing, fetch_directory, list_world, publish_directory};
use coder_host::reach::directory::{Directory, HostEntry, WorldInstance};
use secp256k1::SecretKey;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::access::unix_time().unwrap()
}

fn world(label: &str) -> WorldInstance {
    WorldInstance {
        instance: 170,
        label: label.into(),
        wire: 23,
        content: Some("ab".repeat(32)),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_host_lists_its_instance_in_the_owner_directory() {
    let (relay, _task, _events) = relay::start().await;
    let (owner, host, other) = (key(), key(), key());
    let host_key = coder_host::reach::pubkey(&host);

    assert_eq!(
        list_world(&relay, &owner, &host_key, world("Ritual"), POLICY)
            .await
            .unwrap(),
        WorldListing::NoDirectory
    );

    let listed = Directory::empty(&coder_host::reach::pubkey(&owner), now())
        .with_host(
            HostEntry {
                host: coder_host::reach::pubkey(&other),
                label: "Other".into(),
                relays: vec![relay.clone()],
                weight: 100,
                added_at: now(),
                worlds: Vec::new(),
            },
            now(),
        )
        .unwrap();
    publish_directory(
        &relay,
        &owner,
        &listed,
        &"cd".repeat(32),
        now() + 3600,
        POLICY,
    )
    .await
    .unwrap();
    assert_eq!(
        list_world(&relay, &owner, &host_key, world("Ritual"), POLICY)
            .await
            .unwrap(),
        WorldListing::HostNotListed
    );

    let listed = listed
        .with_host(
            HostEntry {
                host: host_key.clone(),
                label: "Workstation".into(),
                relays: vec![relay.clone()],
                weight: 100,
                added_at: now(),
                worlds: Vec::new(),
            },
            now(),
        )
        .unwrap();
    publish_directory(
        &relay,
        &owner,
        &listed,
        &"cd".repeat(32),
        now() + 3600,
        POLICY,
    )
    .await
    .unwrap();
    assert_eq!(
        list_world(&relay, &owner, &host_key, world("Ritual"), POLICY)
            .await
            .unwrap(),
        WorldListing::Listed {
            revision: 3,
            published: true
        }
    );
    // The same world again changes nothing.
    assert_eq!(
        list_world(&relay, &owner, &host_key, world("Ritual"), POLICY)
            .await
            .unwrap(),
        WorldListing::Listed {
            revision: 3,
            published: false
        }
    );
    // A new label for the same instance replaces it.
    assert_eq!(
        list_world(&relay, &owner, &host_key, world("Ritual II"), POLICY)
            .await
            .unwrap(),
        WorldListing::Listed {
            revision: 4,
            published: true
        }
    );
    let directory = fetch_directory(&relay, &owner, POLICY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        directory.entry(&host_key).unwrap().worlds,
        vec![world("Ritual II")]
    );
    assert!(
        directory
            .entry(&coder_host::reach::pubkey(&other))
            .unwrap()
            .worlds
            .is_empty()
    );
}
