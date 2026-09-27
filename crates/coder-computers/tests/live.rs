//! The live Computers service against a real `coder host serve` on the
//! synthetic NIP-42 relay.
//!
//! - The owner directory: this device enrolls by invitation, holds the owner
//!   key after the person enters it, adds its host to the directory, and
//!   follows a later revision's labels and weights. A listed host this
//!   device has no grant for shows as not enrolled.
//! - SSH: "Add a computer, then SSH" through `coder-ssh` and its fake `ssh`
//!   harness, with a password prompt answered through an input request,
//!   through invitation redemption to Online.
//!
//! One machine, loopback only. The fake `ssh` runs remote commands in a
//! temporary home; the real `~/.ssh` and `~/.openagents` are never read or
//! written. This is not a real `sshd`, device, or production-relay test.
#![cfg(unix)]

#[path = "../../coder-control/src/tests/relay.rs"]
#[allow(dead_code)]
mod relay;

#[path = "../../coder-ssh/tests/support/fake_ssh.rs"]
mod fake_ssh;

use coder_computers::live::{Live, Locality, MemoryStore, Settings, SshSetup};
use coder_computers::{
    Capabilities, Computers, DirectoryState, InputPurpose, Platform, Snapshot, SshStage,
};
use coder_host::access::host::Host;
use coder_host::access::{RelayPolicy, Rights};
use coder_host::client::{fetch_directory_revisions, publish_directory};
use coder_host::reach::directory::HostEntry;
use coder_host::reach::placement::{Assessment, Skip};
use coder_host::reach::presence::{ClientProfile, VersionRange};
use coder_host::{NoTasks, Tasks};
use rust_native::{Activation, Element, Node};
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const WAIT: Duration = Duration::from_secs(45);
const PASSWORD: &str = "correct horse battery staple";

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
/// serve` does, on its own runtime. Returns the store and the host key.
fn serve_host(root: &Path, owner: &SecretKey, relay: &str) -> (Host, String) {
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
    (store, host)
}

fn settings() -> Settings {
    let mut settings = Settings::new(Platform::Terminal);
    settings.policy = POLICY;
    settings.locality = Locality::SameMachine;
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
        "computers:live-test",
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

fn all_text(computers: &Computers) -> String {
    texts(computers)
        .into_iter()
        .map(|(key, value, _)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn press(computers: &mut Computers, key: &str) {
    computers.refresh().unwrap();
    let enabled = texts(computers)
        .into_iter()
        .find(|(node, ..)| node == key)
        .and_then(|(_, _, enabled)| enabled);
    assert_eq!(
        enabled,
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

fn row(snapshot: &Snapshot, host: &str) -> Option<usize> {
    snapshot.hosts.iter().position(|record| record.key == host)
}

#[test]
fn directory_hosts_show_owner_labels_weights_and_revisions() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = runtime();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let (store, host) = serve_host(&temp.path().join("studio"), &owner, &relay);
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();

    let mut computers = open(settings(), &runtime);
    press(&mut computers, "invite-paste");
    submit(&mut computers, InputPurpose::Invitation, &invitation.code);
    // Without the owner key the device lists only what it enrolled with.
    assert_eq!(computers.snapshot().directory, DirectoryState::NoOwnerKey);
    assert_eq!(computers.snapshot().hosts.len(), 1);
    assert!(computers.snapshot().hosts[0].label.starts_with("Computer "));
    press(&mut computers, "first-run-continue");

    // A key that is not the owner is refused before it is saved.
    press(&mut computers, "directory-owner-key");
    let input = computers.input().unwrap().clone();
    assert_eq!(input.purpose, InputPurpose::OwnerKey);
    assert!(input.secret);
    assert!(
        computers
            .submit(&input.token, &key().display_secret().to_string())
            .is_err()
    );
    assert_eq!(computers.snapshot().directory, DirectoryState::NoOwnerKey);
    submit(
        &mut computers,
        InputPurpose::OwnerKey,
        &nostr::nip19::encode_nsec(&owner.secret_bytes()),
    );
    until(&mut computers, "the empty directory", |c| {
        matches!(
            c.snapshot().directory,
            DirectoryState::Current { revision: None, .. }
        )
    });

    // The owner adds the enrolled host to the directory.
    until(&mut computers, "the host online", |c| {
        text(c, "host-0-status").is_some_and(|s| s.starts_with("Online"))
    });
    press(&mut computers, "host-0-list");
    submit(&mut computers, InputPurpose::DirectoryLabel, "Studio");
    let snapshot = computers.snapshot();
    assert!(matches!(
        snapshot.directory,
        DirectoryState::Current {
            revision: Some(1),
            ..
        }
    ));
    assert_eq!(snapshot.hosts[0].label, "Studio");
    assert_eq!(snapshot.hosts[0].weight(), coder_computers::LOCAL_WEIGHT);
    assert_eq!(
        text(&computers, "host-0-directory").as_deref(),
        Some("In your directory. Weight 100.")
    );
    let published = runtime
        .block_on(fetch_directory_revisions(&relay, &owner, POLICY))
        .unwrap();
    let (first, mailbox) = published
        .iter()
        .max_by_key(|(directory, _)| directory.revision)
        .cloned()
        .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.entry(&host).unwrap().label, "Studio");

    // Another owner device publishes revision 2: a new label and weight for
    // this host, and a host this device holds no grant for.
    let unenrolled = pubkey(&key());
    let mut second = first.clone();
    second.revision = 2;
    second.issued_at = first.issued_at + 1;
    for entry in &mut second.hosts {
        if entry.host == host {
            entry.label = "Studio Mac".into();
            entry.weight = 300;
        }
    }
    second.hosts.push(HostEntry {
        host: unenrolled.clone(),
        label: "Build box".into(),
        relays: vec![relay.clone()],
        weight: 0,
        added_at: second.issued_at,
    });
    runtime
        .block_on(publish_directory(
            &relay,
            &owner,
            &second,
            &mailbox,
            now() + 3600,
            POLICY,
        ))
        .unwrap();
    until(&mut computers, "revision 2", |c| {
        matches!(
            c.snapshot().directory,
            DirectoryState::Current {
                revision: Some(2),
                ..
            }
        )
    });
    let snapshot = computers.snapshot().clone();
    let studio = row(&snapshot, &host).unwrap();
    assert_eq!(snapshot.hosts[studio].label, "Studio Mac");
    assert_eq!(snapshot.hosts[studio].weight(), 300);
    let build = row(&snapshot, &unenrolled).unwrap();
    assert_eq!(snapshot.hosts[build].label, "Build box");
    let status = text(&computers, &format!("host-{build}-status")).unwrap();
    assert!(
        status.starts_with("Not enrolled: it's in your directory"),
        "{status}"
    );
    assert_eq!(
        text(&computers, &format!("host-{build}-directory")).as_deref(),
        Some("In your directory. Weight 0: not used for new work.")
    );
    // A directory entry carries no connection to switch or forget.
    assert!(text(&computers, &format!("host-{build}-switch")).is_none());
    assert!(text(&computers, &format!("host-{build}-forget")).is_none());

    // The directory's weights reach placement: the unenrolled host is not
    // admitted, and the enrolled host's score carries its weight of 300.
    let client = ClientProfile {
        protocol: coder_host::reach::PROTOCOL_VERSION,
        accepts: VersionRange {
            min: coder_host::reach::PROTOCOL_VERSION,
            max: coder_host::reach::PROTOCOL_VERSION,
        },
    };
    for assessment in snapshot.assess_placement(&client) {
        match assessment {
            Assessment::Skipped {
                host: skipped,
                reason,
            } if skipped == unenrolled => {
                assert_eq!(reason, Skip::NotAdmitted);
            }
            Assessment::Eligible {
                host: eligible,
                score,
            } => {
                assert_eq!(eligible, host);
                assert_eq!(score % 300, 0);
            }
            // The test machine may withhold telemetry or be busy.
            Assessment::Skipped { reason, .. } => {
                assert!(
                    matches!(
                        reason,
                        Skip::NoTelemetry | Skip::Overloaded | Skip::NoPresence
                    ),
                    "{reason:?}"
                );
            }
        }
    }

    // A lower revision never replaces a higher one.
    let mut stale = first;
    stale.issued_at += 5;
    stale.hosts[0].label = "Rolled back".into();
    runtime
        .block_on(publish_directory(
            &relay,
            &owner,
            &stale,
            &mailbox,
            now() + 3600,
            POLICY,
        ))
        .unwrap();
    std::thread::sleep(Duration::from_secs(3));
    computers.refresh().unwrap();
    let snapshot = computers.snapshot();
    assert_eq!(
        snapshot.hosts[row(snapshot, &host).unwrap()].label,
        "Studio Mac"
    );
}

/// A remote `coder` stand-in. `host serve` records its process and loopback
/// port the way a resident host does and waits; `host invite` prints the
/// invitation the test's real host issued.
const FAKE_CODER: &str = r#"#!/bin/sh
case "${1:-}" in
  --version) echo 'coder 0.0.0-fake'; exit 0 ;;
  host)
    case "${2:-}" in
      serve)
        d="$HOME/.openagents/host"
        mkdir -p "$d"
        printf 'schema=openagents.coder.host-runtime.v1\npid=%s\nport=%s\n' "$$" 47011 > "$d/runtime.tmp"
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

/// Stops the fake remote host when the test ends, pass or fail.
struct Remote {
    home: PathBuf,
}

impl Remote {
    fn pid(&self) -> Option<i32> {
        std::fs::read_to_string(self.home.join(".openagents/host/runtime"))
            .ok()?
            .lines()
            .find_map(|line| line.strip_prefix("pid="))
            .and_then(|pid| pid.parse().ok())
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        if let Some(pid) = self.pid().filter(|pid| *pid > 1) {
            // SAFETY: the identifier was recorded by this test's fake host.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal zero sends nothing.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[test]
fn add_a_computer_over_ssh_enrolls_through_its_invitation() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().canonicalize().unwrap();
    let runtime = runtime();
    let (relay, _relay_task, _events) = runtime.block_on(relay::start());
    let owner = key();
    let (store, host) = serve_host(&dir.join("devbox-host"), &owner, &relay);

    // The fake remote machine: its home holds the invitation its host
    // prints, and every ssh invocation asks for a password.
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let remote = Remote { home: home.clone() };
    let invitation = store
        .invite(&relay, Rights::all(), now(), now() + 3600)
        .unwrap();
    std::fs::write(home.join("invitation"), format!("{}\n", invitation.code)).unwrap();
    std::fs::write(dir.join("password"), PASSWORD).unwrap();
    // The tunnel route is covered in `tests/edits.rs`; here port forwarding
    // is refused, and the host stays on its other routes.
    std::fs::write(dir.join("no-tunnel"), "").unwrap();
    let archive = fake_ssh::archive(&dir, FAKE_CODER, "coder.tar.gz");
    let program = fake_ssh::shim(&dir, &home, "sh");
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
    let mut setup = SshSetup::coder(release, &pubkey(&owner), &relay, true).unwrap();
    setup.program = Some(program);
    let mut settings = settings();
    settings.ssh = Some(setup);

    let mut computers = open(settings, &runtime);
    // First run offers SSH on a terminal client.
    press(&mut computers, "ssh-connect");
    submit(&mut computers, InputPurpose::SshDestination, "devbox");
    // Each ssh invocation asks; the terminal answers through an input
    // request that it masks.
    let deadline = Instant::now() + WAIT;
    let mut answered = 0;
    loop {
        computers.refresh().unwrap();
        let stage = computers.snapshot().ssh.clone().unwrap().stage;
        match stage {
            SshStage::Added { .. } => break,
            SshStage::Failed { reason } => panic!("SSH setup failed: {reason}"),
            SshStage::Prompt { text, .. } => {
                assert!(text.contains("password"), "{text}");
                let input = computers.input().unwrap().clone();
                assert_eq!(input.purpose, InputPurpose::SshPassword);
                assert!(input.secret);
                assert_eq!(input.prompt, text);
                computers.submit(&input.token, PASSWORD).unwrap();
                answered += 1;
            }
            SshStage::Starting | SshStage::Enrolling => {}
            stage => panic!("a setup never reaches {stage:?}"),
        }
        assert!(
            Instant::now() < deadline,
            "timed out in the SSH setup:\n{}",
            all_text(&computers)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // Install asks for the archive, uploads it, installs, then invites.
    assert!(answered >= 3, "answered {answered} prompts");
    // The password never reached an environment, an argument, or a file.
    let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
    assert!(!calls.contains(PASSWORD));

    let snapshot = computers.snapshot().clone();
    let index = row(&snapshot, &host).expect("the SSH host in the list");
    assert_eq!(snapshot.hosts[index].label, "devbox");
    assert_eq!(snapshot.hosts[index].ssh.as_deref(), Some("devbox"));
    assert_eq!(
        text(&computers, "ssh-status").as_deref(),
        Some("Added devbox over SSH.")
    );
    // The SSH host satisfies first run.
    press(&mut computers, "first-run-continue");
    assert_eq!(
        text(&computers, &format!("host-{index}-ssh")).as_deref(),
        Some("Set up over SSH on devbox.")
    );
    until(&mut computers, "the SSH host online", |c| {
        text(c, &format!("host-{index}-status")).is_some_and(|s| s.starts_with("Online"))
    });
    // The host's grant, not the SSH login, now decides access.
    // The serving host can hold the store lock for a moment; retry as
    // `coder host list` does.
    let deadline = Instant::now() + WAIT;
    let device = loop {
        match store.devices(now()) {
            Ok(devices) => break devices,
            Err(error) => {
                assert!(Instant::now() < deadline, "{error:?}");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    assert_eq!(device.len(), 1);
    assert_eq!(device[0].rights, Rights::all());

    // Forgetting the computer leaves the managed remote host running: only
    // an explicit remove stops it.
    let pid = remote.pid().expect("the remote host's runtime record");
    press(&mut computers, &format!("host-{index}-forget"));
    press(&mut computers, &format!("host-{index}-forget-yes"));
    assert!(row(computers.snapshot(), &host).is_none());
    assert!(alive(pid));
}
