//! BOLT11 decoding for the exact Lightning method.
//!
//! Reimplemented from the public BOLT11 encoding contract. This is independent
//! of the older zap parser, which deliberately does not verify invoice crypto.
//! BOLT9's assumed-feature interpretation was checked at revision
//! `1aadb719b4007c4cea0ba6e36b08c4fb53788dee`:
//! <https://github.com/lightning/bolts/blob/1aadb719b4007c4cea0ba6e36b08c4fb53788dee/09-features.md>.
//! This bounded reader supports required bits 8, 14, 16, and 48; it refuses
//! other required features. Wallet routing capabilities remain a host concern.

use secp256k1::{
    Message, PublicKey, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId, Signature},
};
use sha2::{Digest, Sha256};

use super::PaymentError;

const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const GENERATOR: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
const MAX_INVOICE_BYTES: usize = 16_384;

/// Authenticated invoice fields. Construction requires complete strict decode
/// and ECDSA verification. Payment terms and expiry must still be checked.
#[derive(Clone, PartialEq, Eq)]
pub struct Invoice {
    pub(super) currency: String,
    pub(super) amount_msat: u64,
    pub(super) created_at: u64,
    pub(super) expiry_seconds: u64,
    pub(super) payment_hash: [u8; 32],
    pub(super) description_hash: [u8; 32],
    pub(super) payee: PublicKey,
}

impl Invoice {
    pub fn currency(&self) -> &str {
        &self.currency
    }
    pub fn amount_msat(&self) -> u64 {
        self.amount_msat
    }
    pub fn created_at(&self) -> u64 {
        self.created_at
    }
    pub fn expiry_seconds(&self) -> u64 {
        self.expiry_seconds
    }
    pub fn payment_hash(&self) -> [u8; 32] {
        self.payment_hash
    }
    pub fn description_hash(&self) -> [u8; 32] {
        self.description_hash
    }
    pub fn payee(&self) -> [u8; 33] {
        self.payee.serialize()
    }
}

/// Strictly authenticate a BOLT11 invoice on the two x402 Lightning networks.
///
/// Requires one payment hash, payment secret, and description hash, no inline
/// description, minimal integer/feature encodings, valid fixed-field padding,
/// and supported mandatory invoice features. It accepts unknown odd features
/// and unknown optional tagged fields as BOLT11 requires. It performs no payment.
pub fn decode_invoice(text: &str) -> Result<Invoice, PaymentError> {
    let (hrp, words) = decode_bech32(text)?;
    let (currency, amount_msat) = amount(&hrp)?;
    if words.len() < 7 + 104 {
        return Err(PaymentError::InvoiceDecode);
    }
    let unsigned = &words[..words.len() - 104];
    let signature = words_to_bytes(&words[words.len() - 104..], false)?;
    if signature.len() != 65 || signature[64] > 3 {
        return Err(PaymentError::InvoiceSignature);
    }
    let created_at = integer(&unsigned[..7], false)?;
    let mut payment_hash = None;
    let mut payment_secret = None;
    let mut description_hash = None;
    let mut payee = None;
    let mut expiry = None;
    let mut cltv = None;
    let mut features = None;
    let mut metadata = false;
    let mut index = 7;
    while index < unsigned.len() {
        if unsigned.len() - index < 3 {
            return Err(PaymentError::InvoiceDecode);
        }
        let kind = unsigned[index];
        let length = usize::from(unsigned[index + 1]) * 32 + usize::from(unsigned[index + 2]);
        index += 3;
        let end = index
            .checked_add(length)
            .ok_or(PaymentError::InvoiceDecode)?;
        let data = unsigned
            .get(index..end)
            .ok_or(PaymentError::InvoiceDecode)?;
        index = end;
        match kind {
            1 => set_once(&mut payment_hash, fixed::<32>(data, 52)?)?,
            16 => set_once(&mut payment_secret, fixed::<32>(data, 52)?)?,
            23 => {
                if description_hash.is_some() {
                    return Err(PaymentError::InvoiceDescription);
                }
                description_hash = Some(fixed::<32>(data, 52)?);
            }
            13 => return Err(PaymentError::InvoiceDescription),
            19 => {
                let key = fixed::<33>(data, 53)?;
                if !matches!(key[0], 2 | 3) {
                    return Err(PaymentError::InvoiceDecode);
                }
                set_once(
                    &mut payee,
                    PublicKey::from_slice(&key).map_err(|_| PaymentError::InvoiceDecode)?,
                )?;
            }
            6 => set_once(&mut expiry, integer(data, true)?)?,
            24 => set_once(&mut cltv, integer(data, true)?)?,
            5 => {
                if features.is_some() {
                    return Err(PaymentError::InvoiceDecode);
                }
                check_features(data)?;
                features = Some(data.to_vec());
            }
            27 => {
                if metadata {
                    return Err(PaymentError::InvoiceDecode);
                }
                words_to_bytes(data, false)?;
                metadata = true;
            }
            3 => validate_routes(data)?,
            9 => validate_fallback(data)?,
            _ => {}
        }
    }
    let payment_hash = payment_hash.ok_or(PaymentError::InvoiceDecode)?;
    payment_secret.ok_or(PaymentError::InvoiceDecode)?;
    let description_hash = description_hash.ok_or(PaymentError::InvoiceDescription)?;
    let mut signing = hrp.as_bytes().to_vec();
    signing.extend(words_to_bytes(unsigned, true)?);
    let message = Message::from_digest(Sha256::digest(signing).into());
    let signature_standard =
        Signature::from_compact(&signature[..64]).map_err(|_| PaymentError::InvoiceSignature)?;
    let secp = Secp256k1::verification_only();
    let payee = if let Some(payee) = payee {
        let mut normalized = signature_standard;
        normalized.normalize_s();
        if normalized != signature_standard {
            return Err(PaymentError::InvoiceSignature);
        }
        secp.verify_ecdsa(message, &signature_standard, &payee)
            .map_err(|_| PaymentError::InvoiceSignature)?;
        payee
    } else {
        let recoverable = RecoverableSignature::from_compact(
            &signature[..64],
            RecoveryId::try_from(i32::from(signature[64]))
                .map_err(|_| PaymentError::InvoiceSignature)?,
        )
        .map_err(|_| PaymentError::InvoiceSignature)?;
        let payee = secp
            .recover_ecdsa(message, &recoverable)
            .map_err(|_| PaymentError::InvoiceSignature)?;
        let mut normalized = signature_standard;
        normalized.normalize_s();
        secp.verify_ecdsa(message, &normalized, &payee)
            .map_err(|_| PaymentError::InvoiceSignature)?;
        payee
    };
    Ok(Invoice {
        currency,
        amount_msat,
        created_at,
        expiry_seconds: expiry.unwrap_or(3600),
        payment_hash,
        description_hash,
        payee,
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), PaymentError> {
    if slot.is_some() {
        return Err(PaymentError::InvoiceDecode);
    }
    *slot = Some(value);
    Ok(())
}
fn fixed<const N: usize>(words: &[u8], expected_words: usize) -> Result<[u8; N], PaymentError> {
    if words.len() != expected_words {
        return Err(PaymentError::InvoiceDecode);
    }
    words_to_bytes(words, false)?
        .try_into()
        .map_err(|_| PaymentError::InvoiceDecode)
}
fn integer(words: &[u8], minimal: bool) -> Result<u64, PaymentError> {
    if words.is_empty() || (minimal && words[0] == 0) {
        return Err(PaymentError::InvoiceDecode);
    }
    let mut value = 0_u64;
    for word in words {
        value = value
            .checked_mul(32)
            .and_then(|value| value.checked_add(u64::from(*word)))
            .ok_or(PaymentError::InvoiceDecode)?;
    }
    Ok(value)
}
fn amount(hrp: &str) -> Result<(String, u64), PaymentError> {
    let value = hrp.strip_prefix("ln").ok_or(PaymentError::InvoiceDecode)?;
    let first_digit = value
        .find(|ch: char| ch.is_ascii_digit())
        .ok_or(PaymentError::InvoiceAmount)?;
    let currency = &value[..first_digit];
    if !matches!(currency, "bc" | "tb") {
        return Err(PaymentError::InvoiceCurrency);
    }
    let value = &value[first_digit..];
    let (digits, multiplier) = match value.as_bytes().last().copied() {
        Some(byte @ b'0'..=b'9') => {
            let _ = byte;
            (value, None)
        }
        Some(byte) => (&value[..value.len() - 1], Some(byte)),
        None => return Err(PaymentError::InvoiceAmount),
    };
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(PaymentError::InvoiceAmount);
    }
    let value = digits
        .parse::<u128>()
        .map_err(|_| PaymentError::InvoiceAmount)?;
    let msat = match multiplier {
        None => value.checked_mul(100_000_000_000),
        Some(b'm') => value.checked_mul(100_000_000),
        Some(b'u') => value.checked_mul(100_000),
        Some(b'n') => value.checked_mul(100),
        Some(b'p') if value.is_multiple_of(10) => Some(value / 10),
        _ => None,
    }
    .and_then(|value| u64::try_from(value).ok())
    .filter(|value| *value > 0)
    .ok_or(PaymentError::InvoiceAmount)?;
    Ok((currency.into(), msat))
}

fn check_features(words: &[u8]) -> Result<(), PaymentError> {
    if words.is_empty() || words[0] == 0 {
        return Err(PaymentError::InvoiceDecode);
    }
    let bit = |index: usize| -> bool {
        words
            .iter()
            .rev()
            .nth(index / 5)
            .is_some_and(|word| word & (1 << (index % 5)) != 0)
    };
    for index in 0..words.len() * 5 {
        if index % 2 == 0 && bit(index) && ![8, 14, 16, 48].contains(&index) {
            return Err(PaymentError::InvoiceDecode);
        }
    }
    // BOLT9 now treats onion payloads and payment secrets as assumed. The
    // mandatory `s` field above establishes basic_mpp's secret dependency;
    // requiring historical flags would reject valid modern invoices. A pair
    // with both bits set is treated as mandatory by the loop above.
    Ok(())
}
fn validate_routes(words: &[u8]) -> Result<(), PaymentError> {
    let bytes = words_to_bytes(words, false)?;
    if bytes.is_empty() || bytes.len() % 51 != 0 {
        return Err(PaymentError::InvoiceDecode);
    }
    for hop in bytes.chunks_exact(51) {
        if !matches!(hop[0], 2 | 3) || PublicKey::from_slice(&hop[..33]).is_err() {
            return Err(PaymentError::InvoiceDecode);
        }
    }
    Ok(())
}
fn validate_fallback(words: &[u8]) -> Result<(), PaymentError> {
    let Some((&version, program)) = words.split_first() else {
        return Err(PaymentError::InvoiceDecode);
    };
    if version > 18 {
        return Ok(());
    }
    let bytes = words_to_bytes(program, false)?;
    let valid = match version {
        0 => matches!(bytes.len(), 20 | 32),
        1..=16 => (2..=40).contains(&bytes.len()),
        17 | 18 => bytes.len() == 20,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(PaymentError::InvoiceDecode)
    }
}
fn words_to_bytes(words: &[u8], pad: bool) -> Result<Vec<u8>, PaymentError> {
    let mut bytes = Vec::with_capacity((words.len() * 5).div_ceil(8));
    let mut bits = 0_u32;
    let mut count = 0;
    for word in words {
        if *word > 31 {
            return Err(PaymentError::InvoiceDecode);
        }
        bits = (bits << 5) | u32::from(*word);
        count += 5;
        if count >= 8 {
            count -= 8;
            bytes.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    if pad {
        if count > 0 {
            bytes.push((bits << (8 - count)) as u8);
        }
    } else if count >= 5 || bits != 0 {
        return Err(PaymentError::InvoiceDecode);
    }
    Ok(bytes)
}
fn decode_bech32(text: &str) -> Result<(String, Vec<u8>), PaymentError> {
    if text.len() > MAX_INVOICE_BYTES
        || !text.is_ascii()
        || (text.bytes().any(|byte| byte.is_ascii_lowercase())
            && text.bytes().any(|byte| byte.is_ascii_uppercase()))
    {
        return Err(PaymentError::InvoiceDecode);
    }
    let text = text.to_ascii_lowercase();
    let split = text.rfind('1').ok_or(PaymentError::InvoiceDecode)?;
    if split == 0 {
        return Err(PaymentError::InvoiceDecode);
    }
    let hrp = &text[..split];
    if !hrp.bytes().all(|byte| (33..=126).contains(&byte)) {
        return Err(PaymentError::InvoiceDecode);
    }
    let mut data = text[split + 1..]
        .bytes()
        .map(|byte| {
            CHARSET
                .iter()
                .position(|candidate| *candidate == byte)
                .map(|position| position as u8)
                .ok_or(PaymentError::InvoiceDecode)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if data.len() < 6 {
        return Err(PaymentError::InvoiceDecode);
    }
    let expanded = hrp
        .bytes()
        .map(|byte| byte >> 5)
        .chain(std::iter::once(0))
        .chain(hrp.bytes().map(|byte| byte & 31));
    if polymod(expanded.chain(data.iter().copied())) != 1 {
        return Err(PaymentError::InvoiceDecode);
    }
    data.truncate(data.len() - 6);
    Ok((hrp.into(), data))
}
fn polymod(words: impl Iterator<Item = u8>) -> u32 {
    let mut value = 1_u32;
    for word in words {
        let high = value >> 25;
        value = ((value & 0x1ffffff) << 5) ^ u32::from(word);
        for (index, generator) in GENERATOR.iter().enumerate() {
            if (high >> index) & 1 != 0 {
                value ^= generator;
            }
        }
    }
    value
}

#[cfg(test)]
pub(super) mod test_invoice {
    use super::*;
    use secp256k1::SecretKey;
    pub fn words(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut bits = 0_u32;
        let mut count = 0;
        for byte in bytes {
            bits = (bits << 8) | u32::from(*byte);
            count += 8;
            while count >= 5 {
                count -= 5;
                out.push(((bits >> count) & 31) as u8);
                bits &= (1 << count) - 1;
            }
        }
        if count > 0 {
            out.push((bits << (5 - count)) as u8);
        }
        out
    }
    pub fn tag(kind: u8, words: &[u8]) -> Vec<u8> {
        let mut out = vec![kind, (words.len() / 32) as u8, (words.len() % 32) as u8];
        out.extend(words);
        out
    }
    pub fn number(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        while value > 0 {
            out.push((value & 31) as u8);
            value >>= 5;
        }
        out.reverse();
        out
    }
    pub fn encode(hrp: &str, data: &[u8]) -> String {
        let expanded = hrp
            .bytes()
            .map(|byte| byte >> 5)
            .chain(std::iter::once(0))
            .chain(hrp.bytes().map(|byte| byte & 31));
        let checksum = polymod(expanded.chain(data.iter().copied()).chain([0; 6])) ^ 1;
        let suffix = (0..6).map(|index| ((checksum >> (5 * (5 - index))) & 31) as u8);
        format!(
            "{hrp}1{}",
            data.iter()
                .copied()
                .chain(suffix)
                .map(|word| CHARSET[usize::from(word)] as char)
                .collect::<String>()
        )
    }
    pub fn signed(hrp: &str, fields: Vec<u8>, explicit: bool, high_s: bool) -> String {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_byte_array([1; 32]).unwrap();
        let mut unsigned = vec![0; 7];
        let time = number(1_700_000_000);
        unsigned[7 - time.len()..].copy_from_slice(&time);
        unsigned.extend(fields);
        if explicit {
            unsigned.extend(tag(
                19,
                &words(&PublicKey::from_secret_key(&secp, &secret).serialize()),
            ));
        }
        let mut payload = hrp.as_bytes().to_vec();
        payload.extend(words_to_bytes(&unsigned, true).unwrap());
        let (recovery, compact) = secp
            .sign_ecdsa_recoverable(
                Message::from_digest(Sha256::digest(payload).into()),
                &secret,
            )
            .serialize_compact();
        let mut sig = compact.to_vec();
        let mut recovery = i32::from(recovery) as u8;
        if high_s {
            let order = crate::read_state_snapshot::hex_bytes::<32>(
                "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
            )
            .unwrap();
            let mut borrow = 0_i16;
            for index in (0..32).rev() {
                let value = i16::from(order[index]) - i16::from(sig[index + 32]) - borrow;
                sig[index + 32] = value.rem_euclid(256) as u8;
                borrow = i16::from(value < 0);
            }
            recovery ^= 1;
        }
        sig.push(recovery);
        unsigned.extend(words(&sig));
        encode(hrp, &unsigned)
    }
    pub fn fields(description: [u8; 32]) -> Vec<u8> {
        let mut fields = tag(1, &words(&Sha256::digest([7; 32])));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&description)));
        fields.extend(tag(6, &number(300)));
        fields
    }

    #[test]
    fn exact_units_signatures_and_optional_payee_are_authenticated() {
        for (explicit, high_s) in [(false, false), (false, true), (true, false)] {
            let invoice =
                decode_invoice(&signed("lnbc250n", fields([3; 32]), explicit, high_s)).unwrap();
            assert_eq!(invoice.amount_msat, 25000);
            assert_eq!(invoice.expiry_seconds, 300);
            assert_eq!(invoice.description_hash, [3; 32]);
        }
        assert!(matches!(
            decode_invoice(&signed("lnbc250n", fields([3; 32]), true, true)),
            Err(PaymentError::InvoiceSignature)
        ));
        for hrp in [
            "lnbc01n",
            "lnbc1p",
            "lnbc0",
            "lnbc",
            "lnbcrt1n",
            "lnbc18446744073709551615",
        ] {
            assert!(
                decode_invoice(&signed(hrp, fields([3; 32]), false, false)).is_err(),
                "{hrp}"
            );
        }
        assert_eq!(
            decode_invoice(&signed("lnbc10p", fields([3; 32]), false, false))
                .unwrap()
                .amount_msat,
            1
        );
    }

    #[test]
    fn fixed_fields_padding_duplicates_and_unsupported_features_refuse() {
        let mut duplicate = fields([3; 32]);
        duplicate.extend(tag(1, &words(&[4; 32])));
        assert!(decode_invoice(&signed("lnbc250n", duplicate, false, false)).is_err());
        let mut malformed = fields([3; 32]);
        malformed.extend(tag(19, &[0; 52]));
        assert!(decode_invoice(&signed("lnbc250n", malformed, false, false)).is_err());
        let mut unsupported = fields([3; 32]);
        unsupported.extend(tag(5, &[1]));
        assert!(decode_invoice(&signed("lnbc250n", unsupported, false, false)).is_err());
        for feature in [1_u64 << 14, 1 << 16, 1 << 48, (1 << 14) | (1 << 15)] {
            let mut modern = fields([3; 32]);
            modern.extend(tag(5, &number(feature)));
            assert!(decode_invoice(&signed("lnbc250n", modern, false, false)).is_ok());
        }
        let mut missing_secret = tag(1, &words(&[1; 32]));
        missing_secret.extend(tag(23, &words(&[2; 32])));
        missing_secret.extend(tag(5, &number(1 << 16)));
        assert!(decode_invoice(&signed("lnbc250n", missing_secret, false, false)).is_err());
        let mut inline = fields([3; 32]);
        inline.extend(tag(13, &words(b"different operation")));
        assert!(matches!(
            decode_invoice(&signed("lnbc250n", inline, false, false)),
            Err(PaymentError::InvoiceDescription)
        ));
    }

    #[test]
    fn checksum_and_signature_mutations_cannot_preserve_the_expected_payee() {
        let text = signed("lnbc250n", fields([3; 32]), true, false);
        let (mut hrp, mut data) = decode_bech32(&text).unwrap();
        data[0] ^= 1;
        assert!(matches!(
            decode_invoice(&encode(&hrp, &data)),
            Err(PaymentError::InvoiceSignature)
        ));
        hrp.push('x');
        assert!(decode_invoice(&encode(&hrp, &data)).is_err());
        let uppercase = text.to_ascii_uppercase();
        assert!(decode_invoice(&uppercase).is_ok());
        let mixed = format!("L{}", &text[1..]);
        assert!(decode_invoice(&mixed).is_err());
    }
}
