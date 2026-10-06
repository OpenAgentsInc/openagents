//! Scratch agent input uses an explicit lease and never gains terminal reads.
#![cfg(all(unix, feature = "host"))]
use coder_pty::{
    ext::{AgentInput, BlockPageRead, Handoff, Origin},
    host::{Authorize, Config, Host, Right, Rights, Wrap, channel},
    share::{ShareGrant, ViewersRead},
    wire::{Attach, Body, Detach, Input, Launch, Mode, Open, Refusal, Size, TerminalRef, Value},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AGENT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(10000);
    format!("{:064x}", NEXT.fetch_add(1, Ordering::Relaxed))
}
struct Grants(AtomicBool);
impl Rights for Grants {
    fn holds(&self, who: &str, right: Right) -> bool {
        who == OWNER && right == Right::Terminal && self.0.load(Ordering::SeqCst)
    }
}
struct Home(PathBuf);
impl Wrap for Home {
    fn command(
        &self,
        program: &std::path::Path,
        args: &[std::ffi::OsString],
    ) -> Result<std::process::Command, String> {
        let mut command = std::process::Command::new(program);
        command.args(args).env("HOME", &self.0);
        Ok(command)
    }
}
struct Stamp;
impl Authorize for Stamp {
    fn authorize(&self, _: &ShareGrant) -> Result<serde_json::Value, Refusal> {
        Ok(serde_json::Value::Null)
    }
}
struct Fixture {
    _root: tempfile::TempDir,
    host: Arc<Host>,
    grants: Arc<Grants>,
    terminal: TerminalRef,
    attachment: String,
    _frames: std::sync::mpsc::Receiver<coder_pty::wire::Frame>,
}
fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let grants = Arc::new(Grants(AtomicBool::new(true)));
    let mut config = Config::new().workspace(OWNER, root.path());
    config.wrap = Some(Arc::new(Home(root.path().to_path_buf())));
    config.emulator = Some(coder_vt::Authority::factory(500));
    config.shares = Some(Arc::new(Stamp));
    let host = Arc::new(Host::new(config, grants.clone()));
    let command = r#"stty -echo
printf '\033]133;A\007'
while IFS= read -r line; do
 printf '\033]133;B\007\033]777;openagents;command;7072696e7466206d61726b6572\007\033]133;C\007'
 printf 'agent-marker\n'
 printf '\033]133;D;0\007\033]133;A\007'
done"#;
    let (_, Value::Opened { terminal, .. }) = host
        .open(
            OWNER,
            &Open::new(
                id(),
                OWNER,
                "",
                Launch::Command {
                    program: "/bin/sh".into(),
                    args: vec!["-c".into(), command.into()],
                },
                Size::new(24, 80),
            ),
        )
        .unwrap()
    else {
        panic!("open")
    };
    let (sink, frames) = channel(4096);
    let (_, Value::Attached { attachment, .. }) = host
        .attach(
            OWNER,
            &Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 20).with_typist(),
            Box::new(sink),
        )
        .unwrap()
    else {
        panic!("attach")
    };
    let mut startup = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !startup.windows(7).any(|bytes| bytes == b"\x1b]133;A") {
        assert!(Instant::now() < deadline);
        if let Body::Output { data, .. } = frames.recv_timeout(Duration::from_secs(1)).unwrap().body
        {
            startup.extend(data);
        }
    }
    Fixture {
        _root: root,
        host,
        grants,
        terminal,
        attachment,
        _frames: frames,
    }
}
fn handoff(f: &Fixture) -> Handoff {
    Handoff::new(
        id(),
        f.terminal.clone(),
        f.attachment.clone(),
        AGENT,
        "c".repeat(64),
        "d".repeat(64),
    )
}
fn blocks(f: &Fixture, count: usize) -> coder_pty::ext::BlockPage {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let (_, Value::Blocks { page }) = f
            .host
            .block_page(
                OWNER,
                &BlockPageRead::new(id(), f.terminal.clone(), None, 32),
            )
            .unwrap()
        else {
            panic!("blocks")
        };
        if page.blocks.len() >= count
            && page
                .blocks
                .iter()
                .all(|block| block.state != coder_pty::ext::BlockState::Running)
        {
            return page;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn admitted_agent_is_attributed_reclaimable_and_has_no_observation_right() {
    let f = fixture();
    assert!(
        f.host
            .agent_input(
                AGENT,
                &AgentInput::new(id(), f.terminal.clone(), "e".repeat(64), b"printf marker\n")
            )
            .is_err()
    );
    assert!(
        f.host
            .agent_producer(AGENT, &f.terminal, &"e".repeat(64))
            .is_err()
    );
    let producer = f.host.admit_agent(OWNER, &handoff(&f)).unwrap();
    let binding = producer.binding().clone();
    assert!(f.host.owner_viewers(AGENT, &f.terminal).is_err());
    assert_eq!(
        f.host
            .agent_producer(AGENT, &f.terminal, &binding.lease)
            .unwrap()
            .binding(),
        &binding
    );
    let (_, Value::Viewers { viewers }) = f
        .host
        .viewers(OWNER, &ViewersRead::new(id(), f.terminal.clone()))
        .unwrap()
    else {
        panic!("viewers")
    };
    assert_eq!(viewers.agent, Some(binding.clone()));
    let (sink, _) = channel(1);
    assert!(
        f.host
            .attach(
                AGENT,
                &Attach::new(id(), f.terminal.clone(), Mode::Observe, 0, 4096),
                Box::new(sink)
            )
            .is_err()
    );
    let request = id();
    producer.input(request.clone(), b"printf marker\n").unwrap();
    let page = blocks(&f, 1);
    assert_eq!(page.blocks[0].origin, Origin::Agent);
    f.host
        .input(
            OWNER,
            &Input::new(id(), f.terminal.clone(), b"owner\n").from_attachment(f.attachment.clone()),
        )
        .unwrap();
    let page = blocks(&f, 2);
    assert_eq!(
        page.blocks
            .iter()
            .filter(|block| block.origin == Origin::Agent)
            .count(),
        1
    );
    // An exact replay acknowledges the original write; it cannot write again.
    producer.input(request.clone(), b"printf marker\n").unwrap();
    assert!(producer.input(id(), b"stale\n").is_err());
    let (recorded, evidence) = f.host.agent_evidence(&f.terminal).unwrap().unwrap();
    assert_eq!(recorded, binding);
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].request, request);
    assert_eq!(evidence[0].thread, "c".repeat(64));
    assert_eq!(evidence[0].run, "d".repeat(64));
    assert_eq!(blocks(&f, 2).blocks.len(), 2);
}
#[test]
fn revoked_grants_detached_roles_and_other_terminals_refuse_agent_input() {
    let f = fixture();
    let producer = f.host.admit_agent(OWNER, &handoff(&f)).unwrap();
    let (
        _,
        Value::Opened {
            terminal: other, ..
        },
    ) = f
        .host
        .open(
            OWNER,
            &Open::new(
                id(),
                OWNER,
                "",
                Launch::Command {
                    program: "/bin/cat".into(),
                    args: Vec::new(),
                },
                Size::new(24, 80),
            ),
        )
        .unwrap()
    else {
        panic!("other terminal")
    };
    assert!(
        f.host
            .agent_input(
                AGENT,
                &AgentInput::new(id(), other, producer.binding().lease.clone(), b"wrong\n")
            )
            .is_err()
    );
    f.grants.0.store(false, Ordering::SeqCst);
    assert!(producer.input(id(), b"revoked\n").is_err());
    f.grants.0.store(true, Ordering::SeqCst);
    assert!(
        producer
            .input(id(), b"restored grant cannot restore a lease\n")
            .is_err()
    );
    let f = fixture();
    let producer = f.host.admit_agent(OWNER, &handoff(&f)).unwrap();
    f.host
        .detach(
            OWNER,
            &Detach::new(id(), f.terminal.clone(), f.attachment.clone()),
        )
        .unwrap();
    assert!(producer.input(id(), b"detached\n").is_err());
}

#[test]
fn an_agent_replay_cannot_write_after_the_shared_retry_cache_evicts_it() {
    let f = fixture();
    let producer = f.host.admit_agent(OWNER, &handoff(&f)).unwrap();
    let request = id();
    producer.input(request.clone(), b"printf marker\n").unwrap();
    blocks(&f, 1);
    for _ in 0..1100 {
        f.host
            .proposal(
                OWNER,
                &coder_pty::proposal::Request::new(
                    id(),
                    f.terminal.clone(),
                    coder_pty::proposal::Action::Read { limit: 8 },
                ),
            )
            .unwrap();
    }
    assert!(producer.input(request, b"printf marker\n").is_err());
    assert_eq!(blocks(&f, 1).blocks.len(), 1);
    assert_eq!(
        f.host.agent_evidence(&f.terminal).unwrap().unwrap().1.len(),
        1
    );
}

#[test]
fn generated_input_retains_private_thread_evidence_and_replacement_keeps_old_log() {
    use coder_pty::host::{GeneratedInput, PrivateEvidence};
    let f = fixture();
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let evidence = PrivateEvidence::open(root.path()).unwrap();
    let producer = f.host.admit_agent(OWNER, &handoff(&f)).unwrap();
    let generated = GeneratedInput {
        request: id(),
        terminal: f.terminal.clone(),
        thread: producer.binding().thread.clone(),
        run: producer.binding().run.clone(),
        data: b"printf marker\n".to_vec(),
    };
    let mut wrong = generated.clone();
    wrong.thread = "e".repeat(64);
    assert!(producer.produce(&wrong, &evidence).is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    producer.produce(&generated, &evidence).unwrap();
    blocks(&f, 1);
    assert!(producer.produce(&generated, &evidence).is_err());
    let file = root
        .path()
        .join(&generated.thread)
        .join(format!("{}.jsonl", generated.request));
    let records = std::fs::read_to_string(&file).unwrap();
    assert!(!records.contains("printf marker"));
    let rows: Vec<serde_json::Value> = records
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["v"], "openagents.terminal-agent-input-evidence.v1");
    assert_eq!(rows[0]["outcome"], "unknown");
    assert_eq!(rows[1]["outcome"], "written");
    assert_eq!(rows[1]["bytes"], generated.data.len());
    assert_eq!(rows[1]["binding"]["thread"], generated.thread);
    assert_eq!(rows[1]["binding"]["run"], generated.run);
    assert_eq!(rows[1]["terminal"]["terminal"], f.terminal.terminal);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // The owner reclaims, then explicitly admits a replacement run.
    f.host
        .input(
            OWNER,
            &Input::new(id(), f.terminal.clone(), b"owner\n").from_attachment(f.attachment.clone()),
        )
        .unwrap();
    blocks(&f, 2);
    let mut replacement = handoff(&f);
    replacement.run = "e".repeat(64);
    let next = f.host.admit_agent(OWNER, &replacement).unwrap();
    next.input(id(), b"next\n").unwrap();
    blocks(&f, 3);
    let (old, inputs) = f.host.agent_evidence(&f.terminal).unwrap().unwrap();
    assert_eq!(old, *producer.binding());
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].request, generated.request);
    let (new, inputs) = f.host.agent_evidence(&f.terminal).unwrap().unwrap();
    assert_eq!(new, *next.binding());
    assert_eq!(inputs.len(), 1);
    let mut stale = generated.clone();
    stale.request = id();
    assert!(producer.produce(&stale, &evidence).is_err());
    assert_eq!(blocks(&f, 3).blocks.len(), 3);
}

#[test]
fn a_public_evidence_directory_is_refused_before_any_input() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(coder_pty::host::PrivateEvidence::open(root.path()).is_err());
    }
}

#[test]
fn a_drive_share_cannot_reclaim_the_admitted_agent_seat() {
    use coder_pty::share::{ShareMode, ShareRequest};
    let f = fixture();
    let friend = "e".repeat(64);
    f.host
        .share(
            OWNER,
            &ShareRequest::new(
                id(),
                f.terminal.clone(),
                &friend,
                ShareMode::Drive,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 60,
            ),
        )
        .unwrap();
    let (sink, _frames) = channel(4096);
    let (_, Value::Attached { attachment, .. }) = f
        .host
        .attach(
            &friend,
            &Attach::new(id(), f.terminal.clone(), Mode::Interact, 0, 4096).with_typist(),
            Box::new(sink),
        )
        .unwrap()
    else {
        panic!("friend attachment")
    };
    let producer = f.host.admit_agent(OWNER, &handoff(&f)).unwrap();
    assert!(
        f.host
            .input(
                &friend,
                &Input::new(id(), f.terminal.clone(), b"drive cannot reclaim\n")
                    .from_attachment(attachment)
            )
            .is_err()
    );
    producer.input(id(), b"still admitted\n").unwrap();
    assert_eq!(blocks(&f, 1).blocks[0].origin, Origin::Agent);
}
