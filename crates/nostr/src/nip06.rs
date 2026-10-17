//! NIP-06: a Nostr key from BIP-39 recovery words.
//!
//! The words become a 64-byte BIP-39 seed with an empty passphrase, and
//! BIP-32 derives the key at `m/44'/1237'/<account>'/0/0`. Account `0` is
//! the one identity key a basic client uses.
//!
//! BIP-39 (the word list, the checksum, and PBKDF2) comes from the `bip39`
//! crate. BIP-32 is the few lines below, over `secp256k1` and a
//! hand-written HMAC-SHA512 in the same shape as `nip44`'s HMAC-SHA256.
//!
//! Secret types here have no `Debug`, so a key or a phrase cannot reach a
//! log line through `{:?}`. Errors never repeat the words.

use std::fmt;

use secp256k1::{Keypair, PublicKey, Scalar, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha512};

use crate::nip19;

/// The BIP-32 path NIP-06 derives for account `0`.
pub const PATH: &str = "m/44'/1237'/0'/0/0";

const HARDENED: u32 = 0x8000_0000;

/// Why recovery words or a derivation were refused. The messages are
/// written for the person who typed the words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nip06Error {
    /// No words were entered.
    Empty,
    /// The phrase has a word count other than 12 or 24.
    WordCount(usize),
    /// The word at this 1-based position isn't on the BIP-39 English list.
    UnknownWord(usize),
    /// Every word is on the list, but the phrase's checksum doesn't match.
    Checksum,
    /// Entropy for new words wasn't 16 or 32 bytes.
    EntropyLength(usize),
    /// The system couldn't supply randomness for new words.
    NoRandomness,
    /// The derivation produced an invalid key. BIP-32 gives this odds of
    /// about 1 in 2^127 per step.
    InvalidKey,
}

impl fmt::Display for Nip06Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("No recovery words were entered. Type your 12 or 24 words."),
            Self::WordCount(count) => {
                write!(f, "Enter 12 or 24 recovery words; that was {count}.")
            }
            Self::UnknownWord(position) => write!(
                f,
                "Word {position} isn't a recovery word. Check its spelling."
            ),
            Self::Checksum => f.write_str(
                "These words don't form a valid recovery phrase. Check each word and its order.",
            ),
            Self::EntropyLength(_) => f.write_str("New recovery words need 16 or 32 random bytes."),
            Self::NoRandomness => f.write_str("No randomness is available to make new words."),
            Self::InvalidKey => f.write_str("These words can't make a key. Make new words."),
        }
    }
}

impl std::error::Error for Nip06Error {}

/// A checked BIP-39 English phrase of 12 or 24 words. It has no `Debug`.
#[derive(Clone)]
pub struct Mnemonic(bip39::Mnemonic);

impl Mnemonic {
    /// Parse recovery words. Case and extra whitespace are ignored.
    pub fn parse(words: &str) -> Result<Self, Nip06Error> {
        let words: Vec<String> = words.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return Err(Nip06Error::Empty);
        }
        if words.len() != 12 && words.len() != 24 {
            return Err(Nip06Error::WordCount(words.len()));
        }
        bip39::Mnemonic::parse_in(bip39::Language::English, words.join(" "))
            .map(Self)
            .map_err(|error| match error {
                bip39::Error::UnknownWord(index) => Nip06Error::UnknownWord(index + 1),
                bip39::Error::BadWordCount(count) => Nip06Error::WordCount(count),
                _ => Nip06Error::Checksum,
            })
    }

    /// The phrase for 16 bytes (12 words) or 32 bytes (24 words) of entropy.
    pub fn from_entropy(entropy: &[u8]) -> Result<Self, Nip06Error> {
        if entropy.len() != 16 && entropy.len() != 32 {
            return Err(Nip06Error::EntropyLength(entropy.len()));
        }
        bip39::Mnemonic::from_entropy(entropy)
            .map(Self)
            .map_err(|_| Nip06Error::EntropyLength(entropy.len()))
    }

    /// New 12 words from 16 bytes of the operating system's randomness.
    pub fn generate() -> Result<Self, Nip06Error> {
        let mut entropy = [0_u8; 16];
        getrandom::fill(&mut entropy).map_err(|_| Nip06Error::NoRandomness)?;
        let mnemonic = Self::from_entropy(&entropy);
        entropy.fill(0);
        mnemonic
    }

    /// The words, in order.
    pub fn words(&self) -> Vec<&'static str> {
        self.0.words().collect()
    }

    /// The words joined by single spaces.
    pub fn phrase(&self) -> String {
        self.0.to_string()
    }

    /// The entropy the words encode: 16 or 32 bytes.
    pub fn entropy(&self) -> Vec<u8> {
        self.0.to_entropy()
    }

    /// The 64-byte BIP-39 seed with an empty passphrase.
    pub fn seed(&self) -> [u8; 64] {
        self.0.to_seed_normalized("")
    }

    /// The NIP-06 identity for account `0`.
    pub fn identity(&self) -> Result<Identity, Nip06Error> {
        self.identity_for_account(0)
    }

    /// The NIP-06 identity at `m/44'/1237'/<account>'/0/0`.
    pub fn identity_for_account(&self, account: u32) -> Result<Identity, Nip06Error> {
        let mut seed = self.seed();
        let secret = derive_secret_key(&seed, account);
        seed.fill(0);
        Identity::from_secret_key(secret?)
    }
}

/// A Nostr key pair derived by NIP-06. It has no `Debug`.
#[derive(Clone)]
pub struct Identity {
    secret: SecretKey,
    public: XOnlyPublicKey,
}

impl Identity {
    fn from_secret_key(secret: SecretKey) -> Result<Self, Nip06Error> {
        let (public, _) =
            Keypair::from_secret_key(&Secp256k1::signing_only(), &secret).x_only_public_key();
        Ok(Self { secret, public })
    }

    /// The secret key.
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret
    }

    /// The secret key's 32 bytes.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.secret.secret_bytes()
    }

    /// The key pair for signing events.
    pub fn keypair(&self) -> Keypair {
        Keypair::from_secret_key(&Secp256k1::signing_only(), &self.secret)
    }

    /// The x-only public key, as Nostr uses it.
    pub fn public_key(&self) -> XOnlyPublicKey {
        self.public
    }

    /// The public key as 64 lowercase hex characters.
    pub fn public_key_hex(&self) -> String {
        self.public.to_string()
    }

    /// The secret key as NIP-19 `nsec`.
    pub fn nsec(&self) -> String {
        nip19::encode_nsec(&self.secret.secret_bytes())
    }

    /// The public key as NIP-19 `npub`.
    pub fn npub(&self) -> String {
        nip19::encode_npub(&self.public.serialize())
    }
}

/// The secret key at `m/44'/1237'/<account>'/0/0` for a BIP-39 seed.
pub fn derive_secret_key(seed: &[u8], account: u32) -> Result<SecretKey, Nip06Error> {
    derive_path(
        seed,
        &[
            44 | HARDENED,
            1237 | HARDENED,
            (account & !HARDENED) | HARDENED,
            0,
            0,
        ],
    )
}

/// BIP-32 private derivation from `seed` along `path`. An index with the
/// high bit set is hardened.
#[doc(hidden)]
pub fn derive_path(seed: &[u8], path: &[u32]) -> Result<SecretKey, Nip06Error> {
    let master = hmac_sha512(b"Bitcoin seed", seed);
    let (mut key, mut chain) = split(&master)?;
    let secp = Secp256k1::signing_only();
    for &index in path {
        let mut data = Vec::with_capacity(37);
        if index & HARDENED != 0 {
            data.push(0);
            data.extend_from_slice(&key.secret_bytes());
        } else {
            data.extend_from_slice(&PublicKey::from_secret_key(&secp, &key).serialize());
        }
        data.extend_from_slice(&index.to_be_bytes());
        let output = hmac_sha512(&chain, &data);
        data.fill(0);
        let mut tweak_bytes = [0_u8; 32];
        tweak_bytes.copy_from_slice(&output[..32]);
        let tweak = Scalar::from_be_bytes(tweak_bytes).map_err(|_| Nip06Error::InvalidKey)?;
        key = key.add_tweak(&tweak).map_err(|_| Nip06Error::InvalidKey)?;
        chain.copy_from_slice(&output[32..]);
    }
    chain.fill(0);
    Ok(key)
}

fn split(output: &[u8; 64]) -> Result<(SecretKey, [u8; 32]), Nip06Error> {
    let mut key = [0_u8; 32];
    key.copy_from_slice(&output[..32]);
    let mut chain = [0_u8; 32];
    chain.copy_from_slice(&output[32..]);
    let secret = SecretKey::from_byte_array(key).map_err(|_| Nip06Error::InvalidKey);
    key.fill(0);
    Ok((secret?, chain))
}

/// HMAC-SHA512 (RFC 2104) over `sha2`. Public for the differential test.
#[doc(hidden)]
pub fn hmac_sha512(key: &[u8], message: &[u8]) -> [u8; 64] {
    let mut normalized = [0_u8; 128];
    if key.len() > normalized.len() {
        normalized[..64].copy_from_slice(&Sha512::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; 128];
    let mut outer_pad = [0x5c_u8; 128];
    for index in 0..128 {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    normalized.fill(0);
    let mut inner = Sha512::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha512::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    outer.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
            .collect()
    }

    // The two vectors in nips/official/06.md.
    const VECTORS: [(&str, &str, &str, &str, &str); 2] = [
        (
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
            "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a",
            "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp",
            "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917",
            "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu",
        ),
        (
            "what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade",
            "c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add",
            "nsec1c9wh8xy5eqdzln7n5t0ctgxjcrdug73gp5yj0x03gntn67h83twssdfhel",
            "d41b22899549e1f3d335a31002cfd382174006e166d3e658e3a5eecdb6463573",
            "npub16sdj9zv4f8sl85e45vgq9n7nsgt5qphpvmf7vk8r5hhvmdjxx4es8rq74h",
        ),
    ];

    #[test]
    fn nip06_vectors() {
        for (words, secret, nsec, public, npub) in VECTORS {
            let identity = Mnemonic::parse(words).unwrap().identity().unwrap();
            assert_eq!(hex(&identity.secret_bytes()), secret);
            assert_eq!(identity.nsec(), nsec);
            assert_eq!(identity.public_key_hex(), public);
            assert_eq!(identity.npub(), npub);
        }
    }

    #[test]
    fn parse_ignores_case_and_spacing() {
        let (words, secret, ..) = VECTORS[0];
        let messy = format!("  {}\n", words.to_uppercase().replace(' ', "   "));
        let identity = Mnemonic::parse(&messy).unwrap().identity().unwrap();
        assert_eq!(hex(&identity.secret_bytes()), secret);
    }

    #[test]
    fn path_matches_lane() {
        assert_eq!(PATH, crate::lane::nip06_path(0));
    }

    // BIP-32 test vector 1, private keys along m/0'/1/2'.
    #[test]
    fn bip32_vector_one() {
        let seed = unhex("000102030405060708090a0b0c0d0e0f");
        let cases: [(&[u32], &str); 4] = [
            (
                &[],
                "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35",
            ),
            (
                &[HARDENED],
                "edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea",
            ),
            (
                &[HARDENED, 1],
                "3c6cb8d0f6a264c91ea8b5030fadaa8e538b020f0a387421a12de9319dc93368",
            ),
            (
                &[HARDENED, 1, 2 | HARDENED],
                "cbce0d719ecf7431d88e6a89fa1483e02e35092af60c042b1df2ff59fa424dca",
            ),
        ];
        for (path, expected) in cases {
            assert_eq!(
                hex(&derive_path(&seed, path).unwrap().secret_bytes()),
                expected
            );
        }
    }

    #[test]
    fn accounts_differ() {
        let mnemonic = Mnemonic::parse(VECTORS[0].0).unwrap();
        let zero = mnemonic.identity_for_account(0).unwrap();
        let one = mnemonic.identity_for_account(1).unwrap();
        assert_ne!(zero.public_key(), one.public_key());
    }

    #[test]
    fn unknown_word_is_named_by_position() {
        let words = VECTORS[0].0.replace("parrot", "parrots");
        let error = Mnemonic::parse(&words).err().unwrap();
        assert_eq!(error, Nip06Error::UnknownWord(3));
        assert_eq!(
            error.to_string(),
            "Word 3 isn't a recovery word. Check its spelling."
        );
        assert!(!error.to_string().contains("parrots"));
    }

    #[test]
    fn bad_checksum_is_refused() {
        // Swapping two words keeps every word valid but breaks the checksum.
        let words = VECTORS[0].0.replace("leader monkey", "monkey leader");
        let error = Mnemonic::parse(&words).err().unwrap();
        assert_eq!(error, Nip06Error::Checksum);
        assert!(
            error
                .to_string()
                .contains("don't form a valid recovery phrase")
        );
    }

    #[test]
    fn wrong_count_and_empty_are_refused() {
        assert_eq!(Mnemonic::parse("  ").err(), Some(Nip06Error::Empty));
        assert_eq!(
            Mnemonic::parse("leader monkey parrot").err(),
            Some(Nip06Error::WordCount(3))
        );
    }

    #[test]
    fn generate_makes_twelve_words_that_round_trip() {
        let mnemonic = Mnemonic::generate().unwrap();
        assert_eq!(mnemonic.words().len(), 12);
        assert_eq!(mnemonic.entropy().len(), 16);
        let again = Mnemonic::parse(&mnemonic.phrase()).unwrap();
        assert_eq!(
            again.identity().unwrap().npub(),
            mnemonic.identity().unwrap().npub()
        );
        assert_ne!(Mnemonic::generate().unwrap().phrase(), mnemonic.phrase());
    }

    #[test]
    fn from_entropy_checks_length() {
        assert_eq!(
            Mnemonic::from_entropy(&[0; 15]).err(),
            Some(Nip06Error::EntropyLength(15))
        );
        assert_eq!(Mnemonic::from_entropy(&[0; 32]).unwrap().words().len(), 24);
    }

    // RFC 4231 test cases 1, 2, and 6 (key longer than the block).
    #[test]
    fn hmac_sha512_rfc4231() {
        assert_eq!(
            hex(&hmac_sha512(&[0x0b; 20], b"Hi There")),
            "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854"
        );
        assert_eq!(
            hex(&hmac_sha512(b"Jefe", b"what do ya want for nothing?")),
            "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737"
        );
        assert_eq!(
            hex(&hmac_sha512(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "80b24263c7c1a3ebb71493c1dd7be8b49b46d1f41b4aeec1121b013783f8f3526b56d037e05f2598bd0fd2215d6a1e5295e64f73f63f0aec8b915a985d786598"
        );
    }
}
