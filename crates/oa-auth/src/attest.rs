//! App Attest for key accounts made on the iPhone (#11994).
//!
//! A key costs nothing, so the free allowances of an account that signs
//! in with only a Nostr key are bounded by the install it was made on. At
//! its first sign-in the phone asks `DCAppAttestService` for a key, has
//! Apple attest it over the sign-in challenge, and sends the attestation
//! with the signed challenge. [`Verifier::attestation`] checks it the way
//! Apple's "Validating apps that connect to your server" describes:
//!
//! 1. The `x5c` chain (credential certificate, then intermediate) verifies
//!    up to the Apple App Attestation Root CA ([`APPLE_ROOT_PEM`]), and
//!    every certificate is valid at the time given.
//! 2. The credential certificate's nonce extension
//!    (`1.2.840.113635.100.8.2`) is `SHA-256(authData || clientDataHash)`.
//! 3. The key id is the SHA-256 of the credential certificate's public key.
//! 4. `authData`'s relying-party hash is the SHA-256 of the app id
//!    (`<team>.<bundle>`), its counter is 0, its AAGUID names the
//!    production (or, when allowed, the development) environment, and its
//!    credential id is the key id.
//!
//! [`Verifier::assertion`] checks a later assertion from the same key: the
//! signature over `SHA-256(authData || clientDataHash)` under the attested key, the
//! app id, and a counter above the last one seen.
//!
//! [`Book`] records which account each attested key belongs to, so one
//! install makes at most one attested account. Nothing here talks to
//! Apple: verification is offline against the pinned root.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x509_parser::certificate::X509Certificate;
use x509_parser::time::ASN1Time;

/// The OpenAgents phone app's App Attest identity: team id, then bundle id.
pub const APP_ID: &str = "HQWSG26L43.com.openagents.app";

/// The Apple App Attestation Root CA, from
/// <https://www.apple.com/certificateauthority/Apple_App_Attestation_Root_CA.pem>.
pub const APPLE_ROOT_PEM: &str = "-----BEGIN CERTIFICATE-----
MIICITCCAaegAwIBAgIQC/O+DvHN0uD7jG5yH2IXmDAKBggqhkjOPQQDAzBSMSYw
JAYDVQQDDB1BcHBsZSBBcHAgQXR0ZXN0YXRpb24gUm9vdCBDQTETMBEGA1UECgwK
QXBwbGUgSW5jLjETMBEGA1UECAwKQ2FsaWZvcm5pYTAeFw0yMDAzMTgxODMyNTNa
Fw00NTAzMTUwMDAwMDBaMFIxJjAkBgNVBAMMHUFwcGxlIEFwcCBBdHRlc3RhdGlv
biBSb290IENBMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYDVQQIDApDYWxpZm9y
bmlhMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAERTHhmLW07ATaFQIEVwTtT4dyctdh
NbJhFs/Ii2FdCgAHGbpphY3+d8qjuDngIN3WVhQUBHAoMeQ/cLiP1sOUtgjqK9au
Yen1mMEvRq9Sk3Jm5X8U62H+xTD3FE9TgS41o0IwQDAPBgNVHRMBAf8EBTADAQH/
MB0GA1UdDgQWBBSskRBTM72+aEH/pwyp5frq5eWKoTAOBgNVHQ8BAf8EBAMCAQYw
CgYIKoZIzj0EAwMDaAAwZQIwQgFGnByvsiVbpTKwSga0kP0e8EeDS4+sQmTvb7vn
53O5+FRXgeLhpJ06ysC5PrOyAjEAp5U4xDgEgllF7En3VcE3iexZZtKeYnpqtijV
oyFraWVIyd/dganmrduC1bmTBGwD
-----END CERTIFICATE-----
";

/// The credential certificate's nonce extension.
const NONCE_OID: &str = "1.2.840.113635.100.8.2";
/// The AAGUID of a key made by a store, TestFlight, or enterprise build.
const AAGUID_PRODUCTION: &[u8; 16] = b"appattest\0\0\0\0\0\0\0";
/// The AAGUID of a key made by a development build.
const AAGUID_DEVELOPMENT: &[u8; 16] = b"appattestdevelop";
/// The largest attestation or assertion taken, bytes.
pub const MAX_OBJECT: usize = 16 * 1024;
/// The book's file, beside the registry.
pub const BOOK: &str = "app-attest.json";

/// Which App Attest environment made a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Production,
    Development,
}

/// An attested key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attested {
    /// The key id: SHA-256 of the public key.
    pub key_id: Vec<u8>,
    /// The public key, an uncompressed P-256 point (65 bytes).
    pub public_key: Vec<u8>,
    pub environment: Environment,
    /// Apple's receipt, for a later fraud-risk check.
    pub receipt: Vec<u8>,
}

/// Why an attestation or assertion was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttestError {
    /// It isn't the CBOR, DER, or layout App Attest makes.
    Malformed(&'static str),
    /// The format isn't `apple-appattest`.
    Format,
    /// The certificates don't lead to the pinned root.
    Chain,
    /// A certificate isn't valid at the time given.
    Expired,
    /// The nonce doesn't match the challenge.
    Nonce,
    /// The key id doesn't match the attested key.
    KeyId,
    /// It was made for another app.
    AppId,
    /// The counter isn't the one expected.
    Counter,
    /// It was made by a build of an environment that isn't accepted.
    Environment,
    /// The assertion's signature doesn't verify.
    Signature,
}

impl std::fmt::Display for AttestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Malformed(what) => return write!(f, "app attestation is malformed: {what}"),
            Self::Format => "app attestation has the wrong format",
            Self::Chain => "app attestation isn't signed by Apple",
            Self::Expired => "app attestation certificate is out of date",
            Self::Nonce => "app attestation doesn't match the challenge",
            Self::KeyId => "app attestation key id doesn't match its key",
            Self::AppId => "app attestation is for another app",
            Self::Counter => "app attestation counter is wrong",
            Self::Environment => "app attestation comes from a development build",
            Self::Signature => "app assertion signature doesn't verify",
        };
        f.write_str(text)
    }
}

impl std::error::Error for AttestError {}

/// `SHA-256(client data)`, the hash App Attest signs over.
#[must_use]
pub fn client_data_hash(client_data: &[u8]) -> [u8; 32] {
    Sha256::digest(client_data).into()
}

/// Base64 in any of the four common spellings (standard or URL alphabet,
/// padded or not).
#[must_use]
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .iter()
        .find_map(|engine| engine.decode(text).ok())
}

/// The verifier: the app id it accepts, the roots it trusts, and whether
/// development-build keys count.
#[derive(Clone, Debug)]
pub struct Verifier {
    app_id: String,
    roots: Vec<Vec<u8>>,
    allow_development: bool,
}

impl Verifier {
    /// Trusting Apple's App Attestation root.
    #[must_use]
    pub fn apple(app_id: &str, allow_development: bool) -> Self {
        let root = pem_der(APPLE_ROOT_PEM).expect("the pinned root is valid PEM");
        Self::with_roots(app_id, vec![root], allow_development)
    }

    /// Trusting the DER roots given (tests).
    #[must_use]
    pub fn with_roots(app_id: &str, roots: Vec<Vec<u8>>, allow_development: bool) -> Self {
        Self {
            app_id: app_id.to_owned(),
            roots,
            allow_development,
        }
    }

    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Checks an attestation object for `key_id` over `client_data_hash`
    /// at `now` (Unix seconds).
    ///
    /// # Errors
    ///
    /// What failed, as [`AttestError`].
    pub fn attestation(
        &self,
        object: &[u8],
        key_id: &[u8],
        client_data_hash: &[u8; 32],
        now: i64,
    ) -> Result<Attested, AttestError> {
        if object.len() > MAX_OBJECT {
            return Err(AttestError::Malformed("too large"));
        }
        let value = cbor::decode(object)?;
        if value.text_field("fmt")? != "apple-appattest" {
            return Err(AttestError::Format);
        }
        let statement = value.field("attStmt")?;
        let auth_data = value.bytes_field("authData")?;
        let chain: Vec<&[u8]> = match statement.field("x5c")? {
            cbor::Value::Array(items) => items
                .iter()
                .map(|item| match item {
                    cbor::Value::Bytes(bytes) => Ok(bytes.as_slice()),
                    _ => Err(AttestError::Malformed("x5c")),
                })
                .collect::<Result<_, _>>()?,
            _ => return Err(AttestError::Malformed("x5c")),
        };
        let receipt = statement
            .bytes_field("receipt")
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        if chain.len() < 2 || chain.len() > 4 {
            return Err(AttestError::Chain);
        }
        let certs = chain
            .iter()
            .map(|der| parse_cert(der))
            .collect::<Result<Vec<_>, _>>()?;
        let roots = self
            .roots
            .iter()
            .map(|der| parse_cert(der))
            .collect::<Result<Vec<_>, _>>()?;
        verify_chain(&certs, &roots, now)?;

        let leaf = &certs[0];
        let mut signed = auth_data.to_vec();
        signed.extend_from_slice(client_data_hash);
        let nonce: [u8; 32] = Sha256::digest(&signed).into();
        let extension = leaf
            .extensions()
            .iter()
            .find(|ext| ext.oid.to_id_string() == NONCE_OID)
            .ok_or(AttestError::Nonce)?;
        if nonce_of(extension.value)? != nonce {
            return Err(AttestError::Nonce);
        }
        let public_key = leaf.public_key().subject_public_key.data.to_vec();
        if public_key.len() != 65 || public_key[0] != 4 {
            return Err(AttestError::Malformed("public key"));
        }
        if Sha256::digest(&public_key).as_slice() != key_id {
            return Err(AttestError::KeyId);
        }
        let data = AuthData::parse(auth_data)?;
        if data.rp_id_hash != Sha256::digest(self.app_id.as_bytes()).as_slice() {
            return Err(AttestError::AppId);
        }
        if data.counter != 0 {
            return Err(AttestError::Counter);
        }
        let environment = match data.aaguid {
            Some(aaguid) if aaguid == AAGUID_PRODUCTION => Environment::Production,
            Some(aaguid) if aaguid == AAGUID_DEVELOPMENT => Environment::Development,
            _ => return Err(AttestError::Malformed("aaguid")),
        };
        if environment == Environment::Development && !self.allow_development {
            return Err(AttestError::Environment);
        }
        if data.credential_id != Some(key_id) {
            return Err(AttestError::KeyId);
        }
        Ok(Attested {
            key_id: key_id.to_vec(),
            public_key,
            environment,
            receipt,
        })
    }

    /// Checks an assertion over `client_data_hash` from the attested
    /// `public_key`, whose last counter was `previous`. Returns the new
    /// counter, to store.
    ///
    /// # Errors
    ///
    /// What failed, as [`AttestError`].
    pub fn assertion(
        &self,
        assertion: &[u8],
        client_data_hash: &[u8; 32],
        public_key: &[u8],
        previous: u32,
    ) -> Result<u32, AttestError> {
        if assertion.len() > MAX_OBJECT {
            return Err(AttestError::Malformed("too large"));
        }
        let value = cbor::decode(assertion)?;
        let signature = value.bytes_field("signature")?;
        let auth_data = value.bytes_field("authenticatorData")?;
        // The key signs the nonce `SHA-256(authData || clientDataHash)`
        // with ECDSA over SHA-256, so the nonce is the signed message.
        let mut signed = auth_data.to_vec();
        signed.extend_from_slice(client_data_hash);
        let nonce = Sha256::digest(&signed);
        ring::signature::UnparsedPublicKey::new(
            &ring::signature::ECDSA_P256_SHA256_ASN1,
            public_key,
        )
        .verify(&nonce, signature)
        .map_err(|_| AttestError::Signature)?;
        let data = AuthData::parse(auth_data)?;
        if data.rp_id_hash != Sha256::digest(self.app_id.as_bytes()).as_slice() {
            return Err(AttestError::AppId);
        }
        if data.counter <= previous {
            return Err(AttestError::Counter);
        }
        Ok(data.counter)
    }
}

fn parse_cert(der: &[u8]) -> Result<X509Certificate<'_>, AttestError> {
    match x509_parser::parse_x509_certificate(der) {
        Ok((rest, cert)) if rest.is_empty() => Ok(cert),
        _ => Err(AttestError::Malformed("certificate")),
    }
}

/// Each certificate is signed by the next, the last by one of `roots`,
/// and every one (roots included) is valid at `now`.
fn verify_chain(
    certs: &[X509Certificate<'_>],
    roots: &[X509Certificate<'_>],
    now: i64,
) -> Result<(), AttestError> {
    let time = ASN1Time::from_timestamp(now).map_err(|_| AttestError::Expired)?;
    for (index, cert) in certs.iter().enumerate() {
        if !cert.validity().is_valid_at(time) {
            return Err(AttestError::Expired);
        }
        if index > 0 && !cert.is_ca() {
            return Err(AttestError::Chain);
        }
        if let Some(issuer) = certs.get(index + 1) {
            signed_by(cert, issuer)?;
        }
    }
    let last = certs.last().ok_or(AttestError::Chain)?;
    let root = roots
        .iter()
        .find(|root| signed_by(last, root).is_ok())
        .ok_or(AttestError::Chain)?;
    if !root.is_ca() || !root.validity().is_valid_at(time) {
        return Err(AttestError::Chain);
    }
    Ok(())
}

fn signed_by(cert: &X509Certificate<'_>, issuer: &X509Certificate<'_>) -> Result<(), AttestError> {
    if cert.issuer().as_raw() != issuer.subject().as_raw() {
        return Err(AttestError::Chain);
    }
    cert.verify_signature(Some(issuer.public_key()))
        .map_err(|_| AttestError::Chain)
}

/// The nonce in the extension's value: `SEQUENCE { [1] { OCTET STRING } }`.
fn nonce_of(value: &[u8]) -> Result<[u8; 32], AttestError> {
    let bad = AttestError::Malformed("nonce extension");
    let (tag, sequence, _) = der_tlv(value).ok_or(bad)?;
    if tag != 0x30 {
        return Err(bad);
    }
    let (tag, tagged, _) = der_tlv(sequence).ok_or(bad)?;
    if tag != 0xa1 {
        return Err(bad);
    }
    let (tag, octets, _) = der_tlv(tagged).ok_or(bad)?;
    if tag != 0x04 {
        return Err(bad);
    }
    octets.try_into().map_err(|_| bad)
}

/// One DER tag, its contents, and what follows.
fn der_tlv(bytes: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = bytes.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (length, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 2 || rest.len() < count {
            return None;
        }
        let length = rest[..count]
            .iter()
            .fold(0_usize, |n, b| (n << 8) | usize::from(*b));
        (length, &rest[count..])
    };
    if rest.len() < length {
        return None;
    }
    Some((tag, &rest[..length], &rest[length..]))
}

/// WebAuthn authenticator data, as App Attest lays it out.
struct AuthData<'a> {
    rp_id_hash: &'a [u8],
    counter: u32,
    aaguid: Option<&'a [u8]>,
    credential_id: Option<&'a [u8]>,
}

impl<'a> AuthData<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, AttestError> {
        let bad = AttestError::Malformed("authenticator data");
        if bytes.len() < 37 {
            return Err(bad);
        }
        let flags = bytes[32];
        let counter = u32::from_be_bytes(bytes[33..37].try_into().map_err(|_| bad)?);
        let (mut aaguid, mut credential_id) = (None, None);
        if flags & 0x40 != 0 {
            if bytes.len() < 55 {
                return Err(bad);
            }
            aaguid = Some(&bytes[37..53]);
            let length = usize::from(u16::from_be_bytes([bytes[53], bytes[54]]));
            credential_id = Some(bytes.get(55..55 + length).ok_or(bad)?);
        }
        Ok(Self {
            rp_id_hash: &bytes[..32],
            counter,
            aaguid,
            credential_id,
        })
    }
}

/// The DER inside a PEM block.
fn pem_der(pem: &str) -> Option<Vec<u8>> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    STANDARD.decode(body.trim()).ok()
}

/// The little CBOR an attestation and an assertion use: definite-length
/// unsigned integers, byte and text strings, arrays, and maps.
mod cbor {
    use super::AttestError;

    #[derive(Debug)]
    pub enum Value {
        Unsigned,
        Bytes(Vec<u8>),
        Text(String),
        Array(Vec<Value>),
        Map(Vec<(Value, Value)>),
    }

    const BAD: AttestError = AttestError::Malformed("cbor");
    const MAX_DEPTH: usize = 8;

    pub fn decode(bytes: &[u8]) -> Result<Value, AttestError> {
        let mut at = 0;
        let value = item(bytes, &mut at, 0)?;
        if at != bytes.len() {
            return Err(BAD);
        }
        Ok(value)
    }

    fn item(bytes: &[u8], at: &mut usize, depth: usize) -> Result<Value, AttestError> {
        if depth > MAX_DEPTH {
            return Err(BAD);
        }
        let head = *bytes.get(*at).ok_or(BAD)?;
        *at += 1;
        let major = head >> 5;
        let argument = match head & 0x1f {
            small @ 0..=23 => u64::from(small),
            size @ 24..=27 => {
                let width = 1_usize << (size - 24);
                let slice = bytes.get(*at..*at + width).ok_or(BAD)?;
                *at += width;
                slice.iter().fold(0, |n, b| (n << 8) | u64::from(*b))
            }
            _ => return Err(BAD),
        };
        let length = || usize::try_from(argument).map_err(|_| BAD);
        let mut take = |n: usize| -> Result<Vec<u8>, AttestError> {
            let slice = bytes.get(*at..at.checked_add(n).ok_or(BAD)?).ok_or(BAD)?;
            *at += n;
            Ok(slice.to_vec())
        };
        Ok(match major {
            0 => Value::Unsigned,
            2 => Value::Bytes(take(length()?)?),
            3 => Value::Text(String::from_utf8(take(length()?)?).map_err(|_| BAD)?),
            4 => {
                let n = length()?;
                if n > bytes.len() {
                    return Err(BAD);
                }
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(item(bytes, at, depth + 1)?);
                }
                Value::Array(items)
            }
            5 => {
                let n = length()?;
                if n > bytes.len() {
                    return Err(BAD);
                }
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    let key = item(bytes, at, depth + 1)?;
                    let value = item(bytes, at, depth + 1)?;
                    entries.push((key, value));
                }
                Value::Map(entries)
            }
            _ => return Err(BAD),
        })
    }

    impl Value {
        pub fn field(&self, name: &str) -> Result<&Value, AttestError> {
            let Value::Map(entries) = self else {
                return Err(BAD);
            };
            entries
                .iter()
                .find(|(key, _)| matches!(key, Value::Text(text) if text == name))
                .map(|(_, value)| value)
                .ok_or(AttestError::Malformed("missing field"))
        }

        pub fn bytes_field(&self, name: &str) -> Result<&[u8], AttestError> {
            match self.field(name)? {
                Value::Bytes(bytes) => Ok(bytes),
                _ => Err(BAD),
            }
        }

        pub fn text_field(&self, name: &str) -> Result<&str, AttestError> {
            match self.field(name)? {
                Value::Text(text) => Ok(text),
                _ => Err(BAD),
            }
        }
    }
}

/// One attested key's record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRecord {
    pub account: String,
    /// The public key, hex.
    pub public_key: String,
    /// The last assertion counter seen (0 after attestation).
    pub counter: u32,
    pub environment: Environment,
    /// When it was attested, Unix seconds.
    pub attested_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// Key id (hex) to its record.
    #[serde(default)]
    keys: BTreeMap<String, KeyRecord>,
}

/// Why the book refused a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BookRefusal {
    /// The key already belongs to another account: one install, one
    /// attested account.
    TakenByOther,
    /// The key was never attested.
    Unknown,
    /// The book couldn't be written.
    Io(String),
}

/// Attested keys and their accounts, in one file beside the registry.
pub struct Book {
    path: PathBuf,
    saved: Saved,
}

impl Book {
    /// The book in `dir`; empty when it has no file yet.
    ///
    /// # Errors
    ///
    /// The file exists and can't be read.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let path = dir.join(BOOK);
        let saved = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Saved::default(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        Ok(Self { path, saved })
    }

    /// Binds an attested key to `account`. Binding the same key to the same
    /// account again changes nothing.
    ///
    /// # Errors
    ///
    /// The key belongs to another account, or the file can't be written.
    pub fn bind(
        &mut self,
        attested: &Attested,
        account: &str,
        now: u64,
    ) -> Result<(), BookRefusal> {
        let id = hex(&attested.key_id);
        if let Some(record) = self.saved.keys.get(&id) {
            return if record.account == account {
                Ok(())
            } else {
                Err(BookRefusal::TakenByOther)
            };
        }
        self.saved.keys.insert(
            id,
            KeyRecord {
                account: account.to_owned(),
                public_key: hex(&attested.public_key),
                counter: 0,
                environment: attested.environment,
                attested_at: now,
            },
        );
        self.save()
    }

    /// The record for a key id.
    #[must_use]
    pub fn key(&self, key_id: &[u8]) -> Option<&KeyRecord> {
        self.saved.keys.get(&hex(key_id))
    }

    /// Whether any attested key belongs to `account`.
    #[must_use]
    pub fn is_attested(&self, account: &str) -> bool {
        self.saved.keys.values().any(|r| r.account == account)
    }

    /// Stores a key's new assertion counter.
    ///
    /// # Errors
    ///
    /// The key is unknown, or the file can't be written.
    pub fn advance(&mut self, key_id: &[u8], counter: u32) -> Result<(), BookRefusal> {
        let record = self
            .saved
            .keys
            .get_mut(&hex(key_id))
            .ok_or(BookRefusal::Unknown)?;
        record.counter = record.counter.max(counter);
        self.save()
    }

    fn save(&self) -> Result<(), BookRefusal> {
        let bytes =
            serde_json::to_vec_pretty(&self.saved).map_err(|e| BookRefusal::Io(e.to_string()))?;
        let temp = self.path.with_extension("json.tmp");
        let write = || -> std::io::Result<()> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = options.open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temp, &self.path)
        };
        write().map_err(|e| BookRefusal::Io(e.to_string()))
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
#[path = "attest_tests.rs"]
mod tests;
