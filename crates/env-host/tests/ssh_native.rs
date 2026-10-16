#![cfg(target_os = "linux")]
use env_host::{
    provider::{Cost, Create, Lookup, Provider, Teardown},
    ssh_native::{ClientConfig, NativeSsh, RemoteConfig},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::PermissionsExt,
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Fixture {
    temp: tempfile::TempDir,
    helper: PathBuf,
    config: PathBuf,
    program: PathBuf,
    client: ClientConfig,
    runtime: PathBuf,
    starts: PathBuf,
    root: PathBuf,
    external: Option<Child>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let program = dir.join("host");
        let source = dir.join("host.rs");
        fs::write(&source,r#"use std::sync::atomic::{AtomicUsize,Ordering};static SIGNALS:AtomicUsize=AtomicUsize::new(0);extern "C" fn term(_:i32){SIGNALS.fetch_add(1,Ordering::SeqCst);}unsafe extern "C"{fn signal(n:i32,f:extern "C" fn(i32))->usize;}fn main(){let a=std::env::args().collect::<Vec<_>>();if a.len()>3{unsafe{signal(15,term);}}let mut starts=std::fs::OpenOptions::new().create(true).append(true).open(&a[2]).unwrap();use std::io::Write;writeln!(starts,"{}",std::process::id()).unwrap();std::fs::write(&a[1],format!("schema=openagents.coder.host-runtime.v1\npid={}\nport=7000\n",std::process::id())).unwrap();loop{if a.len()>3{std::fs::write(&a[3],SIGNALS.load(Ordering::SeqCst).to_string()).unwrap();}std::thread::sleep(std::time::Duration::from_millis(20));}}"#).unwrap();
        assert!(
            Command::new("rustc")
                .arg(&source)
                .arg("-o")
                .arg(&program)
                .status()
                .unwrap()
                .success()
        );
        let config = dir.join("config.json");
        let runtime = dir.join("runtime");
        let starts = dir.join("starts");
        let root = dir.join("provider");
        let digest = format!("sha256:{:x}", Sha256::digest(fs::read(&program).unwrap()));
        let c = RemoteConfig {
            root: root.clone(),
            account: "fixture".into(),
            executable: program.clone(),
            executable_digest: digest,
            arguments: vec![runtime.display().to_string(), starts.display().to_string()],
            runtime: runtime.clone(),
        };
        fs::write(&config, serde_json::to_vec(&c).unwrap()).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        let helper = PathBuf::from(env!("CARGO_BIN_EXE_env-host"));
        let ssh = dir.join("ssh");
        // This transport never reads the real SSH configuration or reaches a host.
        fs::write(&ssh,format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}/ssh-arguments'\nexec '{}' ssh-remote --config '{}'\n",dir.display(),helper.display(),config.display())).unwrap();
        fs::set_permissions(&ssh, fs::Permissions::from_mode(0o700)).unwrap();
        let client = ClientConfig {
            destination: "fixture".into(),
            helper: helper.clone(),
            configuration: config.clone(),
            configuration_digest: nostr::contracts::digest_value(
                &serde_json::to_value(&c).unwrap(),
            )
            .unwrap(),
        };
        Self {
            temp,
            helper,
            config,
            program,
            client,
            runtime,
            starts,
            root,
            external: None,
        }
    }
    fn provider(&self) -> NativeSsh {
        NativeSsh::new(self.client.clone())
            .unwrap()
            .program(self.temp.path().join("ssh"))
    }
    fn raw(&self, mut input: Value) -> Value {
        input["configuration"] = json!(self.client.configuration_digest);
        let mut c = Command::new(&self.helper)
            .args(["ssh-remote", "--config"])
            .arg(&self.config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&input).unwrap())
            .unwrap();
        let out = c.wait_with_output().unwrap();
        assert!(
            out.status.success() || out.status.code() == Some(1),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn starts(&self) -> usize {
        fs::read_to_string(&self.starts)
            .unwrap_or_default()
            .lines()
            .count()
    }
    fn external(&mut self) {
        let mut child = Command::new(&self.program)
            .arg(&self.runtime)
            .arg(&self.starts)
            .spawn()
            .unwrap();
        let start = Instant::now();
        loop {
            if fs::read_to_string(&self.runtime)
                .unwrap_or_default()
                .contains(&format!("pid={}\n", child.id()))
            {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            assert!(child.try_wait().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(10));
        }
        self.external = Some(child);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(c) = self.external.as_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
        // Clean only fixture processes executing the unique fixture binary.
        for pid in fs::read_to_string(&self.starts)
            .unwrap_or_default()
            .lines()
            .filter_map(|s| s.parse::<u32>().ok())
        {
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            if fd < 0 {
                continue;
            }
            let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
            if fs::read_link(format!("/proc/{pid}/exe")).ok().as_ref() == Some(&self.program) {
                unsafe {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        fd.as_raw_fd(),
                        libc::SIGKILL,
                        std::ptr::null::<libc::siginfo_t>(),
                        0,
                    );
                }
            }
        }
    }
}
#[test]
fn actual_ssh_helper_recovers_lost_create_without_launch_and_stops_exact_process() {
    let fx = Fixture::new();
    let key = "11".repeat(32);
    // The original SSH reply is deliberately ignored. A new client recovers by key.
    let original = fx.raw(json!({"account":"fixture","operation":"create","key":key}));
    assert_eq!(original["status"], "found", "{original}");
    let before = fs::read(fx.root.join("processes.json")).unwrap();
    let mut provider = fx.provider();
    let Lookup::Found(r) = provider.lookup("fixture", &key) else {
        panic!("lookup");
    };
    assert_eq!(fs::read(fx.root.join("processes.json")).unwrap(), before);
    assert_eq!(fx.starts(), 1);
    let Create::Created(same, Cost::Unknown) = provider.create("fixture", &key) else {
        panic!("retry");
    };
    assert_eq!(same, r);
    assert_eq!(fx.starts(), 1);
    let component =
        nostr::contracts::jcs(&provider.observe().unwrap()["profile"]["component"]).unwrap();
    assert_eq!(
        provider
            .materialize(&r, &[("host".into(), component)])
            .unwrap()
            .len(),
        1
    );
    fs::create_dir(fx.root.join("outputs")).unwrap();
    fs::set_permissions(fx.root.join("outputs"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(fx.root.join("outputs/result.txt"), b"done").unwrap();
    assert_eq!(
        provider.collect(&r).unwrap(),
        vec![("result.txt".into(), b"done".to_vec())]
    );
    assert!(matches!(provider.destroy(&r), Teardown::Confirmed(..)));
    assert!(matches!(
        fx.provider().reconcile_teardown(&r, "destroy"),
        Teardown::Confirmed(..)
    ));
    assert_eq!(fx.starts(), 1);
    assert!(matches!(
        provider.lookup("fixture", &"22".repeat(32)),
        Lookup::Unknown(_)
    ));
    let arguments = fs::read_to_string(fx.temp.path().join("ssh-arguments")).unwrap();
    for required in [
        "ControlMaster=no",
        "ControlPath=none",
        "ForwardAgent=no",
        "ClearAllForwardings=yes",
        "BatchMode=yes",
    ] {
        assert!(arguments.contains(required));
    }
}
#[test]
fn external_adoption_is_observation_and_a_replacement_is_never_stopped() {
    let mut fx = Fixture::new();
    fx.external();
    let mut provider = fx.provider();
    let view = provider.observe().unwrap();
    assert_eq!(view["ownership"], "external");
    let r: env_host::provider::Realized = serde_json::from_value(view["resource"].clone()).unwrap();
    assert_eq!(
        provider
            .adopt("fixture", &r.resource, &r.generation, &r.ownership)
            .unwrap(),
        r
    );
    assert!(matches!(
        provider.create("fixture", &"11".repeat(32)),
        Create::Refused(_)
    ));
    assert!(matches!(provider.destroy(&r), Teardown::Unknown(_)));
    assert!(fx.external.as_mut().unwrap().try_wait().unwrap().is_none());
    assert!(matches!(provider.detach(&r), Teardown::Confirmed(..)));
    assert!(fx.external.as_mut().unwrap().try_wait().unwrap().is_none());
    fx.external.as_mut().unwrap().kill().unwrap();
    fx.external.as_mut().unwrap().wait().unwrap();
    fx.external = None;
    let key = "22".repeat(32);
    let Create::Created(owned, _) = provider.create("fixture", &key) else {
        panic!("create");
    };
    let pid = owned
        .resource
        .strip_prefix("pid:")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    assert!(fd >= 0);
    let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            libc::SIGKILL,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        );
    }
    std::thread::sleep(Duration::from_millis(30));
    fx.external();
    assert!(matches!(provider.destroy(&owned), Teardown::Unknown(_)));
    assert!(fx.external.as_mut().unwrap().try_wait().unwrap().is_none());
    assert!(matches!(
        provider.reconcile_teardown(&owned, "destroy"),
        Teardown::Unknown(_)
    ));
    assert!(fx.external.as_mut().unwrap().try_wait().unwrap().is_none());
}
#[test]
fn missing_identity_never_proves_absence_or_authorizes_relaunch() {
    let fx = Fixture::new();
    let key = "33".repeat(32);
    let initial = fx.raw(json!({"account":"fixture","operation":"lookup","key":key}));
    assert_eq!(initial["status"], "unknown");
    assert_eq!(fx.starts(), 0);
    let request = fx.raw(json!({"account":"fixture","operation":"create","key":key}));
    assert_eq!(request["status"], "found", "{request}");
    let mut book: Value =
        serde_json::from_slice(&fs::read(fx.root.join("processes.json")).unwrap()).unwrap();
    book["entries"][&key]["identity"] = Value::Null;
    fs::write(
        fx.root.join("processes.json"),
        serde_json::to_vec(&book).unwrap(),
    )
    .unwrap();
    let mut provider = fx.provider();
    assert!(matches!(
        provider.lookup("fixture", &key),
        Lookup::Unknown(_)
    ));
    assert!(matches!(
        provider.create("fixture", &key),
        Create::Unknown(_)
    ));
    assert_eq!(fx.starts(), 1);
}

#[test]
fn an_unconfirmed_stop_is_observed_after_restart_without_another_signal() {
    let mut fx = Fixture::new();
    let signals = fx.temp.path().join("signals");
    let mut config: RemoteConfig = serde_json::from_slice(&fs::read(&fx.config).unwrap()).unwrap();
    config.arguments.push(signals.display().to_string());
    fs::write(&fx.config, serde_json::to_vec(&config).unwrap()).unwrap();
    fx.client.configuration_digest =
        nostr::contracts::digest_value(&serde_json::to_value(&config).unwrap()).unwrap();
    let mut provider = fx.provider();
    let key = "44".repeat(32);
    let Create::Created(resource, _) = provider.create("fixture", &key) else {
        panic!("create");
    };
    assert!(matches!(provider.destroy(&resource), Teardown::Unknown(_)));
    assert_eq!(fs::read_to_string(&signals).unwrap(), "1");
    let mut restarted = fx.provider();
    assert!(matches!(
        restarted.reconcile_teardown(&resource, "destroy"),
        Teardown::Unknown(_)
    ));
    assert!(matches!(restarted.destroy(&resource), Teardown::Unknown(_)));
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(fs::read_to_string(&signals).unwrap(), "1");
    assert_eq!(fx.starts(), 1);
}
#[test]
fn a_changed_operator_configuration_refuses_before_launch() {
    let fx = Fixture::new();
    let mut config: RemoteConfig = serde_json::from_slice(&fs::read(&fx.config).unwrap()).unwrap();
    config.arguments.push("changed".into());
    fs::write(&fx.config, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(matches!(
        fx.provider().create("fixture", &"55".repeat(32)),
        Create::Unknown(_)
    ));
    assert_eq!(fx.starts(), 0);
}
