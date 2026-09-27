//! What Tailscale says about this machine, and its certificate.
//!
//! Tailscale only introduces machines to each other: a tailnet address is a
//! route, never a grant. The host still proves its key and checks each
//! device's grant on every channel (NIP-REACH).

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::{Error, Result};

/// Where the macOS app keeps its command-line interface when `tailscale` is
/// not on `PATH`.
const MACOS_APP: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

/// This machine on the tailnet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// The machine's tailnet IPv4 address.
    pub ip: Ipv4Addr,
    /// The MagicDNS name without the trailing dot, such as
    /// `box.example.ts.net`, when the tailnet has one.
    pub dns_name: Option<String>,
    /// Whether the tailnet issues certificates for that name.
    pub certificates: bool,
}

impl Node {
    /// The first label of the MagicDNS name, a default display label.
    #[must_use]
    pub fn short_name(&self) -> Option<&str> {
        self.dns_name
            .as_deref()
            .and_then(|name| name.split('.').next())
            .filter(|label| !label.is_empty())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Status {
    #[serde(rename = "Self")]
    me: Option<SelfStatus>,
    #[serde(default)]
    cert_domains: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SelfStatus {
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<String>,
    #[serde(default)]
    online: Option<bool>,
}

/// Parse `tailscale status --json`.
///
/// # Errors
/// Refuses output without this machine, or without an IPv4 tailnet address.
pub fn parse_status(json: &[u8]) -> Result<Node> {
    let status: Status = serde_json::from_slice(json)
        .map_err(|_| Error::new("tailscale status is not the expected JSON"))?;
    let me = status
        .me
        .ok_or_else(|| Error::new("tailscale status does not describe this machine"))?;
    if me.online == Some(false) {
        return Err(Error::new("this machine is offline in Tailscale"));
    }
    let ip = me
        .tailscale_ips
        .iter()
        .find_map(|ip| ip.parse::<Ipv4Addr>().ok())
        .ok_or_else(|| Error::new("this machine has no tailnet IPv4 address"))?;
    let dns_name = Some(me.dns_name.trim_end_matches('.').to_owned()).filter(|n| !n.is_empty());
    let certificates = match (&dns_name, &status.cert_domains) {
        (Some(name), Some(domains)) => domains.iter().any(|domain| domain == name),
        _ => false,
    };
    Ok(Node {
        ip,
        dns_name,
        certificates,
    })
}

/// The `tailscale` command: `given`, else `tailscale` on `PATH`, else the
/// macOS app's.
#[must_use]
pub fn program(given: Option<&Path>) -> PathBuf {
    if let Some(path) = given {
        return path.to_path_buf();
    }
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

/// Ask Tailscale about this machine.
///
/// # Errors
/// Reports a missing or failing `tailscale` command.
pub fn node(program: &Path) -> Result<Node> {
    let output = Command::new(program)
        .args(["status", "--json"])
        .output()
        .map_err(|_| {
            Error::new(format!(
                "cannot run {}; install Tailscale or pass --tailscale PATH",
                program.display()
            ))
        })?;
    if !output.status.success() {
        return Err(Error::new(
            "tailscale status failed; is Tailscale running and signed in?",
        ));
    }
    parse_status(&output.stdout)
}

/// Get or renew this machine's certificate with `tailscale cert`, writing
/// the chain and key to `cert` and `key`, and make the key `0600`.
///
/// # Errors
/// Reports a refusal, such as a tailnet without HTTPS certificates or a
/// user who may not fetch them. The caller falls back to plain `ws`.
pub fn cert(program: &Path, name: &str, cert: &Path, key: &Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if let Some(parent) = cert.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|_| Error::new("cannot create the certificate directory"))?;
    }
    let output = Command::new(program)
        .arg("cert")
        .arg("--cert-file")
        .arg(cert)
        .arg("--key-file")
        .arg(key)
        .arg(name)
        .output()
        .map_err(|_| Error::new("cannot run tailscale cert"))?;
    if !output.status.success() {
        let why = String::from_utf8_lossy(&output.stderr);
        let why = why.lines().next().unwrap_or("").trim();
        return Err(Error::new(format!("tailscale cert failed: {why}")));
    }
    std::fs::set_permissions(key, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| Error::new("cannot make the certificate key private"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_names_this_machine_and_its_certificate_domain() {
        let json = br#"{"Self":{"DNSName":"box.tail0.ts.net.","HostName":"Box",
            "TailscaleIPs":["100.101.102.103","fd7a:115c:a1e0::1"],"Online":true},
            "CertDomains":["box.tail0.ts.net"],"Peer":{}}"#;
        let node = parse_status(json).unwrap();
        assert_eq!(node.ip, Ipv4Addr::new(100, 101, 102, 103));
        assert_eq!(node.dns_name.as_deref(), Some("box.tail0.ts.net"));
        assert!(node.certificates);
        assert_eq!(node.short_name(), Some("box"));
    }

    #[test]
    fn a_tailnet_without_certificates_or_an_offline_machine() {
        let json = br#"{"Self":{"DNSName":"box.tail0.ts.net.","TailscaleIPs":["100.1.2.3"]}}"#;
        assert!(!parse_status(json).unwrap().certificates);
        let offline = br#"{"Self":{"DNSName":"","TailscaleIPs":["100.1.2.3"],"Online":false}}"#;
        assert!(parse_status(offline).is_err());
        let no_ipv4 = br#"{"Self":{"DNSName":"a.b.","TailscaleIPs":["fd7a::1"]}}"#;
        assert!(parse_status(no_ipv4).is_err());
        assert!(parse_status(b"{}").is_err());
    }
}
