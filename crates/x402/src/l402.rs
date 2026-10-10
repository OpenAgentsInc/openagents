//! L402 on the payment router (#11139): the same invoice as x402 and the
//! `Payment` scheme, with a macaroon bound to its payment hash and to this
//! request.
//!
//! - **The challenge.** `WWW-Authenticate: L402 macaroon="…", invoice="…"`
//!   and the same with the older scheme name `LSAT`. The macaroon is the
//!   binary V2 format clients such as Lightning Labs' `lnget` and
//!   `aperture` read, base64; its identifier is aperture's (version `0`,
//!   the payment hash, a token id), and its first-party caveats are ours:
//!   the resource (`POST https://…/v1/responses`), the request's `http:1`
//!   hash, the price, the expiry, how many calls it opens, and the invoice
//!   (public: it is in the `402` already).
//! - **The credential.** `Authorization: L402 <macaroon>:<preimage>` (or
//!   `LSAT`). A macaroon we didn't sign, one past a caveat, or a malformed
//!   value is answered `401`. A good one settles through the shared
//!   Lightning facilitator, so it consumes the invoice's replay key
//!   (`lnbtc:<network>:<payment_hash>`): a preimage that already paid by
//!   x402 or the `Payment` scheme cannot also open an L402 call.
//! - **Reuse.** A token opens at most `calls` calls (its caveat) before its
//!   invoice expires. The first call consumes the shared key, marked as
//!   opened by L402; each later one consumes `l402:<payment_hash>:<n>`.
//!   A key consumed by another encoding never opens a second call.
//!
//! No preimage or macaroon is kept: the replay store holds the public
//! payment hash, and receipts hold the replay key.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use nostr::x402::decode_invoice;
use serde_json::{Map, json};

use crate::facilitator::DUPLICATE_SETTLEMENT;
use crate::payment_scheme::{AUTHORIZATION, Problem, WWW_AUTHENTICATE, hmac_sha256};
use crate::replay::{ReplayEntry, ReplayError};
use crate::router::{
    Adapter, Bound, Challenged, Method, MethodInfo, Offer, Refused, Settle, Settled, settled,
};
use crate::wire::PaymentPayload;

/// The scheme name.
pub const SCHEME: &str = "L402";
/// The scheme's earlier name, still sent and accepted.
pub const LEGACY_SCHEME: &str = "LSAT";

/// libmacaroons' and go-macaroon's key derivation constant.
const KEY_GENERATOR: &[u8] = b"macaroons-key-generator";
/// The longest `Authorization` value read.
const MAX_CREDENTIAL_BYTES: usize = 16 * 1024;
/// The identifier version aperture writes.
const ID_VERSION: u16 = 0;
/// Marks a shared replay key as consumed by an L402 call, so its token may
/// open its later calls.
const PURCHASE_TAG: &str = "l402|";

const RESOURCE: &str = "openagents_resource";
const REQUEST: &str = "openagents_request";
const AMOUNT: &str = "openagents_amount_msat";
const EXPIRES: &str = "openagents_expires";
const CALLS: &str = "openagents_calls";
const INVOICE: &str = "openagents_invoice";

/// A macaroon with first-party caveats only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Macaroon {
    pub location: String,
    pub identifier: Vec<u8>,
    pub caveats: Vec<String>,
    pub signature: [u8; 32],
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn put_uvarint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn put_field(out: &mut Vec<u8>, kind: u8, data: &[u8]) {
    out.push(kind);
    put_uvarint(out, data.len() as u64);
    out.extend_from_slice(data);
}

/// Field types of the V2 binary format.
const EOS: u8 = 0;
const LOCATION: u8 = 1;
const IDENTIFIER: u8 = 2;
const SIGNATURE: u8 = 6;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn uvarint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    /// One field: its type and data. `EOS` has no data.
    fn field(&mut self) -> Option<(u8, &'a [u8])> {
        let kind = self.byte()?;
        if kind == EOS {
            return Some((EOS, &[]));
        }
        let len = usize::try_from(self.uvarint()?).ok()?;
        let end = self.at.checked_add(len)?;
        let data = self.bytes.get(self.at..end)?;
        self.at = end;
        Some((kind, data))
    }
}

impl Macaroon {
    /// Mint a macaroon under `root_key`: the signature chain is
    /// HMAC-SHA256 from the derived key over the identifier, then over each
    /// caveat in order.
    #[must_use]
    pub fn mint(
        root_key: &[u8],
        location: &str,
        identifier: Vec<u8>,
        caveats: Vec<String>,
    ) -> Self {
        let mut signature = hmac_sha256(&hmac_sha256(KEY_GENERATOR, root_key), &identifier);
        for caveat in &caveats {
            signature = hmac_sha256(&signature, caveat.as_bytes());
        }
        Self {
            location: location.to_owned(),
            identifier,
            caveats,
            signature,
        }
    }

    /// Whether `root_key` signed exactly this identifier and these caveats.
    #[must_use]
    pub fn verify(&self, root_key: &[u8]) -> bool {
        let expected = Self::mint(
            root_key,
            &self.location,
            self.identifier.clone(),
            self.caveats.clone(),
        );
        ct_eq(&expected.signature, &self.signature)
    }

    /// The V2 binary encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![2u8];
        if !self.location.is_empty() {
            put_field(&mut out, LOCATION, self.location.as_bytes());
        }
        put_field(&mut out, IDENTIFIER, &self.identifier);
        out.push(EOS);
        for caveat in &self.caveats {
            put_field(&mut out, IDENTIFIER, caveat.as_bytes());
            out.push(EOS);
        }
        out.push(EOS);
        put_field(&mut out, SIGNATURE, &self.signature);
        out
    }

    /// Read the V2 binary encoding. Third-party caveats are refused.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader { bytes, at: 0 };
        if reader.byte()? != 2 {
            return None;
        }
        let mut location = String::new();
        let (mut kind, mut data) = reader.field()?;
        if kind == LOCATION {
            location = String::from_utf8(data.to_vec()).ok()?;
            (kind, data) = reader.field()?;
        }
        if kind != IDENTIFIER {
            return None;
        }
        let identifier = data.to_vec();
        if reader.field()?.0 != EOS {
            return None;
        }
        let mut caveats = Vec::new();
        loop {
            if reader.peek()? == EOS {
                reader.byte()?;
                break;
            }
            let (mut kind, mut data) = reader.field()?;
            if kind == LOCATION {
                (kind, data) = reader.field()?;
            }
            if kind != IDENTIFIER {
                return None;
            }
            caveats.push(String::from_utf8(data.to_vec()).ok()?);
            // A verification id (`VID`) would make it a third-party
            // caveat, which this server never issues.
            let (kind, _) = reader.field()?;
            if kind != EOS {
                return None;
            }
            if caveats.len() > 64 {
                return None;
            }
        }
        let (kind, data) = reader.field()?;
        if kind != SIGNATURE || reader.at != bytes.len() {
            return None;
        }
        Some(Self {
            location,
            identifier,
            caveats,
            signature: data.try_into().ok()?,
        })
    }

    /// Standard base64 of the V2 encoding, as L402 headers carry it.
    #[must_use]
    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.encode())
    }

    /// Read base64 in any of its common forms.
    #[must_use]
    pub fn from_base64(text: &str) -> Option<Self> {
        let text = text.trim();
        [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD]
            .iter()
            .find_map(|engine| engine.decode(text).ok())
            .and_then(|bytes| Self::decode(&bytes))
    }

    /// The payment hash an aperture identifier carries.
    #[must_use]
    pub fn payment_hash(&self) -> Option<[u8; 32]> {
        if self.identifier.len() != 66
            || u16::from_be_bytes([self.identifier[0], self.identifier[1]]) != ID_VERSION
        {
            return None;
        }
        self.identifier[2..34].try_into().ok()
    }
}

/// An aperture identifier: version, payment hash, token id.
#[must_use]
pub fn identifier(payment_hash: &[u8; 32], token_id: &[u8; 32]) -> Vec<u8> {
    let mut out = ID_VERSION.to_be_bytes().to_vec();
    out.extend_from_slice(payment_hash);
    out.extend_from_slice(token_id);
    out
}

/// The caveats this server writes, read back.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Caveats {
    resource: String,
    request: String,
    amount_msat: u64,
    expires: u64,
    calls: u32,
    invoice: String,
}

impl Caveats {
    fn write(&self) -> Vec<String> {
        vec![
            format!("{RESOURCE}={}", self.resource),
            format!("{REQUEST}={}", self.request),
            format!("{AMOUNT}={}", self.amount_msat),
            format!("{EXPIRES}={}", self.expires),
            format!("{CALLS}={}", self.calls),
            format!("{INVOICE}={}", self.invoice),
        ]
    }

    /// Every caveat must be one of ours, each once; a caveat this server
    /// doesn't know can't be satisfied, so the macaroon is refused.
    fn read(caveats: &[String]) -> Option<Self> {
        let mut fields: [Option<&str>; 6] = [None; 6];
        for caveat in caveats {
            let (name, value) = caveat.split_once('=')?;
            let slot = [RESOURCE, REQUEST, AMOUNT, EXPIRES, CALLS, INVOICE]
                .iter()
                .position(|known| *known == name)?;
            if fields[slot].replace(value).is_some() {
                return None;
            }
        }
        let [resource, request, amount, expires, calls, invoice] = fields;
        Some(Self {
            resource: resource?.to_owned(),
            request: request?.to_owned(),
            amount_msat: amount?.parse().ok()?,
            expires: expires?.parse().ok()?,
            calls: calls?.parse().ok().filter(|calls| *calls >= 1)?,
            invoice: invoice?.to_owned(),
        })
    }
}

/// The L402 credential on a request: the scheme name's length and the
/// token after it.
fn token(value: &str) -> Option<&str> {
    let value = value.trim_start();
    for scheme in [SCHEME, LEGACY_SCHEME] {
        if value
            .get(..scheme.len() + 1)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&format!("{scheme} ")))
        {
            return Some(value[scheme.len() + 1..].trim());
        }
    }
    None
}

/// Whether an `Authorization` value uses L402 or LSAT.
#[must_use]
pub fn is_l402_authorization(value: &str) -> bool {
    token(value).is_some()
}

/// L402 on the shared invoice. The root key signs every macaroon; every
/// process that settles must share it.
pub struct L402Lightning {
    location: String,
    root_key: Vec<u8>,
    calls: u32,
}

impl L402Lightning {
    /// A token opens `calls` calls (at least one).
    ///
    /// # Errors
    ///
    /// A sentence when the key is shorter than 32 bytes, the location is
    /// empty, or `calls` is zero.
    pub fn new(location: impl Into<String>, root_key: Vec<u8>, calls: u32) -> Result<Self, String> {
        let location = location.into();
        if root_key.len() < 32 {
            return Err("The L402 root key must be at least 32 bytes.".into());
        }
        if location.is_empty() {
            return Err("The L402 location is empty.".into());
        }
        if calls == 0 {
            return Err("An L402 token must open at least one call.".into());
        }
        Ok(Self {
            location,
            root_key,
            calls,
        })
    }

    fn unauthorized(reason: &str) -> Refused {
        Refused::payment_required(Method::L402, reason, Problem::MalformedCredential)
            .with_status(401)
    }

    fn refused(reason: &str) -> Refused {
        Refused::payment_required(Method::L402, reason, Problem::from_reason(reason))
    }

    /// The macaroon this server issues for `offer`.
    fn macaroon(&self, offer: &Offer<'_>) -> Option<Macaroon> {
        let mut payment_hash = [0u8; 32];
        hex::decode_to_slice(&offer.invoice.payment_hash, &mut payment_hash).ok()?;
        let mut seed = payment_hash.to_vec();
        seed.extend_from_slice(offer.request.request_hash.as_bytes());
        let token_id = hmac_sha256(&self.root_key, &seed);
        let caveats = Caveats {
            resource: format!("{} {}", offer.request.http_method, offer.request.url),
            request: offer.request.request_hash.to_owned(),
            amount_msat: offer.quote.amount_msat,
            expires: offer.invoice.expires_at,
            calls: self.calls,
            invoice: offer.invoice.bolt11.clone(),
        };
        Some(Macaroon::mint(
            &self.root_key,
            &self.location,
            identifier(&payment_hash, &token_id),
            caveats.write(),
        ))
    }

    /// A later call on a token whose first call consumed the shared key.
    fn reuse(&self, ctx: &Settle<'_>, payment_hash: &str, calls: u32) -> Result<Settled, Refused> {
        let store = ctx.lightning.replay();
        let key = format!("{}:{payment_hash}", ctx.lightning.network());
        let opened = match store.get(&key) {
            Ok(Some(entry)) if entry.purchase.starts_with(PURCHASE_TAG) => entry,
            Ok(_) => return Err(Self::refused(DUPLICATE_SETTLEMENT)),
            Err(_) => return Err(Self::refused("replay_store_unavailable")),
        };
        for n in 2..=calls {
            let entry = ReplayEntry {
                key: format!("l402:{payment_hash}:{n}"),
                network: opened.network.clone(),
                payment_hash: payment_hash.to_owned(),
                amount_msat: 0,
                consumed_at: ctx.now,
                retain_until: opened.retain_until,
                purchase: format!("{PURCHASE_TAG}{}", ctx.purchase),
            };
            match store.insert(&entry) {
                Ok(()) => {
                    return Ok(Settled {
                        method: Method::L402,
                        replay_key: entry.key,
                        payment_hash: Some(payment_hash.to_owned()),
                        network: opened.network,
                        asset: "BTC",
                        amount: "0".into(),
                        rail: "lightning",
                        headers: Vec::new(),
                    });
                }
                Err(ReplayError::Duplicate(_)) => {}
                Err(_) => return Err(Self::refused("replay_store_unavailable")),
            }
        }
        Err(Self::refused("l402_token_used"))
    }
}

impl Adapter for L402Lightning {
    fn method(&self) -> Method {
        Method::L402
    }

    fn info(&self, network: &'static str) -> MethodInfo {
        MethodInfo {
            id: "l402",
            name: "L402 on Lightning",
            protocol: "l402",
            rail: "lightning",
            network: network.to_owned(),
            asset: "BTC",
            challenge: "WWW-Authenticate: L402",
            credential: "Authorization: L402",
            receipt: "none (the token is the receipt)",
            spec: "https://github.com/lightninglabs/L402",
        }
    }

    fn credential<'r>(&self, request: &'r Bound<'_>) -> Option<&'r str> {
        request
            .header(AUTHORIZATION)
            .filter(|value| is_l402_authorization(value))
    }

    fn challenge(&self, offer: &Offer<'_>, out: &mut Challenged) {
        let Some(macaroon) = self.macaroon(offer) else {
            return;
        };
        let macaroon = macaroon.to_base64();
        for scheme in [SCHEME, LEGACY_SCHEME] {
            out.headers.push((
                WWW_AUTHENTICATE.to_owned(),
                format!(
                    "{scheme} macaroon=\"{macaroon}\", invoice=\"{}\"",
                    offer.invoice.bolt11
                ),
            ));
        }
    }

    fn settle(&self, ctx: &Settle<'_>, credential: &str) -> Result<Settled, Refused> {
        if credential.len() > MAX_CREDENTIAL_BYTES {
            return Err(Self::unauthorized("malformed_credential"));
        }
        let (macaroon, preimage) = token(credential)
            .and_then(|token| token.rsplit_once(':'))
            .ok_or_else(|| Self::unauthorized("malformed_credential"))?;
        let macaroon = Macaroon::from_base64(macaroon)
            .filter(|macaroon| macaroon.verify(&self.root_key))
            .ok_or_else(|| Self::unauthorized("invalid_macaroon"))?;
        let caveats = Caveats::read(&macaroon.caveats)
            .ok_or_else(|| Self::unauthorized("invalid_macaroon"))?;
        let payment_hash = macaroon
            .payment_hash()
            .ok_or_else(|| Self::unauthorized("invalid_macaroon"))?;
        if caveats.expires < ctx.now {
            return Err(Self::unauthorized("macaroon_expired"));
        }
        if caveats.resource != format!("{} {}", ctx.request.http_method, ctx.request.url)
            || caveats.request != ctx.request.request_hash
        {
            return Err(Self::unauthorized("macaroon_wrong_resource"));
        }
        if caveats.amount_msat != ctx.quote.amount_msat {
            return Err(Self::refused("invalid_exact_lnbtc_amount_mismatch"));
        }
        let invoice_hash = decode_invoice(&caveats.invoice)
            .ok()
            .map(|invoice| invoice.payment_hash());
        if invoice_hash != Some(payment_hash) {
            return Err(Self::unauthorized("invalid_macaroon"));
        }
        let terms = ctx.lightning.requirements(
            ctx.request.request_hash,
            &caveats.invoice,
            ctx.quote.amount_msat,
        );
        let mut proof = Map::new();
        proof.insert("preimage".into(), json!(preimage.trim()));
        let payload = PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted: terms.clone(),
            payload: proof,
            extensions: None,
        };
        let purchase = format!("{PURCHASE_TAG}{}", ctx.purchase);
        match ctx.lightning.settle(&terms, &payload, &purchase, ctx.now) {
            Ok(admitted) => Ok(settled(Method::L402, &admitted, Vec::new())),
            Err(settlement) => {
                let reason = settlement
                    .error_reason
                    .unwrap_or_else(|| "settlement_failed".into());
                if reason == DUPLICATE_SETTLEMENT {
                    return self.reuse(ctx, &hex::encode(payment_hash), caveats.calls);
                }
                Err(Self::refused(&reason))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_macaroon_round_trips_and_its_chain_binds_every_caveat() {
        let id = identifier(&[7; 32], &[9; 32]);
        let minted = Macaroon::mint(
            &[1; 32],
            "api.example.com",
            id,
            vec!["a=1".into(), "b=2".into()],
        );
        let read = Macaroon::from_base64(&minted.to_base64()).unwrap();
        assert_eq!(read, minted);
        assert!(read.verify(&[1; 32]));
        assert!(!read.verify(&[2; 32]));
        assert_eq!(read.payment_hash(), Some([7; 32]));
        // Dropping or changing a caveat breaks the signature.
        let mut fewer = read.clone();
        fewer.caveats.pop();
        assert!(!fewer.verify(&[1; 32]));
        let mut changed = read.clone();
        changed.caveats[0] = "a=2".into();
        assert!(!changed.verify(&[1; 32]));
        // URL-safe and unpadded forms read the same.
        let url = URL_SAFE_NO_PAD.encode(minted.encode());
        assert_eq!(Macaroon::from_base64(&url), Some(minted.clone()));
        // Trailing bytes and a wrong version are refused.
        let mut bytes = minted.encode();
        bytes.push(0);
        assert!(Macaroon::decode(&bytes).is_none());
        let mut bytes = minted.encode();
        bytes[0] = 1;
        assert!(Macaroon::decode(&bytes).is_none());
    }

    #[test]
    fn the_signature_is_go_macaroons_chain() {
        // go-macaroon: key = HMAC(\"macaroons-key-generator\", root);
        // sig = HMAC(key, id); sig = HMAC(sig, caveat) per caveat.
        let root = b"this is our super secret key; only we should know it";
        let key = hmac_sha256(KEY_GENERATOR, root);
        let mut expected = hmac_sha256(&key, b"we used our secret key");
        expected = hmac_sha256(&expected, b"account = 3735928559");
        let minted = Macaroon::mint(
            root,
            "http://mybank/",
            b"we used our secret key".to_vec(),
            vec!["account = 3735928559".into()],
        );
        assert_eq!(minted.signature, expected);
    }

    #[test]
    fn caveats_must_all_be_ours_and_once() {
        let caveats = Caveats {
            resource: "POST https://a/v1/responses".into(),
            request: "ab".repeat(32),
            amount_msat: 21_000,
            expires: 10,
            calls: 1,
            invoice: "lnbc1".into(),
        };
        let written = caveats.write();
        assert_eq!(Caveats::read(&written), Some(caveats));
        let mut extra = written.clone();
        extra.push("time < 2030".into());
        assert!(Caveats::read(&extra).is_none());
        let mut twice = written.clone();
        twice.push(format!("{CALLS}=5"));
        assert!(Caveats::read(&twice).is_none());
        let mut zero = written;
        zero[4] = format!("{CALLS}=0");
        assert!(Caveats::read(&zero).is_none());
    }

    #[test]
    fn both_scheme_names_are_read() {
        assert_eq!(token("L402 mac:pre"), Some("mac:pre"));
        assert_eq!(token("lsat mac:pre"), Some("mac:pre"));
        assert_eq!(token("Payment abc"), None);
        assert_eq!(token("Bearer oak_x"), None);
    }
}
