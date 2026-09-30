//! The `openagents-connect:` payload a computer shows as a QR code.
//!
//! A code has two text forms that carry the same payload:
//!
//! - the **link**, `https://openagents.com/connect#<payload>`, which the QR
//!   code shows so a phone's system camera opens the OpenAgents app (a
//!   universal link on iOS, a verified App Link on Android) or, without the
//!   app, a page that says where to get it. The payload is the URL
//!   fragment, which a browser never sends to a server;
//! - the **text**, `openagents-connect:<payload>`, the canonical form every
//!   reader passes on after [`canonical`], which also reads the link.
//!
//! The payload is unpadded base64url of these bytes, in order:
//!
//! | Field | Bytes |
//! | --- | --- |
//! | version, `1` | 1 |
//! | host Nostr x-only public key | 32 |
//! | host iroh `EndpointId` (Ed25519) | 32 |
//! | invitation ID | 32 |
//! | capability, independently random | 32 |
//! | issue time, big-endian seconds | 8 |
//! | expiry, big-endian seconds | 8 |
//! | iroh relay URL length, then its UTF-8 bytes (0 means none) | 1 + 0–128 |
//! | count of direct addresses (at most 8) | 1 |
//! | each: family `4` or `6`, the address (4 or 16), the port (2, big-endian) | 7 or 19 |
//! | label length, then its UTF-8 bytes (the computer's display name) | 1 + 0–48 |
//!
//! No trailing bytes are allowed. The invitation ID, capability, and times
//! are exactly a NIP-HOST host invitation; only the carriage is new. The
//! label is for display and is never an identity.
//!
//! The capability is a bearer secret until redemption: never log it, and
//! never put it in a URL anywhere but the link's fragment. [`ConnectCode`]'s
//! `Debug` output leaves it out.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use iroh::{EndpointAddr, EndpointId, RelayUrl, TransportAddr};

use crate::{CLOCK_SKEW, Code, Error, Result, fail, hex, unhex32};

/// The text prefix.
pub const PREFIX: &str = "openagents-connect:";
/// The link prefix: everything before the payload, which is the fragment.
pub const LINK_PREFIX: &str = "https://openagents.com/connect#";
/// The only layout version.
pub const VERSION: u8 = 1;
/// A NIP-HOST host invitation lives exactly this long, in seconds.
pub const LIFETIME: u64 = 300;
/// Longest relay URL, in bytes.
pub const MAX_RELAY_BYTES: usize = 128;
/// Most direct addresses.
pub const MAX_ADDRS: usize = 8;
/// Longest label, in bytes.
pub const MAX_LABEL_BYTES: usize = 48;
/// Fixed part: version, four 32-byte keys, two times, and the three
/// length or count bytes.
const FIXED_BYTES: usize = 1 + 4 * 32 + 2 * 8 + 3;
/// Largest decoded payload.
pub const MAX_BYTES: usize = FIXED_BYTES + MAX_RELAY_BYTES + MAX_ADDRS * 19 + MAX_LABEL_BYTES;
/// Longest encoded payload, without a prefix.
const MAX_PAYLOAD_TEXT_BYTES: usize = (MAX_BYTES * 4).div_ceil(3);
/// Longest encoded text, prefix included.
pub const MAX_TEXT_BYTES: usize = PREFIX.len() + MAX_PAYLOAD_TEXT_BYTES;
/// Longest link, prefix included.
pub const MAX_LINK_BYTES: usize = LINK_PREFIX.len() + MAX_PAYLOAD_TEXT_BYTES;

/// The canonical `openagents-connect:` text of a code in either form, the
/// text or the link, or `None` when `text` is neither. Only the prefix
/// changes; the payload is not checked here.
#[must_use]
pub fn canonical(text: &str) -> Option<String> {
    if text.starts_with(PREFIX) {
        return Some(text.to_owned());
    }
    text.strip_prefix(LINK_PREFIX)
        .map(|payload| format!("{PREFIX}{payload}"))
}

/// The link form of a code in either form, for the QR code, or `None` when
/// `text` is neither.
#[must_use]
pub fn link(text: &str) -> Option<String> {
    if text.starts_with(LINK_PREFIX) {
        return Some(text.to_owned());
    }
    text.strip_prefix(PREFIX)
        .map(|payload| format!("{LINK_PREFIX}{payload}"))
}

/// A parsed or freshly issued connect code.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectCode {
    host: [u8; 32],
    endpoint: EndpointId,
    invitation: [u8; 32],
    capability: [u8; 32],
    issued_at: u64,
    expires_at: u64,
    relay: Option<RelayUrl>,
    addrs: Vec<SocketAddr>,
    label: String,
}

impl std::fmt::Debug for ConnectCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The capability stays out of debug output.
        f.debug_struct("ConnectCode")
            .field("host", &self.host())
            .field("endpoint", &self.endpoint)
            .field("invitation", &self.invitation())
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("relay", &self.relay)
            .field("addrs", &self.addrs)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// The parts of a code other than the random invitation ID and capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeParts {
    /// Host Nostr x-only public key, 64 lowercase hex characters.
    pub host: String,
    pub endpoint: EndpointId,
    pub issued_at: u64,
    pub relay: Option<RelayUrl>,
    pub addrs: Vec<SocketAddr>,
    pub label: String,
}

impl ConnectCode {
    /// Issue a code with a fresh random invitation ID and capability and the
    /// NIP-HOST lifetime. The host stores the invitation (with the
    /// capability's digest) before it shows the code.
    ///
    /// # Errors
    /// Refuses parts that break a bound.
    pub fn issue(parts: CodeParts) -> Result<Self> {
        Self::from_parts(
            parts,
            coder_reach::random_bytes(),
            coder_reach::random_bytes(),
        )
    }

    /// Build a code from an invitation the host already stored, such as one
    /// `coder_access` issued. `invitation` and `capability` are 64 lowercase
    /// hex characters each.
    ///
    /// # Errors
    /// Refuses malformed hex or parts that break a bound.
    pub fn from_invitation(parts: CodeParts, invitation: &str, capability: &str) -> Result<Self> {
        Self::from_parts(parts, unhex32(invitation)?, unhex32(capability)?)
    }

    fn from_parts(parts: CodeParts, invitation: [u8; 32], capability: [u8; 32]) -> Result<Self> {
        let expires_at = parts
            .issued_at
            .checked_add(LIFETIME)
            .ok_or_else(|| Error::new(Code::Malformed, "issue time overflows"))?;
        let code = Self {
            host: unhex32(&parts.host)?,
            endpoint: parts.endpoint,
            invitation,
            capability,
            issued_at: parts.issued_at,
            expires_at,
            relay: parts.relay,
            addrs: parts.addrs,
            label: parts.label,
        };
        code.check_shape()?;
        Ok(code)
    }

    /// Host Nostr x-only public key, lowercase hex.
    #[must_use]
    pub fn host(&self) -> String {
        hex(&self.host)
    }

    /// Host iroh endpoint ID.
    #[must_use]
    pub fn endpoint(&self) -> EndpointId {
        self.endpoint
    }

    /// Invitation ID, lowercase hex.
    #[must_use]
    pub fn invitation(&self) -> String {
        hex(&self.invitation)
    }

    /// The bearer capability, lowercase hex. Never log or display it.
    #[must_use]
    pub fn capability(&self) -> String {
        hex(&self.capability)
    }

    #[must_use]
    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }

    #[must_use]
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    #[must_use]
    pub fn relay(&self) -> Option<&RelayUrl> {
        self.relay.as_ref()
    }

    #[must_use]
    pub fn addrs(&self) -> &[SocketAddr] {
        &self.addrs
    }

    /// The computer's name, for display only.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The address to dial: the endpoint ID with its relay and direct
    /// addresses.
    #[must_use]
    pub fn endpoint_addr(&self) -> EndpointAddr {
        let relay = self.relay.iter().cloned().map(TransportAddr::Relay);
        let direct = self.addrs.iter().copied().map(TransportAddr::Ip);
        EndpointAddr::from_parts(self.endpoint, relay.chain(direct))
    }

    /// Encode to the link a QR code shows, `https://openagents.com/connect#`
    /// and the payload.
    #[must_use]
    pub fn encode_link(&self) -> String {
        let text = self.encode();
        format!("{LINK_PREFIX}{}", &text[PREFIX.len()..])
    }

    /// Encode to the `openagents-connect:` text.
    #[must_use]
    pub fn encode(&self) -> String {
        let relay = self.relay.as_ref().map(relay_text).unwrap_or_default();
        let mut bytes = Vec::with_capacity(MAX_BYTES);
        bytes.push(VERSION);
        bytes.extend(self.host);
        bytes.extend(self.endpoint.as_bytes());
        bytes.extend(self.invitation);
        bytes.extend(self.capability);
        bytes.extend(self.issued_at.to_be_bytes());
        bytes.extend(self.expires_at.to_be_bytes());
        // Bounds were checked when the code was built.
        bytes.push(relay.len() as u8);
        bytes.extend(relay.as_bytes());
        bytes.push(self.addrs.len() as u8);
        for addr in &self.addrs {
            match addr.ip() {
                IpAddr::V4(ip) => {
                    bytes.push(4);
                    bytes.extend(ip.octets());
                }
                IpAddr::V6(ip) => {
                    bytes.push(6);
                    bytes.extend(ip.octets());
                }
            }
            bytes.extend(addr.port().to_be_bytes());
        }
        bytes.push(self.label.len() as u8);
        bytes.extend(self.label.as_bytes());
        format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Parse and check a code at time `now`.
    ///
    /// # Errors
    /// `unsupported_version` for another layout version; `bounds` for an
    /// over-long text, relay, or label or too many addresses; `expired` when
    /// `now` is at or past the expiry or the issue time is more than
    /// [`CLOCK_SKEW`] ahead of `now`; `malformed` for anything else,
    /// including trailing bytes and an unknown address family.
    pub fn parse(text: &str, now: u64) -> Result<Self> {
        let code = Self::parse_shape(text)?;
        code.check_time(now)?;
        Ok(code)
    }

    /// Parse without checking the time window. The host decides expiry;
    /// use this only to show what a code names. Both forms parse: the
    /// `openagents-connect:` text and the `https://openagents.com/connect#`
    /// link.
    ///
    /// # Errors
    /// As [`Self::parse`], without `expired`.
    pub fn parse_shape(text: &str) -> Result<Self> {
        let encoded = if let Some(encoded) = text.strip_prefix(LINK_PREFIX) {
            encoded
        } else if let Some(encoded) = text.strip_prefix(PREFIX) {
            encoded
        } else if text.len() > MAX_TEXT_BYTES {
            return fail(Code::Bounds, "code exceeds its length bound");
        } else {
            return fail(Code::Malformed, "not an openagents-connect code");
        };
        if encoded.len() > MAX_PAYLOAD_TEXT_BYTES {
            return fail(Code::Bounds, "code exceeds its length bound");
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| Error::new(Code::Malformed, "code is not unpadded base64url"))?;
        // Refuse a non-canonical encoding (stray bits in the last character).
        if URL_SAFE_NO_PAD.encode(&bytes) != encoded {
            return fail(Code::Malformed, "code is not canonical base64url");
        }
        let mut reader = Reader(&bytes);
        let version = reader.byte()?;
        if version != VERSION {
            return fail(Code::UnsupportedVersion, "unsupported code version");
        }
        let host = reader.array()?;
        let endpoint = EndpointId::from_bytes(&reader.array()?)
            .map_err(|_| Error::new(Code::Malformed, "endpoint ID is not an Ed25519 key"))?;
        let invitation = reader.array()?;
        let capability = reader.array()?;
        let issued_at = u64::from_be_bytes(reader.array()?);
        let expires_at = u64::from_be_bytes(reader.array()?);

        let relay_len = usize::from(reader.byte()?);
        if relay_len > MAX_RELAY_BYTES {
            return fail(Code::Bounds, "relay URL exceeds its bound");
        }
        let relay = match relay_len {
            0 => None,
            len => Some(parse_relay(utf8(reader.take(len)?)?)?),
        };

        let count = usize::from(reader.byte()?);
        if count > MAX_ADDRS {
            return fail(Code::Bounds, "too many direct addresses");
        }
        let mut addrs = Vec::with_capacity(count);
        for _ in 0..count {
            let ip = match reader.byte()? {
                4 => IpAddr::V4(Ipv4Addr::from(reader.array::<4>()?)),
                6 => IpAddr::V6(Ipv6Addr::from(reader.array::<16>()?)),
                _ => return fail(Code::Malformed, "unknown address family"),
            };
            let port = u16::from_be_bytes(reader.array()?);
            addrs.push(SocketAddr::new(ip, port));
        }

        let label_len = usize::from(reader.byte()?);
        if label_len > MAX_LABEL_BYTES {
            return fail(Code::Bounds, "label exceeds its bound");
        }
        let label = utf8(reader.take(label_len)?)?.to_owned();
        if !reader.0.is_empty() {
            return fail(Code::Malformed, "trailing bytes after the label");
        }

        let code = Self {
            host,
            endpoint,
            invitation,
            capability,
            issued_at,
            expires_at,
            relay,
            addrs,
            label,
        };
        code.check_shape()?;
        Ok(code)
    }

    /// Whether the code is inside its window at `now`: issued no more than
    /// [`CLOCK_SKEW`] ahead, and strictly before its expiry.
    ///
    /// # Errors
    /// `expired` otherwise.
    pub fn check_time(&self, now: u64) -> Result<()> {
        if self.issued_at > now.saturating_add(CLOCK_SKEW) {
            return fail(Code::Expired, "code issued in the future");
        }
        if now >= self.expires_at {
            return fail(Code::Expired, "code expired");
        }
        Ok(())
    }

    fn check_shape(&self) -> Result<()> {
        coder_reach::parse_pubkey(&hex(&self.host))
            .map_err(|_| Error::new(Code::Malformed, "host key is not an x-only public key"))?;
        if self.invitation == [0; 32] || self.capability == [0; 32] {
            return fail(Code::Malformed, "invitation or capability is zero");
        }
        if self.expires_at.checked_sub(self.issued_at) != Some(LIFETIME) {
            return fail(
                Code::Malformed,
                "code lifetime differs from the NIP-HOST invitation lifetime",
            );
        }
        if let Some(relay) = &self.relay {
            let text = relay_text(relay);
            if text.len() > MAX_RELAY_BYTES {
                return fail(Code::Bounds, "relay URL exceeds its bound");
            }
            parse_relay(&text)?;
        }
        if self.addrs.len() > MAX_ADDRS {
            return fail(Code::Bounds, "too many direct addresses");
        }
        for (i, addr) in self.addrs.iter().enumerate() {
            let ip = addr.ip();
            if addr.port() == 0 || ip.is_unspecified() || ip.is_multicast() {
                return fail(Code::Malformed, "direct address cannot be dialed");
            }
            if self.addrs[..i].contains(addr) {
                return fail(Code::Malformed, "duplicate direct address");
            }
        }
        if self.label.len() > MAX_LABEL_BYTES {
            return fail(Code::Bounds, "label exceeds its bound");
        }
        if self.label.chars().any(char::is_control) {
            return fail(Code::Malformed, "label holds a control character");
        }
        Ok(())
    }
}

/// The relay URL's text as carried in a code: the URL's canonical form.
fn relay_text(relay: &RelayUrl) -> String {
    relay.as_str().to_owned()
}

fn parse_relay(text: &str) -> Result<RelayUrl> {
    let relay: RelayUrl = text
        .parse()
        .map_err(|_| Error::new(Code::Malformed, "relay is not a URL"))?;
    let url = &*relay;
    if url.as_str() != text {
        return fail(Code::Malformed, "relay URL is not in canonical form");
    }
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none_or(str::is_empty)
    {
        return fail(
            Code::Malformed,
            "relay must be https with no credentials, query, or fragment",
        );
    }
    Ok(relay)
}

fn utf8(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|_| Error::new(Code::Malformed, "text is not UTF-8"))
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.0.len() < len {
            return fail(Code::Malformed, "code ends early");
        }
        let (head, tail) = self.0.split_at(len);
        self.0 = tail;
        Ok(head)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }
}
