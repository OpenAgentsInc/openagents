//! `openagents connect`: the power-user path to pairing a phone with this
//! computer, for a computer without the desktop app or a person who wants
//! to script it.
//!
//! Every command talks to the running host through its local control
//! socket, the same one the desktop app uses, so there is one path for the
//! owner's actions on this computer. The host serves only this user there;
//! nothing here opens the access store directly.

use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use openagents_connect::control::{self, Op, Reply, Request};
use serde_json::{Value, json};
use tokio::net::UnixStream;

use crate::out::{Output, table};

mod ssh;

pub const USAGE: &str = "usage: openagents connect COMMAND [--socket PATH]
       openagents connect --ssh DESTINATION [OPTIONS]
  invite [--text]                show a QR code for the OpenAgents app on a
                                 phone; waits until a phone connects or the
                                 code expires, then cancels it. The phone
                                 gets full access, a terminal included
  devices                        the phones that can reach this computer
  remove DEVICE                  cut a phone off now (hex key or npub)
  status                         this computer's host at a glance
  owner import                   use the owner key from your other computers;
                                 reads an nsec or hex secret key from stdin
The host must be running with its control socket (`openagents host
serve --control`, or the OpenAgents desktop app). --socket names another socket.
--ssh sets up a headless computer over SSH and pairs this computer with it;
see `openagents connect --ssh --help`.";

/// What each `connect` command does. `invite` and `remove` change who can
/// reach this computer, so the phone does them on its computers screen;
/// `owner import` reads a secret key.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::screen("invite", Effect::Grants, "account.computers"),
    Declared::computer("devices", Effect::ReadOnly),
    Declared::screen("remove", Effect::Grants, "account.computers"),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("owner import", Effect::Secret),
];

/// How often `invite` looks for the phone that scanned.
const POLL: Duration = Duration::from_secs(1);

pub fn run(output: &Output, args: &[String]) -> u8 {
    let mut args = args.to_vec();
    let socket = match take_value(&mut args, "--socket") {
        Ok(socket) => socket.map(PathBuf::from).or_else(control::socket_path),
        Err(message) => return output.usage("connect", &message, USAGE),
    };
    let Some((command, rest)) = args.split_first() else {
        return output.usage("connect", "COMMAND is required", USAGE);
    };
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!("{USAGE}");
        return 0;
    }
    if let Some(code) = ssh::dispatch(output, command, rest) {
        return code;
    }
    let Some(socket) = socket else {
        return output.fail(
            "connect",
            "no control socket path on this platform; pass --socket PATH",
        );
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return output.fail("connect", "cannot start the runtime"),
    };
    let rest = rest.to_vec();
    let result = runtime.block_on(async {
        match command.as_str() {
            "invite" => invite(output, &socket, &rest).await,
            "devices" => devices(output, &socket, &rest).await,
            "remove" => remove(output, &socket, &rest).await,
            "status" => status(output, &socket, &rest).await,
            "owner" => owner(output, &socket, &rest).await,
            other => Err(Failure::Usage(format!("unknown command `{other}`"))),
        }
    });
    match result {
        Ok(()) => 0,
        Err(Failure::Usage(message)) => output.usage("connect", &message, USAGE),
        Err(Failure::Failed(message)) => output.fail("connect", &message),
    }
}

enum Failure {
    Usage(String),
    Failed(String),
}

fn failed(message: impl Into<String>) -> Failure {
    Failure::Failed(message.into())
}

fn take_value(args: &mut Vec<String>, name: &str) -> Result<Option<String>, String> {
    let Some(index) = args.iter().position(|arg| arg == name) else {
        return Ok(None);
    };
    if index + 1 >= args.len() {
        return Err(format!("{name} needs a value"));
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Ok(Some(value))
}

fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    let before = args.len();
    args.retain(|arg| arg != name);
    before != args.len()
}

fn no_more(args: &[String]) -> Result<(), Failure> {
    match args.first() {
        None => Ok(()),
        Some(arg) => Err(Failure::Usage(format!("unexpected argument `{arg}`"))),
    }
}

/// A connection to the host's control socket.
struct Host {
    stream: UnixStream,
    next: u64,
}

impl Host {
    async fn open(socket: &Path) -> Result<Self, Failure> {
        let stream = UnixStream::connect(socket).await.map_err(|_| {
            failed(format!(
                "no host answers at {}; start it with `openagents host serve --control` or open the \
                 OpenAgents app",
                socket.display()
            ))
        })?;
        Ok(Self { stream, next: 1 })
    }

    async fn call(&mut self, op: Op) -> Result<Reply, Failure> {
        let id = self.next;
        self.next += 1;
        match control::call(&mut self.stream, &Request::new(id, op)).await {
            Ok(Reply::Refused { code, message }) => Err(failed(format!("{message} ({code})"))),
            Ok(reply) => Ok(reply),
            Err(error) => Err(failed(format!("the host did not answer: {error}"))),
        }
    }
}

async fn invite(output: &Output, socket: &Path, args: &[String]) -> Result<(), Failure> {
    let mut args = args.to_vec();
    // Every code grants the full rights; an earlier `--terminal` is
    // accepted and changes nothing.
    let _ = take_flag(&mut args, "--terminal");
    let text = take_flag(&mut args, "--text");
    no_more(&args)?;
    let mut host = Host::open(socket).await?;
    let Reply::Devices { devices: before } = host.call(Op::DeviceList {}).await? else {
        return Err(failed("the host answered another question"));
    };
    let Reply::Invite {
        invitation,
        code,
        expires_at,
        rights,
    } = host.call(Op::InviteCreate {}).await?
    else {
        return Err(failed("the host answered another question"));
    };
    if output.json() {
        println!(
            "{}",
            json!({"invitation": invitation, "code": code, "expires_at": expires_at, "rights": rights})
        );
    } else {
        // The QR shows the link form, so a phone's own camera opens the app.
        let qr = openagents_connect::code::link(&code)
            .and_then(|link| {
                coder_connect::pairing::terminal_qr_prefixed(
                    openagents_connect::code::LINK_PREFIX,
                    &link,
                )
                .ok()
            })
            .ok_or_else(|| failed("the code cannot be drawn"))?;
        println!("{qr}");
        println!("Scan with the OpenAgents app on your phone: Connect a computer.");
        if text {
            println!("\nOr paste this code in the app. Anyone with it can connect once:\n{code}");
        }
    }
    let known: Vec<String> = before.into_iter().map(|d| d.device).collect();
    let deadline = expires_at;
    let mut terminate =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
    // Wait for the phone that scans, then cancel the code either way, so
    // it is never redeemable after this command ends.
    let mut stopped = false;
    let paired = loop {
        let now = openagents_connect::now();
        if now >= deadline {
            break None;
        }
        let stop = tokio::select! {
            () = tokio::time::sleep(POLL) => false,
            _ = tokio::signal::ctrl_c() => true,
            Some(()) = async {
                match terminate.as_mut() {
                    Some(signal) => signal.recv().await,
                    None => std::future::pending().await,
                }
            } => true,
        };
        if stop {
            stopped = true;
            break None;
        }
        let Reply::Devices { devices } = host.call(Op::DeviceList {}).await? else {
            continue;
        };
        if let Some(device) = devices
            .into_iter()
            .find(|d| !d.revoked && !known.contains(&d.device))
        {
            break Some(device);
        }
    };
    let _ = host.call(Op::InviteCancel { invitation }).await;
    match paired {
        Some(device) => {
            output.emit(
                &json!({"connected": device.device, "rights": device.rights}),
                |_| "Connected. The phone can reach this computer now.".into(),
            );
            Ok(())
        }
        None if stopped => {
            output.emit(&json!({"connected": null, "cancelled": true}), |_| {
                "Stopped. The code is cancelled; no phone was added.".into()
            });
            Ok(())
        }
        None => Err(failed("no phone connected before the code expired")),
    }
}

async fn devices(output: &Output, socket: &Path, args: &[String]) -> Result<(), Failure> {
    no_more(args)?;
    let mut host = Host::open(socket).await?;
    let Reply::Devices { devices } = host.call(Op::DeviceList {}).await? else {
        return Err(failed("the host answered another question"));
    };
    let value = serde_json::to_value(&devices).unwrap_or(Value::Null);
    output.emit(&json!({ "devices": value }), |_| {
        if devices.is_empty() {
            return "No phones yet. Run `openagents connect invite`.".into();
        }
        let mut rows = vec![vec![
            "DEVICE".to_owned(),
            "RIGHTS".to_owned(),
            "LAST SEEN".to_owned(),
            "STATE".to_owned(),
        ]];
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut removed = 0;
        for device in &devices {
            if device.revoked {
                removed += 1;
                continue;
            }
            rows.push(vec![
                short_key(&device.device),
                rights_words(&device.rights),
                device
                    .last_seen
                    .map_or_else(|| "never".to_owned(), |at| ago(now, at)),
                "active".to_owned(),
            ]);
        }
        let mut text = if rows.len() > 1 {
            table(&rows)
        } else {
            "No active phones. Run `openagents connect invite`.".to_owned()
        };
        if removed > 0 {
            text.push_str(&format!(
                "\n{removed} removed {} not shown (`--json` lists them).",
                if removed == 1 { "phone" } else { "phones" }
            ));
        }
        text
    });
    Ok(())
}

/// A device key as a person reads it: the first and last few characters.
fn short_key(key: &str) -> String {
    if key.len() <= 16 {
        return key.to_owned();
    }
    format!("{}…{}", &key[..8], &key[key.len() - 4..])
}

/// Device rights in plain words.
fn rights_words(rights: &[String]) -> String {
    let words: Vec<String> = rights
        .iter()
        .map(|right| {
            match right.as_str() {
                "access_read" => "read chats",
                "access_admin" => "manage access",
                other => {
                    return other
                        .strip_prefix("access_")
                        .unwrap_or(other)
                        .replace('_', " ");
                }
            }
            .to_owned()
        })
        .collect();
    if words.is_empty() {
        "none".to_owned()
    } else {
        words.join(", ")
    }
}

/// How long ago `at` was, relative to `now`, both Unix seconds.
fn ago(now: u64, at: u64) -> String {
    let secs = now.saturating_sub(at);
    match secs {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

async fn remove(output: &Output, socket: &Path, args: &[String]) -> Result<(), Failure> {
    let [device] = args else {
        return Err(Failure::Usage("remove takes one DEVICE".into()));
    };
    let device = device_key(device)?;
    let mut host = Host::open(socket).await?;
    let Reply::Revoked { device, epoch } = host.call(Op::DeviceRevoke { device }).await? else {
        return Err(failed("the host answered another question"));
    };
    output.emit(&json!({"removed": device, "epoch": epoch}), |_| {
        "Removed. That phone can no longer reach this computer.".into()
    });
    Ok(())
}

async fn status(output: &Output, socket: &Path, args: &[String]) -> Result<(), Failure> {
    no_more(args)?;
    let mut host = Host::open(socket).await?;
    let Reply::Status(status) = host.call(Op::Status {}).await? else {
        return Err(failed("the host answered another question"));
    };
    // Tailnet admission (#10125); a host older than it refuses the
    // question, and the status reads as before.
    let tailnet = match host.call(Op::TailnetStatus {}).await {
        Ok(Reply::Tailnet {
            address,
            chats,
            off,
        }) => Some((address, chats, off)),
        _ => None,
    };
    let mut value = serde_json::to_value(&status).unwrap_or(Value::Null);
    let offline = offline_reason(&status.endpoint, status.online);
    if let Some(reason) = offline {
        value["offline_reason"] = json!(reason);
    }
    let tailnet_line = match &tailnet {
        Some((Some(address), chats, _)) => {
            value["tailnet"] = json!({"address": address, "chats": chats});
            format!(
                "\ntailnet {address}{}",
                if *chats { " with chats" } else { "" }
            )
        }
        Some((None, _, Some(off))) => {
            value["tailnet"] = json!({"off": off});
            format!("\ntailnet off: {off}")
        }
        _ => String::new(),
    };
    output.emit(&value, |_| {
        format!(
            "{}\n{}\nhost {}\nendpoint {}\nphones {}\nopen codes {}{tailnet_line}",
            if status.label.is_empty() {
                "This computer"
            } else {
                &status.label
            },
            match offline {
                None => "Online. Your phone can reach this computer.".to_owned(),
                Some(reason) => format!("Offline: {reason}"),
            },
            status.host,
            if status.endpoint.is_empty() {
                "none"
            } else {
                &status.endpoint
            },
            status.devices,
            status.outstanding_invitations,
        )
    });
    Ok(())
}

/// Why a host is offline, in words, or `None` when it is online.
fn offline_reason(endpoint: &str, online: bool) -> Option<&'static str> {
    if online {
        None
    } else if endpoint.is_empty() {
        Some(
            "this host has no phone endpoint; it was started without --iroh. \
             Start it with `openagents host serve --iroh` (the OpenAgents app's host does this for you).",
        )
    } else {
        Some(
            "the phone endpoint can't reach its relay or a direct address yet. \
             Check this computer's network; it keeps retrying on its own.",
        )
    }
}

async fn owner(output: &Output, socket: &Path, args: &[String]) -> Result<(), Failure> {
    if args != ["import"] {
        return Err(Failure::Usage("owner takes `import`".into()));
    }
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|_| failed("cannot read the key from stdin"))?;
    let secret = secret_hex(line.trim())?;
    let mut host = Host::open(socket).await?;
    let Reply::Owner { owner } = host.call(Op::OwnerImport { secret }).await? else {
        return Err(failed("the host answered another question"));
    };
    output.emit(&json!({ "owner": owner }), |_| {
        "This computer now uses your owner key. It starts again to serve under it.".into()
    });
    Ok(())
}

/// A device key as 64 lowercase hex characters, from hex or an npub.
fn device_key(text: &str) -> Result<String, Failure> {
    let hex = if text.starts_with("npub1") {
        nostr::nip19::decode_npub(text)
            .map_err(|_| Failure::Usage("DEVICE is not a valid npub".into()))?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        text.to_owned()
    };
    coder_reach::parse_pubkey(&hex)
        .map_err(|_| Failure::Usage("DEVICE is not a public key".into()))?;
    Ok(hex)
}

/// A secret key as 64 lowercase hex characters, from hex or an nsec. The
/// key is never echoed, even in an error.
fn secret_hex(text: &str) -> Result<String, Failure> {
    let bytes: Vec<u8> = if text.starts_with("nsec1") {
        nostr::nip19::decode_nsec(text)
            .map_err(|_| failed("that is not an nsec secret key"))?
            .to_vec()
    } else {
        let valid = text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit());
        if !valid {
            return Err(failed("that is not a 64-character hex secret key"));
        }
        return Ok(text.to_ascii_lowercase());
    };
    if bytes.len() != 32 {
        return Err(failed("that is not a secret key"));
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn offline_status_says_why() {
        assert_eq!(super::offline_reason("abc", true), None);
        assert!(super::offline_reason("", false).is_some_and(|r| r.contains("--iroh")));
        assert!(super::offline_reason("abc", false).is_some_and(|r| r.contains("relay")));
    }

    use super::*;

    #[test]
    fn keys_parse_from_hex_or_bech32_and_never_echo() {
        let hex = "ab".repeat(32);
        assert_eq!(device_key(&hex).ok(), Some(hex.clone()));
        assert!(device_key("nope").is_err());
        assert_eq!(secret_hex(&"CD".repeat(32)).ok(), Some("cd".repeat(32)));
        let Err(Failure::Failed(message)) = secret_hex("nsec1notakey") else {
            panic!("refused")
        };
        assert!(!message.contains("notakey"));
    }

    #[test]
    fn socket_option_is_taken_before_the_command() {
        let mut args: Vec<String> = ["--socket", "/tmp/x.sock", "status"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            take_value(&mut args, "--socket").unwrap().as_deref(),
            Some("/tmp/x.sock")
        );
        assert_eq!(args, ["status"]);
        assert!(take_value(&mut vec!["--socket".into()], "--socket").is_err());
    }
}

#[cfg(test)]
mod device_words_tests {
    use super::{ago, rights_words, short_key};

    #[test]
    fn devices_read_in_plain_words() {
        assert_eq!(
            short_key(&"a".repeat(64)),
            format!("{}…{}", "a".repeat(8), "a".repeat(4))
        );
        assert_eq!(
            rights_words(&["access_read".into(), "access_admin".into()]),
            "read chats, manage access"
        );
        assert_eq!(rights_words(&[]), "none");
        assert_eq!(ago(1000, 990), "just now");
        assert_eq!(ago(10_000, 10_000 - 7200), "2 h ago");
        assert_eq!(ago(1_000_000, 1_000_000 - 3 * 86_400), "3 days ago");
    }
}
