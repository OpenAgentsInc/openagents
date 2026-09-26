//! Pure x402 v2 `exact` Lightning proof and request-binding verification.
//!
//! The baseline is x402 commit `4fcf836cc393174130e1358577ce5d37356da1c3`
//! and the separately opted-in native profile in NIP-X402. A valid proof is
//! neither wallet authority nor a consumed payment: the host must atomically
//! insert its replay key in durable shared storage before executing anything.
//! This module performs no payment, network request, storage, or execution.

mod invoice;
pub use invoice::{Invoice, decode_invoice};

use secp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::read_state_snapshot::hex_bytes;

pub const MAINNET: &str = "lnbtc:000000000019d6689c085ae165831e93";
pub const TESTNET: &str = "lnbtc:000000000933ea01ad0ee984209779ba";
pub const DEFAULT_CLOCK_SKEW: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentError {
    Terms,
    Network,
    Amount,
    Payee,
    Binding,
    Profile,
    InvoiceMissing,
    InvoiceDecode,
    InvoiceSignature,
    InvoiceDescription,
    InvoiceRequest,
    InvoicePayee,
    InvoiceCurrency,
    InvoiceAmount,
    InvoiceExpiry,
    InvoiceFuture,
    InvoiceExpired,
    Preimage,
    PreimageHash,
    Overflow,
}

/// The method's requirements object. Unknown server `extra` fields remain bound.
/// Decode untrusted wire bytes with [`PaymentRequirements::parse`], not a lossy
/// JSON-map decoder that discards duplicate fields before validation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub amount: String,
    pub asset: String,
    pub pay_to: String,
    pub max_timeout_seconds: u64,
    pub extra: Map<String, Value>,
}

impl PaymentRequirements {
    /// Parse bounded wire JSON with duplicate-key, Unicode, and numeric checks.
    /// This establishes shape only; challenge/proof validation checks the terms.
    pub fn parse(bytes: &[u8]) -> Result<Self, PaymentError> {
        let value = crate::contracts::parse_strict(bytes).map_err(|_| PaymentError::Terms)?;
        serde_json::from_value(value).map_err(|_| PaymentError::Terms)
    }
}

/// Transport roles explicitly implemented by the embedding service. Native is
/// disabled by default: upstream HTTP/MCP support does not imply native support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportedProfiles {
    pub http: bool,
    pub mcp: bool,
    pub native: bool,
}

impl Default for SupportedProfiles {
    fn default() -> Self {
        Self {
            http: true,
            mcp: true,
            native: false,
        }
    }
}

/// A stateless proof result. Do not execute until a durable atomic consume wins.
/// No preimage or invoice is retained in this public accounting shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPaymentProof {
    pub network: String,
    pub payment_hash: String,
    pub invoice_amount_msat: u64,
    pub consumption_key: String,
    /// Upstream minimum retention. Native hosts must extend it through their
    /// recovery contract and any unresolved payment or execution liability.
    pub retain_until: u64,
}

/// Verify the original accepted invoice and preimage against requirements that
/// the server reconstructed from the actual request and its own configuration.
/// A newly generated challenge invoice may differ; no other bound term may.
/// Any decoded inputs must have passed strict parsing before map construction.
pub fn validate_paid_proof(
    requirements: &PaymentRequirements,
    accepted: &PaymentRequirements,
    preimage: &str,
    settlement_time: u64,
    skew: u64,
    profiles: SupportedProfiles,
) -> Result<ValidatedPaymentProof, PaymentError> {
    if requirements.scheme != accepted.scheme
        || requirements.network != accepted.network
        || requirements.amount != accepted.amount
        || requirements.asset != accepted.asset
        || requirements.pay_to != accepted.pay_to
        || requirements.max_timeout_seconds != accepted.max_timeout_seconds
    {
        return Err(PaymentError::Terms);
    }
    validate_requirements(requirements, profiles)?;
    validate_requirements(accepted, profiles)?;
    for (key, value) in &requirements.extra {
        if matches!(key.as_str(), "invoice" | "assetTransferMethod") {
            continue;
        }
        let other = accepted.extra.get(key).ok_or(PaymentError::Terms)?;
        if canonical(value)? != canonical(other)? {
            return Err(PaymentError::Terms);
        }
    }
    let invoice = decode_invoice(extra_string(accepted, "invoice")?)?;
    validate_invoice_terms(&invoice, requirements, settlement_time, skew)?;
    let preimage = hex_bytes::<32>(preimage).map_err(|_| PaymentError::Preimage)?;
    if <[u8; 32]>::from(Sha256::digest(preimage)) != invoice.payment_hash {
        return Err(PaymentError::PreimageHash);
    }
    let invoice_end = invoice
        .created_at
        .checked_add(invoice.expiry_seconds)
        .ok_or(PaymentError::Overflow)?;
    let grace_end = invoice_end
        .checked_add(skew)
        .ok_or(PaymentError::Overflow)?;
    if settlement_time > grace_end {
        return Err(PaymentError::InvoiceExpired);
    }
    let payment_hash = hex(&invoice.payment_hash);
    Ok(ValidatedPaymentProof {
        network: requirements.network.clone(),
        consumption_key: format!("{}:{payment_hash}", requirements.network),
        payment_hash,
        invoice_amount_msat: invoice.amount_msat,
        retain_until: grace_end.checked_add(3600).ok_or(PaymentError::Overflow)?,
    })
}

/// Validate a challenge before dispatching any wallet payment. Unlike paid
/// settlement, challenge validation grants no grace after invoice expiry.
pub fn validate_challenge(
    requirements: &PaymentRequirements,
    expected_request_hash: &str,
    now: u64,
    skew: u64,
    profiles: SupportedProfiles,
) -> Result<Invoice, PaymentError> {
    validate_requirements(requirements, profiles)?;
    hex_bytes::<32>(expected_request_hash).map_err(|_| PaymentError::Binding)?;
    if extra_string(requirements, "requestHash")? != expected_request_hash {
        return Err(PaymentError::Binding);
    }
    let invoice = decode_invoice(extra_string(requirements, "invoice")?)?;
    validate_invoice_terms(&invoice, requirements, now, skew)?;
    if now
        >= invoice
            .created_at
            .checked_add(invoice.expiry_seconds)
            .ok_or(PaymentError::Overflow)?
    {
        return Err(PaymentError::InvoiceExpired);
    }
    Ok(invoice)
}

fn validate_requirements(
    value: &PaymentRequirements,
    profiles: SupportedProfiles,
) -> Result<(), PaymentError> {
    if value.scheme != "exact" || value.asset != "BTC" || value.max_timeout_seconds == 0 {
        return Err(PaymentError::Terms);
    }
    currency(&value.network)?;
    decimal(&value.amount)?;
    let payee = hex_bytes::<33>(&value.pay_to).map_err(|_| PaymentError::Payee)?;
    if !matches!(payee[0], 2 | 3) || PublicKey::from_slice(&payee).is_err() {
        return Err(PaymentError::Payee);
    }
    if value
        .extra
        .get("assetTransferMethod")
        .is_some_and(|value| value.as_str() != Some("bolt11"))
        || extra_string(value, "paymentFlow")? != "upfront"
    {
        return Err(PaymentError::Terms);
    }
    hex_bytes::<32>(extra_string(value, "requestHash")?).map_err(|_| PaymentError::Binding)?;
    let params = value
        .extra
        .get("requestBindingParams")
        .ok_or(PaymentError::Profile)?;
    validate_profile(
        extra_string(value, "requestBindingProfile")?,
        params,
        profiles,
    )?;
    if extra_string(value, "invoice")?.is_empty() {
        return Err(PaymentError::InvoiceMissing);
    }
    Ok(())
}

fn validate_invoice_terms(
    invoice: &Invoice,
    requirements: &PaymentRequirements,
    now: u64,
    skew: u64,
) -> Result<(), PaymentError> {
    if invoice.description_hash
        != hex_bytes::<32>(extra_string(requirements, "requestHash")?)
            .map_err(|_| PaymentError::Binding)?
    {
        return Err(PaymentError::InvoiceRequest);
    }
    if invoice.payee.serialize()
        != hex_bytes::<33>(&requirements.pay_to).map_err(|_| PaymentError::Payee)?
    {
        return Err(PaymentError::InvoicePayee);
    }
    if invoice.currency != currency(&requirements.network)? {
        return Err(PaymentError::InvoiceCurrency);
    }
    if invoice.amount_msat != decimal(&requirements.amount)? {
        return Err(PaymentError::InvoiceAmount);
    }
    if invoice.expiry_seconds != requirements.max_timeout_seconds {
        return Err(PaymentError::InvoiceExpiry);
    }
    if invoice.created_at > now.checked_add(skew).ok_or(PaymentError::Overflow)? {
        return Err(PaymentError::InvoiceFuture);
    }
    Ok(())
}

fn extra_string<'a>(
    requirements: &'a PaymentRequirements,
    key: &str,
) -> Result<&'a str, PaymentError> {
    requirements
        .extra
        .get(key)
        .and_then(Value::as_str)
        .ok_or(if key == "invoice" {
            PaymentError::InvoiceMissing
        } else {
            PaymentError::Terms
        })
}
fn currency(network: &str) -> Result<&'static str, PaymentError> {
    match network {
        MAINNET => Ok("bc"),
        TESTNET => Ok("tb"),
        _ => Err(PaymentError::Network),
    }
}
fn decimal(value: &str) -> Result<u64, PaymentError> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PaymentError::Amount);
    }
    let amount = value.parse::<u64>().map_err(|_| PaymentError::Amount)?;
    if amount == 0 {
        return Err(PaymentError::Amount);
    }
    Ok(amount)
}
fn canonical(value: &Value) -> Result<Vec<u8>, PaymentError> {
    // Round-trip through the strict parser also rejects unsafe integer Values
    // supplied directly by a caller rather than decoded from strict JSON.
    let bytes = serde_json::to_vec(value).map_err(|_| PaymentError::Binding)?;
    let value = crate::contracts::parse_strict(&bytes).map_err(|_| PaymentError::Binding)?;
    crate::contracts::jcs(&value).map_err(|_| PaymentError::Binding)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn validate_profile(
    profile: &str,
    params: &Value,
    supported: SupportedProfiles,
) -> Result<(), PaymentError> {
    let object = params.as_object().ok_or(PaymentError::Profile)?;
    match profile {
        "http:1" if supported.http && object.len() == 1 => {
            header_names(object.get("headers").ok_or(PaymentError::Profile)?)?;
        }
        "mcp:1" if supported.mcp && object.len() == 2 => {
            absolute_uri(
                object
                    .get("server")
                    .and_then(Value::as_str)
                    .ok_or(PaymentError::Profile)?,
                false,
            )?;
            metadata_names(object.get("metadata").ok_or(PaymentError::Profile)?)?;
        }
        "nostr:openagents:1" if supported.native && object.is_empty() => {}
        _ => return Err(PaymentError::Profile),
    }
    Ok(())
}

fn field_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}
fn header_names(value: &Value) -> Result<Vec<&str>, PaymentError> {
    let names = value
        .as_array()
        .ok_or(PaymentError::Profile)?
        .iter()
        .map(|name| name.as_str().ok_or(PaymentError::Profile))
        .collect::<Result<Vec<_>, _>>()?;
    if names.iter().any(|name| {
        !field_token(name)
            || name.bytes().any(|byte| byte.is_ascii_uppercase())
            || *name == "payment-signature"
    }) || names.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(PaymentError::Profile);
    }
    Ok(names)
}
fn metadata_names(value: &Value) -> Result<Vec<&str>, PaymentError> {
    let names = value
        .as_array()
        .ok_or(PaymentError::Profile)?
        .iter()
        .map(|name| name.as_str().ok_or(PaymentError::Profile))
        .collect::<Result<Vec<_>, _>>()?;
    if names
        .iter()
        .any(|name| name.is_empty() || matches!(*name, "x402/payment" | "progressToken"))
        || names.windows(2).any(|pair| {
            pair[0].encode_utf16().cmp(pair[1].encode_utf16()) != std::cmp::Ordering::Less
        })
    {
        return Err(PaymentError::Profile);
    }
    Ok(names)
}

fn absolute_uri(uri: &str, http_only: bool) -> Result<(), PaymentError> {
    if !uri.is_ascii()
        || !uri
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~:/?[]@!$&'()*+,;=%".contains(&byte))
    {
        return Err(PaymentError::Binding);
    }
    let (scheme, rest) = uri.split_once(':').ok_or(PaymentError::Binding)?;
    if scheme.is_empty()
        || !scheme.as_bytes()[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(&byte))
        || rest.is_empty()
        || (http_only
            && !scheme.eq_ignore_ascii_case("http")
            && !scheme.eq_ignore_ascii_case("https"))
    {
        return Err(PaymentError::Binding);
    }
    if let Some(authority) = rest.strip_prefix("//") {
        let authority_end = authority.find(['/', '?']).unwrap_or(authority.len());
        if authority[authority_end..].contains(['[', ']']) {
            return Err(PaymentError::Binding);
        }
        let authority = &authority[..authority_end];
        if authority.is_empty() || authority.contains('@') {
            return Err(PaymentError::Binding);
        }
        if let Some(literal) = authority.strip_prefix('[') {
            let (address, suffix) = literal.split_once(']').ok_or(PaymentError::Binding)?;
            address
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| PaymentError::Binding)?;
            if !suffix.is_empty() {
                validate_port(suffix.strip_prefix(':').ok_or(PaymentError::Binding)?)?;
            }
        } else {
            if authority.contains(['[', ']']) {
                return Err(PaymentError::Binding);
            }
            if let Some((name, port)) = authority.split_once(':') {
                if name.is_empty() {
                    return Err(PaymentError::Binding);
                }
                validate_port(port)?;
            }
        }
    } else if scheme.eq_ignore_ascii_case("http")
        || scheme.eq_ignore_ascii_case("https")
        || rest.contains(['[', ']'])
    {
        return Err(PaymentError::Binding);
    }
    let bytes = uri.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && (index + 2 >= bytes.len()
                || !bytes[index + 1..=index + 2]
                    .iter()
                    .all(u8::is_ascii_hexdigit))
        {
            return Err(PaymentError::Binding);
        }
    }
    Ok(())
}

fn validate_port(port: &str) -> Result<(), PaymentError> {
    if port.is_empty()
        || !port.bytes().all(|byte| byte.is_ascii_digit())
        || port.parse::<u16>().is_err()
    {
        return Err(PaymentError::Binding);
    }
    Ok(())
}

/// Construct the HTTP binding from actual bytes and already canonicalized
/// RFC9421 field-component values. The caller must derive them from all raw
/// occurrences, bind the configured public origin, and reject hidden context.
/// `headers` must include each configured name, with None for an absent field.
pub fn http_binding(
    method: &str,
    url: &str,
    body: &[u8],
    headers: &[(String, Option<String>)],
) -> Result<Value, PaymentError> {
    if !field_token(method) {
        return Err(PaymentError::Binding);
    }
    absolute_uri(url, true)?;
    header_names(&serde_json::json!(
        headers.iter().map(|(name, _)| name).collect::<Vec<_>>()
    ))?;
    let mut bound = Vec::new();
    for (name, value) in headers {
        let hash = match value {
            None => digest(&[0]),
            Some(value) => {
                if !value.is_ascii()
                    || value
                        .bytes()
                        .any(|byte| (byte < 0x20 && byte != b'\t') || byte == 0x7f)
                    || value.starts_with([' ', '\t'])
                    || value.ends_with([' ', '\t'])
                {
                    return Err(PaymentError::Binding);
                }
                let mut bytes = vec![1];
                bytes.extend_from_slice(value.as_bytes());
                digest(&bytes)
            }
        };
        bound.push(serde_json::json!({"name":name,"valueHash":hash}));
    }
    Ok(
        serde_json::json!({"domain":"x402:exact:lnbtc:bolt11:http:1","method":method,"url":url,"bodyHash":digest(body),"headers":bound}),
    )
}

/// Construct an MCP tools/call binding from the configured server identity and
/// unmodified params. Payment/progress metadata and JSON-RPC framing stay out.
/// The caller must reject duplicate original JSON members before creating this
/// Value. Use [`mcp_binding_from_bytes`] when processing untrusted wire bytes.
pub fn mcp_binding(
    server: &str,
    params: &Value,
    configured_metadata: &[String],
) -> Result<Value, PaymentError> {
    absolute_uri(server, false)?;
    let names_value = serde_json::json!(configured_metadata);
    let names = metadata_names(&names_value)?;
    let object = params.as_object().ok_or(PaymentError::Binding)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(PaymentError::Binding)?;
    let empty = serde_json::json!({});
    let arguments = object.get("arguments").unwrap_or(&empty);
    if !arguments.is_object() {
        return Err(PaymentError::Binding);
    }
    canonical(arguments)?;
    let metadata = object
        .get("_meta")
        .unwrap_or(&empty)
        .as_object()
        .ok_or(PaymentError::Binding)?;
    let mut bound = Vec::new();
    for name in names {
        let hash = match metadata.get(name) {
            None => digest(&[0]),
            Some(value) => {
                let mut bytes = vec![1];
                bytes.extend(canonical(value)?);
                digest(&bytes)
            }
        };
        bound.push(serde_json::json!({"name":name,"valueHash":hash}));
    }
    Ok(
        serde_json::json!({"domain":"x402:exact:lnbtc:bolt11:mcp:1","server":server,"method":"tools/call","name":name,"arguments":arguments,"metadata":bound}),
    )
}

/// Strictly decode raw MCP params before constructing their request binding.
pub fn mcp_binding_from_bytes(
    server: &str,
    params: &[u8],
    configured_metadata: &[String],
) -> Result<Value, PaymentError> {
    let params = crate::contracts::parse_strict(params).map_err(|_| PaymentError::Binding)?;
    mcp_binding(server, &params, configured_metadata)
}

/// Construct the separate native profile from an already authenticated request
/// artifact. This hashes identities; it does not authenticate that artifact.
pub fn native_binding(
    buyer: &str,
    provider: &str,
    purchase: &str,
    request_digest: &str,
) -> Result<Value, PaymentError> {
    let buyer_bytes = hex_bytes::<32>(buyer).map_err(|_| PaymentError::Binding)?;
    let provider_bytes = hex_bytes::<32>(provider).map_err(|_| PaymentError::Binding)?;
    secp256k1::XOnlyPublicKey::from_byte_array(buyer_bytes).map_err(|_| PaymentError::Binding)?;
    secp256k1::XOnlyPublicKey::from_byte_array(provider_bytes)
        .map_err(|_| PaymentError::Binding)?;
    if buyer == provider {
        return Err(PaymentError::Binding);
    }
    hex_bytes::<32>(purchase).map_err(|_| PaymentError::Binding)?;
    hex_bytes::<32>(
        request_digest
            .strip_prefix("sha256:")
            .ok_or(PaymentError::Binding)?,
    )
    .map_err(|_| PaymentError::Binding)?;
    Ok(
        serde_json::json!({"domain":"x402:exact:lnbtc:bolt11:nostr:openagents:1","buyer":buyer,"provider":provider,"purchase":purchase,"requestDigest":request_digest}),
    )
}

/// Hash constructed binding bytes with RFC8785 ordering and numeric rules.
/// This function alone validates no profile or request provenance.
pub fn binding_hash(binding: &Value) -> Result<String, PaymentError> {
    Ok(digest(&canonical(binding)?))
}

#[cfg(test)]
mod tests;
