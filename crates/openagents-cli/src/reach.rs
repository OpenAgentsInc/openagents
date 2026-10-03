//! `openagents reach`: the owner directory, host presence, and route probes
//! (NIP-REACH). Every command runs through the same live service the
//! Computers screens use (`coder_computers::live::Live`), so the owner
//! authority rule, the freshness verdict, and route selection cannot drift
//! from the phone.

use std::time::{Duration, Instant};

use coder_computers::live::{FileStore, Live, Settings, Store, load_or_create_key};
use coder_computers::model::{DirectoryState, HostRecord, Snapshot};
use coder_computers::{ComputersService, Platform};
use coder_host::client::Route;
use coder_reach::hints::Locality;
use coder_reach::presence::Freshness;
use serde_json::{Value, json};

use crate::computer::store_dir;
use crate::{Args, Output, out};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents reach COMMAND [OPTIONS]
  directory list                  Read the owner directory this device trusts.
  directory add HOST [--label TEXT]
                                  Publish the next directory revision with HOST listed.
  directory remove HOST           Publish the next directory revision without HOST.
  presence HOST                   Read the host's presence sample and its freshness verdict.
  probe HOST                      Run the encrypted direct-channel handshake and report
                                  the route class: loopback, lan, tailnet, public, or relay.

Options:
  --relay URL         The relay to use. It must be a relay one of this device's
                      grants names; the command refuses any other relay.
  --timeout SECONDS   How long to wait for the relay and the handshake (default: 15).
  --store DIR         The device store (default: the coder-computers directory).
  --same-machine      Allow loopback routes. Claim this only when the host runs
                      on this machine; otherwise the probe refuses loopback hints.
  --loopback-test     Accept loopback relays. Only a local test run uses this.

HOST is the label `openagents computer list` shows, a host key, a unique
prefix of either, or an alias set with
`openagents computer alias`. Directory edits need the owner key on this
device; `openagents computer` imports it. Nothing here prints a key or an
invitation.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("directory list", Effect::ReadOnly),
    Declared::computer("directory add", Effect::Grants),
    Declared::computer("directory remove", Effect::Grants),
    Declared::computer("presence", Effect::ReadOnly),
    Declared::computer("probe", Effect::ReadOnly),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("reach", "a command is required", USAGE);
    };
    if command == "--help" || command == "-h" || command == "help" {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &["same-machine", "loopback-test"]) {
        Ok(args) => args,
        Err(message) => return output.usage("reach", &message, USAGE),
    };
    let timeout = match args.number::<u64>("timeout", 15) {
        Ok(0) => return output.usage("reach", "--timeout must be at least 1 second", USAGE),
        Ok(seconds) => Duration::from_secs(seconds),
        Err(message) => return output.usage("reach", &message, USAGE),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return output.fail("reach", &format!("tokio runtime: {error}")),
    };
    let mut live = match open(&args, timeout, &runtime) {
        Ok(live) => live,
        Err(message) => return output.fail("reach", &message),
    };
    let result = match command.as_str() {
        "directory" | "dir" => directory(output, &mut live, &args, timeout),
        "presence" => presence(output, &mut live, &args, timeout),
        "probe" => probe(output, &mut live, &args, timeout, &runtime),
        _ => Err(Usage(format!("unknown reach command `{command}`"))),
    };
    match result {
        Ok(code) => code,
        Err(Usage(message)) => output.usage("reach", &message, USAGE),
        Err(Refused(message)) => output.fail("reach", &message),
    }
}

/// Why a command stopped: a usage mistake (exit 64) or a refusal (exit 1).
enum Stop {
    Usage(String),
    Refused(String),
}
use Stop::{Refused, Usage};

impl From<String> for Stop {
    fn from(message: String) -> Self {
        Refused(message)
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn open(args: &Args, timeout: Duration, runtime: &tokio::runtime::Runtime) -> Result<Live, String> {
    let directory = store_dir(args.option("store"));
    std::fs::create_dir_all(&directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    let mut settings = Settings::new(Platform::Terminal);
    settings.now = now;
    settings.link.establish_timeout = timeout;
    if args.switch("loopback-test") {
        settings.policy = coder_access::RelayPolicy::LoopbackTest;
    }
    if args.switch("same-machine") {
        settings.locality = Locality::SameMachine;
    }
    let secret = load_or_create_key(&directory)?;
    let mut store = FileStore::open(&directory)?;
    if let Some(relay) = args.option("relay") {
        check_relay(&mut store, relay)?;
    }
    Live::open(settings, secret, Box::new(store), runtime.handle().clone())
        .map_err(|error| error.to_string())
}

/// The live service speaks to the relays the grants name. A `--relay` that
/// names another relay is refused instead of silently ignored.
fn check_relay(store: &mut FileStore, relay: &str) -> Result<(), String> {
    let saved = store.load()?.ok_or_else(|| {
        format!("this device has no grants; --relay {relay} names a relay no grant uses")
    })?;
    let named = saved
        .hosts
        .iter()
        .any(|host| host.access.grant.relay.trim_end_matches('/') == relay.trim_end_matches('/'));
    if named {
        Ok(())
    } else {
        Err(format!(
            "--relay {relay} names a relay no grant on this device uses; reach speaks to the relays the grants name"
        ))
    }
}

fn host_arg(live: &mut Live, args: &Args, index: usize) -> Result<String, Stop> {
    let text = args
        .positional()
        .get(index)
        .ok_or_else(|| Usage("HOST is required".to_owned()))?;
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
    crate::hosts::resolve(&store_dir(args.option("store")), &known, text).map_err(Refused)
}

/// Poll the live service until `ready` accepts a snapshot or the timeout
/// passes. Returns the last snapshot either way; the caller judges it.
fn wait_for(
    live: &mut Live,
    timeout: Duration,
    ready: impl Fn(&Snapshot) -> bool,
) -> Result<Snapshot, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let snapshot = live.snapshot().map_err(|e| e.to_string())?;
        if ready(&snapshot) || Instant::now() >= deadline {
            return Ok(snapshot);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn directory(output: &Output, live: &mut Live, args: &Args, timeout: Duration) -> Result<u8, Stop> {
    let Some(action) = args.positional().first() else {
        return Err(Usage(
            "a directory command is required: list, add, or remove".into(),
        ));
    };
    let snapshot = wait_for(live, timeout, |snapshot| {
        snapshot.directory != DirectoryState::Loading
    })?;
    match action.as_str() {
        "list" => {
            if snapshot.directory == DirectoryState::Loading {
                return Err(Refused(format!(
                    "the owner directory did not load within {}s; pass --timeout SECONDS to wait longer",
                    timeout.as_secs()
                )));
            }
            let value = directory_json(&snapshot);
            output.emit(&value, render_directory);
            Ok(if snapshot.directory == DirectoryState::NoOwnerKey {
                out::EXIT_FAILURE
            } else {
                0
            })
        }
        "add" => {
            let host = host_arg(live, args, 1)?;
            let label = match args.option("label") {
                Some(label) => label.to_owned(),
                None => snapshot
                    .host(&host)
                    .map(|record| record.label.clone())
                    .filter(|label| !label.is_empty())
                    .ok_or_else(|| {
                        Usage("--label TEXT is required for a host without a label".into())
                    })?,
            };
            writable(&snapshot, timeout)?;
            live.list_in_directory(&host, &label)
                .map_err(|e| e.to_string())?;
            let snapshot = live.snapshot().map_err(|e| e.to_string())?;
            let mut value = directory_json(&snapshot);
            value["message"] = json!(format!("listed {host} as {label}"));
            value["host"] = json!(host);
            output.emit(&value, render_message);
            Ok(0)
        }
        "remove" => {
            let host = host_arg(live, args, 1)?;
            let revision = writable(&snapshot, timeout)?;
            live.remove_from_directory(&host, revision)
                .map_err(|e| e.to_string())?;
            let snapshot = live.snapshot().map_err(|e| e.to_string())?;
            let mut value = directory_json(&snapshot);
            value["message"] = json!(format!("removed {host} from the directory"));
            value["host"] = json!(host);
            output.emit(&value, render_message);
            Ok(0)
        }
        other => Err(Usage(format!("unknown directory command `{other}`"))),
    }
}

/// The owner authority rule: only a device that holds the owner key and read
/// the current revision may publish the next one.
fn writable(snapshot: &Snapshot, timeout: Duration) -> Result<u64, Stop> {
    match snapshot.directory {
        DirectoryState::Current { revision, .. } => Ok(revision.unwrap_or(0)),
        DirectoryState::NoOwnerKey => Err(Refused(
            "this device holds no owner key; only the owner edits the directory".into(),
        )),
        DirectoryState::Loading => Err(Refused(format!(
            "the owner directory did not load within {}s; pass --timeout SECONDS to wait longer",
            timeout.as_secs()
        ))),
        DirectoryState::Conflict { revision } => Err(Refused(format!(
            "two directories share revision {revision}; wait for the owner to publish a higher one"
        ))),
        DirectoryState::Failed { .. } => Err(Refused(
            "the last directory read failed; try again before editing".into(),
        )),
    }
}

fn directory_state_json(state: DirectoryState) -> Value {
    match state {
        DirectoryState::NoOwnerKey => json!({ "state": "no_owner_key" }),
        DirectoryState::Loading => json!({ "state": "loading" }),
        DirectoryState::Current { revision, as_of } => {
            json!({ "state": "current", "revision": revision, "as_of": as_of })
        }
        DirectoryState::Conflict { revision } => {
            json!({ "state": "conflict", "revision": revision })
        }
        DirectoryState::Failed { revision } => {
            json!({ "state": "failed", "revision": revision })
        }
    }
}

fn directory_json(snapshot: &Snapshot) -> Value {
    let hosts: Vec<Value> = snapshot
        .hosts
        .iter()
        .filter_map(|host| {
            let listing = host.listing.as_ref()?;
            Some(json!({
                "key": host.key,
                "label": host.label,
                "weight": listing.weight,
                "added_at": listing.added_at,
                "enrolled": !host.directory_only(),
            }))
        })
        .collect();
    json!({
        "owner": snapshot.owner,
        "directory": directory_state_json(snapshot.directory),
        "hosts": hosts,
    })
}

fn render_directory(value: &Value) -> String {
    let directory = &value["directory"];
    let mut lines = vec![format!(
        "directory: {}{}",
        directory["state"].as_str().unwrap_or("unknown"),
        directory["revision"]
            .as_u64()
            .map_or(String::new(), |revision| format!(" (revision {revision})"))
    )];
    let hosts = value["hosts"].as_array().map_or(&[][..], Vec::as_slice);
    if hosts.is_empty() {
        lines.push("no hosts listed".to_owned());
    }
    for host in hosts {
        lines.push(format!(
            "{}  weight {}  {}  {}",
            host["key"].as_str().unwrap_or(""),
            host["weight"].as_u64().unwrap_or(0),
            if host["enrolled"].as_bool().unwrap_or(false) {
                "enrolled"
            } else {
                "directory only"
            },
            host["label"].as_str().unwrap_or(""),
        ));
    }
    lines.join("\n")
}

fn render_message(value: &Value) -> String {
    value["message"]
        .as_str()
        .map_or_else(|| "ok".to_owned(), str::to_owned)
}

fn presence(output: &Output, live: &mut Live, args: &Args, timeout: Duration) -> Result<u8, Stop> {
    let host = host_arg(live, args, 0)?;
    let snapshot = wait_for(live, timeout, |snapshot| {
        snapshot
            .host(&host)
            .is_some_and(|record| record.presence.is_some())
    })?;
    let record = snapshot
        .host(&host)
        .ok_or_else(|| format!("this device knows no host {host}"))?;
    let Some(received) = &record.presence else {
        let name = crate::computer::host_name(record, &host);
        return Err(Refused(crate::computer::not_connected(
            record,
            &name,
            timeout.as_secs(),
            "--timeout",
        )));
    };
    let verdict = received.judge(snapshot.now, Freshness::default());
    let sample = &received.presence;
    let value = json!({
        "host": host,
        "presence": {
            "generation": sample.generation,
            "protocol": sample.protocol,
            "compatibility": sample.compatibility,
            "capabilities": sample.capabilities,
            "observed_at": sample.observed_at,
            "received_at": received.received_at,
            "telemetry": sample.telemetry,
        },
        "freshness": match &verdict {
            Ok(()) => json!({ "fresh": true, "max_age": Freshness::default().max_age }),
            Err(error) => json!({ "fresh": false, "reason": error.to_string() }),
        },
        "compatibility": compatibility_json(record),
        "now": snapshot.now,
    });
    output.emit(&value, render_presence);
    Ok(if verdict.is_ok() {
        0
    } else {
        out::EXIT_FAILURE
    })
}

fn compatibility_json(record: &HostRecord) -> Value {
    json!(format!("{:?}", record.compatibility).to_lowercase())
}

fn render_presence(value: &Value) -> String {
    let sample = &value["presence"];
    let freshness = &value["freshness"];
    let verdict = if freshness["fresh"].as_bool().unwrap_or(false) {
        "fresh".to_owned()
    } else {
        format!("not fresh: {}", freshness["reason"].as_str().unwrap_or(""))
    };
    format!(
        "{}\ngeneration {}  protocol {}  observed at {}  received at {}\nfreshness: {}\ncompatibility: {}\ncapabilities: {}",
        value["host"].as_str().unwrap_or(""),
        sample["generation"],
        sample["protocol"],
        sample["observed_at"],
        sample["received_at"],
        verdict,
        value["compatibility"].as_str().unwrap_or(""),
        sample["capabilities"]
            .as_array()
            .map_or(String::new(), |list| list
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")),
    )
}

fn probe(
    output: &Output,
    live: &mut Live,
    args: &Args,
    timeout: Duration,
    runtime: &tokio::runtime::Runtime,
) -> Result<u8, Stop> {
    let host = host_arg(live, args, 0)?;
    let locality = if args.switch("same-machine") {
        Locality::SameMachine
    } else {
        Locality::OtherMachine
    };
    let snapshot = wait_for(live, timeout, |snapshot| {
        snapshot.host(&host).is_some_and(|record| {
            record.link.as_ref().is_some_and(|link| {
                matches!(
                    link.phase,
                    coder_link::Phase::Connected | coder_link::Phase::Blocked(_)
                )
            })
        })
    })?;
    let record = snapshot
        .host(&host)
        .ok_or_else(|| format!("this device knows no host {host}"))?;
    let phase = record.link.as_ref().map(|link| &link.phase);
    let mut value = json!({
        "host": host,
        "locality": match locality {
            Locality::SameMachine => "same_machine",
            Locality::OtherMachine => "other_machine",
        },
        "loopback": match locality {
            Locality::SameMachine => "allowed",
            Locality::OtherMachine => "refused",
        },
        "compatibility": compatibility_json(record),
        "connected": false,
    });
    match phase {
        Some(coder_link::Phase::Connected) => {}
        Some(coder_link::Phase::Blocked(reason)) => {
            value["error"] = json!(format!(
                "{} can't be reached: {}",
                crate::computer::host_name(record, &host),
                crate::computer::blocked_words(*reason)
            ));
            output.emit(&value, render_probe);
            return Ok(out::EXIT_FAILURE);
        }
        _ => {
            value["error"] = json!(crate::computer::not_connected(
                record,
                &crate::computer::host_name(record, &host),
                timeout.as_secs(),
                "--timeout",
            ));
            output.emit(&value, render_probe);
            return Ok(out::EXIT_FAILURE);
        }
    }
    let link = live.host_link(&host).map_err(|e| e.to_string())?;
    let (kind, address) = match link.route() {
        Route::Direct(address) => ("direct", address.clone()),
        Route::Relay(relay) => ("relay", relay.clone()),
    };
    let started = Instant::now();
    let ping = runtime.block_on(async {
        tokio::time::timeout(timeout, link.ping())
            .await
            .map_err(|_| format!("the ping did not answer within {}s", timeout.as_secs()))
            .and_then(|result| result.map_err(|e| e.to_string()))
    });
    value["connected"] = json!(true);
    value["route"] = json!({
        "kind": kind,
        "address": address,
        "class": record.route.map(|class| format!("{class:?}").to_lowercase()),
    });
    value["generation"] = json!(link.generation());
    match ping {
        Ok(()) => {
            value["ping_ms"] = json!(started.elapsed().as_millis());
            output.emit(&value, render_probe);
            Ok(0)
        }
        Err(error) => {
            value["error"] = json!(error);
            output.emit(&value, render_probe);
            Ok(out::EXIT_FAILURE)
        }
    }
}

fn render_probe(value: &Value) -> String {
    let host = value["host"].as_str().unwrap_or("");
    let loopback = value["loopback"].as_str().unwrap_or("");
    if let Some(error) = value["error"].as_str() {
        return format!("{host}\nloopback {loopback}\n{error}");
    }
    let route = &value["route"];
    format!(
        "{host}\nroute: {} via {} ({})\nloopback {loopback}\nping {} ms",
        route["class"].as_str().unwrap_or("unknown"),
        route["kind"].as_str().unwrap_or(""),
        route["address"].as_str().unwrap_or(""),
        value["ping_ms"],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_computers::model::{
        Compatibility, DeviceList, Enrollment, Listing, LocalHost, ServiceState,
    };

    fn snapshot(directory: DirectoryState, hosts: Vec<HostRecord>) -> Snapshot {
        Snapshot {
            now: 1_000,
            device: "d".repeat(64),
            owner: true,
            service: ServiceState::Ready,
            local_host: LocalHost::ClientOnly,
            first_run_complete: true,
            hosts,
            activity: Vec::new(),
            directory,
            ssh_ready: false,
            ssh: None,
        }
    }

    fn host(key: &str, listing: Option<Listing>) -> HostRecord {
        HostRecord {
            key: key.to_owned(),
            label: "box".to_owned(),
            listing,
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
        }
    }

    #[test]
    fn directory_json_lists_only_listed_hosts() {
        let listed = host(
            &"a".repeat(64),
            Some(Listing {
                weight: 100,
                added_at: 5,
            }),
        );
        let unlisted = host(&"b".repeat(64), None);
        let value = directory_json(&snapshot(
            DirectoryState::Current {
                revision: Some(3),
                as_of: 900,
            },
            vec![listed, unlisted],
        ));
        assert_eq!(value["directory"]["state"], "current");
        assert_eq!(value["directory"]["revision"], 3);
        assert_eq!(value["hosts"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["hosts"][0]["weight"], 100);
        assert_eq!(value["hosts"][0]["enrolled"], false);
        let text = render_directory(&value);
        assert!(text.starts_with("directory: current (revision 3)"));
        assert!(text.contains("directory only"));
    }

    #[test]
    fn owner_authority_rule_gates_edits() {
        let timeout = Duration::from_secs(1);
        assert!(matches!(
            writable(&snapshot(DirectoryState::NoOwnerKey, Vec::new()), timeout),
            Err(Refused(message)) if message.contains("owner key")
        ));
        assert!(matches!(
            writable(
                &snapshot(DirectoryState::Conflict { revision: 4 }, Vec::new()),
                timeout
            ),
            Err(Refused(message)) if message.contains("revision 4")
        ));
        assert_eq!(
            writable(
                &snapshot(
                    DirectoryState::Current {
                        revision: Some(7),
                        as_of: 1
                    },
                    Vec::new()
                ),
                timeout
            )
            .ok(),
            Some(7)
        );
    }

    #[test]
    fn probe_projection_names_the_route_class() {
        let value = json!({
            "host": "h",
            "loopback": "refused",
            "route": { "kind": "direct", "address": "100.64.0.2:7000", "class": "tailnet" },
            "ping_ms": 12,
        });
        let text = render_probe(&value);
        assert!(text.contains("route: tailnet via direct (100.64.0.2:7000)"));
        assert!(text.contains("loopback refused"));
        let failed = json!({ "host": "h", "loopback": "refused", "error": "blocked" });
        assert!(render_probe(&failed).ends_with("blocked"));
    }
}
