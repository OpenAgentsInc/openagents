//! Closed NIP-CTRL v1 artifacts. Parsing establishes shape, never admission.
//!
//! Hosts still authenticate original signers, resolve exact referenced bytes,
//! establish owner/controller relationships, and enforce current scope, rights,
//! deadlines, disclosure, revocation, and durable replay before any effect.

use std::collections::BTreeSet;
use std::str::FromStr;

use secp256k1::XOnlyPublicKey;
use serde_json::{Map, Value};

use crate::contracts::{self, ContractError, RefusalCode};

pub const INVITATION: &str = "openagents.control-invitation.v1";
pub const PAIRING: &str = "openagents.control-pairing.v1";
pub const GRANT: &str = "openagents.control-grant.v1";
pub const ACCESS_RESULT: &str = "openagents.control-access-result.v1";
pub const REVOKE: &str = "openagents.control-revoke.v1";
pub const REVOCATION: &str = "openagents.control-revocation.v1";
pub const COMMAND: &str = "openagents.task-command.v1";
pub const COMMAND_RESULT: &str = "openagents.task-command-result.v1";
pub const COMMAND_RECEIPT: &str = "openagents.task-command-receipt.v1";
pub const READ: &str = "openagents.task-read.v1";
pub const VIEW: &str = "openagents.task-view.v1";
pub const PROJECTION: &str = "openagents.task-projection.v1";

type Result<T> = std::result::Result<T, ContractError>;

/// Parse a bounded, duplicate-free artifact under the shared JSON limits.
///
/// # Errors
/// Refuses unsupported versions, required features, fields, and malformed values.
pub fn parse(bytes: &[u8]) -> Result<Value> {
    let value = contracts::parse_strict(bytes)?;
    validate_shape(&value)?;
    Ok(value)
}

/// Validate a host-produced value under the same limits and closed profile.
///
/// Referenced bytes and signature/authority relationships are not resolved here.
///
/// # Errors
/// Refuses values that cannot be serialized as a valid bounded CTRL artifact.
pub fn validate(value: &Value) -> Result<()> {
    let mut budget = contracts::MAX_BODY_BYTES;
    limits(value, 0, &mut budget)?;
    let bytes = contracts::jcs(value)?;
    let strict = contracts::parse_strict(&bytes)?;
    validate_shape(&strict)
}

fn malformed(field: &str) -> ContractError {
    ContractError::new(RefusalCode::Malformed, field)
}
fn limits(value: &Value, depth: usize, remaining: &mut usize) -> Result<()> {
    if depth > 64 {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "control depth",
        ));
    }
    let charge = match value {
        Value::String(text) => text.len() + 2,
        Value::Object(map) => map.keys().map(|key| key.len() + 3).sum::<usize>() + 2,
        Value::Array(items) => items.len() + 2,
        _ => 1,
    };
    *remaining = remaining
        .checked_sub(charge)
        .ok_or_else(|| ContractError::new(RefusalCode::LimitExceeded, "control bytes"))?;
    match value {
        Value::Object(map) => {
            for item in map.values() {
                limits(item, depth + 1, remaining)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                limits(item, depth + 1, remaining)?;
            }
        }
        _ => (),
    }
    Ok(())
}
fn closed<'a>(value: &'a Value, fields: &[&str], artifact: bool) -> Result<&'a Map<String, Value>> {
    let map = value
        .as_object()
        .ok_or_else(|| malformed("control object"))?;
    if fields.iter().any(|field| !map.contains_key(*field)) {
        return Err(malformed("missing control field"));
    }
    if map.keys().any(|field| {
        !fields.contains(&field.as_str())
            && !(artifact && matches!(field.as_str(), "v" | "requires" | "meta"))
    }) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "unknown control field",
        ));
    }
    if artifact {
        match map.get("requires").and_then(Value::as_array) {
            Some(features) if features.is_empty() => (),
            Some(_) => {
                return Err(ContractError::new(
                    RefusalCode::UnsupportedFeature,
                    "control requires",
                ));
            }
            None => return Err(malformed("control requires")),
        }
    }
    Ok(map)
}
fn integer(value: &Value, field: &str) -> Result<u64> {
    value
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| malformed(field))
}
fn text<'a>(value: &'a Value, field: &str, maximum: usize) -> Result<&'a str> {
    value
        .as_str()
        .filter(|s| s.len() <= maximum)
        .ok_or_else(|| malformed(field))
}
fn common_id(value: &Value, field: &str) -> Result<()> {
    let id = text(value, field, 64)?;
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(malformed(field));
    }
    Ok(())
}
fn pubkey(value: &Value, field: &str) -> Result<()> {
    common_id(value, field)?;
    XOnlyPublicKey::from_str(value.as_str().ok_or_else(|| malformed(field))?)
        .map_err(|_| malformed(field))?;
    Ok(())
}
fn reference(value: &Value, schema: Option<&str>) -> Result<()> {
    let r = contracts::parse_artifact(value)?;
    if schema.is_some_and(|s| r.schema.as_deref() != Some(s)) {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "control reference schema",
        ));
    }
    Ok(())
}
fn enumeration<'a>(value: &'a Value, choices: &[&str], field: &str) -> Result<&'a str> {
    let value = value.as_str().ok_or_else(|| malformed(field))?;
    if !choices.contains(&value) {
        return Err(ContractError::new(RefusalCode::UnsupportedFeature, field));
    }
    Ok(value)
}
fn scope(value: &Value) -> Result<()> {
    closed(value, &["task", "controller", "generation"], false)?;
    common_id(&value["task"], "scope.task")?;
    pubkey(&value["controller"], "scope.controller")?;
    integer(&value["generation"], "scope.generation")?;
    Ok(())
}
fn rights(value: &Value) -> Result<()> {
    let list = value.as_array().ok_or_else(|| malformed("rights"))?;
    if list.is_empty() || list.len() > 3 {
        return Err(malformed("rights"));
    }
    let mut seen = BTreeSet::new();
    for right in list {
        let right = enumeration(right, &["observe", "steer", "cancel"], "right")?;
        if !seen.insert(right) {
            return Err(malformed("duplicate right"));
        }
    }
    Ok(())
}
fn window(value: &Value) -> Result<()> {
    let issued = integer(&value["issued_at"], "issued_at")?;
    let expires = integer(&value["expires_at"], "expires_at")?;
    if expires <= issued {
        return Err(malformed("control expiry window"));
    }
    Ok(())
}
fn principals(value: &Value) -> Result<()> {
    for key in ["owner", "authority", "client"] {
        pubkey(&value[key], key)?;
    }
    scope(&value["scope"])?;
    if value["authority"] == value["client"] || value["scope"]["controller"] != value["authority"] {
        return Err(malformed("control principals or scope"));
    }
    rights(&value["rights"])?;
    reference(&value["policy"], None)?;
    window(value)
}
fn zero(value: &Value) -> Result<()> {
    if integer(value, "epoch")? != 0 {
        return Err(malformed("active control epoch"));
    }
    Ok(())
}
fn reason(value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    enumeration(
        value,
        &[
            "malformed",
            "unsupported_version",
            "unsupported_feature",
            "not_admitted",
            "unavailable",
            "content_unavailable",
            "identity_mismatch",
            "incompatible",
            "revoked",
            "stale",
            "cannot_enforce",
            "limit_exceeded",
            "idempotency_conflict",
            "conflict",
        ],
        "control refusal code",
    )?;
    Ok(())
}
fn cursor(value: &Value) -> Result<()> {
    if !value.is_null() {
        text(value, "cursor", 1024)?;
    }
    Ok(())
}
fn references(value: &Value, unique: bool) -> Result<()> {
    let list = value
        .as_array()
        .ok_or_else(|| malformed("control reference list"))?;
    let mut seen = BTreeSet::new();
    for item in list {
        let r = contracts::parse_artifact(item)?;
        if unique && !seen.insert((r.digest, r.size, r.media_type, r.schema)) {
            return Err(malformed("duplicate control reference"));
        }
    }
    Ok(())
}
fn validate_shape(value: &Value) -> Result<()> {
    match value["v"].as_str() {
        Some(INVITATION) => {
            closed(
                value,
                &[
                    "invitation",
                    "challenge",
                    "owner",
                    "authority",
                    "client",
                    "scope",
                    "rights",
                    "policy",
                    "issued_at",
                    "expires_at",
                ],
                true,
            )?;
            common_id(&value["invitation"], "invitation")?;
            common_id(&value["challenge"], "challenge")?;
            if value["invitation"] == value["challenge"] {
                return Err(malformed("independent invitation and challenge IDs"));
            }
            principals(value)?;
        }
        Some(PAIRING) => {
            closed(
                value,
                &["invitation", "client", "challenge", "rights", "accepted"],
                true,
            )?;
            reference(&value["invitation"], Some(INVITATION))?;
            pubkey(&value["client"], "client")?;
            common_id(&value["challenge"], "challenge")?;
            rights(&value["rights"])?;
            if !value["accepted"].is_boolean() {
                return Err(malformed("accepted"));
            }
        }
        Some(GRANT) => {
            closed(
                value,
                &[
                    "grant",
                    "epoch",
                    "owner",
                    "authority",
                    "client",
                    "scope",
                    "rights",
                    "policy",
                    "invitation",
                    "pairing",
                    "admission",
                    "issued_at",
                    "expires_at",
                ],
                true,
            )?;
            common_id(&value["grant"], "grant")?;
            zero(&value["epoch"])?;
            principals(value)?;
            reference(&value["invitation"], Some(INVITATION))?;
            reference(&value["pairing"], Some(PAIRING))?;
            reference(&value["admission"], None)?;
        }
        Some(ACCESS_RESULT) => {
            closed(value, &["request", "status", "access", "reason"], true)?;
            reference(&value["request"], None)?;
            let status = enumeration(
                &value["status"],
                &["granted", "revoked", "duplicate", "refused"],
                "access status",
            )?;
            reason(&value["reason"])?;
            match status {
                "refused" if value["access"].is_null() && !value["reason"].is_null() => (),
                "refused" => return Err(malformed("refused access result")),
                "granted" => reference(&value["access"], Some(GRANT))?,
                "revoked" => reference(&value["access"], Some(REVOCATION))?,
                _ => {
                    let r = contracts::parse_artifact(&value["access"])?;
                    if !matches!(r.schema.as_deref(), Some(GRANT | REVOCATION)) {
                        return Err(malformed("duplicate access artifact"));
                    }
                }
            }
        }
        Some(REVOKE) => {
            closed(value, &["request", "grant", "reason"], true)?;
            common_id(&value["request"], "request")?;
            reference(&value["grant"], Some(GRANT))?;
            text(&value["reason"], "revocation reason", 1024)?;
        }
        Some(REVOCATION) => {
            closed(
                value,
                &[
                    "grant",
                    "epoch",
                    "authority",
                    "request",
                    "authorization",
                    "revoked_at",
                ],
                true,
            )?;
            reference(&value["grant"], Some(GRANT))?;
            if integer(&value["epoch"], "epoch")? != 1 {
                return Err(malformed("terminal revocation epoch"));
            }
            pubkey(&value["authority"], "authority")?;
            reference(&value["request"], Some(REVOKE))?;
            reference(&value["authorization"], None)?;
            integer(&value["revoked_at"], "revoked_at")?;
        }
        Some(COMMAND) => command(value)?,
        Some(COMMAND_RESULT) => {
            closed(value, &["command", "status", "receipt", "reason"], true)?;
            reference(&value["command"], Some(COMMAND))?;
            let status = enumeration(
                &value["status"],
                &["accepted", "duplicate", "conflict", "refused", "unknown"],
                "command status",
            )?;
            reason(&value["reason"])?;
            if !value["receipt"].is_null() || matches!(status, "accepted" | "duplicate") {
                reference(&value["receipt"], Some(COMMAND_RECEIPT))?;
            }
            if matches!(status, "conflict" | "refused") && value["reason"].is_null() {
                return Err(malformed("command refusal reason"));
            }
        }
        Some(COMMAND_RECEIPT) => {
            closed(
                value,
                &["command", "authority", "admitted_at", "disposition"],
                true,
            )?;
            reference(&value["command"], Some(COMMAND))?;
            pubkey(&value["authority"], "authority")?;
            integer(&value["admitted_at"], "admitted_at")?;
            enumeration(
                &value["disposition"],
                &["correction_recorded", "cancel_requested"],
                "command disposition",
            )?;
        }
        Some(READ) => read(value)?,
        Some(VIEW) => view(value)?,
        Some(PROJECTION) => {
            closed(value, &["content", "sources", "reason", "coverage"], true)?;
            let content = contracts::parse_artifact(&value["content"])?;
            if content.schema.is_none() {
                return Err(malformed("projection display schema"));
            }
            references(&value["sources"], false)?;
            enumeration(
                &value["reason"],
                &["redacted", "unavailable", "unverifiable", "bounded"],
                "projection reason",
            )?;
            enumeration(
                &value["coverage"],
                &["partial", "unknown"],
                "projection coverage",
            )?;
        }
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedVersion,
                "control schema",
            ));
        }
    }
    Ok(())
}
fn command(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "command",
            "grant",
            "epoch",
            "scope",
            "expected_revision",
            "issued_at",
            "expires_at",
            "action",
            "payload",
        ],
        true,
    )?;
    common_id(&value["command"], "command")?;
    reference(&value["grant"], Some(GRANT))?;
    zero(&value["epoch"])?;
    scope(&value["scope"])?;
    integer(&value["expected_revision"], "expected_revision")?;
    window(value)?;
    let payload = &value["payload"];
    match enumeration(&value["action"], &["steer", "cancel"], "control action")? {
        "steer" => {
            closed(payload, &["message", "replaces"], false)?;
            let message = contracts::parse_artifact(&payload["message"])?;
            if message.size > contracts::MAX_BODY_BYTES as u64 {
                return Err(ContractError::new(
                    RefusalCode::LimitExceeded,
                    "steering message bytes",
                ));
            }
            references(&payload["replaces"], true)?;
        }
        _ => {
            closed(payload, &["reason"], false)?;
            text(&payload["reason"], "cancellation reason", 1024)?;
        }
    }
    Ok(())
}
fn read(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "request",
            "grant",
            "epoch",
            "scope",
            "view",
            "after",
            "max_items",
            "max_bytes",
        ],
        true,
    )?;
    common_id(&value["request"], "request")?;
    reference(&value["grant"], Some(GRANT))?;
    zero(&value["epoch"])?;
    scope(&value["scope"])?;
    let view = enumeration(&value["view"], &["state", "history"], "read view")?;
    cursor(&value["after"])?;
    if view == "state" && !value["after"].is_null() {
        return Err(malformed("state read cursor"));
    }
    if !(1..=256).contains(&integer(&value["max_items"], "max_items")?)
        || !(1..=1_048_576).contains(&integer(&value["max_bytes"], "max_bytes")?)
    {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "read bounds",
        ));
    }
    Ok(())
}
fn view(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "request",
            "authority",
            "scope",
            "captured_at",
            "policy",
            "items",
            "next",
            "coverage",
        ],
        true,
    )?;
    reference(&value["request"], Some(READ))?;
    pubkey(&value["authority"], "authority")?;
    scope(&value["scope"])?;
    if value["scope"]["controller"] != value["authority"] {
        return Err(malformed("view authority differs from scope"));
    }
    integer(&value["captured_at"], "captured_at")?;
    reference(&value["policy"], None)?;
    cursor(&value["next"])?;
    enumeration(
        &value["coverage"],
        &["complete", "partial", "unknown"],
        "view coverage",
    )?;
    let items = value["items"]
        .as_array()
        .ok_or_else(|| malformed("view items"))?;
    if items.len() > 256 {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "view items"));
    }
    for item in items {
        closed(item, &["kind", "artifact", "provenance"], false)?;
        let kind = enumeration(
            &item["kind"],
            &[
                "frame",
                "run_record",
                "trace",
                "usage",
                "finding",
                "projection",
            ],
            "view item kind",
        )?;
        let expected = match kind {
            "frame" => Some("openagents.task-frame.v1"),
            "run_record" => Some(crate::run::RECORD_SCHEMA),
            "finding" => Some("openagents.finding.v1"),
            "projection" => Some(PROJECTION),
            _ => None,
        };
        reference(&item["artifact"], expected)?;
        if contracts::parse_artifact(&item["artifact"])?
            .schema
            .is_none()
        {
            return Err(malformed("view item schema"));
        }
        reference(&item["provenance"], Some("openagents.evidence.v1"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
