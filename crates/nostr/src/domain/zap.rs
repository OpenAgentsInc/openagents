//! NIP-57 lightning zaps.
//!
//! Kind `9734` is a zap request. Kind `9735` is a zap receipt whose
//! `description` is that request and whose `bolt11` invoice commits to
//! the same description hash and, when both name an amount, the same
//! number of millisatoshis. `zap_split` divides an amount across `zap`
//! tags. `zap_callback_query` builds the LNURL callback query.
//!
//! The relay does not call LNURL and does not pay invoices. A bolt11
//! signature and its expiry are not checked. The receipt pubkey is not
//! compared with a provider `nostrPubkey`, because that profile is not
//! fetched. These kinds are not added to the NIP-11 list.

use std::str::FromStr;

use sha2::{Digest, Sha256};

use super::hex::decode_lower_hex;
#[cfg(test)]
use super::hex::encode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const REQUEST_KIND: u16 = 9734;
const RECEIPT_KIND: u16 = 9735;
const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const GENERATOR: [u32; 5] = [
    0x3b6a_57b2,
    0x2650_8e6d,
    0x1ea1_19fa,
    0x3d42_33dd,
    0x2a14_62b3,
];
const PAYMENT_HASH: u8 = 1;
const DESCRIPTION_HASH: u8 = 23;

/// A kind `9734` zap request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZapRequest {
    pub sender: String,
    pub recipient: String,
    pub content: String,
    pub amount_msat: Option<u64>,
    pub lnurl: Option<String>,
    pub relays: Vec<String>,
    pub event_id: Option<String>,
    pub address: Option<ReplacementAddress>,
    pub target_kind: Option<u16>,
    pub receipt_pubkey: Option<String>,
}

/// A kind `9735` zap receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZapReceipt {
    pub sender: String,
    pub recipient: String,
    pub amount_msat: Option<u64>,
    pub bolt11: String,
    pub preimage: Option<String>,
    pub request: ZapRequest,
}

/// One receiver in a `zap` tag split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZapShare {
    pub pubkey: String,
    pub relay: String,
    pub weight: u64,
    pub millisats: u64,
}

struct Invoice {
    amount_msat: Option<u64>,
    description_hash: Option<[u8; 32]>,
    payment_hash: Option<[u8; 32]>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
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

fn pubkey(value: &str, reason: &'static str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, reason).map_err(|_| invalid(reason))?;
    Ok(value.to_owned())
}

fn tags<'a>(event: &'a Event, name: &str) -> Vec<&'a super::Tag> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect()
}

fn millisats(value: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(invalid("a zap amount is millisatoshis"));
    }
    value
        .parse()
        .map_err(|_| invalid("a zap amount is millisatoshis"))
}

fn optional_amount(event: &Event) -> Result<Option<u64>, DomainError> {
    let found = tags(event, "amount");
    if found.len() > 1 {
        return Err(invalid("a zap request has one amount"));
    }
    match found.first() {
        None => Ok(None),
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("a zap amount is millisatoshis"));
            };
            Ok(Some(millisats(value)?))
        }
    }
}

fn one_pubkey(event: &Event, name: &str, reason: &'static str) -> Result<String, DomainError> {
    let found = tags(event, name);
    if found.len() != 1 {
        return Err(invalid(reason));
    }
    let Some(value) = found[0].value() else {
        return Err(invalid(reason));
    };
    pubkey(value, reason)
}

fn optional_pubkey(
    event: &Event,
    name: &str,
    reason: &'static str,
) -> Result<Option<String>, DomainError> {
    let found = tags(event, name);
    if found.len() > 1 {
        return Err(invalid(reason));
    }
    match found.first() {
        None => Ok(None),
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid(reason));
            };
            Ok(Some(pubkey(value, reason)?))
        }
    }
}

fn optional_event_id(event: &Event) -> Result<Option<String>, DomainError> {
    let found = tags(event, "e");
    if found.len() > 1 {
        return Err(invalid("a zap request has one event id"));
    }
    match found.first() {
        None => Ok(None),
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("a zap request has one event id"));
            };
            decode_lower_hex::<32>(value, "zap event")
                .map_err(|_| invalid("a zap request has one event id"))?;
            if let Some(relay) = tag.as_slice().get(2)
                && !relay.is_empty()
                && !is_relay(relay)
            {
                return Err(invalid("a zap relay hint is ws:// or wss://"));
            }
            Ok(Some(value.to_owned()))
        }
    }
}

fn optional_address(event: &Event) -> Result<Option<ReplacementAddress>, DomainError> {
    let found = tags(event, "a");
    if found.len() > 1 {
        return Err(invalid("a zap request has one event address"));
    }
    match found.first() {
        None => Ok(None),
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("a zap request has one event address"));
            };
            ReplacementAddress::from_str(value)
                .map(Some)
                .map_err(|_| invalid("a zap request has one event address"))
        }
    }
}

fn optional_kind(event: &Event) -> Result<Option<u16>, DomainError> {
    let found = tags(event, "k");
    if found.len() > 1 {
        return Err(invalid("a zap target kind is an event kind"));
    }
    match found.first().and_then(|tag| tag.value()) {
        None => Ok(None),
        Some(value) => {
            if value.is_empty()
                || value.len() > 5
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || (value.len() > 1 && value.starts_with('0'))
            {
                return Err(invalid("a zap target kind is an event kind"));
            }
            Ok(Some(value.parse().map_err(|_| {
                invalid("a zap target kind is an event kind")
            })?))
        }
    }
}

fn relays_of(event: &Event) -> Result<Vec<String>, DomainError> {
    let found = tags(event, "relays");
    if found.len() != 1 {
        return Err(invalid("a zap request lists relays"));
    }
    let mut relays = Vec::new();
    for value in found[0].as_slice().iter().skip(1) {
        if !is_relay(value) {
            return Err(invalid("a zap request lists ws:// or wss:// relays"));
        }
        relays.push(value.to_owned());
    }
    if relays.is_empty() {
        return Err(invalid("a zap request lists ws:// or wss:// relays"));
    }
    Ok(relays)
}

fn expand_prefix(prefix: &str) -> Vec<u8> {
    let mut expanded = Vec::with_capacity(prefix.len() * 2 + 1);
    expanded.extend(prefix.bytes().map(|byte| byte >> 5));
    expanded.push(0);
    expanded.extend(prefix.bytes().map(|byte| byte & 31));
    expanded
}

fn polymod(expanded_prefix: &[u8], words: &[u8]) -> u32 {
    let mut checksum: u32 = 1;
    for value in expanded_prefix.iter().chain(words) {
        let top = checksum >> 25;
        checksum = ((checksum & 0x01ff_ffff) << 5) ^ u32::from(*value);
        for (bit, generator) in GENERATOR.iter().enumerate() {
            if (top >> bit) & 1 == 1 {
                checksum ^= generator;
            }
        }
    }
    checksum
}

#[cfg(test)]
fn checksum(prefix: &str, words: &[u8]) -> [u8; 6] {
    let mut padded = words.to_vec();
    padded.extend_from_slice(&[0; 6]);
    let folded = polymod(&expand_prefix(prefix), &padded) ^ 1;
    let mut out = [0; 6];
    for (index, word) in out.iter_mut().enumerate() {
        *word = ((folded >> (5 * (5 - index))) & 31) as u8;
    }
    out
}

fn bech32_words(text: &str) -> Result<(String, Vec<u8>), DomainError> {
    if text.is_empty() || !text.is_ascii() {
        return Err(invalid("a bech32 value is lowercase"));
    }
    let has_lower = text.bytes().any(|byte| byte.is_ascii_lowercase());
    let has_upper = text.bytes().any(|byte| byte.is_ascii_uppercase());
    if has_lower && has_upper {
        return Err(invalid("a bech32 value is lowercase"));
    }
    let lowered = text.to_ascii_lowercase();
    let separator = lowered
        .rfind('1')
        .ok_or_else(|| invalid("a bech32 value has a prefix"))?;
    if separator == 0 {
        return Err(invalid("a bech32 value has a prefix"));
    }
    let (prefix, data) = lowered.split_at(separator);
    let data = &data[1..];
    if data.len() < 6 || !prefix.bytes().all(|byte| (33..=126).contains(&byte)) {
        return Err(invalid("a bech32 value has a prefix"));
    }
    let mut words = Vec::with_capacity(data.len());
    for byte in data.bytes() {
        let word = CHARSET
            .iter()
            .position(|candidate| *candidate == byte)
            .ok_or_else(|| invalid("a bech32 value uses its alphabet"))?;
        words.push(word as u8);
    }
    if polymod(&expand_prefix(prefix), &words) != 1 {
        return Err(invalid("a bech32 checksum does not verify"));
    }
    words.truncate(words.len() - 6);
    Ok((prefix.to_owned(), words))
}

fn from_words(words: &[u8]) -> Result<Vec<u8>, DomainError> {
    let mut bytes = Vec::with_capacity(words.len() * 5 / 8);
    let mut accumulator: u32 = 0;
    let mut bits: u8 = 0;
    for word in words {
        if *word > 31 {
            return Err(invalid("a bech32 value uses its alphabet"));
        }
        accumulator = (accumulator << 5) | u32::from(*word);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            bytes.push(((accumulator >> bits) & 0xff) as u8);
        }
    }
    if bits >= 5 || (accumulator & ((1 << bits) - 1)) != 0 {
        return Err(invalid("a bech32 value does not convert to whole bytes"));
    }
    Ok(bytes)
}

#[cfg(test)]
fn to_words(data: &[u8]) -> Vec<u8> {
    let mut words = Vec::new();
    let mut accumulator: u32 = 0;
    let mut bits: u8 = 0;
    for byte in data {
        accumulator = (accumulator << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            words.push(((accumulator >> bits) & 31) as u8);
        }
    }
    if bits > 0 {
        words.push(((accumulator << (5 - bits)) & 31) as u8);
    }
    words
}

#[cfg(test)]
fn encode_words(prefix: &str, words: &[u8]) -> String {
    let mut all = words.to_vec();
    all.extend_from_slice(&checksum(prefix, words));
    let mut encoded = String::with_capacity(prefix.len() + 1 + all.len());
    encoded.push_str(prefix);
    encoded.push('1');
    for word in all {
        encoded.push(char::from(CHARSET[usize::from(word)]));
    }
    encoded
}

fn hash_words(words: &[u8]) -> Result<[u8; 32], DomainError> {
    let bytes = from_words(words)?;
    <[u8; 32]>::try_from(bytes).map_err(|_| invalid("a bolt11 hash is 32 bytes"))
}

fn amount_from_hrp(hrp: &str) -> Result<Option<u64>, DomainError> {
    let Some(rest) = hrp.strip_prefix("ln") else {
        return Err(invalid("a bolt11 invoice starts with ln"));
    };
    let network = rest.chars().take_while(char::is_ascii_alphabetic).count();
    if network < 2 {
        return Err(invalid("a bolt11 invoice names its network"));
    }
    let amount = &rest[network..];
    if amount.is_empty() {
        return Ok(None);
    }
    let last = amount.as_bytes()[amount.len() - 1];
    let (digits, factor) = match last {
        b'm' => (&amount[..amount.len() - 1], 100_000_000_u128),
        b'u' => (&amount[..amount.len() - 1], 100_000),
        b'n' => (&amount[..amount.len() - 1], 100),
        b'p' => (&amount[..amount.len() - 1], 0),
        b'0'..=b'9' => (amount, 100_000_000_000),
        _ => return Err(invalid("a bolt11 amount is a number and a multiplier")),
    };
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err(invalid("a bolt11 amount is a number and a multiplier"));
    }
    let number: u128 = digits
        .parse()
        .map_err(|_| invalid("a bolt11 amount is a number and a multiplier"))?;
    let millis = if last == b'p' {
        if !number.is_multiple_of(10) {
            return Err(invalid(
                "a bolt11 amount is a whole number of millisatoshis",
            ));
        }
        number / 10
    } else {
        number * factor
    };
    u64::try_from(millis)
        .map(Some)
        .map_err(|_| invalid("a bolt11 amount is a whole number of millisatoshis"))
}

fn invoice(text: &str) -> Result<Invoice, DomainError> {
    let (hrp, words) = bech32_words(text)?;
    if words.len() < 7 + 104 {
        return Err(invalid("a bolt11 invoice is too short"));
    }
    let signed = words.len() - 104;
    let mut cursor = 7;
    let mut description_hash = None;
    let mut payment_hash = None;
    while cursor < signed {
        if cursor + 3 > signed {
            return Err(invalid("a bolt11 tagged field is truncated"));
        }
        let tag = words[cursor];
        let length = (usize::from(words[cursor + 1]) << 5) | usize::from(words[cursor + 2]);
        cursor += 3;
        if cursor + length > signed {
            return Err(invalid("a bolt11 tagged field is truncated"));
        }
        let data = &words[cursor..cursor + length];
        cursor += length;
        match tag {
            PAYMENT_HASH => payment_hash = Some(hash_words(data)?),
            DESCRIPTION_HASH => description_hash = Some(hash_words(data)?),
            _ => {}
        }
    }
    Ok(Invoice {
        amount_msat: amount_from_hrp(&hrp)?,
        description_hash,
        payment_hash,
    })
}

/// The millisatoshis named by a bolt11 invoice, when the invoice names an amount.
///
/// # Errors
///
/// Returns a sentence when the invoice checksum or amount is refused.
pub fn bolt11_amount_msat(invoice_text: &str) -> Result<Option<u64>, DomainError> {
    Ok(invoice(invoice_text)?.amount_msat)
}

/// Decode an `lnurl1...` pay URL.
///
/// # Errors
///
/// Returns a sentence when the checksum or the URL is refused.
pub fn decode_lnurl(value: &str) -> Result<String, DomainError> {
    let (prefix, words) = bech32_words(value)?;
    if prefix != "lnurl" {
        return Err(invalid("an lnurl value uses the lnurl prefix"));
    }
    let bytes = from_words(&words)?;
    let url = String::from_utf8(bytes).map_err(|_| invalid("an lnurl value is an HTTP URL"))?;
    if !http_url(&url) {
        return Err(invalid("an lnurl value is an HTTP URL"));
    }
    Ok(url)
}

#[cfg(test)]
fn encode_lnurl(url: &str) -> String {
    encode_words("lnurl", &to_words(url.as_bytes()))
}

fn encode_uri(text: &str) -> String {
    const SAFE: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'();,/?:@&=+$#";
    let mut encoded = String::new();
    for byte in text.bytes() {
        if SAFE.contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// The query string for an LNURL callback. The relay does not send it.
///
/// # Errors
///
/// Returns a sentence when `amount_msat` or `lnurl` disagrees with the request.
pub fn zap_callback_query(
    request: &Event,
    amount_msat: u64,
    lnurl: &str,
) -> Result<String, DomainError> {
    let parsed = open_zap_request(request)?;
    if let Some(amount) = parsed.amount_msat
        && amount != amount_msat
    {
        return Err(invalid("a zap callback amount matches the request"));
    }
    if let Some(expected) = &parsed.lnurl
        && expected != lnurl
    {
        return Err(invalid("a zap callback lnurl matches the request"));
    }
    let encoded = encode_uri(&serde_json::to_string(request).unwrap_or_default());
    Ok(format!(
        "amount={amount_msat}&nostr={encoded}&lnurl={lnurl}"
    ))
}

/// Divide `amount_msat` across `zap` tags.
///
/// Absent weights split the amount equally. A missing weight beside a
/// present weight is zero. The remainder goes to the first positive weight.
///
/// # Errors
///
/// Returns a sentence when a receiver pubkey, relay, or weight is refused.
pub fn zap_split(event: &Event, amount_msat: u64) -> Result<Vec<ZapShare>, DomainError> {
    let found = tags(event, "zap");
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let mut shares = Vec::with_capacity(found.len());
    let mut any_weight = false;
    for tag in found {
        let Some(receiver) = tag.value() else {
            return Err(invalid("a zap receiver is 32 lowercase hex bytes"));
        };
        let relay = tag
            .as_slice()
            .get(2)
            .filter(|value| is_relay(value))
            .ok_or_else(|| invalid("a zap receiver lists a ws:// or wss:// relay"))?;
        let weight = match tag.as_slice().get(3) {
            None => None,
            Some(value) => {
                any_weight = true;
                Some(
                    millisats(value)
                        .map_err(|_| invalid("a zap weight is a non-negative integer"))?,
                )
            }
        };
        shares.push(ZapShare {
            pubkey: pubkey(receiver, "a zap receiver is 32 lowercase hex bytes")?,
            relay: relay.to_owned(),
            weight: weight.unwrap_or(0),
            millisats: 0,
        });
    }
    if !any_weight {
        for share in &mut shares {
            share.weight = 1;
        }
    }
    let total: u128 = shares.iter().map(|share| u128::from(share.weight)).sum();
    if total == 0 {
        return Ok(shares);
    }
    let mut assigned: u128 = 0;
    for share in &mut shares {
        let part = u128::from(amount_msat) * u128::from(share.weight) / total;
        share.millisats = u64::try_from(part).unwrap_or(u64::MAX);
        assigned += part;
    }
    let remainder = u128::from(amount_msat).saturating_sub(assigned);
    if let Some(first) = shares.iter_mut().find(|share| share.weight > 0) {
        first.millisats = first
            .millisats
            .saturating_add(u64::try_from(remainder).unwrap_or(0));
    }
    Ok(shares)
}

/// Read a kind `9734` zap request.
///
/// # Errors
///
/// Returns a sentence when the recipient, relays, or an optional tag is refused.
pub fn open_zap_request(event: &Event) -> Result<ZapRequest, DomainError> {
    if event.kind != REQUEST_KIND {
        return Err(invalid("a zap request has kind 9734"));
    }
    if event.tags.is_empty() {
        return Err(invalid("a zap request has tags"));
    }
    let lnurl = match tags(event, "lnurl").as_slice() {
        [] => None,
        [tag] => {
            let Some(value) = tag.value() else {
                return Err(invalid("an lnurl value uses the lnurl prefix"));
            };
            let _url = decode_lnurl(value)?;
            Some(value.to_owned())
        }
        _ => return Err(invalid("a zap request has one lnurl")),
    };
    Ok(ZapRequest {
        sender: event.pubkey.clone(),
        recipient: one_pubkey(event, "p", "a zap request has one recipient")?,
        content: event.content.clone(),
        amount_msat: optional_amount(event)?,
        lnurl,
        relays: relays_of(event)?,
        event_id: optional_event_id(event)?,
        address: optional_address(event)?,
        target_kind: optional_kind(event)?,
        receipt_pubkey: optional_pubkey(event, "P", "a zap request has one receipt pubkey")?,
    })
}

/// Read a kind `9735` zap receipt.
///
/// # Errors
///
/// Returns a sentence when the invoice, description, or copied tags disagree.
pub fn open_zap_receipt(event: &Event) -> Result<ZapReceipt, DomainError> {
    if event.kind != RECEIPT_KIND {
        return Err(invalid("a zap receipt has kind 9735"));
    }
    let bolt11_tags = tags(event, "bolt11");
    if bolt11_tags.len() != 1 {
        return Err(invalid("a zap receipt has one bolt11 invoice"));
    }
    let Some(bolt11) = bolt11_tags[0].value() else {
        return Err(invalid("a zap receipt has one bolt11 invoice"));
    };
    let parsed_invoice = invoice(bolt11)?;
    let descriptions = tags(event, "description");
    if descriptions.len() != 1 {
        return Err(invalid("a zap receipt has one description"));
    }
    let Some(description) = descriptions[0].value() else {
        return Err(invalid("a zap receipt has one description"));
    };
    if let Some(expected) = parsed_invoice.description_hash {
        let digest: [u8; 32] = Sha256::digest(description.as_bytes()).into();
        if digest != expected {
            return Err(invalid("a zap receipt description matches the bolt11 hash"));
        }
    }
    let embedded: Event = serde_json::from_str(description)
        .map_err(|_| invalid("a zap receipt description is a zap request"))?;
    embedded
        .validate_crypto()
        .map_err(|_| invalid("a zap receipt description is a signed zap request"))?;
    let request = open_zap_request(&embedded)?;
    if let Some(expected) = &request.receipt_pubkey
        && expected != &event.pubkey
    {
        return Err(invalid("a zap request P tag matches the receipt pubkey"));
    }
    if let Some(amount) = request.amount_msat {
        match parsed_invoice.amount_msat {
            Some(invoice_amount) if invoice_amount == amount => {}
            _ => return Err(invalid("a zap receipt amount matches the request")),
        }
    }
    let recipient = one_pubkey(event, "p", "a zap receipt names the recipient")?;
    if recipient != request.recipient {
        return Err(invalid("a zap receipt names the request recipient"));
    }
    let event_id = optional_event_id(event)?;
    if event_id != request.event_id {
        return Err(invalid("a zap receipt names the request event"));
    }
    let address = optional_address(event)?;
    if address != request.address {
        return Err(invalid("a zap receipt names the request address"));
    }
    if let Some(sender) = optional_pubkey(event, "P", "a zap receipt names the sender")?
        && sender != request.sender
    {
        return Err(invalid("a zap receipt P tag names the request sender"));
    }
    let preimage = match tags(event, "preimage").as_slice() {
        [] => None,
        [tag] => {
            let Some(value) = tag.value() else {
                return Err(invalid("a zap preimage is 32 lowercase hex bytes"));
            };
            let bytes = decode_lower_hex::<32>(value, "zap preimage")
                .map_err(|_| invalid("a zap preimage is 32 lowercase hex bytes"))?;
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            match parsed_invoice.payment_hash {
                Some(hash) if hash == digest => Some(value.to_owned()),
                _ => return Err(invalid("a zap preimage matches the bolt11 payment hash")),
            }
        }
        _ => return Err(invalid("a zap receipt has one preimage")),
    };
    Ok(ZapReceipt {
        sender: request.sender.clone(),
        recipient,
        amount_msat: parsed_invoice.amount_msat.or(request.amount_msat),
        bolt11: bolt11.to_owned(),
        preimage,
        request,
    })
}

#[cfg(test)]
fn push_hash(words: &mut Vec<u8>, tag: u8, hash: &[u8; 32]) {
    let data = to_words(hash);
    let length = data.len();
    words.push(tag);
    words.push(u8::try_from(length >> 5).unwrap_or(0));
    words.push(u8::try_from(length & 31).unwrap_or(0));
    words.extend(data);
}

#[cfg(test)]
fn invoice_for(amount_hrp: &str, description: &str, preimage: &[u8; 32]) -> String {
    let description_hash: [u8; 32] = Sha256::digest(description.as_bytes()).into();
    let payment_hash: [u8; 32] = Sha256::digest(preimage).into();
    let mut words = vec![0_u8; 7];
    push_hash(&mut words, DESCRIPTION_HASH, &description_hash);
    push_hash(&mut words, PAYMENT_HASH, &payment_hash);
    words.extend(std::iter::repeat_n(0, 104));
    encode_words(amount_hrp, &words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DomainError, EventClass, RelaySigner, Tag, compare_replacement};

    use super::encode_lower_hex;

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    const PINNED_INVOICE: &str = "lnbc10u1p3unwfusp5t9r3yymhpfqculx78u027lxspgxcr2n2987mx2j55nnfs95nxnzqpp5jmrh92pfld78spqs78v9euf2385t83uvpwk9ldrlvf6ch7tpascqhp5zvkrmemgth3tufcvflmzjzfvjt023nazlhljz2n9hattj4f8jq8qxqyjw5qcqpjrzjqtc4fc44feggv7065fqe5m4ytjarg3repr5j9el35xhmtfexc42yczarjuqqfzqqqqqqqqlgqqqqqqgq9q9qxpqysgq079nkq507a5tw7xgttmj4u990j7wfggtrasah5gd4ywfr2pjcn29383tphp4t48gquelz9z78p4cq7ml3nrrphw5w6eckhjwmhezhnqpy6gyf0";

    #[test]
    fn a_zap_receipt_matches_the_request_amount_and_description() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/57.md"
        ))
        .unwrap();
        assert!(text.contains("9734"));
        assert!(text.contains("9735"));
        assert!(text.contains("millisats"));
        assert!(text.contains(PINNED_INVOICE));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "57.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "57.md")
        );
        assert_eq!(bolt11_amount_msat(PINNED_INVOICE).unwrap(), Some(1_000_000));

        let sender = signer("57");
        let recipient = signer("58");
        let wallet = signer("59");
        let url = "https://example.com/.well-known/lnurlp/alice";
        let lnurl = encode_lnurl(url);
        assert_eq!(decode_lnurl(&lnurl).unwrap(), url);
        let note = recipient.sign(1_679_673_000, 1, Vec::new(), "hello".into());
        let request = sender.sign(
            1_679_673_265,
            REQUEST_KIND,
            vec![
                Tag::new(vec![
                    "relays".into(),
                    "wss://nostr-pub.wellorder.net".into(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["amount".into(), "21000".into()]),
                Tag::new(vec!["lnurl".into(), lnurl.clone()]),
                Tag::new(vec!["p".into(), recipient.pubkey().to_owned()]),
                Tag::new(vec!["e".into(), note.id.clone()]),
                Tag::new(vec!["k".into(), "1".into()]),
                Tag::new(vec!["P".into(), wallet.pubkey().to_owned()]),
            ],
            "Zap!".into(),
        );
        request.validate_structure().unwrap();
        request.validate_crypto().unwrap();
        assert_eq!(request.class(), EventClass::Regular);
        let opened = open_zap_request(&request).unwrap();
        assert_eq!(opened.amount_msat, Some(21_000));
        assert_eq!(opened.recipient, recipient.pubkey());
        assert_eq!(opened.event_id.as_deref(), Some(note.id.as_str()));
        assert_eq!(opened.relays.len(), 2);
        let query = zap_callback_query(&request, 21_000, &lnurl).unwrap();
        assert!(query.starts_with("amount=21000&nostr="));
        assert!(query.ends_with(&format!("&lnurl={lnurl}")));
        assert!(zap_callback_query(&request, 22_000, &lnurl).is_err());
        let later = sender.sign(
            1_679_673_300,
            REQUEST_KIND,
            vec![
                Tag::new(vec!["relays".into(), "wss://relay.example".into()]),
                Tag::new(vec!["p".into(), recipient.pubkey().to_owned()]),
            ],
            String::new(),
        );
        assert!(matches!(
            compare_replacement(&request, &later),
            Err(DomainError::NotReplaceable)
        ));

        let preimage = [0x5d_u8; 32];
        let description = serde_json::to_string(&request).unwrap();
        let bolt11 = invoice_for("lnbc210n", &description, &preimage);
        assert_eq!(bolt11_amount_msat(&bolt11).unwrap(), Some(21_000));
        let receipt = wallet.sign(
            1_679_674_000,
            RECEIPT_KIND,
            vec![
                Tag::new(vec!["p".into(), recipient.pubkey().to_owned()]),
                Tag::new(vec!["P".into(), sender.pubkey().to_owned()]),
                Tag::new(vec!["e".into(), note.id.clone()]),
                Tag::new(vec!["bolt11".into(), bolt11]),
                Tag::new(vec!["description".into(), description]),
                Tag::new(vec!["preimage".into(), encode_lower_hex(&preimage)]),
            ],
            String::new(),
        );
        receipt.validate_structure().unwrap();
        let paid = open_zap_receipt(&receipt).unwrap();
        assert_eq!(paid.amount_msat, Some(21_000));
        assert_eq!(paid.request.event_id.as_deref(), Some(note.id.as_str()));
        assert_eq!(
            paid.preimage.as_deref(),
            Some(encode_lower_hex(&preimage).as_str())
        );
        let mut tampered = receipt.clone();
        tampered.tags.iter_mut().for_each(|tag| {
            if tag.name() == Some("description") {
                tag.0[1].push(' ');
            }
        });
        assert!(open_zap_receipt(&tampered).is_err());

        let note = sender.sign(
            1_679_674_100,
            1,
            vec![
                Tag::new(vec![
                    "zap".into(),
                    "82".repeat(32),
                    "wss://nostr.oxtr.dev".into(),
                    "1".into(),
                ]),
                Tag::new(vec![
                    "zap".into(),
                    "fa".repeat(32),
                    "wss://nostr.wine".into(),
                    "1".into(),
                ]),
                Tag::new(vec![
                    "zap".into(),
                    "46".repeat(32),
                    "wss://nos.lol".into(),
                    "2".into(),
                ]),
            ],
            "split".into(),
        );
        note.validate_structure().unwrap();
        let shares = zap_split(&note, 1_000).unwrap();
        assert_eq!(
            shares
                .iter()
                .map(|share| share.millisats)
                .collect::<Vec<_>>(),
            vec![250, 250, 500]
        );
        assert_eq!(shares.len(), 3);
        let missing = sender.sign(
            1_679_674_200,
            REQUEST_KIND,
            vec![Tag::new(vec!["p".into(), recipient.pubkey().to_owned()])],
            String::new(),
        );
        assert!(missing.validate_structure().is_err());
    }
}
