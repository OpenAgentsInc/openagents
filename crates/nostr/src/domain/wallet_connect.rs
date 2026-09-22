//! NIP-47 Nostr Wallet Connect.
//!
//! A `nostr+walletconnect://` URI names the wallet pubkey, the relays,
//! and the client secret. Kind `13194` is the wallet service's
//! replaceable capability list. Kind `23194` is an encrypted request.
//! Kind `23195` is the encrypted response and names the request id.
//!
//! `nip44_v2` is preferred when the info event offers it. A missing
//! encryption tag means NIP-04. The relay does not pay invoices, and it
//! does not store the ephemeral request and response events. Metadata
//! objects are not interpreted. NIP-47 is a draft, so these kinds stay
//! off the NIP-11 list.

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const INFO_KIND: u16 = 13_194;
const REQUEST_KIND: u16 = 23_194;
const RESPONSE_KIND: u16 = 23_195;
const CORE_METHODS: &[&str] = &[
    "pay_invoice",
    "make_invoice",
    "lookup_invoice",
    "get_balance",
    "get_info",
];
const ERROR_CODES: &[&str] = &[
    "RATE_LIMITED",
    "NOT_IMPLEMENTED",
    "INSUFFICIENT_BALANCE",
    "QUOTA_EXCEEDED",
    "RESTRICTED",
    "UNAUTHORIZED",
    "INTERNAL",
    "UNSUPPORTED_ENCRYPTION",
    "PAYMENT_FAILED",
    "NOT_FOUND",
    "OTHER",
];

/// How a request or response is encrypted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalletEncryption {
    Nip44V2,
    Nip04,
}

/// A parsed `nostr+walletconnect://` URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletConnection {
    pub wallet_pubkey: String,
    pub relays: Vec<String>,
    pub secret: String,
    pub client_pubkey: String,
    pub lud16: Option<String>,
}

/// A kind `13194` capability advertisement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletInfo {
    pub methods: Vec<String>,
    pub encryption: Vec<WalletEncryption>,
    pub extensions: Vec<String>,
}

/// Public fields of a kind `23194` request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletRequest {
    pub wallet_pubkey: String,
    pub encryption: WalletEncryption,
    pub expiration: Option<u64>,
}

/// A decrypted wallet command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalletCommand {
    PayInvoice {
        invoice: String,
        amount_msat: Option<u64>,
    },
    MakeInvoice {
        amount_msat: u64,
        description: Option<String>,
        description_hash: Option<String>,
        expiry_seconds: Option<u64>,
    },
    LookupInvoice {
        payment_hash: Option<String>,
        invoice: Option<String>,
    },
    GetBalance,
    GetInfo,
    Extension {
        method: String,
    },
}

/// Public fields of a kind `23195` response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletResponse {
    pub client_pubkey: String,
    pub request_id: String,
}

/// A typed wallet error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletError {
    pub code: String,
    pub message: String,
}

/// A decrypted wallet response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletReply {
    pub result_type: String,
    pub error: Option<WalletError>,
    pub preimage: Option<String>,
    pub fees_paid: Option<u64>,
    pub balance_msat: Option<u64>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn xonly(secret: &SecretKey) -> XOnlyPublicKey {
    Keypair::from_secret_key(&Secp256k1::signing_only(), secret)
        .x_only_public_key()
        .0
}

fn pubkey_hex(secret_hex: &str) -> Result<String, DomainError> {
    let bytes = decode_lower_hex::<32>(secret_hex, "wallet key")
        .map_err(|_| invalid("a wallet key is 32 lowercase hex bytes"))?;
    let secret = SecretKey::from_byte_array(bytes)
        .map_err(|_| invalid("a wallet key is 32 lowercase hex bytes"))?;
    Ok(xonly(&secret).to_string())
}

fn percent_decode(value: &str) -> Result<String, DomainError> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(invalid("a connection query value is percent-encoded"));
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| invalid("a connection query value is percent-encoded"))?;
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| invalid("a connection query value is percent-encoded"))?;
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|_| invalid("a connection query value is percent-encoded"))
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn relay_url(value: &str) -> bool {
    (value.starts_with("wss://") || value.starts_with("ws://"))
        && value.len() > "wss://".len()
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn method_name(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
        && value.len() <= 64
}

fn extension_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn lud16(value: &str) -> bool {
    let Some((name, domain)) = value.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !value.chars().any(char::is_whitespace)
        && value.len() <= 320
}

fn encryption_name(value: &str) -> Result<WalletEncryption, DomainError> {
    match value {
        "nip44_v2" => Ok(WalletEncryption::Nip44V2),
        "nip04" => Ok(WalletEncryption::Nip04),
        _ => Err(invalid("wallet encryption is nip44_v2 or nip04")),
    }
}

fn one_tag<'a>(event: &'a Event, name: &str) -> Result<Option<&'a super::Tag>, DomainError> {
    let mut found = event.tags.iter().filter(|tag| tag.name() == Some(name));
    let tag = found.next();
    if found.next().is_some() {
        return Err(invalid("a wallet event repeats a tag"));
    }
    Ok(tag)
}

fn pubkey_tag(event: &Event) -> Result<String, DomainError> {
    let Some(tag) = one_tag(event, "p")? else {
        return Err(invalid("a wallet request names one pubkey"));
    };
    let Some(value) = tag.value() else {
        return Err(invalid("a wallet request names one pubkey"));
    };
    decode_lower_hex::<32>(value, "wallet pubkey")
        .map_err(|_| invalid("a wallet request names one pubkey"))?;
    if value == event.pubkey {
        return Err(invalid("a wallet request names the other party"));
    }
    Ok(value.to_owned())
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

fn nip04_shape(content: &str) -> bool {
    let Some((body, iv)) = content.split_once("?iv=") else {
        return false;
    };
    !body.is_empty()
        && !iv.is_empty()
        && !iv.contains("?iv=")
        && content
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'?'))
}

fn ciphertext(content: &str, encryption: WalletEncryption) -> Result<(), DomainError> {
    let ok = match encryption {
        WalletEncryption::Nip44V2 => crate::nip44::payload_shape(content).is_ok(),
        WalletEncryption::Nip04 => nip04_shape(content),
    };
    if ok {
        Ok(())
    } else {
        Err(invalid("wallet content matches its encryption tag"))
    }
}

/// Read a connection URI.
pub fn parse_connection(uri: &str) -> Result<WalletConnection, DomainError> {
    let Some(rest) = uri.strip_prefix("nostr+walletconnect://") else {
        return Err(invalid("a wallet connection uses nostr+walletconnect://"));
    };
    let Some((wallet_pubkey, query)) = rest.split_once('?') else {
        return Err(invalid("a wallet connection lists a relay and a secret"));
    };
    decode_lower_hex::<32>(wallet_pubkey, "wallet pubkey")
        .map_err(|_| invalid("a wallet pubkey is 32 lowercase hex bytes"))?;
    let mut relays = Vec::new();
    let mut secret = None;
    let mut lightning = None;
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(invalid("a connection query value is percent-encoded"));
        };
        let value = percent_decode(value)?;
        match key {
            "relay" => {
                if !relay_url(&value) || relays.iter().any(|seen| seen == &value) {
                    return Err(invalid("a connection relay is ws:// or wss://"));
                }
                relays.push(value);
            }
            "secret" => {
                if secret.is_some() {
                    return Err(invalid("a connection has one secret"));
                }
                decode_lower_hex::<32>(&value, "connection secret")
                    .map_err(|_| invalid("a connection secret is 32 lowercase hex bytes"))?;
                secret = Some(value);
            }
            "lud16" => {
                if lightning.is_some() || !lud16(&value) {
                    return Err(invalid("a connection lud16 is a lightning address"));
                }
                lightning = Some(value);
            }
            _ => return Err(invalid("a connection query is relay, secret, or lud16")),
        }
    }
    let Some(secret) = secret else {
        return Err(invalid("a connection has one secret"));
    };
    if relays.is_empty() {
        return Err(invalid("a connection lists a relay"));
    }
    let client_pubkey = pubkey_hex(&secret)?;
    if client_pubkey == wallet_pubkey {
        return Err(invalid("a connection secret is a separate client key"));
    }
    Ok(WalletConnection {
        wallet_pubkey: wallet_pubkey.to_owned(),
        relays,
        secret,
        client_pubkey,
        lud16: lightning,
    })
}

/// Write a connection URI.
pub fn format_connection(connection: &WalletConnection) -> String {
    let mut query = connection
        .relays
        .iter()
        .map(|relay| format!("relay={}", percent_encode(relay)))
        .collect::<Vec<_>>();
    query.push(format!("secret={}", percent_encode(&connection.secret)));
    if let Some(address) = &connection.lud16 {
        query.push(format!("lud16={}", percent_encode(address)));
    }
    format!(
        "nostr+walletconnect://{}?{}",
        connection.wallet_pubkey,
        query.join("&")
    )
}

/// Read a kind `13194` info event.
pub fn open_wallet_info(event: &Event) -> Result<WalletInfo, DomainError> {
    if event.kind != INFO_KIND {
        return Err(invalid("a wallet info event has kind 13194"));
    }
    let mut methods = Vec::new();
    for method in event.content.split_whitespace() {
        if !method_name(method) || methods.iter().any(|seen| seen == method) {
            return Err(invalid("wallet info lists method names"));
        }
        methods.push(method.to_owned());
    }
    if methods.is_empty() {
        return Err(invalid("wallet info lists method names"));
    }
    let encryption = match one_tag(event, "encryption")? {
        None => Vec::new(),
        Some(tag) => {
            let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
                return Err(invalid("wallet encryption is nip44_v2 or nip04"));
            };
            let mut schemes = Vec::new();
            for name in value.split_whitespace() {
                let scheme = encryption_name(name)?;
                if schemes.contains(&scheme) {
                    return Err(invalid("wallet encryption is listed once"));
                }
                schemes.push(scheme);
            }
            if schemes.is_empty() {
                return Err(invalid("wallet encryption is nip44_v2 or nip04"));
            }
            schemes
        }
    };
    let extensions = match one_tag(event, "extensions")? {
        None => Vec::new(),
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("a wallet extension name is lowercase"));
            };
            let mut names = Vec::new();
            for name in value.split_whitespace() {
                if !extension_name(name) || names.iter().any(|seen| seen == name) {
                    return Err(invalid("a wallet extension name is lowercase"));
                }
                names.push(name.to_owned());
            }
            if names.is_empty() {
                return Err(invalid("a wallet extension name is lowercase"));
            }
            names
        }
    };
    Ok(WalletInfo {
        methods,
        encryption,
        extensions,
    })
}

/// Prefer `nip44_v2`. A missing tag means NIP-04.
pub fn negotiated_encryption(info: &WalletInfo) -> WalletEncryption {
    if info.encryption.contains(&WalletEncryption::Nip44V2) {
        WalletEncryption::Nip44V2
    } else {
        WalletEncryption::Nip04
    }
}

/// Whether `info` offers `requested`.
pub fn encryption_supported(info: &WalletInfo, requested: WalletEncryption) -> bool {
    if info.encryption.is_empty() {
        requested == WalletEncryption::Nip04
    } else {
        info.encryption.contains(&requested)
    }
}

/// Read the public fields of a kind `23194` request.
pub fn open_wallet_request(event: &Event) -> Result<WalletRequest, DomainError> {
    if event.kind != REQUEST_KIND {
        return Err(invalid("a wallet request has kind 23194"));
    }
    let encryption = match one_tag(event, "encryption")? {
        None => WalletEncryption::Nip04,
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("wallet encryption is nip44_v2 or nip04"));
            };
            if value.split_whitespace().nth(1).is_some() {
                return Err(invalid("a request names one encryption scheme"));
            }
            encryption_name(value)?
        }
    };
    ciphertext(&event.content, encryption)?;
    let expiration = match one_tag(event, "expiration")? {
        None => None,
        Some(tag) => Some(whole(
            tag.value()
                .ok_or_else(|| invalid("a request expiration is unix seconds"))?,
            "a request expiration is unix seconds",
        )?),
    };
    Ok(WalletRequest {
        wallet_pubkey: pubkey_tag(event)?,
        encryption,
        expiration,
    })
}

/// A request at `expiration` still counts. A later receipt does not.
pub fn request_is_current(request: &WalletRequest, now: u64) -> bool {
    request
        .expiration
        .is_none_or(|expiration| now <= expiration)
}

fn json_u64(value: &serde_json::Value, reason: &str) -> Result<u64, DomainError> {
    value
        .as_u64()
        .filter(|amount| *amount > 0)
        .ok_or_else(|| invalid(reason))
}

fn optional_object<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<&'a serde_json::Map<String, serde_json::Value>>, DomainError> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_object()
            .map(Some)
            .ok_or_else(|| invalid("wallet metadata is an object")),
    }
}

fn bolt11(invoice: &str) -> Result<Option<u64>, DomainError> {
    super::zap::bolt11_amount_msat(invoice)
        .map_err(|_| invalid("a pay invoice is a bolt11 invoice"))
}

/// Read a decrypted request.
pub fn read_wallet_command(plaintext: &str) -> Result<WalletCommand, DomainError> {
    let value: serde_json::Value =
        serde_json::from_str(plaintext).map_err(|_| invalid("a wallet command is JSON"))?;
    let Some(object) = value.as_object() else {
        return Err(invalid("a wallet command is JSON"));
    };
    if object.keys().any(|key| key != "method" && key != "params") {
        return Err(invalid("a wallet command is a method and params"));
    }
    let Some(method) = object.get("method").and_then(serde_json::Value::as_str) else {
        return Err(invalid("a wallet command names a method"));
    };
    if !method_name(method) {
        return Err(invalid("a wallet command names a method"));
    }
    let Some(params) = object.get("params").and_then(serde_json::Value::as_object) else {
        return Err(invalid("a wallet command has a params object"));
    };
    if CORE_METHODS.contains(&method) {
        for key in params.keys() {
            let known = match method {
                "pay_invoice" => matches!(key.as_str(), "invoice" | "amount" | "metadata"),
                "make_invoice" => matches!(
                    key.as_str(),
                    "amount" | "description" | "description_hash" | "expiry" | "metadata"
                ),
                "lookup_invoice" => matches!(key.as_str(), "payment_hash" | "invoice"),
                "get_balance" | "get_info" => false,
                _ => false,
            };
            if !known {
                return Err(invalid("a wallet command uses the method's params"));
            }
        }
    }
    optional_object(params, "metadata")?;
    match method {
        "pay_invoice" => {
            let Some(invoice) = params.get("invoice").and_then(serde_json::Value::as_str) else {
                return Err(invalid("a pay invoice is a bolt11 invoice"));
            };
            let invoice_amount = bolt11(invoice)?;
            let amount_msat = match params.get("amount") {
                None => None,
                Some(value) => Some(json_u64(value, "a pay invoice amount is millisats")?),
            };
            if let (Some(invoice_amount), Some(amount_msat)) = (invoice_amount, amount_msat)
                && invoice_amount != amount_msat
            {
                return Err(invalid("a pay invoice amount matches the bolt11 invoice"));
            }
            Ok(WalletCommand::PayInvoice {
                invoice: invoice.to_owned(),
                amount_msat,
            })
        }
        "make_invoice" => {
            let Some(amount) = params.get("amount") else {
                return Err(invalid("a made invoice has an amount in millisats"));
            };
            let description_hash = match params.get("description_hash") {
                None => None,
                Some(value) => {
                    let Some(hash) = value.as_str() else {
                        return Err(invalid("a description hash is 32 lowercase hex bytes"));
                    };
                    decode_lower_hex::<32>(hash, "description hash")
                        .map_err(|_| invalid("a description hash is 32 lowercase hex bytes"))?;
                    Some(hash.to_owned())
                }
            };
            let expiry_seconds = match params.get("expiry") {
                None => None,
                Some(value) => Some(json_u64(
                    value,
                    "an invoice expiry is a positive number of seconds",
                )?),
            };
            let description = match params.get("description") {
                None => None,
                Some(value) => {
                    let Some(text) = value.as_str().filter(|text| !text.is_empty()) else {
                        return Err(invalid("an invoice description is text"));
                    };
                    Some(text.to_owned())
                }
            };
            Ok(WalletCommand::MakeInvoice {
                amount_msat: json_u64(amount, "a made invoice has an amount in millisats")?,
                description,
                description_hash,
                expiry_seconds,
            })
        }
        "lookup_invoice" => {
            let payment_hash = match params.get("payment_hash") {
                None => None,
                Some(value) => {
                    let Some(hash) = value.as_str() else {
                        return Err(invalid("a payment hash is 32 lowercase hex bytes"));
                    };
                    decode_lower_hex::<32>(hash, "payment hash")
                        .map_err(|_| invalid("a payment hash is 32 lowercase hex bytes"))?;
                    Some(hash.to_owned())
                }
            };
            let invoice = match params.get("invoice") {
                None => None,
                Some(value) => {
                    let Some(invoice) = value.as_str() else {
                        return Err(invalid("a lookup invoice is a bolt11 invoice"));
                    };
                    bolt11(invoice)?;
                    Some(invoice.to_owned())
                }
            };
            if payment_hash.is_none() && invoice.is_none() {
                return Err(invalid("a lookup names a payment hash or an invoice"));
            }
            Ok(WalletCommand::LookupInvoice {
                payment_hash,
                invoice,
            })
        }
        "get_balance" => Ok(WalletCommand::GetBalance),
        "get_info" => Ok(WalletCommand::GetInfo),
        _ => Ok(WalletCommand::Extension {
            method: method.to_owned(),
        }),
    }
}

/// Read the public fields of a kind `23195` response.
pub fn open_wallet_response(event: &Event) -> Result<WalletResponse, DomainError> {
    if event.kind != RESPONSE_KIND {
        return Err(invalid("a wallet response has kind 23195"));
    }
    if !nip04_shape(&event.content) && crate::nip44::payload_shape(&event.content).is_err() {
        return Err(invalid("a wallet response is encrypted"));
    }
    let Some(tag) = one_tag(event, "e")? else {
        return Err(invalid("a wallet response names the request"));
    };
    let Some(request_id) = tag.value() else {
        return Err(invalid("a wallet response names the request"));
    };
    decode_lower_hex::<32>(request_id, "request id")
        .map_err(|_| invalid("a wallet response names the request"))?;
    Ok(WalletResponse {
        client_pubkey: pubkey_tag(event)?,
        request_id: request_id.to_owned(),
    })
}

/// Read a decrypted response.
pub fn read_wallet_reply(plaintext: &str) -> Result<WalletReply, DomainError> {
    let value: serde_json::Value =
        serde_json::from_str(plaintext).map_err(|_| invalid("a wallet reply is JSON"))?;
    let Some(object) = value.as_object() else {
        return Err(invalid("a wallet reply is JSON"));
    };
    let Some(result_type) = object
        .get("result_type")
        .and_then(serde_json::Value::as_str)
    else {
        return Err(invalid("a wallet reply names its result type"));
    };
    if !method_name(result_type) {
        return Err(invalid("a wallet reply names its result type"));
    }
    let error = object.get("error").filter(|value| !value.is_null());
    let result = object.get("result").filter(|value| !value.is_null());
    match (error, result) {
        (None, Some(result)) => {
            let Some(result) = result.as_object() else {
                return Err(invalid("a wallet result is an object"));
            };
            let (preimage, fees_paid, balance_msat) = match result_type {
                "pay_invoice" => {
                    let Some(preimage) = result.get("preimage").and_then(serde_json::Value::as_str)
                    else {
                        return Err(invalid("a payment preimage is 32 lowercase hex bytes"));
                    };
                    decode_lower_hex::<32>(preimage, "preimage")
                        .map_err(|_| invalid("a payment preimage is 32 lowercase hex bytes"))?;
                    let fees_paid = match result.get("fees_paid") {
                        None => None,
                        Some(value) => Some(
                            value
                                .as_u64()
                                .ok_or_else(|| invalid("fees paid are a number of millisats"))?,
                        ),
                    };
                    (Some(preimage.to_owned()), fees_paid, None)
                }
                "get_balance" => {
                    let Some(balance) = result.get("balance").and_then(serde_json::Value::as_u64)
                    else {
                        return Err(invalid("a balance is a number of millisats"));
                    };
                    (None, None, Some(balance))
                }
                _ => (None, None, None),
            };
            Ok(WalletReply {
                result_type: result_type.to_owned(),
                error: None,
                preimage,
                fees_paid,
                balance_msat,
            })
        }
        (Some(error), None) => {
            let Some(error) = error.as_object() else {
                return Err(invalid("a wallet error has a code and a message"));
            };
            let Some(code) = error.get("code").and_then(serde_json::Value::as_str) else {
                return Err(invalid("a wallet error has a code and a message"));
            };
            if !ERROR_CODES.contains(&code) {
                return Err(invalid("a wallet error uses a known code"));
            }
            let Some(message) = error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .filter(|message| !message.is_empty())
            else {
                return Err(invalid("a wallet error has a message"));
            };
            Ok(WalletReply {
                result_type: result_type.to_owned(),
                error: Some(WalletError {
                    code: code.to_owned(),
                    message: message.to_owned(),
                }),
                preimage: None,
                fees_paid: None,
                balance_msat: None,
            })
        }
        _ => Err(invalid("a wallet reply has a result or an error")),
    }
}

/// Encrypt `plaintext` from `secret` to `peer`.
pub fn seal_wallet_message(
    secret: &SecretKey,
    peer: &XOnlyPublicKey,
    encryption: WalletEncryption,
    plaintext: &str,
    nonce: [u8; 32],
) -> Result<String, DomainError> {
    match encryption {
        WalletEncryption::Nip44V2 => {
            let key = crate::nip44::conversation_key(secret, peer);
            crate::nip44::encrypt(plaintext, &key, nonce)
                .map_err(|_| invalid("wallet content matches its encryption tag"))
        }
        WalletEncryption::Nip04 => {
            let mut iv = [0_u8; 16];
            iv.copy_from_slice(&nonce[..16]);
            crate::nip04::encrypt(plaintext, secret, peer, iv)
                .map_err(|_| invalid("wallet content matches its encryption tag"))
        }
    }
}

/// Decrypt a wallet message addressed to `secret`.
pub fn read_wallet_message(
    secret: &SecretKey,
    peer: &XOnlyPublicKey,
    encryption: WalletEncryption,
    payload: &str,
) -> Result<String, DomainError> {
    match encryption {
        WalletEncryption::Nip44V2 => {
            let key = crate::nip44::conversation_key(secret, peer);
            crate::nip44::decrypt(payload, &key)
                .map_err(|_| invalid("wallet content matches its encryption tag"))
        }
        WalletEncryption::Nip04 => crate::nip04::decrypt(payload, secret, peer)
            .map_err(|_| invalid("wallet content matches its encryption tag")),
    }
}

#[cfg(test)]
mod tests {
    use super::decode_lower_hex;
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    const INVOICE: &str = "lnbc10u1p3unwfusp5t9r3yymhpfqculx78u027lxspgxcr2n2987mx2j55nnfs95nxnzqpp5jmrh92pfld78spqs78v9euf2385t83uvpwk9ldrlvf6ch7tpascqhp5zvkrmemgth3tufcvflmzjzfvjt023nazlhljz2n9hattj4f8jq8qxqyjw5qcqpjrzjqtc4fc44feggv7065fqe5m4ytjarg3repr5j9el35xhmtfexc42yczarjuqqfzqqqqqqqqlgqqqqqqgq9q9qxpqysgq079nkq507a5tw7xgttmj4u990j7wfggtrasah5gd4ywfr2pjcn29383tphp4t48gquelz9z78p4cq7ml3nrrphw5w6eckhjwmhezhnqpy6gyf0";

    fn signer(byte: &str) -> (RelaySigner, SecretKey) {
        let secret =
            SecretKey::from_byte_array(decode_lower_hex::<32>(&byte.repeat(32), "key").unwrap())
                .unwrap();
        let signer = RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap();
        (signer, secret)
    }

    #[test]
    fn a_pay_invoice_round_trips_and_an_info_event_replaces() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/47.md"
        ))
        .unwrap();
        assert!(text.contains("23194"));
        assert!(text.contains("23195"));
        assert!(text.contains("13194"));
        assert!(text.contains("nostr+walletconnect://"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "47.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "47.md")
        );

        let pinned = text
            .lines()
            .find(|line| line.starts_with("nostr+walletconnect://"))
            .unwrap();
        let pinned = parse_connection(pinned).unwrap();
        assert_eq!(pinned.relays, vec!["wss://relay.damus.io".to_owned()]);
        assert_eq!(
            pinned.wallet_pubkey,
            "b889ff5b1513b641e2a139f661a661364979c5beee91842f8f0ef42ab558e9d4"
        );
        assert_ne!(pinned.client_pubkey, pinned.wallet_pubkey);
        assert_eq!(
            parse_connection(&format_connection(&pinned)).unwrap(),
            pinned
        );

        let (wallet, wallet_secret) = signer("47");
        let (client, client_secret) = signer("48");
        let connection = parse_connection(&format_connection(&WalletConnection {
            wallet_pubkey: wallet.pubkey().to_owned(),
            relays: vec!["wss://relay.example".to_owned()],
            secret: "48".repeat(32),
            client_pubkey: client.pubkey().to_owned(),
            lud16: Some("alice@example.com".to_owned()),
        }))
        .unwrap();
        assert_eq!(connection.client_pubkey, client.pubkey());
        assert_eq!(connection.lud16.as_deref(), Some("alice@example.com"));

        let info = wallet.sign(
            1_700_000_000,
            INFO_KIND,
            vec![
                Tag::new(vec!["encryption".into(), "nip44_v2 nip04".into()]),
                Tag::new(vec!["extensions".into(), "02 03".into()]),
            ],
            "pay_invoice get_balance make_invoice lookup_invoice get_info".into(),
        );
        info.validate_structure().unwrap();
        assert_eq!(info.class(), EventClass::Replaceable);
        let opened = open_wallet_info(&info).unwrap();
        assert_eq!(negotiated_encryption(&opened), WalletEncryption::Nip44V2);
        assert!(encryption_supported(&opened, WalletEncryption::Nip44V2));
        assert_eq!(opened.extensions, vec!["02".to_owned(), "03".into()]);

        let replaced = wallet.sign(1_700_000_100, INFO_KIND, Vec::new(), "get_balance".into());
        replaced.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&info, &replaced),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
        let legacy = open_wallet_info(&replaced).unwrap();
        assert_eq!(negotiated_encryption(&legacy), WalletEncryption::Nip04);
        assert!(!encryption_supported(&legacy, WalletEncryption::Nip44V2));

        let command = format!(
            r#"{{"method":"pay_invoice","params":{{"invoice":"{INVOICE}","amount":1000000}}}}"#
        );
        let wallet_key = xonly(&wallet_secret);
        let sealed = seal_wallet_message(
            &client_secret,
            &wallet_key,
            WalletEncryption::Nip44V2,
            &command,
            [0x47; 32],
        )
        .unwrap();
        let request = client.sign(
            1_700_000_200,
            REQUEST_KIND,
            vec![
                Tag::new(vec!["encryption".into(), "nip44_v2".into()]),
                Tag::new(vec!["p".into(), wallet.pubkey().to_owned()]),
                Tag::new(vec!["expiration".into(), "1700000300".into()]),
            ],
            sealed,
        );
        request.validate_structure().unwrap();
        assert_eq!(request.class(), EventClass::Ephemeral);
        let opened_request = open_wallet_request(&request).unwrap();
        assert_eq!(opened_request.wallet_pubkey, wallet.pubkey());
        assert_eq!(opened_request.encryption, WalletEncryption::Nip44V2);
        assert!(request_is_current(&opened_request, 1_700_000_300));
        assert!(!request_is_current(&opened_request, 1_700_000_301));
        let client_key = xonly(&client_secret);
        let plain = read_wallet_message(
            &wallet_secret,
            &client_key,
            WalletEncryption::Nip44V2,
            &request.content,
        )
        .unwrap();
        match read_wallet_command(&plain).unwrap() {
            WalletCommand::PayInvoice {
                invoice,
                amount_msat,
            } => {
                assert_eq!(invoice, INVOICE);
                assert_eq!(amount_msat, Some(1_000_000));
            }
            other => panic!("expected pay_invoice, got {other:?}"),
        }
        assert!(matches!(
            compare_replacement(&request, &request),
            Err(DomainError::NotReplaceable)
        ));

        let preimage = "ab".repeat(32);
        let reply_plain = format!(
            r#"{{"result_type":"pay_invoice","error":null,"result":{{"preimage":"{preimage}","fees_paid":123}}}}"#
        );
        let reply_sealed = seal_wallet_message(
            &wallet_secret,
            &client_key,
            WalletEncryption::Nip44V2,
            &reply_plain,
            [0x48; 32],
        )
        .unwrap();
        let response = wallet.sign(
            1_700_000_400,
            RESPONSE_KIND,
            vec![
                Tag::new(vec!["p".into(), client.pubkey().to_owned()]),
                Tag::new(vec!["e".into(), request.id.clone()]),
            ],
            reply_sealed,
        );
        response.validate_structure().unwrap();
        assert_eq!(response.class(), EventClass::Ephemeral);
        let opened_response = open_wallet_response(&response).unwrap();
        assert_eq!(opened_response.request_id, request.id);
        assert_eq!(opened_response.client_pubkey, client.pubkey());
        let reply = read_wallet_reply(
            &read_wallet_message(
                &client_secret,
                &wallet_key,
                WalletEncryption::Nip44V2,
                &response.content,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(reply.result_type, "pay_invoice");
        assert_eq!(reply.preimage.as_deref(), Some(preimage.as_str()));
        assert_eq!(reply.fees_paid, Some(123));
        assert!(reply.error.is_none());

        let balance = read_wallet_command(r#"{"method":"get_balance","params":{}}"#).unwrap();
        assert!(matches!(balance, WalletCommand::GetBalance));
        let made = read_wallet_command(
            r#"{"method":"make_invoice","params":{"amount":1000,"description":"coffee"}}"#,
        )
        .unwrap();
        assert!(matches!(
            made,
            WalletCommand::MakeInvoice {
                amount_msat: 1000,
                ..
            }
        ));
        let error = read_wallet_reply(
            r#"{"result_type":"pay_invoice","error":{"code":"UNSUPPORTED_ENCRYPTION","message":"nip44 is required"},"result":null}"#,
        )
        .unwrap();
        assert_eq!(error.error.unwrap().code, "UNSUPPORTED_ENCRYPTION");

        let clear = client.sign(
            1_700_000_500,
            REQUEST_KIND,
            vec![
                Tag::new(vec!["encryption".into(), "nip44_v2".into()]),
                Tag::new(vec!["p".into(), wallet.pubkey().to_owned()]),
            ],
            "not ciphertext".into(),
        );
        assert!(clear.validate_structure().is_err());
    }
}
