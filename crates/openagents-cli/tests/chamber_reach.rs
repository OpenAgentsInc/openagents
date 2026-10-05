//! `openagents chamber host` over a NIP-REACH channel (#10552), started
//! through the binary's own startup path against a scratch Coder host: an
//! access store, host root, and computers stores under a temporary
//! directory, file keys only, and a temporary `HOME`. Nothing here reaches
//! the person's own host, keychain, home, or relays.
//!
//! The host grants one device `world` and enrolls it as the primary
//! adventurer, grants a second `world` without a role, and grants a third
//! only `observe`. The first two join with `openagents chamber` over the
//! channel and read the world; the third is refused at the handshake.
#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use coder_access::client::{finish_redeem, prepare_redeem};
use coder_access::host::{Host, Unconnected};
use coder_access::protocol::HostInvitation;
use coder_access::{Access, RelayPolicy, Rights};
use coder_computers::live::{FileStore, Saved, SavedHost, Store as _};
use secp256k1::SecretKey;
use serde_json::{Value, json};

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
/// The relay the grants name. Nothing listens there: the chamber reads
/// grants from the store and needs no relay.
const RELAY: &str = "ws://127.0.0.1:7777";
const INSTANCE: u64 = 170;
const STARTUP: Duration = Duration::from_secs(120);

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn openagents(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command
        .current_dir(workspace())
        .env("HOME", home)
        .env_remove("OPENAGENTS_RELAY")
        .env_remove("OPENAGENTS_HOST_GENERATION");
    command
}

fn now() -> u64 {
    coder_access::unix_time().unwrap()
}

fn pubkey(secret: &SecretKey) -> String {
    coder_reach::pubkey(secret)
}

/// Redeem an invitation for `rights` on the host's store, as a device does
/// over the relay, and return the device's access record.
fn pair(state: &Path, device: &SecretKey, rights: &str) -> Access {
    let host = Host::new(state, POLICY);
    let now = now();
    let issued = host
        .invite(RELAY, Rights::parse_list(rights).unwrap(), now, now + 3600)
        .unwrap();
    let invitation = HostInvitation::parse(&issued.code, now, POLICY).unwrap();
    let pending = prepare_redeem(&invitation, device, now, POLICY).unwrap();
    let reply = host
        .handle(&pending.event, RELAY, now, &mut Unconnected)
        .unwrap();
    finish_redeem(&invitation, &pending, &reply, device, now, POLICY).unwrap()
}

/// A computers store holding `device`'s key and its grant from the host.
fn device_store(dir: &Path, device: &SecretKey, access: Access) -> PathBuf {
    let mut store = FileStore::open(dir).unwrap();
    store
        .save(&Saved {
            hosts: vec![SavedHost {
                access,
                label: "chamber".into(),
                enabled: true,
                revoked: false,
                ssh: None,
                delisted: false,
                iroh: None,
            }],
            ..Saved::default()
        })
        .unwrap();
    let key = dir.join("device.key");
    std::fs::write(&key, device.display_secret().to_string()).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    dir.to_path_buf()
}

/// The chamber host process, stopped when the test ends, pass or fail.
struct Chamber(Child);
impl Drop for Chamber {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Read the host's NDJSON until an `event` line arrives.
fn next_event(lines: &mut impl Iterator<Item = std::io::Result<String>>, event: &str) -> Value {
    let started = Instant::now();
    for line in lines.by_ref() {
        let line = line.unwrap();
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value["event"] == event {
            return value;
        }
        assert!(started.elapsed() < STARTUP, "no {event} line");
    }
    panic!("the chamber host exited before its {event} line");
}

fn status(
    home: &Path,
    address: &str,
    host: &str,
    store: &Path,
    content: &str,
) -> std::process::Output {
    openagents(home)
        .args(["--json", "chamber", "status", "--to", address])
        .args(["--instance", &INSTANCE.to_string(), "--reach", host])
        .arg("--store")
        .arg(store)
        .args(["--content", content])
        .output()
        .unwrap()
}

#[test]
fn a_reach_chamber_admits_world_grants_and_refuses_others() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let state = temp.path().join("coder-access");
    let root = temp.path().join("host");
    let assets = temp.path().join("assets");

    // The pack the host and its clients bind.
    let packed = openagents(&home)
        .args(["--json", "chamber", "pack"])
        .arg(&assets)
        .output()
        .unwrap();
    assert!(
        packed.status.success(),
        "{}",
        String::from_utf8_lossy(&packed.stderr)
    );

    // A scratch Coder host: its owner, its host key in the store, and three
    // devices with different grants.
    let owner = SecretKey::new(&mut secp256k1::rand::rng());
    coder_access::host::ensure_parent(&state).unwrap();
    let host_key = Host::new(&state, POLICY).init(&pubkey(&owner)).unwrap();
    let devices: Vec<SecretKey> = (0..3)
        .map(|_| SecretKey::new(&mut secp256k1::rand::rng()))
        .collect();
    let stores: Vec<PathBuf> = devices
        .iter()
        .zip(["observe,world", "world", "observe"])
        .enumerate()
        .map(|(i, (device, rights))| {
            let access = pair(&state, device, rights);
            device_store(&temp.path().join(format!("device-{i}")), device, access)
        })
        .collect();

    let config = temp.path().join("host.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&json!({
            "listen": "127.0.0.1:0",
            "instance": INSTANCE,
            "scene": workspace().join("assets/verse/original/ritual.json"),
            "pack": assets.join("runtime-pack.json"),
            "transport": {"type": "reach"},
            "enrollments": [{"public_key": pubkey(&devices[0]), "role": {"type": "primary"}}],
        }))
        .unwrap(),
    )
    .unwrap();

    // TLS stays the default: the access options belong to a REACH chamber.
    let tls = temp.path().join("tls.json");
    std::fs::write(
        &tls,
        br#"{"listen":"127.0.0.1:0","instance":170,"scene":"s","pack":"p","certificate_der":"c","private_key_der":"k","enrollments":[{"public_key":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","role":{"type":"primary"}}]}"#,
    )
    .unwrap();
    let refused = openagents(&home)
        .args(["chamber", "host"])
        .arg(&tls)
        .arg("--state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("REACH chamber"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );

    let mut child = openagents(&home)
        .args(["--json", "chamber", "host"])
        .arg(&config)
        .arg("--state")
        .arg(&state)
        .arg("--root")
        .arg(&root)
        .arg("--loopback-test")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let _chamber = Chamber(child);
    let mut lines = BufReader::new(stdout).lines();
    let listening = next_event(&mut lines, "listening");
    assert_eq!(listening["transport"], "reach");
    assert_eq!(listening["host"], host_key.as_str());
    let address = listening["address"].as_str().unwrap().to_owned();
    let content = listening["content"].as_str().unwrap().to_owned();
    // Without the owner key the host serves unlisted and says why.
    let directory = next_event(&mut lines, "directory");
    assert_eq!(directory["listed"], false);

    // The enrolled device controls its adventurer; the other granted
    // device watches.
    for (store, role) in [(&stores[0], "player"), (&stores[1], "spectator")] {
        let output = status(&home, &address, &host_key, store, &content);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["instance"], INSTANCE);
        assert_eq!(value["control"]["role"], role, "{value}");
        assert!(value["actors"].as_array().is_some_and(|a| !a.is_empty()));
    }

    // A grant without `world` opens no world.
    let output = status(&home, &address, &host_key, &stores[2], &content);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Chamber channel refused"), "{stderr}");

    // The chamber is still serving.
    let output = status(&home, &address, &host_key, &stores[1], &content);
    assert!(output.status.success());
}
