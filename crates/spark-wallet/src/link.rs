//! The sealed envelope that carries the wallet seed from the phone to a
//! computer the owner approved (`openagents wallet link`).
//!
//! The computer makes a one-time key ([`Requester`]) for each link and shows
//! a six-digit code derived from its public half ([`code`]). The phone shows
//! the same code beside the computer's name; when the owner approves, the
//! phone seals the seed's entropy to that public key with NIP-44 v2 from a
//! fresh key of its own ([`seal`]). Only the computer process that holds the
//! one-time secret can open the envelope ([`Requester::open`]); the host that
//! relays it, the relay, and the request book on disk see ciphertext. The
//! one-time secret lives in memory only, and the seed is never printed.

use crate::seed::Seed;
use nostr::nip44;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The envelope's version tag.
pub const ENVELOPE: &str = "openagents.wallet-link.v1";

/// The plaintext inside the envelope.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inner {
    v: String,
    /// The seed's BIP39 entropy, lowercase hex.
    entropy: String,
}

/// The envelope as it travels: the phone's one-time public key and the
/// NIP-44 payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sealed {
    pub v: String,
    /// The sender's one-time x-only public key, hex.
    pub from: String,
    /// NIP-44 v2 ciphertext of the inner document.
    pub payload: String,
}

/// The computer's one-time key for one link. It has no `Debug` and is never
/// written anywhere.
pub struct Requester {
    secret: SecretKey,
    public: XOnlyPublicKey,
}

impl Requester {
    /// A fresh one-time key.
    ///
    /// # Errors
    ///
    /// When the system has no randomness to give.
    pub fn new() -> Result<Self, String> {
        let secret = random_secret()?;
        let public = Keypair::from_secret_key(&Secp256k1::new(), &secret)
            .x_only_public_key()
            .0;
        Ok(Self { secret, public })
    }

    /// The public key the phone seals to, hex.
    #[must_use]
    pub fn public_hex(&self) -> String {
        hex(&self.public.serialize())
    }

    /// The code both screens show for this link.
    #[must_use]
    pub fn code(&self) -> String {
        code(&self.public_hex()).unwrap_or_default()
    }

    /// Open an envelope sealed to this key.
    ///
    /// # Errors
    ///
    /// A document that is not this envelope, was sealed to another key, or
    /// carries no valid seed. The message never contains the seed.
    pub fn open(&self, sealed: &Sealed) -> Result<Seed, String> {
        if sealed.v != ENVELOPE {
            return Err("The phone sent a wallet in a format this computer can't read.".into());
        }
        let from = parse_public(&sealed.from)?;
        let key = nip44::conversation_key(&self.secret, &from);
        let text = nip44::decrypt(&sealed.payload, &key)
            .map_err(|_| "The phone's reply could not be opened.".to_string())?;
        let inner: Inner = serde_json::from_str(&text)
            .map_err(|_| "The phone's reply could not be read.".to_string())?;
        if inner.v != ENVELOPE {
            return Err("The phone's reply could not be read.".into());
        }
        let entropy = unhex(&inner.entropy)?;
        Seed::from_entropy(entropy)
    }
}

/// The six-digit code for a requester's public key: `123 456`.
///
/// # Errors
///
/// When `public_hex` is not an x-only public key.
pub fn code(public_hex: &str) -> Result<String, String> {
    let public = parse_public(public_hex)?;
    let digest = Sha256::new()
        .chain_update(b"openagents-wallet-link-code-v1")
        .chain_update(public.serialize())
        .finalize();
    let number = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    let digits = format!("{number:06}");
    Ok(format!("{} {}", &digits[..3], &digits[3..]))
}

/// Seal `seed` to the computer's one-time public key, from a fresh key of
/// the phone's.
///
/// # Errors
///
/// When `to_hex` is not an x-only public key or no randomness is available.
pub fn seal(seed: &Seed, to_hex: &str) -> Result<Sealed, String> {
    let to = parse_public(to_hex)?;
    let secret = random_secret()?;
    let from = Keypair::from_secret_key(&Secp256k1::new(), &secret)
        .x_only_public_key()
        .0;
    let key = nip44::conversation_key(&secret, &to);
    let mut nonce = [0u8; 32];
    getrandom::fill(&mut nonce).map_err(|_| "No randomness is available.".to_string())?;
    let inner = serde_json::to_string(&Inner {
        v: ENVELOPE.into(),
        entropy: hex(&seed.entropy),
    })
    .map_err(|_| "The wallet could not be sealed.".to_string())?;
    let payload = nip44::encrypt(&inner, &key, nonce)
        .map_err(|_| "The wallet could not be sealed.".to_string())?;
    Ok(Sealed {
        v: ENVELOPE.into(),
        from: hex(&from.serialize()),
        payload,
    })
}

fn random_secret() -> Result<SecretKey, String> {
    loop {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| "No randomness is available.".to_string())?;
        if let Ok(secret) = SecretKey::from_byte_array(bytes) {
            return Ok(secret);
        }
    }
}

fn parse_public(text: &str) -> Result<XOnlyPublicKey, String> {
    let bytes = unhex(text)?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "That isn't a link key.".to_string())?;
    XOnlyPublicKey::from_byte_array(bytes).map_err(|_| "That isn't a link key.".to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("The phone's reply could not be read.".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16))
        .collect::<Result<_, _>>()
        .map_err(|_| "The phone's reply could not be read.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> Seed {
        Seed::from_entropy(vec![7u8; 16]).expect("a seed")
    }

    #[test]
    fn only_the_requester_opens_the_envelope() {
        let computer = Requester::new().expect("a key");
        let sealed = seal(&seed(), &computer.public_hex()).expect("sealed");
        // The envelope carries neither the entropy nor the words.
        let wire = serde_json::to_string(&sealed).expect("json");
        assert!(!wire.contains(&hex(&seed().entropy)));
        assert!(!wire.contains(seed().mnemonic.split(' ').next().unwrap()));
        let opened = computer.open(&sealed).expect("opened");
        assert_eq!(opened.entropy, seed().entropy);
        assert_eq!(opened.mnemonic, seed().mnemonic);
        // Another computer's key cannot open it.
        let other = Requester::new().expect("a key");
        assert!(other.open(&sealed).is_err());
        // A changed payload is refused, and the refusal names no secret.
        let mut tampered = sealed.clone();
        tampered.payload.replace_range(10..11, "A");
        let refused = computer.open(&tampered).err().unwrap_or_default();
        assert!(!refused.contains(&seed().mnemonic));
    }

    #[test]
    fn both_screens_show_the_same_six_digits() {
        let computer = Requester::new().expect("a key");
        let shown = computer.code();
        assert_eq!(code(&computer.public_hex()).unwrap(), shown);
        assert_eq!(shown.len(), 7);
        assert!(shown[..3].chars().all(|c| c.is_ascii_digit()));
        assert_eq!(&shown[3..4], " ");
        assert!(code("nothex").is_err());
    }
}
