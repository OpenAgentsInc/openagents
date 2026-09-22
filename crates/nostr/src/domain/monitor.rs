//! NIP-66 relay liveness monitoring.
//!
//! Kind `30166` is an addressable observation of one relay. The `d` tag
//! is that relay's normalized `ws://` or `wss://` URL, or a 32-byte hex
//! pubkey when the relay has no URL. Kind `10166` is a replaceable
//! announcement that a monitor will publish those observations.
//!
//! The relay does not open a socket, fetch a NIP-11 document, or measure
//! round-trip time. It stores the monitor's claim. NIP-66 is a draft, so
//! these kinds stay off the NIP-11 list.

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const DISCOVERY_KIND: u16 = 30_166;
const ANNOUNCEMENT_KIND: u16 = 10_166;
const GEOHASH: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// The network a monitor assigned to a relay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayNetwork {
    Clearnet,
    Tor,
    I2p,
    Loki,
}

/// One NIP-11 requirement a monitor reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayRequirement {
    pub name: String,
    pub enabled: bool,
}

/// A kind `30166` relay observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayDiscovery {
    pub relay: String,
    pub network: Option<RelayNetwork>,
    pub relay_type: Option<String>,
    pub nips: Vec<String>,
    pub requirements: Vec<RelayRequirement>,
    pub topics: Vec<String>,
    pub accepted_kinds: Vec<u16>,
    pub rejected_kinds: Vec<u16>,
    pub geohash: Option<String>,
    pub rtt_open: Option<u64>,
    pub rtt_read: Option<u64>,
    pub rtt_write: Option<u64>,
    pub nip11: Option<String>,
}

/// One timeout from a kind `10166` announcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorTimeout {
    pub check: Option<String>,
    pub millis: u64,
}

/// A kind `10166` monitor announcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorAnnouncement {
    pub frequency: u64,
    pub timeouts: Vec<MonitorTimeout>,
    pub checks: Vec<String>,
    pub geohash: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn whole(value: &str, reason: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(reason));
    }
    value.parse().map_err(|_| invalid(reason))
}

fn check_name(value: &str) -> bool {
    let mut chars = value.bytes();
    matches!(chars.next(), Some(byte) if byte.is_ascii_lowercase())
        && chars.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.len() <= 16
}

fn geohash_ok(value: &str) -> bool {
    (1..=12).contains(&value.len()) && value.bytes().all(|byte| GEOHASH.contains(&byte))
}

fn normalize_path(raw: &str) -> Result<String, DomainError> {
    if raw.contains('%') {
        return Err(invalid("a relay url does not use percent-encoding"));
    }
    let mut segments = Vec::new();
    for segment in raw.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            if segments.pop().is_none() {
                return Err(invalid(
                    "a relay url is a normalized ws:// or wss:// address",
                ));
            }
            continue;
        }
        segments.push(segment);
    }
    if segments.is_empty() {
        Ok("/".to_owned())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

/// Normalize a relay URL: lowercase scheme and host, drop the default port,
/// and use `/` when the path is empty.
pub fn normalize_relay_url(value: &str) -> Result<String, DomainError> {
    let reason = "a relay url is a normalized ws:// or wss:// address";
    if value.is_empty()
        || value.len() > 2_048
        || value
            .chars()
            .any(|char| char.is_whitespace() || char.is_control())
    {
        return Err(invalid(reason));
    }
    let Some((raw_scheme, rest)) = value.split_once("://") else {
        return Err(invalid(reason));
    };
    let scheme = raw_scheme.to_ascii_lowercase();
    if scheme != "ws" && scheme != "wss" {
        return Err(invalid(reason));
    }
    if rest.contains('@') || rest.contains('?') || rest.contains('#') || rest.contains('\\') {
        return Err(invalid(reason));
    }
    let (authority, raw_path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => (rest, ""),
    };
    if authority.is_empty() || authority.starts_with('[') {
        return Err(invalid(reason));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if port.bytes().all(|byte| byte.is_ascii_digit()) => (host, Some(port)),
        Some(_) => return Err(invalid(reason)),
        None => (authority, None),
    };
    if host.is_empty()
        || host.starts_with('.')
        || host.ends_with('.')
        || host.contains("..")
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
    {
        return Err(invalid(reason));
    }
    let host = host.to_ascii_lowercase();
    let port = match port {
        None => None,
        Some(port) => {
            let parsed = whole(port, reason)?;
            let parsed = u16::try_from(parsed).map_err(|_| invalid(reason))?;
            let default = if scheme == "wss" { 443 } else { 80 };
            if parsed == default {
                None
            } else {
                Some(parsed)
            }
        }
    };
    let path = normalize_path(raw_path)?;
    let mut url = format!("{scheme}://{host}");
    if let Some(port) = port {
        url.push(':');
        url.push_str(&port.to_string());
    }
    url.push_str(&path);
    Ok(url)
}

fn subject(value: &str) -> Result<String, DomainError> {
    if decode_lower_hex::<32>(value, "relay pubkey").is_ok() {
        return Ok(value.to_owned());
    }
    let normalized = normalize_relay_url(value)?;
    if normalized != value {
        return Err(invalid("a relay discovery d tag is a normalized url"));
    }
    Ok(value.to_owned())
}

fn network(value: &str) -> Result<RelayNetwork, DomainError> {
    match value {
        "clearnet" => Ok(RelayNetwork::Clearnet),
        "tor" => Ok(RelayNetwork::Tor),
        "i2p" => Ok(RelayNetwork::I2p),
        "loki" => Ok(RelayNetwork::Loki),
        _ => Err(invalid("a relay network is clearnet, tor, i2p, or loki")),
    }
}

fn relay_type(value: &str) -> Result<String, DomainError> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(invalid("a relay type is PascalCase"));
    };
    if first.is_ascii_uppercase()
        && (2..=64).contains(&value.len())
        && chars.all(|char| char.is_ascii_alphanumeric())
    {
        Ok(value.to_owned())
    } else {
        Err(invalid("a relay type is PascalCase"))
    }
}

fn nip_id(value: &str) -> Result<String, DomainError> {
    if (1..=4).contains(&value.len())
        && !value.starts_with('0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_uppercase())
    {
        Ok(value.to_owned())
    } else {
        Err(invalid("a supported nip is a short identifier"))
    }
}

fn requirement(value: &str) -> Result<RelayRequirement, DomainError> {
    let (enabled, name) = match value.strip_prefix('!') {
        Some(name) => (false, name),
        None => (true, value),
    };
    if check_name(name) {
        Ok(RelayRequirement {
            name: name.to_owned(),
            enabled,
        })
    } else {
        Err(invalid("a relay requirement is a lowercase name"))
    }
}

fn kind_flag(value: &str) -> Result<(u16, bool), DomainError> {
    let (accepted, raw) = match value.strip_prefix('!') {
        Some(raw) => (false, raw),
        None => (true, value),
    };
    let parsed = whole(raw, "a relay kind is a number")?;
    let kind = u16::try_from(parsed).map_err(|_| invalid("a relay kind is a number"))?;
    Ok((kind, accepted))
}

fn one_value<'a>(tag: &'a super::Tag, reason: &'static str) -> Result<&'a str, DomainError> {
    if tag.as_slice().len() != 2 {
        return Err(invalid(reason));
    }
    tag.value().ok_or_else(|| invalid(reason))
}

/// Read a kind `30166` relay observation.
pub fn open_relay_discovery(event: &Event) -> Result<RelayDiscovery, DomainError> {
    if event.kind != DISCOVERY_KIND {
        return Err(invalid("a relay discovery event has kind 30166"));
    }
    if !event.content.is_empty() {
        let value: serde_json::Value = serde_json::from_str(&event.content)
            .map_err(|_| invalid("relay discovery content is a NIP-11 object"))?;
        if !value.is_object() {
            return Err(invalid("relay discovery content is a NIP-11 object"));
        }
    }
    let mut relay = None;
    let mut network_name = None;
    let mut relay_type_name = None;
    let mut nips = Vec::new();
    let mut requirements = Vec::new();
    let mut topics = Vec::new();
    let mut accepted_kinds = Vec::new();
    let mut rejected_kinds = Vec::new();
    let mut geohash = None;
    let mut rtt_open = None;
    let mut rtt_read = None;
    let mut rtt_write = None;
    for tag in &event.tags {
        match tag.name() {
            Some("d") => {
                if relay.is_some() {
                    return Err(invalid("a relay discovery event has one subject"));
                }
                relay = Some(subject(one_value(
                    tag,
                    "a relay discovery event has one subject",
                )?)?);
            }
            Some("n") => {
                if network_name.is_some() {
                    return Err(invalid("a relay network is listed once"));
                }
                network_name = Some(network(one_value(tag, "a relay network is listed once")?)?);
            }
            Some("T") => {
                if relay_type_name.is_some() {
                    return Err(invalid("a relay type is listed once"));
                }
                relay_type_name = Some(relay_type(one_value(tag, "a relay type is listed once")?)?);
            }
            Some("N") => {
                let nip = nip_id(one_value(tag, "a supported nip is one value")?)?;
                if nips.iter().any(|seen| seen == &nip) {
                    return Err(invalid("a supported nip is listed once"));
                }
                nips.push(nip);
            }
            Some("R") => {
                let requirement = requirement(one_value(tag, "a relay requirement is one value")?)?;
                if requirements
                    .iter()
                    .any(|seen: &RelayRequirement| seen.name == requirement.name)
                {
                    return Err(invalid("a relay requirement is listed once"));
                }
                requirements.push(requirement);
            }
            Some("t") => {
                let topic = one_value(tag, "a relay topic is one value")?;
                if topic.is_empty()
                    || topic.len() > 32
                    || !topic.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
                {
                    return Err(invalid("a relay topic is a short lowercase name"));
                }
                if topics.iter().any(|seen| seen == topic) {
                    return Err(invalid("a relay topic is listed once"));
                }
                topics.push(topic.to_owned());
            }
            Some("k") => {
                let (kind, accepted) = kind_flag(one_value(tag, "a relay kind is one value")?)?;
                if accepted_kinds.contains(&kind) || rejected_kinds.contains(&kind) {
                    return Err(invalid("a relay kind is listed once"));
                }
                if accepted {
                    accepted_kinds.push(kind);
                } else {
                    rejected_kinds.push(kind);
                }
            }
            Some("g") => {
                if geohash.is_some() {
                    return Err(invalid("a relay geohash is listed once"));
                }
                let value = one_value(tag, "a relay geohash is listed once")?;
                if !geohash_ok(value) {
                    return Err(invalid("a relay geohash is 1 to 12 characters"));
                }
                geohash = Some(value.to_owned());
            }
            Some("rtt-open") => {
                if rtt_open.is_some() {
                    return Err(invalid("a relay round trip is listed once"));
                }
                rtt_open = Some(whole(
                    one_value(tag, "a relay round trip is milliseconds")?,
                    "a relay round trip is milliseconds",
                )?);
            }
            Some("rtt-read") => {
                if rtt_read.is_some() {
                    return Err(invalid("a relay round trip is listed once"));
                }
                rtt_read = Some(whole(
                    one_value(tag, "a relay round trip is milliseconds")?,
                    "a relay round trip is milliseconds",
                )?);
            }
            Some("rtt-write") => {
                if rtt_write.is_some() {
                    return Err(invalid("a relay round trip is listed once"));
                }
                rtt_write = Some(whole(
                    one_value(tag, "a relay round trip is milliseconds")?,
                    "a relay round trip is milliseconds",
                )?);
            }
            _ => {}
        }
    }
    let Some(relay) = relay else {
        return Err(invalid("a relay discovery event has one subject"));
    };
    Ok(RelayDiscovery {
        relay,
        network: network_name,
        relay_type: relay_type_name,
        nips,
        requirements,
        topics,
        accepted_kinds,
        rejected_kinds,
        geohash,
        rtt_open,
        rtt_read,
        rtt_write,
        nip11: if event.content.is_empty() {
            None
        } else {
            Some(event.content.clone())
        },
    })
}

fn timeout_tag(tag: &super::Tag) -> Result<MonitorTimeout, DomainError> {
    let parts = tag.as_slice();
    let reason = "a monitor timeout is milliseconds and an optional check";
    match parts.len() {
        2 => Ok(MonitorTimeout {
            check: None,
            millis: whole(&parts[1], reason)?,
        }),
        3 if parts[1].bytes().all(|byte| byte.is_ascii_digit()) && check_name(&parts[2]) => {
            Ok(MonitorTimeout {
                check: Some(parts[2].clone()),
                millis: whole(&parts[1], reason)?,
            })
        }
        3 if check_name(&parts[1]) && parts[2].bytes().all(|byte| byte.is_ascii_digit()) => {
            Ok(MonitorTimeout {
                check: Some(parts[1].clone()),
                millis: whole(&parts[2], reason)?,
            })
        }
        _ => Err(invalid(reason)),
    }
}

/// Read a kind `10166` monitor announcement.
pub fn open_monitor_announcement(event: &Event) -> Result<MonitorAnnouncement, DomainError> {
    if event.kind != ANNOUNCEMENT_KIND {
        return Err(invalid("a monitor announcement has kind 10166"));
    }
    let mut frequency = None;
    let mut timeouts = Vec::new();
    let mut checks = Vec::new();
    let mut geohash = None;
    for tag in &event.tags {
        match tag.name() {
            Some("frequency") => {
                if frequency.is_some() {
                    return Err(invalid("a monitor frequency is listed once"));
                }
                frequency = Some(whole(
                    one_value(tag, "a monitor frequency is seconds")?,
                    "a monitor frequency is seconds",
                )?);
            }
            Some("timeout") => {
                let timeout = timeout_tag(tag)?;
                if timeouts
                    .iter()
                    .any(|seen: &MonitorTimeout| seen.check == timeout.check)
                {
                    return Err(invalid("a monitor timeout is listed once"));
                }
                timeouts.push(timeout);
            }
            Some("c") => {
                let check = one_value(tag, "a monitor check is one lowercase name")?;
                if !check_name(check) {
                    return Err(invalid("a monitor check is one lowercase name"));
                }
                if checks.iter().any(|seen| seen == check) {
                    return Err(invalid("a monitor check is listed once"));
                }
                checks.push(check.to_owned());
            }
            Some("g") => {
                if geohash.is_some() {
                    return Err(invalid("a monitor geohash is listed once"));
                }
                let value = one_value(tag, "a monitor geohash is listed once")?;
                if !geohash_ok(value) {
                    return Err(invalid("a monitor geohash is 1 to 12 characters"));
                }
                geohash = Some(value.to_owned());
            }
            _ => {}
        }
    }
    let Some(frequency) = frequency else {
        return Err(invalid("a monitor frequency is seconds"));
    };
    if frequency == 0 {
        return Err(invalid("a monitor frequency is seconds"));
    }
    if checks.is_empty() {
        return Err(invalid("a monitor lists one check"));
    }
    Ok(MonitorAnnouncement {
        frequency,
        timeouts,
        checks,
        geohash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"66".repeat(32)).unwrap()
    }

    #[test]
    fn a_relay_observation_replaces_and_a_monitor_lists_its_checks() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/66.md"
        ))
        .unwrap();
        assert!(text.contains("30166"));
        assert!(text.contains("10166"));
        assert!(text.contains("\"d\""));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "66.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "66.md")
        );

        let canonical = "wss://some.relay/";
        assert_eq!(normalize_relay_url(canonical).unwrap(), canonical);
        assert_eq!(normalize_relay_url("wss://some.relay").unwrap(), canonical);
        assert_eq!(
            normalize_relay_url("WSS://Some.Relay:443").unwrap(),
            canonical
        );
        assert_eq!(
            normalize_relay_url("wss://some.relay/a/../b").unwrap(),
            "wss://some.relay/b"
        );

        let monitor = signer();
        let discovery = monitor.sign(
            1_700_000_000,
            DISCOVERY_KIND,
            vec![
                Tag::new(vec!["d".into(), canonical.into()]),
                Tag::new(vec!["n".into(), "clearnet".into()]),
                Tag::new(vec!["T".into(), "PrivateInbox".into()]),
                Tag::new(vec!["N".into(), "40".into()]),
                Tag::new(vec!["N".into(), "33".into()]),
                Tag::new(vec!["R".into(), "!payment".into()]),
                Tag::new(vec!["R".into(), "auth".into()]),
                Tag::new(vec!["g".into(), "ww8p1r4t8".into()]),
                Tag::new(vec!["l".into(), "en".into(), "ISO-639-1".into()]),
                Tag::new(vec!["t".into(), "nsfw".into()]),
                Tag::new(vec!["k".into(), "1".into()]),
                Tag::new(vec!["k".into(), "!4".into()]),
                Tag::new(vec!["rtt-open".into(), "234".into()]),
            ],
            r#"{"supported_nips":[1]}"#.into(),
        );
        discovery.validate_structure().unwrap();
        assert_eq!(discovery.class(), EventClass::Addressable);
        let opened = open_relay_discovery(&discovery).unwrap();
        assert_eq!(opened.relay, canonical);
        assert_eq!(opened.network, Some(RelayNetwork::Clearnet));
        assert_eq!(opened.relay_type.as_deref(), Some("PrivateInbox"));
        assert_eq!(opened.nips, ["40", "33"]);
        assert!(
            opened
                .requirements
                .iter()
                .any(|requirement| { requirement.name == "payment" && !requirement.enabled })
        );
        assert_eq!(opened.rtt_open, Some(234));
        assert_eq!(opened.accepted_kinds, vec![1]);
        assert_eq!(opened.rejected_kinds, vec![4]);
        assert!(opened.nip11.as_ref().unwrap().contains("supported_nips"));

        let revised = monitor.sign(
            1_700_000_100,
            DISCOVERY_KIND,
            vec![
                Tag::new(vec!["d".into(), canonical.into()]),
                Tag::new(vec!["rtt-open".into(), "180".into()]),
            ],
            String::new(),
        );
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&discovery, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
        let messy = monitor.sign(
            1_700_000_150,
            DISCOVERY_KIND,
            vec![Tag::new(vec!["d".into(), "wss://some.relay".into()])],
            String::new(),
        );
        assert!(messy.validate_structure().is_err());

        let pubkey = "ab".repeat(32);
        let hidden = monitor.sign(
            1_700_000_180,
            DISCOVERY_KIND,
            vec![Tag::new(vec!["d".into(), pubkey.clone()])],
            String::new(),
        );
        hidden.validate_structure().unwrap();
        assert_eq!(open_relay_discovery(&hidden).unwrap().relay, pubkey);

        let announcement = monitor.sign(
            1_700_000_200,
            ANNOUNCEMENT_KIND,
            vec![
                Tag::new(vec!["timeout".into(), "open".into(), "5000".into()]),
                Tag::new(vec!["timeout".into(), "3000".into()]),
                Tag::new(vec!["frequency".into(), "3600".into()]),
                Tag::new(vec!["c".into(), "ws".into()]),
                Tag::new(vec!["c".into(), "nip11".into()]),
                Tag::new(vec!["g".into(), "ww8p1r4t8".into()]),
            ],
            String::new(),
        );
        announcement.validate_structure().unwrap();
        assert_eq!(announcement.class(), EventClass::Replaceable);
        let listed = open_monitor_announcement(&announcement).unwrap();
        assert_eq!(listed.frequency, 3_600);
        assert_eq!(listed.checks, ["ws", "nip11"]);
        assert_eq!(listed.timeouts[0].check.as_deref(), Some("open"));
        assert_eq!(listed.timeouts[0].millis, 5_000);
        assert_eq!(listed.timeouts[1].check, None);
        assert_eq!(listed.timeouts[1].millis, 3_000);
        let later = monitor.sign(
            1_700_000_300,
            ANNOUNCEMENT_KIND,
            vec![
                Tag::new(vec!["frequency".into(), "7200".into()]),
                Tag::new(vec!["c".into(), "dns".into()]),
            ],
            String::new(),
        );
        later.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&announcement, &later),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
    }
}
