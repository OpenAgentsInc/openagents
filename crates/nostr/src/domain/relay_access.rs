//! NIP-43 relay access declarations and admission requests.
//!
//! These parsers verify signatures and exact relay identity or request freshness.
//! They do not issue, redeem, revoke, or persist claims. A membership projection
//! alone is not exhaustive access authority; the user's kind-10010 declaration
//! and the relay's actual admission policy remain separate inputs. The removed
//! kind-28935 issuance flow is deliberately unsupported.

use super::hex::decode_lower_hex;
use super::{DomainError, Event, Tag};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayMember {
    pub pubkey: String,
    pub roles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RelayAccessDeclaration {
    Role {
        id: String,
        label: Option<String>,
        description: Option<String>,
        color: Option<f64>,
        order: Option<i64>,
    },
    Members(Vec<RelayMember>),
    Added(String),
    Removed(String),
}

/// A user-signed request, not an accepted claim or a membership grant.
#[derive(Clone, PartialEq, Eq)]
pub enum RelayAccessRequest {
    Join { claim: String },
    Leave,
}

impl std::fmt::Debug for RelayAccessRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Join { .. } => formatter.write_str("Join { claim: [redacted] }"),
            Self::Leave => formatter.write_str("Leave"),
        }
    }
}

/// Verify and read a role, membership list, or add/remove declaration.
///
/// `relay_self` must come from the caller's trusted NIP-11 identity association,
/// not from the candidate event itself. No membership is inferred from absence.
///
/// # Errors
///
/// Returns an error for a signature/identity mismatch, missing protection tag,
/// unsupported kind, or malformed required fields.
pub fn open_relay_access_declaration(
    event: &Event,
    relay_self: &str,
) -> Result<RelayAccessDeclaration, DomainError> {
    decode_lower_hex::<32>(relay_self, "relay self pubkey")?;
    event.validate_nip01_structure()?;
    event.validate_crypto()?;
    if event.pubkey != relay_self {
        return Err(invalid(
            "relay access declarations must be signed by the configured relay self key",
        ));
    }
    protected(event)?;
    match event.kind {
        33_534 => {
            let id = one_value(&event.tags, "d")?
                .ok_or_else(|| invalid("a relay role needs a d tag"))?;
            let color = one_value(&event.tags, "color")?
                .map(|value| {
                    let hue = value
                        .parse::<f64>()
                        .map_err(|_| invalid("a role hue must be between 0 and 360"))?;
                    if !hue.is_finite() || !(0.0..=360.0).contains(&hue) {
                        return Err(invalid("a role hue must be between 0 and 360"));
                    }
                    Ok(hue)
                })
                .transpose()?;
            let order = one_value(&event.tags, "order")?
                .map(|value| {
                    value
                        .parse::<i64>()
                        .map_err(|_| invalid("a role order must be an integer"))
                })
                .transpose()?;
            Ok(RelayAccessDeclaration::Role {
                id: id.into(),
                label: one_value(&event.tags, "label")?.map(str::to_owned),
                description: one_value(&event.tags, "description")?.map(str::to_owned),
                color,
                order,
            })
        }
        13_534 => {
            let mut members = Vec::new();
            for tag in event.tags.iter().filter(|tag| tag.name() == Some("member")) {
                let key = tag
                    .value()
                    .ok_or_else(|| invalid("a relay member tag needs a pubkey"))?;
                decode_lower_hex::<32>(key, "relay member")?;
                members.push(RelayMember {
                    pubkey: key.into(),
                    roles: tag.as_slice()[2..].to_vec(),
                });
            }
            Ok(RelayAccessDeclaration::Members(members))
        }
        8_000 | 8_001 => {
            let member = one_value(&event.tags, "p")?
                .ok_or_else(|| invalid("a relay membership change needs a p tag"))?;
            decode_lower_hex::<32>(member, "relay member")?;
            if event.kind == 8_000 {
                Ok(RelayAccessDeclaration::Added(member.into()))
            } else {
                Ok(RelayAccessDeclaration::Removed(member.into()))
            }
        }
        _ => Err(invalid("unsupported relay access declaration kind")),
    }
}

/// Verify a definite NIP-43 join/leave request under a caller-specified clock
/// tolerance, in seconds. The host enforces a bounded tolerance and separately
/// checks advertisement, claim validity, replay, and the original request ID.
///
/// # Errors
///
/// Returns an error for a malformed or unsigned request, a stale/future timestamp,
/// missing join claim, unprotected leave, or unsupported kind (including 28935).
pub fn open_relay_access_request(
    event: &Event,
    now: u64,
    tolerance_seconds: u64,
) -> Result<RelayAccessRequest, DomainError> {
    event.validate_nip01_structure()?;
    event.validate_crypto()?;
    if event.created_at.abs_diff(now) > tolerance_seconds {
        return Err(invalid(
            "a relay access request is outside the admitted clock tolerance",
        ));
    }
    match event.kind {
        28_934 => {
            let claim = one_value(&event.tags, "claim")?
                .filter(|claim| !claim.is_empty())
                .ok_or_else(|| invalid("a join request needs one nonempty claim"))?;
            Ok(RelayAccessRequest::Join {
                claim: claim.into(),
            })
        }
        28_936 => {
            protected(event)?;
            Ok(RelayAccessRequest::Leave)
        }
        _ => Err(invalid("unsupported relay access request kind")),
    }
}

fn protected(event: &Event) -> Result<(), DomainError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("-"))
        .collect();
    if tags.len() != 1 || tags[0].as_slice().len() != 1 {
        return Err(invalid(
            "a relay declaration or leave request needs one NIP-70 protection tag",
        ));
    }
    Ok(())
}

fn one_value<'a>(tags: &'a [Tag], name: &str) -> Result<Option<&'a str>, DomainError> {
    let mut matches = tags.iter().filter(|tag| tag.name() == Some(name));
    let Some(tag) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() || tag.as_slice().len() != 2 {
        return Err(invalid(
            "a relay access field must appear once with one value",
        ));
    }
    Ok(tag.value())
}
fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RelaySigner;

    #[test]
    fn declarations_require_the_pinned_relay_self_key_and_protection() {
        let relay = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        let member = RelaySigner::from_secret_hex(&"24".repeat(32)).unwrap();
        let role = relay.sign(
            100,
            33_534,
            vec![
                Tag::new(vec!["-".into()]),
                Tag::new(vec!["d".into(), "admin".into()]),
                Tag::new(vec!["color".into(), "37.5".into()]),
                Tag::new(vec!["order".into(), "-1".into()]),
            ],
            String::new(),
        );
        assert!(matches!(
            open_relay_access_declaration(&role, relay.pubkey()).unwrap(),
            RelayAccessDeclaration::Role {
                color: Some(37.5),
                order: Some(-1),
                ..
            }
        ));
        assert!(open_relay_access_declaration(&role, member.pubkey()).is_err());
        let mut tampered = role.clone();
        tampered.content = "changed".into();
        assert!(open_relay_access_declaration(&tampered, relay.pubkey()).is_err());
        for kind in [13_534, 8_000, 8_001] {
            let tag = if kind == 13_534 { "member" } else { "p" };
            let event = relay.sign(
                100,
                kind,
                vec![
                    Tag::new(vec!["-".into()]),
                    Tag::new(vec![tag.into(), member.pubkey().into()]),
                ],
                String::new(),
            );
            assert!(open_relay_access_declaration(&event, relay.pubkey()).is_ok());
            let unprotected = relay.sign(
                100,
                kind,
                vec![Tag::new(vec![tag.into(), member.pubkey().into()])],
                String::new(),
            );
            assert!(open_relay_access_declaration(&unprotected, relay.pubkey()).is_err());
        }
    }

    #[test]
    fn join_leave_freshness_and_removed_issuance_are_separate_from_acceptance() {
        let member = RelaySigner::from_secret_hex(&"24".repeat(32)).unwrap();
        let join = member.sign(
            100,
            28_934,
            vec![Tag::new(vec!["claim".into(), "fixture-only-invite".into()])],
            String::new(),
        );
        assert!(matches!(
            open_relay_access_request(&join, 130, 30).unwrap(),
            RelayAccessRequest::Join { .. }
        ));
        assert!(open_relay_access_request(&join, 131, 30).is_err());
        assert!(open_relay_access_request(&join, 69, 30).is_err());
        assert!(
            !format!("{:?}", open_relay_access_request(&join, 100, 30).unwrap())
                .contains("fixture-only-invite")
        );
        let leave = member.sign(100, 28_936, vec![Tag::new(vec!["-".into()])], String::new());
        assert_eq!(
            open_relay_access_request(&leave, 100, 30).unwrap(),
            RelayAccessRequest::Leave
        );
        for kind in [28_935, 28_936] {
            let bad = member.sign(100, kind, vec![], String::new());
            assert!(open_relay_access_request(&bad, 100, 30).is_err());
        }
    }
}
