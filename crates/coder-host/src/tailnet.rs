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
/// The history roots a chat invitation admits: `~/.codex` and `~/.claude`
/// when they exist.
#[must_use]
pub fn default_sources() -> coder_history::Config {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let root = |folder: &str| home.as_ref().map(|h| h.join(folder)).filter(|p| p.is_dir());
    coder_history::Config {
        codex: root(".codex"),
        claude: root(".claude"),
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
    let me = me(&settings.tailscale).await?;
    let listener = TcpListener::bind(SocketAddr::from((me.ip, settings.port)))
        .await
        .map_err(|_| Error::Config("the tailnet admission listener cannot bind".into()))?;
    let address = listener
        .local_addr()
        .map_err(|_| Error::Config("the admission listener has no address".into()))?;
    if let Some(chats) = &settings.chats {
        coder_connect::host::ensure_parent(&chats.observer)
            .map_err(|_| Error::Config("the chat history store cannot be created".into()))?;
        let observer = coder_connect::host::Host::new(&chats.observer, settings.policy);
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
    let shared = Arc::new((settings, me, host_key));
    let permits = Arc::new(Semaphore::new(CONCURRENT));
    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                continue;
            };
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                continue;
            };
            let shared = shared.clone();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(
                    EXCHANGE_LIMIT,
                    exchange(stream, peer, &shared.0, &shared.1, &shared.2),
                )
                .await;
                drop(permit);
            });
        }
    });
    Ok(address)
}

#[cfg(feature = "host")]
async fn exchange(
    stream: TcpStream,
    peer: SocketAddr,
    settings: &Settings,
    me: &Me,
    host: &str,
) -> std::io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut line = Vec::new();
    let mut reader = BufReader::new(read.take(MAX_REQUEST_BYTES as u64 + 1));
    reader.read_until(b'\n', &mut line).await?;
    let reply = answer(&line, peer.ip(), settings, me, host).await;
    let mut bytes = serde_json::to_vec(&reply).unwrap_or_default();
    bytes.push(b'\n');
    write.write_all(&bytes).await?;
    write.shutdown().await
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
    if owner.tagged {
        return refuse(reply, "tagged");
    }
    if owner.user != me.user {
        return refuse(reply, "not_owner");
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
