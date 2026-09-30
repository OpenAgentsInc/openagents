//! `coder host` as separate processes: owner setup, the one-line invitation
//! an SSH launcher reads, and `serve` under the host service's contract.

// The service contract is stopped with `SIGTERM` and checks Unix modes.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use coder_host::access::protocol::HostInvitation;
use coder_host::access::{RelayPolicy, client};
use coder_host::reach::pubkey;
use secp256k1::SecretKey;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn coder(state: &Path, root: &Path, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_coder"))
        .arg("host")
        .args(args)
        .arg("--state")
        .arg(state)
        .arg("--root")
        .arg(root)
        .arg("--loopback-test")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn lines(output: &Output) -> Vec<String> {
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn wait_for(path: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_commands_and_serve_under_the_service_contract() {
    use std::os::unix::fs::PermissionsExt;
    let (relay, _relay_task, _) = relay::start().await;
    let temp = tempfile::tempdir().unwrap();
    let (state, root) = (temp.path().join("access"), temp.path().join("host"));
    let checkout = temp.path().join("checkout");
    std::fs::create_dir_all(&checkout).unwrap();
    let owner = SecretKey::new(&mut secp256k1::rand::rng());

    let workspace = format!("checkout={}", checkout.display());
    let init = coder(
        &state,
        &root,
        &[
            "init",
            "--owner",
            &pubkey(&owner),
            "--relay",
            &relay,
            "--workspace",
            &workspace,
        ],
    );
    let host = lines(&init);
    assert_eq!(host.len(), 1);
    let host = host[0].clone();
    assert_eq!(
        lines(&coder(&state, &root, &["public-key"])),
        vec![host.clone()]
    );
    let settings = std::fs::metadata(root.join("serve.json")).unwrap();
    assert_eq!(settings.permissions().mode() & 0o777, 0o600);

    // The invitation is exactly one line in the characters the SSH
    // launcher's script accepts, and the relay comes from the settings.
    let invite = lines(&coder(&state, &root, &["invite"]));
    assert_eq!(invite.len(), 1);
    let code = invite[0].clone();
    assert!(code.starts_with("coder-host:"));
    assert!(
        code.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/+=?&%@~-".contains(&b))
    );
    let now = coder_host::unix_time().unwrap();
    let parsed = HostInvitation::parse(&code, now, POLICY).unwrap();
    assert_eq!(
        (parsed.host(), parsed.relay()),
        (host.as_str(), relay.as_str())
    );

    // `serve` with no relay argument, as the host service starts it.
    // Another owner is refused before anything binds.
    let other = Command::new(env!("CARGO_BIN_EXE_coder"))
        .args(["host", "serve", "--loopback-test", "--no-runtime"])
        .args([
            "--owner",
            &pubkey(&SecretKey::new(&mut secp256k1::rand::rng())),
        ])
        .arg("--state")
        .arg(&state)
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();
    assert_eq!(other.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&other.stderr).contains("another owner"));
    let ready = temp.path().join("run/ready-4.json");
    let runtime = root.join("runtime");
    let mut serve = Command::new(env!("CARGO_BIN_EXE_coder"))
        .args(["host", "serve", "--loopback", "--loopback-test"])
        // As an SSH launcher passes it; the same owner is a no-op.
        .args(["--owner", &pubkey(&owner)])
        .arg("--state")
        .arg(&state)
        .arg("--root")
        .arg(&root)
        .arg("--tasks")
        .arg(temp.path().join("tasks"))
        .env("OPENAGENTS_HOST_READY_FILE", &ready)
        .env("OPENAGENTS_HOST_GENERATION", "4")
        .env("OPENAGENTS_HOST_VERSION", "e".repeat(64))
        .env("OPENAGENTS_HOST_LISTEN", "127.0.0.1:0")
        .env("OPENAGENTS_HOST_TRIAL", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let record: serde_json::Value = serde_json::from_str(&wait_for(&ready)).unwrap();
    assert_eq!(record["schema"], "openagents.coder.host-ready.v1");
    assert_eq!(record["generation"], 4);
    assert_eq!(record["version"], "e".repeat(64));
    let runtime_text = wait_for(&runtime);
    assert!(runtime_text.starts_with(&format!(
        "schema=openagents.coder.host-runtime.v1\npid={}\nport=",
        serve.id()
    )));

    // The resident host answers enrollment over the relay.
    let device = SecretKey::new(&mut secp256k1::rand::rng());
    let access = client::redeem(&code, &device, POLICY).await.unwrap();
    assert_eq!(access.grant.host, host);
    let listed: serde_json::Value =
        serde_json::from_slice(&coder(&state, &root, &["list", "--json"]).stdout).unwrap();
    assert_eq!(listed[0]["device"], pubkey(&device));
    assert_eq!(listed[0]["state"], "active");

    // SIGTERM stops the host cleanly and removes the runtime record.
    let pid = libc::pid_t::try_from(serve.id()).unwrap();
    // SAFETY: `kill` takes two integers; the process is this test's child.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    let status = serve.wait().unwrap();
    assert!(status.success(), "{status:?}");
    assert!(!runtime.exists());
}

/// One request on the host's control socket (`openagents.control.v1`: a
/// 4-byte big-endian length, then JSON), or `None` when nothing answers.
fn control(socket: &Path, op: serde_json::Value) -> Option<serde_json::Value> {
    use std::io::{Read, Write};
    let mut stream = std::os::unix::net::UnixStream::connect(socket).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let body =
        serde_json::to_vec(&serde_json::json!({"v": "openagents.control.v1", "id": 1, "op": op}))
            .unwrap();
    stream
        .write_all(&u32::try_from(body.len()).unwrap().to_be_bytes())
        .ok()?;
    stream.write_all(&body).ok()?;
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).ok()?;
    let mut reply = vec![0u8; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut reply).ok()?;
    serde_json::from_slice::<serde_json::Value>(&reply)
        .ok()
        .map(|response| response["result"].clone())
}

/// A rebuild replaces the running `coder` (the file is removed and written
/// again, as cargo does), then a project change asks the host to start
/// again: it starts again from the same path, as the same process, and
/// answers on its control socket with the new project. On Linux the running
/// file then reads as `PATH (deleted)`, which the host once tried to run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replaced_program_still_starts_again_after_a_project_change() {
    let (relay, _relay_task, _) = relay::start().await;
    let temp = tempfile::tempdir().unwrap();
    let (state, root) = (temp.path().join("access"), temp.path().join("host"));
    let owner = SecretKey::new(&mut secp256k1::rand::rng());
    coder(
        &state,
        &root,
        &["init", "--owner", &pubkey(&owner), "--relay", &relay],
    );
    // The host runs a copy, which this test can replace.
    let bin = temp.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let program = bin.join("coder");
    std::fs::copy(env!("CARGO_BIN_EXE_coder"), &program).unwrap();
    let socket = temp.path().join("c/control.sock");
    let log = temp.path().join("host.log");
    let mut serve = Command::new(&program)
        .args(["host", "serve", "--loopback-test", "--no-runtime"])
        .arg("--state")
        .arg(&state)
        .arg("--root")
        .arg(&root)
        .arg("--tasks")
        .arg(temp.path().join("tasks"))
        .arg("--control-socket")
        .arg(&socket)
        .env("OPENAGENTS_HOST_LISTEN", "127.0.0.1:0")
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log).unwrap())
        .spawn()
        .unwrap();
    let answers = |what: &str| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(reply) = control(&socket, serde_json::json!({"kind": "status"})) {
                return reply;
            }
            assert!(
                Instant::now() < deadline,
                "the host never answered {what}: {}",
                std::fs::read_to_string(&log).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    assert_eq!(answers("at first")["kind"], "status");

    let checkout = temp.path().join("site");
    std::fs::create_dir_all(&checkout).unwrap();
    for args in [
        &["init", "--quiet"][..],
        &["commit", "--quiet", "--allow-empty", "-m", "first"][..],
    ] {
        let status = Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .args([
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@example.invalid",
            ])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    // The rebuild: a new file at the same path.
    std::fs::remove_file(&program).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_coder"), &program).unwrap();
    let added = control(
        &socket,
        serde_json::json!({"kind": "project_add", "path": checkout.display().to_string()}),
    )
    .expect("an answer");
    assert_eq!(added["kind"], "projects", "{added}");

    // It goes away to start again, and comes back as the same process.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !std::fs::read_to_string(&log)
        .unwrap_or_default()
        .contains("starting again")
    {
        assert!(Instant::now() < deadline, "the host never started again");
        std::thread::sleep(Duration::from_millis(50));
    }
    answers("after starting again");
    let listed =
        control(&socket, serde_json::json!({"kind": "project_list"})).expect("the projects");
    assert_eq!(listed["projects"][0]["label"], "site", "{listed}");
    assert!(serve.try_wait().unwrap().is_none(), "the host exited");
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains("cannot start again"), "{text}");
    assert_eq!(text.matches("control socket").count(), 2, "{text}");

    let pid = libc::pid_t::try_from(serve.id()).unwrap();
    // SAFETY: `kill` takes two integers; the process is this test's child.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    assert!(serve.wait().unwrap().success());
}
