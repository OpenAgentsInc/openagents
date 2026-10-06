//! Scratch task-worktree admission through the existing host terminal transport.
#![cfg(unix)]
#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;
use coder_host::{
    access::{Code, RelayPolicy, Right, Rights},
    client::{Device, Link, fetch_reach},
    config::Config,
    reach::pubkey,
};
use secp256k1::SecretKey;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use terminal_core::pty::{Program, Session, Sessions};
use terminal_remote::Remote;

struct TaskOwner {
    directory: std::path::PathBuf,
    archived: AtomicBool,
    rebound: AtomicBool,
}
impl coder_host::Tasks for TaskOwner {
    fn terminal_binding(&self, task: &str) -> Result<coder_host::tasks::TerminalBinding, Code> {
        if task == "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            || self.archived.load(Ordering::SeqCst)
        {
            return Err(Code::Forbidden);
        }
        if task == "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd" {
            return Err(Code::Unsupported);
        }
        Ok(coder_host::tasks::TerminalBinding {
            directory: if self.rebound.load(Ordering::SeqCst) {
                self.directory.join("replacement")
            } else {
                self.directory.clone()
            },
            interactive: task != "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        })
    }
    fn create(
        &self,
        _: &str,
        _: &str,
        _: &coder_host::TaskCreate,
    ) -> Result<coder_host::TaskRef, Code> {
        panic!("a task terminal never creates a task")
    }
    fn steer(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: u64,
        _: &str,
    ) -> Result<coder_host::TaskRef, Code> {
        panic!("shell input never steers a task")
    }
    fn cancel(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: u64,
        _: &str,
    ) -> Result<coder_host::TaskRef, Code> {
        panic!("closing a terminal never cancels its task")
    }
    fn archive(&self, _: &str, _: &str, _: &str) -> Result<(), Code> {
        self.archived.store(true, Ordering::SeqCst);
        Ok(())
    }
}
fn pump_until(sessions: &Sessions, session: &mut Session, condition: impl Fn(&Session) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        sessions.pump(session, 65536, Instant::now() + Duration::from_millis(10));
        if condition(session) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{:?}: {}",
            session.status,
            session.vt.text()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn mount(
    runtime: &tokio::runtime::Runtime,
    host: &str,
    link: Arc<Link>,
    saved: Option<coder_host::pty::wire::TerminalRef>,
    task: Option<&str>,
) -> Sessions {
    let remote = Remote::injected(
        host.into(),
        "Scratch task host".into(),
        Arc::new(move || Ok(link.clone())),
        runtime.handle().clone(),
        saved,
        Arc::new(Local),
    );
    let remote = match task {
        Some(task) => remote.for_task(task.into()).unwrap(),
        None => remote,
    };
    Sessions(Arc::new(remote))
}
#[test]
fn task_shells_enforce_binding_rights_archive_revocation_and_generation() {
    use std::os::unix::fs::PermissionsExt;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("studio-worktree");
    std::fs::create_dir(&directory).unwrap();
    let owner = Arc::new(TaskOwner {
        directory: directory.clone(),
        archived: AtomicBool::new(false),
        rebound: AtomicBool::new(false),
    });
    let (relay, _task, _) = runtime.block_on(relay::start());
    let policy = RelayPolicy::LoopbackTest;
    let key = SecretKey::new(&mut secp256k1::rand::rng());
    let store = coder_host::access::host::Host::new(temp.path().join("access"), policy);
    store.init(&pubkey(&key)).unwrap();
    let shell = temp.path().join("shell");
    std::fs::write(&shell,format!("#!/bin/sh\nexport HOME='{}'\nstty -echo\nprintf '\\033]7;file://host%s\\007\\033]133;A\\007' \"$PWD\"\nwhile IFS= read -r line; do eval \"$line\"; done\n",temp.path().display())).unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = Config::new(temp.path().join("access"), vec![relay.clone()], 1);
    config.policy = policy;
    config.terminal_shell = Some(shell);
    config.workspaces = BTreeMap::from([("scratch".into(), temp.path().to_path_buf())]);
    let running = runtime
        .block_on(coder_host::start(config.clone(), owner.clone()))
        .unwrap();
    let host = running.host_key().to_owned();
    let enroll = |rights| {
        runtime.block_on(async {
            let now = coder_host::unix_time().unwrap();
            let code = store.invite(&relay, rights, now, now + 3600).unwrap().code;
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
        })
    };
    let device = enroll(Rights::standard());
    let link = Arc::new(Link::relay_at(device.clone(), relay.clone(), 1));
    for task in [
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    ] {
        let sessions = mount(&runtime, &host, link.clone(), None, Some(task));
        let mut session = sessions.open(&Program::Shell, 24, 80).unwrap();
        pump_until(&sessions, &mut session, |s| s.exited.is_some());
        assert!(session.reference().is_none());
        drop(session);
    }
    let observer = enroll(Rights::new([Right::Observe]).unwrap());
    let sessions = mount(
        &runtime,
        &host,
        Arc::new(Link::relay_at(observer, relay.clone(), 1)),
        None,
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    );
    let mut denied = sessions.open(&Program::Shell, 24, 80).unwrap();
    pump_until(&sessions, &mut denied, |s| s.exited.is_some());
    assert!(denied.reference().is_none());
    drop(denied);
    drop(sessions);
    let sessions = mount(
        &runtime,
        &host,
        link.clone(),
        None,
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    );
    let mut session = sessions.open(&Program::Shell, 24, 80).unwrap();
    pump_until(&sessions, &mut session, |s| s.input_available());
    assert!(session.status.as_ref().unwrap().contains(
        "task aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa / interactive"
    ));
    sessions.input(
        &session,
        b"pwd; printf admitted > counter; printf 'task-ready'\n",
    );
    pump_until(&sessions, &mut session, |s| {
        s.vt.text().contains("task-ready")
    });
    assert!(session.vt.text().contains(&directory.display().to_string()));
    assert_eq!(
        std::fs::read(directory.join("counter")).unwrap(),
        b"admitted"
    );
    pump_until(&sessions, &mut session, |s| {
        s.cwd.as_deref() == directory.to_str()
    });
    let reference = session.reference().unwrap();
    // Closing a second shell neither cancels the task nor replaces this terminal.
    let extra = mount(
        &runtime,
        &host,
        link.clone(),
        None,
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    );
    let mut extra_session = extra.open(&Program::Shell, 24, 80).unwrap();
    pump_until(&extra, &mut extra_session, |s| s.input_available());
    let extra_reference = extra_session.reference().unwrap();
    assert_ne!(extra_reference, reference);
    let closed = runtime
        .block_on(link.terminal(coder_host::message::TermRequest::Close(
            coder_host::pty::wire::Close::new(coder_host::reach::new_id(), extra_reference),
        )))
        .unwrap();
    assert_ne!(closed.status, coder_host::pty::wire::Status::Refused);
    drop(extra_session);
    let mut reopened = extra.open(&Program::Shell, 24, 80).unwrap();
    pump_until(&extra, &mut reopened, |s| s.input_available());
    assert_ne!(reopened.reference().unwrap(), reference);
    drop(reopened);
    drop(extra);
    std::fs::create_dir(directory.join("replacement")).unwrap();
    owner.rebound.store(true, Ordering::SeqCst);
    let answer = runtime
        .block_on(link.terminal(coder_host::message::TermRequest::Input(
            coder_host::pty::wire::Input::new(
                coder_host::reach::new_id(),
                reference.clone(),
                b"printf wrong-binding >> counter\n".to_vec(),
            ),
        )))
        .unwrap();
    assert_eq!(
        answer.reason.unwrap(),
        coder_host::pty::wire::Reason::NotAdmitted
    );
    owner.rebound.store(false, Ordering::SeqCst);
    drop(session);
    drop(sessions);
    let sessions = mount(&runtime, &host, link.clone(), Some(reference.clone()), None);
    let mut session = sessions.open(&Program::Shell, 24, 80).unwrap();
    pump_until(&sessions, &mut session, |s| s.input_available());
    runtime
        .block_on(
            link.call(coder_host::access::protocol::Operation::ArchiveTask {
                task: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            }),
        )
        .unwrap();
    let input = coder_host::pty::wire::Input::new(
        coder_host::reach::new_id(),
        reference.clone(),
        b"printf duplicated >> counter\n".to_vec(),
    );
    let answer = runtime
        .block_on(link.terminal(coder_host::message::TermRequest::Input(input)))
        .unwrap();
    assert_eq!(
        answer.reason.unwrap(),
        coder_host::pty::wire::Reason::NotAdmitted
    );
    drop(session);
    drop(sessions);
    let sessions = mount(&runtime, &host, link.clone(), Some(reference.clone()), None);
    let mut session = sessions.open(&Program::Shell, 24, 80).unwrap();
    pump_until(&sessions, &mut session, |s| s.exited.is_some());
    drop(session);
    drop(sessions);
    assert_eq!(
        std::fs::read(directory.join("counter")).unwrap(),
        b"admitted"
    );
    owner.archived.store(false, Ordering::SeqCst);
    store
        .revoke(&device.key(), coder_host::unix_time().unwrap())
        .unwrap();
    let answer = runtime
        .block_on(link.terminal(coder_host::message::TermRequest::Input(
            coder_host::pty::wire::Input::new(
                coder_host::reach::new_id(),
                reference.clone(),
                b"printf duplicated >> counter\n".to_vec(),
            ),
        )))
        .unwrap();
    assert_eq!(
        answer.reason.unwrap(),
        coder_host::pty::wire::Reason::Revoked
    );
    runtime.block_on(running.shutdown());
    config.generation = 2;
    let restarted = runtime.block_on(coder_host::start(config, owner)).unwrap();
    let fresh = enroll(Rights::standard());
    let sessions = mount(
        &runtime,
        &host,
        Arc::new(Link::relay_at(fresh, relay, 2)),
        Some(reference),
        None,
    );
    let mut lost = sessions.open(&Program::Shell, 24, 80).unwrap();
    pump_until(&sessions, &mut lost, |s| s.exited.is_some());
    assert!(lost.exited.as_ref().unwrap().contains("Lost"));
    drop(lost);
    drop(sessions);
    assert_eq!(
        std::fs::read(directory.join("counter")).unwrap(),
        b"admitted"
    );
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
