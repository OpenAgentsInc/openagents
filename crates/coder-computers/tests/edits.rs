//! Directory editing and SSH routes through the live Computers service,
//! against a real `coder host serve` on the synthetic NIP-42 relay.
//!
//! - The owner directory: relabel, reweigh, and remove a host, each as the
//!   next revision on the relay; a removed host stays reachable and leaves
//!   placement; concurrent revisions stay a visible conflict until the owner
//!   keeps one; an edit against a stale revision and an edit without the
//!   owner key are refused.
//! - SSH: through `coder-ssh`'s fake `ssh` harness, the connector uses the
//!   tunnel's loopback port while the tunnel is up and the relay after it
//!   dies, the host keeps running, and **Remove over SSH** stops a managed
//!   host and detaches from an external one.
//!
//! One machine, loopback only. The fake `ssh` runs remote commands in a
//! temporary home and a test thread stands in for `ssh -L`; the real
//! `~/.ssh` and `~/.openagents` are never read or written. This is not a
//! real `sshd`, device, or production-relay test.
#![cfg(unix)]

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

#[path = "../../coder-ssh/tests/support/fake_ssh.rs"]
mod fake_ssh;

use coder_computers::live::{Live, Locality, MemoryStore, Settings, SshSetup};
use coder_computers::{
    Capabilities, Computers, ComputersService, Denial, DirectoryState, InputPurpose, ListingChange,
    Platform, Refusal, Snapshot, SshRemoval, SshStage, Tunnel,
};
use coder_host::access::host::Host;
use coder_host::access::{Code, RelayPolicy, Rights};
use coder_host::client::{fetch_directory_revisions, publish_directory};
use coder_host::reach::directory::Directory;
use coder_host::reach::placement::{Assessment, Skip};
use coder_host::reach::presence::{ClientProfile, VersionRange};
use coder_host::{NoTasks, Tasks};
use rust_native::{Activation, Element, Node};
use secp256k1::SecretKey;
use std::io::{ErrorKind, Read as _};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const WAIT: Duration = Duration::from_secs(45);

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

fn now() -> u64 {
    coder_host::unix_time().unwrap()
}

fn pubkey(secret: &SecretKey) -> String {
    coder_host::reach::pubkey(secret)
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .unwrap()
}

/// Initialize a host store for `owner` and serve it the way `coder host
/// serve` does, on its own runtime. Returns the store, the host key, and the
/// port of its loopback direct-channel listener.
fn serve_host(root: &Path, owner: &SecretKey, relay: &str) -> (Host, String, u16) {
    let access = root.join("access");
    let store = Host::new(&access, POLICY);
    let host = store.init(&pubkey(owner)).unwrap();
    let workspace = root.join("checkout");
    std::fs::create_dir_all(&workspace).unwrap();
    let record = root.join("host/runtime");
    let args: Vec<String> = [
        "serve",
        "--state",
        &access.to_string_lossy(),
        "--root",
        &root.join("host").to_string_lossy(),
        "--loopback-test",
        "--relay",
        relay,
        "--workspace",
        &format!("checkout={}", workspace.canonicalize().unwrap().display()),
        "--runtime",
        &record.to_string_lossy(),
        "--tasks",
        &root.join("tasks").to_string_lossy(),
    ]
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect();
    std::thread::spawn(move || {
        runtime().block_on(coder_host::cli::run(
            &args,
            Box::new(|_, _| Ok(Arc::new(NoTasks) as Arc<dyn Tasks>)),
        ))
    });
    let deadline = Instant::now() + WAIT;
    while !record.exists() {
        assert!(Instant::now() < deadline, "the host did not start serving");
        std::thread::sleep(Duration::from_millis(50));
    }
    let port = std::fs::read_to_string(&record)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("port="))
        .and_then(|port| port.parse().ok())
        .unwrap();
    (store, host, port)
}

fn settings(locality: Locality) -> Settings {
    let mut settings = Settings::new(Platform::Terminal);
    settings.policy = POLICY;
    settings.locality = locality;
    settings.refresh_every = Duration::from_secs(1);
    settings
}

fn open(settings: Settings, runtime: &tokio::runtime::Runtime) -> Computers {
    let live = Live::open(
        settings,
        key(),
        Box::new(MemoryStore::default()),
        runtime.handle().clone(),
    )
    .unwrap();
    Computers::new(
        Box::new(live),
        Capabilities {
            platform: Platform::Terminal,
            camera: false,
        },
        "computers:edits-test",
    )
    .unwrap()
}

fn walk<'a, I>(node: &'a Node<I>, out: &mut Vec<&'a Node<I>>) {
    out.push(node);
    if let Element::Stack { children, .. } | Element::List { children, .. } = &node.element {
        for child in children {
            walk(child, out);
        }
    }
}

fn texts(computers: &Computers) -> Vec<(String, String, Option<bool>)> {
    let mut nodes = Vec::new();
    walk(&computers.view().unwrap().view().root, &mut nodes);
    nodes
        .into_iter()
        .filter_map(|node| match &node.element {
            Element::Text { value, .. } => Some((node.key.clone(), value.clone(), None)),
            Element::Button { label, enabled, .. } => {
                Some((node.key.clone(), label.clone(), Some(*enabled)))
            }
            _ => None,
        })
        .collect()
}

fn text(computers: &Computers, key: &str) -> Option<String> {
    texts(computers)
        .into_iter()
        .find(|(node, ..)| node == key)
        .map(|(_, value, _)| value)
}

fn enabled(computers: &Computers, key: &str) -> Option<bool> {
    texts(computers)
        .into_iter()
        .find(|(node, ..)| node == key)
        .and_then(|(_, _, enabled)| enabled)
}

fn all_text(computers: &Computers) -> String {
    texts(computers)
        .into_iter()
        .map(|(key, value, _)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn press(computers: &mut Computers, key: &str) {
    computers.refresh().unwrap();
    assert_eq!(
        enabled(computers, key),
        Some(true),
        "{key} is not an enabled control:\n{}",
        all_text(computers)
    );
    let view = computers.view().unwrap().view();
    let activation = Activation {
        instance: view.instance.clone(),
        revision: view.revision,
        node: key.into(),
    };
    computers
        .activate(&activation)
        .unwrap_or_else(|refusal| panic!("{key}: {}", refusal.reason()));
}

fn submit(computers: &mut Computers, purpose: InputPurpose, value: &str) {
    let input = computers.input().expect("an input request").clone();
    assert_eq!(input.purpose, purpose);
    computers
        .submit(&input.token, value)
        .unwrap_or_else(|refusal| panic!("{purpose:?}: {}", refusal.reason()));
}

/// Poll the screens, the way a client's timer does, until `done` holds.
fn until(computers: &mut Computers, what: &str, done: impl Fn(&Computers) -> bool) {
    let deadline = Instant::now() + WAIT;
    loop {
        computers.refresh().unwrap();
        if done(computers) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}:\n{}",
            all_text(computers)
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn revision(snapshot: &Snapshot) -> Option<u64> {
    match snapshot.directory {
        DirectoryState::Current { revision, .. } => revision,
        _ => None,
    }
}

/// The highest directory revision on the relay, and its mailbox.
fn published(
    runtime: &tokio::runtime::Runtime,
    relay: &str,
    owner: &SecretKey,
) -> (Directory, String) {
    runtime
        .block_on(fetch_directory_revisions(relay, owner, POLICY))
        .unwrap()
        .into_iter()
        .max_by_key(|(directory, _)| directory.revision)
        .unwrap()
}

/// Publish `directory` as another owner device would.
fn publish_elsewhere(
    runtime: &tokio::runtime::Runtime,
    relay: &str,
    owner: &SecretKey,
    directory: &Directory,
    mailbox: &str,
) {
    runtime
        .block_on(publish_directory(
            relay,
            owner,
            directory,
            mailbox,
            now() + 3600,
            POLICY,
        ))
        .unwrap();
}

#[test]
fn the_owner_edits_removes_and_settles_conflicts_in_the_directory() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = runtime();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let (store, host, _) = serve_host(&temp.path().join("studio"), &owner, &relay);
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();

    let mut computers = open(settings(Locality::SameMachine), &runtime);
    press(&mut computers, "invite-paste");
    submit(&mut computers, InputPurpose::Invitation, &invitation.code);
    press(&mut computers, "first-run-continue");
    // A device without the owner key offers no directory edits.
    assert!(text(&computers, "host-0-rename").is_none());
    assert!(text(&computers, "host-0-list").is_none());
    press(&mut computers, "directory-owner-key");
    submit(
        &mut computers,
        InputPurpose::OwnerKey,
        &nostr::nip19::encode_nsec(&owner.secret_bytes()),
    );
    until(&mut computers, "the host online", |c| {
        matches!(
            c.snapshot().directory,
            DirectoryState::Current { revision: None, .. }
        ) && text(c, "host-0-status").is_some_and(|s| s.starts_with("Online"))
    });
    press(&mut computers, "host-0-list");
    submit(&mut computers, InputPurpose::DirectoryLabel, "Studio");
    assert_eq!(revision(computers.snapshot()), Some(1));

    // Rename: revision 2 on the relay carries the new label.
    press(&mut computers, "host-0-rename");
    submit(&mut computers, InputPurpose::DirectoryLabel, "Studio Mac");
    assert_eq!(revision(computers.snapshot()), Some(2));
    assert_eq!(computers.snapshot().hosts[0].label, "Studio Mac");
    let (second, _) = published(&runtime, &relay, &owner);
    assert_eq!(second.revision, 2);
    assert_eq!(second.entry(&host).unwrap().label, "Studio Mac");

    // Change weight: revision 3, and placement follows it.
    press(&mut computers, "host-0-weight");
    submit(&mut computers, InputPurpose::DirectoryWeight, "250");
    assert_eq!(revision(computers.snapshot()), Some(3));
    assert_eq!(computers.snapshot().hosts[0].weight(), 250);
    assert_eq!(
        text(&computers, "host-0-directory").as_deref(),
        Some("In your directory. Weight 250.")
    );
    let (third, _) = published(&runtime, &relay, &owner);
    assert_eq!(third.revision, 3);
    assert_eq!(third.entry(&host).unwrap().weight, 250);
    assert_eq!(third.entry(&host).unwrap().label, "Studio Mac");

    // Remove: revision 4 no longer lists the host. This device keeps its
    // grant, so the host stays reachable, and it leaves placement.
    press(&mut computers, "host-0-delist");
    press(&mut computers, "host-0-delist-yes");
    assert_eq!(revision(computers.snapshot()), Some(4));
    let (fourth, mailbox) = published(&runtime, &relay, &owner);
    assert_eq!(fourth.revision, 4);
    assert!(fourth.entry(&host).is_none());
    until(&mut computers, "the removed host still online", |c| {
        text(c, "host-0-status").is_some_and(|s| s.starts_with("Online"))
    });
    let snapshot = computers.snapshot().clone();
    assert_eq!(snapshot.hosts.len(), 1);
    assert!(snapshot.hosts[0].listing.is_none());
    assert!(snapshot.hosts[0].delisted);
    assert_eq!(snapshot.hosts[0].weight(), 0);
    assert_eq!(
        text(&computers, "host-0-directory").as_deref(),
        Some("Removed from your directory. This device can still reach it; it gets no new work.")
    );
    let client = ClientProfile {
        protocol: coder_host::reach::PROTOCOL_VERSION,
        accepts: VersionRange {
            min: coder_host::reach::PROTOCOL_VERSION,
            max: coder_host::reach::PROTOCOL_VERSION,
        },
    };
    assert_eq!(snapshot.place(&client), None);
    assert_eq!(
        snapshot.assess_placement(&client),
        vec![Assessment::Skipped {
            host: &host,
            reason: Skip::ZeroWeight
        }]
    );
    // The host still answers live requests: its device list loads.
    press(&mut computers, "host-0-access");
    press(&mut computers, "devices-refresh");
    assert!(text(&computers, "devices-as-of").is_some_and(|s| s.starts_with("1 devices")));
    press(&mut computers, "tab-computers");

    // Adding it back clears the removal: revision 5.
    press(&mut computers, "host-0-list");
    submit(&mut computers, InputPurpose::DirectoryLabel, "Studio");
    assert_eq!(revision(computers.snapshot()), Some(5));
    assert!(!computers.snapshot().hosts[0].delisted);

    // Concurrent edits: this device publishes revision 6, and another owner
    // device publishes a different revision 6 from the same parent.
    let (fifth, _) = published(&runtime, &relay, &owner);
    press(&mut computers, "host-0-rename");
    submit(&mut computers, InputPurpose::DirectoryLabel, "Mine");
    assert_eq!(revision(computers.snapshot()), Some(6));
    let mut theirs = fifth.clone();
    theirs.revision = 6;
    theirs.issued_at = fifth.issued_at + 1;
    theirs.hosts[0].label = "Theirs".into();
    publish_elsewhere(&runtime, &relay, &owner, &theirs, &mailbox);
    until(&mut computers, "the conflict", |c| {
        c.snapshot().directory == DirectoryState::Conflict { revision: 6 }
    });
    // The conflict stays visible: the list keeps the version this device
    // trusted, and no edit is offered until the owner settles it.
    std::thread::sleep(Duration::from_secs(3));
    computers.refresh().unwrap();
    assert_eq!(
        computers.snapshot().directory,
        DirectoryState::Conflict { revision: 6 }
    );
    assert_eq!(computers.snapshot().hosts[0].label, "Mine");
    assert!(
        text(&computers, "directory-status")
            .unwrap()
            .contains("two different versions at revision 6")
    );
    assert_eq!(enabled(&computers, "host-0-rename"), Some(false));
    assert!(
        text(&computers, "host-0-rename-reason")
            .unwrap()
            .contains("two different versions")
    );
    press(&mut computers, "directory-keep");
    until(&mut computers, "revision 7", |c| {
        revision(c.snapshot()) == Some(7)
    });
    let (seventh, _) = published(&runtime, &relay, &owner);
    assert_eq!(seventh.revision, 7);
    assert_eq!(seventh.entry(&host).unwrap().label, "Mine");
    assert!(text(&computers, "directory-keep").is_none());

    // An edit asked against revision 7 is stale once revision 8 arrives.
    press(&mut computers, "host-0-weight");
    let asked = computers.input().unwrap().clone();
    let mut eighth = seventh.clone();
    eighth.revision = 8;
    eighth.issued_at = seventh.issued_at + 1;
    eighth.hosts[0].weight = 999;
    publish_elsewhere(&runtime, &relay, &owner, &eighth, &mailbox);
    until(&mut computers, "revision 8", |c| {
        revision(c.snapshot()) == Some(8)
    });
    assert_eq!(
        computers.submit(&asked.token, "5"),
        Err(Refusal::Denied(Denial::StaleDirectory))
    );
    let (latest, _) = published(&runtime, &relay, &owner);
    assert_eq!(latest.revision, 8);
    assert_eq!(computers.snapshot().hosts[0].weight(), 999);

    // Without the owner key the service refuses every owner edit.
    let mut other = Live::open(
        settings(Locality::SameMachine),
        key(),
        Box::new(MemoryStore::default()),
        runtime.handle().clone(),
    )
    .unwrap();
    for result in [
        other.edit_listing(&host, 8, &ListingChange::Weight(1)),
        other.remove_from_directory(&host, 8),
        other.keep_directory(8),
    ] {
        assert_eq!(result.unwrap_err().code, Code::Forbidden);
    }
    let (latest, _) = published(&runtime, &relay, &owner);
    assert_eq!(latest.revision, 8);
}

/// A remote `coder` stand-in. `host serve` records its process and the real
/// test host's loopback port, which the test wrote to `$HOME/port`, and
/// waits; `host invite` prints the invitation the real host issued.
const FAKE_CODER: &str = r#"#!/bin/sh
case "${1:-}" in
  --version) echo 'coder 0.0.0-fake'; exit 0 ;;
  host)
    case "${2:-}" in
      serve)
        d="$HOME/.openagents/host"
        mkdir -p "$d"
        printf 'schema=openagents.coder.host-runtime.v1\npid=%s\nport=%s\n' "$$" "$(cat "$HOME/port")" > "$d/runtime.tmp"
        mv "$d/runtime.tmp" "$d/runtime"
        sleep 600 &
        child=$!
        trap 'kill "$child" 2>/dev/null; exit 0' HUP INT TERM
        wait "$child"
        exit 0 ;;
      invite) cat "$HOME/invitation"; exit 0 ;;
    esac ;;
esac
exit 2
"#;

fn alive(pid: i32) -> bool {
    // SAFETY: signal zero sends nothing.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn stop(pid: i32) {
    if pid > 1 {
        // SAFETY: the identifier belongs to a process this test started.
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

/// Stands in for `ssh -L`. The fake `ssh` records `PID 127.0.0.1:L:...:R`
/// and sleeps; while that process lives, this forwards local port `L` to the
/// remote port `R`. When it ends, the port closes and every forwarded
/// connection drops, as they do when a real `ssh` tunnel dies.
struct Forwarder {
    stop: Arc<AtomicBool>,
    tunnels: Arc<Mutex<Vec<i32>>>,
}

impl Forwarder {
    fn start(dir: &Path) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let tunnels = Arc::new(Mutex::new(Vec::new()));
        let (file, done, seen) = (dir.join("tunnel"), stop.clone(), tunnels.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if let Some((pid, local, remote)) = std::fs::read_to_string(&file)
                    .ok()
                    .and_then(|line| parse_tunnel(&line))
                    && !seen.lock().unwrap().contains(&pid)
                {
                    seen.lock().unwrap().push(pid);
                    std::thread::spawn(move || forward(pid, local, remote));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        Self { stop, tunnels }
    }

    /// The fake `ssh` processes that carried tunnels, oldest first.
    fn pids(&self) -> Vec<i32> {
        self.tunnels.lock().unwrap().clone()
    }
}

impl Drop for Forwarder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for pid in self.pids() {
            stop(pid);
        }
    }
}

fn parse_tunnel(line: &str) -> Option<(i32, u16, u16)> {
    let (pid, spec) = line.trim().split_once(' ')?;
    let parts: Vec<&str> = spec.split(':').collect();
    let [_, local, _, remote] = parts.as_slice() else {
        return None;
    };
    Some((pid.parse().ok()?, local.parse().ok()?, remote.parse().ok()?))
}

fn forward(pid: i32, local: u16, remote: u16) {
    let deadline = Instant::now() + WAIT;
    let listener = loop {
        match TcpListener::bind(("127.0.0.1", local)) {
            Ok(listener) => break listener,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => panic!("the tunnel port {local} did not bind: {error}"),
        }
    };
    listener.set_nonblocking(true).unwrap();
    let mut open: Vec<TcpStream> = Vec::new();
    while alive(pid) {
        match listener.accept() {
            Ok((client, _)) => {
                client.set_nonblocking(false).unwrap();
                let Ok(server) = TcpStream::connect(("127.0.0.1", remote)) else {
                    continue;
                };
                for (from, to) in [
                    (client.try_clone().unwrap(), server.try_clone().unwrap()),
                    (server.try_clone().unwrap(), client.try_clone().unwrap()),
                ] {
                    std::thread::spawn(move || pipe(from, to));
                }
                open.extend([client, server]);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
    drop(listener);
    for stream in open {
        let _ = stream.shutdown(Shutdown::Both);
    }
}

fn pipe(mut from: TcpStream, mut to: TcpStream) {
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        match from.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                if std::io::Write::write_all(&mut to, &buffer[..read]).is_err() {
                    break;
                }
            }
        }
    }
    let _ = to.shutdown(Shutdown::Write);
}

/// A fake remote machine for one SSH destination: its home, the fake `ssh`,
/// and the release archive. Stops its processes when the test ends.
struct Remote {
    dir: PathBuf,
    home: PathBuf,
    program: PathBuf,
    release: coder_ssh::Release,
    extra: Vec<i32>,
}

impl Remote {
    fn new(dir: &Path, port: u16, invitation: &str) -> Self {
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(home.join("port"), port.to_string()).unwrap();
        std::fs::write(home.join("invitation"), format!("{invitation}\n")).unwrap();
        let archive = fake_ssh::archive(dir, FAKE_CODER, "coder.tar.gz");
        let program = fake_ssh::shim(dir, &home, "sh");
        let (os, arch) = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => (coder_ssh::Os::Macos, coder_ssh::Arch::Aarch64),
            ("macos", _) => (coder_ssh::Os::Macos, coder_ssh::Arch::X86_64),
            (_, "aarch64") => (coder_ssh::Os::Linux, coder_ssh::Arch::Aarch64),
            _ => (coder_ssh::Os::Linux, coder_ssh::Arch::X86_64),
        };
        let release = coder_ssh::Release::new(vec![coder_ssh::Artifact {
            os,
            arch,
            sha256: fake_ssh::sha256_file(&archive),
            archive,
        }])
        .unwrap();
        Self {
            dir: dir.to_owned(),
            home,
            program,
            release,
            extra: Vec::new(),
        }
    }

    fn settings(&self, owner: &SecretKey, relay: &str) -> Settings {
        let mut setup = SshSetup::coder(self.release.clone(), &pubkey(owner), relay, true).unwrap();
        setup.program = Some(self.program.clone());
        // The client runs on another machine: the host's loopback hints are
        // never offered, so only the tunnel or the relay can carry it.
        let mut settings = settings(Locality::OtherMachine);
        settings.ssh = Some(setup);
        settings
    }

    /// The process the remote runtime record names.
    fn host_pid(&self) -> Option<i32> {
        std::fs::read_to_string(self.home.join(".openagents/host/runtime"))
            .ok()?
            .lines()
            .find_map(|line| line.strip_prefix("pid="))
            .and_then(|pid| pid.parse().ok())
    }

    /// Start a host outside any launcher: a process whose runtime record
    /// names the real host's port. `up` adopts it as external.
    fn start_external(&mut self, port: u16) -> i32 {
        let child = std::process::Command::new("sleep")
            .arg("600")
            .spawn()
            .unwrap();
        let pid = i32::try_from(child.id()).unwrap();
        self.extra.push(pid);
        let record = self.home.join(".openagents/host");
        std::fs::create_dir_all(&record).unwrap();
        std::fs::write(
            record.join("runtime"),
            format!("schema=openagents.coder.host-runtime.v1\npid={pid}\nport={port}\n"),
        )
        .unwrap();
        // The child is reaped when the test process exits.
        std::mem::forget(child);
        pid
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        if let Some(pid) = self.host_pid() {
            stop(pid);
        }
        for pid in &self.extra {
            stop(*pid);
        }
    }
}

/// Run **Connect over SSH** to `destination` until the host is added.
fn connect_over_ssh(computers: &mut Computers, destination: &str) -> String {
    press(computers, "ssh-connect");
    submit(computers, InputPurpose::SshDestination, destination);
    let deadline = Instant::now() + WAIT;
    loop {
        computers.refresh().unwrap();
        match computers.snapshot().ssh.clone().unwrap().stage {
            SshStage::Added { host } => return host,
            SshStage::Failed { reason } => panic!("SSH setup failed: {reason}"),
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "timed out in the SSH setup:\n{}",
            all_text(computers)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Run **Remove over SSH** on row 0 until it finishes.
fn remove_over_ssh(computers: &mut Computers) -> SshRemoval {
    press(computers, "host-0-ssh-remove");
    press(computers, "host-0-ssh-remove-yes");
    let deadline = Instant::now() + WAIT;
    loop {
        computers.refresh().unwrap();
        match computers.snapshot().ssh.clone().unwrap().stage {
            SshStage::Removed { removal, .. } => return removal,
            SshStage::RemoveFailed { reason, .. } => panic!("remove failed: {reason}"),
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "timed out in the SSH remove:\n{}",
            all_text(computers)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_ssh_tunnel_carries_the_host_until_it_dies_and_remove_stops_a_managed_host() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().canonicalize().unwrap();
    let runtime = runtime();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let (store, host, port) = serve_host(&dir.join("devbox-host"), &owner, &relay);
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();
    let remote = Remote::new(&dir.join("remote"), port, &invitation.code);
    let forwarder = Forwarder::start(&remote.dir);

    let mut computers = open(remote.settings(&owner, &relay), &runtime);
    let added = connect_over_ssh(&mut computers, "devbox");
    assert_eq!(added, host);
    press(&mut computers, "first-run-continue");

    // The connector uses the tunnel's loopback port as a direct route.
    until(&mut computers, "the host online through the tunnel", |c| {
        c.snapshot().hosts[0].tunnel
            == Some(Tunnel {
                open: true,
                in_use: true,
            })
            && text(c, "host-0-status")
                .is_some_and(|s| s.starts_with("Online through the SSH tunnel"))
    });
    assert_eq!(
        text(&computers, "host-0-tunnel").as_deref(),
        Some("SSH tunnel open. This device connects through it.")
    );
    let tunnels = forwarder.pids();
    assert_eq!(tunnels.len(), 1, "one tunnel");
    let managed = remote
        .host_pid()
        .expect("the managed host's runtime record");

    // Kill the tunnel: the relay carries the host, which keeps running.
    stop(tunnels[0]);
    until(&mut computers, "the host online through its relay", |c| {
        c.snapshot().hosts[0].tunnel
            == Some(Tunnel {
                open: false,
                in_use: false,
            })
            && text(c, "host-0-status").is_some_and(|s| s.starts_with("Online through a relay"))
    });
    assert_eq!(
        text(&computers, "host-0-tunnel").as_deref(),
        Some("SSH tunnel closed. This device uses the relay; the computer keeps running.")
    );
    assert!(alive(managed), "a dead tunnel never stops the host");
    // The relay route carries operations.
    press(&mut computers, "host-0-access");
    press(&mut computers, "devices-refresh");
    press(&mut computers, "tab-computers");

    // Remove over SSH stops the host this app's setup started, then forgets
    // the computer.
    assert_eq!(remove_over_ssh(&mut computers), SshRemoval::Stopped);
    assert!(computers.snapshot().hosts.is_empty());
    assert_eq!(
        text(&computers, "computers-ssh").as_deref(),
        Some("Removed devbox. Its host on devbox stopped, and this device forgot it.")
    );
    let deadline = Instant::now() + WAIT;
    while alive(managed) {
        assert!(Instant::now() < deadline, "the managed host did not stop");
        std::thread::sleep(Duration::from_millis(50));
    }
    let calls = std::fs::read_to_string(remote.dir.join("calls")).unwrap();
    assert!(calls.lines().any(|line| line.contains("-L 127.0.0.1:")));
}

#[test]
fn remove_over_ssh_detaches_from_an_external_host() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().canonicalize().unwrap();
    let runtime = runtime();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let (store, host, port) = serve_host(&dir.join("devbox-host"), &owner, &relay);
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();
    let mut remote = Remote::new(&dir.join("remote"), port, &invitation.code);
    let external = remote.start_external(port);
    let _forwarder = Forwarder::start(&remote.dir);

    let mut computers = open(remote.settings(&owner, &relay), &runtime);
    assert_eq!(connect_over_ssh(&mut computers, "me@devbox"), host);
    press(&mut computers, "first-run-continue");
    until(
        &mut computers,
        "the adopted host online through the tunnel",
        |c| {
            c.snapshot().hosts[0]
                .tunnel
                .is_some_and(|tunnel| tunnel.in_use)
        },
    );

    assert_eq!(remove_over_ssh(&mut computers), SshRemoval::Detached);
    assert!(computers.snapshot().hosts.is_empty());
    assert!(
        text(&computers, "computers-ssh")
            .unwrap()
            .contains("was already running before setup, so it keeps running")
    );
    // The external host keeps running.
    std::thread::sleep(Duration::from_millis(500));
    assert!(alive(external));
    assert_eq!(remote.host_pid(), Some(external));
}
