//! The `sealed` tier's wrap (NIP-VAULT, "Wraps"): HPKE RFC 9180 `mode_psk`
//! with DHKEM(X25519, HKDF-SHA256), HKDF-SHA256 and AES-256-GCM. Opening
//! needs the system key's private half **and** the person's share `S_user`;
//! either alone fails. A client makes the wrap without contacting the
//! workload, from a system key certificate it verified elsewhere.
//!
//! The session request (`mode_base` to the attested endpoint's key) and the
//! reply sealing use the same suite. Nothing here talks to a network.
use crate::keys::{Dek, Vmk};
use crate::object::Wrap;
use crate::{Error, Result, b64, hex, unb64, unb64_n};
use hpke::{
    Deserializable, Kem as KemTrait, OpModeR, OpModeS, PskBundle, Serializable,
    aead::AesGcm256,
    kdf::HkdfSha256,
    kem::X25519HkdfSha256,
    rand_core::{CryptoRng, RngCore},
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

type Kem = X25519HkdfSha256;

/// The suite string a `sealed` wrap carries.
pub const SUITE: &str = "x25519-hkdf-sha256/hkdf-sha256/aes-256-gcm";
const PSK_ID_PREFIX: &[u8] = b"openagents.vault.v1/";
const WRAP_INFO: &[u8] = b"openagents.vault.v1/wrap-sealed\0";
const REQUEST_INFO: &[u8] = b"openagents.vault.v1/request\0";
const REPLY_INFO: &[u8] = b"openagents.vault.v1/reply\0";

/// The operating system's (or browser's) random source for HPKE.
pub struct OsRandom;

impl RngCore for OsRandom {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        getrandom::fill(dest).expect("the random source failed");
    }
    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> std::result::Result<(), hpke::rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
impl CryptoRng for OsRandom {}

/// A deterministic stream from a seed, for test vectors only.
pub struct SeededRandom {
    seed: [u8; 32],
    counter: u64,
}

impl SeededRandom {
    pub fn new(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }
}

impl RngCore for SeededRandom {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(32) {
            let mut hasher = Sha256::new();
            hasher.update(self.seed);
            hasher.update(self.counter.to_be_bytes());
            self.counter += 1;
            let block = hasher.finalize();
            chunk.copy_from_slice(&block[..chunk.len()]);
        }
    }
    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> std::result::Result<(), hpke::rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
impl CryptoRng for SeededRandom {}

/// An X25519 key pair for a system key, a session, or a reply key. The
/// private half never implements `Debug` and is zeroed on drop.
pub struct KeyPair {
    private: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}

impl KeyPair {
    /// A key pair derived from input keying material (RFC 9180 DeriveKeyPair).
    pub fn derive(ikm: &[u8]) -> Self {
        let (private, public) = Kem::derive_keypair(ikm);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&private.to_bytes());
        let mut public_bytes = [0u8; 32];
        public_bytes.copy_from_slice(&public.to_bytes());
        Self {
            private: Zeroizing::new(bytes),
            public: public_bytes,
        }
    }
    pub fn generate() -> Result<Self> {
        Ok(Self::derive(&crate::random::<32>()?))
    }
    pub fn public(&self) -> [u8; 32] {
        self.public
    }
    pub fn private(&self) -> &[u8; 32] {
        &self.private
    }
    pub fn from_private(private: [u8; 32]) -> Result<Self> {
        let key = <Kem as KemTrait>::PrivateKey::from_bytes(&private)
            .map_err(|_| Error::Format("The private key is not an X25519 key."))?;
        let public = <Kem as KemTrait>::sk_to_pk(&key);
        let mut public_bytes = [0u8; 32];
        public_bytes.copy_from_slice(&public.to_bytes());
        Ok(Self {
            private: Zeroizing::new(private),
            public: public_bytes,
        })
    }
}

/// The system key's public id: lowercase hex SHA-256 of the 32 raw bytes.
pub fn system_key_id(public: &[u8; 32]) -> String {
    hex(&Sha256::digest(public))
}

fn public_key(bytes: &[u8; 32]) -> Result<<Kem as KemTrait>::PublicKey> {
    <Kem as KemTrait>::PublicKey::from_bytes(bytes)
        .map_err(|_| Error::Format("The public key is not an X25519 key."))
}

fn private_key(bytes: &[u8; 32]) -> Result<<Kem as KemTrait>::PrivateKey> {
    <Kem as KemTrait>::PrivateKey::from_bytes(bytes)
        .map_err(|_| Error::Format("The private key is not an X25519 key."))
}

fn wrap_info(digest: &[u8; 32]) -> Vec<u8> {
    let mut info = WRAP_INFO.to_vec();
    info.extend_from_slice(digest);
    info
}

fn psk_id(user_key_id: &str) -> Vec<u8> {
    let mut id = PSK_ID_PREFIX.to_vec();
    id.extend_from_slice(user_key_id.as_bytes());
    id
}

/// The `sealed` wrap of `dek` to `system_key`, with the person's share.
pub fn wrap_sealed(vmk: &Vmk, digest: &[u8; 32], dek: &Dek, system_key: &[u8; 32]) -> Result<Wrap> {
    wrap_sealed_with(vmk, digest, dek, system_key, &mut OsRandom)
}

/// [`wrap_sealed`] with the randomness given (test vectors).
pub fn wrap_sealed_with<R: RngCore + CryptoRng>(
    vmk: &Vmk,
    digest: &[u8; 32],
    dek: &Dek,
    system_key: &[u8; 32],
    random: &mut R,
) -> Result<Wrap> {
    let share = vmk.user_share();
    let user_key = vmk.user_key_id();
    let id = psk_id(&user_key);
    let mode = OpModeS::<Kem>::Psk(PskBundle {
        psk: share.as_ref(),
        psk_id: &id,
    });
    let (enc, sealed) = hpke::single_shot_seal::<AesGcm256, HkdfSha256, Kem, _>(
        &mode,
        &public_key(system_key)?,
        &wrap_info(digest),
        dek.bytes(),
        digest,
        random,
    )
    .map_err(|_| Error::Refused("The sealed wrap could not be made."))?;
    Ok(Wrap::Sealed {
        system_key: system_key_id(system_key),
        user_key,
        suite: SUITE.into(),
        enc: b64(&enc.to_bytes()),
        dek: b64(&sealed),
    })
}

/// Open a `sealed` wrap. Needs the system key's private half **and** the
/// person's share; the attested workload calls this, never a storage host.
pub fn open_sealed(
    wrap: &Wrap,
    digest: &[u8; 32],
    system_key: &KeyPair,
    user_share: &[u8; 32],
) -> Result<Dek> {
    let Wrap::Sealed {
        system_key: id,
        user_key,
        suite,
        enc,
        dek,
    } = wrap
    else {
        return Err(Error::Refused("Only a sealed wrap opens here."));
    };
    if suite != SUITE {
        return Err(Error::Format("The sealed wrap's suite is unknown."));
    }
    if *id != system_key_id(&system_key.public) {
        return Err(Error::Decrypt("This wrap is for another system key."));
    }
    let psk = psk_id(user_key);
    let mode = OpModeR::<Kem>::Psk(PskBundle {
        psk: user_share,
        psk_id: &psk,
    });
    let enc = <Kem as KemTrait>::EncappedKey::from_bytes(&unb64_n::<32>(enc)?)
        .map_err(|_| Error::Format("The sealed wrap's key encapsulation is invalid."))?;
    let opened = Zeroizing::new(
        hpke::single_shot_open::<AesGcm256, HkdfSha256, Kem>(
            &mode,
            &private_key(system_key.private())?,
            &enc,
            &wrap_info(digest),
            &unb64(dek)?,
            digest,
        )
        .map_err(|_| {
            Error::Decrypt(
                "The sealed wrap did not open: wrong system key, wrong share, or another file.",
            )
        })?,
    );
    let bytes: [u8; 32] = opened
        .as_slice()
        .try_into()
        .map_err(|_| Error::Format("A wrapped data key has the wrong length."))?;
    Ok(Dek::from_bytes(bytes))
}

fn context_info(prefix: &[u8], binding: &[u8; 32]) -> Vec<u8> {
    let mut info = prefix.to_vec();
    info.extend_from_slice(binding);
    info
}

/// Seal a session request (`mode_base`) to the attested endpoint's key.
/// `endpoint_event` is the raw 32-byte id of the verified endpoint event.
pub fn seal_request<R: RngCore + CryptoRng>(
    endpoint_key: &[u8; 32],
    endpoint_event: &[u8; 32],
    plaintext: &[u8],
    random: &mut R,
) -> Result<([u8; 32], Vec<u8>)> {
    base_seal(
        endpoint_key,
        &context_info(REQUEST_INFO, endpoint_event),
        plaintext,
        random,
    )
}

/// Open a session request with the endpoint's private key.
pub fn open_request(
    endpoint: &KeyPair,
    endpoint_event: &[u8; 32],
    enc: &[u8; 32],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    base_open(
        endpoint,
        &context_info(REQUEST_INFO, endpoint_event),
        enc,
        sealed,
    )
}

/// Seal one reply to the client's `reply_to` key, bound to the lease id.
pub fn seal_reply<R: RngCore + CryptoRng>(
    reply_to: &[u8; 32],
    lease: &[u8; 32],
    plaintext: &[u8],
    random: &mut R,
) -> Result<([u8; 32], Vec<u8>)> {
    base_seal(
        reply_to,
        &context_info(REPLY_INFO, lease),
        plaintext,
        random,
    )
}

/// Open a reply with the client's reply key.
pub fn open_reply(
    reply: &KeyPair,
    lease: &[u8; 32],
    enc: &[u8; 32],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    base_open(reply, &context_info(REPLY_INFO, lease), enc, sealed)
}

fn base_seal<R: RngCore + CryptoRng>(
    recipient: &[u8; 32],
    info: &[u8],
    plaintext: &[u8],
    random: &mut R,
) -> Result<([u8; 32], Vec<u8>)> {
    let (enc, sealed) = hpke::single_shot_seal::<AesGcm256, HkdfSha256, Kem, _>(
        &OpModeS::Base,
        &public_key(recipient)?,
        info,
        plaintext,
        &[],
        random,
    )
    .map_err(|_| Error::Refused("The message could not be sealed."))?;
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&enc.to_bytes());
    Ok((bytes, sealed))
}

fn base_open(
    recipient: &KeyPair,
    info: &[u8],
    enc: &[u8; 32],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let enc = <Kem as KemTrait>::EncappedKey::from_bytes(enc)
        .map_err(|_| Error::Format("The key encapsulation is invalid."))?;
    hpke::single_shot_open::<AesGcm256, HkdfSha256, Kem>(
        &OpModeR::Base,
        &private_key(recipient.private())?,
        &enc,
        info,
        sealed,
        &[],
    )
    .map(Zeroizing::new)
    .map_err(|_| Error::Decrypt("The message did not open."))
}
