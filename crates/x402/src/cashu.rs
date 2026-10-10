//! Cashu NUT-24 on the payment router (#11140): ecash at the mints the
//! owner accepts.
//!
//! - **The challenge.** `X-Cashu: creqA…`, a NUT-18 payment request (CBOR,
//!   base64url) naming the accepted mints, the unit (`sat`), and the price
//!   in whole sats, with no transport: the token comes back in the header.
//! - **The credential.** `X-Cashu: cashuB…`, a NUT-00 V4 token. A token
//!   from a mint not on the list, in another unit, or short of the price is
//!   answered `400` and nothing is consumed.
//! - **Settling.** Each proof's point `Y = hash_to_curve(secret)` is
//!   consumed once in the router's replay store (`cashu:<mint id>:<Y>`),
//!   then the proofs are swapped at the mint (NUT-03) for new ones only we
//!   hold, before the request runs. A swap the mint refuses (a double
//!   spend) gives the keys back and answers nothing.
//!
//! Cashu is its own profile, never a fallback for x402: NIP-X402's
//! "Wallets" section says so. The [`Mint`] trait is the network side; the
//! gateway supplies one over HTTPS ([`MintWallet`] does the NUT-03 swap
//! over any [`MintHttp`]). Proof secrets are bearer money: they are only
//! ever written to the private proof store, never to a log or a receipt.

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, URL_SAFE};
use base64::engine::DecodePaddingMode;
use secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::cbor::Cbor;
use crate::payment_scheme::Problem;
use crate::replay::{ReplayEntry, ReplayError};
use crate::router::{Adapter, Bound, Challenged, Method, MethodInfo, Offer, Refused, Settle, Settled};

/// The header both directions use.
pub const HEADER: &str = "x-cashu";
/// A NUT-18 payment request's prefix (version A).
pub const REQUEST_PREFIX: &str = "creqA";
/// A NUT-00 V4 token's prefix.
pub const TOKEN_PREFIX: &str = "cashuB";
/// The unit every price here is in.
pub const UNIT: &str = "sat";
/// The NUT-00 domain separator for `hash_to_curve`.
const DOMAIN_SEPARATOR: &[u8] = b"Secp256k1_HashToCurve_Cashu_";
/// The longest token read.
const MAX_TOKEN_BYTES: usize = 64 * 1024;
/// How long a spent proof's key is kept: a proof never becomes spendable
/// again, so this only bounds the store.
const RETAIN_SECS: u64 = 400 * 86_400;

/// Base64url that reads padded and unpadded text alike.
const URL_SAFE_ANY: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// A mint URL as compared: no trailing slash.
#[must_use]
pub fn normalize_mint(url: &str) -> String {
    url.trim().trim_end_matches('/').to_owned()
}

/// A NUT-18 payment request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaymentRequest {
    pub id: Option<String>,
    pub amount: Option<u64>,
    pub unit: Option<String>,
    pub single_use: Option<bool>,
    pub mints: Vec<String>,
    pub description: Option<String>,
}

fn text(value: &str) -> Cbor {
    Cbor::Text(value.to_owned())
}

impl PaymentRequest {
    /// `creqA` and base64url of its CBOR.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut entries = Vec::new();
        if let Some(id) = &self.id {
            entries.push((text("i"), text(id)));
        }
        if let Some(amount) = self.amount {
            entries.push((text("a"), Cbor::Uint(amount)));
        }
        if let Some(unit) = &self.unit {
            entries.push((text("u"), text(unit)));
        }
        if let Some(single_use) = self.single_use {
            entries.push((text("s"), Cbor::Bool(single_use)));
        }
        if !self.mints.is_empty() {
            entries.push((
                text("m"),
                Cbor::Array(self.mints.iter().map(|mint| text(mint)).collect()),
            ));
        }
        if let Some(description) = &self.description {
            entries.push((text("d"), text(description)));
        }
        format!(
            "{REQUEST_PREFIX}{}",
            URL_SAFE.encode(Cbor::Map(entries).encode())
        )
    }

    /// Read a `creqA…` request.
    #[must_use]
    pub fn decode(value: &str) -> Option<Self> {
        let body = value.trim().strip_prefix(REQUEST_PREFIX)?;
        let map = Cbor::decode(&URL_SAFE_ANY.decode(body).ok()?)?;
        if !matches!(map, Cbor::Map(_)) {
            return None;
        }
        let mints = match map.get("m") {
            None | Some(Cbor::Null) => Vec::new(),
            Some(mints) => mints
                .as_array()?
                .iter()
                .map(|mint| mint.as_text().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
        };
        Some(Self {
            id: map.get("i").and_then(Cbor::as_text).map(str::to_owned),
            amount: map.get("a").and_then(Cbor::as_u64),
            unit: map.get("u").and_then(Cbor::as_text).map(str::to_owned),
            single_use: map.get("s").and_then(Cbor::as_bool),
            mints,
            description: map.get("d").and_then(Cbor::as_text).map(str::to_owned),
        })
    }
}

/// One proof: an amount, its keyset, the secret, and the mint's signature
/// `C`, hex. The secret is bearer money.
#[derive(Clone, PartialEq, Eq)]
pub struct Proof {
    pub amount: u64,
    /// The keyset id, hex.
    pub keyset_id: String,
    pub secret: String,
    /// The unblinded signature, compressed point hex.
    pub c: String,
    pub witness: Option<String>,
}

impl std::fmt::Debug for Proof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the secret.
        f.debug_struct("Proof")
            .field("amount", &self.amount)
            .field("keyset_id", &self.keyset_id)
            .finish_non_exhaustive()
    }
}

impl Proof {
    /// The proof as a mint's API reads it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut proof = json!({
            "amount": self.amount,
            "id": self.keyset_id,
            "secret": self.secret,
            "C": self.c,
        });
        if let Some(witness) = &self.witness {
            proof["witness"] = json!(witness);
        }
        proof
    }

    /// `Y = hash_to_curve(secret)`, compressed hex: the proof's public id.
    #[must_use]
    pub fn y(&self) -> Option<String> {
        hash_to_curve(self.secret.as_bytes()).map(|point| hex::encode(point.serialize()))
    }
}

/// A NUT-00 V4 token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub mint: String,
    pub unit: String,
    pub memo: Option<String>,
    pub proofs: Vec<Proof>,
}

impl Token {
    /// Read a `cashuB…` token.
    #[must_use]
    pub fn decode(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.len() > MAX_TOKEN_BYTES {
            return None;
        }
        let body = value.strip_prefix(TOKEN_PREFIX)?;
        let map = Cbor::decode(&URL_SAFE_ANY.decode(body).ok()?)?;
        let mint = map.get("m")?.as_text()?.to_owned();
        let unit = map.get("u")?.as_text()?.to_owned();
        let memo = map.get("d").and_then(Cbor::as_text).map(str::to_owned);
        let mut proofs = Vec::new();
        for group in map.get("t")?.as_array()? {
            let keyset_id = hex::encode(group.get("i")?.as_bytes()?);
            for proof in group.get("p")?.as_array()? {
                let c = proof.get("c")?.as_bytes()?;
                if c.len() != 33 {
                    return None;
                }
                proofs.push(Proof {
                    amount: proof.get("a")?.as_u64()?,
                    keyset_id: keyset_id.clone(),
                    secret: proof.get("s")?.as_text()?.to_owned(),
                    c: hex::encode(c),
                    witness: proof.get("w").and_then(Cbor::as_text).map(str::to_owned),
                });
            }
        }
        if proofs.is_empty() || proofs.len() > 256 {
            return None;
        }
        Some(Self {
            mint,
            unit,
            memo,
            proofs,
        })
    }

    /// `cashuB` and base64url of its CBOR, proofs grouped by keyset.
    #[must_use]
    pub fn encode(&self) -> Option<String> {
        let mut groups: Vec<(String, Vec<Cbor>)> = Vec::new();
        for proof in &self.proofs {
            let mut entries = vec![
                (text("a"), Cbor::Uint(proof.amount)),
                (text("s"), text(&proof.secret)),
                (text("c"), Cbor::Bytes(hex::decode(&proof.c).ok()?)),
            ];
            if let Some(witness) = &proof.witness {
                entries.push((text("w"), text(witness)));
            }
            let item = Cbor::Map(entries);
            match groups.iter_mut().find(|(id, _)| *id == proof.keyset_id) {
                Some((_, items)) => items.push(item),
                None => groups.push((proof.keyset_id.clone(), vec![item])),
            }
        }
        let mut groups_cbor = Vec::new();
        for (id, items) in groups {
            groups_cbor.push(Cbor::Map(vec![
                (text("i"), Cbor::Bytes(hex::decode(&id).ok()?)),
                (text("p"), Cbor::Array(items)),
            ]));
        }
        let mut entries = vec![(text("m"), text(&self.mint)), (text("u"), text(&self.unit))];
        if let Some(memo) = &self.memo {
            entries.push((text("d"), text(memo)));
        }
        entries.push((text("t"), Cbor::Array(groups_cbor)));
        Some(format!(
            "{TOKEN_PREFIX}{}",
            URL_SAFE.encode(Cbor::Map(entries).encode())
        ))
    }

    /// The token's value in its unit, `None` on overflow.
    #[must_use]
    pub fn amount(&self) -> Option<u64> {
        self.proofs
            .iter()
            .try_fold(0u64, |sum, proof| sum.checked_add(proof.amount))
    }
}

/// NUT-00 `hash_to_curve`: the first `02 || SHA256(SHA256(separator ||
/// message) || counter_le32)` that is a curve point.
#[must_use]
pub fn hash_to_curve(message: &[u8]) -> Option<PublicKey> {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_SEPARATOR);
    hasher.update(message);
    let msg_hash = hasher.finalize();
    for counter in 0u32..=u32::from(u16::MAX) {
        let mut hasher = Sha256::new();
        hasher.update(&msg_hash);
        hasher.update(counter.to_le_bytes());
        let hash = hasher.finalize();
        let mut candidate = [0u8; 33];
        candidate[0] = 0x02;
        candidate[1..].copy_from_slice(&hash);
        if let Ok(point) = PublicKey::from_slice(&candidate) {
            return Some(point);
        }
    }
    None
}

/// 32 random bytes from the operating system.
fn random32() -> Option<[u8; 32]> {
    let mut bytes = [0u8; 32];
    // SAFETY: the buffer is 32 writable bytes, at most getentropy's limit.
    let rc = unsafe { libc::getentropy(bytes.as_mut_ptr().cast(), bytes.len()) };
    (rc == 0).then_some(bytes)
}

/// A blinded output for a swap: what the mint signs, and what we keep to
/// unblind its signature.
pub struct Blinded {
    pub amount: u64,
    pub keyset_id: String,
    /// `B_ = Y + rG`, compressed hex.
    pub b: String,
    secret: String,
    r: SecretKey,
}

/// Blind `secret` with `r` (NUT-00 step one).
#[must_use]
pub fn blind(amount: u64, keyset_id: &str, secret: String, r: SecretKey) -> Option<Blinded> {
    let secp = Secp256k1::verification_only();
    let y = hash_to_curve(secret.as_bytes())?;
    let tweak = Scalar::from_be_bytes(r.secret_bytes()).ok()?;
    let b = y.add_exp_tweak(&secp, &tweak).ok()?;
    Some(Blinded {
        amount,
        keyset_id: keyset_id.to_owned(),
        b: hex::encode(b.serialize()),
        secret,
        r,
    })
}

/// Unblind the mint's `C_` with `r` and its key `K` for the amount:
/// `C = C_ - rK` (NUT-00 step three).
#[must_use]
pub fn unblind(c_blinded: &PublicKey, r: &SecretKey, k: &PublicKey) -> Option<PublicKey> {
    let secp = Secp256k1::verification_only();
    let tweak = Scalar::from_be_bytes(r.secret_bytes()).ok()?;
    let rk = k.mul_tweak(&secp, &tweak).ok()?;
    c_blinded.combine(&rk.negate(&secp)).ok()
}

/// `amount` as powers of two, smallest first.
#[must_use]
pub fn split(amount: u64) -> Vec<u64> {
    (0..64)
        .map(|bit| 1u64 << bit)
        .filter(|part| amount & part != 0)
        .collect()
}

/// Why a mint didn't swap a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintRefusal {
    /// A proof is already spent (NUT-00 error 11001).
    Spent,
    /// The mint refused for another reason.
    Refused(String),
    /// The mint couldn't be reached.
    Unreachable(String),
}

/// The network side of settling ecash: swap a buyer's token for new
/// proofs only this receiver holds. `Ok` with what was kept, in the unit.
pub trait Mint: Send + Sync {
    /// # Errors
    ///
    /// [`MintRefusal`] when the swap didn't happen.
    fn swap(&self, token: &Token) -> Result<u64, MintRefusal>;
}

/// The two HTTP calls a NUT-03 swap needs.
pub trait MintHttp: Send + Sync {
    /// GET `url`, its JSON body.
    ///
    /// # Errors
    ///
    /// A sentence when the mint can't be reached or answers non-JSON.
    fn get(&self, url: &str) -> Result<Value, String>;
    /// POST `body` as JSON to `url`: the status and the JSON answer.
    ///
    /// # Errors
    ///
    /// A sentence when the mint can't be reached or answers non-JSON.
    fn post(&self, url: &str, body: &Value) -> Result<(u16, Value), String>;
}

/// Where received proofs are kept: one private file per proof, named by
/// its `Y`, written once.
pub struct FileProofStore {
    dir: PathBuf,
}

impl FileProofStore {
    /// # Errors
    ///
    /// When the directory can't be made private to this user.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Keep `proofs` from `mint`.
    ///
    /// # Errors
    ///
    /// The first write that failed; earlier proofs stay written.
    pub fn keep(&self, mint: &str, unit: &str, proofs: &[Proof]) -> std::io::Result<()> {
        for proof in proofs {
            let y = proof
                .y()
                .ok_or_else(|| std::io::Error::other("a proof's secret has no curve point"))?;
            let mut record = proof.to_json();
            record["mint"] = json!(mint);
            record["unit"] = json!(unit);
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.dir.join(format!("{y}.json")))?;
            file.write_all(record.to_string().as_bytes())?;
            file.sync_all()?;
        }
        Ok(())
    }

    /// What is kept, per mint, in each mint's unit.
    ///
    /// # Errors
    ///
    /// An I/O failure reading the directory.
    pub fn balances(&self) -> std::io::Result<std::collections::BTreeMap<String, u64>> {
        let mut out = std::collections::BTreeMap::new();
        for item in fs::read_dir(&self.dir)? {
            let path = item?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if let (Some(mint), Some(amount)) = (
                record["mint"].as_str(),
                record["amount"].as_u64(),
            ) {
                *out.entry(mint.to_owned()).or_insert(0) += amount;
            }
        }
        Ok(out)
    }
}

/// A NUT-03 swap over [`MintHttp`], keeping the new proofs in a
/// [`FileProofStore`].
pub struct MintWallet<H: MintHttp> {
    http: H,
    store: FileProofStore,
}

/// The mint's error code for a spent proof.
const SPENT: u64 = 11_001;

impl<H: MintHttp> MintWallet<H> {
    #[must_use]
    pub fn new(http: H, store: FileProofStore) -> Self {
        Self { http, store }
    }

    #[must_use]
    pub fn store(&self) -> &FileProofStore {
        &self.store
    }
}

fn refused(message: impl Into<String>) -> MintRefusal {
    MintRefusal::Refused(message.into())
}

impl<H: MintHttp> Mint for MintWallet<H> {
    fn swap(&self, token: &Token) -> Result<u64, MintRefusal> {
        let mint = normalize_mint(&token.mint);
        let keysets = self
            .http
            .get(&format!("{mint}/v1/keysets"))
            .map_err(MintRefusal::Unreachable)?;
        let keysets = keysets["keysets"]
            .as_array()
            .ok_or_else(|| refused("The mint listed no keysets."))?;
        let fee_of = |id: &str| -> Option<u64> {
            keysets
                .iter()
                .find(|keyset| keyset["id"].as_str() == Some(id))
                .map(|keyset| keyset["input_fee_ppk"].as_u64().unwrap_or(0))
        };
        let mut fee_ppk = 0u64;
        for proof in &token.proofs {
            let fee = fee_of(&proof.keyset_id)
                .ok_or_else(|| refused("A proof is from a keyset the mint doesn't list."))?;
            fee_ppk = fee_ppk.saturating_add(fee);
        }
        let fee = fee_ppk.div_ceil(1000);
        let total = token
            .amount()
            .ok_or_else(|| refused("The token's amount overflows."))?;
        let kept = total
            .checked_sub(fee)
            .filter(|kept| *kept > 0)
            .ok_or_else(|| refused("The token doesn't cover the mint's fee."))?;
        let active = keysets
            .iter()
            .filter(|keyset| {
                keyset["active"].as_bool() == Some(true)
                    && keyset["unit"].as_str() == Some(token.unit.as_str())
            })
            .min_by_key(|keyset| keyset["input_fee_ppk"].as_u64().unwrap_or(0))
            .and_then(|keyset| keyset["id"].as_str())
            .ok_or_else(|| refused("The mint has no active keyset in this unit."))?
            .to_owned();
        let keys = self
            .http
            .get(&format!("{mint}/v1/keys/{active}"))
            .map_err(MintRefusal::Unreachable)?;
        let keys = keys["keysets"]
            .as_array()
            .and_then(|sets| sets.iter().find(|set| set["id"].as_str() == Some(&active)))
            .and_then(|set| set["keys"].as_object())
            .ok_or_else(|| refused("The mint didn't list its keys."))?;
        let key_for = |amount: u64| -> Option<PublicKey> {
            let hex_key = keys.get(&amount.to_string())?.as_str()?;
            PublicKey::from_slice(&hex::decode(hex_key).ok()?).ok()
        };
        let mut outputs = Vec::new();
        for amount in split(kept) {
            key_for(amount).ok_or_else(|| refused("The mint has no key for an amount."))?;
            let secret = hex::encode(
                random32().ok_or_else(|| refused("No randomness for a new proof."))?,
            );
            let r = random32()
                .and_then(|bytes| SecretKey::from_byte_array(bytes).ok())
                .ok_or_else(|| refused("No randomness for a blinding factor."))?;
            outputs.push(
                blind(amount, &active, secret, r)
                    .ok_or_else(|| refused("A new proof couldn't be blinded."))?,
            );
        }
        let body = json!({
            "inputs": token.proofs.iter().map(Proof::to_json).collect::<Vec<_>>(),
            "outputs": outputs.iter().map(|output| json!({
                "amount": output.amount, "id": output.keyset_id, "B_": output.b,
            })).collect::<Vec<_>>(),
        });
        let (status, answer) = self
            .http
            .post(&format!("{mint}/v1/swap"), &body)
            .map_err(MintRefusal::Unreachable)?;
        if status != 200 {
            if answer["code"].as_u64() == Some(SPENT) {
                return Err(MintRefusal::Spent);
            }
            return Err(refused(format!(
                "The mint refused the swap ({}).",
                answer["detail"].as_str().unwrap_or("no reason given")
            )));
        }
        let signatures = answer["signatures"]
            .as_array()
            .filter(|signatures| signatures.len() == outputs.len())
            .ok_or_else(|| refused("The mint's answer didn't match the outputs."))?;
        let mut kept_proofs = Vec::new();
        for (output, signature) in outputs.iter().zip(signatures) {
            let c_blinded = signature["C_"]
                .as_str()
                .and_then(|text| hex::decode(text).ok())
                .and_then(|bytes| PublicKey::from_slice(&bytes).ok());
            let c = c_blinded
                .zip(key_for(output.amount))
                .filter(|_| signature["amount"].as_u64() == Some(output.amount))
                .and_then(|(c_blinded, k)| unblind(&c_blinded, &output.r, &k));
            match c {
                Some(c) => kept_proofs.push(Proof {
                    amount: output.amount,
                    keyset_id: output.keyset_id.clone(),
                    secret: output.secret.clone(),
                    c: hex::encode(c.serialize()),
                    witness: None,
                }),
                None => return Err(refused("A signature from the mint didn't unblind.")),
            }
        }
        if let Err(error) = self.store.keep(&mint, &token.unit, &kept_proofs) {
            // The swap happened: the buyer paid. Say so for the operator,
            // without any secret.
            eprintln!(
                "{}",
                json!({"event": "cashu_proofs_unkept", "mint": mint, "amount": kept,
                       "error": error.to_string()})
            );
        }
        Ok(kept)
    }
}

/// Cashu NUT-24 on the router: the accepted mints and the mint client.
pub struct CashuNut24 {
    mints: Vec<String>,
    mint: Arc<dyn Mint>,
}

impl CashuNut24 {
    /// # Errors
    ///
    /// A sentence when no mint is listed or one isn't an `https` (or, for
    /// local tests, `http`) URL.
    pub fn new(mints: &[String], mint: Arc<dyn Mint>) -> Result<Self, String> {
        let mints: Vec<String> = mints.iter().map(|mint| normalize_mint(mint)).collect();
        if mints.is_empty() {
            return Err("List at least one accepted Cashu mint.".into());
        }
        if let Some(bad) = mints
            .iter()
            .find(|mint| !(mint.starts_with("https://") || mint.starts_with("http://")))
        {
            return Err(format!("The Cashu mint {bad} isn't an https URL."));
        }
        Ok(Self { mints, mint })
    }

    fn bad(reason: &str) -> Refused {
        Refused::payment_required(Method::Cashu, reason, Problem::VerificationFailed)
            .with_status(400)
    }

    fn refused(reason: &str) -> Refused {
        Refused::payment_required(Method::Cashu, reason, Problem::VerificationFailed)
    }

    /// The short id of a mint in replay keys (a URL has `/` in it).
    fn mint_id(mint: &str) -> String {
        hex::encode(&Sha256::digest(mint.as_bytes())[..16])
    }
}

/// The price in whole sats, rounded up.
fn sats(amount_msat: u64) -> u64 {
    amount_msat.div_ceil(1000)
}

impl Adapter for CashuNut24 {
    fn method(&self) -> Method {
        Method::Cashu
    }

    fn info(&self, _network: &'static str) -> MethodInfo {
        MethodInfo {
            id: "cashu",
            name: "Cashu ecash (NUT-24)",
            protocol: "cashu",
            rail: "ecash",
            network: format!("cashu:{}", self.mints.join(",")),
            asset: "sat-ecash",
            challenge: "X-Cashu: creqA",
            credential: "X-Cashu: cashuB",
            receipt: "none",
            spec: "https://github.com/cashubtc/nuts/blob/main/24.md",
        }
    }

    fn credential<'r>(&self, request: &'r Bound<'_>) -> Option<&'r str> {
        request
            .header(HEADER)
            .filter(|value| value.trim_start().starts_with(TOKEN_PREFIX))
    }

    fn challenge(&self, offer: &Offer<'_>, out: &mut Challenged) {
        let request = PaymentRequest {
            id: Some(offer.invoice.payment_hash.chars().take(16).collect()),
            amount: Some(sats(offer.quote.amount_msat)),
            unit: Some(UNIT.to_owned()),
            single_use: Some(true),
            mints: self.mints.clone(),
            description: offer.resource.description.clone(),
        };
        out.headers.push((HEADER.to_owned(), request.encode()));
    }

    fn settle(&self, ctx: &Settle<'_>, credential: &str) -> Result<Settled, Refused> {
        let mut token = Token::decode(credential).ok_or_else(|| Self::bad("malformed_token"))?;
        token.mint = normalize_mint(&token.mint);
        if !self.mints.contains(&token.mint) {
            return Err(Self::bad("cashu_mint_not_accepted"));
        }
        if token.unit != UNIT {
            return Err(Self::bad("cashu_unit_mismatch"));
        }
        let total = token.amount().ok_or_else(|| Self::bad("malformed_token"))?;
        if total < sats(ctx.quote.amount_msat) {
            return Err(Self::bad("cashu_amount_short"));
        }
        let mint_id = Self::mint_id(&token.mint);
        let mut keys = Vec::new();
        for proof in &token.proofs {
            let y = proof.y().ok_or_else(|| Self::bad("malformed_token"))?;
            let key = format!("cashu:{mint_id}:{y}");
            if keys.iter().any(|(seen, _)| *seen == key) {
                return Err(Self::bad("malformed_token"));
            }
            keys.push((key, proof.amount));
        }
        let store = ctx.lightning.replay();
        let release = |taken: &[String]| {
            for key in taken {
                let _ = store.release(key);
            }
        };
        let network = format!("cashu:{}", token.mint);
        let mut taken = Vec::new();
        for (key, amount) in &keys {
            let entry = ReplayEntry {
                key: key.clone(),
                network: network.clone(),
                payment_hash: key.rsplit(':').next().unwrap_or_default().to_owned(),
                amount_msat: amount.saturating_mul(1000),
                consumed_at: ctx.now,
                retain_until: ctx.now.saturating_add(RETAIN_SECS),
                purchase: ctx.purchase.to_owned(),
            };
            match store.insert(&entry) {
                Ok(()) => taken.push(key.clone()),
                Err(ReplayError::Duplicate(_)) => {
                    release(&taken);
                    return Err(Self::refused(crate::facilitator::DUPLICATE_SETTLEMENT));
                }
                Err(_) => {
                    release(&taken);
                    return Err(Self::refused("replay_store_unavailable"));
                }
            }
        }
        match self.mint.swap(&token) {
            Ok(_) => Ok(Settled {
                method: Method::Cashu,
                replay_key: taken[0].clone(),
                payment_hash: None,
                network,
                asset: "sat-ecash",
                amount: total.to_string(),
                rail: "ecash",
                headers: Vec::new(),
            }),
            Err(refusal) => {
                release(&taken);
                Err(match refusal {
                    MintRefusal::Spent => Self::refused("cashu_token_spent"),
                    MintRefusal::Refused(_) => Self::bad("cashu_swap_refused"),
                    MintRefusal::Unreachable(_) => Self::refused("cashu_mint_unreachable"),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_to_curve_matches_nut_00() {
        // NUT-00 test vectors.
        let vector = |x: &str| {
            hex::encode(
                hash_to_curve(&hex::decode(x).unwrap())
                    .unwrap()
                    .serialize(),
            )
        };
        assert_eq!(
            vector("0000000000000000000000000000000000000000000000000000000000000000"),
            "024cce997d3b518f739663b757deaec95bcd9473c30a14ac2fd04023a739d1a725"
        );
        assert_eq!(
            vector("0000000000000000000000000000000000000000000000000000000000000001"),
            "022e7158e11c9506f1aa4248bf531298daa7febd6194f003edcd9b93ade6253acf"
        );
    }

    #[test]
    fn a_payment_request_round_trips() {
        let request = PaymentRequest {
            id: Some("b7a90176".into()),
            amount: Some(21),
            unit: Some("sat".into()),
            single_use: Some(true),
            mints: vec!["https://mint.example.com".into()],
            description: Some("One call".into()),
        };
        let encoded = request.encode();
        assert!(encoded.starts_with("creqA"));
        assert_eq!(PaymentRequest::decode(&encoded), Some(request));
    }

    #[test]
    fn a_token_round_trips_and_never_shows_its_secret() {
        let point = hash_to_curve(b"c").unwrap();
        let token = Token {
            mint: "https://mint.example.com".into(),
            unit: "sat".into(),
            memo: None,
            proofs: vec![
                Proof {
                    amount: 1,
                    keyset_id: "00ad268c4d1f5826".into(),
                    secret: "secret-one".into(),
                    c: hex::encode(point.serialize()),
                    witness: None,
                },
                Proof {
                    amount: 4,
                    keyset_id: "00ad268c4d1f5826".into(),
                    secret: "secret-two".into(),
                    c: hex::encode(point.serialize()),
                    witness: None,
                },
            ],
        };
        let encoded = token.encode().unwrap();
        assert!(encoded.starts_with("cashuB"));
        let read = Token::decode(&encoded).unwrap();
        assert_eq!(read, token);
        assert_eq!(read.amount(), Some(5));
        assert!(!format!("{read:?}").contains("secret-one"));
    }

    #[test]
    fn blinding_then_signing_then_unblinding_gives_k_times_y() {
        let secp = Secp256k1::new();
        let k = SecretKey::from_byte_array([3; 32]).unwrap();
        let big_k = PublicKey::from_secret_key(&secp, &k);
        let r = SecretKey::from_byte_array([5; 32]).unwrap();
        let output = blind(8, "00ad268c4d1f5826", "abc".into(), r).unwrap();
        // The mint signs: C_ = k * B_.
        let b = PublicKey::from_slice(&hex::decode(&output.b).unwrap()).unwrap();
        let scalar = Scalar::from_be_bytes(k.secret_bytes()).unwrap();
        let c_blinded = b.mul_tweak(&secp, &scalar).unwrap();
        let c = unblind(&c_blinded, &output.r, &big_k).unwrap();
        let expected = hash_to_curve(b"abc").unwrap().mul_tweak(&secp, &scalar).unwrap();
        assert_eq!(c, expected);
    }

    #[test]
    fn amounts_split_into_powers_of_two() {
        assert_eq!(split(13), vec![1, 4, 8]);
        assert_eq!(split(0), Vec::<u64>::new());
    }
}
