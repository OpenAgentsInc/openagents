//! Author-hosted paid resources sold through the central receiver (#10194).
//!
//! An author with an HTTP service registers it with the pay front instead of
//! running a receiver: a JSON [`Registration`] (resource name, upstream URL,
//! method, price, payout address) posted to [`REGISTER_PATH`] under a NIP-98
//! `Authorization: Nostr …` event signed by the author's key, whose
//! `payload` tag binds the body. The signed pair is kept as a [`Signed`]
//! line in the [`Registry`], so a registration can be checked again at any
//! time without trusting the front.
//!
//! The front sells `{base}/x/{resource}`: its own `402`, its own invoice, its
//! own replay store. After settlement it forwards the request to the
//! upstream with an [`PAID_HEADER`] header: a NIP-98 event signed by the pay
//! host key (published at [`KEY_PATH`]) over the upstream URL, the method,
//! and the forwarded body, with `resource`, `request`, `payment`, and
//! `settled_at` tags. The upstream checks it with [`verify_paid`] and the
//! published key; a payment hash it has seen before is a replay.
//!
//! [`public_ip`] is the address rule the front holds an upstream to: an
//! upstream that resolves to a loopback, private, link-local, or otherwise
//! non-public address is refused at registration and again at call time.

use std::collections::HashMap;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use base64::Engine;
use nostr::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The header a paid upstream request carries.
pub const PAID_HEADER: &str = "openagents-paid";
/// Where an author posts a signed registration (and reads one back as
/// `{REGISTER_PATH}/{resource}`).
pub const REGISTER_PATH: &str = "/v1/resources";
/// Where the pay host's header key is published.
pub const KEY_PATH: &str = "/v1/paid-key";
/// The path the front sells a resource at: `/x/{resource}`.
pub const RESOURCE_PATH: &str = "/x/{resource}";
/// The split role a hosted resource's settlement carries.
pub const ROLE: &str = "hosted_resource";
/// NIP-98 HTTP authorization, the kind both signatures here use.
pub const HTTP_AUTH_KIND: u16 = 27_235;
/// How old a paid header may be when the upstream checks it.
pub const PAID_WINDOW_SECS: u64 = 300;

/// What an author registers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub v: u32,
    /// The name under `/x/`: 1 to 64 of `a-z`, `0-9`, and `-`.
    pub resource: String,
    /// The author's service. The query of a buyer's request is appended.
    pub upstream: String,
    /// `GET` or `POST`.
    pub method: String,
    pub price_msat: u64,
    /// A Spark address, a Lightning address, or a node key.
    pub payout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

pub fn valid_resource(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Registration {
    /// The shape rules that need no network: the upstream's address is
    /// checked separately ([`public_ip`]).
    pub fn check(&self) -> Result<(), String> {
        if self.v != 1 {
            return Err("registration v must be 1".into());
        }
        if !valid_resource(&self.resource) {
            return Err("resource must be 1-64 of a-z, 0-9, and -".into());
        }
        if self.method != "GET" && self.method != "POST" {
            return Err("method must be GET or POST".into());
        }
        if self.price_msat == 0 || self.price_msat > 100_000_000_000 {
            return Err("price_msat must be positive and at most 100,000,000 sats".into());
        }
        if self.payout.trim().is_empty() || self.payout.len() > 256 {
            return Err("payout is required".into());
        }
        if self.summary.as_ref().is_some_and(|s| s.len() > 280) {
            return Err("summary is at most 280 bytes".into());
        }
        if self.upstream.len() > 2048
            || self.upstream.contains(['#', ' ', '\n', '\r'])
            || !self
                .upstream
                .split_once("://")
                .is_some_and(|(scheme, rest)| !scheme.is_empty() && !rest.is_empty())
        {
            return Err("upstream must be an absolute URL without a fragment".into());
        }
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// A NIP-98 `Nostr …` header value: `signer` over `method`, `url`, and the
/// body's SHA-256, with `extra` tags.
pub fn http_auth(
    signer: &RelaySigner,
    method: &str,
    url: &str,
    body: &[u8],
    now: u64,
    extra: Vec<Tag>,
) -> String {
    let mut tags = vec![
        Tag::new(vec!["u".into(), url.into()]),
        Tag::new(vec!["method".into(), method.into()]),
        Tag::new(vec!["payload".into(), sha256_hex(body)]),
    ];
    tags.extend(extra);
    let event = signer.sign(now, HTTP_AUTH_KIND, tags, String::new());
    let json = serde_json::to_vec(&event).unwrap_or_default();
    format!(
        "Nostr {}",
        base64::engine::general_purpose::STANDARD.encode(json)
    )
}

/// One registration as stored: the author's signed authorization event and
/// the exact body it signed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signed {
    pub event: Event,
    pub body: String,
}

impl Signed {
    /// Check the signature, the kind, the method, the body binding, and the
    /// registration's shape; returns the owner key and the registration.
    /// The NIP-98 freshness window is the HTTP layer's, not this.
    pub fn verify(&self) -> Result<(String, Registration), String> {
        let event = &self.event;
        event.validate_structure().map_err(|e| e.to_string())?;
        event.validate_crypto().map_err(|e| e.to_string())?;
        if event.kind != HTTP_AUTH_KIND {
            return Err("a registration is signed as a kind 27235 event".into());
        }
        let one = |name: &'static str| -> Result<String, String> {
            let values: Vec<&str> = event.tag_values(name).collect();
            match values.as_slice() {
                [value] => Ok((*value).to_owned()),
                _ => Err(format!("the registration needs one {name} tag")),
            }
        };
        if one("method")? != "POST" {
            return Err("a registration is a signed POST".into());
        }
        if !one("u")?.ends_with(REGISTER_PATH) {
            return Err(format!("a registration is signed for {REGISTER_PATH}"));
        }
        if one("payload")? != sha256_hex(self.body.as_bytes()) {
            return Err("the signature does not cover this body".into());
        }
        let registration: Registration =
            serde_json::from_str(&self.body).map_err(|e| format!("registration: {e}"))?;
        registration.check()?;
        Ok((event.pubkey.clone(), registration))
    }
}

/// One registered resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub owner: String,
    pub registration: Registration,
    /// The authorization event's id: which signed registration this is.
    pub id: String,
    pub registered_at: u64,
}

/// Why a registration was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub status: u16,
    pub kind: &'static str,
    pub message: String,
}

fn refused(status: u16, kind: &'static str, message: impl Into<String>) -> Refused {
    Refused {
        status,
        kind,
        message: message.into(),
    }
}

/// The registered resources: an append-only file of [`Signed`] lines, and
/// the newest registration per name in memory. A name belongs to the key
/// that registered it first; only that key can register it again.
pub struct Registry {
    path: Option<PathBuf>,
    entries: RwLock<HashMap<String, Entry>>,
    write: std::sync::Mutex<()>,
}

impl Registry {
    /// Open (or start) the file at `path`, replaying every line that still
    /// verifies.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let registry = Self {
            path: Some(path.to_path_buf()),
            entries: RwLock::new(HashMap::new()),
            write: std::sync::Mutex::new(()),
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let Ok(signed) = serde_json::from_str::<Signed>(line) else {
                continue;
            };
            if let Ok(entry) = registry.decide(&signed) {
                registry.insert(&entry);
            }
        }
        Ok(registry)
    }

    /// A registry kept in memory only.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            entries: RwLock::new(HashMap::new()),
            write: std::sync::Mutex::new(()),
        }
    }

    pub fn get(&self, resource: &str) -> Option<Entry> {
        self.entries.read().ok()?.get(resource).cloned()
    }

    pub fn len(&self) -> usize {
        self.entries.read().map_or(0, |entries| entries.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Verify `signed` and apply the ownership rule, changing nothing.
    fn decide(&self, signed: &Signed) -> Result<Entry, Refused> {
        let (owner, registration) = signed
            .verify()
            .map_err(|message| refused(400, "invalid_registration", message))?;
        if let Some(existing) = self.get(&registration.resource) {
            if existing.owner != owner {
                return Err(refused(
                    409,
                    "resource_taken",
                    format!("{} is registered by another key", registration.resource),
                ));
            }
            if existing.registered_at > signed.event.created_at {
                return Err(refused(
                    409,
                    "stale_registration",
                    "a newer registration of this resource exists",
                ));
            }
        }
        Ok(Entry {
            owner,
            registration,
            id: signed.event.id.clone(),
            registered_at: signed.event.created_at,
        })
    }

    fn insert(&self, entry: &Entry) {
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(entry.registration.resource.clone(), entry.clone());
        }
    }

    /// Admit `signed` and append it to the file (synced) before it counts.
    pub fn register(&self, signed: &Signed) -> Result<Entry, Refused> {
        let _guard = self
            .write
            .lock()
            .map_err(|_| refused(500, "registry_lock", "the registry lock is poisoned"))?;
        let entry = self.decide(signed)?;
        if let Some(path) = &self.path {
            let mut line = serde_json::to_vec(signed)
                .map_err(|e| refused(500, "registry_write", e.to_string()))?;
            line.push(b'\n');
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut file| file.write_all(&line).and_then(|()| file.sync_data()))
                .map_err(|e| refused(503, "registry_write", format!("{}: {e}", path.display())))?;
        }
        self.insert(&entry);
        Ok(entry)
    }
}

/// Whether `ip` is a public unicast address an upstream may live at.
/// Loopback, private, link-local (including cloud metadata at
/// 169.254.169.254), shared, documentation, benchmarking, multicast,
/// reserved, unspecified, unique-local, and IPv4-mapped forms of any of
/// these are not.
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public_v4(v4);
            }
            let s = v6.segments();
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local
                || (s[0] & 0xffc0) == 0xfe80 // link local
                || (s[0] & 0xffc0) == 0xfec0 // site local
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
                || (s[0] == 0x0064 && s[1] == 0xff9b) // NAT64
                || (s[0] == 0x2002) // 6to4 embeds an IPv4 address
                || (s[0] == 0x2001 && s[1] == 0) // Teredo
                || s[..6].iter().all(|x| *x == 0)) // IPv4-compatible
        }
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // shared (CGNAT)
        || (a == 192 && b == 0 && c == 0) // IETF protocol assignments
        || (a == 198 && (b == 18 || b == 19)) // benchmarking
        || a >= 240)
}

/// What a paid header says: which resource, which front request, which
/// payment, and when it settled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paid {
    pub resource: String,
    /// The front request's x402 `http:1` binding hash.
    pub request_hash: String,
    pub payment_hash: String,
    pub settled_at: u64,
    /// The header event's id, unique per forward.
    pub id: String,
}

/// The [`PAID_HEADER`] value for one forward: signed by the pay host key
/// over the upstream `method`, `url`, and forwarded `body`.
pub fn paid_header(
    host: &RelaySigner,
    paid: &Paid,
    method: &str,
    url: &str,
    body: &[u8],
    now: u64,
) -> String {
    http_auth(
        host,
        method,
        url,
        body,
        now,
        vec![
            Tag::new(vec!["resource".into(), paid.resource.clone()]),
            Tag::new(vec!["request".into(), paid.request_hash.clone()]),
            Tag::new(vec!["payment".into(), paid.payment_hash.clone()]),
            Tag::new(vec!["settled_at".into(), paid.settled_at.to_string()]),
        ],
    )
}

/// Check a [`PAID_HEADER`] value as the upstream that received it: signed
/// by `host_key` (the published pay host key), for this `method`, the URL
/// the upstream was called at, and the body it received, within
/// [`PAID_WINDOW_SECS`] of `now`. The caller keeps the payment hashes it
/// has seen and refuses a second one.
pub fn verify_paid(
    header: &str,
    host_key: &str,
    method: &str,
    url: &str,
    body: &[u8],
    now: u64,
) -> Result<Paid, String> {
    let encoded = header
        .strip_prefix("Nostr ")
        .ok_or("the paid header is not a Nostr authorization")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_| "the paid header is not base64")?;
    let event: Event =
        serde_json::from_slice(&bytes).map_err(|_| "the paid header is not an event")?;
    event.validate_structure().map_err(|e| e.to_string())?;
    event.validate_crypto().map_err(|e| e.to_string())?;
    if event.kind != HTTP_AUTH_KIND {
        return Err("the paid header has the wrong kind".into());
    }
    if event.pubkey != host_key {
        return Err("the paid header is not signed by the pay host key".into());
    }
    if event.created_at.abs_diff(now) > PAID_WINDOW_SECS {
        return Err("the paid header is too old or from the future".into());
    }
    let one = |name: &str| {
        let values: Vec<&str> = event.tag_values(name).collect();
        match values.as_slice() {
            [value] => Ok((*value).to_owned()),
            _ => Err(format!("the paid header needs one {name} tag")),
        }
    };
    if one("u")? != url {
        return Err("the paid header is for another URL".into());
    }
    if one("method")? != method {
        return Err("the paid header is for another method".into());
    }
    if one("payload")? != sha256_hex(body) {
        return Err("the paid header does not cover this body".into());
    }
    Ok(Paid {
        resource: one("resource")?,
        request_hash: one("request")?,
        payment_hash: one("payment")?,
        settled_at: one("settled_at")?
            .parse()
            .map_err(|_| "settled_at is not a number")?,
        id: event.id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    fn registration(resource: &str, price_msat: u64) -> Registration {
        Registration {
            v: 1,
            resource: resource.into(),
            upstream: "https://author.example/weather".into(),
            method: "GET".into(),
            price_msat,
            payout: "alice@getalby.com".into(),
            summary: None,
        }
    }

    fn signed(by: &RelaySigner, registration: &Registration, at: u64) -> Signed {
        let body = serde_json::to_string(registration).unwrap();
        let header = http_auth(
            by,
            "POST",
            "https://api.example.com/v1/resources",
            body.as_bytes(),
            at,
            vec![],
        );
        let json = base64::engine::general_purpose::STANDARD
            .decode(header.strip_prefix("Nostr ").unwrap())
            .unwrap();
        Signed {
            event: serde_json::from_slice(&json).unwrap(),
            body,
        }
    }

    #[test]
    fn a_name_belongs_to_its_first_key_and_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosted.ndjson");
        let alice = signer("11");
        let bob = signer("22");
        let registry = Registry::open(&path).unwrap();
        let first = registry
            .register(&signed(&alice, &registration("weather", 3_000), 100))
            .unwrap();
        assert_eq!(first.owner, alice.pubkey());
        let taken = registry
            .register(&signed(&bob, &registration("weather", 1_000), 101))
            .unwrap_err();
        assert_eq!((taken.status, taken.kind), (409, "resource_taken"));
        registry
            .register(&signed(&alice, &registration("weather", 5_000), 102))
            .unwrap();
        let stale = registry
            .register(&signed(&alice, &registration("weather", 9_000), 50))
            .unwrap_err();
        assert_eq!(stale.kind, "stale_registration");

        // A forged body under a real signature is refused.
        let mut forged = signed(&alice, &registration("other", 1_000), 103);
        forged.body = forged.body.replace("1000", "1");
        assert_eq!(registry.register(&forged).unwrap_err().status, 400);

        let reopened = Registry::open(&path).unwrap();
        let entry = reopened.get("weather").unwrap();
        assert_eq!(entry.registration.price_msat, 5_000);
        assert_eq!(entry.owner, alice.pubkey());
        assert_eq!(reopened.len(), 1);
    }

    #[test]
    fn registrations_are_shape_checked() {
        let bad = |edit: fn(&mut Registration)| {
            let mut r = registration("weather", 1_000);
            edit(&mut r);
            r.check().unwrap_err()
        };
        assert!(bad(|r| r.resource = "Weather".into()).contains("resource"));
        assert!(bad(|r| r.resource = "a/b".into()).contains("resource"));
        assert!(bad(|r| r.method = "DELETE".into()).contains("method"));
        assert!(bad(|r| r.price_msat = 0).contains("price"));
        assert!(bad(|r| r.payout = String::new()).contains("payout"));
        assert!(bad(|r| r.upstream = "https://a.example/#x".into()).contains("upstream"));
        assert!(bad(|r| r.v = 2).contains("v must"));
    }

    #[test]
    fn only_public_unicast_addresses_are_upstreams() {
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::a00:1",
            "2002:7f00:1::",
            "2001:db8::1",
        ] {
            assert!(!public_ip(private.parse().unwrap()), "{private}");
        }
        for public in [
            "1.1.1.1",
            "8.8.8.8",
            "2606:4700:4700::1111",
            "::ffff:1.1.1.1",
        ] {
            assert!(public_ip(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn the_paid_header_verifies_only_for_its_url_body_and_key() {
        let host = signer("33");
        let paid = Paid {
            resource: "weather".into(),
            request_hash: "ab".repeat(32),
            payment_hash: "cd".repeat(32),
            settled_at: 1_000,
            id: String::new(),
        };
        let url = "https://author.example/weather?city=oslo";
        let header = paid_header(&host, &paid, "GET", url, b"", 1_000);
        let got = verify_paid(&header, host.pubkey(), "GET", url, b"", 1_010).unwrap();
        assert_eq!(got.resource, "weather");
        assert_eq!(got.payment_hash, paid.payment_hash);
        assert_eq!(got.settled_at, 1_000);
        assert!(verify_paid(&header, signer("44").pubkey(), "GET", url, b"", 1_010).is_err());
        assert!(verify_paid(&header, host.pubkey(), "POST", url, b"", 1_010).is_err());
        assert!(
            verify_paid(
                &header,
                host.pubkey(),
                "GET",
                "https://x.example/",
                b"",
                1_010
            )
            .is_err()
        );
        assert!(verify_paid(&header, host.pubkey(), "GET", url, b"x", 1_010).is_err());
        assert!(verify_paid(&header, host.pubkey(), "GET", url, b"", 2_000).is_err());
    }
}
