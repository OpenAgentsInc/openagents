//! A phone screen recreated after the app went to the background attaches
//! to the terminal it showed instead of opening another, can watch it
//! without typing, and reads the host's command list and saved sessions
//! (#10683).
//!
//! A real host on the synthetic NIP-42 relay serves one enrolled device
//! over a relay route. One machine, loopback only, with scratch state.
#![cfg(unix)]

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder_computers::terminal::session::{Links, Session};
use coder_computers::terminal::{Blocks, Model, Phase, Saved, SavedMember};
use coder_host::access::{RelayPolicy, Rights};
use coder_host::client::{Device, Link, fetch_reach};
use coder_host::config::Config;
use coder_host::message::TermRequest;
use coder_host::pty::ext::{Layout, Member, Node, SessionRecord, SessionWrite, Tab};
use coder_host::pty::wire::TerminalRef;
use coder_host::reach::pubkey;
use coder_host::{NoTasks, Running};
use secp256k1::SecretKey;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

async fn serve(root: &Path, relay: &str) -> Running {
    let workspace = root.join("checkout");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut config = Config::new(root.join("access"), vec![relay.to_owned()], 1);
    config.policy = POLICY;
    config.workspaces = BTreeMap::from([(
        "checkout".to_owned(),
        std::fs::canonicalize(&workspace).unwrap(),
    )]);
    coder_host::start(config, Arc::new(NoTasks)).await.unwrap()
}

/// Waits until `check` holds for the session's model.
fn until(session: &Session, what: &str, check: impl Fn(&Model) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        {
            let model = session.model();
            if check(&model) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}: {:?}\n{}",
                model.phase,
                model.vt.text()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_recreated_screen_reattaches_watches_and_lists_commands() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, _events) = runtime.block_on(relay::start());
    let store = coder_host::access::host::Host::new(temp.path().join("access"), POLICY);
    store.init(&pubkey(&key())).unwrap();
    let running = runtime.block_on(serve(temp.path(), &relay));
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
    runtime.block_on(async {
        let deadline = Instant::now() + Duration::from_secs(30);
        while fetch_reach(&device, &relay).await.is_err() {
            assert!(Instant::now() < deadline, "no presence");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    let link = Arc::new(Link::relay_at(device.clone(), relay.clone(), 1));
    let writer = link.clone();
    let links: Links = Arc::new(move || Ok(link.clone()));
    // A second screen, as on another phone, has a link of its own: one
    // link's frames go to one screen.
    let other = Arc::new(Link::relay_at(device.clone(), relay.clone(), 1));
    let other_links: Links = Arc::new(move || Ok(other.clone()));

    // The first screen opens a shell and runs a command. The marker shows
    // only in the command's output, never in its echo.
    let first = Session::start(
        runtime.handle(),
        links.clone(),
        Model::new(host.clone(), "Mac", 24, 80),
    );
    until(&first, "attach", |model| model.phase == Phase::Attached);
    first.send(b"echo marker-$((2+3))\n".to_vec());
    until(&first, "output", |model| {
        model.vt.text().contains("marker-5")
    });
    let reference = first.model().reference.clone().expect("named terminal");
    // The app goes to the background: the screen and its session go away.
    drop(first);

    // The recreated screen attaches to the same terminal, which kept the
    // command's output, rather than opening another shell.
    let (generation, terminal) = reference.clone();
    let again = Session::attach(
        runtime.handle(),
        links.clone(),
        Model::new(host.clone(), "Mac", 24, 80),
        TerminalRef {
            generation,
            terminal,
        },
    );
    until(&again, "reattach", |model| model.phase == Phase::Attached);
    until(&again, "restored output", |model| {
        model.vt.text().contains("marker-5")
    });
    assert_eq!(again.model().reference, Some(reference.clone()));

    // The host's command list: this shell has no integration hooks, so the
    // list is empty and says why.
    again.blocks(None);
    until(&again, "the command list", |model| {
        !matches!(model.blocks, Blocks::Reading | Blocks::Hidden)
    });
    assert_eq!(
        again.model().blocks,
        Blocks::Page {
            rows: vec![],
            more: false
        }
    );
    again.hide_blocks();

    // A saved session holds this terminal and a thread link, as the
    // desktop workbench saves it; the phone lists it and opens it.
    let record = SessionRecord {
        session: None,
        revision: 0,
        name: "build".into(),
        members: vec![
            Member::Terminal {
                member: 1,
                terminal: TerminalRef {
                    generation: reference.0.clone(),
                    terminal: reference.1.clone(),
                },
                state: None,
            },
            Member::Resource {
                member: 2,
                resource: serde_json::json!({"kind": "thread", "id": "7".repeat(64)}),
            },
        ],
        layout: Layout {
            tabs: vec![Tab {
                name: "main".into(),
                root: Node::Pane { member: 1 },
            }],
            active: 0,
        },
    };
    let written = runtime
        .block_on(writer.terminal(TermRequest::SessionWrite(SessionWrite::new(
            coder_host::reach::new_id(),
            None,
            0,
            record,
        ))))
        .unwrap();
    assert!(written.reason.is_none(), "{written:?}");
    again.saved(None);
    until(&again, "the session list", |model| {
        matches!(model.saved, Saved::List(_))
    });
    let Saved::List(entries) = again.model().saved.clone() else {
        unreachable!()
    };
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].name.as_str(), entries[0].members), ("build", 2));
    again.saved(Some(entries[0].session.clone()));
    until(&again, "the session", |model| {
        matches!(model.saved, Saved::Open { .. })
    });
    let Saved::Open { members, .. } = again.model().saved.clone() else {
        unreachable!()
    };
    assert_eq!(
        members,
        vec![
            SavedMember::Terminal {
                member: 1,
                generation: reference.0.clone(),
                terminal: reference.1.clone(),
                state: "live",
            },
            SavedMember::Thread {
                member: 2,
                thread: "7".repeat(64),
            },
        ]
    );
    again.hide_saved();
    assert_eq!(again.model().blocks, Blocks::Hidden);

    // A watching screen attaches in observe mode and sends nothing.
    let mut watching = Model::new(host.clone(), "Mac", 24, 80);
    watching.watch = true;
    let (generation, terminal) = reference;
    let watcher = Session::attach(
        runtime.handle(),
        other_links,
        watching,
        TerminalRef {
            generation,
            terminal,
        },
    );
    until(&watcher, "watch", |model| model.phase == Phase::Attached);
    watcher.send(b"echo typed-$((3+4))\n".to_vec());
    let notice = watcher.model().notice.clone().unwrap_or_default();
    assert!(notice.contains("watching"), "{notice:?}");
    // The typing screen still types, and the watcher sees it.
    again.send(b"echo after-$((4+5))\n".to_vec());
    until(&watcher, "watched output", |model| {
        model.vt.text().contains("after-9")
    });
    std::thread::sleep(Duration::from_millis(500));
    assert!(!again.model().vt.text().contains("typed-7"));

    again.close();
    until(&again, "exit", |model| model.phase.ended());
    drop((again, watcher));
    runtime.block_on(running.shutdown());
}
