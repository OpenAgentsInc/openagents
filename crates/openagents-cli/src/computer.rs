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

const USAGE: &str = "usage: openagents computer COMMAND [OPTIONS]
  list [--wait SECONDS]     Every host this device knows, with its link and grant.
  show HOST                 One host: grant, devices, pending enrollments, workspaces.
  link INVITATION           Redeem a coder-host: invitation (QR text or paste).
  link --ssh USER@HOST      Install or adopt a host over ssh and redeem its invitation.
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
  task HOST --workspace LABEL --title TITLE PROMPT...
                            Order work on the host; prints the task id.
  steer HOST TASK --revision N PROMPT...
  cancel HOST TASK --revision N [--reason TEXT]
  exec HOST [--timeout S] [--rows N --cols N] -- CMD [ARGS...]
                            Run a command in a shell on the host (NIP-TERM) and
                            return its output and exit code.
  shell HOST                An interactive shell on the host. Ctrl-] detaches.
  client-only               Record that this machine runs no local host.
Options: --store DIR (default ~/.openagents/coder-computers), --wait SECONDS
(how long to wait for the host's link; default 15), --same-machine
(hosts run on this computer; allows loopback routes), --loopback-test.";

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

fn open(args: &Args, runtime: &tokio::runtime::Runtime) -> Result<Live, String> {
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

fn host_json(snapshot: &Snapshot, host: &coder_computers::model::HostRecord) -> Value {
    json!({
        "key": host.key,
        "label": host.label,
        "listed": host.listing.is_some(),
        "delisted": host.delisted,
        "weight": host.weight(),
        "ssh": host.ssh,
        "tunnel": host.tunnel.as_ref().map(|t| json!({ "open": t.open, "in_use": t.in_use })),
        "enrollment": enrollment_json(&host.enrollment),
        "rights_now": host.enrollment.rights(snapshot.now).map(rights_text),
        "link": host.link.as_ref().map(|link| json!({
            "phase": format!("{:?}", link.phase),
            "freshness": format!("{:?}", link.freshness),
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

fn snapshot_json(snapshot: &Snapshot) -> Value {
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
        "hosts": snapshot.hosts.iter().map(|h| host_json(snapshot, h)).collect::<Vec<_>>(),
    })
}

fn render_hosts(value: &Value) -> String {
    let mut rows = vec![vec![
        "label".to_owned(),
        "link".to_owned(),
        "rights".to_owned(),
        "route".to_owned(),
        "host".to_owned(),
    ]];
    for host in value["hosts"].as_array().into_iter().flatten() {
        rows.push(vec![
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
    let args = match Args::parse(rest, &["same-machine", "loopback-test"]) {
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

fn positional<'a>(args: &'a Args, index: usize, name: &str) -> Result<&'a str, String> {
    args.positional()
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} is required"))
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
fn connected(live: &mut Live, host: &str, args: &Args) -> Result<(), String> {
    let seconds: u64 = args.number("wait", 15)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
    let mut last = String::from("no link");
    loop {
        let snapshot = live.snapshot().map_err(|e| e.to_string())?;
        let record = snapshot
            .host(host)
            .ok_or_else(|| format!("this device knows no host {host}"))?;
        match record.link.as_ref().map(|link| &link.phase) {
            Some(coder_link::Phase::Connected) => return Ok(()),
            Some(coder_link::Phase::Blocked(reason)) => {
                return Err(format!("the link to {host} is blocked: {reason:?}"));
            }
            Some(phase) => last = format!("{phase:?}"),
            None => {}
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "{host} did not connect within {seconds}s (link {last}); pass --wait SECONDS to wait longer"
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
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
    match command {
        "list" | "ls" => {
            let snapshot = settle(live, args.number("wait", 3)?)?;
            output.emit(&snapshot_json(&snapshot), render_hosts);
            Ok(0)
        }
        "show" => {
            let key = positional(args, 0, "HOST")?;
            let snapshot = settle(live, args.number("wait", 3)?)?;
            let host = snapshot
                .host(key)
                .ok_or_else(|| format!("this device knows no host {key}"))?;
            output.emit(&host_json(&snapshot, host), |v| {
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
                            &json!({ "destination": destination, "stage": stage, "hosts": snapshot_json(&snapshot)["hosts"] }),
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
            let host = live
                .redeem_invitation(&invitation)
                .map_err(|e| e.to_string())?;
            let snapshot = settle(live, 5)?;
            let record = snapshot.host(&host).map(|h| host_json(&snapshot, h));
            output.emit(&json!({ "host": host, "record": record }), |v| {
                format!("linked host {}", v["host"].as_str().unwrap_or(""))
            });
            Ok(0)
        }
        "approve" => {
            let host = positional(args, 0, "HOST")?;
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
            let host = positional(args, 0, "HOST")?;
            connected(live, host, args)?;
            live.deny_enrollment(host, positional(args, 1, "ENROLLMENT")?)
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "invite" => {
            let host = positional(args, 0, "HOST")?;
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
            let host = positional(args, 0, "HOST")?;
            connected(live, host, args)?;
            live.refresh_devices(host).map_err(|e| e.to_string())?;
            let snapshot = live.snapshot().map_err(|e| e.to_string())?;
            let record = snapshot.host(host).ok_or("unknown host")?;
            let value = host_json(&snapshot, record);
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
            let host = positional(args, 0, "HOST")?;
            connected(live, host, args)?;
            live.revoke(host, positional(args, 1, "DEVICE")?)
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "forget" => {
            live.forget(positional(args, 0, "HOST")?)
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "enable" | "disable" => {
            live.set_enabled(positional(args, 0, "HOST")?, command == "enable")
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "retry" => {
            live.retry_now(positional(args, 0, "HOST")?)
                .map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        "workspaces" => {
            let host = positional(args, 0, "HOST")?;
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
            let host = positional(args, 0, "HOST")?;
            let prompt = args.positional()[1..].join(" ");
            if prompt.trim().is_empty() {
                return Err("PROMPT is required".into());
            }
            let task = TaskCreate {
                title: args
                    .option("title")
                    .map_or_else(|| prompt.chars().take(60).collect(), str::to_owned),
                prompt,
                workspace: args
                    .option("workspace")
                    .ok_or("--workspace LABEL is required")?
                    .to_owned(),
            };
            connected(live, host, args)?;
            let id = live.create_task(host, &task).map_err(|e| e.to_string())?;
            output.emit(&json!({ "host": host, "task": id }), |v| {
                v["task"].as_str().unwrap_or("").to_owned()
            });
            Ok(0)
        }
        "steer" => {
            let host = positional(args, 0, "HOST")?;
            let task = positional(args, 1, "TASK")?;
            let revision: u64 = args.number("revision", 0)?;
            let prompt = args.positional()[2..].join(" ");
            connected(live, host, args)?;
            live.steer_task(host, task, revision, &prompt)
                .map_err(|e| e.to_string())?;
            ok(output, json!({ "task": task }))
        }
        "cancel" => {
            let host = positional(args, 0, "HOST")?;
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
        "exec" | "run" => {
            let host = positional(args, 0, "HOST")?;
            if args.positional().len() < 2 {
                return Ok(output.usage(
                    "computer",
                    "a command is required after HOST (put `--` before it)",
                    USAGE,
                ));
            }
            connected(live, host, args)?;
            crate::terminal::exec(*output, live, runtime, host, &args.positional()[1..], args)
        }
        "shell" | "sh" => {
            let host = positional(args, 0, "HOST")?;
            connected(live, host, args)?;
            crate::terminal::shell(*output, live, runtime, host, args)
        }
        "client-only" => {
            live.run_without_local_host().map_err(|e| e.to_string())?;
            live.complete_first_run().map_err(|e| e.to_string())?;
            ok(output, json!({}))
        }
        other => Ok(output.usage("computer", &format!("unknown command `{other}`"), USAGE)),
    }
}
