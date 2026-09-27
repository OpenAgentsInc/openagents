//! `coder link`'s device side end to end against a real host on a local
//! test relay: join by invitation, prove the direct TCP and WebSocket routes
//! and the relay route, list the host in the owner directory without
//! duplicate revisions, and fail every route after revocation.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use coder_access::host::Host;
use coder_access::{RelayPolicy, Rights};
use coder_host::NoTasks;
use coder_host::config::Config;
use coder_reach::hints::Locality;
use coder_setup::devices::{Client, Which};
use coder_setup::directory;
use secp256k1::SecretKey;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn join_check_every_route_list_the_host_and_lose_it_on_revocation() {
    let temp = tempfile::tempdir().unwrap();
    let (relay, _relay_task, _events) = relay::start().await;
    let owner = key();
    let access_dir = temp.path().join("access");
    let store = Host::new(&access_dir, POLICY);
    let host_key = store.init(&coder_reach::pubkey(&owner)).unwrap();

    let mut config = Config::new(access_dir, vec![relay.clone()], 3);
    config.policy = POLICY;
    config.listen_websocket = Some(SocketAddr::from(([127, 0, 0, 1], 0)));
    config.workspaces = BTreeMap::new();
    config.recheck_every = Duration::from_millis(100);
    let running = coder_host::start(config, Arc::new(NoTasks)).await.unwrap();

    // The rights a peer computer gets, including access_read for the relay
    // round trip.
    let rights = Rights::parse_list("observe,operate,terminal,review,access_read").unwrap();
    let issued = store.invite(&relay, rights, now(), now() + 3600).unwrap();
    let mut client = Client::open(&temp.path().join("computers"), POLICY)
        .unwrap()
        .with_locality(Locality::SameMachine);
    let joined = client.join(&issued.code, "box").await.unwrap();
    assert_eq!(joined, host_key);
    // A used invitation refuses a second redemption by another device.
    let mut other = Client::open(&temp.path().join("other"), POLICY).unwrap();
    assert!(other.join(&issued.code, "box").await.is_err());
    // The grant survives reopening the store.
    let client = Client::open(&temp.path().join("computers"), POLICY)
        .unwrap()
        .with_locality(Locality::SameMachine);
    assert_eq!(client.hosts().len(), 1);

    // Presence arrives shortly after the grant.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let results = loop {
        let results = client.check(Which::Both).await;
        if results.iter().all(|r| r.ok) || tokio::time::Instant::now() > deadline {
            break results;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    assert!(results.iter().all(|r| r.ok), "{results:#?}");
    let kinds: Vec<(&str, bool)> = results
        .iter()
        .map(|r| (r.kind, r.route.starts_with("ws://")))
        .collect();
    assert!(kinds.contains(&("direct", false)), "a tcp route: {kinds:?}");
    assert!(
        kinds.contains(&("direct", true)),
        "a websocket route: {kinds:?}"
    );
    let relayed = results.iter().find(|r| r.kind == "relay").unwrap();
    assert!(relayed.detail.contains("device.list"), "{relayed:?}");
    // From another machine, loopback hints are never offered.
    let far = Client::open(&temp.path().join("computers"), POLICY).unwrap();
    let direct: Vec<_> = far
        .check(Which::Direct)
        .await
        .into_iter()
        .filter(|r| r.ok)
        .collect();
    assert!(direct.is_empty(), "{direct:?}");

    // The owner lists the host; listing again publishes nothing.
    let relays = vec![relay.clone()];
    let owner_hex = coder_reach::pubkey(&owner);
    assert_eq!(
        directory::read(&relays, &owner, POLICY).await.unwrap(),
        None
    );
    let next = directory::with_host(None, &owner_hex, &host_key, "box", &relays, now())
        .unwrap()
        .unwrap();
    directory::publish(&relays, &owner, None, &next, POLICY)
        .await
        .unwrap();
    let current = directory::read(&relays, &owner, POLICY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.directory.revision, 1);
    assert_eq!(current.directory.entry(&host_key).unwrap().label, "box");
    assert_eq!(
        directory::with_host(
            Some(&current.directory),
            &owner_hex,
            &host_key,
            "box",
            &relays,
            now()
        )
        .unwrap(),
        None
    );
    // A host key cannot read the owner's directory.
    assert_eq!(
        directory::read(&relays, &key(), POLICY).await.unwrap(),
        None
    );

    // Revocation ends every route.
    store.revoke(&client.key(), now()).unwrap();
    let results = client.check(Which::Both).await;
    assert!(results.iter().all(|r| !r.ok), "{results:#?}");
    running.shutdown().await;
}
