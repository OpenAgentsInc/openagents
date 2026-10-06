//! Scratch host acceptance for the shared native and Verse transport.
#![cfg(unix)]
#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;
use coder_host::{
    NoTasks,
    access::{RelayPolicy, Rights},
    client::{Device, Link, fetch_reach},
    config::Config,
    reach::pubkey,
};
use secp256k1::SecretKey;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use terminal_core::pty::{Program, Session, Sessions};
use terminal_remote::Remote;

fn until(sessions: &Sessions, session: &mut Session, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        sessions.pump(
            session,
            64 * 1024,
            Instant::now() + Duration::from_millis(10),
        );
        if session.vt.text().contains(text) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "waiting for {text}: {:?}: {}",
            session.status,
            session.vt.text()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
#[test]
fn window_verse_window_keep_one_host_process_and_reconcile_routes() {
    use std::os::unix::fs::PermissionsExt;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, _) = runtime.block_on(relay::start());
    let policy = RelayPolicy::LoopbackTest;
    let owner = SecretKey::new(&mut secp256k1::rand::rng());
    let store = coder_host::access::host::Host::new(temp.path().join("access"), policy);
    store.init(&pubkey(&owner)).unwrap();
    let shell = temp.path().join("scratch-shell");
    std::fs::write(
        &shell,
        format!("#!/bin/sh\nexport HOME='{}'\n", temp.path().display())
            + r#"stty -echo
printf '\033]7;file://host%s\007\033]133;A\007' "$PWD"
while IFS= read -r line; do
    hex=$(printf %s "$line" | od -An -tx1 | tr -d ' \n')
    printf '\033]133;B\007\033]777;openagents;command;%s\007\033]133;C\007' "$hex"
    eval "$line"
    status=$?
    printf '\033]133;D;%s\007\033]133;A\007' "$status"
done
"#,
    )
    .unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = Config::new(temp.path().join("access"), vec![relay.clone()], 1);
    config.policy = policy;
    config.workspaces = BTreeMap::from([("scratch".into(), temp.path().to_path_buf())]);
    config.terminal_shell = Some(shell);
    let running = runtime
        .block_on(coder_host::start(config.clone(), Arc::new(NoTasks)))
        .unwrap();
    let host = running.host_key().to_owned();
    let device = runtime.block_on(async {
        let now = coder_host::unix_time().unwrap();
        let code = store
            .invite(&relay, Rights::standard(), now, now + 3600)
            .unwrap()
            .code;
        let key = SecretKey::new(&mut secp256k1::rand::rng());
        let access = coder_host::access::client::redeem(&code, &key, policy)
            .await
            .unwrap();
        let device = Arc::new(Device::new(access, key, policy).unwrap());
        let deadline = Instant::now() + Duration::from_secs(30);
        while fetch_reach(&device, &relay).await.is_err() {
            assert!(Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        device
    });
    let mut reference = None;
    for label in ["Standalone", "Verse", "Standalone again"] {
        let link = Arc::new(Link::relay_at(device.clone(), relay.clone(), 1));
        let current = Arc::new(Mutex::new(Some(link.clone())));
        let get = current.clone();
        let transport = Remote::injected(
            host.clone(),
            label.into(),
            Arc::new(move || {
                get.lock().unwrap().clone().ok_or_else(|| {
                    coder_host::access::Error::new(
                        coder_host::access::Code::Transport,
                        "Route disconnected.",
                    )
                })
            }),
            runtime.handle().clone(),
            reference.clone(),
            Arc::new(Local),
        );
        let sessions = Sessions(Arc::new(transport));
        let mut session = sessions.open(&Program::Shell, 24, 80).unwrap();
        if reference.is_none() {
            // Opening is asynchronous: wait for an attached status, then send once.
            let deadline = Instant::now() + Duration::from_secs(30);
            while session.reference().is_none()
                || !session
                    .status
                    .as_deref()
                    .is_some_and(|s| s.contains("Connected"))
            {
                sessions.pump(
                    &mut session,
                    65536,
                    Instant::now() + Duration::from_millis(10),
                );
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(25));
            }
            sessions.input(
                &session,
                b"printf x >> counter; printf 'original-%s\\n' marker\n",
            );
            until(&sessions, &mut session, "original-marker");
        } else {
            until(&sessions, &mut session, "original-marker");
        }
        reference = session.reference();
        assert!(reference.is_some());
        if label == "Standalone" {
            use coder_pty::{share::ShareMode, wire::Value};
            use terminal_core::sharing::Action;
            let share = |action| {
                session
                    .sharing(action)
                    .unwrap()
                    .recv_timeout(Duration::from_secs(20))
                    .unwrap()
                    .unwrap()
            };
            let grantee = pubkey(&SecretKey::new(&mut secp256k1::rand::rng()));
            let Value::Shared {
                grant,
                authorization,
            } = share(Action::Issue {
                grantee: grantee.clone(),
                mode: ShareMode::Watch,
                expires_at: coder_host::unix_time().unwrap() + 600,
            })
            else {
                panic!("share acknowledgment")
            };
            assert_eq!(grant.grantee, grantee);
            assert!(!authorization.is_null());
            let Value::Viewers { viewers } = share(Action::Read) else {
                panic!("viewer page")
            };
            assert_eq!(viewers.shares.len(), 1);
            assert_eq!(viewers.viewers.len(), 1);
            assert!(!viewers.paused);
            assert_eq!(share(Action::Pause(true)), Value::Done);
            let Value::Viewers { viewers } = share(Action::Read) else {
                panic!("paused viewer page")
            };
            assert!(viewers.paused);
            assert_eq!(share(Action::Pause(false)), Value::Done);
            assert_eq!(share(Action::Revoke(Some(grant.share))), Value::Done);
            let Value::Viewers { viewers } = share(Action::Read) else {
                panic!("reconciled viewer page")
            };
            assert!(!viewers.paused);
            assert!(viewers.shares.is_empty());
        }

        if label == "Verse" {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !session.journal.as_ref().is_some_and(|page| {
                page.blocks
                    .iter()
                    .any(|block| block.command.contains("counter"))
            }) {
                sessions.pump(
                    &mut session,
                    65536,
                    Instant::now() + Duration::from_millis(10),
                );
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(25));
            }
            assert!(
                session
                    .blocks
                    .records
                    .iter()
                    .any(|block| block.command.contains("counter"))
            );
            let before = (session.vt.rows(), session.vt.cols());
            sessions.resize(&mut session, 12, 52);
            assert_eq!((session.vt.rows(), session.vt.cols()), before);
            let deadline = Instant::now() + Duration::from_secs(5);
            while (session.vt.rows(), session.vt.cols()) != (12, 52) {
                sessions.pump(
                    &mut session,
                    65536,
                    Instant::now() + Duration::from_millis(10),
                );
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        if label == "Verse" {
            let target = session.reference().unwrap();
            let mut proposal = coder_host::pty::proposal::Proposal {
                thread: coder_host::reach::new_id(),
                id: coder_host::reach::new_id(),
                revision: 1,
                command: "printf approved >> proposal_counter".into(),
                binding: coder_host::pty::proposal::Binding {
                    terminal: target.terminal,
                    generation: target.generation,
                    cwd: temp.path().display().to_string(),
                    shell_directory: Some(temp.path().display().to_string()),
                    context_digest: coder_host::reach::new_id(),
                },
            };
            assert!(session.offer_proposal(&proposal).unwrap().is_ok());
            assert!(matches!(
                session.decide_proposal(&proposal, true).unwrap().unwrap(),
                coder_host::pty::proposal::State::Warned { .. }
            ));
            link.shutdown();
            *current.lock().unwrap() = None;
            let deadline = Instant::now() + Duration::from_secs(10);
            while !session
                .status
                .as_deref()
                .is_some_and(|s| s.contains("Reconnecting"))
            {
                sessions.pump(
                    &mut session,
                    65536,
                    Instant::now() + Duration::from_millis(10),
                );
                assert!(Instant::now() < deadline, "status: {:?}", session.status);
                std::thread::sleep(Duration::from_millis(25));
            }
            sessions.input(&session, b"printf replayed >> counter\n");
            assert!(session.decide_proposal(&proposal, true).unwrap().is_err());
            assert!(!temp.path().join("proposal_counter").exists());
            *current.lock().unwrap() =
                Some(Arc::new(Link::relay_at(device.clone(), relay.clone(), 1)));
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                sessions.pump(
                    &mut session,
                    65536,
                    Instant::now() + Duration::from_millis(10),
                );
                if session
                    .status
                    .as_deref()
                    .is_some_and(|s| s.contains("Connected"))
                {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(25));
            }
            sessions.input(&session, b"printf 'route-%s\\n' repaired\n");
            until(&sessions, &mut session, "route-repaired");
            // Changed input invalidates the old offer. A fresh revision requires two new confirmations.
            assert!(session.decide_proposal(&proposal, true).unwrap().is_err());
            std::thread::sleep(Duration::from_millis(100));
            proposal.revision = 2;
            assert!(session.offer_proposal(&proposal).unwrap().is_ok());
            assert!(matches!(
                session.decide_proposal(&proposal, true).unwrap().unwrap(),
                coder_host::pty::proposal::State::Warned { .. }
            ));
            assert_eq!(
                session.decide_proposal(&proposal, true).unwrap().unwrap(),
                coder_host::pty::proposal::State::Executing
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            while std::fs::read(temp.path().join("proposal_counter")).unwrap_or_default()
                != b"approved"
            {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(25));
            }
            assert!(session.decide_proposal(&proposal, true).unwrap().is_err());
        }
        drop(session);
        drop(sessions);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(std::fs::read(temp.path().join("counter")).unwrap(), b"x");
    assert_eq!(
        std::fs::read(temp.path().join("proposal_counter")).unwrap(),
        b"approved"
    );
    runtime.block_on(running.shutdown());
    config.generation = 2;
    let restarted = runtime
        .block_on(coder_host::start(config, Arc::new(NoTasks)))
        .unwrap();
    let link = Arc::new(Link::relay_at(device, relay, 2));
    let remote = Remote::injected(
        host,
        "Restarted".into(),
        Arc::new(move || Ok(link.clone())),
        runtime.handle().clone(),
        reference,
        Arc::new(Local),
    );
    let sessions = Sessions(Arc::new(remote));
    let mut session = sessions.open(&Program::Shell, 24, 80).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while session.exited.is_none() {
        sessions.pump(
            &mut session,
            65536,
            Instant::now() + Duration::from_millis(10),
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(session.exited.as_ref().unwrap().contains("Lost"));
    drop(session);
    runtime.block_on(restarted.shutdown());
}

struct Local;
impl terminal_core::pty::Transport for Local {
    fn shell(&self) -> &std::path::Path {
        std::path::Path::new("/bin/sh")
    }
    fn open(
        &self,
        _: &Program,
        _: u16,
        _: u16,
    ) -> Result<Box<dyn terminal_core::pty::Attachment>, String> {
        Err("No local terminal in this test.".into())
    }
    fn shutdown(&self) {}
    fn thread_program(&self) -> Option<Program> {
        None
    }
    fn resolve(&self, _: &str) -> Option<std::path::PathBuf> {
        None
    }
    fn request(
        &self,
        _: &terminal_core::bridge::Request,
    ) -> Result<terminal_core::bridge::Connection, String> {
        Err("No chat client in this test.".into())
    }
    fn git_summary(&self, _: u64, _: String) -> std::sync::mpsc::Receiver<(u64, String, String)> {
        std::sync::mpsc::channel().1
    }
    fn open_link(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn clipboard(&self) -> Option<String> {
        None
    }
    fn copy(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}
