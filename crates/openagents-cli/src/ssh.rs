//! Hosts started or adopted over SSH, through `coder_ssh`: the launcher
//! side of the NIP-ENV SSH-launched host profile. `add` installs a pinned
//! `coder` release on an account you reach with `ssh`, starts or adopts
//! its host, and redeems the host's invitation on this device. `tunnel`
//! forwards a local port to that host's loopback listener, and `remove`
//! stops a host this launcher started or detaches from one it adopted.
//! The system `ssh` runs in batch mode, so nothing prompts; set up keys
//! first. Invitation text goes to the Computers client and nowhere else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use coder_computers::ComputersService;
use coder_ssh::{Arch, Artifact, Host, Install, Launcher, Os, Ownership, Release, Removal, Start};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::{Args, Output};

const USAGE: &str = "usage: openagents ssh COMMAND [OPTIONS]
  add USER@HOST --owner PUBKEY --archive OS/ARCH=PATH... [--relay URL]
      [--timeout SECONDS] [--rights LIST]
        Install the pinned coder release over ssh, start or adopt the host,
        and redeem its invitation on this device.
  tunnel USER@HOST [--timeout SECONDS] [--for SECONDS]
        Forward a local loopback port to the host until SECONDS pass, the
        tunnel ends, or you press Ctrl-C.
  remove USER@HOST [--timeout SECONDS]
        Stop the host if `add` started it, or detach if it was already
        running.
OS is linux or macos and ARCH is x86_64 or aarch64; each archive is pinned
to its SHA-256 when `add` runs. --relay defaults to OPENAGENTS_RELAY or
wss://relay.openagents.com. --timeout bounds the whole command (default 300
for add, 60 otherwise). --rights is the invitation's rights (default all).
Options: --ssh PROGRAM (default ssh on PATH), --store DIR (default
~/.openagents/coder-computers), --same-machine, --loopback-test.";

/// The schema of the file that records each destination `add` set up.
const RECORD_SCHEMA: &str = "openagents.cli.ssh-hosts.v1";
const RECORD_FILE: &str = "ssh-hosts.json";

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("ssh", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &["same-machine", "loopback-test"]) {
        Ok(args) => args,
        Err(message) => return output.usage("ssh", &message, USAGE),
    };
    let destination = match args.positional() {
        [destination] => destination.as_str(),
        [] => return output.usage("ssh", "USER@HOST is required", USAGE),
        [_, extra, ..] => {
            return output.usage("ssh", &format!("unexpected argument `{extra}`"), USAGE);
        }
    };
    let result = match command.as_str() {
        "add" => add(output, &args, destination),
        "tunnel" => tunnel(output, &args, destination),
        "remove" => remove(output, &args, destination),
        other => return output.usage("ssh", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok(code) => code,
        Err(Failure::Usage(message)) => output.usage("ssh", &message, USAGE),
        Err(Failure::Refused(message)) => output.fail("ssh", &message),
    }
}

#[derive(Debug)]
enum Failure {
    Usage(String),
    Refused(String),
}

impl From<coder_ssh::Error> for Failure {
    fn from(error: coder_ssh::Error) -> Self {
        Failure::Refused(error.to_string())
    }
}

/// One pinned archive, as `add` recorded it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Archive {
    os: String,
    arch: String,
    sha256: String,
    path: PathBuf,
}

/// What `add` set up for one destination. It holds no secret: the
/// invitation was redeemed and dropped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    owner: String,
    relay: String,
    rights: String,
    loopback_test: bool,
    archives: Vec<Archive>,
    computer: Option<String>,
    host: Value,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Records {
    schema: String,
    hosts: BTreeMap<String, Record>,
}

fn records_path(args: &Args) -> PathBuf {
    crate::computer::store_dir(args.option("store")).join(RECORD_FILE)
}

fn load_records(path: &Path) -> Result<Records, Failure> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let records: Records = serde_json::from_slice(&bytes)
                .map_err(|error| Failure::Refused(format!("{}: {error}", path.display())))?;
            if records.schema != RECORD_SCHEMA {
                return Err(Failure::Refused(format!(
                    "{}: unsupported schema `{}`",
                    path.display(),
                    records.schema
                )));
            }
            Ok(records)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Records {
            schema: RECORD_SCHEMA.to_owned(),
            hosts: BTreeMap::new(),
        }),
        Err(error) => Err(Failure::Refused(format!("{}: {error}", path.display()))),
    }
}

fn save_records(path: &Path, records: &Records) -> Result<(), Failure> {
    let failed = |error: std::io::Error| Failure::Refused(format!("{}: {error}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    let bytes =
        serde_json::to_vec_pretty(records).map_err(|error| Failure::Refused(error.to_string()))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(failed)?;
    std::fs::rename(&temporary, path).map_err(failed)
}

fn recorded(args: &Args, destination: &str) -> Result<Record, Failure> {
    let path = records_path(args);
    load_records(&path)?
        .hosts
        .remove(destination)
        .ok_or_else(|| {
            Failure::Refused(format!(
                "no SSH host recorded for {destination}; run `openagents ssh add {destination}` first"
            ))
        })
}

fn parse_os(text: &str) -> Option<Os> {
    match text {
        "linux" => Some(Os::Linux),
        "macos" => Some(Os::Macos),
        _ => None,
    }
}

fn parse_arch(text: &str) -> Option<Arch> {
    match text {
        "x86_64" => Some(Arch::X86_64),
        "aarch64" => Some(Arch::Aarch64),
        _ => None,
    }
}

/// Parse `OS/ARCH=PATH` and pin the archive to its SHA-256.
fn archive(spec: &str) -> Result<Archive, Failure> {
    let usage = || Failure::Usage(format!("--archive takes OS/ARCH=PATH, not `{spec}`"));
    let (platform, path) = spec.split_once('=').ok_or_else(usage)?;
    let (os, arch) = platform.split_once('/').ok_or_else(usage)?;
    if parse_os(os).is_none() {
        return Err(Failure::Usage(format!(
            "--archive OS is linux or macos, not `{os}`"
        )));
    }
    if parse_arch(arch).is_none() {
        return Err(Failure::Usage(format!(
            "--archive ARCH is x86_64 or aarch64, not `{arch}`"
        )));
    }
    if path.is_empty() {
        return Err(usage());
    }
    let path =
        std::path::absolute(path).map_err(|error| Failure::Refused(format!("{path}: {error}")))?;
    Ok(Archive {
        os: os.to_owned(),
        arch: arch.to_owned(),
        sha256: sha256_file(&path)?,
        path,
    })
}

fn sha256_file(path: &Path) -> Result<String, Failure> {
    let failed = |error: std::io::Error| Failure::Refused(format!("{}: {error}", path.display()));
    let mut file = std::fs::File::open(path).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer).map_err(failed)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn release(archives: &[Archive]) -> Result<Release, Failure> {
    let artifacts = archives
        .iter()
        .map(|archive| {
            Ok(Artifact {
                os: parse_os(&archive.os).ok_or_else(|| {
                    Failure::Refused(format!("unknown recorded OS `{}`", archive.os))
                })?,
                arch: parse_arch(&archive.arch).ok_or_else(|| {
                    Failure::Refused(format!("unknown recorded ARCH `{}`", archive.arch))
                })?,
                sha256: archive.sha256.clone(),
                archive: archive.path.clone(),
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    Ok(Release::new(artifacts)?)
}

/// The `coder` serve and invite arguments the remote account runs: the
/// same commands the Computers screens use.
fn runner_args(record: &Record) -> (Vec<String>, Vec<String>) {
    let words = |list: &[&str]| list.iter().map(|word| (*word).to_owned()).collect();
    let mut serve: Vec<String> = words(&[
        "host",
        "serve",
        "--loopback",
        "--owner",
        &record.owner,
        "--relay",
        &record.relay,
    ]);
    let mut invite: Vec<String> = words(&[
        "host",
        "invite",
        "--relay",
        &record.relay,
        "--rights",
        &record.rights,
    ]);
    if record.loopback_test {
        serve.push("--loopback-test".into());
        invite.push("--loopback-test".into());
    }
    (serve, invite)
}

fn runner(record: &Record) -> Result<coder_ssh::Runner, Failure> {
    let (serve, invite) = runner_args(record);
    Ok(coder_ssh::Runner::new(serve, invite)?)
}

fn launcher(args: &Args, destination: &str, record: &Record) -> Result<Launcher, Failure> {
    let launcher = Launcher::new(destination, release(&record.archives)?, runner(record)?)?;
    Ok(match args.option("ssh") {
        Some(program) => launcher.program(program),
        None => launcher,
    })
}

fn timeout(args: &Args, fallback: u64) -> Result<Duration, Failure> {
    let seconds: u64 = args.number("timeout", fallback).map_err(Failure::Usage)?;
    if seconds == 0 {
        return Err(Failure::Usage("--timeout is at least 1".into()));
    }
    Ok(Duration::from_secs(seconds))
}

/// Run `work` on its own thread and wait at most until `deadline`. On
/// time out the command ends; `ssh` stops at its own bound.
fn bounded<T: Send + 'static>(
    deadline: Instant,
    what: &str,
    work: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Result<T, Failure> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(Failure::Refused(format!(
            "{what} did not finish before --timeout; the remote step may still complete"
        ))),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(Failure::Refused(format!("{what} stopped without a result")))
        }
    }
}

fn host_json(host: &Host) -> Value {
    json!({
        "os": host.os.as_str(),
        "arch": host.arch.as_str(),
        "install": match host.install {
            Install::Fresh => "fresh",
            Install::Reused => "reused",
        },
        "version": host.version,
        "start": match host.start {
            Start::Started => "started",
            Start::Reused => "reused",
            Start::Relaunched => "relaunched",
            Start::Adopted => "adopted",
        },
        "ownership": match host.ownership {
            Ownership::Managed => "managed",
            Ownership::External => "external",
        },
        "pid": host.pid,
        "port": host.port,
        "reclaimed_lock": host.reclaimed_lock,
    })
}

fn removal_json(removal: Removal) -> Value {
    match removal {
        Removal::Stopped { pid } => json!({ "result": "stopped", "pid": pid }),
        Removal::Detached { pid } => json!({ "result": "detached", "pid": pid }),
        Removal::Absent => json!({ "result": "absent", "pid": null }),
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn host_line(host: &Value) -> String {
    format!(
        "{} host (pid {}, {}) on {}/{}, port {}, version {}",
        text(&host["ownership"]),
        text(&host["pid"]),
        text(&host["start"]),
        text(&host["os"]),
        text(&host["arch"]),
        text(&host["port"]),
        text(&host["version"]),
    )
}

fn add(output: &Output, args: &Args, destination: &str) -> Result<u8, Failure> {
    let owner = args
        .option("owner")
        .ok_or_else(|| Failure::Usage("add needs --owner with the owner's public key".into()))?;
    if owner.len() != 64
        || !owner
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(Failure::Usage(
            "--owner is a 64-character lowercase hex public key".into(),
        ));
    }
    let specs = args.options("archive");
    if specs.is_empty() {
        return Err(Failure::Usage(
            "add needs at least one --archive OS/ARCH=PATH".into(),
        ));
    }
    let rights = args.option("rights").unwrap_or("all");
    coder_access::Rights::parse_list(rights)
        .map_err(|error| Failure::Usage(format!("--rights: {error}")))?;
    let deadline = Instant::now() + timeout(args, 300)?;
    let mut record = Record {
        owner: owner.to_owned(),
        relay: crate::relay::relay_url(args.option("relay")),
        rights: rights.to_owned(),
        loopback_test: args.switch("loopback-test"),
        archives: specs
            .into_iter()
            .map(archive)
            .collect::<Result<Vec<_>, _>>()?,
        computer: None,
        host: Value::Null,
    };
    let launcher = launcher(args, destination, &record)?;
    let (resolved, host, invitation) = bounded(deadline, "ssh setup", move || {
        let resolved = launcher.resolve()?;
        let host = launcher.up()?;
        let invitation = launcher.invite(&host)?;
        Ok((resolved, host, invitation))
    })?;
    record.host = host_json(&host);
    let path = records_path(args);
    let mut records = load_records(&path)?;
    records.hosts.insert(destination.to_owned(), record.clone());
    save_records(&path, &records)?;

    let runtime = crate::runtime();
    let redeemed = crate::computer::open(args, &runtime).and_then(|mut live| {
        let computer = live
            .redeem_invitation(invitation.expose())
            .map_err(|error| error.to_string())?;
        drop(invitation);
        let linked = wait_for_link(&mut live, &computer, deadline);
        Ok((computer, linked))
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
    let (computer, linked) = redeemed.map_err(|message| {
        Failure::Refused(format!(
            "the host runs on {destination}, but redeeming its invitation failed: {message}; `openagents ssh remove {destination}` stops it"
        ))
    })?;
    record.computer = Some(computer.clone());
    records.hosts.insert(destination.to_owned(), record.clone());
    save_records(&path, &records)?;
    let value = json!({
        "destination": destination,
        "resolved": {
            "hostname": resolved.hostname,
            "user": resolved.user,
            "port": resolved.port,
        },
        "host": record.host,
        "computer": computer,
        "link": linked,
        "relay": record.relay,
        "archives": record.archives,
        "record": path,
    });
    output.emit(&value, |value| {
        format!(
            "{}: {}\ncomputer {} (link {})",
            text(&value["destination"]),
            host_line(&value["host"]),
            text(&value["computer"]),
            text(&value["link"]),
        )
    });
    Ok(0)
}

/// Wait until the new computer's link leaves `Connecting` or `deadline`
/// passes, and report its phase.
fn wait_for_link(
    live: &mut coder_computers::live::Live,
    computer: &str,
    deadline: Instant,
) -> Option<String> {
    loop {
        let phase = live.snapshot().ok().and_then(|snapshot| {
            snapshot
                .host(computer)
                .and_then(|host| host.link.as_ref())
                .map(|link| link.phase)
        });
        let connecting = matches!(phase, Some(coder_link::Phase::Connecting(_)));
        if !connecting || Instant::now() >= deadline {
            return phase.map(|phase| format!("{phase:?}"));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn tunnel(output: &Output, args: &Args, destination: &str) -> Result<u8, Failure> {
    let record = recorded(args, destination)?;
    let wait = timeout(args, 60)?;
    let hold = match args.option("for") {
        Some(_) => Some(Duration::from_secs(
            args.number("for", 0).map_err(Failure::Usage)?,
        )),
        None => None,
    };
    let deadline = Instant::now() + wait;
    let launcher = launcher(args, destination, &record)?;
    let (host, mut tunnel) = bounded(deadline, "ssh tunnel", move || {
        let host = launcher.up()?;
        let tunnel = launcher.connect(&host)?;
        Ok((host, tunnel))
    })?;
    tunnel.ready(deadline.saturating_duration_since(Instant::now()))?;
    // SAFETY: the handler only stores to an atomic, which is
    // async-signal-safe.
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
    let opened = json!({
        "event": "open",
        "destination": destination,
        "local": format!("127.0.0.1:{}", tunnel.local_port()),
        "local_port": tunnel.local_port(),
        "remote_port": tunnel.remote_port(),
        "pid": tunnel.pid(),
        "host": host_json(&host),
        "computer": record.computer,
    });
    output.line(&opened, |value| {
        format!(
            "{} forwards to {} port {}; press Ctrl-C to close",
            text(&value["local"]),
            text(&value["destination"]),
            text(&value["remote_port"]),
        )
    });
    let until = hold.map(|hold| Instant::now() + hold);
    let reason = loop {
        if STOP.load(Ordering::SeqCst) {
            break "interrupted";
        }
        if until.is_some_and(|until| Instant::now() >= until) {
            break "elapsed";
        }
        if !tunnel.alive() {
            break "ended";
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    tunnel.close();
    output.line(
        &json!({ "event": "closed", "destination": destination, "reason": reason }),
        |value| {
            format!(
                "tunnel to {} closed: {}",
                text(&value["destination"]),
                text(&value["reason"])
            )
        },
    );
    Ok(if reason == "ended" {
        crate::EXIT_FAILURE
    } else {
        0
    })
}

fn remove(output: &Output, args: &Args, destination: &str) -> Result<u8, Failure> {
    let record = recorded(args, destination)?;
    let deadline = Instant::now() + timeout(args, 60)?;
    let launcher = launcher(args, destination, &record)?;
    let removal = bounded(deadline, "ssh remove", move || Ok(launcher.remove()?))?;
    let path = records_path(args);
    let mut records = load_records(&path)?;
    records.hosts.remove(destination);
    save_records(&path, &records)?;
    let mut value = removal_json(removal);
    value["destination"] = json!(destination);
    value["computer"] = json!(record.computer);
    output.emit(&value, |value| {
        let what = match value["result"].as_str() {
            Some("stopped") => format!("stopped the host (pid {})", text(&value["pid"])),
            Some("detached") => format!(
                "left the host (pid {}) running and detached",
                text(&value["pid"])
            ),
            _ => "no host was running".to_owned(),
        };
        let forget = match value["computer"].as_str() {
            Some(computer) => {
                format!("\n`openagents computer forget {computer}` drops it from this device")
            }
            None => String::new(),
        };
        format!("{}: {what}{forget}", text(&value["destination"]))
    });
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("openagents-ssh-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch dir");
        directory
    }

    fn record() -> Record {
        Record {
            owner: "a".repeat(64),
            relay: "wss://relay.example/".into(),
            rights: "all".into(),
            loopback_test: false,
            archives: vec![Archive {
                os: "linux".into(),
                arch: "x86_64".into(),
                sha256: "b".repeat(64),
                path: "/tmp/coder.tar.gz".into(),
            }],
            computer: None,
            host: Value::Null,
        }
    }

    #[test]
    fn archive_specs_are_checked_and_pinned() {
        let directory = scratch("archive");
        let path = directory.join("coder.tar.gz");
        std::fs::write(&path, b"archive").expect("write");
        let pinned = archive(&format!("linux/aarch64={}", path.display())).expect("archive");
        assert_eq!(pinned.os, "linux");
        assert_eq!(pinned.arch, "aarch64");
        assert_eq!(
            pinned.sha256,
            "0eb3e36bfb24dcd9bb1d1bece1531216b59539a8fde17ee80224af0653c92aa3"
        );
        for bad in [
            "linux",
            "linux/x86_64",
            "bsd/x86_64=/x",
            "linux/arm=/x",
            "linux/x86_64=",
        ] {
            assert!(matches!(archive(bad), Err(Failure::Usage(_))), "{bad}");
        }
        assert!(matches!(
            archive("linux/x86_64=/no/such/archive"),
            Err(Failure::Refused(_))
        ));
    }

    #[test]
    fn runner_matches_the_computers_screens() {
        let mut record = record();
        runner(&record).expect("runner");
        let (serve, invite) = runner_args(&record);
        assert_eq!(
            serve,
            [
                "host",
                "serve",
                "--loopback",
                "--owner",
                &"a".repeat(64),
                "--relay",
                "wss://relay.example/"
            ]
        );
        assert_eq!(
            invite,
            [
                "host",
                "invite",
                "--relay",
                "wss://relay.example/",
                "--rights",
                "all"
            ]
        );
        record.loopback_test = true;
        let (serve, invite) = runner_args(&record);
        assert_eq!(serve.last().map(String::as_str), Some("--loopback-test"));
        assert_eq!(invite.last().map(String::as_str), Some("--loopback-test"));
    }

    #[test]
    fn records_round_trip_and_refuse_other_schemas() {
        let directory = scratch("records");
        let path = directory.join(RECORD_FILE);
        assert!(load_records(&path).expect("empty").hosts.is_empty());
        let mut records = load_records(&path).expect("empty");
        records.hosts.insert("me@box".into(), record());
        save_records(&path, &records).expect("save");
        assert_eq!(load_records(&path).expect("load"), records);
        std::fs::write(&path, br#"{"schema":"other","hosts":{}}"#).expect("write");
        assert!(matches!(load_records(&path), Err(Failure::Refused(_))));
    }

    #[test]
    fn removal_reports_every_outcome() {
        assert_eq!(
            removal_json(Removal::Stopped { pid: 7 })["result"],
            "stopped"
        );
        assert_eq!(removal_json(Removal::Detached { pid: 7 })["pid"], 7);
        assert_eq!(removal_json(Removal::Absent)["pid"], Value::Null);
    }
}
