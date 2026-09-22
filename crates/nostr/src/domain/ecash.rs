//! NIP-87 Cashu and Fedimint discovery.
//!
//! Kind `38172` announces a Cashu mint. Kind `38173` announces a
//! Fedimint. Kind `38000` recommends one of those mints. Each event is
//! addressable: a newer event with the same author, kind, and `d`
//! identifier replaces the older one.
//!
//! A Cashu `d` tag is a 32-byte pubkey. A Fedimint `d` tag is the
//! federation id. `u` lists how to connect. `nuts` or `modules` lists
//! what the mint supports. `n` is `mainnet`, `testnet`, `signet`, or
//! `regtest`. Mint content is empty or a JSON object.
//!
//! The relay does not contact a mint, decode an invite code, or read a
//! kind `0` profile when content is empty. NIP-87 is a draft, so these
//! kinds stay off the NIP-11 list. One recommendation names one mint
//! identifier.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const CASHU_KIND: u16 = 38_172;
const FEDIMINT_KIND: u16 = 38_173;
const RECOMMEND_KIND: u16 = 38_000;

/// Which mint a tag names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MintKind {
    Cashu,
    Fedimint,
}

impl MintKind {
    const fn number(self) -> u16 {
        match self {
            Self::Cashu => CASHU_KIND,
            Self::Fedimint => FEDIMINT_KIND,
        }
    }

    fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "38172" => Ok(Self::Cashu),
            "38173" => Ok(Self::Fedimint),
            _ => Err(invalid("a recommendation kind is 38172 or 38173")),
        }
    }
}

/// The network named by an `n` tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EcashNetwork {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

/// A kind `38172` or kind `38173` announcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintAnnouncement {
    pub kind: MintKind,
    pub identifier: String,
    pub endpoints: Vec<String>,
    pub nuts: Option<Vec<u16>>,
    pub modules: Option<Vec<String>>,
    pub network: EcashNetwork,
    /// Raw JSON object. Empty content is `None`.
    pub metadata: Option<String>,
}

/// One `u` tag on a recommendation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintEndpoint {
    pub value: String,
    pub marker: Option<MintKind>,
}

/// One `a` tag pointing at a mint announcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintPointer {
    pub address: ReplacementAddress,
    pub relay: Option<String>,
    pub marker: Option<MintKind>,
}

/// A kind `38000` recommendation of one mint identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintRecommendation {
    pub identifier: String,
    pub mint_kind: MintKind,
    pub endpoints: Vec<MintEndpoint>,
    pub mints: Vec<MintPointer>,
    pub review: String,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn required<'a>(event: &'a Event, name: &str, reason: &str) -> Result<&'a str, DomainError> {
    let found: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect();
    if found.len() != 1 {
        return Err(invalid(reason));
    }
    found[0]
        .value()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(reason))
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn relay_url(value: &str) -> bool {
    (value.starts_with("wss://") || value.starts_with("ws://"))
        && value.len() > "wss://".len()
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn federation_id(value: &str) -> Result<String, DomainError> {
    if value.is_empty()
        || value.len() > 1_024
        || value
            .chars()
            .any(|char| char.is_control() || char.is_whitespace())
    {
        return Err(invalid("a federation id is 1 to 1024 characters"));
    }
    Ok(value.to_owned())
}

fn cashu_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "cashu mint id")
        .map_err(|_| invalid("a cashu mint id is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn identifier(kind: MintKind, value: &str) -> Result<String, DomainError> {
    match kind {
        MintKind::Cashu => cashu_id(value),
        MintKind::Fedimint => federation_id(value),
    }
}

fn network(value: &str) -> Result<EcashNetwork, DomainError> {
    match value {
        "mainnet" => Ok(EcashNetwork::Mainnet),
        "testnet" => Ok(EcashNetwork::Testnet),
        "signet" => Ok(EcashNetwork::Signet),
        "regtest" => Ok(EcashNetwork::Regtest),
        _ => Err(invalid(
            "an ecash network is mainnet, testnet, signet, or regtest",
        )),
    }
}

fn nuts(value: &str) -> Result<Vec<u16>, DomainError> {
    let mut found = Vec::new();
    for part in value.split(',') {
        if part.is_empty()
            || (part.len() > 1 && part.starts_with('0'))
            || !part.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(invalid("a nut list is comma-separated positive integers"));
        }
        let nut: u16 = part
            .parse()
            .map_err(|_| invalid("a nut list is comma-separated positive integers"))?;
        if nut == 0 || found.contains(&nut) {
            return Err(invalid("a nut list is comma-separated positive integers"));
        }
        found.push(nut);
    }
    Ok(found)
}

fn modules(value: &str) -> Result<Vec<String>, DomainError> {
    let mut found = Vec::new();
    for part in value.split(',') {
        if part.is_empty()
            || !part.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            })
            || found.iter().any(|seen: &String| seen == part)
        {
            return Err(invalid("a module list is comma-separated lowercase names"));
        }
        found.push(part.to_owned());
    }
    Ok(found)
}

fn metadata(content: &str) -> Result<Option<String>, DomainError> {
    if content.is_empty() {
        return Ok(None);
    }
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(value) if value.is_object() => Ok(Some(content.to_owned())),
        _ => Err(invalid("mint metadata is a JSON object")),
    }
}

fn endpoint_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn mint_endpoints(event: &Event, kind: MintKind) -> Result<Vec<String>, DomainError> {
    let mut endpoints = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("u")) {
        if tag.as_slice().len() != 2 {
            return Err(invalid("a mint endpoint is one value"));
        }
        let Some(value) = tag.value() else {
            return Err(invalid("a mint endpoint is one value"));
        };
        let ok = match kind {
            MintKind::Cashu => http_url(value),
            MintKind::Fedimint => endpoint_token(value),
        };
        if !ok {
            return Err(invalid(match kind {
                MintKind::Cashu => "a cashu endpoint is an http:// or https:// URL",
                MintKind::Fedimint => "a fedimint endpoint is an invite code without spaces",
            }));
        }
        endpoints.push(value.to_owned());
    }
    if endpoints.is_empty() {
        return Err(invalid("a mint lists a u endpoint"));
    }
    Ok(endpoints)
}

fn marker_kind(value: &str) -> Result<MintKind, DomainError> {
    match value {
        "cashu" => Ok(MintKind::Cashu),
        "fedimint" => Ok(MintKind::Fedimint),
        _ => Err(invalid("an ecash marker is cashu or fedimint")),
    }
}

fn open_announcement(event: &Event, kind: MintKind) -> Result<MintAnnouncement, DomainError> {
    if event.kind != kind.number() {
        return Err(invalid(match kind {
            MintKind::Cashu => "a cashu mint has kind 38172",
            MintKind::Fedimint => "a fedimint has kind 38173",
        }));
    }
    let identifier = identifier(kind, required(event, "d", "a mint has one d tag")?)?;
    let endpoints = mint_endpoints(event, kind)?;
    let network = network(required(event, "n", "a mint has one network")?)?;
    let (nuts, modules) = match kind {
        MintKind::Cashu => {
            if event.tags.iter().any(|tag| tag.name() == Some("modules")) {
                return Err(invalid("a cashu mint lists nuts"));
            }
            (
                Some(nuts(required(
                    event,
                    "nuts",
                    "a cashu mint has one nuts tag",
                )?)?),
                None,
            )
        }
        MintKind::Fedimint => {
            if event.tags.iter().any(|tag| tag.name() == Some("nuts")) {
                return Err(invalid("a fedimint lists modules"));
            }
            (
                None,
                Some(modules(required(
                    event,
                    "modules",
                    "a fedimint has one modules tag",
                )?)?),
            )
        }
    };
    Ok(MintAnnouncement {
        kind,
        identifier,
        endpoints,
        nuts,
        modules,
        network,
        metadata: metadata(&event.content)?,
    })
}

/// Read a kind `38172` Cashu mint announcement.
pub fn open_cashu_mint(event: &Event) -> Result<MintAnnouncement, DomainError> {
    open_announcement(event, MintKind::Cashu)
}

/// Read a kind `38173` Fedimint announcement.
pub fn open_fedimint(event: &Event) -> Result<MintAnnouncement, DomainError> {
    open_announcement(event, MintKind::Fedimint)
}

/// Read a kind `38000` recommendation of one Cashu mint or Fedimint.
pub fn open_mint_recommendation(event: &Event) -> Result<MintRecommendation, DomainError> {
    if event.kind != RECOMMEND_KIND {
        return Err(invalid("a mint recommendation has kind 38000"));
    }
    let mint_kind = MintKind::parse(required(event, "k", "a recommendation has one k tag")?)?;
    let identifier = identifier(
        mint_kind,
        required(event, "d", "a recommendation has one d tag")?,
    )?;
    let mut endpoints = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("u")) {
        let parts = tag.as_slice();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(invalid("a recommendation endpoint is a URL or invite code"));
        }
        if !endpoint_token(&parts[1]) {
            return Err(invalid("a recommendation endpoint is a URL or invite code"));
        }
        let marker = match parts.get(2) {
            None => None,
            Some(value) => {
                let parsed = marker_kind(value)?;
                if parsed != mint_kind {
                    return Err(invalid("a recommendation marker matches the mint kind"));
                }
                Some(parsed)
            }
        };
        endpoints.push(MintEndpoint {
            value: parts[1].clone(),
            marker,
        });
    }
    let mut mints = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("a")) {
        let parts = tag.as_slice();
        if parts.len() < 2 || parts.len() > 4 {
            return Err(invalid("a recommendation address uses the k kind"));
        }
        let address = ReplacementAddress::from_str(&parts[1])
            .map_err(|_| invalid("a recommendation address uses the k kind"))?;
        if address.kind != mint_kind.number() {
            return Err(invalid("a recommendation address uses the k kind"));
        }
        if address.identifier != identifier {
            return Err(invalid("a recommendation address uses the d identifier"));
        }
        let relay = match parts.get(2) {
            None => None,
            Some(value) if relay_url(value) => Some(value.clone()),
            Some(_) => return Err(invalid("a recommendation relay is ws:// or wss://")),
        };
        let marker = match parts.get(3) {
            None => None,
            Some(value) => {
                let parsed = marker_kind(value)?;
                if parsed != mint_kind {
                    return Err(invalid("a recommendation marker matches the mint kind"));
                }
                Some(parsed)
            }
        };
        mints.push(MintPointer {
            address,
            relay,
            marker,
        });
    }
    Ok(MintRecommendation {
        identifier,
        mint_kind,
        endpoints,
        mints,
        review: event.content.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"87".repeat(32)).unwrap()
    }

    #[test]
    fn a_mint_announcement_replaces_and_a_recommendation_names_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/87.md"
        ))
        .unwrap();
        assert!(text.contains("38173"));
        assert!(text.contains("38172"));
        assert!(text.contains("38000"));
        assert!(text.contains("nuts"));
        assert!(text.contains("modules"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "87.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "87.md")
        );

        let operator = signer();
        let federation = "signet-federation";
        let invite = "fed1exampleinvite";
        let announced = operator.sign(
            1_700_000_000,
            FEDIMINT_KIND,
            vec![
                Tag::new(vec!["d".into(), federation.into()]),
                Tag::new(vec!["u".into(), invite.into()]),
                Tag::new(vec!["u".into(), "fed1secondinvite".into()]),
                Tag::new(vec!["modules".into(), "lightning,wallet,mint".into()]),
                Tag::new(vec!["n".into(), "signet".into()]),
            ],
            r#"{"name":"Signet mint"}"#.into(),
        );
        announced.validate_structure().unwrap();
        assert_eq!(announced.class(), EventClass::Addressable);
        let mint = open_fedimint(&announced).unwrap();
        assert_eq!(mint.identifier, federation);
        assert_eq!(mint.endpoints, vec![invite, "fed1secondinvite"]);
        assert_eq!(
            mint.modules.as_deref(),
            Some(&["lightning".to_owned(), "wallet".into(), "mint".into()][..])
        );
        assert_eq!(mint.network, EcashNetwork::Signet);
        assert_eq!(mint.metadata.as_deref(), Some(r#"{"name":"Signet mint"}"#));

        let revised = operator.sign(
            1_700_000_100,
            FEDIMINT_KIND,
            vec![
                Tag::new(vec!["d".into(), federation.into()]),
                Tag::new(vec!["u".into(), invite.into()]),
                Tag::new(vec!["modules".into(), "lightning,wallet".into()]),
                Tag::new(vec!["n".into(), "signet".into()]),
            ],
            String::new(),
        );
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&announced, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
        assert!(open_fedimint(&revised).unwrap().metadata.is_none());

        let mint_pubkey = "cd".repeat(32);
        let cashu = operator.sign(
            1_700_000_200,
            CASHU_KIND,
            vec![
                Tag::new(vec!["d".into(), mint_pubkey.clone()]),
                Tag::new(vec!["u".into(), "https://cashu.example.com".into()]),
                Tag::new(vec!["nuts".into(), "1,2,3,4,5,6,7".into()]),
                Tag::new(vec!["n".into(), "mainnet".into()]),
            ],
            String::new(),
        );
        cashu.validate_structure().unwrap();
        let cashu_mint = open_cashu_mint(&cashu).unwrap();
        assert_eq!(cashu_mint.identifier, mint_pubkey);
        assert_ne!(cashu_mint.identifier, cashu.pubkey);
        assert_eq!(cashu_mint.nuts.as_deref(), Some(&[1, 2, 3, 4, 5, 6, 7][..]));
        assert_eq!(cashu_mint.network, EcashNetwork::Mainnet);
        assert_eq!(
            cashu_mint.endpoints,
            vec!["https://cashu.example.com".to_owned()]
        );

        let repeated = operator.sign(
            1_700_000_300,
            CASHU_KIND,
            vec![
                Tag::new(vec!["d".into(), mint_pubkey.clone()]),
                Tag::new(vec!["u".into(), "https://cashu.example.com".into()]),
                Tag::new(vec!["nuts".into(), "1,2,2".into()]),
                Tag::new(vec!["n".into(), "mainnet".into()]),
            ],
            String::new(),
        );
        assert!(repeated.validate_structure().is_err());
        let unknown_network = operator.sign(
            1_700_000_400,
            FEDIMINT_KIND,
            vec![
                Tag::new(vec!["d".into(), federation.into()]),
                Tag::new(vec!["u".into(), invite.into()]),
                Tag::new(vec!["modules".into(), "mint".into()]),
                Tag::new(vec!["n".into(), "mutinynet".into()]),
            ],
            "[]".into(),
        );
        assert!(unknown_network.validate_structure().is_err());

        let address = format!("38173:{}:{federation}", announced.pubkey);
        let recommendation = operator.sign(
            1_700_000_500,
            RECOMMEND_KIND,
            vec![
                Tag::new(vec!["k".into(), "38173".into()]),
                Tag::new(vec!["d".into(), federation.into()]),
                Tag::new(vec!["u".into(), invite.into(), "fedimint".into()]),
                Tag::new(vec![
                    "a".into(),
                    address,
                    "wss://relay.example".into(),
                    "fedimint".into(),
                ]),
            ],
            "I trust this mint with my life".into(),
        );
        recommendation.validate_structure().unwrap();
        assert_eq!(recommendation.class(), EventClass::Addressable);
        let named = open_mint_recommendation(&recommendation).unwrap();
        assert_eq!(named.mint_kind, MintKind::Fedimint);
        assert_eq!(named.identifier, federation);
        assert_eq!(named.review, "I trust this mint with my life");
        assert_eq!(named.endpoints[0].marker, Some(MintKind::Fedimint));
        assert_eq!(named.mints[0].address.kind, FEDIMINT_KIND);
        assert_eq!(named.mints[0].address.identifier, federation);
        assert_eq!(named.mints[0].address.pubkey, announced.pubkey);
        assert_eq!(named.mints[0].relay.as_deref(), Some("wss://relay.example"));

        let updated = operator.sign(
            1_700_000_600,
            RECOMMEND_KIND,
            vec![
                Tag::new(vec!["k".into(), "38173".into()]),
                Tag::new(vec!["d".into(), federation.into()]),
            ],
            "Still the mint I use".into(),
        );
        updated.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&recommendation, &updated),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let mismatched = operator.sign(
            1_700_000_700,
            RECOMMEND_KIND,
            vec![
                Tag::new(vec!["k".into(), "38173".into()]),
                Tag::new(vec!["d".into(), federation.into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("38173:{}:other-federation", announced.pubkey),
                    "wss://relay.example".into(),
                ]),
            ],
            String::new(),
        );
        assert!(mismatched.validate_structure().is_err());
    }
}
