//! Local host administration and bounded relay serving for NIP-HOST access.
//! Commands never take a secret key as an argument and never print one.
use coder_access::{
    Access, Client, Code, Error, RelayPolicy, Result, Rights,
    client::pending_enrollments,
    host::{EnrollmentStatus, Host, Unconnected, serve, serve_once},
    protocol::{Outcome, pubkey},
    unix_time,
};
use secp256k1::SecretKey;
use std::{collections::BTreeMap, path::PathBuf, str::FromStr, time::Duration};

const DEFAULT_GRANT_SECS: u64 = 7 * 24 * 60 * 60;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("coder-access: {error}");
        std::process::exit(1);
    }
}
fn bad(message: &str) -> Error {
    Error::new(Code::Malformed, message)
}
fn usage() -> &'static str {
    "usage: coder-access [--state DIR] [--loopback-test] COMMAND
  init --owner KEY
  public-key
  invite --relay URL [--rights LIST] [--grant-secs N] [--no-qr]
  cancel --invitation ID
  request --relay URL [--rights LIST]
  approve --relay URL --host KEY --device KEY --code CODE [--rights LIST]
          [--grant-secs N] [--access FILE] [--enrollment ID] [--deny]
  list [--json]
  revoke --device KEY
  serve-once --relay URL [--wait-secs N]
LIST is standard, admin, all, or comma-separated rights."
}

struct Args {
    values: BTreeMap<String, String>,
    flags: Vec<String>,
}
impl Args {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut result = Self {
            values: BTreeMap::new(),
            flags: Vec::new(),
        };
        let mut args = args.peekable();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--loopback-test" | "--no-qr" | "--json" | "--deny" => {
                    if result.flags.contains(&flag) {
                        return Err(bad("duplicate option"));
                    }
                    result.flags.push(flag);
                }
                _ if flag.starts_with("--") => {
                    let value = args
                        .next()
                        .filter(|v| !v.starts_with("--"))
                        .ok_or_else(|| bad("option requires a value"))?;
                    if result.values.insert(flag, value).is_some() {
                        return Err(bad("duplicate option"));
                    }
                }
                _ => return Err(bad("unexpected positional argument")),
            }
        }
        Ok(result)
    }
    fn flag(&mut self, name: &str) -> bool {
        let found = self.flags.iter().position(|f| f == name);
        found.map(|i| self.flags.remove(i)).is_some()
    }
    fn value(&mut self, key: &str) -> Option<String> {
        self.values.remove(key)
    }
    fn required(&mut self, key: &str) -> Result<String> {
        self.value(key)
            .ok_or_else(|| bad(&format!("required option: {key}")))
    }
    fn number(&mut self, key: &str, default: u64) -> Result<u64> {
        self.value(key)
            .map(|v| v.parse().map_err(|_| bad("expected a whole number")))
            .unwrap_or(Ok(default))
    }
    fn rights(&mut self, default: Rights) -> Result<Rights> {
        self.value("--rights")
            .map(|v| Rights::parse_list(&v))
            .unwrap_or(Ok(default))
    }
    fn key(&mut self, name: &str) -> Result<String> {
        public_key(&self.required(name)?)
    }
    fn finish(&self) -> Result<()> {
        if self.values.is_empty() && self.flags.is_empty() {
            Ok(())
        } else {
            Err(bad("unknown option for this command"))
        }
    }
}

/// Accept a lower-case hex x-only key or an `npub`.
fn public_key(text: &str) -> Result<String> {
    let hex = if text.starts_with("npub1") {
        nostr::nip19::decode_npub(text)
            .map_err(|_| bad("invalid npub"))?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        text.to_owned()
    };
    coder_connect::protocol::public(&hex).map_err(Error::from)?;
    Ok(hex)
}
/// Read the signing key from a private, singly linked, owner-only file.
fn signing_key() -> Result<SecretKey> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let path = std::env::var_os("CODER_ACCESS_KEY_FILE")
        .ok_or_else(|| bad("set CODER_ACCESS_KEY_FILE to a private 0600 hex-key file"))?;
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|_| Error::new(Code::Unavailable, "key file is unavailable"))?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.permissions().mode() & 0o077 != 0 {
        return Err(Error::new(
            Code::Forbidden,
            "key file must be a private singly linked regular file",
        ));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|_| Error::new(Code::Unavailable, "key file is unreadable"))?;
    SecretKey::from_str(text.trim()).map_err(|_| bad("invalid secret key"))
}
fn default_state() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".openagents/coder-access"))
        .ok_or_else(|| Error::new(Code::Unavailable, "HOME is not set; pass --state"))
}

async fn run() -> Result<()> {
    let mut raw = std::env::args().skip(1).collect::<Vec<_>>();
    let mut state = None;
    let mut loopback = false;
    // Global options come before the command.
    loop {
        match raw.first().map(String::as_str) {
            Some("--state") if raw.len() > 1 => {
                state = Some(PathBuf::from(raw.remove(1)));
                raw.remove(0);
            }
            Some("--loopback-test") => {
                loopback = true;
                raw.remove(0);
            }
            _ => break,
        }
    }
    let Some(command) = (!raw.is_empty()).then(|| raw.remove(0)) else {
        println!("{}", usage());
        return Ok(());
    };
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    let mut args = Args::parse(raw.into_iter())?;
    let policy = if loopback {
        RelayPolicy::LoopbackTest
    } else {
        RelayPolicy::Production
    };
    let host = Host::new(
        match state {
            Some(state) => state,
            None => default_state()?,
        },
        policy,
    );
    let now = unix_time()?;
    match command.as_str() {
        "init" => {
            let owner = args.key("--owner")?;
            args.finish()?;
            println!("host {}", host.init(&owner)?);
            println!("owner {owner}");
        }
        "public-key" => {
            args.finish()?;
            println!("{}", host.public_key()?);
        }
        "invite" => {
            let relay = args.required("--relay")?;
            let rights = args.rights(Rights::standard())?;
            let grant_secs = args.number("--grant-secs", DEFAULT_GRANT_SECS)?;
            let no_qr = args.flag("--no-qr");
            args.finish()?;
            let issued =
                host.invite(&relay, rights.clone(), now, now.saturating_add(grant_secs))?;
            eprintln!(
                "Show this invitation only to the device you are enrolling. It admits one device \
                 with {} and expires at {}.",
                rights.to_list(),
                issued.expires_at
            );
            println!("invitation {}", issued.id);
            println!("{}", issued.code);
            if !no_qr {
                let qr = coder_connect::pairing::terminal_qr_prefixed(
                    coder_access::protocol::INVITATION_PREFIX,
                    &issued.code,
                )?;
                eprint!("{qr}");
            }
        }
        "cancel" => {
            let id = args.required("--invitation")?;
            args.finish()?;
            host.cancel_invitation(&id)?;
            println!("cancelled {id}");
        }
        "request" => {
            let relay = args.required("--relay")?;
            let rights = args.rights(Rights::standard())?;
            args.finish()?;
            let pending = host.request_enrollment(&relay, rights, now)?;
            host.publish(&relay, &pending.events).await?;
            println!("enrollment {}", pending.id);
            println!("code {}", pending.code);
            eprintln!(
                "Approve this request from the owner key or a device with access_admin, using the code. \
                 It expires at {}.",
                pending.expires_at
            );
            let wait = pending.expires_at.saturating_sub(unix_time()?);
            let id = pending.id.clone();
            serve(
                &host,
                &relay,
                &mut Unconnected,
                Duration::from_secs(wait),
                |_| {
                    !matches!(
                        host.enrollment_status(&id, unix_time().unwrap_or(u64::MAX)),
                        Ok(EnrollmentStatus::Pending)
                    )
                },
            )
            .await?;
            match host.enrollment_status(&id, unix_time()?)? {
                EnrollmentStatus::Approved { device, grant } => {
                    println!("approved device {device} grant {grant}");
                }
                other => return Err(Error::new(Code::Denied, format!("enrollment {other:?}"))),
            }
        }
        "approve" => {
            let relay = args.required("--relay")?;
            let target = args.key("--host")?;
            let device = args.key("--device")?;
            let code = args.value("--code");
            let deny = args.flag("--deny");
            let grant_secs = args.number("--grant-secs", DEFAULT_GRANT_SECS)?;
            let access = args.value("--access");
            let chosen = args.value("--enrollment");
            let rights = args.value("--rights");
            args.finish()?;
            let secret = signing_key()?;
            let client = match access {
                Some(path) => {
                    let bytes = std::fs::read(path)
                        .map_err(|_| Error::new(Code::Unavailable, "access file is unreadable"))?;
                    Client::device(Access::parse(&bytes)?, secret, policy)?
                }
                None => Client::owner(&target, &relay, secret, policy)?,
            };
            let mut found = pending_enrollments(&relay, &secret, &target, policy).await?;
            if let Some(id) = chosen {
                found.retain(|e| e.enrollment.enrollment == id);
            }
            let [enrollment] = found.as_slice() else {
                return Err(bad(
                    "expected exactly one current enrollment; pass --enrollment ID",
                ));
            };
            let op = if deny {
                enrollment.deny()
            } else {
                let rights = match rights {
                    Some(list) => Rights::parse_list(&list)?,
                    None => enrollment.enrollment.rights.clone(),
                };
                let code = code.ok_or_else(|| bad("required option: --code"))?;
                enrollment.approve(&code, &device, rights, now.saturating_add(grant_secs))
            };
            match client.call(op).await? {
                Outcome::Granted { authorization } => {
                    eprintln!("Approved. Give this grant envelope to device {device}.");
                    println!(
                        "{}",
                        serde_json::to_string(&authorization).map_err(|_| bad("grant encoding"))?
                    );
                }
                Outcome::Denied {} => println!("denied {}", enrollment.enrollment.enrollment),
                _ => return Err(bad("unexpected host answer")),
            }
            eprintln!("approver {}", pubkey(&secret));
        }
        "list" => {
            let json = args.flag("--json");
            args.finish()?;
            let devices = host.devices(now)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&devices).map_err(|_| bad("device encoding"))?
                );
            } else {
                for d in devices {
                    println!(
                        "{} grant {} {:?} rights {} epoch {} expires {}",
                        d.device,
                        d.grant,
                        d.state,
                        d.rights.to_list(),
                        d.epoch,
                        d.expires_at
                    );
                }
            }
        }
        "revoke" => {
            let device = args.key("--device")?;
            args.finish()?;
            let (epoch, grants) = host.revoke(&device, now)?;
            println!("revoked {device} epoch {epoch} grants {}", grants.join(","));
        }
        "serve-once" => {
            let relay = args.required("--relay")?;
            let wait = args.number("--wait-secs", 60)?;
            args.finish()?;
            println!("serving {} on {relay}", host.public_key()?);
            let reply =
                serve_once(&host, &relay, &mut Unconnected, Duration::from_secs(wait)).await?;
            println!("answered {}", reply.id);
        }
        _ => return Err(bad(usage())),
    }
    Ok(())
}
