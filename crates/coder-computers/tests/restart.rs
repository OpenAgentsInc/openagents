//! A phone's command card after its host restarts (#9986).
//!
//! A real host on the synthetic NIP-42 relay serves one enrolled device
//! over a relay route, the route a phone falls back to and keeps. The host
//! then stops and starts again with the next generation, as it does on a
//! project change or an update, while the device keeps the same link. The
//! next command runs on the new generation instead of being refused as
//! `lost`, and the link follows the new generation. One machine, loopback
//! only.
#![cfg(unix)]

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder_computers::terminal::exec::{self, Limits};
use coder_computers::terminal::session::Links;
use coder_host::access::{RelayPolicy, Rights};
use coder_host::client::{Device, Link, fetch_reach};
use coder_host::config::Config;
use coder_host::reach::pubkey;
use coder_host::{NoTasks, Running};
use secp256k1::SecretKey;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

async fn serve(root: &Path, relay: &str, generation: u64) -> Running {
    let workspace = root.join("checkout");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut config = Config::new(root.join("access"), vec![relay.to_owned()], generation);
    config.policy = POLICY;
    config.workspaces = BTreeMap::from([(
        "checkout".to_owned(),
        std::fs::canonicalize(&workspace).unwrap(),
    )]);
    coder_host::start(config, Arc::new(NoTasks)).await.unwrap()
}

/// Wait until the host's presence to `device` names `generation`.
async fn presence(device: &Device, relay: &str, generation: u64) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(reach) = fetch_reach(device, relay).await
            && reach.presence.presence.generation == generation
        {
            return;
        }
        assert!(Instant::now() < deadline, "no presence at {generation}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn echo(runtime: &tokio::runtime::Runtime, links: &Links, host: &str, word: &str) -> String {
    let limits = Limits {
        wait: Duration::from_secs(30),
        timeout: Duration::from_secs(30),
        ..Limits::default()
    };
    let words = vec!["echo".to_owned(), word.to_owned()];
    let run = exec::run(
        runtime.handle(),
        links.clone(),
        host,
        &words,
        limits,
        1,
        &mut |_| {},
    )
    .unwrap_or_else(|phase| panic!("`echo {word}` ended {phase:?}"));
    assert_eq!(run.exit, 0, "{run:?}");
    run.output
}

#[test]
fn the_next_command_after_a_host_restart_runs_on_the_new_generation() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, _events) = runtime.block_on(relay::start());
    let store = coder_host::access::host::Host::new(temp.path().join("access"), POLICY);
    store.init(&pubkey(&key())).unwrap();
    let running = runtime.block_on(serve(temp.path(), &relay, 1));
    let host = running.host_key().to_owned();

    let device = runtime.block_on(async {
        let now = coder_host::unix_time().unwrap();
        let code = store
            .invite(&relay, Rights::standard(), now, now + 3600)
            .unwrap()
            .code;
        let secret = key();
        let access = coder_host::access::client::redeem(&code, &secret, POLICY)
            .await
            .unwrap();
        Arc::new(Device::new(access, secret, POLICY).unwrap())
    });
    runtime.block_on(presence(&device, &relay, 1));

    // The relay route the supervisor proved against generation 1. A relay
    // route never closes, so it outlives the restart below.
    let link = Arc::new(Link::relay_at(device.clone(), relay.clone(), 1));
    let held = link.clone();
    let links: Links = Arc::new(move || Ok(held.clone()));
    assert!(echo(&runtime, &links, &host, "before-restart").contains("before-restart"));

    // The host starts again with the next generation, as on a project
    // change; the device keeps its link and never relaunches.
    runtime.block_on(running.shutdown());
    let running = runtime.block_on(serve(temp.path(), &relay, 2));
    runtime.block_on(presence(&device, &relay, 2));
    assert_eq!(link.generation(), Some(1));

    let output = echo(&runtime, &links, &host, "after-restart");
    assert!(output.contains("after-restart"), "{output:?}");
    assert_eq!(link.generation(), Some(2));
    // The next one runs on the new generation straight away.
    assert!(echo(&runtime, &links, &host, "again").contains("again"));

    runtime.block_on(running.shutdown());
}
