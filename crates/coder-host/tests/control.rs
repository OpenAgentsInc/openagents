//! The local control socket: only this user reaches it, and over it the
//! owner mints, rotates, and cancels connect codes, lists and removes
//! phones, and changes projects.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use coder_host::access::Code;
use coder_host::reach::pubkey;
use openagents_connect::control::{Autostart, Op, Reply};

#[path = "support/connect.rs"]
mod support;

#[cfg(unix)]
use support::{Options, host_with};
use support::{Phone, call, host, now};

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_socket_is_private_and_serves_only_this_user() {
    let host = host().await;
    assert_eq!(mode(&host.socket), 0o600);
    assert_eq!(mode(host.socket.parent().unwrap()), 0o700);
    let Reply::Status(status) = call(&host.socket, Op::Status {}).await.unwrap() else {
        panic!("status")
    };
    assert_eq!(status.host, host.running.host_key());
    assert_eq!(status.endpoint.len(), 64);
    assert_eq!(status.label, "Studio Mac");
    assert_eq!(status.devices, 0);
    // A second host cannot take a live socket.
    let bound =
        coder_host::control::socket::bind(&host.socket, coder_host::control::own_uid()).await;
    assert!(bound.is_err());
    host.running.shutdown().await;

    // A host that admits another user ID closes this user's connection
    // before reading a byte.
    let other = coder_host::control::own_uid().wrapping_add(1);
    let host = host_with(Options {
        uid: other,
        ..Options::default()
    })
    .await;
    let refused = call(&host.socket, Op::Status {}).await.unwrap_err();
    assert_eq!(refused.code, openagents_connect::Code::Unavailable);
    assert!(coder_host::control::socket::admits(7, Some(7)));
    assert!(!coder_host::control::socket::admits(7, Some(8)));
    assert!(!coder_host::control::socket::admits(7, None));
    host.running.shutdown().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_widened_directory_is_made_private_and_a_stale_socket_is_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("c");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = dir.join("control.sock");
    // A socket left by a host that is gone.
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    let bound = coder_host::control::socket::bind(&path, coder_host::control::own_uid())
        .await
        .unwrap();
    assert_eq!(bound.path(), path);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&path), 0o600);
    // Anything but a socket at the path is refused, never removed.
    drop(bound);
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"not a socket").unwrap();
    assert!(
        coder_host::control::socket::bind(&path, coder_host::control::own_uid())
            .await
            .is_err()
    );
    assert!(path.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn codes_are_minted_rotated_and_cancelled_over_the_socket() {
    let host = host().await;
    let (first_id, first) = host.code().await;
    let Reply::Invite { rights, .. } = call(&host.socket, Op::InviteCreate {}).await.unwrap()
    else {
        panic!("invite")
    };
    // Every code carries the full pairing rights; nothing narrows them.
    assert_eq!(
        rights,
        [
            "observe",
            "operate",
            "terminal",
            "review",
            "access_read",
            "access_admin"
        ]
    );
    // Rotation: a new code, then the one it replaced is cancelled.
    let (_, second) = host.code().await;
    let Reply::Cancelled { count } = call(
        &host.socket,
        Op::InviteCancel {
            invitation: first_id,
        },
    )
    .await
    .unwrap() else {
        panic!("cancelled")
    };
    assert_eq!(count, 1);
    let Reply::Status(status) = call(&host.socket, Op::Status {}).await.unwrap() else {
        panic!("status")
    };
    assert_eq!(status.outstanding_invitations, 2);

    // The replaced code is refused with a signed `revoked`.
    let phone = Phone::new().await;
    let (reply, refused, _) = phone.redeem(&first, &host.relay, now()).await;
    assert!(reply.is_some());
    assert_eq!(refused.unwrap_err().code, Code::Revoked);

    // Hiding the window cancels every outstanding code.
    let Reply::Cancelled { count } = call(&host.socket, Op::InviteCancelAll {}).await.unwrap()
    else {
        panic!("cancelled")
    };
    assert_eq!(count, 2);
    let (_, refused, _) = phone.redeem(&second, &host.relay, now()).await;
    assert_eq!(refused.unwrap_err().code, Code::Revoked);

    // A fresh code pairs, and the phone is listed, then removed.
    let (_, third) = host.code().await;
    let (_, access, _) = phone.redeem(&third, &host.relay, now()).await;
    access.unwrap();
    let Reply::Devices { devices } = call(&host.socket, Op::DeviceList {}).await.unwrap() else {
        panic!("devices")
    };
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device, pubkey(&phone.secret));
    assert_eq!(devices[0].rights.len(), 6);
    assert!(devices[0].rights.iter().any(|right| right == "terminal"));
    assert!(!devices[0].revoked);
    let Reply::Revoked { epoch, .. } = call(
        &host.socket,
        Op::DeviceRevoke {
            device: pubkey(&phone.secret),
        },
    )
    .await
    .unwrap() else {
        panic!("revoked")
    };
    assert_eq!(epoch, 1);
    let Reply::Devices { devices } = call(&host.socket, Op::DeviceList {}).await.unwrap() else {
        panic!("devices")
    };
    assert!(devices[0].revoked);
    // Revoking a key the host never enrolled is refused.
    let Reply::Refused { code, .. } = call(
        &host.socket,
        Op::DeviceRevoke {
            device: pubkey(&support::key()),
        },
    )
    .await
    .unwrap() else {
        panic!("refused")
    };
    assert_eq!(code, "forbidden");
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_change_is_recorded_and_asks_the_host_to_start_again() {
    let host = host().await;
    let checkout = host.temp.path().join("site");
    std::fs::create_dir_all(&checkout).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .args([
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@example.invalid",
            ])
            .args(args)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "--quiet"]);
    // A checkout with no commit has nothing to copy for Coder.
    let Reply::Refused { .. } = call(
        &host.socket,
        Op::ProjectAdd {
            path: checkout.display().to_string(),
        },
    )
    .await
    .unwrap() else {
        panic!("a checkout with no commit is refused")
    };
    std::fs::write(checkout.join("README.md"), "site\n").unwrap();
    git(&["add", "README.md"]);
    git(&["commit", "--quiet", "-m", "first"]);
    let plain = host.temp.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();

    let Reply::Refused { .. } = call(
        &host.socket,
        Op::ProjectAdd {
            path: plain.display().to_string(),
        },
    )
    .await
    .unwrap() else {
        panic!("a folder that is not a Git checkout is refused")
    };
    let Reply::Projects { projects } = call(
        &host.socket,
        Op::ProjectAdd {
            path: checkout.display().to_string(),
        },
    )
    .await
    .unwrap() else {
        panic!("projects")
    };
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].label, "site");
    // The person's checkout holds its own Git directory, so the host admits
    // a detached worktree of it under its root, where Coder may write.
    let admitted = std::path::PathBuf::from(&projects[0].path);
    assert!(admitted.starts_with(host.root.canonicalize().unwrap().join("projects")));
    assert!(admitted.join(".git").is_file());
    // The desktop shows the folder the person picked, not the worktree.
    assert_eq!(
        projects[0].folder.as_deref(),
        Some(
            checkout
                .canonicalize()
                .unwrap()
                .display()
                .to_string()
                .as_str()
        )
    );
    // Git for Windows checks text out with CRLF line ends by default.
    assert_eq!(
        std::fs::read_to_string(admitted.join("README.md"))
            .unwrap()
            .replace("\r\n", "\n"),
        "site\n"
    );
    // Picking the same folder again admits the same worktree.
    let Reply::Projects { projects } = call(
        &host.socket,
        Op::ProjectAdd {
            path: checkout.display().to_string(),
        },
    )
    .await
    .unwrap() else {
        panic!("projects")
    };
    assert_eq!(projects.len(), 1);
    tokio::time::timeout(Duration::from_secs(2), host.running.restart_requested())
        .await
        .expect("the host asks to start again");
    let settings = coder_host::settings::ServeSettings::load(&host.root).unwrap();
    assert!(settings.workspaces.contains_key("site"));
    assert_eq!(settings.relays, std::slice::from_ref(&host.relay));

    // Auto-start reads as off, and this host cannot change it.
    let Reply::Autostart { policy } = call(&host.socket, Op::AutostartGet {}).await.unwrap() else {
        panic!("autostart")
    };
    assert!(!policy.enabled);
    let Reply::Refused { code, .. } = call(
        &host.socket,
        Op::AutostartSet {
            policy: Autostart {
                enabled: true,
                projects: vec!["site".into()],
                max_running: 1,
            },
        },
    )
    .await
    .unwrap() else {
        panic!("refused")
    };
    assert_eq!(code, "unavailable");
    host.running.shutdown().await;
    assert!(!host.socket.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owner_key_is_imported_only_once_no_phone_holds_a_grant() {
    let host = host().await;
    let (_, code) = host.code().await;
    let phone = Phone::new().await;
    let (_, access, _) = phone.redeem(&code, &host.relay, now()).await;
    access.unwrap();
    let imported = support::key();
    let secret: String = imported
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let import = || {
        call(
            &host.socket,
            Op::OwnerImport {
                secret: secret.clone(),
            },
        )
    };
    let Reply::Refused { code, .. } = import().await.unwrap() else {
        panic!("refused while a phone holds a grant")
    };
    assert_eq!(code, "conflict");
    let Reply::Refused { code, .. } = call(
        &host.socket,
        Op::OwnerImport {
            secret: "not hex".into(),
        },
    )
    .await
    .unwrap() else {
        panic!("refused")
    };
    assert_eq!(code, "malformed");

    call(
        &host.socket,
        Op::DeviceRevoke {
            device: pubkey(&phone.secret),
        },
    )
    .await
    .unwrap();
    let Reply::Owner { owner } = import().await.unwrap() else {
        panic!("owner")
    };
    assert_eq!(owner, pubkey(&imported));
    // Read through the host's store, which waits for the host's own reads.
    let stored = host.running.authority().local(|store, _| store.owner());
    assert_eq!(stored.unwrap(), owner);
    tokio::time::timeout(Duration::from_secs(2), host.running.restart_requested())
        .await
        .expect("the host starts again under its new owner");
    host.running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_host_under_a_key_source_makes_its_own_owner_and_keeps_no_key_file() {
    use coder_host::serve::keys::{FileKeySource, KeyName, KeySource, Keys};
    let temp = tempfile::tempdir().unwrap();
    let (relay, _task, _) = support::relay::start().await;
    let source = std::sync::Arc::new(FileKeySource::new(temp.path().join("keys")));
    let access = temp.path().join("access");
    let mut config = coder_host::config::Config::new(access.clone(), vec![relay], 1);
    config.policy = support::POLICY;
    config.keys = Some(Keys(source.clone()));
    config.iroh = Some(coder_host::config::Iroh::loopback());
    let running = coder_host::start(config.clone(), std::sync::Arc::new(coder_host::NoTasks))
        .await
        .unwrap();
    let owner = source.load(KeyName::Owner).unwrap().unwrap();
    let host = source.load(KeyName::Host).unwrap().unwrap();
    let iroh = source.load(KeyName::HostIroh).unwrap().unwrap();
    assert_eq!(
        running.owner(),
        pubkey(&secp256k1::SecretKey::from_byte_array(*owner.expose()).unwrap())
    );
    assert_eq!(
        running.host_key(),
        pubkey(&secp256k1::SecretKey::from_byte_array(*host.expose()).unwrap())
    );
    assert_eq!(
        running.iroh_addr().unwrap().id,
        openagents_connect::iroh::SecretKey::from_bytes(iroh.expose()).public()
    );
    assert!(!access.join("host.key").exists());
    let (host_key, owner_key) = (running.host_key().to_owned(), running.owner().to_owned());
    running.shutdown().await;
    // Started again, it is the same host with the same owner.
    let running = coder_host::start(config, std::sync::Arc::new(coder_host::NoTasks))
        .await
        .unwrap();
    assert_eq!(running.host_key(), host_key);
    assert_eq!(running.owner(), owner_key);
    running.shutdown().await;
}

// The stand-in command is a shell script.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auto_start_changes_run_the_hosts_own_command() {
    let bin = tempfile::tempdir().unwrap();
    let program = bin.path().join("coder");
    // Stands in for `coder host autostart`: records its arguments and
    // writes the policy the real command would.
    std::fs::write(
        &program,
        "#!/bin/sh\neval root=\\${$#}\nprintf '%s\\n' \"$@\" > \"$root/args\"\n\
         printf '{\"schema\":\"s\",\"enabled\":true,\"workspaces\":[\"site\"],\"max_running\":2,\"engine\":{}}' > \"$root/autostart.json\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let host = host_with(Options {
        autostart: Some(program),
        ..Options::default()
    })
    .await;
    std::fs::create_dir_all(&host.root).unwrap();
    let Reply::Autostart { policy } = call(
        &host.socket,
        Op::AutostartSet {
            policy: Autostart {
                enabled: true,
                projects: vec!["site".into()],
                max_running: 2,
            },
        },
    )
    .await
    .unwrap() else {
        panic!("autostart")
    };
    assert!(policy.enabled);
    assert_eq!(policy.projects, ["site"]);
    assert_eq!(policy.max_running, 2);
    let args = std::fs::read_to_string(host.root.join("args")).unwrap();
    let root = host.root.display().to_string();
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        [
            "host",
            "autostart",
            "on",
            "--keep-engine",
            "--workspace",
            "site",
            "--max-running",
            "2",
            "--route",
            "codex:gpt-6-luna",
            "--route",
            "claude:claude-opus-5-5",
            "--root",
            root.as_str()
        ]
    );
    // Out-of-bounds requests never reach the command.
    let Reply::Refused { code, .. } = call(
        &host.socket,
        Op::AutostartSet {
            policy: Autostart {
                enabled: true,
                projects: vec!["site".into()],
                max_running: 9,
            },
        },
    )
    .await
    .unwrap() else {
        panic!("refused")
    };
    assert_eq!(code, "bounds");

    // Removing a project the policy names takes it off the policy (here
    // the last one, so the policy goes off); removing another leaves the
    // policy alone.
    let folder = host.root.join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    coder_host::settings::ServeSettings::new(
        vec![host.relay.clone()],
        std::collections::BTreeMap::from([
            ("site".into(), folder.clone()),
            ("docs".into(), folder),
        ]),
    )
    .save(&host.root)
    .unwrap();
    std::fs::remove_file(host.root.join("args")).unwrap();
    let remove = |label: &str| {
        call(
            &host.socket,
            Op::ProjectRemove {
                label: label.into(),
            },
        )
    };
    let Reply::Projects { projects } = remove("docs").await.unwrap() else {
        panic!("projects")
    };
    assert_eq!(projects.len(), 1);
    assert!(!host.root.join("args").exists());
    let Reply::Projects { projects } = remove("site").await.unwrap() else {
        panic!("projects")
    };
    assert!(projects.is_empty());
    let args = std::fs::read_to_string(host.root.join("args")).unwrap();
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        ["host", "autostart", "off", "--root", root.as_str()]
    );
    host.running.shutdown().await;
}

/// On Windows the control channel is a pipe only this user opens: the host
/// serves this user's requests over it, and while it runs no second host
/// can bind the name. Wine does not enforce the first-instance flag, so the
/// second bind is checked only on Windows.
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_pipe_serves_this_user_and_a_second_host_cannot_bind_it() {
    let host = host().await;
    let Reply::Status(status) = call(&host.socket, Op::Status {}).await.unwrap() else {
        panic!("status")
    };
    assert_eq!(status.label, "Studio Mac");
    if std::env::var_os("OPENAGENTS_TEST_UNDER_WINE").is_none() {
        let control = coder_host::config::Control {
            path: host.socket.clone(),
            root: host.root.clone(),
            autostart: None,
            uid: coder_host::control::own_uid(),
        };
        let started = std::time::Instant::now();
        assert!(coder_host::control::bind(&control).await.is_err());
        // A held name is waited for, then refused.
        assert!(started.elapsed() >= Duration::from_secs(4));
    }
    // Several requests at once each get their own instance.
    let calls = (0..4).map(|_| call(&host.socket, Op::Status {}));
    for reply in futures_util::future::join_all(calls).await {
        assert!(matches!(reply.unwrap(), Reply::Status(_)));
    }
    host.running.shutdown().await;
    assert!(call(&host.socket, Op::Status {}).await.is_err());
}
