//! The owner's computers, from this device: the same client the Computers
//! screens use (`coder_computers::live::Live`), with its device key and
//! grants under `~/.openagents/coder-computers/`. Every call is an effect
//! request the host authorizes against its own grant records; this command
//! only carries it.

use std::path::PathBuf;
use std::time::Duration;

use coder_access::Rights;
use coder_access::protocol::TaskCreate;
use coder_computers::live::{FileStore, Live, Settings, load_or_create_key};
use coder_computers::model::{DeviceList, Enrollment, LocalHost, ServiceState, Snapshot};
use coder_computers::{ComputersService, Platform};
use coder_reach::hints::Locality;
use serde_json::{Value, json};

use crate::{Args, Output, out};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents computer COMMAND [OPTIONS]
  list [--wait SECONDS]     Every host this device knows, with its link and grant.
  show HOST                 One host: grant, devices, pending enrollments, workspaces.
  link CODE                 Pair with a computer: its `openagents connect invite --text`
                            code, or a coder-host: invitation (QR text or paste).
  link --ssh USER@HOST      The desktop app's ssh pairing; on the command line use
                            `openagents connect --ssh USER@HOST`.
  approve HOST ENROLLMENT --code CODE [--rights standard|admin|all|LIST] [--days N]
                            Approve a headless host's enrollment request.
  deny HOST ENROLLMENT
  invite HOST [--rights standard|admin|all|LIST] [--days N]
                            Create a single-use invitation for another device.
  devices HOST              Fetch and print the host's device list.
  revoke HOST DEVICE
  forget HOST               Drop the host from this device (its grant stays).
  enable HOST | disable HOST | retry HOST
  workspaces HOST           Fetch the workspace labels the host accepts.
  task HOST --workspace LABEL --title TITLE [--engine ENGINE] PROMPT...
                            Order work on the host; prints the task id.
                            ENGINE is one of codex, claude_code, grok_build,
                            opencode, devin — a preference the host's
                            auto-start policy puts first when it admits it.
  steer HOST TASK --revision N PROMPT...
  cancel HOST TASK --revision N [--reason TEXT]
  exec HOST [--timeout S] [--rows N --cols N] -- CMD [ARGS...]
                            Run a command in a shell on the host (NIP-TERM) and
                            return its output and exit code.
  shell HOST                An interactive shell on the host. Ctrl-] detaches.
  watch HOST [--every S] [--for S] [--until TEXT | --until-exit] -- CMD [ARGS...]
                            Rerun a command until its output contains TEXT (or it
                            exits 0), printing every result; --json is NDJSON.
  tail HOST PATH [--lines N] [--follow]
                            The end of a file on the host; --json is one line each.
  alias NAME HOST           Name a host; every HOST above accepts a name, the label
                            `list` shows, a key, or a unique prefix of a key or label.
                            `alias --list`, `alias --remove NAME`.
  screenshot HOST [--screen NAME | --android [--serial S]] [--out FILE]
                            A PNG of the computer's screen (or an attached Android
                            device's), saved here; prints its path.
  apps HOST                 The windows open on the computer's screen.
  push HOST LOCAL REMOTE [--overwrite]
                            Copy a file to the computer (at most 256 MiB); REMOTE
                            is absolute or starts with ~/, and ending in / keeps
                            the name. It lands only whole, with its SHA-256
                            checked, and never replaces a file without --overwrite.
  pull HOST REMOTE LOCAL [--overwrite] [--max-bytes N]
                            Copy a file from the computer, checked the same way.
  journal [HOST] [--lines N]
                            What exec, watch, and tail ran, from this device's log.
  client-only               Record that this machine runs no local host.
Options: --store DIR (default ~/.openagents/coder-computers), --wait SECONDS
(how long to wait for the host's link; default 15), --same-machine
(hosts run on this computer; allows loopback routes), --loopback-test.";

const LINK_USAGE: &str = "usage: openagents computer link CODE [--wait SECONDS]
       openagents computer link --ssh USER@HOST [--wait SECONDS]
Pair this device with a computer. CODE is that computer's `openagents connect
invite --text` code, or a coder-host: invitation (QR text or paste). --ssh
asks the running host's ssh launcher (the desktop app's) to install or adopt a
host there; from the command line, `openagents connect --ssh USER@HOST` does
the whole setup in one step. --wait is how long to wait for the link (default
15, 120 with --ssh).";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::device("list", Effect::ReadOnly),
    Declared::device("show", Effect::ReadOnly),
    Declared::screen("link", Effect::Grants, "account.computers"),
    Declared::screen("approve", Effect::Grants, "account.computers"),
    Declared::screen("deny", Effect::Grants, "account.computers"),
    Declared::screen("invite", Effect::Grants, "account.computers"),
    Declared::device("devices", Effect::ReadOnly),
    Declared::screen("revoke", Effect::Grants, "account.computers"),
    Declared::screen("forget", Effect::LocalWrite, "account.computers"),
    Declared::screen("enable", Effect::LocalWrite, "account.computers"),
    Declared::screen("disable", Effect::LocalWrite, "account.computers"),
    Declared::screen("retry", Effect::LocalWrite, "account.computers"),
    Declared::device("workspaces", Effect::ReadOnly),
    Declared::device("task", Effect::Publishes),
    Declared::device("steer", Effect::Publishes),
    Declared::device("cancel", Effect::Publishes),
    Declared::device("exec", Effect::Publishes),
    Declared::device("shell", Effect::LongRunning),
    Declared::device("watch", Effect::LongRunning),
    Declared::device("tail", Effect::ReadOnly),
    Declared::device("screenshot", Effect::LocalWrite),
    Declared::device("apps", Effect::ReadOnly),
    Declared::device("push", Effect::Publishes),
    Declared::device("pull", Effect::LocalWrite),
    Declared::device("alias", Effect::LocalWrite),
    Declared::device("journal", Effect::ReadOnly),
    Declared::device("client-only", Effect::LocalWrite),
];

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub fn store_dir(flag: Option<&str>) -> PathBuf {
    flag.map(PathBuf::from).unwrap_or_else(|| {
        verse::identity::home()
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("coder-computers")
    })
}

pub(crate) fn open(args: &Args, runtime: &tokio::runtime::Runtime) -> Result<Live, String> {
    let directory = store_dir(args.option("store"));
    std::fs::create_dir_all(&directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    let mut settings = Settings::new(Platform::Terminal);
    settings.now = now;
    if args.switch("loopback-test") {
        settings.policy = coder_access::RelayPolicy::LoopbackTest;
    }
    if args.switch("same-machine") {
        settings.locality = Locality::SameMachine;
    }
    // iroh first, as the apps do: a computer paired by connect code is
    // then reached directly, not only through the Nostr relay, which
    // matters for screenshots and file copies.
    settings.iroh_secret = Some(coder_computers::live::load_or_create_iroh_key(&directory)?);
    let secret = load_or_create_key(&directory)?;
    let store = FileStore::open(&directory)?;
    Live::open(settings, secret, Box::new(store), runtime.handle().clone())
        .map_err(|error| error.to_string())
}

fn rights_from(args: &Args) -> Result<(Rights, u64), String> {
    let rights = Rights::parse_list(args.option("rights").unwrap_or("standard"))
        .map_err(|e| e.to_string())?;
    let days: u64 = args.number("days", 30)?;
    Ok((rights, now() + days.clamp(1, 3650) * 86_400))
}

fn rights_text(rights: &Rights) -> String {
    rights
        .iter()
        .map(|right| right.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn enrollment_json(enrollment: &Enrollment) -> Value {
    match enrollment {
        Enrollment::Enrolled {
            grant,
            rights,
            epoch,
            expires_at,
        } => json!({
            "state": "enrolled", "grant": grant, "rights": rights_text(rights),
            "epoch": epoch, "expires_at": expires_at,
        }),
        Enrollment::NotEnrolled => json!({ "state": "not-enrolled" }),
        Enrollment::Expired { at } => json!({ "state": "expired", "at": at }),
        other => json!({ "state": format!("{other:?}").to_lowercase() }),
    }
}

fn host_json(
    snapshot: &Snapshot,
    host: &coder_computers::model::HostRecord,
    store: &std::path::Path,
) -> Value {
    json!({
        "key": host.key,
        "alias": crate::hosts::alias_of(store, &host.key),
        "label": host.label,
        "listed": host.listing.is_some(),
        "delisted": host.delisted,
        "weight": host.weight(),
        "ssh": host.ssh,
        "tunnel": host.tunnel.as_ref().map(|t| json!({ "open": t.open, "in_use": t.in_use })),
        "enrollment": enrollment_json(&host.enrollment),
        "rights_now": host.enrollment.rights(snapshot.now).map(rights_text),
        "link": host.link.as_ref().map(|link| json!({
            "phase": phase_words(&link.phase),
            "freshness": format!("{:?}", link.freshness).to_lowercase(),
            "enabled": link.enabled,
            "last_failure": link.last_failure.as_ref().map(|f| format!("{f:?}")),
        })),
        "route": host.route.map(|class| format!("{class:?}").to_lowercase()),
        "compatibility": format!("{:?}", host.compatibility),
        "online": host.presence.is_some(),
        "pending_enrollments": host.enrollments.iter().map(|e| json!({
            "enrollment": e.enrollment, "rights": rights_text(&e.rights), "expires_at": e.expires_at,
        })).collect::<Vec<_>>(),
        "devices": match &host.devices {
            DeviceList::NotLoaded => Value::Null,
            DeviceList::Loaded { devices, as_of } => json!({
                "as_of": as_of,
                "rows": devices.iter().map(|d| json!({
                    "device": d.device, "label": d.label, "rights": rights_text(&d.rights),
                    "state": format!("{:?}", d.state).to_lowercase(),
                    "origin": format!("{:?}", d.origin).to_lowercase(),
                    "expires_at": d.expires_at,
                })).collect::<Vec<_>>(),
            }),
        },
        "workspaces": host.workspaces,
    })
}

fn snapshot_json(snapshot: &Snapshot, store: &std::path::Path) -> Value {
    json!({
        "now": snapshot.now,
        "device": snapshot.device,
        "owner": snapshot.owner,
        "service": match &snapshot.service {
            ServiceState::Ready => json!("ready"),
            ServiceState::Unavailable { reason } => json!({ "unavailable": reason }),
        },
        "local_host": match &snapshot.local_host {
            LocalHost::Running { host } => json!({ "running": host }),
            other => json!(format!("{other:?}").to_lowercase()),
        },
        "ssh_ready": snapshot.ssh_ready,
        "hosts": snapshot.hosts.iter().map(|h| host_json(snapshot, h, store)).collect::<Vec<_>>(),
    })
}

fn render_hosts(value: &Value) -> String {
    let mut rows = vec![vec![
        "alias".to_owned(),
        "label".to_owned(),
        "link".to_owned(),
        "rights".to_owned(),
        "route".to_owned(),
        "host".to_owned(),
    ]];
    for host in value["hosts"].as_array().into_iter().flatten() {
        rows.push(vec![
            host["alias"].as_str().unwrap_or("-").to_owned(),
            host["label"].as_str().unwrap_or("").to_owned(),
            host["link"]["phase"]
                .as_str()
                .unwrap_or(if host["online"].as_bool().unwrap_or(false) {
                    "online"
                } else {
                    "-"
                })
                .to_owned(),
            host["rights_now"].as_str().unwrap_or("none").to_owned(),
            host["route"].as_str().unwrap_or("-").to_owned(),
            host["key"].as_str().unwrap_or("").to_owned(),
        ]);
    }
    format!(
        "device {}{}\n{}",
        value["device"].as_str().unwrap_or(""),
        if value["owner"].as_bool().unwrap_or(false) {
            " (owner)"
        } else {
            ""
        },
        if rows.len() == 1 {
            "no hosts; redeem an invitation with `openagents computer link`".to_owned()
        } else {
            out::table(&rows)
        }
    )
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("computer", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    if matches!(command.as_str(), "link" | "redeem")
        && rest
            .iter()
            .any(|word| matches!(word.as_str(), "--help" | "-h"))
    {
        println!("{LINK_USAGE}");
        return 0;
    }
    let args = match Args::parse(
        rest,
        &[
            "same-machine",
            "loopback-test",
            "until-exit",
            "follow",
            "list",
            "android",
            "overwrite",
        ],
    ) {
        Ok(args) => args,
        Err(message) => return output.usage("computer", &message, USAGE),
    };
    let runtime = crate::runtime();
    let mut live = match open(&args, &runtime) {
        Ok(live) => live,
        Err(message) => return output.fail("computer", &message),
    };
    let result = dispatch(output, &mut live, runtime.handle(), command, &args);
    drop(live);
    runtime.shutdown_timeout(Duration::from_secs(2));
    match result {
        Ok(code) => code,
        Err(message) => output.fail("computer", &message),
    }
}

/// Run `words` on the host `text` names and return the run as the JSON
/// `computer exec --json` prints, journaled the same way. For commands
/// built on top of `exec`.
pub fn exec_json(args: &Args, text: &str, words: &[String]) -> Result<Value, String> {
    let runtime = crate::runtime();
    let mut live = open(args, &runtime)?;
    let result = (|| {
        let host = host_arg_text(&mut live, args, text)?;
        connected(&mut live, &host, args)?;
        let run = crate::terminal::run(&live, runtime.handle(), &host, words, args, &mut |_| {})?;
        crate::hosts::journal(args, "exec", &host, words, &run);
        Ok(run.json(&host, words))
    })();
    drop(live);
    runtime.shutdown_timeout(Duration::from_secs(2));
    result
}

fn positional<'a>(args: &'a Args, index: usize, name: &str) -> Result<&'a str, String> {
    args.positional()
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} is required"))
}

/// The host key the positional at `index` names: an alias, a key, or a
/// unique prefix of a host this device knows.
fn host_arg(live: &mut Live, args: &Args, index: usize) -> Result<String, String> {
    host_arg_text(live, args, positional(args, index, "HOST")?)
}

pub(crate) fn host_arg_text(live: &mut Live, args: &Args, text: &str) -> Result<String, String> {
    let known: Vec<crate::hosts::Known> = live
        .snapshot()
        .map_err(|e| e.to_string())?
        .hosts
        .iter()
        .map(|h| crate::hosts::Known {
            key: h.key.clone(),
            label: h.label.clone(),
        })
        .collect();
    crate::hosts::resolve(&store_dir(args.option("store")), &known, text)
}

fn settle(live: &mut Live, seconds: u64) -> Result<Snapshot, String> {
    // Supervisors connect in the background; give them a moment to report.
    let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let snapshot = live.snapshot().map_err(|e| e.to_string())?;
        let pending = snapshot.hosts.iter().any(|host| {
            host.link
                .as_ref()
                .is_some_and(|link| matches!(link.phase, coder_link::Phase::Connecting(_)))
        });
        if !pending || std::time::Instant::now() >= deadline {
            return Ok(snapshot);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Wait until the supervisor reports the host's link connected, so a call
/// over it is not refused while the first attempt is still in flight.
pub(crate) fn connected(live: &mut Live, host: &str, args: &Args) -> Result<(), String> {
    let seconds: u64 = args.number("wait", 15)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let snapshot = live.snapshot().map_err(|e| e.to_string())?;
        let record = snapshot
            .host(host)
            .ok_or_else(|| format!("this device knows no host {host}"))?;
        let name = host_name(record, host);
        match record.link.as_ref().map(|link| &link.phase) {
            Some(coder_link::Phase::Connected) => return Ok(()),
            Some(coder_link::Phase::Blocked(reason)) => {
                return Err(format!(
                    "{name} can't be reached: {}",
                    blocked_words(*reason)
                ));
            }
            _ => {}
        }
        if std::time::Instant::now() >= deadline {
            return Err(not_connected(record, &name, seconds, "--wait"));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The computer's label, or its key when it has none.
pub(crate) fn host_name(record: &coder_computers::model::HostRecord, key: &str) -> String {
    if record.label.is_empty() {
        key.to_owned()
    } else {
        record.label.clone()
    }
}

/// A link's phase in plain words.
pub(crate) fn phase_words(phase: &coder_link::Phase) -> String {
    use coder_link::Phase;
    match phase {
        Phase::Available => "not connecting".into(),
        Phase::Offline => "this computer has no network".into(),
        Phase::Connecting(_) => "connecting".into(),
        Phase::Backoff { .. } => "retrying".into(),
        Phase::Connected => "connected".into(),
        Phase::Blocked(reason) => blocked_words(*reason).into(),
    }
}

/// Why a blocked link can't connect, as a person reads it.
pub(crate) fn blocked_words(reason: coder_link::BlockReason) -> &'static str {
    use coder_link::BlockReason;
    match reason {
        BlockReason::Authentication => "it doesn't recognize this computer",
        BlockReason::Revoked => "it removed this computer's access",
        BlockReason::Incompatible => "it runs a version this one can't talk to; update both",
        BlockReason::Configuration => "this computer's settings for it are invalid",
    }
}

/// Why a host did not connect in time, and the next step. A host seals its
/// presence to each device it admits, so a host that removed this computer
/// looks exactly like one that is off: it publishes nothing here. Say both,
/// and how to check and enroll again (#10368).
pub(crate) fn not_connected(
    record: &coder_computers::model::HostRecord,
    name: &str,
    seconds: u64,
    flag: &str,
) -> String {
    let state = record.link.as_ref().map_or_else(
        || "not connecting".to_owned(),
        |link| phase_words(&link.phase),
    );
    if record.presence.is_some() {
        return format!(
            "{name} did not connect within {seconds}s ({state}); pass {flag} SECONDS to wait longer"
        );
    }
    format!(
        "{name} did not connect within {seconds}s ({state}). It has published nothing to this \
         computer: it is off or offline, or it removed this computer. On {name}, `openagents \
         connect devices` lists the devices it admits; if this one is gone, run `openagents \
         connect invite --text` there and `openagents computer link CODE` here. Pass {flag} \
         SECONDS to wait longer."
    )
}

fn dispatch(
    output: &Output,
    live: &mut Live,
    runtime: &tokio::runtime::Handle,
    command: &str,
    args: &Args,
) -> Result<u8, String> {
    let ok = |output: &Output, value: Value| {
        output.emit(&value, |v| {
            v.get("message")
                .and_then(Value::as_str)
                .map_or_else(|| "ok".to_owned(), str::to_owned)
        });
        Ok(0)
    };
    let store = store_dir(args.option("store"));
    match command {
        "list" | "ls" => {
            let snapshot = settle(live, args.number("wait", 3)?)?;
            output.emit(&snapshot_json(&snapshot, &store), render_hosts);
            Ok(0)
        }
        "show" => {
            let key = host_arg(live, args, 0)?;
            let snapshot = settle(live, args.number("wait", 3)?)?;
            let host = snapshot
                .host(&key)
                .ok_or_else(|| format!("this device knows no host {key}"))?;
            output.emit(&host_json(&snapshot, host, &store), |v| {
                serde_json::to_string_pretty(v).unwrap_or_default()
            });
            Ok(0)
        }
        "link" | "redeem" => {
            if let Some(destination) = args.option("ssh") {
                live.connect_ssh(destination).map_err(|e| e.to_string())?;
                let deadline =
                    std::time::Instant::now() + Duration::from_secs(args.number("wait", 120)?);
                loop {
                    let snapshot = live.snapshot().map_err(|e| e.to_string())?;
                    let Some(attempt) = &snapshot.ssh else {
                        break;
                    };
                    let stage = format!("{:?}", attempt.stage);
                    if stage.starts_with("Prompt") {
                        return Err(format!(
                            "ssh asks for input ({stage}); set up keys for {destination} and try again"
                        ));
                    }
                    if !stage.starts_with("Starting") {
                        output.emit(
                            &json!({ "destination": destination, "stage": stage, "hosts": snapshot_json(&snapshot, &store)["hosts"] }),
                            |v| format!("ssh {}: {}", v["destination"].as_str().unwrap_or(""), v["stage"]),
                        );
                        return Ok(if stage.starts_with("Failed") {
                            crate::EXIT_FAILURE
                        } else {
                            0
                        });
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err("ssh setup did not finish in time; `openagents computer list` shows its progress".into());
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                return ok(output, json!({ "message": "ssh setup finished" }));
            }
            let invitation = match positional(args, 0, "INVITATION") {
                Ok(text) => text.to_owned(),
                Err(_) => {
                    let mut text = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin().lock(), &mut text)
                        .map_err(|e| e.to_string())?;
                    text.trim().to_owned()
                }
            };
            if invitation.is_empty() {
                return Err("INVITATION is required (argument or stdin)".into());
            }
            // A connect code (`openagents connect invite --text`, the code a
            // phone pairs with) pairs as the phone does; a `coder-host:`
            // invitation is redeemed on the relay it names.
            let host = match coder_computers::connect::classify(&invitation)? {
                coder_computers::connect::Scanned::Connect(code) => {
                    runtime
                        .block_on(live.pairing().pair(&code))
                        .map_err(|failure| failure.message)?
                        .host
                }
                coder_computers::connect::Scanned::HostInvitation(text) => {
                    live.redeem_invitation(&text).map_err(|e| e.to_string())?
                }
            };
            let snapshot = settle(live, 5)?;
            let record = snapshot
                .host(&host)
                .map(|h| host_json(&snapshot, h, &store));
            output.emit(&json!({ "host": host, "record": record }), |v| {
                format!("linked host {}", v["host"].as_str().unwrap_or(""))
            });
            Ok(0)
        }
        "approve" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            let enrollment = positional(args, 1, "ENROLLMENT")?;
            let code = args.option("code").ok_or("--code CODE is required")?;
            let (rights, expires) = rights_from(args)?;
            connected(live, host, args)?;
            live.approve_enrollment(host, enrollment, code, &rights, expires)
                .map_err(|e| e.to_string())?;
            ok(
                output,
                json!({ "host": host, "enrollment": enrollment, "rights": rights_text(&rights), "expires_at": expires }),
            )
        }
        "deny" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            connected(live, host, args)?;
            live.deny_enrollment(host, positional(args, 1, "ENROLLMENT")?)
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "invite" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            let (rights, expires) = rights_from(args)?;
            connected(live, host, args)?;
            let created = live
                .create_invitation(host, &rights, expires)
                .map_err(|e| e.to_string())?;
            // The code is a single-use capability: shown once, on purpose.
            output.emit(
                &json!({ "host": host, "invitation": created.invitation, "code": created.code,
                         "rights": rights_text(&created.rights), "expires_at": created.expires_at }),
                |v| format!("{}\n(single use; expires at {})", v["code"].as_str().unwrap_or(""), v["expires_at"]),
            );
            Ok(0)
        }
        "devices" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            connected(live, host, args)?;
            live.refresh_devices(host).map_err(|e| e.to_string())?;
            let snapshot = live.snapshot().map_err(|e| e.to_string())?;
            let record = snapshot.host(host).ok_or("unknown host")?;
            let value = host_json(&snapshot, record, &store);
            output.emit(&value["devices"], |v| {
                let mut rows = vec![vec![
                    "device".to_owned(),
                    "rights".to_owned(),
                    "state".to_owned(),
                    "origin".to_owned(),
                ]];
                for row in v["rows"].as_array().into_iter().flatten() {
                    rows.push(vec![
                        row["device"].as_str().unwrap_or("").to_owned(),
                        row["rights"].as_str().unwrap_or("").to_owned(),
                        row["state"].as_str().unwrap_or("").to_owned(),
                        row["origin"].as_str().unwrap_or("").to_owned(),
                    ]);
                }
                out::table(&rows)
            });
            Ok(0)
        }
        "revoke" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            connected(live, host, args)?;
            live.revoke(host, positional(args, 1, "DEVICE")?)
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "forget" => {
            let host = host_arg(live, args, 0)?;
            live.forget(&host).map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "enable" | "disable" => {
            let host = host_arg(live, args, 0)?;
            live.set_enabled(&host, command == "enable")
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "retry" => {
            let host = host_arg(live, args, 0)?;
            live.retry_now(&host).map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "workspaces" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            connected(live, host, args)?;
            live.refresh_workspaces(host).map_err(|e| e.to_string())?;
            let snapshot = live.snapshot().map_err(|e| e.to_string())?;
            let record = snapshot.host(host).ok_or("unknown host")?;
            output.emit(
                &json!({ "host": host, "workspaces": record.workspaces }),
                |v| {
                    v["workspaces"]
                        .as_array()
                        .map(|w| {
                            w.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_else(|| "the host lists no workspaces".to_owned())
                },
            );
            Ok(0)
        }
        "task" | "order" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            let prompt = args.positional()[1..].join(" ");
            if prompt.trim().is_empty() {
                return Err("PROMPT is required".into());
            }
            let engine = match args.option("engine") {
                Some(word) => {
                    Some(nostr::cj_conversation::Engine::parse(word).ok_or_else(|| {
                        format!(
                            "--engine wants one of {}",
                            nostr::cj_conversation::Engine::ALL
                                .iter()
                                .map(|engine| engine.word())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?)
                }
                None => None,
            };
            let task = TaskCreate {
                title: args
                    .option("title")
                    .map_or_else(|| prompt.chars().take(60).collect(), str::to_owned),
                prompt,
                workspace: args
                    .option("workspace")
                    .ok_or("--workspace LABEL is required")?
                    .to_owned(),
                images: Vec::new(),
                engine,
            };
            connected(live, host, args)?;
            let id = live.create_task(host, &task).map_err(|e| e.to_string())?;
            output.emit(&json!({ "host": host, "task": id }), |v| {
                v["task"].as_str().unwrap_or("").to_owned()
            });
            Ok(0)
        }
        "steer" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            let task = positional(args, 1, "TASK")?;
            let revision: u64 = args.number("revision", 0)?;
            let prompt = args.positional()[2..].join(" ");
            connected(live, host, args)?;
            live.steer_task(host, task, revision, &prompt)
                .map_err(|e| e.to_string())?;
            ok(output, json!({ "task": task }))
        }
        "cancel" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            let task = positional(args, 1, "TASK")?;
            let revision: u64 = args.number("revision", 0)?;
            connected(live, host, args)?;
            live.cancel_task(
                host,
                task,
                revision,
                args.option("reason")
                    .unwrap_or("cancelled from the command line"),
            )
            .map_err(|e| e.to_string())?;
            ok(output, json!({ "task": task }))
        }
        "exec" | "run" | "watch" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            if args.positional().len() < 2 {
                return Ok(output.usage(
                    "computer",
                    "a command is required after HOST (put `--` before it)",
                    USAGE,
                ));
            }
            connected(live, host, args)?;
            let words = &args.positional()[1..];
            if command == "watch" {
                crate::terminal::watch(*output, live, runtime, host, words, args)
            } else {
                crate::terminal::exec(*output, live, runtime, host, words, args)
            }
        }
        "tail" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            let path = positional(args, 1, "PATH")?;
            connected(live, host, args)?;
            crate::terminal::tail(*output, live, runtime, host, path, args)
        }
        "shell" | "sh" => {
            let host = host_arg(live, args, 0)?;
            let host = host.as_str();
            connected(live, host, args)?;
            crate::terminal::shell(*output, live, runtime, host, args)
        }
        "screenshot" | "shot" => {
            let host = host_arg(live, args, 0)?;
            connected(live, &host, args)?;
            screenshot(output, live, &host, args)
        }
        "apps" => {
            let host = host_arg(live, args, 0)?;
            connected(live, &host, args)?;
            apps(output, live, &host)
        }
        "push" => {
            let host = host_arg(live, args, 0)?;
            let local = positional(args, 1, "LOCAL")?;
            let remote = positional(args, 2, "REMOTE")?;
            connected(live, &host, args)?;
            push(output, live, &host, local, remote, args)
        }
        "pull" => {
            let host = host_arg(live, args, 0)?;
            let remote = positional(args, 1, "REMOTE")?;
            let local = positional(args, 2, "LOCAL")?;
            connected(live, &host, args)?;
            pull(output, live, &host, remote, local, args)
        }
        "alias" => {
            if let Some(name) = args.option("remove") {
                let removed = crate::hosts::remove_alias(&store, name)?;
                return ok(
                    output,
                    json!({ "alias": name, "removed": removed, "message": if removed { "removed" } else { "no such alias" } }),
                );
            }
            if args.switch("list") || args.positional().is_empty() {
                let aliases = crate::hosts::aliases(&store);
                output.emit(&json!({ "aliases": aliases }), |v| {
                    let rows: Vec<Vec<String>> = v["aliases"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(name, key)| {
                            vec![name.clone(), key.as_str().unwrap_or("").to_owned()]
                        })
                        .collect();
                    if rows.is_empty() {
                        "no aliases; `openagents computer alias NAME HOST`".to_owned()
                    } else {
                        out::table(&rows)
                    }
                });
                return Ok(0);
            }
            let name = positional(args, 0, "NAME")?;
            let host = host_arg(live, args, 1)?;
            crate::hosts::set_alias(&store, name, &host)?;
            ok(
                output,
                json!({ "alias": name, "host": host, "message": format!("{name} -> {host}") }),
            )
        }
        "journal" => {
            let host = match args.positional().first() {
                Some(_) => Some(host_arg(live, args, 0)?),
                None => None,
            };
            let lines: usize = args.number("lines", 50)?;
            let entries = crate::hosts::journal_entries(&store, host.as_deref(), lines);
            output.emit(&json!({ "entries": entries }), |v| {
                let lines: Vec<String> = v["entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(crate::hosts::render_entry)
                    .collect();
                if lines.is_empty() {
                    "nothing has run yet".to_owned()
                } else {
                    lines.join("\n")
                }
            });
            Ok(0)
        }
        "client-only" => {
            live.run_without_local_host().map_err(|e| e.to_string())?;
            live.complete_first_run().map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        other => Ok(output.usage("computer", &format!("unknown command `{other}`"), USAGE)),
    }
}

/// The calls a transfer makes: each one NIP-HOST `computer` request the
/// host checks `terminal` on.
fn caller<'a>(
    live: &'a Live,
    host: &'a str,
) -> impl FnMut(coder_access::computer::Request) -> coder_access::Result<coder_access::computer::Answer>
+ 'a {
    move |request| live.computer(host, request)
}

/// A window of `computer` requests in flight at once over the host's
/// link, for file chunks.
fn batcher<'a>(
    live: &'a Live,
    host: &'a str,
) -> impl FnMut(
    Vec<coder_access::computer::Request>,
) -> Vec<coder_access::Result<coder_access::computer::Answer>>
+ 'a {
    move |requests| live.computer_many(host, requests)
}

/// A progress line on standard error while a transfer runs, when a person
/// watches it.
fn progress(output: &Output) -> impl FnMut(u64, u64) + use<> {
    let shown = !output.json() && std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut last = std::time::Instant::now();
    move |done: u64, total: u64| {
        if shown && (done == total || last.elapsed() >= Duration::from_millis(250)) {
            last = std::time::Instant::now();
            eprint!(
                "\r{:.1} / {:.1} MB{}",
                done as f64 / 1e6,
                total as f64 / 1e6,
                if done == total { "\n" } else { "" }
            );
        }
    }
}

/// Words for a transfer failure, naming the way on for the ones a person
/// can fix.
fn transfer_error(error: &coder_access::Error, remote: &str) -> String {
    match error.code {
        coder_access::Code::Conflict if !error.message.contains("changed while") => format!(
            "a file is already at {remote} on that computer, or what arrived did not match; \
             pass --overwrite to replace it"
        ),
        coder_access::Code::MissingRight => {
            "this device does not hold the `terminal` right on that computer".to_owned()
        }
        coder_access::Code::Unsupported => {
            "that computer's host does not serve files and screenshots yet; update it".to_owned()
        }
        _ => error.message.clone(),
    }
}

fn screenshot(output: &Output, live: &Live, host: &str, args: &Args) -> Result<u8, String> {
    use coder_access::computer::{Answer, MAX_SCREENSHOT_BYTES, Request, Source};
    let source = if args.switch("android") {
        Source::Android {
            serial: args.option("serial").map(str::to_owned),
        }
    } else {
        Source::Screen {
            screen: args.option("screen").map(str::to_owned),
        }
    };
    let mut call = caller(live, host);
    let started = std::time::Instant::now();
    let answer = call(Request::Screenshot { source })
        .and_then(Answer::able)
        .map_err(|e| transfer_error(&e, "the capture"))?;
    let Answer::File { file } = answer else {
        return Err("the host did not answer a screenshot".into());
    };
    let local = match args.option("out") {
        Some(path) => PathBuf::from(path),
        None => {
            let dir = store_dir(args.option("store")).join("captures");
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let name = crate::hosts::alias_of(&store_dir(args.option("store")), host)
                .unwrap_or_else(|| host.chars().take(12).collect());
            dir.join(format!("{name}-{}.png", now()))
        }
    };
    let bytes = fetch_to(
        &mut batcher(live, host),
        &file,
        MAX_SCREENSHOT_BYTES,
        &local,
        true,
        output,
    )?;
    output.emit(
        &json!({
            "host": host, "path": local.display().to_string(), "remote": file.path,
            "size": bytes, "digest": file.digest, "media_type": "image/png",
            "seconds": started.elapsed().as_secs_f64(),
        }),
        |v| v["path"].as_str().unwrap_or("").to_owned(),
    );
    Ok(0)
}

fn apps(output: &Output, live: &Live, host: &str) -> Result<u8, String> {
    use coder_access::computer::{Answer, Request};
    let answer = live
        .computer(host, Request::Apps {})
        .and_then(Answer::able)
        .map_err(|e| transfer_error(&e, "the app list"))?;
    let Answer::Apps { apps, source } = answer else {
        return Err("the host did not list its apps".into());
    };
    output.emit(
        &json!({ "host": host, "source": source, "apps": apps }),
        |v| {
            let mut rows = vec![vec!["app".to_owned(), "pid".to_owned(), "title".to_owned()]];
            for app in v["apps"].as_array().into_iter().flatten() {
                rows.push(vec![
                    format!(
                        "{}{}",
                        app["name"].as_str().unwrap_or(""),
                        if app["focused"].as_bool().unwrap_or(false) {
                            " *"
                        } else {
                            ""
                        }
                    ),
                    app["pid"]
                        .as_i64()
                        .map_or_else(|| "-".into(), |p| p.to_string()),
                    app["title"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(80)
                        .collect(),
                ]);
            }
            if rows.len() == 1 {
                "no windows are open on that computer".to_owned()
            } else {
                out::table(&rows)
            }
        },
    );
    Ok(0)
}

/// Read `file` from the host into `local`: into a partial file beside it
/// first, renamed only once every byte matched the digest. An existing
/// `local` is replaced only when `overwrite` is set.
fn fetch_to(
    batch: &mut coder_access::computer::Batch<'_>,
    file: &coder_access::computer::FileInfo,
    limit: u64,
    local: &std::path::Path,
    overwrite: bool,
    output: &Output,
) -> Result<u64, String> {
    if local.exists() && !overwrite {
        return Err(format!(
            "{} already exists here; pass --overwrite to replace it",
            local.display()
        ));
    }
    let partial = local.with_file_name(format!(
        ".{}.oa-partial",
        local
            .file_name()
            .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned())
    ));
    let mut sink = std::io::BufWriter::new(
        std::fs::File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?,
    );
    let mut shown = progress(output);
    let fetched =
        coder_access::computer::fetch_described_many(batch, file, limit, &mut sink, &mut shown);
    let flushed = std::io::Write::flush(&mut sink).map_err(|e| e.to_string());
    drop(sink);
    if let Err(error) = fetched
        .map_err(|e| transfer_error(&e, &file.path))
        .and(flushed)
    {
        let _ = std::fs::remove_file(&partial);
        return Err(error);
    }
    std::fs::rename(&partial, local).map_err(|e| format!("{}: {e}", local.display()))?;
    Ok(file.size)
}

fn push(
    output: &Output,
    live: &Live,
    host: &str,
    local: &str,
    remote: &str,
    args: &Args,
) -> Result<u8, String> {
    use coder_access::computer::MAX_FILE_BYTES;
    let local = std::path::Path::new(local);
    let size = std::fs::metadata(local)
        .map_err(|e| format!("{}: {e}", local.display()))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(format!(
            "{} is {size} bytes, over the {MAX_FILE_BYTES} byte limit",
            local.display()
        ));
    }
    let bytes = std::fs::read(local).map_err(|e| format!("{}: {e}", local.display()))?;
    // A folder-shaped destination keeps the file's own name.
    let remote = if remote.ends_with('/') || remote == "~" {
        let name = local
            .file_name()
            .ok_or("LOCAL names no file")?
            .to_string_lossy();
        format!("{}/{name}", remote.trim_end_matches('/'))
    } else {
        remote.to_owned()
    };
    let started = std::time::Instant::now();
    let mut shown = progress(output);
    let digest = coder_access::computer::send_many(
        &mut batcher(live, host),
        &remote,
        &bytes,
        args.switch("overwrite"),
        &mut shown,
    )
    .map_err(|e| transfer_error(&e, &remote))?;
    let seconds = started.elapsed().as_secs_f64();
    output.emit(
        &json!({ "host": host, "local": local.display().to_string(), "remote": remote,
                 "size": size, "digest": digest, "seconds": seconds }),
        |v| {
            format!(
                "{} -> {} ({} bytes, {})",
                v["local"].as_str().unwrap_or(""),
                v["remote"].as_str().unwrap_or(""),
                v["size"],
                v["digest"].as_str().unwrap_or("")
            )
        },
    );
    Ok(0)
}

fn pull(
    output: &Output,
    live: &Live,
    host: &str,
    remote: &str,
    local: &str,
    args: &Args,
) -> Result<u8, String> {
    use coder_access::computer::{Answer, MAX_FILE_BYTES, Request};
    let limit: u64 = args.number("max-bytes", MAX_FILE_BYTES)?;
    let started = std::time::Instant::now();
    let mut call = caller(live, host);
    let Answer::File { file } = call(Request::Stat {
        path: remote.to_owned(),
    })
    .and_then(Answer::able)
    .map_err(|e| transfer_error(&e, remote))?
    else {
        return Err("the host did not describe the file".into());
    };
    let mut local = PathBuf::from(local);
    if local.is_dir() {
        let name = file.path.rsplit(['/', '\\']).next().unwrap_or("file");
        local = local.join(name);
    }
    let size = fetch_to(
        &mut batcher(live, host),
        &file,
        limit,
        &local,
        args.switch("overwrite"),
        output,
    )?;
    output.emit(
        &json!({ "host": host, "remote": file.path, "local": local.display().to_string(),
                 "size": size, "digest": file.digest,
                 "seconds": started.elapsed().as_secs_f64() }),
        |v| {
            format!(
                "{} -> {} ({} bytes, {})",
                v["remote"].as_str().unwrap_or(""),
                v["local"].as_str().unwrap_or(""),
                v["size"],
                v["digest"].as_str().unwrap_or("")
            )
        },
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_computers::model::{Compatibility, DeviceList, Enrollment, HostRecord};

    /// A host that publishes nothing to this computer may be off or may have
    /// removed it; the message says both and how to check and enroll again,
    /// in words rather than the supervisor's debug text (#10368).
    #[test]
    fn a_silent_host_names_both_causes_and_the_way_back() {
        let record = HostRecord {
            key: "93".repeat(32),
            label: "coderos-4080".into(),
            listing: None,
            delisted: false,
            ssh: None,
            tunnel: None,
            enrollment: Enrollment::NotEnrolled,
            link: None,
            route: None,
            compatibility: Compatibility::Unknown,
            presence: None,
            devices: DeviceList::NotLoaded,
            enrollments: Vec::new(),
            workspaces: None,
            watchers: None,
            background: None,
        };
        let name = host_name(&record, &record.key);
        assert_eq!(name, "coderos-4080");
        let text = not_connected(&record, &name, 15, "--wait");
        assert!(text.starts_with("coderos-4080 did not connect within 15s (not connecting)."));
        assert!(text.contains("it removed this computer"));
        assert!(text.contains("`openagents connect devices`"));
        assert!(text.contains("`openagents computer link CODE`"));
        assert!(!text.contains("Backoff"));
        assert_eq!(
            phase_words(&coder_link::Phase::Backoff {
                until: coder_link::Moment(5)
            }),
            "retrying"
        );
        assert_eq!(
            phase_words(&coder_link::Phase::Blocked(
                coder_link::BlockReason::Revoked
            )),
            "it removed this computer's access"
        );
    }
}
