//! Adoption against an old-style `~/.openagents` tree in a temporary home,
//! a keychain in memory, and a recording service runner. Nothing here
//! touches the real home, keychain, launchd, or systemd.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;

use coder_access::client::{Client, finish_redeem, prepare_redeem};
use coder_access::host::{Host, Unconnected};
use coder_access::protocol::{HostInvitation, Outcome, pubkey};
use coder_access::{Operation, RelayPolicy, Rights};

use super::*;
use crate::launcher::CONFIG_SCHEMA;
use crate::service::{Output, Platform};

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
const RELAY: &str = "ws://127.0.0.1:9";
const LABEL: &str = "org.openagents.coder-host";

fn now() -> u64 {
    coder_access::unix_time().unwrap()
}

/// A keychain in memory. `corrupt` makes every read return another value,
/// like an item that did not store what it was given.
#[derive(Default)]
struct Memory {
    items: BTreeMap<String, String>,
    corrupt: bool,
}

impl Keychain for Memory {
    fn read(&mut self, account: &str) -> Result<Option<String>> {
        Ok(self.items.get(account).map(|value| {
            if self.corrupt {
                "0".repeat(value.len())
            } else {
                value.clone()
            }
        }))
    }
    fn write(&mut self, account: &str, value: &str) -> Result<()> {
        self.items.insert(account.into(), value.into());
        Ok(())
    }
}

/// launchd as a recording: the agent is loaded until `bootout`.
#[derive(Default)]
struct Launchd {
    calls: Vec<String>,
    booted_out: bool,
}

impl Runner for Launchd {
    fn run(&mut self, program: &str, args: &[String]) -> Result<Output> {
        let line = format!("{program} {}", args.join(" "));
        self.calls.push(line);
        let code = match args.first().map(String::as_str) {
            Some("bootout") => {
                self.booted_out = true;
                0
            }
            Some("print") if self.booted_out => 113,
            _ => 0,
        };
        Ok(Output {
            code: Some(code),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// An old-style home: an access store with an owner, one enrolled phone,
/// and one revoked device; the owner key file; a registered launchd agent;
/// serve settings with tailnet admission; an auto-start policy; and a task.
struct OldHome {
    _temp: tempfile::TempDir,
    home: PathBuf,
    paths: Paths,
    owner: SecretKey,
    host: String,
    phone: SecretKey,
    phone_access: coder_access::Access,
}

fn private(path: &Path) {
    fs::create_dir_all(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn old_home() -> OldHome {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().canonicalize().unwrap();
    let paths = Paths::under(&home);
    private(&home.join(".openagents"));

    // The owner key, as the old setup command wrote it.
    let owner = SecretKey::new(&mut secp256k1::rand::rng());
    private(paths.owner_key.parent().unwrap());
    write_private(
        &paths.owner_key,
        format!("{}\n", owner.display_secret()).as_bytes(),
    );

    // The access store, as the old setup command left it, and a phone
    // enrolled through an invitation.
    let access = Host::new(&paths.access, POLICY);
    let host = access.init(&pubkey(&owner)).unwrap();
    let at = now();
    let phone = SecretKey::new(&mut secp256k1::rand::rng());
    // The rights the owner's phone holds today.
    let rights = Rights::parse_list("observe,operate,terminal,review,access_read").unwrap();
    let code = access.invite(RELAY, rights, at, at + 3600).unwrap().code;
    let invitation = HostInvitation::parse(&code, at, POLICY).unwrap();
    let pending = prepare_redeem(&invitation, &phone, at, POLICY).unwrap();
    let reply = access
        .handle(&pending.event, RELAY, at, &mut Unconnected)
        .unwrap();
    let phone_access = finish_redeem(&invitation, &pending, &reply, &phone, at, POLICY).unwrap();
    // A second device, enrolled and revoked, so the store has an epoch.
    let lost = SecretKey::new(&mut secp256k1::rand::rng());
    let code = access
        .invite(RELAY, Rights::standard(), at, at + 3600)
        .unwrap()
        .code;
    let invitation = HostInvitation::parse(&code, at, POLICY).unwrap();
    let pending = prepare_redeem(&invitation, &lost, at, POLICY).unwrap();
    access
        .handle(&pending.event, RELAY, at, &mut Unconnected)
        .unwrap();
    access.revoke(&pubkey(&lost), at).unwrap();

    // The host root with its registered agent, settings, and policy.
    let layout = Layout::new(&paths.host_root);
    private(&paths.host_root);
    let config = Config {
        schema: CONFIG_SCHEMA.into(),
        label: LABEL.into(),
        platform: Platform::Macos,
        registration_dir: home.join("Library/LaunchAgents"),
        launcher: home.join(".openagents/host/bin/abc/coder-service"),
        bundle_root: home.join(".openagents/host-bundle"),
        host_args: vec!["host".into(), "serve".into()],
        state_dirs: vec![paths.tasks.clone()],
        listen: "127.0.0.1:47100".into(),
        host_key: host.clone(),
        ready_timeout_secs: 60,
        stop_grace_secs: 1,
        snapshot_max_bytes: 1 << 30,
        search_path: "/usr/bin:/bin".into(),
    };
    write_private(
        &layout.config(),
        serde_json::to_vec_pretty(&config).unwrap().as_slice(),
    );
    private(&layout.service_dir());
    let canonical = service::canonical_path(&layout, &config);
    write_private(&canonical, b"<plist/>");
    fs::create_dir_all(&config.registration_dir).unwrap();
    std::os::unix::fs::symlink(&canonical, service::registration_path(&config)).unwrap();
    write_private(
        &paths.host_root.join("serve.json"),
        br#"{"schema":"openagents.coder.host-serve-settings.v1","relays":["wss://relay.openagents.com/"],"tailnet_admission":{"rights":"standard"}}"#,
    );
    write_private(
        &paths.host_root.join("autostart.json"),
        br#"{"schema":"openagents.coder.host-autostart.v1","enabled":true}"#,
    );
    private(&paths.tasks.join("t1"));
    write_private(&paths.tasks.join("t1/task.json"), b"{\"id\":\"t1\"}");

    OldHome {
        _temp: temp,
        home,
        paths,
        owner,
        host,
        phone,
        phone_access,
    }
}

impl OldHome {
    /// The bytes of everything adoption must keep.
    fn kept_bytes(&self) -> Vec<Vec<u8>> {
        [
            self.paths.access.join("access.json"),
            self.paths.host_root.join("serve.json"),
            self.paths.host_root.join("autostart.json"),
            self.paths.tasks.join("t1/task.json"),
        ]
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect()
    }
}

#[test]
fn detection_reads_an_old_setup_without_changing_it() {
    let old = old_home();
    let before = old.kept_bytes();
    let detection = detect(&old.paths, now()).unwrap().unwrap();
    assert_eq!(detection.host, old.host);
    assert_eq!(detection.owner, pubkey(&old.owner));
    assert_eq!(detection.host_key, HostKey::File);
    assert_eq!(detection.owner_key, OwnerKey::ThisHost);
    let agent = detection.agent.as_ref().unwrap();
    assert_eq!(agent.label, LABEL);
    assert!(agent.registered && agent.definition && agent.same_host);
    assert_eq!(detection.kept.grants, 2);
    assert_eq!(detection.kept.active_grants, 1);
    assert_eq!(detection.kept.epochs, 1);
    assert!(detection.kept.serve_settings && detection.kept.tailnet_admission);
    assert!(detection.kept.autostart && detection.kept.tasks);
    assert!(detection.problems.is_empty(), "{:?}", detection.problems);
    assert_eq!(old.kept_bytes(), before);
    // The report carries no secret.
    let report = serde_json::to_string(&detection).unwrap();
    assert!(!report.contains(&old.owner.display_secret().to_string()));

    let empty = tempfile::tempdir().unwrap();
    assert_eq!(detect(&Paths::under(empty.path()), now()).unwrap(), None);
}

#[test]
fn adoption_keeps_the_host_key_grants_policy_and_tasks_and_a_paired_phone_still_passes() {
    let old = old_home();
    let before = old.kept_bytes();
    let host_secret = fs::read(old.paths.access.join("host.key")).unwrap();
    let mut keychain = Memory::default();
    let mut launchd = Launchd::default();
    let mut registered = Vec::new();
    let adopted = adopt(
        &old.paths,
        now(),
        &mut keychain,
        &mut launchd,
        &mut |detection| {
            registered.push(detection.host.clone());
            Ok(())
        },
    )
    .unwrap();

    // The same host, the keys in the keychain, and the files gone.
    assert_eq!(adopted.host, old.host);
    assert!(adopted.owner_moved);
    assert_eq!(registered, std::slice::from_ref(&old.host));
    let host_value = keychain.items[HOST_KEY_ACCOUNT].clone();
    assert_eq!(
        parse_hex(&host_value).unwrap().secret_bytes().as_slice(),
        host_secret
    );
    assert_eq!(
        keychain.items[OWNER_KEY_ACCOUNT],
        old.owner.display_secret().to_string()
    );
    assert!(!old.paths.access.join("host.key").exists());
    assert!(!old.paths.owner_key.exists());
    assert_eq!(adopted.removed.len(), 2);

    // The old agent is stopped and unregistered; its state stays.
    let uninstalled = adopted.uninstalled.unwrap();
    assert!(uninstalled.stopped && uninstalled.registration_removed);
    assert!(
        launchd
            .calls
            .iter()
            .any(|call| call.starts_with("launchctl bootout"))
    );
    assert!(
        fs::symlink_metadata(
            old.home
                .join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist"))
        )
        .is_err()
    );
    // Its record is retired, so no installer brings it back.
    assert!(!old.paths.host_root.join("service.json").exists());
    assert!(old.paths.host_root.join(RETIRED_RECORD).exists());

    // Grants, epochs, settings, auto-start policy, and tasks, byte for byte.
    assert_eq!(old.kept_bytes(), before);
    assert_eq!(adopted.kept.active_grants, 1);
    assert_eq!(adopted.kept.epochs, 1);

    // A host whose key source is the keychain serves the same store: the
    // phone enrolled before adoption still passes the handshake.
    let served = tempfile::tempdir().unwrap();
    let directory = served.path().join("access");
    private(&directory);
    fs::copy(
        old.paths.access.join("access.json"),
        directory.join("access.json"),
    )
    .unwrap();
    write_private(
        &directory.join("host.key"),
        &parse_hex(&host_value).unwrap().secret_bytes(),
    );
    write_private(&directory.join("access.lock"), b"");
    let host = Host::new(&directory, POLICY);
    assert_eq!(host.public_key().unwrap(), old.host);
    let phone = Client::device(old.phone_access.clone(), old.phone, POLICY).unwrap();
    let at = now();
    let pending = phone.prepare(Operation::ListDevices {}, at).unwrap();
    let reply = host
        .handle(&pending.event, RELAY, at, &mut Unconnected)
        .unwrap();
    let Outcome::Devices { devices } = phone.verify_reply(&pending, &reply, at).unwrap() else {
        panic!("the host did not list its devices");
    };
    assert!(
        devices
            .iter()
            .any(|device| device.device == pubkey(&old.phone))
    );

    // Adoption again finds the keys in the keychain and changes nothing.
    let detection = detect(&old.paths, now()).unwrap().unwrap();
    assert_eq!(detection.host_key, HostKey::Moved);
    assert_eq!(detection.owner_key, OwnerKey::Absent);
    assert!(detection.agent.is_none());
    assert!(
        !detection.pending(),
        "an adopted setup has nothing left to move"
    );
    let again = adopt(&old.paths, now(), &mut keychain, &mut launchd, &mut |_| {
        Ok(())
    })
    .unwrap();
    assert!(again.removed.is_empty() && again.uninstalled.is_none());
    assert_eq!(old.kept_bytes(), before);
}

#[test]
fn a_failed_read_back_leaves_the_files_and_the_agent_in_place() {
    let old = old_home();
    let before = old.kept_bytes();
    let mut keychain = Memory {
        corrupt: true,
        ..Memory::default()
    };
    let mut launchd = Launchd::default();
    let mut registered = false;
    let error = adopt(&old.paths, now(), &mut keychain, &mut launchd, &mut |_| {
        registered = true;
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("left in place"), "{error}");
    assert!(old.paths.access.join("host.key").exists());
    assert!(old.paths.owner_key.exists());
    assert!(launchd.calls.is_empty(), "{:?}", launchd.calls);
    assert!(!registered);
    assert_eq!(old.kept_bytes(), before);
    // The message never repeats a key.
    let host = detect(&old.paths, now()).unwrap().unwrap().host;
    assert!(
        !error
            .to_string()
            .contains(&old.owner.display_secret().to_string())
    );
    assert!(!error.to_string().contains(&host));
}

#[test]
fn a_different_key_in_the_keychain_is_never_overwritten() {
    let old = old_home();
    let other = SecretKey::new(&mut secp256k1::rand::rng())
        .display_secret()
        .to_string();
    let mut keychain = Memory::default();
    keychain
        .items
        .insert(HOST_KEY_ACCOUNT.into(), other.clone());
    let mut launchd = Launchd::default();
    let error = adopt(&old.paths, now(), &mut keychain, &mut launchd, &mut |_| {
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("different host key"), "{error}");
    assert_eq!(keychain.items[HOST_KEY_ACCOUNT], other);
    assert!(old.paths.access.join("host.key").exists());
    assert!(launchd.calls.is_empty());
}

#[test]
fn a_host_still_holding_the_store_keeps_its_files() {
    let old = old_home();
    // A host serving the store holds its lock.
    let store = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(old.paths.access.join("access.lock"))
        .unwrap();
    store.try_lock().unwrap();
    let mut keychain = Memory::default();
    let error = adopt(
        &old.paths,
        now(),
        &mut keychain,
        &mut Launchd::default(),
        &mut |_| Ok(()),
    )
    .unwrap_err();
    assert!(error.to_string().contains("still holds"), "{error}");
    assert!(old.paths.access.join("host.key").exists());
    assert!(old.paths.owner_key.exists());
    drop(store);
    // Once it lets go, a rerun finishes.
    adopt(
        &old.paths,
        now(),
        &mut keychain,
        &mut Launchd::default(),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(!old.paths.access.join("host.key").exists());
}

#[test]
fn another_owners_key_file_stays() {
    let old = old_home();
    let other = SecretKey::new(&mut secp256k1::rand::rng());
    write_private(
        &old.paths.owner_key,
        format!("{}\n", other.display_secret()).as_bytes(),
    );
    let detection = detect(&old.paths, now()).unwrap().unwrap();
    assert_eq!(detection.owner_key, OwnerKey::OtherOwner);
    let mut keychain = Memory::default();
    let adopted = adopt(
        &old.paths,
        now(),
        &mut keychain,
        &mut Launchd::default(),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(!adopted.owner_moved);
    assert!(old.paths.owner_key.exists());
    assert!(!keychain.items.contains_key(OWNER_KEY_ACCOUNT));
}

#[test]
fn a_host_key_that_is_not_the_stores_host_is_refused() {
    let old = old_home();
    let other = SecretKey::new(&mut secp256k1::rand::rng());
    write_private(&old.paths.access.join("host.key"), &other.secret_bytes());
    let detection = detect(&old.paths, now()).unwrap().unwrap();
    assert_eq!(detection.host_key, HostKey::Invalid);
    let mut keychain = Memory::default();
    assert!(
        adopt(
            &old.paths,
            now(),
            &mut keychain,
            &mut Launchd::default(),
            &mut |_| Ok(())
        )
        .is_err()
    );
    assert!(keychain.items.is_empty());
}

/// Writes an agent definition in the user's agent directory that runs
/// `words`, as this platform's service manager reads it.
fn write_agent(paths: &Paths, name: &str, words: &[&str]) -> PathBuf {
    let dir = paths.registrations.clone().unwrap();
    fs::create_dir_all(&dir).unwrap();
    let (path, text) = if cfg!(target_os = "macos") {
        let strings: String = words
            .iter()
            .map(|word| format!("<string>{word}</string>"))
            .collect();
        (
            dir.join(format!("{name}.plist")),
            format!(
                "<plist><dict><key>ProgramArguments</key><array>{strings}</array></dict></plist>"
            ),
        )
    } else {
        let quoted: Vec<String> = words.iter().map(|word| format!("\"{word}\"")).collect();
        (
            dir.join(format!("{name}.service")),
            format!("[Service]\nExecStart={}\n", quoted.join(" ")),
        )
    };
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn any_other_agent_serving_a_host_is_stopped_and_removed_and_others_stay() {
    let old = old_home();
    let stray = write_agent(
        &old.paths,
        "coder-host",
        &["/home/me/.local/bin/coder", "host", "serve"],
    );
    let launcher = write_agent(
        &old.paths,
        "openagents-host",
        &[
            "/home/me/.openagents/host/bin/x/coder-service",
            "--root",
            "/home/me/.openagents/host",
            "run",
        ],
    );
    let earn = write_agent(
        &old.paths,
        "coder-earn",
        &["/home/me/.local/bin/coder", "earn", "serve"],
    );
    let own = write_agent(
        &old.paths,
        APP_AGENT,
        &["/opt/OpenAgents/coder", "host", "serve", "--keychain"],
    );
    let detection = detect(&old.paths, now()).unwrap().unwrap();
    let names: Vec<&str> = detection
        .strays
        .iter()
        .map(|stray| stray.name.as_str())
        .collect();
    assert_eq!(names, ["coder-host", "openagents-host"]);
    assert!(detection.pending());

    let adopted = adopt(
        &old.paths,
        now(),
        &mut Memory::default(),
        &mut Launchd::default(),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(adopted.strays_removed, ["coder-host", "openagents-host"]);
    assert!(!stray.exists() && !launcher.exists());
    assert!(earn.exists(), "coder earn is not a host");
    assert!(own.exists(), "the desktop app's own agent stays");
    assert!(!detect(&old.paths, now()).unwrap().unwrap().pending());
}

#[test]
fn a_host_command_is_read_from_a_unit_or_a_property_list() {
    let words = |text: &str| unit_words(text);
    assert!(runs_a_host(&words(
        "[Service]\nExecStart=\"/home/me/.openagents/host/bin/4f/coder-service\" \"--root\" \"/home/me/.openagents/host\" \"run\"\n"
    )));
    assert!(runs_a_host(&words(
        "ExecStart=/usr/bin/coder host serve --iroh\n"
    )));
    assert!(!runs_a_host(&words(
        "ExecStart=/home/me/.local/bin/coder earn serve\n"
    )));
    assert!(!runs_a_host(&words("ExecStart=/usr/bin/sleep infinity\n")));
    assert!(!runs_a_host(&words("Description=coder host serve\n")));
    assert!(runs_a_host(&plist_words(
        "<array><string>/Applications/X/coder</string><string>host</string><string>serve</string></array>"
    )));
    assert!(!runs_a_host(&plist_words(
        "<string>coder</string><string>host</string><string>list</string>"
    )));
}

#[test]
#[should_panic(expected = "a test reached the real home")]
fn a_test_that_reaches_the_real_home_fails() {
    let real = test_home::real_home().expect("a home");
    let _ = Paths::under(&real);
}
