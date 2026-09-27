//! The host descriptor: what a client reads before it assumes anything
//! about a host.
//!
//! A descriptor names the host's public key, the protocol version the
//! running host reported, the host generation, the capability flags, the
//! running bundle, the loopback address, and the latest update's state. The
//! launcher rewrites it on every transition, so a client that reconnects
//! after an update reads `committed` or `rolled-back` with the target
//! version.
//!
//! The encoding is canonical: one JSON object, keys in lexicographic order,
//! no insignificant whitespace, capability flags sorted and unique, and a
//! trailing newline. Two equal descriptors encode to the same bytes.
//! Decoding is strict: an unknown schema, an unknown field, or a malformed
//! value refuses rather than guessing.

use std::net::SocketAddr;
use std::str::FromStr as _;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, fsx};

/// The descriptor schema this crate writes and reads.
pub const DESCRIPTOR_SCHEMA: &str = "openagents.coder.host-descriptor.v1";

/// The most capability flags one descriptor carries.
pub const CAPABILITIES_MAX: usize = 64;

/// Capability flags the launcher itself contributes, whatever host runs.
pub const LAUNCHER_CAPABILITIES: [&str; 2] = ["host-rollback", "host-trial-update"];

/// Whether the running host is serving.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostState {
    /// The launcher started the host and has no ready report yet.
    Starting,
    /// The host reported ready for this generation.
    Ready,
    /// An update is in progress. Reconnect and read the descriptor again.
    Updating,
    /// No host is running.
    Stopped,
}

/// The latest update's state, as a client sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateState {
    /// No update has run since the launcher's state was created.
    None,
    /// The launcher took a snapshot and recorded the trial.
    Prepared,
    /// The target version is running as a trial.
    Trial,
    /// The target version reported ready and is now the committed version.
    Committed,
    /// The trial failed; the snapshot was restored and the previous version
    /// runs again.
    RolledBack,
}

/// The latest update, with its versions and, after a rollback, the reason.
///
/// Fields are declared in lexicographic order, which is the order they
/// encode in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateView {
    /// The version the update started from.
    pub from: Option<String>,
    /// Why a trial rolled back.
    pub reason: Option<String>,
    /// The update request this state answers.
    pub request: Option<String>,
    /// Where the update is.
    pub state: UpdateState,
    /// The version the update tried to reach.
    pub target: Option<String>,
}

impl UpdateView {
    /// The view before any update.
    #[must_use]
    pub fn none() -> Self {
        UpdateView {
            from: None,
            reason: None,
            request: None,
            state: UpdateState::None,
            target: None,
        }
    }
}

/// The host descriptor.
///
/// Fields are declared in lexicographic order, which is the order they
/// encode in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostDescriptor {
    /// Sorted, unique capability flags. The host reports its own in its
    /// ready record; the launcher adds [`LAUNCHER_CAPABILITIES`]. Before the
    /// host is ready, only the launcher's flags appear.
    pub capabilities: Vec<String>,
    /// The generation of the running host process. It increases every time
    /// the launcher starts a host, so a client that saw an earlier
    /// generation knows the host restarted.
    pub host_generation: u64,
    /// The host's Nostr public key: 64 lowercase hexadecimal characters of
    /// an x-only secp256k1 key.
    pub host_key: String,
    /// The loopback address the host is told to bind.
    pub listen: String,
    /// The host protocol version the running host reported, or `None`
    /// before it reported ready.
    pub protocol_version: Option<u32>,
    /// Always [`DESCRIPTOR_SCHEMA`].
    pub schema: String,
    /// Whether the host is serving.
    pub state: HostState,
    /// The latest update.
    pub update: UpdateView,
    /// The bundle identity the host runs, or the committed one when no
    /// host runs.
    pub version: Option<String>,
}

impl HostDescriptor {
    /// Encodes the descriptor canonically after validating it.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut canonical = self.clone();
        canonical.capabilities.sort();
        canonical.capabilities.dedup();
        canonical.validate()?;
        let mut bytes = serde_json::to_vec(&canonical)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Decodes and validates a descriptor.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let descriptor: HostDescriptor = serde_json::from_slice(bytes)?;
        descriptor.validate()?;
        if descriptor
            .capabilities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::refused(
                "descriptor capability flags must be sorted and unique",
            ));
        }
        Ok(descriptor)
    }

    /// Checks every field against its contract.
    pub fn validate(&self) -> Result<()> {
        if self.schema != DESCRIPTOR_SCHEMA {
            return Err(Error::refused(format!(
                "unsupported descriptor schema {}",
                self.schema
            )));
        }
        validate_host_key(&self.host_key)?;
        validate_loopback(&self.listen)?;
        if self.capabilities.len() > CAPABILITIES_MAX {
            return Err(Error::refused(
                "a descriptor carries too many capability flags",
            ));
        }
        for flag in &self.capabilities {
            validate_capability(flag)?;
        }
        for version in [&self.version, &self.update.from, &self.update.target]
            .into_iter()
            .flatten()
        {
            if !fsx::is_digest(version) {
                return Err(Error::refused(
                    "a descriptor version is not a bundle identity",
                ));
            }
        }
        if let Some(reason) = &self.update.reason
            && (reason.len() > 512 || reason.chars().any(char::is_control))
        {
            return Err(Error::refused("a rollback reason is malformed"));
        }
        if let Some(request) = &self.update.request {
            validate_request_id(request)?;
        }
        Ok(())
    }
}

/// Checks a host key: 64 lowercase hexadecimal characters naming a valid
/// x-only secp256k1 public key.
pub fn validate_host_key(key: &str) -> Result<()> {
    if !fsx::is_digest(key) || secp256k1::XOnlyPublicKey::from_str(key).is_err() {
        return Err(Error::refused(
            "a host key is a 64-character lowercase hexadecimal x-only public key",
        ));
    }
    Ok(())
}

/// Checks that an address is a loopback socket address. Remote reach is a
/// separate, granted route; this service never binds anything else.
pub fn validate_loopback(listen: &str) -> Result<()> {
    match listen.parse::<SocketAddr>() {
        Ok(address) if address.ip().is_loopback() => Ok(()),
        _ => Err(Error::refused(
            "the host listens on a loopback address, such as 127.0.0.1:47100",
        )),
    }
}

/// Checks a capability flag: 1 to 40 characters of lowercase letters,
/// digits, and hyphens.
pub fn validate_capability(flag: &str) -> Result<()> {
    if flag.is_empty()
        || flag.len() > 40
        || !flag
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
    {
        return Err(Error::refused(format!(
            "malformed capability flag {flag:?}"
        )));
    }
    Ok(())
}

/// Checks an update request identifier: 1 to 64 characters of letters,
/// digits, and hyphens.
pub fn validate_request_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(Error::refused("malformed update request identifier"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    fn sample() -> HostDescriptor {
        HostDescriptor {
            capabilities: vec!["tasks".into(), "host-rollback".into(), "tasks".into()],
            host_generation: 7,
            host_key: KEY.into(),
            listen: "127.0.0.1:47100".into(),
            protocol_version: Some(1),
            schema: DESCRIPTOR_SCHEMA.into(),
            state: HostState::Ready,
            update: UpdateView {
                from: Some("a".repeat(64)),
                reason: None,
                request: Some("req-1".into()),
                state: UpdateState::Committed,
                target: Some("b".repeat(64)),
            },
            version: Some("b".repeat(64)),
        }
    }

    #[test]
    fn encoding_is_canonical_and_round_trips() {
        let bytes = sample().encode().unwrap();
        let expected = format!(
            concat!(
                r#"{{"capabilities":["host-rollback","tasks"],"host_generation":7,"#,
                r#""host_key":"{key}","listen":"127.0.0.1:47100","protocol_version":1,"#,
                r#""schema":"openagents.coder.host-descriptor.v1","state":"ready","#,
                r#""update":{{"from":"{a}","reason":null,"request":"req-1","#,
                r#""state":"committed","target":"{b}"}},"version":"{b}"}}"#,
                "\n"
            ),
            key = KEY,
            a = "a".repeat(64),
            b = "b".repeat(64),
        );
        assert_eq!(String::from_utf8(bytes.clone()).unwrap(), expected);
        let decoded = HostDescriptor::decode(&bytes).unwrap();
        assert_eq!(decoded.capabilities, vec!["host-rollback", "tasks"]);
        assert_eq!(decoded.encode().unwrap(), bytes);
    }

    #[test]
    fn decoding_refuses_what_a_client_must_not_assume() {
        let good = String::from_utf8(sample().encode().unwrap()).unwrap();
        let cases = [
            good.replace("host-descriptor.v1", "host-descriptor.v2"),
            good.replace(KEY, &"0".repeat(64)),
            good.replace(KEY, &KEY.to_uppercase()),
            good.replace("127.0.0.1:47100", "0.0.0.0:47100"),
            good.replace("\"tasks\"", "\"Tasks\""),
            good.replace(
                r#"["host-rollback","tasks"]"#,
                r#"["tasks","host-rollback"]"#,
            ),
            good.replace(r#""version":"#, r#""extra":1,"version":"#),
            good.replace(r#""state":"committed""#, r#""state":"finished""#),
            good.replace(
                &format!(r#""target":"{}""#, "b".repeat(64)),
                r#""target":"v2""#,
            ),
        ];
        for case in cases {
            assert!(HostDescriptor::decode(case.as_bytes()).is_err(), "{case}");
        }
    }

    #[test]
    fn ipv6_loopback_is_accepted_and_other_addresses_are_not() {
        assert!(validate_loopback("[::1]:47100").is_ok());
        assert!(validate_loopback("192.168.1.2:47100").is_err());
        assert!(validate_loopback("localhost:47100").is_err());
    }
}
