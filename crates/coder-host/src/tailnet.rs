//! Tailnet admission: an opt-in listener on this machine's tailnet address
//! that hands a device of the operator's own Tailscale account a fresh host
//! invitation, and a chat invitation, without a QR code.
//!
//! Tailscale identifies the caller; the host still decides. The listener asks
//! the local `tailscale whois` who owns the connecting address and answers
//! only a device of the same Tailscale user as this machine, never a tagged
//! device. It then issues an ordinary single-use NIP-HOST invitation with the
//! rights the operator chose when enabling admission. The device redeems it
//! over the relay like any other invitation, so the host signs every grant
//! and revocation, epochs, and device listing are unchanged. See NIP-HOST's
//! tailnet admission section.
//!
//! The wire format is one line of JSON each way over TCP:
//! `openagents.host-tailnet-admission-request.v1` in, then
//! `openagents.host-tailnet-admission.v1` out.
//!
//! Deprecated (issue #9978): nearby pairing with a confirmation code replaces
//! it, and it is removed one release after that ships. See
//! `docs/coder/design/2026-09-29-auto-pairing.md`.

#[cfg(feature = "host")]
use std::net::Ipv4Addr;
use std::net::{IpAddr, SocketAddr};
#[cfg(feature = "host")]
use std::path::{Path, PathBuf};
#[cfg(feature = "host")]
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "host")]
use coder_access::{RelayPolicy, Rights};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
#[cfg(feature = "host")]
use tokio::net::TcpListener;
use tokio::net::TcpStream;
#[cfg(feature = "host")]
use tokio::sync::Semaphore;

#[cfg(feature = "host")]
use crate::{Error, Result};

/// Printed when the admission listener starts.
pub const DEPRECATED: &str = "coder host: tailnet admission (--tailnet-admission) is deprecated and will be removed one release after nearby pairing ships. Connect a phone with the OpenAgents desktop app's QR code instead. See docs/coder/guides/link-devices.md.";

/// The default admission port on the tailnet address.
pub const PORT: u16 = 47109;
pub const REQUEST: &str = "openagents.host-tailnet-admission-request.v1";
pub const REPLY: &str = "openagents.host-tailnet-admission.v1";
/// The largest request line.
pub const MAX_REQUEST_BYTES: usize = 1024;
#[cfg(feature = "host")]
/// How long one exchange may take, including the `whois` lookup.
const EXCHANGE_LIMIT: Duration = Duration::from_secs(10);
#[cfg(feature = "host")]
const CONCURRENT: usize = 8;
#[cfg(feature = "host")]
/// Chat grants last this long; the device asks again when one expires.
const CHAT_GRANT_SECS: u64 = 29 * 24 * 60 * 60;

#[cfg(feature = "host")]
/// Where the macOS app keeps its command-line interface when `tailscale` is
/// not on `PATH`.
const MACOS_APP: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

/// A device's request. `chats` asks for a chat invitation too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionRequest {
    pub v: String,
    pub requires: Vec<String>,
    pub chats: bool,
}

impl AdmissionRequest {
    #[must_use]
    pub fn new(chats: bool) -> Self {
        Self {
            v: REQUEST.into(),
            requires: vec![],
            chats,
        }
    }
}

/// The host's answer: invitations, or a refusal code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub v: String,
    /// The host key the invitation names.
    pub host: String,
    /// This machine's tailnet name, for display.
    pub label: String,
    /// A single-use `coder-host:` invitation.
    pub invitation: Option<String>,
    /// A single-use `coder-pair:` chat invitation, when chats are served.
    pub chats: Option<String>,
    /// `not_tailnet`, `not_owner`, `tagged`, `malformed`, or `unavailable`.
    pub refused: Option<String>,
}

#[cfg(feature = "host")]
/// Read-only chat history the host serves to admitted devices.
#[derive(Clone, Debug)]
pub struct Chats {
    /// The `coder-connect` observer store.
    pub observer: PathBuf,
    pub sources: coder_history::Config,
}

#[cfg(feature = "host")]
#[derive(Clone, Debug)]
pub struct Settings {
    /// The host's access store.
    pub state: PathBuf,
    pub policy: RelayPolicy,
    /// The relay invitations name: the host's primary relay.
    pub relay: String,
    /// The rights every admitted device's invitation carries.
    pub rights: Rights,
    pub grant_secs: u64,
    pub port: u16,
    pub tailscale: PathBuf,
    pub chats: Option<Chats>,
}

#[cfg(feature = "host")]
/// This machine on the tailnet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Me {
    pub ip: Ipv4Addr,
    pub user: u64,
    pub label: String,
}

#[cfg(feature = "host")]
/// Who owns a tailnet address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub user: u64,
    pub tagged: bool,
    pub name: String,
}

#[cfg(feature = "host")]
/// The `tailscale` command: `tailscale` on `PATH`, else the macOS app's.
#[must_use]
pub fn program() -> PathBuf {
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("tailscale"))
            .find(|candidate| candidate.is_file())
    });
    on_path.unwrap_or_else(|| {
        if Path::new(MACOS_APP).is_file() {
            PathBuf::from(MACOS_APP)
        } else {
            PathBuf::from("tailscale")
        }
    })
}

#[cfg(feature = "host")]
/// The history root a chat invitation admits: Coder's task directory,
/// `~/.openagents/tasks`, when it exists. The phone shows only Coder chats
/// ([#9920](https://github.com/OpenAgentsInc/openagents/issues/9920)), so
/// the host offers neither `~/.codex` nor `~/.claude` and keeps no copy of
/// OpenCode's or Devin's session stores; a session Coder delegates to one
/// of them is kept beside its task ([`coder_history::delegate`]).
#[must_use]
pub fn default_sources() -> coder_history::Config {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    coder_history::Config {
        coder: home
            .map(|home| home.join(".openagents/tasks"))
            .filter(|path| path.is_dir()),
        ..coder_history::Config::default()
    }
}

#[cfg(feature = "host")]
async fn tailscale_json(program: &Path, args: &[&str]) -> Result<serde_json::Value> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new(program)
            .args(args)
            // The macOS app's binary acts as the command-line tool only when
            // SHLVL is set, as a shell sets it; under launchd it would start
            // the app instead and never answer.
            .env("SHLVL", "1")
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| Error::Config("tailscale did not answer".into()))?
    .map_err(|_| Error::Config(format!("cannot run {}", program.display())))?;
    if !output.status.success() {
        return Err(Error::Config(
            "tailscale failed; is it running and signed in?".into(),
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| Error::Config("tailscale answered with unexpected JSON".into()))
}

#[cfg(feature = "host")]
/// Parse `tailscale status --json` for this machine.
///
/// # Errors
/// Refuses output without this machine's user or IPv4 address.
pub fn parse_me(status: &serde_json::Value) -> Result<Me> {
    let me = &status["Self"];
    let user = me["UserID"]
        .as_u64()
        .ok_or_else(|| Error::Config("tailscale status names no user".into()))?;
    let ip = me["TailscaleIPs"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|ip| ip.as_str()?.parse::<Ipv4Addr>().ok())
        .ok_or_else(|| Error::Config("this machine has no tailnet IPv4 address".into()))?;
    let label = me["DNSName"]
        .as_str()
        .and_then(|name| name.split('.').next())
        .filter(|label| !label.is_empty())
        .or_else(|| me["HostName"].as_str())
        .unwrap_or("computer")
        .to_owned();
    Ok(Me { ip, user, label })
}

#[cfg(feature = "host")]
/// Parse `tailscale whois --json ADDRESS`.
///
/// # Errors
/// Refuses output without a user.
pub fn parse_whois(whois: &serde_json::Value) -> Result<Owner> {
    let user = whois["UserProfile"]["ID"]
        .as_u64()
        .ok_or_else(|| Error::Config("whois names no user".into()))?;
    let tagged = whois["Node"]["Tags"]
        .as_array()
        .is_some_and(|tags| !tags.is_empty());
    let name = whois["Node"]["Name"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_owned();
    Ok(Owner { user, tagged, name })
}

#[cfg(feature = "host")]
/// Ask Tailscale about this machine.
///
/// # Errors
/// Reports a missing, failing, or signed-out `tailscale`.
pub async fn me(program: &Path) -> Result<Me> {
    parse_me(&tailscale_json(program, &["status", "--json"]).await?)
}

#[cfg(feature = "host")]
async fn whois(program: &Path, ip: IpAddr) -> Result<Owner> {
    parse_whois(&tailscale_json(program, &["whois", "--json", &ip.to_string()]).await?)
}

/// Tailscale's address ranges: `100.64.0.0/10` and `fd7a:115c:a1e0::/48`.
#[must_use]
pub fn is_tailnet(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            a == 100 && (64..128).contains(&b)
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0
        }
    }
}

#[cfg(feature = "host")]
/// Bind the admission listener on this machine's tailnet address and serve
/// it until the process stops. With chats, also serve read-only history.
///
/// # Errors
/// Reports a missing Tailscale, or a listener that cannot bind.
pub async fn start(settings: Settings) -> Result<SocketAddr> {
    eprintln!("{DEPRECATED}");
    let me = me(&settings.tailscale).await?;
    let listener = TcpListener::bind(listen_address(&me, settings.port)?)
        .await
        .map_err(|_| Error::Config("the tailnet admission listener cannot bind".into()))?;
    let address = listener
        .local_addr()
        .map_err(|_| Error::Config("the admission listener has no address".into()))?;
    if let Some(chats) = &settings.chats {
        coder_connect::host::ensure_parent(&chats.observer)
            .map_err(|_| Error::Config("the chat history store cannot be created".into()))?;
        let observer =
            coder_connect::host::Host::new(&chats.observer, settings.policy).coder_only();
        // Read the chat list once now, so the first device to ask finds each
        // session's head already read.
        let sources = chats.sources.clone();
        let index = coder_connect::host::catalog_index(&chats.observer);
        tokio::task::spawn_blocking(move || {
            if let Ok(history) = coder_history::History::open(sources) {
                let history = history.with_catalog_index(index);
                let _ = history.catalog(coder_history::CatalogRequest {
                    cursor: None,
                    limit: coder_history::MAX_CATALOG_PAGE,
                });
            }
        });
        let (relay, policy) = (settings.relay.clone(), settings.policy);
        tokio::spawn(async move {
            if coder_connect::cli::serve_observer(observer, relay, policy)
                .await
                .is_err()
            {
                eprintln!("coder host: chat history stopped");
            }
        });
    }
    let host_key = coder_access::host::Host::new(&settings.state, settings.policy).public_key()?;
    let observer = settings.chats.as_ref().map(|chats| {
        Arc::new(coder_connect::host::Host::new(&chats.observer, settings.policy).coder_only())
    });
    let shared = Arc::new(Shared {
        settings,
        me,
        host: host_key,
        observer,
        owners: std::sync::Mutex::default(),
    });
    let permits = Arc::new(Semaphore::new(CONCURRENT));
    let direct = Arc::new(Semaphore::new(DIRECT_CONNECTIONS));
    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                continue;
            };
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                continue;
            };
            let (shared, direct) = (shared.clone(), direct.clone());
            tokio::spawn(async move {
                let _ = stream.set_nodelay(true);
                let (read, mut write) = stream.into_split();
                let mut reader = BufReader::new(read);
                let Ok(Ok(line)) =
                    tokio::time::timeout(EXCHANGE_LIMIT, first_line(&mut reader)).await
                else {
                    return;
                };
                if coder_connect::direct::Hello::parse(&line).is_some() {
                    // A direct chat connection outlives an admission
                    // exchange; it holds a permit of its own instead.
                    drop(permit);
                    observe(reader, write, peer, &shared, &direct).await;
                    return;
                }
                let _ = tokio::time::timeout(EXCHANGE_LIMIT, async {
                    let reply =
                        answer(&line, peer.ip(), &shared.settings, &shared.me, &shared.host).await;
                    let mut bytes = serde_json::to_vec(&reply).unwrap_or_default();
                    bytes.push(b'\n');
                    write.write_all(&bytes).await?;
                    write.shutdown().await
                })
                .await;
                drop(permit);
            });
        }
    });
    Ok(address)
}

#[cfg(feature = "host")]
/// Where the listener binds: this machine's own tailnet address, never a
/// wildcard or LAN address, since admission and direct chat reads are for
/// tailnet callers only.
///
/// # Errors
/// Refuses an address outside Tailscale's ranges.
pub fn listen_address(me: &Me, port: u16) -> Result<SocketAddr> {
    let ip = IpAddr::V4(me.ip);
    if !is_tailnet(ip) || me.ip.is_unspecified() {
        return Err(Error::Config(
            "the tailnet listener binds only a tailnet address".into(),
        ));
    }
    Ok(SocketAddr::new(ip, port))
}

#[cfg(feature = "host")]
/// Direct chat connections served at once.
const DIRECT_CONNECTIONS: usize = 16;
#[cfg(feature = "host")]
/// How long a caller's Tailscale owner is remembered after `whois` names it.
const OWNER_FOR: Duration = Duration::from_secs(10 * 60);

#[cfg(feature = "host")]
struct Shared {
    settings: Settings,
    me: Me,
    host: String,
    observer: Option<Arc<coder_connect::host::Host>>,
    /// Recent `whois` answers by caller address.
    owners: std::sync::Mutex<std::collections::HashMap<IpAddr, (Owner, std::time::Instant)>>,
}

#[cfg(feature = "host")]
/// The first line, up to [`MAX_REQUEST_BYTES`] and one more byte: through
/// its newline, or all that came before the caller stopped writing.
async fn first_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf().await?;
        if buffer.is_empty() {
            return Ok(line);
        }
        let room = MAX_REQUEST_BYTES + 1 - line.len();
        let (count, done) = match buffer.iter().take(room).position(|b| *b == b'\n') {
            Some(index) => (index + 1, true),
            None => (buffer.len().min(room), false),
        };
        line.extend_from_slice(&buffer[..count]);
        reader.consume(count);
        if done || line.len() > MAX_REQUEST_BYTES {
            return Ok(line);
        }
    }
}

#[cfg(feature = "host")]
/// Welcome a direct chat connection from this machine's own untagged
/// Tailscale user, the callers admission answers, and serve it. The listener
/// is bound to the tailnet address, and the caller's sealed requests carry
/// their own authority; Tailscale identity only admits the connection.
async fn observe(
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    mut write: tokio::net::tcp::OwnedWriteHalf,
    peer: SocketAddr,
    shared: &Shared,
    direct: &Arc<Semaphore>,
) {
    let permit = direct.clone().try_acquire_owned();
    let refused = match (&shared.observer, &permit) {
        (None, _) => Some("not_serving".to_string()),
        (_, Err(_)) => Some("unavailable".to_string()),
        _ => tokio::time::timeout(EXCHANGE_LIMIT, owner_refusal(peer.ip(), shared))
            .await
            .unwrap_or_else(|_| Some("unavailable".into())),
    };
    let welcome = coder_connect::direct::Welcome {
        v: coder_connect::direct::HELLO.into(),
        refused: refused.clone(),
    };
    let mut bytes = serde_json::to_vec(&welcome).unwrap_or_default();
    bytes.push(b'\n');
    if write.write_all(&bytes).await.is_err() {
        return;
    }
    match (refused, &shared.observer) {
        (None, Some(observer)) => {
            coder_connect::direct::serve(reader, write, observer.clone()).await;
        }
        _ => {
            let _ = write.shutdown().await;
        }
    }
    drop(permit);
}

#[cfg(feature = "host")]
/// Why the caller at `ip` is not this machine's own untagged Tailscale
/// user, or `None` when it is.
async fn owner_refusal(ip: IpAddr, shared: &Shared) -> Option<String> {
    if !is_tailnet(ip) {
        return Some("not_tailnet".into());
    }
    let known = shared
        .owners
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(&ip)
        .filter(|(_, at)| at.elapsed() < OWNER_FOR)
        .map(|(owner, _)| owner.clone());
    let owner = match known {
        Some(owner) => owner,
        None => match whois(&shared.settings.tailscale, ip).await {
            Ok(owner) => {
                shared
                    .owners
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .insert(ip, (owner.clone(), std::time::Instant::now()));
                owner
            }
            Err(_) => return Some("unavailable".into()),
        },
    };
    refusal(&owner, &shared.me)
}

#[cfg(feature = "host")]
/// Whether `owner` may be answered by this machine, `me`.
fn refusal(owner: &Owner, me: &Me) -> Option<String> {
    if owner.tagged {
        Some("tagged".into())
    } else if owner.user != me.user {
        Some("not_owner".into())
    } else {
        None
    }
}

#[cfg(feature = "host")]
async fn answer(line: &[u8], peer: IpAddr, settings: &Settings, me: &Me, host: &str) -> Admission {
    let mut reply = Admission {
        v: REPLY.into(),
        host: host.into(),
        label: me.label.clone(),
        invitation: None,
        chats: None,
        refused: None,
    };
    let refuse = |mut reply: Admission, code: &str| {
        reply.refused = Some(code.into());
        reply
    };
    let request = match serde_json::from_slice::<AdmissionRequest>(line) {
        Ok(request)
            if line.len() <= MAX_REQUEST_BYTES
                && request.v == REQUEST
                && request.requires.is_empty() =>
        {
            request
        }
        _ => return refuse(reply, "malformed"),
    };
    if !is_tailnet(peer) {
        return refuse(reply, "not_tailnet");
    }
    let owner = match whois(&settings.tailscale, peer).await {
        Ok(owner) => owner,
        Err(_) => return refuse(reply, "unavailable"),
    };
    if let Some(code) = refusal(&owner, me) {
        return refuse(reply, &code);
    }
    let Ok(now) = crate::unix_time() else {
        return refuse(reply, "unavailable");
    };
    let access = coder_access::host::Host::new(&settings.state, settings.policy);
    let invitation = retry_busy(|| {
        access.invite(
            &settings.relay,
            settings.rights.clone(),
            now,
            now.saturating_add(settings.grant_secs),
        )
    });
    match invitation {
        Ok(issued) => reply.invitation = Some(issued.code),
        Err(_) => return refuse(reply, "unavailable"),
    }
    if request.chats
        && let Some(chats) = &settings.chats
    {
        let observer = coder_connect::host::Host::new(&chats.observer, settings.policy);
        reply.chats = observer
            .invite(
                &settings.relay,
                chats.sources.clone(),
                now,
                now.saturating_add(CHAT_GRANT_SECS),
            )
            .ok();
    }
    eprintln!("coder host: tailnet admission for {}", owner.name);
    reply
}

#[cfg(feature = "host")]
fn retry_busy<T>(
    mut operation: impl FnMut() -> coder_access::Result<T>,
) -> coder_access::Result<T> {
    let started = std::time::Instant::now();
    loop {
        match operation() {
            Err(error)
                if error.code == coder_access::Code::Conflict
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

/// Ask the host at `address` for invitations. A device runs this over its
/// tailnet connection.
///
/// # Errors
/// Reports an unreachable host or an unreadable answer; a refusal is an
/// answer with `refused` set.
pub async fn request(
    address: SocketAddr,
    chats: bool,
    limit: Duration,
) -> std::io::Result<Admission> {
    tokio::time::timeout(limit, async {
        let stream = TcpStream::connect(address).await?;
        let (read, mut write) = stream.into_split();
        let mut line =
            serde_json::to_vec(&AdmissionRequest::new(chats)).map_err(std::io::Error::other)?;
        line.push(b'\n');
        write.write_all(&line).await?;
        let mut answer = Vec::new();
        BufReader::new(read.take(64 * 1024))
            .read_until(b'\n', &mut answer)
            .await?;
        let admission: Admission =
            serde_json::from_slice(&answer).map_err(std::io::Error::other)?;
        if admission.v != REPLY {
            return Err(std::io::Error::other("unexpected admission version"));
        }
        Ok(admission)
    })
    .await
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "admission timed out"))?
}

#[cfg(all(test, feature = "host"))]
mod tests {
    use super::*;

    #[test]
    fn the_deprecation_notice_names_the_replacement() {
        assert!(DEPRECATED.contains("--tailnet-admission"));
        assert!(DEPRECATED.contains("is deprecated"));
        assert!(DEPRECATED.contains("OpenAgents desktop app"));
        assert!(DEPRECATED.contains("docs/coder/guides/link-devices.md"));
    }

    #[test]
    fn tailnet_ranges() {
        assert!(is_tailnet("100.64.0.1".parse().unwrap()));
        assert!(is_tailnet("100.127.107.31".parse().unwrap()));
        assert!(!is_tailnet("100.128.0.1".parse().unwrap()));
        assert!(!is_tailnet("192.168.1.2".parse().unwrap()));
        assert!(is_tailnet("fd7a:115c:a1e0::1".parse().unwrap()));
        assert!(!is_tailnet("fd7a:115c:a1e1::1".parse().unwrap()));
    }

    #[test]
    fn parses_status_and_whois() {
        let status = serde_json::json!({"Self": {"UserID": 42, "TailscaleIPs": ["100.64.0.9", "fd7a:115c:a1e0::9"],
            "DNSName": "box.example.ts.net.", "HostName": "Box"}});
        assert_eq!(
            parse_me(&status).unwrap(),
            Me {
                ip: "100.64.0.9".parse().unwrap(),
                user: 42,
                label: "box".into()
            }
        );
        let whois = serde_json::json!({"UserProfile": {"ID": 42}, "Node": {"Name": "phone.example.ts.net.", "Tags": null}});
        assert_eq!(
            parse_whois(&whois).unwrap(),
            Owner {
                user: 42,
                tagged: false,
                name: "phone.example.ts.net".into()
            }
        );
        let tagged = serde_json::json!({"UserProfile": {"ID": 7}, "Node": {"Name": "ci", "Tags": ["tag:ci"]}});
        assert!(parse_whois(&tagged).unwrap().tagged);
    }

    #[test]
    fn the_listener_binds_only_a_tailnet_address() {
        let me = |ip: &str| Me {
            ip: ip.parse().unwrap(),
            user: 42,
            label: "box".into(),
        };
        assert_eq!(
            listen_address(&me("100.64.0.9"), PORT).unwrap(),
            "100.64.0.9:47109".parse().unwrap()
        );
        for other in ["0.0.0.0", "192.168.1.2", "127.0.0.1", "100.128.0.1"] {
            assert!(listen_address(&me(other), PORT).is_err(), "{other}");
        }
    }

    #[tokio::test]
    async fn a_direct_connection_is_welcomed_only_for_the_hosts_own_user() {
        let me = Me {
            ip: "100.64.0.9".parse().unwrap(),
            user: 42,
            label: "box".into(),
        };
        let owner = |user, tagged| Owner {
            user,
            tagged,
            name: "phone".into(),
        };
        assert_eq!(refusal(&owner(42, false), &me), None);
        assert_eq!(refusal(&owner(42, true), &me).as_deref(), Some("tagged"));
        assert_eq!(refusal(&owner(7, false), &me).as_deref(), Some("not_owner"));
        let shared = Shared {
            settings: Settings {
                state: PathBuf::from("/nonexistent"),
                policy: RelayPolicy::Production,
                relay: "wss://relay.example".into(),
                rights: Rights::standard(),
                grant_secs: 3600,
                port: 0,
                // A whois fails: a caller it cannot name is refused.
                tailscale: PathBuf::from("/nonexistent/tailscale"),
                chats: None,
            },
            me,
            host: "hostkey".into(),
            observer: None,
            owners: std::sync::Mutex::default(),
        };
        let refused = |ip: &str| owner_refusal(ip.parse().unwrap(), &shared);
        assert_eq!(refused("192.168.1.2").await.as_deref(), Some("not_tailnet"));
        assert_eq!(refused("100.64.0.2").await.as_deref(), Some("unavailable"));
        // A remembered answer is used as given.
        shared.owners.lock().unwrap().insert(
            "100.64.0.3".parse().unwrap(),
            (owner(7, false), std::time::Instant::now()),
        );
        assert_eq!(refused("100.64.0.3").await.as_deref(), Some("not_owner"));
        shared.owners.lock().unwrap().insert(
            "100.64.0.4".parse().unwrap(),
            (owner(42, false), std::time::Instant::now()),
        );
        assert_eq!(refused("100.64.0.4").await, None);
    }

    #[tokio::test]
    async fn refuses_malformed_and_non_tailnet_callers_before_whois() {
        let settings = Settings {
            state: PathBuf::from("/nonexistent"),
            policy: RelayPolicy::Production,
            relay: "wss://relay.example".into(),
            rights: Rights::standard(),
            grant_secs: 3600,
            port: 0,
            // A whois would fail; these refusals must not reach it.
            tailscale: PathBuf::from("/nonexistent/tailscale"),
            chats: None,
        };
        let me = Me {
            ip: "100.64.0.9".parse().unwrap(),
            user: 42,
            label: "box".into(),
        };
        let bad = answer(
            b"{}",
            "100.64.0.2".parse().unwrap(),
            &settings,
            &me,
            "hostkey",
        )
        .await;
        assert_eq!(bad.refused.as_deref(), Some("malformed"));
        let line = serde_json::to_vec(&AdmissionRequest::new(true)).unwrap();
        let lan = answer(
            &line,
            "192.168.1.2".parse().unwrap(),
            &settings,
            &me,
            "hostkey",
        )
        .await;
        assert_eq!(lan.refused.as_deref(), Some("not_tailnet"));
        assert!(lan.invitation.is_none());
        let unknown = answer(
            &line,
            "100.64.0.2".parse().unwrap(),
            &settings,
            &me,
            "hostkey",
        )
        .await;
        assert_eq!(unknown.refused.as_deref(), Some("unavailable"));
    }
}
